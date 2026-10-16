//! `html` — `escape`/`unescape` plus a re-export of the parser entry points,
//! mirroring `html.escape`, `html.unescape`, `html.parser`.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::native::{func, host_module_mixed};
use crate::ops::string_arg;
use crate::value::Value;

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        func("escape", 860, 1, 1),
        func("unescape", 861, 1, 1),
        func("parse", 862, 1, 1),
        func("tags", 863, 1, 1),
        func("events", 864, 1, 1),
        func("data", 865, 1, 1),
    ])
}

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    match id {
        860 => Ok(Value::string(crate::native::html_parser::escape_str(
            &string_arg(args, 0)?,
        ))),
        861 => Ok(Value::string(crate::native::html_parser::unescape_str(
            &string_arg(args, 0)?,
        ))),
        862 | 863 => crate::native::html_parser::dispatch(886, args),
        864 => crate::native::html_parser::dispatch(890, args),
        865 => crate::native::html_parser::dispatch(889, args),
        _ => Err(VmError::NativeError(format!("unknown html native id {id}"))),
    }
}
