//! Foreign (CPython) bridge seam for `pymport`.
//!
//! `pymport` statements compile to [`Opcode::PyImport`](crate::bytecode::Opcode::PyImport),
//! whose execution routes here instead of the Zen module linker — native
//! imports never pay a foreign check. The trait keeps the VM and the
//! tree-walking interpreter decoupled from any particular Python embedding:
//!
//! - [`UnlinkedBridge`] (always available) fails with an actionable message
//!   telling the user how to link a bridge.
//! - A real embedding (PyO3, behind the `python` cargo feature) implements
//!   this trait, owns a handle registry (`u32` id → referent), and publishes
//!   itself on [`Vm::py_bridge`](crate::vm::Vm::py_bridge).
//!
//! Handles are registry-owned: [`crate::gc::new_foreign`] values carry only
//! the id, so cloning/sharing them is free and `Drop` is a no-op. Entries
//! live until the bridge drops (bounded by distinct handles).
//!
//! v1 marshaling boundary: primitives cross by value through
//! [`PyValue`] (nil/bool/int/float/string plus flat lists/dicts of those);
//! anything else crosses as an opaque handle. Zero-copy transmutation of
//! unboxed registers is future work at the call sites, not here.

use crate::errors::{VmError, VmResult};
use crate::value::Value;

/// A primitive value that crossed the foreign boundary by value. Compound
/// shapes nest only primitives; anything richer stays an opaque handle.
#[derive(Debug, Clone, PartialEq)]
pub enum PyValue {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<PyValue>),
    Dict(Vec<(String, PyValue)>),
    /// Opaque bridge-registry id for a non-primitive referent.
    Handle(u32),
}

/// Errors from the foreign boundary, reported with the failing operation.
/// `traceback` carries formatted CPython frames when the failure raised
/// inside Python (populated by embeddings; `None` for local failures like
/// unlinked bridges and marshal rejections).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PyBridgeError {
    pub op: &'static str,
    pub message: String,
    pub traceback: Option<String>,
}

impl PyBridgeError {
    pub fn new(op: &'static str, message: impl Into<String>) -> Self {
        Self {
            op,
            message: message.into(),
            traceback: None,
        }
    }

    /// Attach a formatted Python traceback (embeddings only).
    pub fn with_traceback(mut self, traceback: String) -> Self {
        self.traceback = Some(traceback);
        self
    }
}

impl std::fmt::Display for PyBridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pymport {} failed: {}", self.op, self.message)
    }
}

/// The CPython bridge. Object-safe so embeddings share it as
/// `Arc<dyn PyBridge>`.
pub trait PyBridge: Send + Sync {
    /// Stable dialect name for diagnostics (e.g. `"unlinked"`, `"pyo3-embed"`).
    fn dialect_id(&self) -> &'static str;

    /// Import a module by dotted path, returning its registry handle.
    fn import_module(&self, name: &str) -> Result<u32, PyBridgeError>;

    /// Read an attribute off a handle. Primitives cross by value; callables
    /// and instances MUST come back as [`PyValue::Handle`] (opaque), never
    /// eagerly marshaled — [`PyBridge::call_attr`]'s default threading
    /// depends on it.
    fn getattr(&self, handle: u32, attr: &str) -> Result<PyValue, PyBridgeError>;

    /// Write an attribute on a handle.
    fn setattr(&self, handle: u32, attr: &str, value: PyValue) -> Result<(), PyBridgeError>;

    /// Call a handle with positional arguments.
    fn call(&self, handle: u32, args: Vec<PyValue>) -> Result<PyValue, PyBridgeError>;

    /// Resolve `attr` on `handle` and call it with `args`, fused into one
    /// bridge round-trip. The default threads [`PyBridge::getattr`] into
    /// [`PyBridge::call`]; embeddings SHOULD override this to
    /// resolve-and-dispatch server-side — one GIL acquisition instead of two,
    /// and the natural home for a monomorphic FFI inline cache keyed by
    /// `(handle, attr)` (first call resolves the callable pointer, later
    /// calls jump straight to it).
    fn call_attr(
        &self,
        handle: u32,
        attr: &str,
        args: Vec<PyValue>,
    ) -> Result<PyValue, PyBridgeError> {
        let target = self.getattr(handle, attr)?;
        match target {
            PyValue::Handle(h) => self.call(h, args),
            // Data attributes are values, not callables: calling one is a
            // type error on both sides of the boundary.
            PyValue::Nil => Err(PyBridgeError::new("call", format!("{attr} is nil"))),
            PyValue::Bool(_) => Err(PyBridgeError::new("call", format!("{attr} is a bool"))),
            PyValue::Int(_) => Err(PyBridgeError::new("call", format!("{attr} is an int"))),
            PyValue::Float(_) => Err(PyBridgeError::new("call", format!("{attr} is a float"))),
            PyValue::Str(_) => Err(PyBridgeError::new("call", format!("{attr} is a string"))),
            PyValue::Bytes(_) => Err(PyBridgeError::new("call", format!("{attr} is bytes"))),
            PyValue::List(_) => Err(PyBridgeError::new("call", format!("{attr} is a list"))),
            PyValue::Dict(_) => Err(PyBridgeError::new("call", format!("{attr} is a dict"))),
        }
    }

    /// Release a handle early (default: retain until bridge teardown).
    fn release(&self, _handle: u32) {}
}

/// Convert a [`PyValue`] produced by the bridge into a [`Value`]. Handles
/// become [`crate::gc::new_foreign`] registry references.
pub fn pyvalue_to_value(v: PyValue) -> Value {
    match v {
        PyValue::Nil => Value::nil(),
        PyValue::Bool(b) => Value::bool(b),
        PyValue::Int(n) => Value::int_promote(n),
        PyValue::Float(f) => Value::float(f),
        PyValue::Str(s) => Value::string(s),
        PyValue::Bytes(b) => Value::bytes(std::sync::Arc::<[u8]>::from(b)),
        PyValue::List(items) => {
            crate::gc::new_list(items.into_iter().map(pyvalue_to_value).collect())
        }
        PyValue::Dict(entries) => {
            let map: indexmap::IndexMap<crate::value::DictKey, Value> = entries
                .into_iter()
                .map(|(k, val)| {
                    (
                        crate::value::DictKey(Value::string(k)),
                        pyvalue_to_value(val),
                    )
                })
                .collect();
            crate::gc::new_dict(map)
        }
        PyValue::Handle(h) => crate::gc::new_foreign(h),
    }
}

/// Convert a [`Value`] argument into a [`PyValue`] for the bridge. Heap
/// containers recurse only through primitives; anything else (instances,
/// funcs, ropes, existing foreign handles excepted) is an error rather than
/// a lossy coercion.
pub fn value_to_pyvalue(v: Value) -> Result<PyValue, PyBridgeError> {
    if v.is_nil() {
        return Ok(PyValue::Nil);
    }
    if let Some(b) = v.as_bool() {
        return Ok(PyValue::Bool(b));
    }
    if let Some(n) = v.as_int() {
        return Ok(PyValue::Int(n));
    }
    if let Some(f) = v.as_float() {
        return Ok(PyValue::Float(f));
    }
    if let Some(s) = v.as_str() {
        return Ok(PyValue::Str(s.to_string()));
    }
    if let Some(b) = v.as_bytes() {
        return Ok(PyValue::Bytes(b.to_vec()));
    }
    if let Some(h) = v.as_foreign() {
        return Ok(PyValue::Handle(h));
    }
    if let Some(items) = v.as_list() {
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(value_to_pyvalue(*item)?);
        }
        return Ok(PyValue::List(out));
    }
    if let Some(items) = v.as_tuple() {
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(value_to_pyvalue(*item)?);
        }
        return Ok(PyValue::List(out));
    }
    if let Some(map) = v.as_dict() {
        let mut out = Vec::with_capacity(map.len());
        for (k, val) in map.iter() {
            let key = k
                .0
                .as_str()
                .ok_or_else(|| {
                    PyBridgeError::new("marshal", "dict keys crossing into Python must be strings")
                })?
                .to_string();
            out.push((key, value_to_pyvalue(*val)?));
        }
        return Ok(PyValue::Dict(out));
    }
    Err(PyBridgeError::new(
        "marshal",
        format!("cannot marshal {} across the Python boundary", v.type_name()),
    ))
}

/// The always-available bridge: every operation fails with an actionable
/// message telling the user how to link a real embedding.
pub struct UnlinkedBridge;

impl PyBridge for UnlinkedBridge {
    fn dialect_id(&self) -> &'static str {
        "unlinked"
    }

    fn import_module(&self, name: &str) -> Result<u32, PyBridgeError> {
        Err(PyBridgeError::new(
            "import",
            format!(
                "pymport '{name}' needs a linked Python bridge (this build has none). \
                 Rebuild with `--features python` (requires libpython) or route the \
                 dependency through a native Zen module instead."
            ),
        ))
    }

    fn getattr(&self, _handle: u32, attr: &str) -> Result<PyValue, PyBridgeError> {
        Err(PyBridgeError::new(
            "getattr",
            format!("cannot read '{attr}': no Python bridge is linked (see `pymport` docs)"),
        ))
    }

    fn setattr(&self, _handle: u32, attr: &str, _value: PyValue) -> Result<(), PyBridgeError> {
        Err(PyBridgeError::new(
            "setattr",
            format!("cannot write '{attr}': no Python bridge is linked (see `pymport` docs)"),
        ))
    }

    fn call(&self, _handle: u32, _args: Vec<PyValue>) -> Result<PyValue, PyBridgeError> {
        Err(PyBridgeError::new(
            "call",
            "cannot call into Python: no Python bridge is linked (see `pymport` docs)",
        ))
    }
}

/// Which foreign operation a failure belongs to. Decides the `VmError`
/// variant (and therefore the `Z04xx` code and the `zen explain` essay) —
/// an import problem, a missing attribute, a failed call, and a marshal
/// rejection are different user actions, so they must not share a code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeOp {
    Import,
    GetAttr,
    SetAttr,
    Call,
    Marshal,
}

/// Fetch the VM's bridge or fail with the unlinked message (as a plain
/// string — each call site wraps it in its operation's error variant).
pub fn bridge_of(vm: &crate::vm::Vm) -> Result<std::sync::Arc<dyn PyBridge>, String> {
    vm.py_bridge.clone().ok_or_else(|| {
        "pymport needs a linked Python bridge (this build has none). \
         Rebuild with `--features python` (requires libpython)."
            .to_string()
    })
}

/// Run a bridge fallible returning `PyValue`, mapping errors to the
/// operation's `VmError` (traceback included when the bridge captured one).
pub fn bridge_value(
    r: Result<PyValue, PyBridgeError>,
    op: BridgeOp,
) -> VmResult<Value> {
    r.map(pyvalue_to_value).map_err(|e| into_vm_error(e, op))
}

/// Map a bridge failure to its operation's `VmError` variant.
pub fn into_vm_error(e: PyBridgeError, op: BridgeOp) -> VmError {
    let PyBridgeError {
        message,
        traceback,
        ..
    } = e;
    match op {
        BridgeOp::Import => VmError::PyImport { message, traceback },
        BridgeOp::GetAttr | BridgeOp::SetAttr => VmError::ForeignAttribute { message, traceback },
        BridgeOp::Call => VmError::ForeignCall { message, traceback },
        BridgeOp::Marshal => VmError::Marshal { message },
    }
}
