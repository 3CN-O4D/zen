//! `urllib` — flat convenience surface over `urllib.parse` and
//! `urllib.request`. Real code uses the dotted submodules; this aggregator
//! mirrors Python's common `urllib.quote`-style one-liners where they appear.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::native::{func, host_module_mixed};
use crate::value::Value;

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        func("quote", 840, 1, 2),
        func("quote_plus", 841, 1, 2),
        func("unquote", 842, 1, 1),
        func("unquote_plus", 843, 1, 1),
        func("urlencode", 844, 1, 1),
        func("parse_qs", 845, 1, 1),
        func("parse_qsl", 846, 1, 1),
        func("urlparse", 847, 1, 1),
        func("urljoin", 848, 2, 2),
        func("urlopen", 849, 1, 2),
        func("urlretrieve", 850, 1, 2),
    ])
}

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    match id {
        840 => crate::native::urllib_parse::dispatch(800, args),
        841 => crate::native::urllib_parse::dispatch(801, args),
        842 => crate::native::urllib_parse::dispatch(802, args),
        843 => crate::native::urllib_parse::dispatch(803, args),
        844 => crate::native::urllib_parse::dispatch(804, args),
        845 => crate::native::urllib_parse::dispatch(805, args),
        846 => crate::native::urllib_parse::dispatch(806, args),
        847 => crate::native::urllib_parse::dispatch(807, args),
        848 => crate::native::urllib_parse::dispatch(810, args),
        849 => crate::native::urllib_request::dispatch(820, args),
        850 => crate::native::urllib_request::dispatch(821, args),
        _ => Err(VmError::NativeError(format!(
            "unknown urllib native id {id}"
        ))),
    }
}
