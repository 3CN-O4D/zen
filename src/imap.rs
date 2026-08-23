//! Zen `imap` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;

pub fn init_imap_module(vm: &mut Vm) {
let imap = Value::Dict(BTreeMap::from([
    ("connect".into(), Value::NativeFunction("imap_connect".into())),
    ("select".into(), Value::NativeFunction("imap_select".into())),
    ("search".into(), Value::NativeFunction("imap_search".into())),
    ("fetch".into(), Value::NativeFunction("imap_fetch".into())),
    ("list".into(), Value::NativeFunction("imap_list".into())),
    ("logout".into(), Value::NativeFunction("imap_logout".into())),
]));
vm.vars.insert("imap".into(), imap);
}
