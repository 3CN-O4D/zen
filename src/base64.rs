//! Zen `base64` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_base64_module(vm: &mut Vm) {
let base64 = Value::Dict(Arc::new(ahash::AHashMap::from([
    ("encode".into(), Value::NativeFunction("b64_encode".into())),
    ("decode".into(), Value::NativeFunction("b64_decode".into())),
    ("url_encode".into(), Value::NativeFunction("b64_url_encode".into())),
    ("url_decode".into(), Value::NativeFunction("b64_url_decode".into())),
])));
vm.vars.insert("base64".into(), base64);
}
