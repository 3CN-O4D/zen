use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use crate::resource::Resource;
use crate::value::Value;
use crate::vm::{Vm, VmError};

pub fn native_tcp_connect(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let addr = args.get(0).and_then(|v| v.as_string())
        .ok_or_else(|| VmError::TypeError("Address must be a string".into()))?;

    let stream = TcpStream::connect(addr.as_str())
        .map_err(|e| VmError::RuntimeError(format!("Connection failed: {}", e)))?;

    stream.set_read_timeout(Some(Duration::from_secs(3))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(3))).ok();

    let handle_id = vm.resources.insert(Resource::TcpStream(stream));
    Ok(Value::Int(handle_id as i64))
}

pub fn native_tcp_write(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let handle_id = args.get(0).and_then(|v| match v {
        Value::Int(i) => Some(*i as usize),
        _ => None,
    }).ok_or_else(|| VmError::TypeError("Handle ID must be an integer".into()))?;

    let data = args.get(1).and_then(|v| v.as_string())
        .ok_or_else(|| VmError::TypeError("Data must be a string".into()))?;

    let resource = vm.resources.get_mut(handle_id)
        .ok_or_else(|| VmError::RuntimeError("Invalid TCP Stream Handle".into()))?;

    if let Resource::TcpStream(stream) = resource {
        stream.write_all(data.as_bytes())
            .map_err(|e| VmError::RuntimeError(format!("Write failed: {}", e)))?;
        Ok(Value::nil())
    } else {
        Err(VmError::TypeError("Handle is not a TcpStream".into()))
    }
}

pub fn native_tcp_read(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let handle_id = args.get(0).and_then(|v| match v {
        Value::Int(i) => Some(*i as usize),
        _ => None,
    }).ok_or_else(|| VmError::TypeError("Handle ID must be an integer".into()))?;

    let max_bytes = args.get(1).and_then(|v| match v {
        Value::Int(i) => Some(*i as usize),
        _ => None,
    }).unwrap_or(1024);

    let resource = vm.resources.get_mut(handle_id)
        .ok_or_else(|| VmError::RuntimeError("Invalid TCP Stream Handle".into()))?;

    if let Resource::TcpStream(stream) = resource {
        let mut buf = vec![0u8; max_bytes];
        match stream.read(&mut buf) {
            Ok(0) => Ok(Value::string(Arc::new(String::new()))),
            Ok(n) => {
                buf.truncate(n);
                let text = String::from_utf8_lossy(&buf).into_owned();
                Ok(Value::string(Arc::new(text)))
            }
            Err(e) => Err(VmError::RuntimeError(format!("Read failed: {}", e))),
        }
    } else {
        Err(VmError::TypeError("Handle is not a TcpStream".into()))
    }
}

pub fn native_tcp_listen(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let addr = args.get(0).and_then(|v| v.as_string())
        .ok_or_else(|| VmError::TypeError("Address must be a string".into()))?;

    let listener = TcpListener::bind(addr.as_str())
        .map_err(|e| VmError::RuntimeError(format!("Bind failed: {}", e)))?;

    let handle_id = vm.resources.insert(Resource::TcpListener(listener));
    Ok(Value::Int(handle_id as i64))
}

pub fn native_tcp_accept(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let handle_id = args.get(0).and_then(|v| match v {
        Value::Int(i) => Some(*i as usize),
        _ => None,
    }).ok_or_else(|| VmError::TypeError("Handle ID must be an integer".into()))?;

    let resource = vm.resources.get_mut(handle_id)
        .ok_or_else(|| VmError::RuntimeError("Invalid TCP Listener Handle".into()))?;

    if let Resource::TcpListener(listener) = resource {
        let (stream, _addr) = listener.accept()
            .map_err(|e| VmError::RuntimeError(format!("Accept failed: {}", e)))?;
        stream.set_read_timeout(Some(Duration::from_secs(3))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(3))).ok();
        let stream_id = vm.resources.insert(Resource::TcpStream(stream));
        Ok(Value::Int(stream_id as i64))
    } else {
        Err(VmError::TypeError("Handle is not a TcpListener".into()))
    }
}

pub fn native_udp_bind(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let addr = args.get(0).and_then(|v| v.as_string())
        .ok_or_else(|| VmError::TypeError("Address must be a string".into()))?;

    let socket = UdpSocket::bind(addr.as_str())
        .map_err(|e| VmError::RuntimeError(format!("Bind failed: {}", e)))?;

    let handle_id = vm.resources.insert(Resource::UdpSocket(socket));
    Ok(Value::Int(handle_id as i64))
}

pub fn native_udp_send(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let handle_id = args.get(0).and_then(|v| match v {
        Value::Int(i) => Some(*i as usize),
        _ => None,
    }).ok_or_else(|| VmError::TypeError("Handle ID must be an integer".into()))?;

    let data = args.get(1).and_then(|v| v.as_string())
        .ok_or_else(|| VmError::TypeError("Data must be a string".into()))?;

    let addr = args.get(2).and_then(|v| v.as_string())
        .ok_or_else(|| VmError::TypeError("Address must be a string".into()))?;

    let resource = vm.resources.get_mut(handle_id)
        .ok_or_else(|| VmError::RuntimeError("Invalid UDP Socket Handle".into()))?;

    if let Resource::UdpSocket(socket) = resource {
        let bytes_sent = socket.send_to(data.as_bytes(), addr.as_str())
            .map_err(|e| VmError::RuntimeError(format!("Send failed: {}", e)))?;
        Ok(Value::Int(bytes_sent as i64))
    } else {
        Err(VmError::TypeError("Handle is not a UdpSocket".into()))
    }
}

pub fn native_udp_recv(vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let handle_id = args.get(0).and_then(|v| match v { Value::Int(i) => Some(*i as usize), _ => None }).ok_or_else(|| VmError::TypeError("Handle".into()))?;
    let max_bytes = args.get(1).and_then(|v| match v { Value::Int(i) => Some(*i as usize), _ => None }).unwrap_or(1024);
    let resource = vm.resources.get_mut(handle_id).ok_or_else(|| VmError::RuntimeError("Invalid".into()))?;
    match resource {
        Resource::UdpSocket(socket) => {
            let mut buf = vec![0u8; max_bytes];
            match socket.recv_from(&mut buf) {
                Ok((n, _addr)) => {
                    buf.truncate(n);
                    let t = String::from_utf8_lossy(&buf).into_owned();
                    Ok(Value::string(Arc::new(t)))
                }
                Err(e) => Err(VmError::RuntimeError(format!("recv: {}", e))),
            }
        }
        _ => Err(VmError::TypeError("not udp".into())),
    }
}
