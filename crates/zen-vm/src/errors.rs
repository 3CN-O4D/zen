//! VM-level runtime error types.
//!
//! These mirror the error classes in [`docs/04-value-and-types.md`] §10 and
//! [`docs/13-errors.md`]. A `VmError` becomes a `Value` of kind `error` when it
//! crosses into Zen code, where it is catchable by the `try`/`catch` forms.

use thiserror::Error;

use crate::value::Value;

#[derive(Debug, Clone, Error)]
pub enum VmError {
    #[error("TypeError: {0}")]
    TypeError(String),
    #[error("IndexError: index {index} out of bounds for length {len}")]
    IndexError { index: i64, len: usize },
    #[error("KeyError: {0}")]
    KeyError(String),
    #[error("NameError: {0}")]
    NameError(String),
    #[error("ValueError: {0}")]
    ValueError(String),
    #[error("DivideByZero: {0}")]
    DivideByZero(String),
    #[error("StackOverflow: {0}")]
    StackOverflow(String),
    #[error("ArgumentCount: expected {expected}, got {got}")]
    ArgumentCount { expected: usize, got: usize },
    #[error("HandleError: {0}")]
    HandleError(String),
    #[error("IOMismatch: {0}")]
    IOMismatch(String),
    #[error("NativeError: {0}")]
    NativeError(String),
    /// A `pymport` import failed (unknown module, unlinked bridge, broken
    /// venv). The Python message rides along; `traceback` carries formatted
    /// CPython frames when the failure raised inside Python.
    #[error("PyImportError: {message}")]
    PyImport {
        message: String,
        traceback: Option<String>,
    },
    /// A foreign attribute read/write failed (missing member, version drift).
    #[error("ForeignAttributeError: {message}")]
    ForeignAttribute {
        message: String,
        traceback: Option<String>,
    },
    /// A foreign call failed (bad args, Python-side exception).
    #[error("ForeignCallError: {message}")]
    ForeignCall {
        message: String,
        traceback: Option<String>,
    },
    /// A value could not cross the Python boundary in either direction
    /// (never a lossy coercion — the operation fails instead).
    #[error("MarshalError: {message}")]
    Marshal { message: String },
    #[error("UserError: {0}")]
    UserError(String),
    /// A Rust panic escaped a native call. Catchable in Zen.
    #[error("Panic: {0}")]
    Panic(String),
}

impl VmError {
    /// The Zen error class name, as reported by `typeof`/`err.name`.
    pub const fn kind(&self) -> &'static str {
        match self {
            VmError::TypeError(_) => "TypeError",
            VmError::IndexError { .. } => "IndexError",
            VmError::KeyError(_) => "KeyError",
            VmError::NameError(_) => "NameError",
            VmError::ValueError(_) => "ValueError",
            VmError::DivideByZero(_) => "ZeroDivisionError",
            VmError::StackOverflow(_) => "RecursionError",
            VmError::ArgumentCount { .. } => "TypeError",
            VmError::HandleError(_) => "HandleError",
            VmError::IOMismatch(_) => "IOMismatch",
            VmError::NativeError(_) => "NativeError",
            VmError::PyImport { .. } => "PyImportError",
            VmError::ForeignAttribute { .. } => "ForeignAttributeError",
            VmError::ForeignCall { .. } => "ForeignCallError",
            VmError::Marshal { .. } => "MarshalError",
            VmError::UserError(_) => "UserError",
            VmError::Panic(_) => "PanicError",
        }
    }

    /// A `TypeError` for a value whose `typeof` did not match `expected`.
    pub fn type_mismatch(expected: &str, got: Value) -> VmError {
        VmError::TypeError(format!("expected {expected}, got {}", got.type_name()))
    }

    /// Convert to a heap `error` value for `throw`/`catch`.
    pub fn into_value(self) -> Value {
        crate::gc::new_error(std::sync::Arc::from(self.to_string()), 0, Value::nil())
    }
}

pub type VmResult<T> = Result<T, VmError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_kinds_are_stable() {
        assert_eq!(VmError::TypeError(String::new()).kind(), "TypeError");
        assert_eq!(
            VmError::IndexError { index: 3, len: 2 }.kind(),
            "IndexError"
        );
        assert_eq!(
            VmError::DivideByZero(String::new()).kind(),
            "ZeroDivisionError"
        );
        assert_eq!(VmError::Panic(String::new()).kind(), "PanicError");
    }

    #[test]
    fn type_mismatch_reports_typeof() {
        let e = VmError::type_mismatch("int", Value::bool(true));
        assert_eq!(e.to_string(), "TypeError: expected int, got bool");
    }

    #[test]
    fn index_error_message() {
        let e = VmError::IndexError { index: 5, len: 3 };
        assert_eq!(
            e.to_string(),
            "IndexError: index 5 out of bounds for length 3"
        );
    }
}
