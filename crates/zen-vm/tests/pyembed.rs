//! End-to-end `pymport` against a real embedded CPython (libpython).
//! Standard-library modules only — no network, deterministic.
//!
//! Run with `cargo test -p zen-vm --features python`.
#![cfg(feature = "python")]

use std::sync::Arc;

use zen_compiler::compile_source;
use zen_vm::pybridge::{value_to_pyvalue, PyValue};
use zen_vm::pyembed::PythonBridge;
use zen_vm::{RefCountGc, Value, Vm};

fn run_py(src: &str) -> Result<Value, String> {
    let module =
        compile_source(src).unwrap_or_else(|e| panic!("compile failed: {e:?}\n{src}"));
    let mut vm = Vm::new(Box::new(RefCountGc));
    vm.py_bridge = Some(Arc::new(PythonBridge::new()));
    vm.run_module(&module.main, &module.children)
        .map_err(|e| format!("{e:?}"))
}

#[test]
fn real_json_roundtrip() {
    let v = run_py("pymport json\nreturn json.dumps({\"a\": 1})").unwrap();
    assert_eq!(v.as_str(), Some("{\"a\": 1}"));
}

#[test]
fn real_math_and_alias() {
    let v = run_py("pymport math as m\nreturn m.sqrt(16.0)").unwrap();
    assert_eq!(v.as_float(), Some(4.0));
    let v = run_py("pymport math\nreturn math.floor(2.9)").unwrap();
    assert_eq!(v.as_int(), Some(2));
}

#[test]
fn real_nested_structures() {
    let v = run_py("pymport json\nreturn json.loads(\"[1, \\\"a\\\", true, null, 1.5]\")").unwrap();
    let items = v.as_list().expect("list");
    assert_eq!(items.len(), 5);
    assert_eq!(items[0].as_int(), Some(1));
    assert_eq!(items[1].as_str(), Some("a"));
    assert_eq!(items[2].as_bool(), Some(true));
    assert!(items[3].is_nil());
    assert_eq!(items[4].as_float(), Some(1.5));
}

#[test]
fn real_tuple_becomes_list() {
    // `divmod` returns a tuple; tuples cross as lists (documented).
    let v = run_py("pymport builtins\nreturn builtins.divmod(7, 2)").unwrap();
    let items = v.as_list().expect("list");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].as_int(), Some(3));
    assert_eq!(items[1].as_int(), Some(1));
}

#[test]
fn real_bigint_promotes_to_float() {
    // Beyond i64: precision is best-effort (documented f64 fallback).
    let v = run_py("pymport builtins\nreturn builtins.int(\"1267650600228229401496703205376\")").unwrap();
    let f = v.as_float().expect("float fallback");
    assert!((f - 2f64.powi(100)).abs() < 1.0);
}

#[test]
fn real_none_is_nil() {
    let v = run_py("pymport json\nreturn json.loads(\"{}\").get(\"zzz\")").unwrap();
    assert!(v.is_nil());
}

#[test]
fn real_instance_attr_roundtrip() {
    // `types.SimpleNamespace` instances cross as opaque handles; attribute
    // writes and reads round-trip through the bridge.
    let v = run_py(
        "pymport types\nlet ns = types.SimpleNamespace()\nns.tag = \"x\"\nreturn ns.tag",
    )
    .unwrap();
    assert_eq!(v.as_str(), Some("x"));
}

#[test]
fn real_missing_module_errors() {
    let e = run_py("pymport no_such_module_xyz").unwrap_err();
    assert!(e.contains("ModuleNotFoundError"), "unexpected: {e}");
}

#[test]
fn real_missing_attr_errors() {
    let e = run_py("pymport math\nreturn math.nonexistent_attr_xyz").unwrap_err();
    assert!(e.contains("AttributeError"), "unexpected: {e}");
}

#[test]
fn real_calling_data_errors() {
    // `sys.version` is a string: calling it must fail, not coerce.
    let e = run_py("pymport sys\nreturn sys.version(1)").unwrap_err();
    assert!(e.contains("not callable"), "unexpected: {e}");
}

#[test]
fn real_int_keyed_dict_rejected() {
    // Dict keys crossing into Python must be strings — loudly, not lossy.
    let e = run_py("pymport json\nreturn json.dumps({1: \"a\"})").unwrap_err();
    assert!(e.contains("Marshal"), "wrong code: {e}");
    assert!(e.contains("must be strings"), "unexpected: {e}");
}

#[test]
fn real_closure_arg_rejected() {
    let e = run_py("pymport json\nlet f = fn(x) { return x }\nreturn json.dumps(f)").unwrap_err();
    assert!(e.contains("cannot marshal"), "unexpected: {e}");
}

#[test]
fn marshal_unit_bytes_and_handles() {    // Pure unit coverage (no VM needed): bytes cross by value, foreign
    // handles pass straight through the registry.
    let b = Value::bytes(vec![104, 105]);
    assert_eq!(
        value_to_pyvalue(b),
        Ok(PyValue::Bytes(vec![104, 105])),
        "bytes must cross by value"
    );
    let back = zen_vm::pybridge::pyvalue_to_value(PyValue::Bytes(vec![104, 105]));
    assert_eq!(back.as_bytes(), Some([104u8, 105].as_slice()));
    let h = zen_vm::gc::new_foreign(7);
    assert_eq!(value_to_pyvalue(h), Ok(PyValue::Handle(7)));
}

/// Third-party validation through `paramiko` (SSH), when importable.
/// Skips cleanly otherwise — no network, no server: `SSHConfig.from_text`
/// is pure parsing and `RSAKey.generate` is local CPU work. Assertions cover
/// shape (names, bits, dict contents), never random key material.
fn try_paramiko() -> Option<()> {
    let probe = run_py("pymport paramiko\nreturn paramiko.__version__");
    match probe {
        Ok(_) => Some(()),
        Err(e) if e.contains("ModuleNotFoundError") || e.contains("No module named") => {
            eprintln!("skipping paramiko tests: {e}");
            None
        }
        Err(e) => panic!("unexpected paramiko probe failure: {e}"),
    }
}

#[test]
fn real_paramiko_sshconfig() {
    if try_paramiko().is_none() {
        return;
    }
    let v = run_py(
        "pymport paramiko\n\
         let cfg = paramiko.SSHConfig.from_text(\"Host db\\n    HostName 10.0.0.5\\n    User admin\\n\")\n\
         let h = cfg.lookup(\"db\")\n\
         return h[\"hostname\"] + \"/\" + h[\"user\"]",
    )
    .unwrap();
    assert_eq!(v.as_str(), Some("10.0.0.5/admin"));
}

#[test]
fn real_paramiko_rsakey() {
    if try_paramiko().is_none() {
        return;
    }
    let v = run_py(
        "pymport paramiko\n\
         let key = paramiko.RSAKey.generate(1024)\n\
         return [key.get_name(), key.get_bits()]",
    )
    .unwrap();
    let items = v.as_list().expect("list");
    assert_eq!(items[0].as_str(), Some("ssh-rsa"));
    assert_eq!(items[1].as_int(), Some(1024));
}

#[test]
fn real_paramiko_fingerprint_bytes() {
    if try_paramiko().is_none() {
        return;
    }
    // Key fingerprints cross as Zen `bytes` (16-byte MD5).
    let v = run_py(
        "pymport paramiko\n\
         let key = paramiko.RSAKey.generate(1024)\n\
         return key.get_fingerprint()",
    )
    .unwrap();
    assert_eq!(v.as_bytes().map(|b| b.len()), Some(16));
}
