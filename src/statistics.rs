//! Statistics module for Zen — basic statistical functions.
//! 
//! Provides: mean, median, mode, stdev, variance, min, max, sum.

use crate::runtime::Vm;
use crate::runtime::native_functions::NativeFunctions;

/// Initialize the statistics module by creating the statistics dict and registering natives.
pub fn init_statistics_module(vm: &mut crate::runtime::Vm) {
    let statistics = Value::Dict(crate::runtime::BTreeMap::from([
        ("mean".into(), Value::NativeFunction("statistics_mean".into())),
        ("median".into(), Value::NativeFunction("statistics_median".into())),
        ("mode".into(), Value::NativeFunction("statistics_mode".into())),
        ("stdev".into(), Value::NativeFunction("statistics_stdev".into())),
        ("variance".into(), Value::NativeFunction("statistics_variance".into())),
        ("min".into(), Value::NativeFunction("math_min".into())),
        ("max".into(), Value::NativeFunction("math_max".into())),
        ("sum".into(), Value::NativeFunction("statistics_sum".into())),
    ]));
    vm.vars.insert("statistics".into(), statistics);
}
