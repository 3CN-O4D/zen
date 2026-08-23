//! Statistics module for Zen — descriptive statistics functions.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Initialize the statistics module.
pub fn init_statistics_module(vm: &mut Vm) {
    let statistics = Value::Dict(Arc::new(BTreeMap::from([
        ("mean".into(), Value::NativeFunction("statistics_mean".into())),
        ("median".into(), Value::NativeFunction("statistics_median".into())),
        ("mode".into(), Value::NativeFunction("statistics_mode".into())),
        ("stdev".into(), Value::NativeFunction("statistics_stdev".into())),
        ("variance".into(), Value::NativeFunction("statistics_variance".into())),
        ("min".into(), Value::NativeFunction("math_min".into())),
        ("max".into(), Value::NativeFunction("math_max".into())),
        ("sum".into(), Value::NativeFunction("statistics_sum".into())),
    ])));
    vm.vars.insert("statistics".into(), statistics);
}
