//! The lexer: source → [`Lexed`].
//!
//! See `docs/05-lexer.md`, which is the normative contract. Deadlines worth
//! repeating: never `panic`/`unwrap`, never index a slice without a guard, and
//! every byte of input is consumed exactly once by exactly one branch of
//! [`Lexer::next_token`].
//!
//! The hard rules that shape the loop:
//!
//! - **RULING C1 / A1.** The leaky-delimiter set (`LParen`, `LBracket`,
//!   `DictStart`, `SetStart`, `TupleStart`, `LBrace`) puts a *suppressing*
//!   entry on the delimiter stack for its whole open extent, so newlines inside
//!   `f(a,\nb)` do not break the statement. Only `(` and `[` (plus the sigils)
//!   suppress; `{` does **not**, because a newline closes a block.
//! - **A2.** Otherwise a newline is suppressed iff the previous token
//!   `continues_expression` (`Dot`, `Colon`, binary operators, …) or no
//!   significant token has been seen yet (A4: the file's first line).
//! - **A3 / ASI-2.** A *pending* newline is flushed lazily, just before a
//!   significant token, a friendlier version of the legacy rule. A `;` at that
//!   moment subsumes the pending newline, so `a = 1\n;\n` lexes `a Semi`.
//! - **A5.** Line comments consume up to but *not* the newline (so a `#`-"key"
//!   on its own line still breaks the statement); block comments consume
//!   through `*/`, embedded newlines included. Comments change no state.
//! - **Recovery.** A bad number emits its valid prefix (`1.2.3` → `FLT(1.2)
//!   FLT(0.3)` + one error). A bad string records one error and resyncs to the
//!   end of the physical line, so a real file produces one error per bad line.
//! - **Rule of two.** Interpolated bodies are sub-lexed with a fresh [`Lexer`],
//!   sharing the parent's `Arc<LineIndex>`, at `depth + 1`; escaping depth 8
//!   is an error (T-S29), as is a delimiter stack over 256 (T-E6), after which
//!   the lexer degrades and every remaining `(`/`[` is plain punctuation.
//!
//! The class table splits the work before this loop sees a byte:
//!
//! | byte class        | handled by                              |
//! |-------------------|-----------------------------------------|
//! | Digit (`0-9`)     | [`Lexer::lex_number`]                    |
//! | `.` + digit       | [`Lexer::lex_number`] (§5.1: `.5`)       |
//! | Ident-start       | [`Lexer::lex_word`]                      |
//! | `"`, `'`, `` ` `` | [`Lexer::lex_string`]                    |
//! | `#`, `#{`, `#(`   | [`Lexer::lex_hash`] (comment or sigil)  |
//! | `/`, `//`, `/*`   | operator, or a comment skip              |
//! | newline, space    | [`Lexer::on_newline`], inline skip       |
#![allow(clippy::missing_docs_in_private_items)]
#![allow(clippy::arithmetic_side_effects)]

use std::borrow::Cow;
use std::sync::Arc;

pub mod error;
mod interner;
pub mod kind;
mod number;
pub mod string;
pub mod table;
pub mod token;

pub use self::error::{LexError, LexErrorKind};
pub use self::kind::TokenKind;
pub use self::number::{NumErr, NumLit};
pub use self::table::Class;
pub use self::token::{Interp, InterpPart, Lexed, Range, StrForm, Token};

use crate::diag::Severity;
use crate::span::{FileId, LineIndex, Span};

use self::interner::Interner;
use self::kind::keyword_of;
use self::number::{scan_number, NumScan};
use self::string::{scan_string, ScanPart};
use self::table::{class_of, is_ident_continue, OPERATORS, OP_RANGE};

/// Recovery cap (§11): more than this and scanning stops recording; a single
/// `TooManyErrors` is appended so the caller still sees *something*.
pub const MAX_LEX_ERRORS: usize = 64;

/// T-E6: `(` / `[` nesting beyond this turns the lexer degraded.
pub const MAX_DELIM_DEPTH: usize = 256;

/// One entry of the delimiter stack.
///
/// `suppress` is true exactly for the leaky-delimiter set, and is what makes
/// `f(a,\nb)` one statement (A1) while a `{`-block still closes on a newline
/// (the stack entry is *non*-suppressing: the `LBrace` token's own A2 bit does
/// the block's first newline, and later ones close the block).
#[derive(Clone, Copy)]
struct Delim {
    byte: u8,
    suppress: bool,
}

/// Lexer state. A sub-lexer's `src` is a *slice* of its parent's source, so a
/// [`Lexer`] is an owned value that moves into an iterator without borrows.
pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    /// The file this source (and every sub-lexer of it) belongs to.
    file: FileId,
    /// Byte offset inside the file of this lexer's `src[0]`.
    base: usize,
    /// Interpolation nesting depth (0 at the top level; T-S29 caps at 8).
    depth: u32,
    /// Next unconsumed byte.
    i: usize,

    /// Open `(`/`[`/`{`-family delimiters.
    delims: Vec<Delim>,
    /// Last significant token, for A2.
    last: TokenKind,
    has_last: bool,
    /// Whether the last pushed token was `Dot`/`SafeDot` (K1).
    after_dot: bool,
    /// Whether a separator may be about to close the statement.
    sep_pending: bool,
    /// The newline waiting to be flushed (A3). `Some` only while `sep_pending`.
    pending_nl: Option<(usize, usize)>,
    /// T-E6: a huge nesting blew up; `delims` stopped tracking.
    degraded: bool,
    /// Recovery cap reached; `record` appends nothing more.
    errors_hit_cap: bool,

    /// Shared by every sub-lexer: interpolation diagnostics report
    /// file-absolute lines/columns.
    lines: Arc<LineIndex<'a>>,

    toks: Vec<Token>,
    idents: Interner<'a, str>,
    strs: Interner<'a, [u8]>,
    interps: Vec<Interp<'a>>,
    nums: Vec<NumLit>,
    errors: Vec<LexError>,
}

impl<'a> Lexer<'a> {
    /// Lex a whole file.
    pub fn new(src: &'a str, file: FileId) -> Self {
        let lines = Arc::new(LineIndex::build(src));
        Self::new_at(src, file, 0, 0, lines)
    }

    /// Lex `src`, whose first byte is file-offset `base`. Sub-lexers use this
    /// with the parent's `lines`, so their spans are file-absolute.
    pub fn new_at(
        src: &'a str,
        file: FileId,
        base: usize,
        depth: u32,
        lines: Arc<LineIndex<'a>>,
    ) -> Self {
        let mut i = 0;
        // T-A15: a UTF-8 BOM is skipped once, at the start of the *file*; a
        // sub-lexer's `${...}` body never begins with one.
        if base == 0 && is_bom_at(src.as_bytes(), 0) {
            i = 3;
        }
        Self {
            src,
            bytes: src.as_bytes(),
            file,
            base,
            depth,
            i,
            delims: Vec::with_capacity(16),
            last: TokenKind::Eof,
            has_last: false,
            after_dot: false,
            sep_pending: false,
            pending_nl: None,
            degraded: false,
            errors_hit_cap: false,
            lines,
            toks: Vec::new(),
            idents: Interner::new(),
            strs: Interner::new(),
            interps: Vec::new(),
            nums: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Lex to the end and collect a [`Lexed`].
    pub fn run(&mut self) -> Lexed<'a> {
        self.toks.reserve(self.src.len() / 4 + 16);
        while self.next_token().is_some() {}
        self.finish()
    }

    /// The next token, or `None` at the end of input. `Eof` is emitted by
    /// [`Lexer::finish`]. For the REPL (M8): call repeatedly, then `finish()`
    /// and read `errors`/`Eof` from the collected lexed.
    #[inline]
    pub fn next_token(&mut self) -> Option<Token> {
        let n = self.src.len();
        loop {
            if self.i >= n {
                return None;
            }
            let prev_i = self.i;
            let i = self.i;
            let b = self.bytes.get(i).copied().unwrap_or(0);
            match class_of(b) {
                Class::Space => self.i += 1,
                Class::Newline => self.on_newline(),
                Class::Semi => self.on_semi(),
                Class::Hash => self.lex_hash(),
                // A token producer returns `None` after it has *consumed* input
                // and recorded a diagnostic. That is not end-of-input: `None`
                // from `next_token` means "source exhausted", so we must keep
                // scanning or the rest of the file is silently dropped.
                Class::Quote => {
                    if let Some(t) = self.lex_string() {
                        return Some(t);
                    }
                }
                Class::Digit => {
                    if let Some(t) = self.lex_number() {
                        return Some(t);
                    }
                }
                Class::IdentStart => {
                    if let Some(t) = self.lex_word() {
                        return Some(t);
                    }
                }
                Class::Slash => match self.bytes.get(i + 1) {
                    Some(b'/') => self.skip_line_comment(),
                    Some(b'*') => self.skip_block_comment(),
                    _ => {
                        if let Some(t) = self.lex_operator() {
                            return Some(t);
                        }
                    }
                },
                Class::Operator => {
                    // §5.1: `.` followed by a digit enters the number scanner
                    // even though the class table calls `.` an operator.
                    let is_number =
                        b == b'.' && self.bytes.get(i + 1).is_some_and(|c| c.is_ascii_digit());
                    if let Some(t) = if is_number {
                        self.lex_number()
                    } else {
                        self.lex_operator()
                    } {
                        return Some(t);
                    }
                }
                Class::Illegal => self.lex_illegal(),
            }
            debug_assert!(self.i > prev_i, "the main loop must advance");
        }
    }

    /// Push `kind` covering bytes `[lo, hi)`, doing delimiter bookkeeping.
    #[inline]
    pub fn push_token(&mut self, kind: TokenKind, aux: u32, lo: usize, hi: usize) -> Token {
        match kind {
            TokenKind::LParen
            | TokenKind::LBracket
            | TokenKind::DictStart
            | TokenKind::SetStart
            | TokenKind::TupleStart
            | TokenKind::LBrace => self.push_delim(kind),
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => self.pop_delim(kind),
            _ => {}
        }
        self.push(kind, aux, lo, hi)
    }

    /// Push a token without touching the delimiter stack.
    #[inline]
    pub fn push(&mut self, kind: TokenKind, aux: u32, lo: usize, hi: usize) -> Token {
        debug_assert!(
            lo <= hi && hi <= self.src.len(),
            "token span is inside the source"
        );
        if !matches!(kind, TokenKind::Newline | TokenKind::Semi) {
            // A3: flush the pending newline just before a significant token.
            if let Some((plo, phi)) = self.pending_nl.take() {
                self.push_newline(plo, phi);
            }
            self.sep_pending = false;
            self.last = kind;
            self.has_last = true;
        }
        self.after_dot = matches!(kind, TokenKind::Dot | TokenKind::SafeDot);
        let t = Token::new(kind, aux, self.abs(lo), self.abs(hi));
        self.toks.push(t);
        t
    }

    /// Push the physical newline at `[lo, hi)`.
    #[inline]
    fn push_newline(&mut self, lo: usize, hi: usize) -> Token {
        self.after_dot = false;
        let t = Token::new(TokenKind::Newline, 0, self.abs(lo), self.abs(hi));
        self.toks.push(t);
        t
    }

    #[inline]
    fn push_delim(&mut self, kind: TokenKind) {
        if self.degraded {
            return;
        }
        if self.delims.len() >= MAX_DELIM_DEPTH {
            let depth = MAX_DELIM_DEPTH as u32;
            let s = self.span(0, self.src.len());
            self.record(LexError::error(
                LexErrorKind::NestingTooDeep { depth },
                s,
                s,
            ));
            self.degraded = true;
            return;
        }
        self.delims.push(Delim {
            byte: TokenKind::delim_byte(kind).unwrap_or(0),
            suppress: TokenKind::opener_suppresses_newline(kind),
        });
    }

    #[inline]
    fn pop_delim(&mut self, kind: TokenKind) {
        if self.degraded {
            return;
        }
        match self.delims.last() {
            Some(top) if top.byte == TokenKind::delim_byte(kind).unwrap_or(0) => {
                self.delims.pop();
            }
            _ => {}
        }
    }

    /// `#`, `#(`, `#{`: RULING H1, a one-byte lookahead truth table.
    fn lex_hash(&mut self) {
        let i = self.i;
        match self.bytes.get(i + 1) {
            Some(b'{') => {
                self.i += 2;
                self.push_token(TokenKind::DictStart, 0, i, i + 2);
            }
            Some(b'(') => {
                self.i += 2;
                self.push_token(TokenKind::TupleStart, 0, i, i + 2);
            }
            // A bare `#` begins a line comment (§4.2, row 18).
            _ => self.skip_line_comment(),
        }
    }

    /// Consume `//…` and the `#` comment, but **not** the newline (row 18).
    fn skip_line_comment(&mut self) {
        while let Some(&b) = self.bytes.get(self.i) {
            if b == b'\n' {
                break;
            }
            self.i += 1;
        }
    }

    /// Consume `/* … */`, embedded newlines included, or error to EOF (§9.2).
    fn skip_block_comment(&mut self) {
        let start = self.i;
        self.i = self.i.saturating_add(2);
        let mut closed = false;
        while let Some(&b) = self.bytes.get(self.i) {
            if b == b'*' && self.bytes.get(self.i + 1) == Some(&b'/') {
                self.i = self.i.saturating_add(2);
                closed = true;
                break;
            }
            self.i += 1;
        }
        if !closed {
            let s = self.span(start, self.src.len());
            self.record(LexError::error(
                LexErrorKind::UnterminatedBlockComment,
                s,
                s,
            ));
        }
    }

    /// Keywords, or an identifier, interned. `after_dot` forces `Ident`.
    fn lex_word(&mut self) -> Option<Token> {
        let start = self.i;
        let mut j = start;
        while let Some(&b) = self.bytes.get(j) {
            if is_ident_continue(b) {
                j += 1;
            } else {
                break;
            }
        }
        self.i = j;
        let text = std::str::from_utf8(self.bytes.get(start..j).unwrap_or(&[])).unwrap_or("");
        let kind = if self.after_dot {
            TokenKind::Ident
        } else {
            keyword_of(text).unwrap_or(TokenKind::Ident)
        };
        if matches!(kind, TokenKind::Ident) {
            let id = self.idents.intern(text);
            Some(self.push_token(TokenKind::Ident, id, start, j))
        } else {
            Some(self.push_token(kind, 0, start, j))
        }
    }

    /// Longest-prefix operator match against the class table's bucket.
    fn lex_operator(&mut self) -> Option<Token> {
        let start = self.i;
        let b = self.bytes.get(start).copied().unwrap_or(0);
        let (rlo, rhi) = OP_RANGE.get(usize::from(b)).copied().unwrap_or((0, 0));
        let bucket = OPERATORS
            .get(usize::from(rlo)..usize::from(rhi))
            .unwrap_or(&[]);
        let rest = self.bytes.get(start..).unwrap_or(&[]);
        // Longest-first: the first prefix match is the operator (§5.2).
        let hit = bucket
            .iter()
            .find(|(text, _)| rest.starts_with(text.as_bytes()));
        let Some((text, kind)) = hit else {
            // Table invariant (table.rs): every recognised operator byte has a
            // 1-byte fallback in its bucket. This arm is a belt-and-braces
            // guard against a hypothetical gap: report the byte and advance.
            self.error_kind(
                LexErrorKind::UnexpectedCharacter { ch: b as char },
                start,
                start + 1,
                start,
                start + 1,
                Severity::Error,
            );
            self.i = start.saturating_add(1);
            return None;
        };
        let width = text.len();
        self.i = start + width;
        Some(self.push_token(*kind, 0, start, start + width))
    }

    /// Numbers with recovery: a bad literal emits its valid prefix.
    fn lex_number(&mut self) -> Option<Token> {
        let start = self.i;
        match scan_number(self.bytes, start) {
            NumScan::Ok { end, lit } => {
                self.i = end;
                let kind = if matches!(&lit, NumLit::Float(_)) {
                    TokenKind::Float
                } else {
                    TokenKind::Int
                };
                let aux = self.push_num(lit);
                Some(self.push_token(kind, aux, start, end))
            }
            NumScan::Err {
                err_pos,
                reason,
                ok_end,
            } => {
                self.error_kind(
                    LexErrorKind::InvalidNumber { reason },
                    start,
                    err_pos.max(start),
                    start,
                    err_pos.max(start),
                    Severity::Error,
                );
                if ok_end > start {
                    // Valid prefix: re-scan just the prefix and emit it, then
                    // resume at the error, so `1.2.3` → FLT(1.2) FLT(0.3) + one
                    // error (T-N23).
                    let lit = match scan_number(self.bytes.get(start..ok_end).unwrap_or(&[]), 0) {
                        NumScan::Ok { lit, .. } => lit,
                        // Unreachable in practice: `ok_end` > start means the
                        // prefix already validated.
                        NumScan::Err { .. } => NumLit::Int(0),
                    };
                    let kind = if matches!(&lit, NumLit::Float(_)) {
                        TokenKind::Float
                    } else {
                        TokenKind::Int
                    };
                    let aux = self.push_num(lit);
                    self.i = err_pos;
                    Some(self.push_token(kind, aux, start, ok_end))
                } else {
                    // Nothing salvageable: skip one byte and retry.
                    self.i = start.saturating_add(1).min(self.src.len());
                    None
                }
            }
        }
    }

    /// Strings, with sub-lexed interpolation bodies (RULING S6).
    fn lex_string(&mut self) -> Option<Token> {
        let start = self.i;
        let file = self.file;
        let depth = self.depth;
        let lines = self.lines.clone();
        let sub: &dyn Fn(&'a str, FileId, usize, usize, u32) -> Lexed<'a> =
            &|src, file, lo, hi, depth| {
                let body = src.get(lo..hi).unwrap_or("");
                Lexer::new_at(body, file, lo, depth, lines.clone()).run()
            };
        let sc = scan_string(self.src, file, start, depth, sub);
        self.i = sc.next;
        if let Some(fail) = sc.fail {
            let end = self.resync_to_eol_from(self.i);
            let mut fail = fail;
            // The resync swallows the rest of the line, and those bytes would
            // then belong to neither a token nor an error, breaking the "loses
            // no bytes" invariant (T-P3). So extend `hi` just far enough to
            // cover them.
            //
            // Only `hi` moves. `scan_string` anchors every failure at the
            // opening quote -- `span` from the opener to the failure point,
            // `at` on the opener alone -- and RULING S4 depends on that
            // precision, so neither is recomputed here. Failures that already
            // reach EOF (`UnterminatedString`) leave this a no-op.
            let resynced = self.abs(end);
            if resynced > fail.span.hi {
                fail.span.hi = resynced;
            }
            self.record(fail);
            return None;
        }
        for w in &sc.warnings {
            self.record(w.clone());
        }
        let qlen = usize::try_from(sc.form.open_len()).unwrap_or(1);
        if sc.is_interpolated() {
            let parts = sc
                .parts
                .into_iter()
                .map(|p| match p {
                    ScanPart::Text(r) => InterpPart::Text(r),
                    ScanPart::Expr(idx) => InterpPart::Expr(idx),
                })
                .collect();
            let id = u32::try_from(self.interps.len()).unwrap_or(u32::MAX);
            self.interps.push(Interp {
                parts,
                subs: sc.subs,
            });
            Some(self.push_token(TokenKind::Interp, id, start, sc.end))
        } else if sc.verbatim {
            let content_lo = start.saturating_add(qlen);
            let content_hi = sc.end.checked_sub(qlen).unwrap_or(content_lo);
            let text = self.bytes.get(content_lo..content_hi).unwrap_or(&[]);
            let id = self.strs.intern(text);
            Some(self.push_token(TokenKind::Str, id, start, sc.end))
        } else {
            let owned = sc.cooked.unwrap_or_default();
            let id = self.strs.intern_cow(Cow::Owned(owned));
            Some(self.push_token(TokenKind::Str, id, start, sc.end))
        }
    }

    /// Recovers a string failure by consuming through the physical newline
    /// (§11, T-E5): one error per bad line.
    fn resync_to_eol_from(&mut self, from: usize) -> usize {
        let start = from.max(self.i);
        let mut i = start;
        while let Some(&b) = self.bytes.get(i) {
            i += 1;
            if b == b'\n' {
                break;
            }
        }
        self.i = i;
        i
    }

    /// Non-ASCII or `\`: report, advance by one code point.
    #[cold]
    fn lex_illegal(&mut self) {
        let i = self.i;
        let b = self.bytes.get(i).copied().unwrap_or(0);
        if b == b'\\' {
            // §8: a continuation outside a string is an error.
            self.error_kind(
                LexErrorKind::StrayLineContinuation,
                i,
                i + 1,
                i,
                i + 1,
                Severity::Error,
            );
            self.i = i.saturating_add(1);
            return;
        }
        let (ch, w) = decode_char(self.bytes, i);
        self.error_kind(
            LexErrorKind::UnexpectedCharacter { ch },
            i,
            i + w,
            i,
            i + w,
            Severity::Error,
        );
        self.i = i.saturating_add(w);
    }

    /// A physical newline: possibly suppressed (A1/A2/A4), else pended (A3).
    #[inline]
    fn on_newline(&mut self) {
        let lo = self.i;
        self.i += 1;
        let suppress_newline = self.delims.last().is_some_and(|d| d.suppress)
            || (self.has_last && self.last.continues_expression())
            || !self.has_last;
        if suppress_newline {
            return;
        }
        if self.sep_pending {
            // ASI-2: blank and comment-only lines produce nothing. Widen the
            // separator already emitted so these bytes stay in its span (T-P3);
            // a `Semi` absorbs the newline, a `Newline` absorbs the blank line.
            let hi = self.abs(lo.saturating_add(1));
            if let Some(prev) = self
                .toks
                .last_mut()
                .filter(|t| matches!(t.kind, TokenKind::Semi | TokenKind::Newline))
            {
                prev.hi = hi;
            }
            return;
        }
        self.pending_nl = Some((lo, lo.saturating_add(1)));
        self.sep_pending = true;
    }

    /// `;` subsumes a pending newline; a second `;` merges into it (§7.3).
    #[inline]
    fn on_semi(&mut self) {
        let lo = self.i;
        self.i += 1;
        debug_assert!(lo < self.src.len());
        self.after_dot = false;
        if self.sep_pending {
            if self.pending_nl.take().is_some() {
                self.toks.push(Token::new(
                    TokenKind::Semi,
                    0,
                    self.abs(lo),
                    self.abs(lo + 1),
                ));
                return;
            }
            // ASI-2: a `;` run collapses to one `Semi`. Widen the token we
            // already emitted so the merged bytes stay inside its span.
            let hi = self.abs(lo + 1);
            if let Some(prev) = self.toks.last_mut().filter(|t| t.kind == TokenKind::Semi) {
                prev.hi = hi;
            }
            return;
        }
        self.toks.push(Token::new(
            TokenKind::Semi,
            0,
            self.abs(lo),
            self.abs(lo + 1),
        ));
        self.sep_pending = true;
    }

    /// Record an error, capped by [`MAX_LEX_ERRORS`]; the cap itself appends
    /// one `TooManyErrors` and stops.
    #[cold]
    fn record(&mut self, e: LexError) {
        if self.errors_hit_cap {
            return;
        }
        if self.errors.len() >= MAX_LEX_ERRORS {
            self.errors_hit_cap = true;
            let whole = Span::new(self.file, self.abs(0), self.abs(self.src.len()));
            self.errors.push(LexError::error(
                LexErrorKind::TooManyErrors {
                    limit: MAX_LEX_ERRORS as u32,
                },
                whole,
                whole,
            ));
            return;
        }
        self.errors.push(e);
    }

    #[cold]
    fn error_kind(
        &mut self,
        kind: LexErrorKind,
        lo: usize,
        hi: usize,
        at_lo: usize,
        at_hi: usize,
        severity: Severity,
    ) {
        let e = LexError::new(kind, severity, self.span(lo, hi), self.span(at_lo, at_hi));
        self.record(e);
    }

    #[inline]
    fn push_num(&mut self, lit: NumLit) -> u32 {
        let id = u32::try_from(self.nums.len()).unwrap_or(u32::MAX);
        self.nums.push(lit);
        id
    }

    /// File-absolute offset of a relative one.
    #[inline]
    fn abs(&self, off: usize) -> u32 {
        u32::try_from(off.saturating_add(self.base)).unwrap_or(u32::MAX)
    }

    #[inline]
    fn span(&self, lo: usize, hi: usize) -> Span {
        Span::new(self.file, self.abs(lo), self.abs(hi))
    }

    /// Flush the pending newline, emit the single `Eof`, and assemble.
    #[cold]
    pub fn finish(&mut self) -> Lexed<'a> {
        if let Some((lo, hi)) = self.pending_nl.take() {
            self.push_newline(lo, hi);
            self.sep_pending = false;
        }
        let len = self.src.len();
        let at = self.abs(len);
        self.toks.push(Token::new(TokenKind::Eof, 0, at, at));
        Lexed {
            file: self.file,
            toks: std::mem::take(&mut self.toks),
            idents: std::mem::take(&mut self.idents),
            strs: std::mem::take(&mut self.strs),
            interps: std::mem::take(&mut self.interps),
            nums: std::mem::take(&mut self.nums),
            lines: self.lines.clone(),
            errors: std::mem::take(&mut self.errors),
        }
    }
}

/// Lex a whole file into a [`Lexed`].
pub fn lex<'a>(src: &'a str, file: FileId) -> Lexed<'a> {
    let mut lx = Lexer::new(src, file);
    lx.run()
}

#[inline]
fn is_bom_at(bytes: &[u8], i: usize) -> bool {
    bytes.get(i).copied() == Some(0xEF)
        && bytes.get(i + 1).copied() == Some(0xBB)
        && bytes.get(i + 2).copied() == Some(0xBF)
}

/// Decode the code point at `bytes[i]`; width is its UTF-8 length, or 1 if it
/// is not a valid UTF-8 lead byte (the source is still valid UTF-8, so at a
/// non-ASCII illegality this is the multibyte sequence; `\u{FFFD}` never
/// fires but keeps the function total).
fn decode_char(bytes: &[u8], i: usize) -> (char, usize) {
    match bytes.get(i) {
        None => ('\u{FFFD}', 1),
        Some(&b) if b < 0x80 => (b as char, 1),
        Some(_) => match std::str::from_utf8(bytes.get(i..).unwrap_or(&[])) {
            Ok(s) => match s.chars().next() {
                Some(c) => (c, c.len_utf8()),
                None => ('\u{FFFD}', 1),
            },
            Err(_) => {
                // The source is valid UTF-8, so this lead byte still begins a
                // sequence of some valid length; report it whole.
                let b = bytes.get(i).copied().unwrap_or(0);
                let width = utf8_len(b);
                (char::from_u32(u32::from(b)).unwrap_or('\u{FFFD}'), width)
            }
        },
    }
}

/// Width in bytes of the UTF-8 sequence starting at `lead`.
fn utf8_len(lead: u8) -> usize {
    match lead {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::FileId;

    const F: FileId = FileId(0);

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src, F).toks.into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn hw_parens_number_ident() {
        let lx = lex("fn main() { 42 }", F);
        assert_eq!(
            kinds("fn main() { 42 }"),
            vec![
                TokenKind::KwFn,
                TokenKind::Ident,
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::LBrace,
                TokenKind::Int,
                TokenKind::RBrace,
                TokenKind::Eof,
            ]
        );
        assert_eq!(lx.toks.len(), 8);
    }

    /// `case 1:` newline `"one"`: Colon continues (A2), the `{`-block's first
    /// newline is suppressed by the LBrace token's own A2 bit, and the match
    /// arm closes on the newline before `}` (flushed just before `RBrace`).
    #[test]
    fn newline_semantics_in_use() {
        assert_eq!(
            kinds("switch x {\n case 1:\n   \"one\"\n}"),
            vec![
                TokenKind::KwSwitch,
                TokenKind::Ident,
                TokenKind::LBrace,
                TokenKind::KwCase,
                TokenKind::Int,
                TokenKind::Colon,
                TokenKind::Str,
                TokenKind::Newline,
                TokenKind::RBrace,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn dodad_suppresses_newline_inside_suppressor() {
        // A1: `(` suppresses, so the newline disappears but the bracketed
        // group is one statement.
        let t = kinds("f(a,\nb)\n");
        assert_eq!(
            t,
            vec![
                TokenKind::Ident,
                TokenKind::LParen,
                TokenKind::Ident,
                TokenKind::Comma,
                TokenKind::Ident,
                TokenKind::RParen,
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn safe_dot_continues() {
        assert_eq!(
            kinds("x = 5\n.real\n"),
            vec![
                TokenKind::Ident,
                TokenKind::Assign,
                TokenKind::Int,
                TokenKind::Newline,
                TokenKind::Dot,
                TokenKind::Ident,
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn semi_subsumes_newline() {
        assert_eq!(
            kinds("a = 1\n;\nb = 2\n"),
            vec![
                TokenKind::Ident,
                TokenKind::Assign,
                TokenKind::Int,
                TokenKind::Semi,
                TokenKind::Ident,
                TokenKind::Assign,
                TokenKind::Int,
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn interp_basic() {
        let lx = lex(r#""x = ${a + 1}""#, F);
        assert_eq!(lx.toks.len(), 2);
        assert_eq!(lx.toks[0].kind, TokenKind::Interp);
        assert_eq!(lx.toks[1].kind, TokenKind::Eof);
        let i = &lx.interps[0];
        assert_eq!(i.parts.len(), 2);
        let sub = &i.subs[0];
        assert_eq!(sub.toks[0].kind, TokenKind::Ident);
        assert_eq!(sub.toks[1].kind, TokenKind::Plus);
        assert_eq!(sub.toks[2].kind, TokenKind::Int);
    }

    #[test]
    fn recovered_errors_are_recorded() {
        let lx = lex("1.2.3\n", F);
        assert_eq!(lx.toks[0].kind, TokenKind::Float);
        assert_eq!(lx.toks[1].kind, TokenKind::Float);
        assert_eq!(lx.errors.len(), 1);
    }

    #[test]
    fn eof_span_empty_file() {
        let lx = lex("", F);
        assert_eq!(lx.toks.len(), 1);
        assert_eq!(lx.toks[0].kind, TokenKind::Eof);
        assert_eq!(lx.toks[0].lo, 0);
    }

    #[test]
    fn messy_comments_are_free() {
        assert_eq!(
            kinds("# comment\nfn main() # trailing\n{ 1 }\n"),
            vec![
                TokenKind::KwFn,
                TokenKind::Ident,
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::Newline,
                TokenKind::LBrace,
                TokenKind::Int,
                TokenKind::RBrace,
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }
}
