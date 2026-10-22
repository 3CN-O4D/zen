//! Differential `pymport`: the same scripts through the tree-walking
//! interpreter and the register VM, sharing a mock bridge. Both engines must
//! agree on values and on failures.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zen_compiler::compile_source;
use zen_vm::pybridge::{PyBridge, PyBridgeError, PyValue};
use zen_vm::{RefCountGc, Vm};

struct MockBridge {
    store: Mutex<HashMap<(u32, String), PyValue>>,
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
            (1, "answer") => Ok(PyValue::Int(42)),
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
            _ => Err(PyBridgeError::new(
                "call",
                format!("mock cannot call handle {handle}"),
            )),
        }
    }
}

fn bridge() -> Arc<dyn PyBridge> {
    Arc::new(MockBridge {
        store: Mutex::new(HashMap::new()),
    })
}

fn interp_out(src: &str) -> Result<String, String> {
    let mut interp = zen_interp::Interp::new();
    interp.py_bridge = Some(bridge());
    match interp.run_source(src) {
        Ok(_) => Ok(interp.take_output()),
        Err(e) => Err(e),
    }
}

fn vm_out(src: &str) -> Result<String, String> {
    let module =
        compile_source(src).unwrap_or_else(|e| panic!("compile failed: {e:?}\n{src}"));
    let mut vm = Vm::new(Box::new(RefCountGc));
    vm.py_bridge = Some(bridge());
    match vm.run_module(&module.main, &module.children) {
        Ok(_) => Ok(vm.take_output()),
        Err(e) => Err(e.to_string()),
    }
}

fn assert_parity(src: &str, expect: &str) {
    let i = interp_out(src).unwrap_or_else(|e| panic!("interp failed: {e}\n{src}"));
    let v = vm_out(src).unwrap_or_else(|e| panic!("vm failed: {e}\n{src}"));
    assert_eq!(i, v, "engine divergence on:\n{src}");
    assert_eq!(i, expect, "wrong output on:\n{src}");
}

#[test]
fn both_read_attributes() {
    assert_parity("pymport mockmath\nprint mockmath.answer\n", "42\n");
}

#[test]
fn both_call_methods() {
    assert_parity("pymport mockmath\nprint mockmath.double(21)\n", "42\n");
}

#[test]
fn both_roundtrip_setattr() {
    assert_parity(
        "pymport mockmath\nmockmath.tag = \"x\"\nprint mockmath.tag\n",
        "x\n",
    );
}

#[test]
fn both_fail_missing_module() {
    let e1 = interp_out("pymport nope\n").unwrap_err();
    let e2 = vm_out("pymport nope\n").unwrap_err();
    assert!(e1.contains("No module named"), "interp: {e1}");
    assert!(e2.contains("No module named"), "vm: {e2}");
}

#[test]
fn both_fail_missing_attr() {
    let e1 = interp_out("pymport mockmath\nprint mockmath.zzz\n").unwrap_err();
    let e2 = vm_out("pymport mockmath\nprint mockmath.zzz\n").unwrap_err();
    assert!(e1.contains("zzz"), "interp: {e1}");
    assert!(e2.contains("zzz"), "vm: {e2}");
}

#[test]
fn both_fail_unlinked() {
    let src = "pymport mockmath\n";
    let mut interp = zen_interp::Interp::new();
    let e1 = match interp.run_source(src) {
        Ok(_) => panic!("interp should fail without a bridge"),
        Err(e) => e,
    };
    let module = compile_source(src).unwrap();
    let mut vm = Vm::new(Box::new(RefCountGc));
    let e2 = vm
        .run_module(&module.main, &module.children)
        .unwrap_err()
        .to_string();
    assert!(e1.contains("linked Python bridge"), "interp: {e1}");
    assert!(e2.contains("linked Python bridge"), "vm: {e2}");
}
