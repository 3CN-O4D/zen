//! Zen `shutil` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_shutil_module(vm: &mut Vm) {
let shutil = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("copy".into(), Value::NativeFunction("shutil_copy".into())),
    ("copy2".into(), Value::NativeFunction("shutil_copy2".into())),
    ("move".into(), Value::NativeFunction("shutil_move".into())),
    ("rmtree".into(), Value::NativeFunction("shutil_rmtree".into())),
    ("copytree".into(), Value::NativeFunction("shutil_copytree".into())),
    ("which".into(), Value::NativeFunction("shutil_which".into())),
    ("disk_usage".into(), Value::NativeFunction("shutil_disk_usage".into())),
])));
vm.vars.insert("shutil".into(), shutil);
}
