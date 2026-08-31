//! Zen `json` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_json_module(vm: &mut Vm) {
let json = Value::Dict(Arc::new(ahash::AHashMap::from([
    ("parse".into(), Value::NativeFunction("json_decode".into())),
    ("encode".into(), Value::NativeFunction("json_encode".into())),
    ("stringify".into(), Value::NativeFunction("json_encode".into())),
    ("load".into(), Value::NativeFunction("json_load".into())),
    ("save".into(), Value::NativeFunction("json_save".into())),
])));
vm.vars.insert("json".into(), json);
}
