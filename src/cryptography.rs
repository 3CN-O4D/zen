//! Zen `cryptography` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;

pub fn init_cryptography_module(vm: &mut Vm) {
let cryptography = Value::Dict(BTreeMap::from([(
    "fernet".into(),
    Value::Dict(BTreeMap::from([
        ("generate_key".into(), Value::NativeFunction("fernet_generate_key".into())),
        ("encrypt".into(), Value::NativeFunction("fernet_encrypt".into())),
        ("decrypt".into(), Value::NativeFunction("fernet_decrypt".into())),
    ])),
)]));
vm.vars.insert("cryptography".into(), cryptography);
}
