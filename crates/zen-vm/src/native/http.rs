//! `http` — convenience HTTP client verbs (Tier 4 umbrella).
//!
//! Thin surface over the `requests` engine so `http.get(url, opts?)` and
//! friends return the same response dictionaries. `http.server` is a separate
//! module (`zen -m http.server`). Low-level `http.client`-style connection
//! objects are intentionally deferred in favour of `http`/`requests`.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::native::{func, host_module_mixed};
use crate::value::Value;

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        func("request", 760, 2, 3),
        func("get", 761, 1, 2),
        func("post", 762, 1, 2),
        func("put", 763, 1, 2),
        func("patch", 764, 1, 2),
        func("delete", 765, 1, 2),
        func("head", 766, 1, 2),
        func("options", 767, 1, 2),
    ])
}

/// Route ids in the `http` block (760–779) onto `requests`' verbs.
pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    let requests_id = match id {
        760 => 700,
        761 => 701,
        762 => 702,
        763 => 703,
        764 => 704,
        765 => 705,
        766 => 706,
        767 => 707,
        _ => {
            return Err(VmError::NativeError(format!(
                "unknown http native id {id}"
            )))
        }
    };
    crate::native::requests::dispatch(requests_id, args)
}
