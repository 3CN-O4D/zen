//! Zen `re` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_re_module(vm: &mut Vm) {
let re = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("match".into(), Value::NativeFunction("regex_match".into())),
    ("matches".into(), Value::NativeFunction("regex_match".into())),
    ("search".into(), Value::NativeFunction("regex_search".into())),
    ("find".into(), Value::NativeFunction("regex_find".into())),
    ("findall".into(), Value::NativeFunction("regex_find".into())),
    ("split".into(), Value::NativeFunction("regex_split".into())),
    ("replace".into(), Value::NativeFunction("regex_replace".into())),
    ("sub".into(), Value::NativeFunction("regex_replace".into())),
])));
vm.vars.insert("re".into(), re);
}
