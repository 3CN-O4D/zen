//! `pymport` through a mock bridge: import, attribute access, calls, and
//! marshaling — without libpython (runs in every build configuration). The
//! mock models the bridge contract: primitives cross by value, callables
//! cross as handles, and anything unmarshalable is an error rather than a
//! coercion.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zen_compiler::compile_source;
use zen_vm::pybridge::{PyBridge, PyBridgeError, PyValue};
use zen_vm::{RefCountGc, Value, Vm};

struct MockBridge {
    store: Mutex<HashMap<(u32, String), PyValue>>,
}

impl MockBridge {
    fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }
}

impl PyBridge for MockBridge {
    fn dialect_id(&self) -> &'static str {
        "mock"
    }

    fn import_module(&self, name: &str) -> Result<u32, PyBridgeError> {
        match name {
            "mockmath" => Ok(1),
            _ => Err(PyBridgeError::new(
                "import",
                format!("No module named '{name}'"),
            )),
        }
    }

    fn getattr(&self, handle: u32, attr: &str) -> Result<PyValue, PyBridgeError> {
        if let Some(v) = self.store.lock().unwrap().get(&(handle, attr.to_string())) {
            return Ok(v.clone());
        }
        match (handle, attr) {
            (1, "double") => Ok(PyValue::Handle(2)),
            (1, "greet") => Ok(PyValue::Handle(3)),
            (1, "answer") => Ok(PyValue::Int(42)),
            (1, "name") => Ok(PyValue::Str("mockmath".to_string())),
            _ => Err(PyBridgeError::new(
                "getattr",
                format!("mock object {handle} has no attribute '{attr}'"),
            )),
        }
    }

    fn setattr(&self, handle: u32, attr: &str, value: PyValue) -> Result<(), PyBridgeError> {
        self.store
            .lock()
            .unwrap()
            .insert((handle, attr.to_string()), value);
        Ok(())
    }

    fn call(&self, handle: u32, args: Vec<PyValue>) -> Result<PyValue, PyBridgeError> {
        match (handle, args.as_slice()) {
            (2, [PyValue::Int(x)]) => Ok(PyValue::Int(x * 2)),
            (3, [PyValue::Str(s)]) => Ok(PyValue::Str(format!("hi {s}"))),
            _ => Err(PyBridgeError::new(
                "call",
                format!("mock cannot call handle {handle}"),
            )),
        }
    }
}

fn run_with_bridge(src: &str) -> Result<Value, String> {
    let module =
        compile_source(src).unwrap_or_else(|e| panic!("compile failed: {e:?}\n{src}"));
    let mut vm = Vm::new(Box::new(RefCountGc));
    vm.py_bridge = Some(Arc::new(MockBridge::new()));
    vm.run_module(&module.main, &module.children)
        .map_err(|e| format!("{e:?}"))
}

fn run_plain(src: &str) -> Result<Value, String> {
    let module =
        compile_source(src).unwrap_or_else(|e| panic!("compile failed: {e:?}\n{src}"));
    let mut vm = Vm::new(Box::new(RefCountGc));
    vm.run_module(&module.main, &module.children)
        .map_err(|e| format!("{e:?}"))
}

#[test]
fn pymport_binds_and_reads_attributes() {
    let v = run_with_bridge("pymport mockmath\nreturn mockmath.answer").unwrap();
    assert_eq!(v.as_int(), Some(42));
    let v = run_with_bridge("pymport mockmath\nreturn mockmath.name").unwrap();
    assert_eq!(v.as_str(), Some("mockmath"));
}

#[test]
fn pymport_method_call_fuses_through_call_attr() {
    // `mockmath.double(21)` is a CallMethod on the module handle: the default
    // `call_attr` threading resolves then calls.
    let v = run_with_bridge("pymport mockmath\nreturn mockmath.double(21)").unwrap();
    assert_eq!(v.as_int(), Some(42));
}

#[test]
fn pymport_callable_handle_direct_call() {
    // A resolved callable stays a handle; calling it goes straight to `call`.
    let v = run_with_bridge("pymport mockmath\nlet f = mockmath.greet\nreturn f(\"bob\")").unwrap();
    assert_eq!(v.as_str(), Some("hi bob"));
}

#[test]
fn pymport_setattr_roundtrip() {
    let v = run_with_bridge("pymport mockmath\nmockmath.tag = \"x\"\nreturn mockmath.tag").unwrap();
    assert_eq!(v.as_str(), Some("x"));
}

#[test]
fn pymport_missing_module_errors_cleanly() {
    let e = run_with_bridge("pymport nope").unwrap_err();
    assert!(e.contains("PyImport"), "wrong code: {e}");
    assert!(e.contains("No module named"), "unexpected: {e}");
}

#[test]
fn pymport_missing_attr_errors_cleanly() {
    let e = run_with_bridge("pymport mockmath\nreturn mockmath.zzz").unwrap_err();
    assert!(e.contains("ForeignAttribute"), "wrong code: {e}");
    assert!(e.contains("no attribute"), "unexpected: {e}");
}

#[test]
fn pymport_calling_data_errors_cleanly() {
    // `answer` is an int, not a callable: the default `call_attr` threading
    // reports the type error instead of calling into nothing.
    let e = run_with_bridge("pymport mockmath\nreturn mockmath.answer(1)").unwrap_err();
    assert!(e.contains("ForeignCall"), "wrong code: {e}");
    assert!(e.contains("not callable") || e.contains("is an int"), "unexpected: {e}");
}

#[test]
fn pymport_unlinked_errors_actionably() {
    let e = run_plain("pymport mockmath").unwrap_err();
    assert!(e.contains("PyImport"), "wrong code: {e}");
    assert!(e.contains("linked Python bridge"), "unexpected: {e}");
}

#[test]
fn marshal_rejects_non_values() {
    use zen_vm::pybridge::value_to_pyvalue;
    // Instances, funcs, and friends never cross by coercion.
    let module =
        compile_source("class P {}\nvar p = new P()\nreturn p").unwrap();
    let mut vm = Vm::new(Box::new(RefCountGc));
    let v = vm
        .run_module(&module.main, &module.children)
        .unwrap();
    assert!(value_to_pyvalue(v).is_err());
    // Primitives round-trip exactly.
    assert_eq!(
        zen_vm::pybridge::pyvalue_to_value(PyValue::Int(7)).as_int(),
        Some(7)
    );
    assert_eq!(
        zen_vm::pybridge::pyvalue_to_value(PyValue::Str("s".into())).as_str(),
        Some("s")
    );
}
