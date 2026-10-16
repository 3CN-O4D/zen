//! The five string forms, escape decoding, and `${…}` interpolation.
//!
//! See `docs/05-lexer.md` §6.
//!
//! **RULING S6** — escapes are *not* processed inside `${…}`. The body between
//! `${` and its matching `}` is a raw byte slice handed to a sub-lexer, because
//! the body is real Zen source: if escapes were cooked there but not outside
//! it, `"a\n${b\nc}d"` would feed the parser an expression containing a literal
//! newline that does not exist in the source the user can see, and every span in
//! the sub-tree would point at bytes the user never wrote. One rule — the body is
//! source, the rest is text — cannot disagree with itself.

use std::borrow::Cow;

use super::error::{LexError, LexErrorKind};
use super::token::{Lexed, Range, StrForm};
use crate::span::FileId;

/// Interpolation nesting beyond this is a `NestingTooDeep` error (T-S29).
pub const MAX_INTERP_NESTING: u32 = 8;

/// One piece of a scanned interpolated string.
#[derive(Clone, Debug, PartialEq)]
pub enum ScanPart<'a> {
    Text(Range<'a>),
    /// Index into [`StringScan::subs`].
    Expr(u32),
}

/// The outcome of scanning one string literal.
pub struct StringScan<'a> {
    /// Where scanning should resume.
    pub next: usize,
    /// Cooked bytes, when the literal has no interpolation.
    pub cooked: Option<Vec<u8>>,
    /// True when no escape was cooked anywhere, so `cooked` is byte-identical
    /// to the source slice and the interner can borrow instead of own.
    pub verbatim: bool,
    pub parts: Vec<ScanPart<'a>>,
    pub subs: Vec<Lexed<'a>>,
    pub form: StrForm,
    /// Offset of the opening quote, for the caret.
    pub open: usize,
    /// One past the closing quote, or the failure point.
    pub end: usize,
    /// Warnings that do not stop the literal producing a token.
    pub warnings: Vec<LexError>,
    /// A hard error: the literal produced **no** token.
    pub fail: Option<LexError>,
}

impl<'a> StringScan<'a> {
    /// True when there is at least one `Expr` part.
    pub fn is_interpolated(&self) -> bool {
        self.parts.iter().any(|p| matches!(p, ScanPart::Expr(_)))
    }
}

#[inline]
fn hex_val(b: u8) -> Option<u32> {
    Some(match b {
        b'0'..=b'9' => u32::from(b - b'0'),
        b'a'..=b'f' => u32::from(b - b'a') + 10,
        b'A'..=b'F' => u32::from(b - b'A') + 10,
        _ => return None,
    })
}

/// A byte that ends the string rather than being part of an escape. Used to tell
/// `\x1"` (*incomplete*) from `\xZZ` (*invalid*).
#[inline]
fn is_string_bound(b: u8) -> bool {
    matches!(b, b'"' | b'\'' | b'`' | b'\n' | b'\r')
}

enum Escape {
    Done {
        next: usize,
        bytes: Vec<u8>,
    },
    /// RULING S5: an unknown escape warns and means the literal character.
    Warn {
        next: usize,
        bytes: Vec<u8>,
        byte: u8,
    },
    Fail {
        next: usize,
        kind: LexErrorKind,
        lo: usize,
        hi: usize,
    },
}

/// Decode one escape, with `i` at the backslash.
fn scan_escape(src: &[u8], i: usize) -> Escape {
    let Some(&e) = src.get(i + 1) else {
        return Escape::Fail {
            next: i + 1,
            kind: LexErrorKind::UnfinishedEscape,
            lo: i,
            hi: i.saturating_add(1),
        };
    };
    let simple = |b: u8| -> Vec<u8> { vec![b] };
    match e {
        b'n' => Escape::Done {
            next: i + 2,
            bytes: vec![0x0A],
        },
        b't' => Escape::Done {
            next: i + 2,
            bytes: vec![0x09],
        },
        b'r' => Escape::Done {
            next: i + 2,
            bytes: vec![0x0D],
        },
        b'0' => Escape::Done {
            next: i + 2,
            bytes: vec![0x00],
        },
        b'a' => Escape::Done {
            next: i + 2,
            bytes: vec![0x07],
        },
        b'b' => Escape::Done {
            next: i + 2,
            bytes: vec![0x08],
        },
        b'e' => Escape::Done {
            next: i + 2,
            bytes: vec![0x1B],
        },
        b'f' => Escape::Done {
            next: i + 2,
            bytes: vec![0x0C],
        },
        b'v' => Escape::Done {
            next: i + 2,
            bytes: vec![0x0B],
        },
        b'\\' => Escape::Done {
            next: i + 2,
            bytes: simple(b'\\'),
        },
        b'\'' => Escape::Done {
            next: i + 2,
            bytes: simple(b'\''),
        },
        b'"' => Escape::Done {
            next: i + 2,
            bytes: simple(b'"'),
        },
        b'`' => Escape::Done {
            next: i + 2,
            bytes: simple(b'`'),
        },
        b'$' => Escape::Done {
            next: i + 2,
            bytes: simple(b'$'),
        },
        b'x' => scan_hex_escape(src, i),
        b'u' | b'U' => scan_unicode_escape(src, i, e),
        other => Escape::Warn {
            next: i + 2,
            bytes: simple(other),
            byte: other,
        },
    }
}

/// `\xNN`: exactly two hex digits, mapped Latin-1 (`\xFF` is U+00FF, two UTF-8
/// bytes). That is what the legacy scanner does and what `binascii` depends on.
fn scan_hex_escape(src: &[u8], i: usize) -> Escape {
    let a = src.get(i + 2).copied();
    let b = src.get(i + 3).copied();
    let incomplete = |next: usize| Escape::Fail {
        next,
        kind: LexErrorKind::IncompleteHexEscape,
        lo: i,
        hi: i.saturating_add(4).min(src.len()),
    };
    let invalid = |next: usize| Escape::Fail {
        next,
        kind: LexErrorKind::InvalidHexEscape,
        lo: i,
        hi: i.saturating_add(4).min(src.len()),
    };
    match (a, b) {
        (None, _) | (_, None) => incomplete(i.saturating_add(4).min(src.len())),
        (Some(x), Some(y)) => match (hex_val(x), hex_val(y)) {
            (Some(hi), Some(lo)) => {
                let code = (hi * 16 + lo) as u8;
                // Latin-1 mapping, deliberately: `char::from(u8)` gives
                // U+0000..=U+00FF, which encodes as one or two UTF-8 bytes.
                let ch = char::from(code);
                Escape::Done {
                    next: i + 4,
                    bytes: ch.to_string().into_bytes(),
                }
            }
            _ => {
                if is_string_bound(y) || is_string_bound(x) {
                    incomplete(i.saturating_add(3))
                } else {
                    invalid(i.saturating_add(3))
                }
            }
        },
    }
}

/// `\u{H…}` (1–6 digits, legacy) and `\U{H…}` (1–8, new, with the two checks the
/// legacy form lacks: `<= 0x10FFFF` and not a surrogate).
fn scan_unicode_escape(src: &[u8], i: usize, tag: u8) -> Escape {
    let max_digits = u32::from(tag == b'u') * 6 + u32::from(tag == b'U') * 8;
    let fail = |kind: LexErrorKind, next: usize| Escape::Fail {
        next,
        kind,
        lo: i,
        hi: next.max(i).min(src.len()),
    };
    if src.get(i + 2) != Some(&b'{') {
        return fail(
            LexErrorKind::InvalidUnicodeEscape { tag },
            i.saturating_add(3),
        );
    }
    let mut j = i + 3;
    let mut digits: u32 = 0;
    let mut value: u32 = 0;
    while let Some(&c) = src.get(j) {
        if c == b'}' {
            break;
        }
        match hex_val(c) {
            Some(d) => {
                digits += 1;
                if digits > max_digits {
                    return fail(
                        LexErrorKind::UnicodeEscapeTooLong { tag },
                        j.saturating_add(1),
                    );
                }
                value = value.saturating_mul(16).saturating_add(d);
                j += 1;
            }
            None => {
                let kind = if digits == 0 {
                    LexErrorKind::InvalidUnicodeEscape { tag }
                } else {
                    LexErrorKind::UnterminatedUnicodeEscape { tag }
                };
                return fail(kind, j);
            }
        }
    }
    match src.get(j) {
        None => fail(LexErrorKind::UnterminatedUnicodeEscape { tag }, src.len()),
        Some(b'}') => {
            if digits == 0 {
                fail(
                    LexErrorKind::EmptyUnicodeEscape { tag },
                    j.saturating_add(1),
                )
            } else if value > 0x10_FFFF || (0xD800..=0xDFFF).contains(&value) {
                fail(
                    LexErrorKind::InvalidCodePoint { cp: value },
                    j.saturating_add(1),
                )
            } else {
                let ch = char::from_u32(value).unwrap_or(char::REPLACEMENT_CHARACTER);
                Escape::Done {
                    next: j + 1,
                    bytes: ch.to_string().into_bytes(),
                }
            }
        }
        Some(_) => fail(LexErrorKind::UnterminatedUnicodeEscape { tag }, j),
    }
}

/// RULING S7: the `${…}` body is string-aware. The legacy scanner counted
/// braces blindly, so `"${d["}"]}"` lexed wrong; this one skips nested string
/// literals as units.
fn scan_interp_body(src: &[u8], dollar: usize) -> Option<(usize, usize, usize)> {
    let mut depth: i32 = 1;
    let mut k = dollar.saturating_add(2);
    let start = k;
    while k < src.len() {
        let b = src[k];
        match b {
            b'"' | b'\'' | b'`' => k = skip_nested_string(src, k, 0),
            b'{' => {
                depth += 1;
                k += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, k, k + 1));
                }
                k += 1;
            }
            _ => k += 1,
        }
    }
    None
}

/// Skip a whole nested string literal starting at `k`.
///
/// Three-quote aware, escape aware, and interpolation aware. Recursion is bounded
/// by `depth <= MAX_INTERP_NESTING` — each nested `${…}` raises it, and past the
/// cap the scan reports the end of input. That is what keeps this from being the
/// unbounded recursion §2.5 forbids.
fn skip_nested_string(src: &[u8], k: usize, depth: u32) -> usize {
    if depth > MAX_INTERP_NESTING {
        return src.len();
    }
    let Some(&q) = src.get(k) else {
        return src.len();
    };
    let triple = src.get(k + 1) == Some(&q) && src.get(k + 2) == Some(&q);
    let qlen = if triple { 3 } else { 1 };
    let mut m = k + qlen;
    loop {
        let Some(&b) = src.get(m) else {
            return src.len();
        };
        if b == b'\\' {
            m = m.saturating_add(2).min(src.len());
            continue;
        }
        if b == b'$' && src.get(m + 1) == Some(&b'{') {
            m = skip_interpolation_raw(src, m, depth.saturating_add(1));
            continue;
        }
        if b == q {
            if triple {
                if src.get(m + 1) == Some(&q) && src.get(m + 2) == Some(&q) {
                    return m + 3;
                }
                m += 1;
                continue;
            }
            return m + 1;
        }
        if !triple && b == b'\n' {
            // A single-line nested string cannot span a line; stop so the
            // caller's own error reporting takes over.
            return m;
        }
        m += 1;
    }
}

/// Skip a `${…}` body as an opaque unit, without reporting errors.
fn skip_interpolation_raw(src: &[u8], dollar: usize, depth: u32) -> usize {
    if depth > MAX_INTERP_NESTING {
        return src.len();
    }
    let mut d: i32 = 1;
    let mut m = dollar.saturating_add(2);
    while m < src.len() {
        match src[m] {
            b'"' | b'\'' | b'`' => m = skip_nested_string(src, m, depth.saturating_add(1)),
            b'{' => {
                d += 1;
                m += 1;
            }
            b'}' => {
                d -= 1;
                if d == 0 {
                    return m + 1;
                }
                m += 1;
            }
            _ => m += 1,
        }
    }
    src.len()
}

#[allow(unused_assignments)] // `flush!` writes `run_start` after the last read
/// Scan one string literal starting at the opening quote.
pub fn scan_string<'a>(
    src: &'a str,
    file: FileId,
    start: usize,
    depth: u32,
    lex_slice: &dyn Fn(&'a str, FileId, usize, usize, u32) -> Lexed<'a>,
) -> StringScan<'a> {
    let bytes = src.as_bytes();
    let q = bytes.get(start).copied().unwrap_or(b'"');
    // RULING S2: triple detection is exact — three adjacent identical bytes.
    // RULING S1: the backtick is excluded, so there is no triple backtick.
    let adjacent = |n: usize| -> bool { (1..=n).all(|k| bytes.get(start + k) == Some(&q)) };
    let form = match (q, adjacent(2)) {
        (b'"', true) => StrForm::TripleDouble,
        (b'"', false) => StrForm::Double,
        (b'\'', true) => StrForm::TripleSingle,
        (b'\'', false) => StrForm::Single,
        _ => StrForm::Backtick,
    };
    let interp = form.interpolates();
    let qlen = usize::try_from(form.open_len()).unwrap_or(1);

    let mut j = start + qlen;
    let mut text: Vec<u8> = Vec::new();
    let mut run_start = j;
    // While true, `text` mirrors `src[run_start..j]` byte for byte, so the Text
    // part can borrow instead of own.
    let mut verbatim = true;
    // Whole-literal "no escape anywhere" flag; unlike `verbatim`, the `flush!`
    // macro never resets it, so it survives every run boundary.
    let mut any_cooked = false;
    let mut parts: Vec<ScanPart<'a>> = Vec::new();
    let mut subs: Vec<Lexed<'a>> = Vec::new();
    let mut warnings: Vec<LexError> = Vec::new();

    let span = |lo: usize, hi: usize| {
        crate::span::Span::new(
            file,
            u32::try_from(lo).unwrap_or(u32::MAX),
            u32::try_from(hi).unwrap_or(u32::MAX),
        )
    };

    macro_rules! flush {
        () => {{
            if j > run_start || !text.is_empty() {
                parts.push(ScanPart::Text(make_range(
                    src, run_start, j, &mut text, verbatim,
                )));
                run_start = j;
                text.clear();
                verbatim = true;
            }
        }};
    }

    macro_rules! bail {
        ($kind:expr) => {{
            let kind = $kind;
            let at = span(start, start + qlen);
            let full = span(start, j.max(start));
            return StringScan {
                next: j,
                cooked: None,
                verbatim: !any_cooked,
                parts,
                subs,
                form,
                open: start,
                end: j,
                warnings,
                fail: Some(LexError::error(kind, full, at)),
            };
        }};
    }

    loop {
        let Some(&b) = bytes.get(j) else {
            // End of input. RULING S4: the span runs to EOF but the caret goes
            // on the opening quote.
            if form.is_triple() {
                bail!(LexErrorKind::UnterminatedTripleString { form });
            }
            let line = count_lines(bytes, start);
            bail!(LexErrorKind::UnterminatedString { form, line });
        };

        // ── escapes ──────────────────────────────────────────────────────
        if b == b'\\' {
            match bytes.get(j + 1) {
                // RULING LC-1: backslash-newline is a continuation, removed
                // *before* escape processing, so it can never be confused
                // with `\n`.
                Some(b'\n') => {
                    j += 2;
                    continue;
                }
                Some(b'\r') if bytes.get(j + 2) == Some(&b'\n') => {
                    j += 3;
                    continue;
                }
                // A trailing `\` is almost always a truncated file, and the fix
                // is different from a stray one.
                None => bail!(LexErrorKind::TrailingLineContinuation),
                _ => {}
            }
            match scan_escape(bytes, j) {
                Escape::Done { next, bytes: out } => {
                    text.extend_from_slice(&out);
                    verbatim = false;
                    any_cooked = true;
                    j = next;
                }
                Escape::Warn {
                    next,
                    bytes: out,
                    byte,
                } => {
                    warnings.push(LexError::warning(
                        LexErrorKind::UnknownEscape { byte },
                        span(j, next),
                        span(j, next),
                    ));
                    text.extend_from_slice(&out);
                    verbatim = false;
                    any_cooked = true;
                    j = next;
                }
                Escape::Fail { next, kind, lo, hi } => {
                    j = next.max(j + 1);
                    let at = span(start, start + qlen);
                    let full = span(lo.min(start), hi);
                    return StringScan {
                        next: j,
                        cooked: None,
                        verbatim: !any_cooked,
                        parts,
                        subs,
                        form,
                        open: start,
                        end: j,
                        warnings,
                        fail: Some(LexError::error(kind, full, at)),
                    };
                }
            }
            continue;
        }

        // ── RULING S3: no raw newline in a single-line string ────────────
        if b == b'\n' && !form.multiline() {
            let line = count_lines(bytes, start);
            j += 1;
            bail!(LexErrorKind::RawNewlineInString { line });
        }

        // ── terminator ───────────────────────────────────────────────────
        if b == q {
            if form.is_triple() {
                if bytes.get(j + 1) == Some(&q) && bytes.get(j + 2) == Some(&q) {
                    flush!();
                    j += 3;
                    break;
                }
                text.push(b);
                j += 1;
                continue;
            }
            flush!();
            j += 1;
            break;
        }

        // ── interpolation ────────────────────────────────────────────────
        if interp && b == b'$' && bytes.get(j + 1) == Some(&b'{') {
            let dollar = j;
            let Some((body_lo, body_hi, next)) = scan_interp_body(bytes, j) else {
                // RULING S6's failure shape: the caret goes on `${`, not the
                // opening quote, so the user can see where the expression was
                // supposed to open.
                let (expr_line, expr_col) = line_col_in(bytes, dollar);
                let kind = LexErrorKind::UnterminatedInterpolation {
                    expr_line,
                    expr_col,
                };
                let at = span(dollar, dollar.saturating_add(2));
                let full = span(start, bytes.len());
                j = bytes.len();
                return StringScan {
                    next: j,
                    cooked: None,
                    verbatim: !any_cooked,
                    parts,
                    subs,
                    form,
                    open: start,
                    end: j,
                    warnings,
                    fail: Some(LexError::error(kind, full, at)),
                };
            };
            flush!();
            if depth >= MAX_INTERP_NESTING {
                warnings.push(LexError::error(
                    LexErrorKind::NestingTooDeep { depth },
                    span(j, next),
                    span(j, next),
                ));
            }
            let sub = lex_slice(src, file, body_lo, body_hi, depth.saturating_add(1));
            let idx = u32::try_from(subs.len()).unwrap_or(u32::MAX);
            subs.push(sub);
            parts.push(ScanPart::Expr(idx));
            j = next;
            run_start = j;
            continue;
        }

        text.push(b);
        j += 1;
    }

    let interpolated = parts.iter().any(|p| matches!(p, ScanPart::Expr(_)));
    let cooked = if interpolated {
        None
    } else {
        // Reassemble the (possibly empty) text run.
        let mut out = Vec::new();
        for part in &parts {
            if let ScanPart::Text(range) = part {
                out.extend_from_slice(range.as_str().as_bytes());
            }
        }
        Some(out)
    };
    if !interpolated {
        parts.clear();
    }

    StringScan {
        next: j,
        cooked,
        verbatim: !any_cooked,
        parts,
        subs,
        form,
        open: start,
        end: j,
        warnings,
        fail: None,
    }
}

fn make_range<'a>(
    src: &'a str,
    lo: usize,
    hi: usize,
    text: &mut Vec<u8>,
    verbatim: bool,
) -> Range<'a> {
    let lo32 = u32::try_from(lo).unwrap_or(u32::MAX);
    let hi32 = u32::try_from(hi).unwrap_or(lo32);
    if verbatim {
        let slice = src.get(lo..hi).unwrap_or("");
        text.clear();
        Range {
            lo: lo32,
            hi: hi32,
            text: Cow::Borrowed(slice),
        }
    } else {
        let owned = String::from_utf8(std::mem::take(text)).unwrap_or_default();
        Range {
            lo: lo32,
            hi: hi32,
            text: Cow::Owned(owned),
        }
    }
}

fn count_lines(bytes: &[u8], from: usize) -> u32 {
    let mut n: u32 = 1;
    for &b in bytes.get(..from).unwrap_or(&[]) {
        if b == b'\n' {
            n = n.saturating_add(1);
        }
    }
    n
}

/// 1-based (line, col) of the byte at `at`, for the interpolation error's
/// context fields.
fn line_col_in(bytes: &[u8], at: usize) -> (u32, u32) {
    let line = count_lines(bytes, at);
    let line_start = bytes
        .get(..at)
        .unwrap_or(&[])
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|p| p.saturating_add(1))
        .unwrap_or(0);
    let col = at.saturating_sub(line_start) as u32;
    (line, col.max(1))
}
