use std::io::{BufRead, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
use crate::resource::Resource;
use crate::value::Value;
use crate::vm::{Vm, VmError};

pub fn native_process_spawn(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let cmd = args.get(0).and_then(|v| v.as_string()).ok_or_else(|| VmError::TypeError("Command must be a string".into()))?;
    let args_list = args.get(1).ok_or_else(|| VmError::TypeError("Args must be a list".into()))?;
    let args_vec = args_list.as_list().ok_or_else(|| VmError::TypeError("Args must be a list".into()))?;

    let mut rust_args = Vec::new();
    for arg in args_vec.iter() {
        if let Some(s) = arg.as_string() {
            rust_args.push(s.to_string());
        }
    }

    let mut child = Command::new(cmd.as_str())
        .args(&rust_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| VmError::RuntimeError(format!("Process spawn failed: {}", e)))?;

    let stdout_reader = child.stdout.take().map(std::io::BufReader::new);
    let stdin_writer = child.stdin.take();

    let id = vm.resources.insert(Resource::Process(child, stdout_reader, stdin_writer));
    Ok(Value::Int(id as i64))
}

pub fn native_process_read_line(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let id = args.get(0).and_then(|v| v.as_int()).ok_or_else(|| VmError::TypeError("Handle must be integer".into()))? as usize;
    let resource = vm.resources.get_mut(id).ok_or_else(|| VmError::RuntimeError("Invalid Process Handle".into()))?;

    match resource {
        Resource::Process(_, Some(reader), _) => {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => Ok(Value::nil()),
                Ok(_) => Ok(Value::string(Arc::new(line))),
                Err(e) => Err(VmError::RuntimeError(format!("Read error: {}", e))),
            }
        }
        _ => Err(VmError::RuntimeError("Process stdout unavailable".into())),
    }
}

pub fn native_process_write(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let id = args.get(0).and_then(|v| v.as_int()).ok_or_else(|| VmError::TypeError("Handle must be integer".into()))? as usize;
    let data = args.get(1).and_then(|v| v.as_string()).ok_or_else(|| VmError::TypeError("Data must be string".into()))?;
    let resource = vm.resources.get_mut(id).ok_or_else(|| VmError::RuntimeError("Invalid Process Handle".into()))?;

    match resource {
        Resource::Process(_, _, Some(stdin)) => {
            stdin.write_all(data.as_bytes())
                .and_then(|_| stdin.flush())
                .map_err(|e| VmError::RuntimeError(format!("Write error: {}", e)))?;
            Ok(Value::nil())
        }
        _ => Err(VmError::RuntimeError("Process stdin unavailable".into())),
    }
}

pub fn native_process_wait(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let id = args.get(0).and_then(|v| v.as_int()).ok_or_else(|| VmError::TypeError("Handle must be integer".into()))? as usize;
    let resource = vm.resources.take(id).ok_or_else(|| VmError::RuntimeError("Invalid Process Handle".into()))?;

    match resource {
        Resource::Process(mut child, _, _) => {
            let status = child.wait().map_err(|e| VmError::RuntimeError(format!("Wait error: {}", e)))?;
            Ok(Value::Int(status.code().unwrap_or(-1) as i64))
        }
        _ => Err(VmError::TypeError("Handle is not a Process".into())),
    }
}
