//! Zen `subprocess` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;

pub fn init_subprocess_module(vm: &mut Vm) {
let subprocess = Value::Dict(BTreeMap::from([
    ("run".into(), Value::NativeFunction("subprocess_run".into())),
    ("call".into(), Value::NativeFunction("subprocess_call".into())),
    ("check_output".into(), Value::NativeFunction("subprocess_check_output".into())),
]));
vm.vars.insert("subprocess".into(), subprocess);
}
