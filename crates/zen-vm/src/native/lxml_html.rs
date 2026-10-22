//! `lxml.html` — tolerant HTML variant of the `lxml.etree` surface, sharing the
//! same arena and element handles.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::native::{func, host_module_mixed};
use crate::value::Value;

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        func("fromstring", 930, 1, 1),
        func("tostring", 931, 1, 1),
        func("text_content", 932, 1, 1),
        func("find", 933, 2, 2),
        func("findall", 934, 2, 2),
        func("iter", 935, 1, 2),
        func("tag", 936, 1, 1),
        func("attrib", 937, 1, 1),
        func("get", 938, 2, 3),
    ])
}

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    let etree_id = match id {
        930 => 900,
        931 => 901,
        932 => 910,
        933 => 907,
        934 => 908,
        935 => 909,
        936 => 902,
        937 => 904,
        938 => 905,
        _ => {
            return Err(VmError::NativeError(format!(
                "unknown lxml.html native id {id}"
            )))
        }
    };
    crate::native::lxml_etree::dispatch(etree_id, args)
}
