//! Zen `ssh` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn init_ssh_module(vm: &mut Vm) {
let ssh = Value::Dict(Arc::new(BTreeMap::from([
    ("run".into(), Value::NativeFunction("ssh_run".into())),
    ("upload".into(), Value::NativeFunction("ssh_upload".into())),
    ("download".into(), Value::NativeFunction("ssh_download".into())),
    ("available".into(), Value::NativeFunction("ssh_available".into())),
])));
vm.vars.insert("ssh".into(), ssh);
}
