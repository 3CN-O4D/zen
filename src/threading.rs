//! Zen `threading` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;

pub fn init_threading_module(vm: &mut Vm) {
let threading = Value::Dict(BTreeMap::from([
    ("start".into(), Value::NativeFunction("threading_start".into())),
]));
vm.vars.insert("threading".into(), threading);
}
