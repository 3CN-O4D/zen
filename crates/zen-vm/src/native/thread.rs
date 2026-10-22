//! Native thread handlers for spawning and joining OS threads.

use crate::resource::Resource;
use crate::value::Value;
use crate::vm::{Vm, VmError};

/// Native handler: thread.spawn(function, payload)
pub fn native_thread_spawn(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let target_func = args.get(0).ok_or_else(|| VmError::TypeError("Expected function as first argument".into()))?;
    let payload = args.get(1).ok_or_else(|| VmError::TypeError("Expected argument payload".into()))?;

    let isolated_func = target_func
        .deep_clone_cross_isolate()
        .map_err(|e| VmError::RuntimeError(format!("Cannot send function across threads: {}", e)))?;
    let isolated_payload = payload
        .deep_clone_cross_isolate()
        .map_err(|e| VmError::RuntimeError(format!("Cannot send payload across threads: {}", e)))?;

    let handle = std::thread::spawn(move || -> Result<Value, String> {
        let mut thread_vm = Vm::new();
        match thread_vm.call(&isolated_func, &[isolated_payload]) {
            Ok(val) => Ok(val),
            Err(e) => Err(format!("{:?}", e)),
        }
    });

    let handle_id = vm.resources.insert(Resource::Thread(handle));
    Ok(Value::Int(handle_id as i64))
}

/// Native handler: thread.join(handle_id)
pub fn native_thread_join(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let handle_id = args
        .get(0)
        .and_then(|v| match v {
            Value::Int(i) => Some(*i as usize),
            _ => None,
        })
        .ok_or_else(|| VmError::TypeError("Handle ID must be an integer".into()))?;

    let resource = vm
        .resources
        .take(handle_id)
        .ok_or_else(|| VmError::RuntimeError("Invalid Thread Handle ID".into()))?;

    if let Resource::Thread(handle) = resource {
        match handle.join() {
            Ok(Ok(return_val)) => {
                let safe_return = return_val
                    .deep_clone_cross_isolate()
                    .map_err(|e| VmError::RuntimeError(format!("Failed to receive thread result: {}", e)))?;
                Ok(safe_return)
            }
            Ok(Err(e)) => Err(VmError::RuntimeError(format!("Thread execution failed: {}", e))),
            Err(_) => Err(VmError::RuntimeError("Thread panicked fatally (Rust panic)".into())),
        }
    } else {
        Err(VmError::TypeError("Handle is not a Thread".into()))
    }
}
