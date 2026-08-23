//! Time module for Zen — time and date operations.
//! 
//! Provides: now, unix, utc, date, format, parse, sleep, wait, year, month, day, 
//! hour, minute, second, weekday, timestamp.

use crate::runtime::Vm;
use crate::runtime::native_functions::NativeFunctions;

/// Initialize the time module by creating the time dict and registering natives.
pub fn init_time_module(vm: &mut crate::runtime::Vm) {
    let time = Value::Dict(crate::runtime::BTreeMap::from([
        ("now".into(), Value::NativeFunction("time_now".into())),
        ("unix".into(), Value::NativeFunction("time_unix".into())),
        ("utc".into(), Value::NativeFunction("time_utc".into())),
        ("date".into(), Value::NativeFunction("time_date".into())),
        ("format".into(), Value::NativeFunction("time_format".into())),
        ("parse".into(), Value::NativeFunction("time_parse".into())),
        ("sleep".into(), Value::NativeFunction("time_sleep".into())),
        ("wait".into(), Value::NativeFunction("time_wait".into())),
        ("year".into(), Value::NativeFunction("time_year".into())),
        ("month".into(), Value::NativeFunction("time_month".into())),
        ("day".into(), Value::NativeFunction("time_day".into())),
        ("hour".into(), Value::NativeFunction("time_hour".into())),
        ("minute".into(), Value::NativeFunction("time_minute".into())),
        ("second".into(), Value::NativeFunction("time_second".into())),
        ("weekday".into(), Value::NativeFunction("time_weekday".into())),
        ("timestamp".into(), Value::NativeFunction("time_unix".into())),
    ]));
    vm.vars.insert("time".into(), time);
}
