use crate::value::Value;
use crate::vm::{Vm, VmError};

pub fn native_tls_connect(_vm: &mut Vm, _args: &[Value]) -> Result<Value, VmError> {
    Err(VmError::RuntimeError("TLS support stub - needs rustls integration".into()))
}
