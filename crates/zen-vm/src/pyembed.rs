//! Real CPython embedding behind the `python` cargo feature (`PyO3`).
//!
//! [`PythonBridge`] implements [`crate::pybridge::PyBridge`] by driving the
//! system CPython (`libpython3.13` here) through PyO3. Registry ids (`u32`)
//! own `Py<PyAny>` referents under a mutex; Zen `Foreign` values carry only
//! the id. Every entry point takes the GIL via `Python::with_gil` (one
//! acquisition per bridge call — [`PythonBridge::call_attr`] fuses
//! resolve+dispatch into a single section; per-thread state caching is
//! future work).
//!
//! Build with `--features python` (needs libpython at build AND run time).
//! Default builds stay hermetic: `pymport` fails with the actionable
//! unlinked message instead.
#![allow(unsafe_code)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyFloat, PyInt, PyList, PyModule, PyString, PyTuple};

use crate::pybridge::{PyBridge, PyBridgeError, PyValue};

/// A linked CPython embedding. Share as `Arc<PythonBridge>` on
/// [`Vm::py_bridge`](crate::vm::Vm::py_bridge).
pub struct PythonBridge {
    objects: Mutex<HashMap<u32, Py<PyAny>>>,
    next: AtomicU32,
}

impl PythonBridge {
    pub fn new() -> Self {
        Self {
            objects: Mutex::new(HashMap::new()),
            // 0 is reserved as "no referent".
            next: AtomicU32::new(1),
        }
    }

    fn retain(&self, obj: Py<PyAny>) -> u32 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        // In the absurd case of u32 wraparound, ids could collide; pins and
        // practical counts make this unreachable, and debug builds assert it.
        debug_assert_ne!(id, 0);
        self.objects
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, obj);
        id
    }

    /// Clone a registry referent (needs the GIL: `Py<T>` is not `Clone`).
    fn lookup(&self, py: Python<'_>, handle: u32) -> Result<Py<PyAny>, PyBridgeError> {
        match self
            .objects
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&handle)
        {
            Some(obj) => Ok(obj.clone_ref(py)),
            None => Err(PyBridgeError::new(
                "handle",
                format!("stale Python handle {handle} (object was released)"),
            )),
        }
    }

    /// Convert a live bound object into a [`PyValue`]: primitives by value,
    /// everything else as a fresh registry handle. Type checks are explicit
    /// (`is_instance_of`) rather than trial extraction — trial order
    /// misclassifies (e.g. a small-int tuple extracts as bytes).
    fn export(&self, py: Python<'_>, obj: &Bound<'_, PyAny>) -> Result<PyValue, PyBridgeError> {
        if obj.is_none() {
            return Ok(PyValue::Nil);
        }
        if obj.is_instance_of::<PyBool>() {
            let b: bool = obj.extract().map_err(|e| err("marshal", e, py))?;
            return Ok(PyValue::Bool(b));
        }
        if obj.is_instance_of::<PyInt>() {
            if let Ok(n) = obj.extract::<i64>() {
                return Ok(PyValue::Int(n));
            }
            // Beyond i64: best-effort f64 (documented precision caveat).
            let f: f64 = obj.extract().map_err(|e| err("marshal", e, py))?;
            return Ok(PyValue::Float(f));
        }
        if obj.is_instance_of::<PyFloat>() {
            let f: f64 = obj.extract().map_err(|e| err("marshal", e, py))?;
            return Ok(PyValue::Float(f));
        }
        if obj.is_instance_of::<PyString>() {
            let s: String = obj.extract().map_err(|e| err("marshal", e, py))?;
            return Ok(PyValue::Str(s));
        }
        if obj.is_instance_of::<PyBytes>() {
            let b: Vec<u8> = obj.extract().map_err(|e| err("marshal", e, py))?;
            return Ok(PyValue::Bytes(b));
        }
        if let Ok(list) = obj.cast::<PyList>() {
            let mut out = Vec::with_capacity(list.len());
            for i in 0..list.len() {
                let item = list.get_item(i).map_err(|e| err("marshal", e, py))?;
                out.push(self.export(py, &item)?);
            }
            return Ok(PyValue::List(out));
        }
        // Tuples (and named tuples) cross as Zen lists — Zen draws no
        // list/tuple distinction at the foreign boundary (documented).
        if let Ok(tuple) = obj.cast::<PyTuple>() {
            let mut out = Vec::with_capacity(tuple.len());
            for i in 0..tuple.len() {
                let item = tuple.get_item(i).map_err(|e| err("marshal", e, py))?;
                out.push(self.export(py, &item)?);
            }
            return Ok(PyValue::List(out));
        }
        if let Ok(d) = obj.cast::<PyDict>() {
            let mut out = Vec::with_capacity(d.len());
            for (k, v) in d.iter() {
                let key: String = k.extract().map_err(|_| {
                    PyBridgeError::new("marshal", "dict keys crossing from Python must be strings")
                })?;
                out.push((key, self.export(py, &v)?));
            }
            return Ok(PyValue::Dict(out));
        }
        Ok(PyValue::Handle(self.retain(obj.clone().unbind())))
    }

    /// Build a bound object from a [`PyValue`]. Handles resolve through the
    /// registry (zero remarshaling — the referent passes straight through).
    fn ingest(&self, py: Python<'_>, v: &PyValue) -> Result<Py<PyAny>, PyBridgeError> {
        let bound: Bound<'_, PyAny> = match v {
            PyValue::Nil => py.None().into_bound(py),
            // `bool` converts to an interned singleton borrow; `to_owned`
            // lifts it to an owned reference first.
            PyValue::Bool(b) => (*b)
                .into_pyobject(py)
                .expect("bool->pyobject is infallible")
                .to_owned()
                .into_any(),
            PyValue::Int(n) => n.into_pyobject(py).expect("int->pyobject is infallible").into_any(),
            PyValue::Float(f) => f.into_pyobject(py).expect("float->pyobject is infallible").into_any(),
            PyValue::Str(s) => s.into_pyobject(py).expect("str->pyobject is infallible").into_any(),
            PyValue::Bytes(b) => PyBytes::new(py, b).into_any(),
            PyValue::List(items) => {
                let list = PyList::empty(py);
                for item in items {
                    list.append(self.ingest(py, item)?.bind(py)).map_err(|e| {
                        PyBridgeError::new("marshal", format!("list append failed: {e}"))
                    })?;
                }
                list.into_any()
            }
            PyValue::Dict(entries) => {
                let dict = PyDict::new(py);
                for (k, val) in entries {
                    dict.set_item(k, self.ingest(py, val)?.bind(py)).map_err(|e| {
                        PyBridgeError::new("marshal", format!("dict insert failed: {e}"))
                    })?;
                }
                dict.into_any()
            }
            PyValue::Handle(h) => return self.lookup(py, *h),
        };
        Ok(bound.unbind())
    }
}

impl Default for PythonBridge {
    fn default() -> Self {
        Self::new()
    }
}

fn err(op: &'static str, e: PyErr, py: Python<'_>) -> PyBridgeError {
    let msg = e.to_string();
    let traceback = format_traceback(py, &e);
    // Drop the fetched error without printing: the message (and chained
    // traceback) travel in the `PyBridgeError`, and an unhandled PyErr would
    // warn on drop.
    unsafe {
        pyo3::ffi::PyErr_Clear();
    }
    PyBridgeError {
        op,
        message: msg,
        traceback,
    }
}

/// Format the live Python traceback as `File ... line ... in ...` lines
/// (capped), or `None` when there is no traceback to show. Best-effort:
/// any failure here degrades to no traceback, never to a new error.
fn format_traceback(py: Python<'_>, e: &PyErr) -> Option<String> {
    let tb = e.traceback(py)?;
    let module = PyModule::import(py, "traceback").ok()?;
    let entries = module
        .getattr("extract_tb")
        .ok()?
        .call1((tb,))
        .ok()?;
    // FrameSummary objects expose filename/lineno/name attributes (they are
    // not plain tuples — extracting as tuples silently fails).
    let frames: Vec<Bound<'_, PyAny>> = entries.extract().ok()?;
    if frames.is_empty() {
        return None;
    }
    let mut out = String::from("Python traceback (most recent call last):");
    let mut shown = 0;
    for frame in frames.iter().take(8) {
        let file: String = frame.getattr("filename").ok()?.extract().ok()?;
        let line: u32 = frame.getattr("lineno").ok()?.extract().ok()?;
        let func: String = frame.getattr("name").ok()?.extract().ok()?;
        out.push_str(&format!("\n  File \"{file}\", line {line} in {func}"));
        shown += 1;
    }
    if frames.len() > shown {
        out.push_str(&format!("\n  ... ({} more frames)", frames.len() - shown));
    }
    Some(out)
}

impl PyBridge for PythonBridge {
    fn dialect_id(&self) -> &'static str {
        "pyo3-embed"
    }

    fn import_module(&self, name: &str) -> Result<u32, PyBridgeError> {
        Python::attach(|py| {
            let module = PyModule::import(py, name).map_err(|e| err("import", e, py))?;
            Ok(self.retain(module.into_any().unbind()))
        })
    }

    fn getattr(&self, handle: u32, attr: &str) -> Result<PyValue, PyBridgeError> {
        Python::attach(|py| {
            let obj = self.lookup(py, handle)?;
            let value = obj.bind(py).getattr(attr).map_err(|e| err("getattr", e, py))?;
            self.export(py, &value)
        })
    }

    fn setattr(&self, handle: u32, attr: &str, value: PyValue) -> Result<(), PyBridgeError> {
        Python::attach(|py| {
            let obj = self.lookup(py, handle)?;
            let v = self.ingest(py, &value)?;
            obj.bind(py)
                .setattr(attr, v.bind(py))
                .map_err(|e| err("setattr", e, py))?;
            Ok(())
        })
    }

    fn call(&self, handle: u32, args: Vec<PyValue>) -> Result<PyValue, PyBridgeError> {
        Python::attach(|py| {
            let obj = self.lookup(py, handle)?;
            let mut tuple_args = Vec::with_capacity(args.len());
            for a in &args {
                tuple_args.push(self.ingest(py, a)?);
            }
            let bound_args = PyTuple::new(py, tuple_args.iter().map(|o| o.bind(py)))
                .map_err(|e| err("call", e, py))?;
            let out = obj
                .bind(py)
                .call1(bound_args)
                .map_err(|e| err("call", e, py))?;
            self.export(py, &out)
        })
    }

    /// Fused resolve+dispatch in ONE GIL section (the v1 fast path: half the
    /// acquisitions of the default threading).
    fn call_attr(
        &self,
        handle: u32,
        attr: &str,
        args: Vec<PyValue>,
    ) -> Result<PyValue, PyBridgeError> {
        Python::attach(|py| {
            let obj = self.lookup(py, handle)?;
            let bound = obj.bind(py);
            let target = bound
                .getattr(attr)
                .map_err(|e| err("getattr", e, py))?;
            // Calling data raises Python's own TypeError, mapped below —
            // same outcome as the default threading, one GIL section.
            let mut tuple_args = Vec::with_capacity(args.len());
            for a in &args {
                tuple_args.push(self.ingest(py, a)?);
            }
            let bound_args = PyTuple::new(py, tuple_args.iter().map(|o| o.bind(py)))
                .map_err(|e| err("call", e, py))?;
            let out = target
                .call1(bound_args)
                .map_err(|e| err("call", e, py))?;
            self.export(py, &out)
        })
    }

    fn release(&self, handle: u32) {
        self.objects
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pybridge::PyBridge;

    /// A Python-raised error carries its message plus formatted frames.
    #[test]
    fn python_errors_chain_tracebacks() {
        let b = PythonBridge::new();
        let json = b.import_module("json").expect("import json");
        let loads = match b.getattr(json, "loads").expect("loads attr") {
            crate::pybridge::PyValue::Handle(h) => h,
            other => panic!("loads should be a handle, got {other:?}"),
        };
        let err = b
            .call(
                loads,
                vec![crate::pybridge::PyValue::Str("{oops".to_string())],
            )
            .expect_err("invalid JSON must raise");
        assert!(
            err.message.contains("Expecting property name"),
            "message should be Python's own: {}",
            err.message
        );
        let tb = err.traceback.expect("traceback must be captured");
        assert!(
            tb.contains("Python traceback") && tb.contains("line"),
            "traceback should name frames, got: {tb}"
        );
    }
}
