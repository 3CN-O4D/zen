//! Compiler error types.

use thiserror::Error;
use zen_syntax::span::Span;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum CompileError {
    #[error("{span}: undefined variable `{name}`")]
    UndefinedVariable { span: Span, name: String },

    #[error("{span}: variable `{name}` already declared in this scope")]
    RedeclaredVariable { span: Span, name: String },

    #[error("{span}: cannot assign to constant `{name}`")]
    AssignToConst { span: Span, name: String },

    #[error("{span}: break outside of loop")]
    BreakOutsideLoop { span: Span },

    #[error("{span}: continue outside of loop")]
    ContinueOutsideLoop { span: Span },

    #[error("{span}: return outside of function")]
    ReturnOutsideFunction { span: Span },

    #[error("{span}: too many local variables (max 65535)")]
    TooManyLocals { span: Span },

    #[error("{span}: too many simultaneously live registers (max 250)")]
    TooManyRegisters { span: Span },

    #[error("{span}: {what} is not supported by the M4 compiler yet")]
    Unsupported { span: Span, what: String },

    #[error("{span}: too many constants (max 65535)")]
    TooManyConstants { span: Span },

    #[error("{span}: too many upvalues (max 65535)")]
    TooManyUpvalues { span: Span },

    #[error("{span}: jump offset out of range")]
    JumpOffsetOverflow { span: Span },

    #[error("{span}: invalid match arm pattern")]
    InvalidMatchPattern { span: Span },

    #[error("{span}: pattern binds different names in alternatives")]
    OrPatternBindingMismatch { span: Span },

    #[error("{span}: rest pattern must be last")]
    RestPatternNotLast { span: Span },

    #[error("{span}: only one rest pattern allowed")]
    MultipleRestPatterns { span: Span },

    #[error("{span}: duplicate dict key")]
    DuplicateDictKey { span: Span },

    #[error("{span}: invalid comprehension clause")]
    InvalidComprehension { span: Span },

    #[error("{span}: super call outside of class")]
    SuperOutsideClass { span: Span },

    #[error("{span}: super method call outside of class")]
    SuperMethodOutsideClass { span: Span },

    #[error("{span}: pattern destructuring not supported here")]
    UnsupportedDestructuring { span: Span },

    #[error("{span}: {msg}")]
    Custom { span: Span, msg: String },
}

impl CompileError {
    pub fn span(&self) -> Span {
        match self {
            CompileError::UndefinedVariable { span, .. }
            | CompileError::RedeclaredVariable { span, .. }
            | CompileError::AssignToConst { span, .. }
            | CompileError::BreakOutsideLoop { span }
            | CompileError::ContinueOutsideLoop { span }
            | CompileError::ReturnOutsideFunction { span }
            | CompileError::TooManyLocals { span }
            | CompileError::TooManyConstants { span }
            | CompileError::TooManyUpvalues { span }
            | CompileError::JumpOffsetOverflow { span }
            | CompileError::InvalidMatchPattern { span }
            | CompileError::OrPatternBindingMismatch { span }
            | CompileError::RestPatternNotLast { span }
            | CompileError::MultipleRestPatterns { span }
            | CompileError::DuplicateDictKey { span }
            | CompileError::InvalidComprehension { span }
            | CompileError::SuperOutsideClass { span }
            | CompileError::SuperMethodOutsideClass { span }
            | CompileError::UnsupportedDestructuring { span }
            | CompileError::TooManyRegisters { span }
            | CompileError::Unsupported { span, .. }
            | CompileError::Custom { span, .. } => *span,
        }
    }
}

pub type CompileResult<T> = Result<T, CompileError>;

#[derive(Debug, Clone, Default)]
pub struct CompileErrorCollector {
    errors: Vec<CompileError>,
}

impl CompileErrorCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, err: CompileError) {
        self.errors.push(err);
    }

    pub fn extend(&mut self, errs: Vec<CompileError>) {
        self.errors.extend(errs);
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn into_errors(self) -> Vec<CompileError> {
        self.errors
    }

    pub fn errors(&self) -> &[CompileError] {
        &self.errors
    }
}

impl From<Vec<CompileError>> for CompileErrorCollector {
    fn from(errors: Vec<CompileError>) -> Self {
        Self { errors }
    }
}
