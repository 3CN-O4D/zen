//! Zen `glob` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn init_glob_module(vm: &mut Vm) {
let glob = Value::Dict(Arc::new(BTreeMap::from([
    ("glob".into(), Value::NativeFunction("fs_glob".into())),
])));
vm.vars.insert("glob".into(), glob);
}
