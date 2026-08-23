//! Zen `pop3` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn init_pop3_module(vm: &mut Vm) {
let pop3 = Value::Dict(Arc::new(BTreeMap::from([
    ("connect".into(), Value::NativeFunction("pop3_connect".into())),
    ("stat".into(), Value::NativeFunction("pop3_stat".into())),
    ("list".into(), Value::NativeFunction("pop3_list".into())),
    ("retr".into(), Value::NativeFunction("pop3_retr".into())),
    ("dele".into(), Value::NativeFunction("pop3_dele".into())),
    ("quit".into(), Value::NativeFunction("pop3_quit".into())),
])));
vm.vars.insert("pop3".into(), pop3);
}
