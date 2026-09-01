//! Zen `collections` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_collections_module(vm: &mut Vm) {
let collections = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("Counter".into(), Value::NativeFunction("collections_counter".into())),
    ("chain".into(), Value::NativeFunction("collections_chain".into())),
    ("flatten".into(), Value::NativeFunction("collections_flatten".into())),
])));
vm.vars.insert("collections".into(), collections);
}
