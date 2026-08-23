//! Zen `telnet` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;

pub fn init_telnet_module(vm: &mut Vm) {
let telnet = Value::Dict(BTreeMap::from([
    ("connect".into(), Value::NativeFunction("telnet_connect".into())),
    ("write".into(), Value::NativeFunction("telnet_write".into())),
    ("read".into(), Value::NativeFunction("telnet_read".into())),
    ("read_until".into(), Value::NativeFunction("telnet_read_until".into())),
    ("close".into(), Value::NativeFunction("telnet_close".into())),
]));
vm.vars.insert("telnet".into(), telnet);
}
