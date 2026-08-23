//! Zen `urllib` module.

use crate::runtime::{Vm, Value};
use std::collections::BTreeMap;

pub fn init_urllib_module(vm: &mut Vm) {
let urllib = Value::Dict(BTreeMap::from([
    ("urlopen".into(), Value::NativeFunction("urllib_urlopen".into())),
    ("quote".into(), Value::NativeFunction("urllib_quote".into())),
    ("unquote".into(), Value::NativeFunction("urllib_unquote".into())),
    ("urlencode".into(), Value::NativeFunction("urllib_urlencode".into())),
    ("parse".into(), Value::NativeFunction("urllib_parse".into())),
    ("parse_qs".into(), Value::NativeFunction("urllib_parse_qs".into())),
]));
vm.vars.insert("urllib".into(), urllib);
}
