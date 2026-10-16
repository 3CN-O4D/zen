//! `lxml` — flat convenience surface over `lxml.etree` / `lxml.html`.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::native::{func, host_module_mixed};
use crate::value::Value;

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        func("fromstring", 950, 1, 1),
        func("tostring", 951, 1, 1),
        func("find", 952, 2, 2),
        func("findall", 953, 2, 2),
        func("text_content", 954, 1, 1),
        func("iter", 955, 1, 2),
        func("tag", 956, 1, 1),
        func("attrib", 957, 1, 1),
    ])
}

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    let etree_id = match id {
        950 => 900,
        951 => 901,
        952 => 907,
        953 => 908,
        954 => 910,
        955 => 909,
        956 => 902,
        957 => 904,
        _ => return Err(VmError::NativeError(format!("unknown lxml native id {id}"))),
    };
    crate::native::lxml_etree::dispatch(etree_id, args)
}
