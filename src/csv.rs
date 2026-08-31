//! Zen `csv` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_csv_module(vm: &mut Vm) {
let csv = Value::Dict(Arc::new(ahash::AHashMap::from([
    ("read".into(), Value::NativeFunction("csv_read".into())),
    ("write".into(), Value::NativeFunction("csv_write".into())),
    ("parse".into(), Value::NativeFunction("csv_parse".into())),
    ("encode".into(), Value::NativeFunction("csv_encode".into())),
])));
vm.vars.insert("csv".into(), csv);
}
