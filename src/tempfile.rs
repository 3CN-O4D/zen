//! Zen `tempfile` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_tempfile_module(vm: &mut Vm) {
let tempfile = Value::Dict(Arc::new(ahash::AHashMap::from([
    ("dir".into(), Value::NativeFunction("tempfile_dir".into())),
    ("mkdtemp".into(), Value::NativeFunction("tempfile_mkdtemp".into())),
    ("mkstemp".into(), Value::NativeFunction("tempfile_mkstemp".into())),
])));
vm.vars.insert("tempfile".into(), tempfile);
}
