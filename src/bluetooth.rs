//! Zen `bluetooth` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_bluetooth_module(vm: &mut Vm) {
let bluetooth = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("status".into(), Value::NativeFunction("bt_status".into())),
    ("power".into(), Value::NativeFunction("bt_power".into())),
    ("scan".into(), Value::NativeFunction("bt_scan".into())),
    ("scan_stop".into(), Value::NativeFunction("bt_scan_stop".into())),
    ("devices".into(), Value::NativeFunction("bt_devices".into())),
    ("pair".into(), Value::NativeFunction("bt_pair".into())),
    ("unpair".into(), Value::NativeFunction("bt_unpair".into())),
    ("connect".into(), Value::NativeFunction("bt_connect".into())),
    ("disconnect".into(), Value::NativeFunction("bt_disconnect".into())),
    ("trust".into(), Value::NativeFunction("bt_trust".into())),
    ("send".into(), Value::NativeFunction("bt_send".into())),
])));
vm.vars.insert("bluetooth".into(), bluetooth);
}
