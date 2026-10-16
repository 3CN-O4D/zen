
pub fn native_raw_bind(_vm: &mut Vm, _args: &[Value]) -> Result<Value, VmError> {
    Err(VmError::RuntimeError("raw bind stub".into()))
}

pub fn native_raw_recv(_vm: &mut Vm, _args: &[Value]) -> Result<Value, VmError> {
    Err(VmError::RuntimeError("raw recv stub".into()))
}

pub fn native_raw_close(_vm: &mut Vm, _args: &[Value]) -> Result<Value, VmError> {
    Err(VmError::RuntimeError("raw close stub".into()))
}

pub fn native_craft_ipv4(_vm: &mut Vm, _args: &[Value]) -> Result<Value, VmError> {
    Err(VmError::RuntimeError("craft ipv4 stub".into()))
}

pub fn native_craft_tcp(_vm: &mut Vm, _args: &[Value]) -> Result<Value, VmError> {
    Err(VmError::RuntimeError("craft tcp stub".into()))
}
