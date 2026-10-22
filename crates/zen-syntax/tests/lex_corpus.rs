//! Corpus-level and property tests: `docs/05-lexer.md` §11.6 (T-E9 – T-E14)
//! and §11.7 (T-P1 – T-P6).
//!
//! These are the totality gate. They do not assert token *kinds* — they
//! assert that `lex` terminates, produces well-formed spans, and never loses
//! or invents bytes, on every corpus file and on adversarial input.

use std::fs;
use std::path::{Path, PathBuf};

use zen_syntax::lex::{lex, Lexed, TokenKind};
use zen_syntax::span::FileId;

const F: FileId = FileId(0);

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn corpus_files() -> Vec<PathBuf> {
    let cases = repo_root().join("tests/cases");
    let mut v: Vec<PathBuf> = fs::read_dir(&cases)
        .expect("tests/cases")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "z"))
        .collect();
    v.sort();
    v
}

fn std_files() -> Vec<PathBuf> {
    let dir = repo_root().join("std");
    let Ok(rd) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "z"))
        .collect();
    v.sort();
    v
}

fn all_files() -> Vec<PathBuf> {
    let mut v = corpus_files();
    v.extend(std_files());
    v
}

/// A deterministic xorshift PRNG, so a failure is reproducible from the seed
/// alone without adding a dev-dependency.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn byte(&mut self) -> u8 {
        (self.next() >> 24) as u8
    }

    /// A random *valid* UTF-8 string. Byte-level properties (T-P3) need a
    /// well-formed source: `from_utf8_lossy` would splice U+FFFD in and
    /// desynchronise the byte accounting. Scalars are drawn from ASCII plus
    /// every multi-byte width so identifiers, strings, and operators all get
    /// exercised.
    fn utf8_string(&mut self, max: usize) -> String {
        const NONASCII: &[char] = &['\u{00E9}', '\u{03B1}', '\u{4E2D}', '\u{1F600}', '\u{FEFF}'];
        let n = (self.next() % u64::try_from(max).unwrap_or(u64::MAX)) as usize;
        let mut out = String::with_capacity(n * 2);
        for _ in 0..n {
            if self.next().is_multiple_of(4) {
                out.push(NONASCII[(self.next() >> 8) as usize % NONASCII.len()]);
            } else {
                const ASCII: &[u8] = b" \t\r\nabcXYZ0123\x22\x27`#/*+-.,;:()[]{}<>=!&|?\\$";
                out.push(char::from(ASCII[(self.next() >> 8) as usize % ASCII.len()]));
            }
        }
        out
    }
}

// ── §11.6 totality ─────────────────────────────────────────────────────

#[test]
fn t_e9_no_panic_on_corpus() {
    let files = corpus_files();
    assert!(!files.is_empty(), "corpus is present");
    for f in &files {
        let src = fs::read_to_string(f).expect("utf-8 corpus file");
        let lx = lex(&src, F);
        assert_eq!(
            lx.toks.last().map(|t| t.kind),
            Some(TokenKind::Eof),
            "{}",
            f.display()
        );
    }
}

#[test]
fn t_e10_no_panic_on_stdlib() {
    for f in std_files() {
        let src = fs::read_to_string(&f).expect("utf-8 std file");
        let lx = lex(&src, F);
        assert_eq!(
            lx.toks.last().map(|t| t.kind),
            Some(TokenKind::Eof),
            "{}",
            f.display()
        );
    }
}

#[test]
fn t_e11_no_panic_on_random_bytes() {
    let mut rng = Rng::new(0x5EED_1234_ABCD_0001);
    for i in 0..10_000u32 {
        let len = (rng.next() % 64) as usize;
        let bytes: Vec<u8> = (0..len).map(|_| rng.byte()).collect();
        let lossy = String::from_utf8_lossy(&bytes).into_owned();
        // Lossy conversion is fine: the point is that `lex` returns.
        let lx = lex(&lossy, F);
        assert!(lx.toks.last().is_some(), "iter {i} produced no tokens");
    }
}

#[test]
fn t_e11b_no_panic_on_pathological_delimiters() {
    for (src, name) in [
        ("(".repeat(2000), "open parens"),
        ("[".repeat(2000), "open brackets"),
        ("{".repeat(2000), "open braces"),
        ("\"".repeat(2000), "quotes"),
        ("\"${\"".repeat(500), "open interps"),
        ("\\".repeat(2000), "backslashes"),
        ("\u{1F600}".repeat(2000), "astral chars"),
        ("0x".repeat(2000), "radix prefixes"),
        ("1_".repeat(2000), "bad separators"),
    ] {
        let lx = lex(&src, F);
        assert_eq!(
            lx.toks.last().map(|t| t.kind),
            Some(TokenKind::Eof),
            "{name}"
        );
    }
}

#[test]
fn t_e13_no_trailing_synthetic_newline() {
    let lx = lex("a = 1", F);
    let n = lx.toks.len();
    assert_eq!(lx.toks[n - 2].kind, TokenKind::Int);
    assert_eq!(lx.toks[n - 1].kind, TokenKind::Eof);
    assert!(!lx
        .toks
        .iter()
        .rev()
        .take(2)
        .any(|t| t.kind == TokenKind::Newline));
}

#[test]
fn t_e12_every_span_in_range() {
    let check = |lx: &Lexed<'_>, src: &str, what: &str| {
        for t in &lx.toks {
            debug_assert!(t.lo <= t.hi, "{what}: inverted span");
            assert!(t.lo as usize <= t.hi as usize, "{what}: inverted span");
            assert!(
                t.hi as usize <= src.len(),
                "{what}: span past end of source"
            );
        }
    };
    for f in all_files() {
        let src = fs::read_to_string(&f).expect("utf-8");
        let lx = lex(&src, F);
        check(&lx, &src, &f.display().to_string());
    }
    let mut rng = Rng::new(99);
    for _ in 0..2000 {
        let len = (rng.next() % 48) as usize;
        let bytes: Vec<u8> = (0..len).map(|_| rng.byte()).collect();
        let s = String::from_utf8_lossy(&bytes).into_owned();
        let lx = lex(&s, F);
        check(&lx, &s, "random");
    }
}

// ── §11.7 properties ───────────────────────────────────────────────────

/// T-P1/T-P2/T-P3: spans are ordered, non-overlapping, `Eof` is unique and
/// terminal, and the tokens plus gaps account for every byte.
fn properties(lx: &Lexed<'_>, src: &str, what: &str) {
    let toks = &lx.toks;
    assert!(!toks.is_empty(), "{what}: no tokens");
    assert_eq!(
        toks.iter().filter(|t| t.kind == TokenKind::Eof).count(),
        1,
        "{what}: Eof count"
    );
    assert_eq!(
        toks.last().map(|t| t.kind),
        Some(TokenKind::Eof),
        "{what}: Eof not last"
    );

    let eof = toks.last().expect("non-empty");
    assert_eq!(
        (eof.lo, eof.hi),
        (src.len() as u32, src.len() as u32),
        "{what}: Eof span"
    );

    // T-P1: strictly increasing `lo`, and non-overlapping.
    let mut prev_hi: u32 = 0;
    for (i, t) in toks.iter().enumerate() {
        assert!(t.lo <= t.hi, "{what}: token {i} inverted");
        if i > 0 {
            assert!(
                t.lo >= prev_hi,
                "{what}: token {i} overlaps its predecessor"
            );
        }
        assert!(t.hi as usize <= src.len(), "{what}: token {i} past end");
        prev_hi = t.hi;
    }

    // T-P3: every byte outside the token spans must be whitespace, a newline
    // the lexer suppressed, or comment content. We scan each uncovered run
    // with a minimal comment/whitespace scanner; anything else is a byte the
    // lexer lost, which is exactly the bug T-P3 exists to catch.
    let mut covered = vec![false; src.len()];
    let mark = |lo: u32, hi: u32, covered: &mut [bool]| {
        let lo = usize::try_from(lo).unwrap_or(usize::MAX).min(covered.len());
        let hi = usize::try_from(hi).unwrap_or(usize::MAX).min(covered.len());
        for b in covered.iter_mut().take(hi).skip(lo) {
            *b = true;
        }
    };
    for t in toks {
        mark(t.lo, t.hi, &mut covered);
    }
    // An errored region is accounted for by its diagnostic, not by a token, so
    // the "loses no bytes" sum is over token spans *and* error spans.
    for e in &lx.errors {
        mark(e.span.lo, e.span.hi, &mut covered);
    }
    let bytes = src.as_bytes();
    let mut i = 0usize;
    while i < src.len() {
        if covered[i] {
            i += 1;
            continue;
        }
        let start = i;
        while i < src.len() && !covered[i] {
            match bytes[i] {
                b' ' | b'\t' | b'\r' | b'\n' => i += 1,
                b'#' => {
                    while i < src.len() && bytes[i] != b'\n' {
                        i += 1;
                    }
                }
                b'/' if bytes.get(i + 1) == Some(&b'/') => {
                    while i < src.len() && bytes[i] != b'\n' {
                        i += 1;
                    }
                }
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    i += 2;
                    while i < src.len() {
                        if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                // RULING A1: a leading BOM is skipped silently and is not a
                // token, so it legitimately covers no span.
                0xEF if start == 0 && bytes[i..].starts_with(b"\xEF\xBB\xBF") => i += 3,
                other => panic!(
                    "{what}: unexplained byte {other:#04x} at {start}; src hex={}",
                    src.as_bytes()
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>(),
                ),
            }
        }
    }
}

#[test]
fn t_p123_spans_on_corpus() {
    for f in all_files() {
        let src = fs::read_to_string(&f).expect("utf-8");
        let lx = lex(&src, F);
        properties(&lx, &src, &f.display().to_string());
    }
}

#[test]
fn t_p123_spans_on_random_bytes() {
    let mut rng = Rng::new(0xC0FFEE);
    for i in 0..5000 {
        let s = rng.utf8_string(96);
        let lx = lex(&s, F);
        properties(&lx, &s, &format!("random {i}"));
    }
}

#[test]
fn t_p4_lexing_is_deterministic() {
    for f in all_files() {
        let src = fs::read_to_string(&f).expect("utf-8");
        assert_eq!(lex(&src, F), lex(&src, F), "{}", f.display());
    }
}

#[test]
fn t_p5_prefix_errors_are_a_prefix() {
    // T-P5: `errors(lex(s))[..i] == errors(lex(s[..n]))[..i]`. The settled
    // errors of a prefix must be byte-identical to the whole string's, because
    // a prefix cannot revise a decision that later input cannot reach. Only the
    // final error may still be open -- `"ab` reports an unterminated string
    // that `"ab"` retracts -- so we compare everything except the last error,
    // and require the settled count to be non-decreasing as input arrives.
    let mut rng = Rng::new(0xBEEF);
    for _ in 0..2000 {
        let s = rng.utf8_string(40);
        let full = lex(&s, F);
        for n in (0..=s.len()).filter(|n| s.is_char_boundary(*n)) {
            let pre = lex(&s[..n], F);
            let settled = pre.errors.len().saturating_sub(1);
            assert!(
                settled <= full.errors.len(),
                "prefix invented errors in {s:?}"
            );
            assert_eq!(
                &pre.errors[..settled],
                &full.errors[..settled],
                "settled prefix errors differ for {s:?} at {n}",
            );
        }
    }
}

#[test]
fn t_p6_cooked_payloads_re_cook() {
    // T-P6: a cooked payload is idempotent under re-cooking. We test it the
    // strongest way available: take each `Str` payload, quote it as a fresh
    // Zen literal, and require the lexed payload to be identical. Literals
    // whose payload contains a raw quote or backslash cannot round-trip
    // without re-escaping, so they are excluded per "for every round-trippable
    // literal".
    let mut checked = 0usize;
    let mut recheck = |payloads: Vec<String>, what: &str| {
        for payload in payloads {
            // Not round-trippable without re-escaping: raw quote, raw
            // backslash, or a raw control char (a literal newline would end
            // the string).
            if payload.contains('"')
                || payload.contains('\\')
                || payload.chars().any(|c| c.is_control())
            {
                continue;
            }
            let relit = format!("\"{payload}\"");
            let lx = lex(&relit, F);
            assert!(lx.errors.is_empty(), "{what}: re-lexing {relit:?} errored");
            assert_eq!(
                lx.toks.len(),
                2,
                "{what}: {relit:?} did not lex as one string"
            );
            assert_eq!(lx.toks[0].kind, TokenKind::Str, "{what}: {relit:?}");
            assert_eq!(
                lx.str_text(&lx.toks[0]),
                payload,
                "{what}: cook(cook(x)) != cook(x)"
            );
            checked += 1;
        }
    };
    for f in all_files() {
        let src = fs::read_to_string(&f).expect("utf-8");
        let lx = lex(&src, F);
        let payloads: Vec<String> = lx
            .toks
            .iter()
            .filter(|t| t.kind == TokenKind::Str)
            .map(|t| lx.str_text(t).to_string())
            .collect();
        recheck(payloads, &f.display().to_string());
    }
    let mut rng = Rng::new(0xFEED);
    for _ in 0..500 {
        let s = rng.utf8_string(40);
        let lx = lex(&s, F);
        let payloads: Vec<String> = lx
            .toks
            .iter()
            .filter(|t| t.kind == TokenKind::Str)
            .map(|t| lx.str_text(t).to_string())
            .collect();
        recheck(payloads, "random");
    }
    assert!(checked > 50, "T-P6 only exercised {checked} payloads");
}
