//! Zen `wifi` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_wifi_module(vm: &mut Vm) {
let wifi = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("scan".into(), Value::NativeFunction("wifi_scan".into())),
    ("status".into(), Value::NativeFunction("wifi_status".into())),
    ("connect".into(), Value::NativeFunction("wifi_connect".into())),
    ("disconnect".into(), Value::NativeFunction("wifi_disconnect".into())),
    ("forget".into(), Value::NativeFunction("wifi_forget".into())),
    ("interfaces".into(), Value::NativeFunction("wifi_interfaces".into())),
    ("list".into(), Value::NativeFunction("wifi_list".into())),
])));
vm.vars.insert("wifi".into(), wifi);
}
