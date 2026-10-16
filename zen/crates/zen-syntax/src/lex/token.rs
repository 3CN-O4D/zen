//! Tokens, the lexed result, and its payload accessors.
//!
//! See `docs/05-lexer.md` §2.2–2.3.
//!
//! **Correction to the doc's arithmetic.** It states `Token` is 12 bytes, but
//! `#[repr(C)]` over a `u8` discriminant plus three `u32`s gives a 4-byte
//! alignment and therefore **16** bytes. The side tables still buy the thing the
//! doc was after — a 4-byte-aligned token with no pointer and no `f64` — they
//! just do not buy 12. Four tokens fit in a 64-byte cache line.

use std::borrow::Cow;
use std::sync::Arc;

use super::error::{LexError, LexErrorKind};
use super::interner::Interner;
use super::kind::TokenKind;
use super::number::NumLit;
use crate::diag::{Diagnostic, DiagnosticNote};
use crate::span::{FileId, LineIndex, Span};

/// One token. `Copy`, `PartialEq`, and free of pointers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(C)]
pub struct Token {
    pub kind: TokenKind,
    /// `Ident`/`Str` → interner id, `Interp` → `interps` id, `Int`/`Float` →
    /// `nums` id. Unused otherwise.
    pub aux: u32,
    /// Byte offset of the first byte, inclusive.
    pub lo: u32,
    /// Byte offset one past the last byte, exclusive.
    pub hi: u32,
}

impl Token {
    pub const fn new(kind: TokenKind, aux: u32, lo: u32, hi: u32) -> Self {
        Self { kind, aux, lo, hi }
    }
}

const _: () = assert!(std::mem::size_of::<Token>() == 16);
const _: () = assert!(std::mem::align_of::<Token>() == 4);

/// A cooked run of string text, with the source range it came from.
///
/// `text` borrows the source when the run had no escapes and owns its bytes
/// when escapes were cooked, so `lo`/`hi` still point at real source offsets.
#[derive(Clone, Debug, PartialEq)]
pub struct Range<'a> {
    pub lo: u32,
    pub hi: u32,
    pub text: Cow<'a, str>,
}

impl Range<'_> {
    pub fn as_str(&self) -> &str {
        self.text.as_ref()
    }
}

/// One piece of an interpolated string.
#[derive(Clone, Debug, PartialEq)]
pub enum InterpPart<'a> {
    /// Already-cooked bytes; no interpolation inside.
    Text(Range<'a>),
    /// Index into [`Interp::subs`].
    Expr(u32),
}

/// The payload of a `TokenKind::Interp`.
#[derive(Clone, Debug, PartialEq)]
pub struct Interp<'a> {
    pub parts: Vec<InterpPart<'a>>,
    /// One sub-lex per `Expr` part, in order. Each was lexed from a slice of the
    /// same source buffer, so every span in it is relative to the whole file.
    pub subs: Vec<Lexed<'a>>,
}

impl Interp<'_> {
    /// Reassemble the literal's text, with each interpolation rendered as its
    /// source text. Diagnostics and the REPL use this; the runtime does not.
    pub fn source_text(&self) -> String {
        let mut out = String::new();
        for part in &self.parts {
            match part {
                InterpPart::Text(range) => out.push_str(range.as_str()),
                InterpPart::Expr(id) => {
                    let idx = usize::try_from(*id).unwrap_or(usize::MAX);
                    out.push('{');
                    if let Some(sub) = self.subs.get(idx) {
                        out.push_str(&sub.source_of_tokens());
                    }
                    out.push('}');
                }
            }
        }
        out
    }
}

/// Which of the five string forms a literal used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StrForm {
    Double,
    Single,
    TripleDouble,
    TripleSingle,
    Backtick,
}

impl StrForm {
    #[inline]
    pub const fn quote(self) -> u8 {
        match self {
            StrForm::Double | StrForm::TripleDouble => b'"',
            StrForm::Single | StrForm::TripleSingle => b'\'',
            StrForm::Backtick => b'`',
        }
    }

    #[inline]
    pub const fn is_triple(self) -> bool {
        matches!(self, StrForm::TripleDouble | StrForm::TripleSingle)
    }

    /// Whether a raw newline may appear inside.
    #[inline]
    pub const fn multiline(self) -> bool {
        self.is_triple()
    }

    /// Whether `${…}` interpolation applies. `'''` does **not** interpolate,
    /// preserved from the legacy `is_interpolated` test.
    #[inline]
    pub const fn interpolates(self) -> bool {
        matches!(
            self,
            StrForm::Double | StrForm::TripleDouble | StrForm::Backtick
        )
    }

    /// Length of the opening quote sequence.
    #[inline]
    pub const fn open_len(self) -> u32 {
        if self.is_triple() {
            3
        } else {
            1
        }
    }

    pub const fn open_text(self) -> &'static str {
        match self {
            StrForm::Double => "\"",
            StrForm::Single => "'",
            StrForm::TripleDouble => "\"\"\"",
            StrForm::TripleSingle => "'''",
            StrForm::Backtick => "`",
        }
    }
}

impl std::fmt::Display for StrForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.open_text())
    }
}

/// Everything one `lex` call produced: the token stream plus its side tables.
///
/// `errors` holds the diagnostics from *this* level only. Diagnostics raised
/// inside a `${…}` body live in the sub-lex; [`Lexed::all_errors`] walks them.
pub struct Lexed<'a> {
    pub file: FileId,
    /// Always ends with exactly one `TokenKind::Eof`.
    pub toks: Vec<Token>,
    pub idents: Interner<'a, str>,
    pub strs: Interner<'a, [u8]>,
    pub interps: Vec<Interp<'a>>,
    pub nums: Vec<NumLit>,
    /// Byte offset → line/col, shared by every sub-lex of the same file so an
    /// interpolation's diagnostics report file-absolute lines.
    pub lines: Arc<LineIndex<'a>>,
    pub errors: Vec<LexError>,
}

impl std::fmt::Debug for Lexed<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lexed")
            .field("file", &self.file)
            .field("tokens", &self.toks.len())
            .field("idents", &self.idents.len())
            .field("strs", &self.strs.len())
            .field("interps", &self.interps.len())
            .field("nums", &self.nums.len())
            .field("errors", &self.errors.len())
            .finish()
    }
}

impl Clone for Lexed<'_> {
    fn clone(&self) -> Self {
        Self {
            file: self.file,
            toks: self.toks.clone(),
            idents: self.idents.clone(),
            strs: self.strs.clone(),
            interps: self.interps.clone(),
            nums: self.nums.clone(),
            lines: self.lines.clone(),
            errors: self.errors.clone(),
        }
    }
}

impl PartialEq for Lexed<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.file == other.file
            && self.toks == other.toks
            && self.idents == other.idents
            && self.strs == other.strs
            && self.interps == other.interps
            && self.nums == other.nums
            && self.errors == other.errors
    }
}

impl<'a> Lexed<'a> {
    /// The identifier text of an `Ident` token.
    #[inline]
    pub fn ident(&self, t: &Token) -> &str {
        self.idents.get(t.aux).unwrap_or("")
    }

    /// The cooked bytes of a `Str` token. Always valid UTF-8, per §6.4.
    #[inline]
    pub fn str_bytes(&self, t: &Token) -> &[u8] {
        self.strs.get(t.aux).unwrap_or(&[])
    }

    #[inline]
    pub fn str_text(&self, t: &Token) -> &str {
        std::str::from_utf8(self.str_bytes(t)).unwrap_or("")
    }

    /// The value of an `Int` or `Float` token.
    #[inline]
    pub fn num(&self, t: &Token) -> Option<NumLit> {
        self.nums
            .get(usize::try_from(t.aux).unwrap_or(usize::MAX))
            .copied()
    }

    /// The payload of an `Interp` token.
    #[inline]
    pub fn interp(&self, t: &Token) -> Option<&Interp<'a>> {
        self.interps
            .get(usize::try_from(t.aux).unwrap_or(usize::MAX))
    }

    #[inline]
    pub fn span(&self, t: &Token) -> Span {
        Span::new(self.file, t.lo, t.hi)
    }

    /// The canonical source text of a token, payload included. This is what a
    /// diagnostic prints after "found", so it teaches the preferred spelling.
    pub fn describe(&self, t: &Token) -> String {
        match t.kind {
            TokenKind::Ident => self.ident(t).to_string(),
            TokenKind::Str => format!("\"{}\"", self.str_text(t)),
            TokenKind::Interp => match self.interp(t) {
                Some(i) => format!("\"{}\"", i.source_text()),
                None => "\"<interp>\"".to_string(),
            },
            TokenKind::Int | TokenKind::Float => match self.num(t) {
                Some(NumLit::Int(v)) => v.to_string(),
                Some(NumLit::Float(v)) => format!("{v}"),
                None => t.kind.to_string(),
            },
            other => other.to_string(),
        }
    }

    /// A readable rendering of the token stream, one token per space. Used for
    /// sub-lex bodies in diagnostics and by `zentok`.
    pub fn source_of_tokens(&self) -> String {
        let parts: Vec<String> = self
            .toks
            .iter()
            .filter(|t| !matches!(t.kind, TokenKind::Eof))
            .map(|t| self.describe(t))
            .collect();
        parts.join(" ")
    }

    /// Every error in this lex and, recursively, in every `${…}` sub-lex.
    pub fn all_errors(&self) -> Vec<&LexError> {
        let mut out: Vec<&LexError> = self.errors.iter().collect();
        for interp in &self.interps {
            for sub in &interp.subs {
                out.extend(sub.all_errors());
            }
        }
        out
    }

    /// Errors only, as renderable diagnostics.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        self.all_errors()
            .iter()
            .map(|e| e.to_diagnostic())
            .collect()
    }
}

impl LexError {
    /// Renderable form, carrying the notes the message table prescribes.
    pub fn to_diagnostic(&self) -> Diagnostic {
        let mut d = Diagnostic::new(self.severity, self.kind.message(), self.at);
        d.notes = self.notes.clone();
        if self.notes.is_empty() {
            if let Some(note) = self.kind.default_note() {
                d.notes.push(note);
            }
        }
        d
    }
}

impl LexErrorKind {
    /// The `= help:` / `= note:` line that goes with this error, where the
    /// legacy house style has one.
    pub fn default_note(&self) -> Option<DiagnosticNote> {
        Some(match self {
            LexErrorKind::UnterminatedString { .. } => {
                DiagnosticNote::help("add a matching quote to close the string")
            }
            LexErrorKind::UnterminatedTripleString { .. } => {
                DiagnosticNote::help("add a matching `'''` or `\"\"\"` to close the string")
            }
            LexErrorKind::UnterminatedBlockComment => {
                DiagnosticNote::help("add `*/` to close the comment block")
            }
            LexErrorKind::UnterminatedInterpolation { .. } => {
                DiagnosticNote::help("add `}` to close the `${…}` interpolation")
            }
            LexErrorKind::RawNewlineInString { .. } => DiagnosticNote::note(
                "use `\"\"\"…\"\"\"` for a multi-line string, or a `\\` continuation",
            ),
            LexErrorKind::StrayLineContinuation => {
                DiagnosticNote::note("a line continuation is only valid inside a string literal")
            }
            LexErrorKind::TrailingLineContinuation => {
                DiagnosticNote::note("the file ends with an unfinished line continuation")
            }
            LexErrorKind::UnknownEscape { byte } => DiagnosticNote::note(format!(
                "`\\{}` is not a known escape; it means `{}`",
                *byte as char, *byte as char
            )),
            LexErrorKind::InvalidNumber { reason } => DiagnosticNote::note(reason.message()),
            LexErrorKind::NestingTooDeep { .. } => {
                DiagnosticNote::note("this is usually an unbalanced bracket")
            }
            _ => return None,
        })
    }
}

pub use super::number::NumErr as NumberError;
