//! Zen `scapy` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn init_scapy_module(vm: &mut Vm) {
let scapy = Value::Dict(Arc::new(BTreeMap::from([
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
])));
vm.vars.insert("scapy".into(), scapy);
}
