//! Zen `struct_mod` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_struct_mod_module(vm: &mut Vm) {
let struct_mod = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("pack".into(), Value::NativeFunction("struct_pack".into())),
    ("unpack".into(), Value::NativeFunction("struct_unpack".into())),
    ("calcsize".into(), Value::NativeFunction("struct_calcsize".into())),
])));
vm.vars.insert("struct".into(), struct_mod);
}
