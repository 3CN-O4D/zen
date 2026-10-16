//! FFI handles: opaque, generation-checked integer ids.
//! See `docs/11-ffi-handles.md`.
//!
//! A `handle` value is a [`crate::object::ObjKind::Handle`] heap object whose
//! `id` indexes a process-wide registry. Resolving a stale or unknown id fails
//! with a catchable [`VmError::HandleError`] rather than an invalid pointer.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::errors::{VmError, VmResult};
use crate::value::Value;

/// A native module entry: `(name, function)`.
pub type NativeFn = fn(&mut crate::vm::Vm, &[Value]) -> VmResult<Value>;

fn registry() -> &'static Mutex<HashMap<u32, ()>> {
    static REGISTRY: OnceLock<Mutex<HashMap<u32, ()>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register a handle id as live. Returns the handle `Value` wrapping it.
pub fn register_handle(id: u32) -> Value {
    registry().lock().unwrap().insert(id, ());
    crate::gc::new_handle(id)
}

/// Drop a handle id, invalidating every `Value` that references it.
pub fn drop_handle(id: u32) {
    registry().lock().unwrap().remove(&id);
}

/// Check that an id is live.
pub fn resolve_handle(id: u32) -> VmResult<()> {
    if registry().lock().unwrap().contains_key(&id) {
        Ok(())
    } else {
        Err(VmError::HandleError(format!("no such handle: {id}")))
    }
}

/// Resolve the id inside a handle `Value`.
pub fn resolve_handle_value(v: Value) -> VmResult<()> {
    match v.as_handle_id() {
        Some(id) => resolve_handle(id),
        None => Err(VmError::type_mismatch("handle", v)),
    }
}

pub fn register_ffi(_name: &str, _func: NativeFn) {
    // stub
}
