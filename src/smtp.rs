//! Zen `smtp` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_smtp_module(vm: &mut Vm) {
let smtp = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("connect".into(), Value::NativeFunction("smtp_connect".into())),
    ("login".into(), Value::NativeFunction("smtp_login".into())),
    ("sendmail".into(), Value::NativeFunction("smtp_sendmail".into())),
    ("quit".into(), Value::NativeFunction("smtp_quit".into())),
    ("message".into(), Value::NativeFunction("smtp_message".into())),
])));
vm.vars.insert("smtp".into(), smtp);
}
