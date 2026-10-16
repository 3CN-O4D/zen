use std::net::Ipv4Addr;
use std::sync::Arc;
use crate::value::Value;
use crate::vm::{Vm, VmError};

pub fn native_pack_ip(_vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let src_str = args.get(0).and_then(|v| v.as_string())
        .ok_or_else(|| VmError::TypeError("Source IP must be a string".into()))?;
    let dst_str = args.get(1).and_then(|v| v.as_string())
        .ok_or_else(|| VmError::TypeError("Destination IP must be a string".into()))?;
    let proto = args.get(2).and_then(|v| v.as_int())
        .ok_or_else(|| VmError::TypeError("Protocol must be an integer".into()))? as u8;

    let src_ip: Ipv4Addr = src_str.parse()
        .map_err(|_| VmError::RuntimeError("Invalid source IPv4 address".into()))?;
    let dst_ip: Ipv4Addr = dst_str.parse()
        .map_err(|_| VmError::RuntimeError("Invalid destination IPv4 address".into()))?;

    let mut packet = vec![0u8; 20];
    packet[0] = 0x45;
    packet[1] = 0x00;
    let total_len: u16 = 20;
    packet[2..4].copy_from_slice(&total_len.to_be_bytes());
    packet[4..6].copy_from_slice(&0u16.to_be_bytes());
    packet[6..8].copy_from_slice(&0u16.to_be_bytes());
    packet[8] = 64;
    packet[9] = proto;
    packet[10..12].copy_from_slice(&0u16.to_be_bytes());
    packet[12..16].copy_from_slice(&src_ip.octets());
    packet[16..20].copy_from_slice(&dst_ip.octets());

    let s = String::from_utf8_lossy(&packet).into_owned();
    Ok(Value::string(Arc::new(s)))
}

pub fn native_pack_tcp(_vm: &mut Vm, args: &[Value]) -> Result<Value, VmError> {
    let sport = args.get(0).and_then(|v| v.as_int()).unwrap_or(0) as u16;
    let dport = args.get(1).and_then(|v| v.as_int()).unwrap_or(0) as u16;
    let flags_str = args.get(2).and_then(|v| v.as_string()).unwrap_or_else(|| Arc::new(String::new()));

    let mut flags: u8 = 0;
    let fs = flags_str.as_str();
    if fs.contains("FIN") { flags |= 0x01; }
    if fs.contains("SYN") { flags |= 0x02; }
    if fs.contains("RST") { flags |= 0x04; }
    if fs.contains("PSH") { flags |= 0x08; }
    if fs.contains("ACK") { flags |= 0x10; }
    if fs.contains("URG") { flags |= 0x20; }

    let mut packet = vec![0u8; 20];
    packet[0..2].copy_from_slice(&sport.to_be_bytes());
    packet[2..4].copy_from_slice(&dport.to_be_bytes());
    packet[4..8].copy_from_slice(&0u32.to_be_bytes());
    packet[8..12].copy_from_slice(&0u32.to_be_bytes());
    packet[12] = 0x50;
    packet[13] = flags;
    packet[14..16].copy_from_slice(&65535u16.to_be_bytes());
    packet[16..18].copy_from_slice(&0u16.to_be_bytes());
    packet[18..20].copy_from_slice(&0u16.to_be_bytes());

    let s = String::from_utf8_lossy(&packet).into_owned();
    Ok(Value::string(Arc::new(s)))
}
