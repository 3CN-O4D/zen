//! Zen `base32` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_base32_module(vm: &mut Vm) {
let base32 = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("encode".into(), Value::NativeFunction("b32_encode".into())),
    ("decode".into(), Value::NativeFunction("b32_decode".into())),
])));
vm.vars.insert("base32".into(), base32);
}
