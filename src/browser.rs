//! Zen `browser` module.

use crate::runtime::{Vm, Value};
use std::sync::Arc;

pub fn init_browser_module(vm: &mut Vm) {
let browser = Value::Dict(Arc::new(indexmap::IndexMap::from([
    ("launch".into(), Value::NativeFunction("browser_launch".into())),
    ("connect".into(), Value::NativeFunction("browser_connect".into())),
    ("navigate".into(), Value::NativeFunction("browser_navigate".into())),
    ("go".into(), Value::NativeFunction("browser_navigate".into())),
    ("evaluate".into(), Value::NativeFunction("browser_evaluate".into())),
    ("eval".into(), Value::NativeFunction("browser_evaluate".into())),
    ("screenshot".into(), Value::NativeFunction("browser_capture_screenshot".into())),
    ("shot".into(), Value::NativeFunction("browser_capture_screenshot".into())),
    ("html".into(), Value::NativeFunction("browser_get_html".into())),
    ("page".into(), Value::NativeFunction("browser_get_html".into())),
    ("get_title".into(), Value::NativeFunction("browser_get_title".into())),
    ("title".into(), Value::NativeFunction("browser_get_title".into())),
    ("get_url".into(), Value::NativeFunction("browser_get_url".into())),
    ("url".into(), Value::NativeFunction("browser_get_url".into())),
    ("get_text".into(), Value::NativeFunction("browser_get_text".into())),
    ("text".into(), Value::NativeFunction("browser_get_text".into())),
    ("click".into(), Value::NativeFunction("browser_click".into())),
    ("fill".into(), Value::NativeFunction("browser_fill".into())),
    ("query".into(), Value::NativeFunction("browser_query".into())),
    ("wait_for".into(), Value::NativeFunction("browser_wait_for".into())),
    ("wait_for_ms".into(), Value::NativeFunction("browser_wait_for_ms".into())),
    ("attr".into(), Value::NativeFunction("browser_attr".into())),
    ("page_text".into(), Value::NativeFunction("browser_page_text".into())),
    ("close".into(), Value::NativeFunction("browser_close".into())),
    ("quit".into(), Value::NativeFunction("browser_close".into())),
])));
vm.vars.insert("browser".into(), browser);
}
