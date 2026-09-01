//! Zen `itertools` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_itertools_module(vm: &mut Vm) {
let itertools = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("enumerate".into(), Value::NativeFunction("itertools_enumerate".into())),
    ("zip".into(), Value::NativeFunction("itertools_zip".into())),
    ("chain".into(), Value::NativeFunction("itertools_chain".into())),
    ("repeat".into(), Value::NativeFunction("itertools_repeat".into())),
    ("product".into(), Value::NativeFunction("itertools_product".into())),
    ("permutations".into(), Value::NativeFunction("itertools_permutations".into())),
    ("combinations".into(), Value::NativeFunction("itertools_combinations".into())),
    ("accumulate".into(), Value::NativeFunction("itertools_accumulate".into())),
    ("take".into(), Value::NativeFunction("itertools_take".into())),
    ("drop".into(), Value::NativeFunction("itertools_drop".into())),
    ("range".into(), Value::NativeFunction("itertools_range".into())),
])));
vm.vars.insert("itertools".into(), itertools);
}
