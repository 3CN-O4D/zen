//! Diagnostics gate (`docs/19-roadmap.md` M1): the parser reports errors
//! instead of panicking, and error spans point at the exact offending byte.

use zen_syntax::lex::lex;
use zen_syntax::parser::{ParseError, Parser};
use zen_syntax::span::FileId;

fn parse_err(src: &str) -> ParseError {
    let lx = lex(src, FileId(0));
    Parser::new(lx)
        .parse_module()
        .expect_err("input should not parse")
}

#[test]
fn m1_malformed_input_never_panics() {
    // A grab-bag of truncations and malformed constructs. The only requirement
    // is `Err` — reaching the assertion proves the parser did not panic.
    let bad = [
        "",
        "let",
        "let x =",
        "let x = }",
        "if",
        "if x",
        "if x {",
        "func",
        "func f(",
        "func f() {",
        "while {",
        "for x in {",
        "match x {",
        "when {",
        "try",
        "try {",
        "class",
        "class Foo extends {",
        "return +",
        "x = 1 +",
        "[1, 2",
        "#{a: 1",
        "%{1, 2",
        "(1, 2",
        "a[1",
        "a[1:",
        "[x for",
        "[x for x in",
        "[x for x in xs if",
        "import",
        "from x import",
        "new",
        "new Foo(",
        "x => ",
        "lambda(",
        "\"unterminated ${x}",
        "? :",
        "1 ? 2",
    ];
    for src in bad {
        let lx = lex(src, FileId(0));
        let _ = Parser::new(lx).parse_module();
    }
}

#[test]
fn m1_random_bytes_never_panic() {
    // Deterministic LCG over a token-ish alphabet: the lexer and parser must
    // return for every input, valid or not, without panicking. A panic here
    // fails the test (the harness unwinds and reports it).
    let alphabet = b"(){}[]<>.,:;?=+-*/%!&|^~\"'`#$\n\t abcxyz0123456789_";
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    for _ in 0..4000 {
        let len = next() % 64;
        let bytes: Vec<u8> = (0..len)
            .map(|_| alphabet[next() % alphabet.len()])
            .collect();
        if let Ok(src) = std::str::from_utf8(&bytes) {
            let lx = lex(src, FileId(0));
            let _ = Parser::new(lx).parse_module();
        }
    }
}

#[test]
fn m1_error_span_is_exact_byte() {
    // `a[1, 2}` — `]` is expected where `}` appears (offset 6).
    let e = parse_err("a[1, 2}");
    assert_eq!((e.span().lo, e.span().hi), (6, 7), "{e}");

    // `if x }` — `{` is expected where `}` appears (offset 5).
    let e = parse_err("if x }");
    assert_eq!((e.span().lo, e.span().hi), (5, 6), "{e}");

    // `func f() }` — `{` is expected where `}` appears (offset 9).
    let e = parse_err("func f() }");
    assert_eq!((e.span().lo, e.span().hi), (9, 10), "{e}");

    // `1 ? 2` — the ternary's `:` is expected at end of input (offset 5).
    let e = parse_err("1 ? 2");
    assert_eq!((e.span().lo, e.span().hi), (5, 5), "{e}");
}

#[test]
fn m1_truncated_input_reports_eof_at_end() {
    // An unclosed block must point at the end of the source, not byte 0.
    for (src, end) in [("match x {", 9), ("when {", 6), ("try {", 5)] {
        let e = parse_err(src);
        assert!(matches!(e, ParseError::UnexpectedEof { .. }), "{src}: {e}");
        assert_eq!(e.span().lo, end, "{src}: {e}");
    }
}

#[test]
fn m1_deep_nesting_errors_without_overflowing_the_stack() {
    // Deeply nested expressions and blocks must produce a diagnostic, not a
    // native stack overflow (which aborts the process and is unrecoverable).
    let parens = format!("{}1{}", "(".repeat(5000), ")".repeat(5000));
    let e = parse_err(&parens);
    assert!(matches!(e, ParseError::Custom { .. }), "{e}");

    let blocks = format!("{}print(1){}", "if true {".repeat(5000), "}".repeat(5000));
    let e = parse_err(&blocks);
    assert!(matches!(e, ParseError::Custom { .. }), "{e}");
}

#[test]
fn m1_legacy_brace_dict_emits_deprecation_warning() {
    // Legacy `{…}` in value position warns once, and the warning span covers
    // the whole dict (`docs/06-parser.md` §5.5).
    let src = "x = {\"a\": 1}";
    let lx = lex(src, FileId(0));
    let mut p = Parser::new(lx);
    p.parse_module().expect("parses");
    assert_eq!(p.warnings().len(), 1);
    let span = p.warnings().spans()[0];
    assert_eq!((span.lo, span.hi), (4, 12));

    // The new `#{…}` form is silent.
    let lx = lex("x = #{\"a\": 1}", FileId(0));
    let mut p = Parser::new(lx);
    p.parse_module().expect("parses");
    assert!(p.warnings().is_empty());

    // One warning per source site.
    let lx = lex("x = {\"a\": 1}\ny = {\"b\": 2}", FileId(0));
    let mut p = Parser::new(lx);
    p.parse_module().expect("parses");
    assert_eq!(p.warnings().len(), 2);
}
