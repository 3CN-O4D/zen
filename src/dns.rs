//! Zen `dns` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn init_dns_module(vm: &mut Vm) {
let dns = Value::Dict(Arc::new(BTreeMap::from([
    ("resolve".into(), Value::NativeFunction("dns_resolve".into())),
    ("lookup".into(), Value::NativeFunction("dns_resolve".into())),
    ("query".into(), Value::NativeFunction("dns_query".into())),
])));
vm.vars.insert("dns".into(), dns);
}
