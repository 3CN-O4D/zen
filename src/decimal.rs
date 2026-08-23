//! Zen `decimal` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

pub fn init_decimal_module(vm: &mut Vm) {
let decimal = Value::Dict(Arc::new(BTreeMap::from([
    ("Decimal".into(), Value::NativeFunction("decimal_decimal".into())),
    ("getcontext".into(), Value::NativeFunction("decimal_getcontext".into())),
    ("setcontext".into(), Value::NativeFunction("decimal_setcontext".into())),
    ("localcontext".into(), Value::NativeFunction("decimal_localcontext".into())),
    ("ROUND_HALF_UP".into(), Value::String("ROUND_HALF_UP".into())),
    ("ROUND_HALF_EVEN".into(), Value::String("ROUND_HALF_EVEN".into())),
    ("ROUND_DOWN".into(), Value::String("ROUND_DOWN".into())),
    ("ROUND_UP".into(), Value::String("ROUND_UP".into())),
    ("ROUND_CEILING".into(), Value::String("ROUND_CEILING".into())),
    ("ROUND_FLOOR".into(), Value::String("ROUND_FLOOR".into())),
    ("ROUND_HALF_DOWN".into(), Value::String("ROUND_HALF_DOWN".into())),
    ("ROUND_05UP".into(), Value::String("ROUND_05UP".into())),
])));
vm.vars.insert("decimal".into(), decimal);
}
