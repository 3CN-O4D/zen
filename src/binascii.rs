//! Zen `binascii` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_binascii_module(vm: &mut Vm) {
let binascii = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("hexlify".into(), Value::NativeFunction("binascii_hexlify".into())),
    ("unhexlify".into(), Value::NativeFunction("binascii_unhexlify".into())),
    ("a2b_base64".into(), Value::NativeFunction("binascii_a2b_base64".into())),
    ("b2a_base64".into(), Value::NativeFunction("binascii_b2a_base64".into())),
])));
vm.vars.insert("binascii".into(), binascii);
}
