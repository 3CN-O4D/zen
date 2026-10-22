use std::sync::{Arc, RwLock};
use crate::value::Value;
use crate::vm::{Vm, VmError};

pub fn native_shared_create(_vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let initial = args.get(0).unwrap_or(&Value::nil());
    let safe_val = initial
        .deep_clone_cross_isolate()
        .map_err(|e| VmError::RuntimeError(e))?;
    Ok(Value::shared_box(Arc::new(RwLock::new(safe_val))))
}

pub fn native_shared_get(_vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    if let Some(v) = args.get(0) {
        if let Some(lock) = v.as_shared_box() {
            let guard = lock.read().map_err(|_| VmError::RuntimeError("SharedBox read lock poisoned".into()))?;
            return guard.deep_clone_cross_isolate().map_err(|e| VmError::RuntimeError(e));
        }
    }
    Err(VmError::TypeError("Expected SharedBox".into()))
}

pub fn native_shared_set(_vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let box_val = args.get(0).ok_or_else(|| VmError::TypeError("Expected SharedBox".into()))?;
    let new_val = args.get(1).ok_or_else(|| VmError::TypeError("Expected value".into()))?;
    let safe_val = new_val.deep_clone_cross_isolate().map_err(|e| VmError::RuntimeError(e))?;

    if let Some(lock) = box_val.as_shared_box() {
        let mut guard = lock.write().map_err(|_| VmError::RuntimeError("SharedBox write lock poisoned".into()))?;
        *guard = safe_val;
        Ok(Value::nil())
    } else {
        Err(VmError::TypeError("Expected SharedBox".into()))
    }
}

pub fn native_shared_increment(_vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let box_val = args.get(0).ok_or_else(|| VmError::TypeError("Expected SharedBox".into()))?;
    let amount = match args.get(1) {
        Some(v) => v.as_int().ok_or_else(|| VmError::TypeError("Increment amount must be integer".into()))?,
        None => 1,
    };

    if let Some(lock) = box_val.as_shared_box() {
        let mut guard = lock.write().map_err(|_| VmError::RuntimeError("SharedBox write lock poisoned".into()))?;
        match &mut *guard {
            Value::Int(ref mut val) => {
                *val += amount;
                Ok(Value::Int(*val))
            }
            _ => Err(VmError::TypeError("SharedBox contents is not an integer".into())),
        }
    } else {
        Err(VmError::TypeError("Expected SharedBox".into()))
    }
}
