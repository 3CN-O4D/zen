//! Browser module for Zen — CDP-based browser automation.
//! 
//! Provides: launch, connect, navigate, evaluate, screenshot, click, fill, 
//! query, wait_for, wait_for_ms, attr, page_text, close, quit.

use crate::runtime::Vm;
use crate::runtime::native_functions::NativeFunctions;

/// Initialize the browser module by creating the browser dict and registering natives.
pub fn init_browser_module(vm: &mut crate::runtime::Vm) {
    let browser = Value::Dict(crate::runtime::BTreeMap::from([
        ("launch".into(), Value::NativeFunction("browser_launch".into())),
        ("connect".into(), Value::NativeFunction("browser_connect".into())),
        ("navigate".into(), Value::NativeFunction("browser_navigate".into())),
        ("evaluate".into(), Value::NativeFunction("browser_evaluate".into())),
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
    ]));
    vm.vars.insert("browser".into(), browser);
}
