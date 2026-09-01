//! Zen `scapy` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_scapy_module(vm: &mut Vm) {
let scapy = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("checksum".into(), Value::NativeFunction("scapy_checksum".into())),
    ("ip".into(), Value::NativeFunction("scapy_ip".into())),
    ("tcp".into(), Value::NativeFunction("scapy_tcp".into())),
    ("udp".into(), Value::NativeFunction("scapy_udp".into())),
    ("icmp".into(), Value::NativeFunction("scapy_icmp".into())),
    ("raw".into(), Value::NativeFunction("scapy_raw".into())),
    ("build".into(), Value::NativeFunction("scapy_build".into())),
    ("parse".into(), Value::NativeFunction("scapy_parse".into())),
    ("send".into(), Value::NativeFunction("scapy_send".into())),
    ("sniff".into(), Value::NativeFunction("scapy_sniff".into())),
    ("ip_to_int".into(), Value::NativeFunction("scapy_ip_to_int".into())),
    ("int_to_ip".into(), Value::NativeFunction("scapy_int_to_ip".into())),
    ("cidr_expand".into(), Value::NativeFunction("scapy_cidr_expand".into())),
    ("subnet_hosts".into(), Value::NativeFunction("scapy_subnet_hosts".into())),
    ("ether".into(), Value::NativeFunction("scapy_ether".into())),
    ("arp".into(), Value::NativeFunction("scapy_arp".into())),
    ("sendp".into(), Value::NativeFunction("scapy_sendp".into())),
    ("sr1".into(), Value::NativeFunction("scapy_sr1".into())),
    ("srp1".into(), Value::NativeFunction("scapy_srp1".into())),
    ("syn_scan".into(), Value::NativeFunction("scapy_syn_scan".into())),
    ("handshake".into(), Value::NativeFunction("scapy_handshake".into())),
    ("arp_scan".into(), Value::NativeFunction("scapy_arp_scan".into())),
    ("src_mac".into(), Value::NativeFunction("scapy_src_mac".into())),
    ("src_ip".into(), Value::NativeFunction("scapy_src_ip".into())),
    ("hostname".into(), Value::NativeFunction("scapy_hostname".into())),
])));
vm.vars.insert("scapy".into(), scapy);
}
