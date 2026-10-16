//! Shared helpers for the doc §11 integration tests.
//!
//! See `docs/05-lexer.md` §11.

use zen_syntax::lex::{lex, LexErrorKind, Lexed};
use zen_syntax::span::FileId;
use zen_syntax::TokenKind;

pub const F: FileId = FileId(0);

/// Token kinds of `src`, in order (including `Eof`).
pub fn kinds(src: &str) -> Vec<TokenKind> {
    lex(src, F).toks.into_iter().map(|t| t.kind).collect()
}

/// Kinds of `src` without the trailing `Eof`, so rows can be compared to the
/// doc's "∅ NL …" lists directly.
pub fn kinds_no_eof(src: &str) -> Vec<TokenKind> {
    let mut v = kinds(src);
    assert_eq!(v.pop(), Some(TokenKind::Eof), "Eof must be last");
    v
}

/// Assert that `src` lexes with exactly the given error kinds (in order) and
/// returns normally.
pub fn assert_errors(src: &str, expected: &[LexErrorKind]) {
    let lx = lex(src, F);
    let actual: Vec<LexErrorKind> = lx.errors.iter().map(|e| e.kind.clone()).collect();
    assert_eq!(actual, expected, "error kinds for {src:?}");
}

/// Assert that scanning reaches the end (Eof present) even after errors.
pub fn reaches_eof(src: &str) {
    let lx = lex(src, F);
    assert_eq!(
        lx.toks.last().map(|t| t.kind),
        Some(TokenKind::Eof),
        "{src:?}"
    );
}

pub fn lx(src: &str) -> Lexed<'_> {
    lex(src, F)
}
