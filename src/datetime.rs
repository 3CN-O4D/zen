//! Zen `datetime` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_datetime_module(vm: &mut Vm) {
let datetime = Value::Dict(Arc::new(ahash::AHashMap::from([
    ("now".into(), Value::NativeFunction("time_now".into())),
    ("utcnow".into(), Value::NativeFunction("time_utc".into())),
    ("today".into(), Value::NativeFunction("time_date".into())),
    ("unix".into(), Value::NativeFunction("time_unix".into())),
    ("from_unix".into(), Value::NativeFunction("time_from_unix".into())),
    ("parse".into(), Value::NativeFunction("time_parse".into())),
    ("format".into(), Value::NativeFunction("time_format".into())),
    ("year".into(), Value::NativeFunction("time_year".into())),
    ("month".into(), Value::NativeFunction("time_month".into())),
    ("day".into(), Value::NativeFunction("time_day".into())),
    ("hour".into(), Value::NativeFunction("time_hour".into())),
    ("minute".into(), Value::NativeFunction("time_minute".into())),
    ("second".into(), Value::NativeFunction("time_second".into())),
    ("weekday".into(), Value::NativeFunction("time_weekday".into())),
    ("add_days".into(), Value::NativeFunction("time_add_days".into())),
    ("MONDAY".into(), Value::Number(0.0)),
    ("TUESDAY".into(), Value::Number(1.0)),
    ("WEDNESDAY".into(), Value::Number(2.0)),
    ("THURSDAY".into(), Value::Number(3.0)),
    ("FRIDAY".into(), Value::Number(4.0)),
    ("SATURDAY".into(), Value::Number(5.0)),
    ("SUNDAY".into(), Value::Number(6.0)),
])));
vm.vars.insert("datetime".into(), datetime);
}
