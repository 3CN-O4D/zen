//! Zen language syntax: lexer, AST, parser, diagnostics.
//!
//! See `docs/05-lexer.md` (lexer contract) and `docs/06-parser.md` (parser).

pub mod ast;
pub mod diag;
pub mod lex;
pub mod parser;
pub mod source;
pub mod span;

pub use diag::{Diagnostic, DiagnosticNote, Severity};
pub use lex::token::{Interp, InterpPart, Range};
pub use lex::{lex, LexError, LexErrorKind, Lexed, NumErr, NumLit, StrForm, Token, TokenKind};
pub use span::{FileId, LineIndex, Span};
