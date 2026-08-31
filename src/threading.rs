//! Zen `threading` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_threading_module(vm: &mut Vm) {
let threading = Value::Dict(Arc::new(ahash::AHashMap::from([
    ("start".into(), Value::NativeFunction("threading_start".into())),
    ("join".into(), Value::NativeFunction("threading_join".into())),
])));
vm.vars.insert("threading".into(), threading);
}
