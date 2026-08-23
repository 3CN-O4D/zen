//! Zen `crunch` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;

pub fn init_crunch_module(vm: &mut Vm) {
let crunch = Value::Dict(BTreeMap::from([
    ("charset".into(), Value::NativeFunction("crunch_charset".into())),
    ("generate".into(), Value::NativeFunction("crunch_generate".into())),
    ("pattern".into(), Value::NativeFunction("crunch_pattern".into())),
]));
vm.vars.insert("crunch".into(), crunch);
}
