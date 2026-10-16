//! Module system stub.
//! See `docs/14-modules-and-packages.md`.

use crate::value::Value;
use crate::vm::VmResult;

/// A native module entry: `(name, function)`.
pub type NativeFn = fn(&mut crate::vm::Vm, &[Value]) -> VmResult<Value>;

pub fn register_native_module(_name: &str, _funcs: &[(&str, NativeFn)]) {
    // stub
}
