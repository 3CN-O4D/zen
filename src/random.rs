//! Zen `random` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;

pub fn init_random_module(vm: &mut Vm) {
let random = Value::Dict(BTreeMap::from([
    ("random".into(), Value::NativeFunction("random_random".into())),
    ("randint".into(), Value::NativeFunction("random_randint".into())),
    ("randrange".into(), Value::NativeFunction("random_randrange".into())),
    ("choice".into(), Value::NativeFunction("random_choice".into())),
    ("choices".into(), Value::NativeFunction("random_choices".into())),
    ("sample".into(), Value::NativeFunction("random_sample".into())),
    ("shuffle".into(), Value::NativeFunction("random_shuffle".into())),
    ("uniform".into(), Value::NativeFunction("random_uniform".into())),
    ("hex".into(), Value::NativeFunction("random_hex".into())),
    ("seed".into(), Value::NativeFunction("random_seed".into())),
]));
vm.vars.insert("random".into(), random);
}
