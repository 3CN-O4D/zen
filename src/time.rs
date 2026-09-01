//! Zen `time` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_time_module(vm: &mut Vm) {
let time = Value::Dict(Arc::new(indexmap::IndexMap::from([
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
])));
vm.vars.insert("time".into(), time);
}
