//! Lexer diagnostics: one error type, one severity field, and recovery that is
//! bounded by wrong *lines* rather than wrong bytes.
//!
//! See `docs/05-lexer.md` §9.

use crate::diag::{DiagnosticNote, Severity};
use crate::span::Span;

use super::number::NumErr;
use super::token::StrForm;

/// A recorded problem. The lexer never aborts; it records and keeps scanning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    /// Per-error rather than per-`Lexed`, so an unknown-escape *warning* can
    /// coexist with hard errors in the same run (RULING S5).
    pub severity: Severity,
    /// The full offending region.
    pub span: Span,
    /// Where the caret goes — often the opening quote rather than the whole run.
    pub at: Span,
    pub notes: Vec<DiagnosticNote>,
}

impl LexError {
    pub fn new(kind: LexErrorKind, severity: Severity, span: Span, at: Span) -> Self {
        Self {
            kind,
            severity,
            span,
            at,
            notes: Vec::new(),
        }
    }

    pub fn error(kind: LexErrorKind, span: Span, at: Span) -> Self {
        Self::new(kind, Severity::Error, span, at)
    }

    pub fn warning(kind: LexErrorKind, span: Span, at: Span) -> Self {
        Self::new(kind, Severity::Warning, span, at)
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LexErrorKind {
    UnterminatedString {
        form: StrForm,
        line: u32,
    },
    UnterminatedTripleString {
        form: StrForm,
    },
    UnterminatedBlockComment,
    UnterminatedInterpolation {
        expr_line: u32,
        expr_col: u32,
    },
    RawNewlineInString {
        line: u32,
    },
    UnfinishedEscape,
    /// Warning only (RULING S5): `\q` means `q`.
    UnknownEscape {
        byte: u8,
    },
    IncompleteHexEscape,
    InvalidHexEscape,
    InvalidUnicodeEscape {
        tag: u8,
    },
    UnterminatedUnicodeEscape {
        tag: u8,
    },
    EmptyUnicodeEscape {
        tag: u8,
    },
    UnicodeEscapeTooLong {
        tag: u8,
    },
    InvalidCodePoint {
        cp: u32,
    },
    InvalidNumber {
        reason: NumErr,
    },
    UnexpectedCharacter {
        ch: char,
    },
    StrayLineContinuation,
    TrailingLineContinuation,
    NestingTooDeep {
        depth: u32,
    },
    TooManyErrors {
        limit: u32,
    },
}

impl LexErrorKind {
    /// The message stem, preserved verbatim from the legacy lexer wherever an
    /// equivalent message exists.
    pub fn message(&self) -> String {
        match self {
            LexErrorKind::UnterminatedString { form, .. } => {
                format!("unterminated {form} string literal")
            }
            LexErrorKind::UnterminatedTripleString { .. } => {
                "unterminated string literal".to_string()
            }
            LexErrorKind::UnterminatedBlockComment => "unterminated block comment".to_string(),
            LexErrorKind::UnterminatedInterpolation { .. } => {
                "unterminated `${…}` interpolation".to_string()
            }
            LexErrorKind::RawNewlineInString { .. } => {
                "raw newline in a single-line string".to_string()
            }
            LexErrorKind::UnfinishedEscape => "unfinished escape".to_string(),
            LexErrorKind::UnknownEscape { byte } => {
                format!("unknown escape `\\{}`", *byte as char)
            }
            LexErrorKind::IncompleteHexEscape => {
                "incomplete `\\xNN` escape, expected two hex digits".to_string()
            }
            LexErrorKind::InvalidHexEscape => {
                "invalid `\\xNN` escape, expected two hex digits".to_string()
            }
            LexErrorKind::InvalidUnicodeEscape { tag } => {
                format!(
                    "invalid `\\{{{}}}` escape, expected `{{` after `\\{}`",
                    *tag as char, *tag as char
                )
            }
            LexErrorKind::UnterminatedUnicodeEscape { tag } => {
                format!("unterminated `\\{{{}}}` escape", *tag as char)
            }
            LexErrorKind::EmptyUnicodeEscape { tag } => {
                format!("empty `\\{{{}}}` escape", *tag as char)
            }
            LexErrorKind::UnicodeEscapeTooLong { tag } => {
                format!("`\\{{{}}}` escape has too many digits", *tag as char)
            }
            LexErrorKind::InvalidCodePoint { cp } => {
                format!("invalid unicode code point U+{cp:04X}")
            }
            LexErrorKind::InvalidNumber { reason } => match reason {
                NumErr::SecondDot => {
                    "multi-dot literals are not allowed; write `1.2` or `1..2`".to_string()
                }
                NumErr::BadSeparator => "`_` may only appear between digits".to_string(),
                other => other.message().to_string(),
            },
            LexErrorKind::UnexpectedCharacter { ch } => {
                format!("unexpected character `{ch}`")
            }
            LexErrorKind::StrayLineContinuation => {
                "a line continuation is only valid inside a string literal".to_string()
            }
            LexErrorKind::TrailingLineContinuation => {
                "the file ends with an unfinished line continuation".to_string()
            }
            LexErrorKind::NestingTooDeep { depth } => {
                format!("nesting deeper than {depth} delimiters")
            }
            LexErrorKind::TooManyErrors { limit } => {
                format!("more than {limit} errors; stopping reporting")
            }
        }
    }
}
