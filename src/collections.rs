//! Zen `collections` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;

pub fn init_collections_module(vm: &mut Vm) {
let collections = Value::Dict(BTreeMap::from([
    ("Counter".into(), Value::NativeFunction("collections_counter".into())),
    ("chain".into(), Value::NativeFunction("collections_chain".into())),
    ("flatten".into(), Value::NativeFunction("collections_flatten".into())),
]));
vm.vars.insert("collections".into(), collections);
}
