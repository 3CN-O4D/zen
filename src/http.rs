//! Zen `http` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn init_http_module(vm: &mut Vm) {
let http = Value::Dict(Arc::new(BTreeMap::from([
    ("get".into(), Value::NativeFunction("http_get".into())),
    ("post".into(), Value::NativeFunction("http_post".into())),
    ("put".into(), Value::NativeFunction("http_put".into())),
    ("del".into(), Value::NativeFunction("http_del".into())),
    ("head".into(), Value::NativeFunction("http_head".into())),
    ("patch".into(), Value::NativeFunction("http_patch".into())),
])));
vm.vars.insert("http".into(), http);
}
