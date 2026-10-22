//! Row-driven lexer tests transcribed from `docs/05-lexer.md` §11.
//!
//! Each `#[test]` fn corresponds to one row of the test plan, T-A1 …
//! T-C25. Number payload values use the *corrected* constants (see
//! `lex/number.rs` tests): `2**47 = 140737488355328.0`, hex `0x800000000000`
//! promotes to the same value, `i48::MAX = 140737488355327`.

mod common;

use common::{assert_errors, kinds, kinds_no_eof, lx, reaches_eof};
use zen_syntax::lex::{InterpPart, LexErrorKind, NumErr, NumLit, StrForm, TokenKind};

// ── §11.1 Atoms and keywords ───────────────────────────────────────────

#[test]
fn atom_ident() {
    let lx = lx("foo");
    assert_eq!(lx.toks[0].kind, TokenKind::Ident);
    assert_eq!(lx.ident(&lx.toks[0]), "foo");
}

#[test]
fn atom_underscore_ident() {
    let lx = lx("_x1");
    assert_eq!(lx.toks[0].kind, TokenKind::Ident);
    assert_eq!(lx.ident(&lx.toks[0]), "_x1");
}

#[test]
fn atom_keyword() {
    assert_eq!(kinds("let"), vec![TokenKind::KwLet, TokenKind::Eof]);
}

#[test]
fn keyword_after_dot_is_ident() {
    assert_eq!(
        kinds("a.default"),
        vec![
            TokenKind::Ident,
            TokenKind::Dot,
            TokenKind::Ident,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn keyword_after_safe_dot_is_ident() {
    assert_eq!(
        kinds("a?.null"),
        vec![
            TokenKind::Ident,
            TokenKind::SafeDot,
            TokenKind::Ident,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn keyword_is_not_ident_elsewhere() {
    assert_eq!(kinds("null"), vec![TokenKind::KwNil, TokenKind::Eof]);
}

#[test]
fn global_folds_to_var() {
    assert_eq!(kinds("global"), vec![TokenKind::KwVar, TokenKind::Eof]);
}

#[test]
fn def_folds_to_fn() {
    assert_eq!(kinds("def"), vec![TokenKind::KwFn, TokenKind::Eof]);
}

#[test]
fn null_and_nil_one_kind() {
    assert_eq!(
        kinds_no_eof("nil null"),
        vec![TokenKind::KwNil, TokenKind::KwNil]
    );
}

#[test]
fn all_six_fn_spellings() {
    assert_eq!(
        kinds_no_eof("fn func function procedure proc def"),
        vec![TokenKind::KwFn; 6],
    );
}

#[test]
fn amp_amp_folds_to_and() {
    assert_eq!(
        kinds_no_eof("a && b"),
        vec![TokenKind::Ident, TokenKind::KwAnd, TokenKind::Ident],
    );
}

#[test]
fn except_folds_to_catch() {
    assert_eq!(kinds("except"), vec![TokenKind::KwCatch, TokenKind::Eof]);
}

#[test]
fn empty_file() {
    assert_eq!(kinds(""), vec![TokenKind::Eof]);
}

#[test]
fn whitespace_only_file() {
    assert_eq!(kinds("  \n\t\n"), vec![TokenKind::Eof]);
}

#[test]
fn bom_skipped_spans_absolute() {
    let lx = lx("\u{FEFF}let x = 1");
    assert_eq!(lx.toks[0].kind, TokenKind::KwLet);
    assert_eq!(lx.toks[0].lo, 3, "BOM is 3 bytes, spans stay in src");
}

#[test]
fn non_leading_bom_is_illegal() {
    assert_errors(
        "a\u{FEFF}b",
        &[LexErrorKind::UnexpectedCharacter { ch: '\u{FEFF}' }],
    );
}

// ── §11.2 Comments and the `#` truth table ────────────────────────────

#[test]
fn hash_dict_literal() {
    assert_eq!(
        kinds_no_eof("#{a:1}"),
        vec![
            TokenKind::DictStart,
            TokenKind::Ident,
            TokenKind::Colon,
            TokenKind::Int,
            TokenKind::RBrace,
        ],
    );
}

#[test]
fn hash_space_brace_is_comment() {
    // A comment-only file emits no tokens at all: the comment consumes to the
    // newline and A4 suppresses the (only) newline, so no `Newline` is left.
    assert_eq!(kinds_no_eof("# {a:1}"), Vec::<TokenKind>::new());
}

#[test]
fn comment_ends_at_newline() {
    assert_eq!(
        kinds_no_eof("x = 1 #\n#{a:1}"),
        vec![
            TokenKind::Ident,
            TokenKind::Assign,
            TokenKind::Int,
            TokenKind::Newline,
            TokenKind::DictStart,
            TokenKind::Ident,
            TokenKind::Colon,
            TokenKind::Int,
            TokenKind::RBrace,
        ],
    );
}

#[test]
fn spaced_percent_brace_is_modulo_block() {
    assert_eq!(
        kinds_no_eof("a % {b}"),
        vec![
            TokenKind::Ident,
            TokenKind::Percent,
            TokenKind::LBrace,
            TokenKind::Ident,
            TokenKind::RBrace,
        ],
    );
}

#[test]
fn glued_percent_brace_is_set() {
    assert_eq!(
        kinds_no_eof("a %{b}"),
        vec![
            TokenKind::Ident,
            TokenKind::SetStart,
            TokenKind::Ident,
            TokenKind::RBrace
        ],
    );
}

#[test]
fn sigils_inside_strings_are_literal() {
    let lx = lx("\"#{\"");
    assert_eq!(lx.str_text(&lx.toks[0]), "#{");
}

#[test]
fn block_comment_swallows_newline() {
    assert_eq!(kinds_no_eof("/* a\nb */x"), vec![TokenKind::Ident]);
}

#[test]
fn comment_mid_expression_emits_nothing() {
    assert_eq!(
        kinds_no_eof("a /*x*/ b"),
        vec![TokenKind::Ident, TokenKind::Ident],
    );
}

// ── §11.3 Numbers ──────────────────────────────────────────────────────

fn num(src: &str) -> NumLit {
    let lx = lx(src);
    lx.num(&lx.toks[0]).expect("expected a number literal")
}

fn int(src: &str) -> i64 {
    match num(src) {
        NumLit::Int(v) => v,
        NumLit::Float(_) => panic!("expected Int for {src:?}"),
    }
}

fn float(src: &str) -> f64 {
    match num(src) {
        NumLit::Float(v) => v,
        NumLit::Int(_) => panic!("expected Float for {src:?}"),
    }
}

#[test]
fn number_decimal_zero() {
    assert_eq!(int("0"), 0);
}

#[test]
fn number_decimal_int() {
    assert_eq!(int("42"), 42);
}

#[test]
fn number_leading_zero_decimal() {
    assert_eq!(int("010"), 10);
}

#[test]
fn number_float() {
    assert_eq!(float("3.14"), f64::from_bits(0x4009_1EB8_51EB_851F));
}

#[test]
fn number_leading_dot_float() {
    assert_eq!(float(".5"), 0.5);
}

#[test]
fn number_trailing_dot_float() {
    assert_eq!(float("5."), 5.0);
}

#[test]
fn number_int_exponent_is_float() {
    assert_eq!(float("1e10"), 1e10);
}

#[test]
fn number_uppercase_negative_exponent() {
    assert_eq!(float("1E-10"), 1e-10);
}

#[test]
fn number_signed_exponent() {
    assert_eq!(float("2.5E+3"), 2500.0);
}

#[test]
fn number_separators() {
    assert_eq!(int("1_000_000"), 1_000_000);
}

#[test]
fn number_hex_lower() {
    assert_eq!(int(r"0xff"), 255);
}

#[test]
fn number_hex_upper() {
    assert_eq!(int(r"0XFF"), 255);
}

#[test]
fn number_hex_separators() {
    assert_eq!(int(r"0xFF_FF"), 65535);
}

#[test]
fn number_binary() {
    assert_eq!(int(r"0b1010_1010"), 170);
}

#[test]
fn number_octal() {
    assert_eq!(int(r"0o755"), 493);
}

#[test]
fn number_i48_max_minus_one() {
    assert_eq!(int("140737488355326"), 140737488355326);
}

#[test]
fn number_i48_max() {
    assert_eq!(int("140737488355327"), 140737488355327);
}

#[test]
fn number_i48_max_plus_one_promotes() {
    assert_eq!(float("140737488355328"), 140737488355328.0);
}

#[test]
fn number_i64_max_promotes_not_wraps() {
    let lx = lx("9223372036854775807");
    match lx.num(&lx.toks[0]) {
        Some(NumLit::Float(v)) => assert_eq!(v, 9.223372036854776e18),
        Some(NumLit::Int(v)) => panic!("expected Float, got Int({v})"),
        None => panic!("no payload"),
    }
}

#[test]
fn number_hex_boundary() {
    assert_eq!(int(r"0x7FFFFFFFFFFF"), 140737488355327);
}

#[test]
fn number_hex_above_boundary() {
    assert_eq!(float(r"0x800000000000"), 140737488355328.0);
}

#[test]
fn number_multi_dot_rejected() {
    assert_errors(
        "1.2.3",
        &[LexErrorKind::InvalidNumber {
            reason: NumErr::SecondDot,
        }],
    );
}

#[test]
fn number_multi_dot_recovers() {
    let lx = lx("1.2.3");
    assert_eq!(lx.toks[0].kind, TokenKind::Float);
    assert_eq!(lx.toks[1].kind, TokenKind::Float);
    assert_eq!(lx.errors.len(), 1);
    reaches_eof("1.2.3");
}

#[test]
fn float_member_access_ok() {
    assert_eq!(
        kinds_no_eof("1.2.real"),
        vec![TokenKind::Float, TokenKind::Dot, TokenKind::Ident],
    );
}

#[test]
fn dotdot_after_number_is_range() {
    assert_eq!(
        kinds_no_eof(r"1..3"),
        vec![TokenKind::Int, TokenKind::DotDot, TokenKind::Int],
    );
}

#[test]
fn glued_identifier_rejected() {
    assert_errors(
        "1abc",
        &[LexErrorKind::InvalidNumber {
            reason: NumErr::GluedIdentifier,
        }],
    );
}

#[test]
fn bare_e_rejected() {
    assert_errors(
        r"1e",
        &[LexErrorKind::InvalidNumber {
            reason: NumErr::EmptyExponent,
        }],
    );
}

#[test]
fn trailing_separator_rejected() {
    assert_errors(
        "1_",
        &[LexErrorKind::InvalidNumber {
            reason: NumErr::BadSeparator,
        }],
    );
}

#[test]
fn doubled_separator_rejected() {
    assert_errors(
        "1__0",
        &[LexErrorKind::InvalidNumber {
            reason: NumErr::BadSeparator,
        }],
    );
}

#[test]
fn prefix_without_digits_rejected() {
    assert_errors(
        r"0x",
        &[LexErrorKind::InvalidNumber {
            reason: NumErr::NoDigitsAfterPrefix,
        }],
    );
}

#[test]
fn digit_outside_base_rejected() {
    assert_errors(
        r"0b2",
        &[LexErrorKind::InvalidNumber {
            reason: NumErr::InvalidDigit,
        }],
    );
}

#[test]
fn trailing_dot_float_glued_identifier() {
    assert_errors(
        r"5.foo",
        &[LexErrorKind::InvalidNumber {
            reason: NumErr::GluedIdentifier,
        }],
    );
}

#[test]
fn separator_after_radix_prefix_rejected() {
    assert_errors(
        r"0x_FF",
        &[LexErrorKind::InvalidNumber {
            reason: NumErr::BadSeparator,
        }],
    );
}

#[test]
fn trailing_separator_radix_rejected() {
    assert_errors(
        r"0xFF_",
        &[LexErrorKind::InvalidNumber {
            reason: NumErr::BadSeparator,
        }],
    );
}

// ── §11.4 Strings ──────────────────────────────────────────────────────

fn str_payload(src: &str) -> String {
    let lx = lx(src);
    lx.str_text(&lx.toks[0]).to_string()
}

#[test]
fn string_plain_double() {
    assert_eq!(str_payload(r#""a""#), "a");
}

#[test]
fn string_single_quotes_no_interp() {
    assert_eq!(str_payload(r#"'a${b}'"#), r#"a${b}"#);
}

#[test]
fn string_backslash() {
    assert_eq!(str_payload(r#""\\""#), "\\");
}

#[test]
fn string_every_simple_escape() {
    assert_eq!(
        str_payload(r#""\n\t\r\0\a\b\f\v\\\'\"\`\$""#),
        "\n\t\r\u{0}\u{7}\u{8}\u{c}\u{b}\\'\"`$",
    );
}

#[test]
fn string_e_equals_x1b() {
    let lx = lx(r#""\e" "\x1b""#);
    assert_eq!(lx.toks[0].kind, TokenKind::Str);
    assert_eq!(lx.toks[1].kind, TokenKind::Str);
    assert_eq!(lx.str_text(&lx.toks[0]), lx.str_text(&lx.toks[1]));
}

#[test]
fn string_x_nn() {
    assert_eq!(str_payload(r#""\x41\x42\x43""#), "ABC");
}

#[test]
fn string_latin1_xff() {
    let pal = lx(r#""\xFF""#);
    let b = pal.str_bytes(&pal.toks[0]);
    assert_eq!(b, &[0xC3, 0xBF]);
}

#[test]
fn string_unicode_6_digits() {
    let lx = lx(r#""\u{1F600}""#);
    let text = lx.str_text(&lx.toks[0]);
    assert_eq!(text.chars().count(), 1);
    assert_eq!(text, "😀");
}

#[test]
fn string_uppercase_unicode_too_big() {
    assert_errors(
        r#""\U{110000}""#,
        &[LexErrorKind::InvalidCodePoint { cp: 0x110000 }],
    );
}

#[test]
fn string_surrogate_rejected() {
    assert_errors(
        r#""\U{D800}""#,
        &[LexErrorKind::InvalidCodePoint { cp: 0xD800 }],
    );
}

#[test]
fn string_unicode_empty_rejected() {
    assert_errors(
        r#""\u{}""#,
        &[LexErrorKind::EmptyUnicodeEscape { tag: b'u' }],
    );
}

#[test]
fn string_unicode_too_long_rejected() {
    assert_errors(
        r#""\u{1234567}""#,
        &[LexErrorKind::UnicodeEscapeTooLong { tag: b'u' }],
    );
}

#[test]
fn string_incomplete_hex_rejected() {
    assert_errors(r#""\x1""#, &[LexErrorKind::IncompleteHexEscape]);
}

#[test]
fn string_unknown_escape_warns() {
    let lx = lx(r#""\q""#);
    assert_eq!(lx.str_text(&lx.toks[0]), "q");
    assert_eq!(lx.errors.len(), 1);
    assert!(!lx.errors[0].is_error(), "unknown escape is a warning");
}

#[test]
fn string_unterminated() {
    assert_errors(
        "\"abc",
        &[LexErrorKind::UnterminatedString {
            form: StrForm::Double,
            line: 1,
        }],
    );
}

#[test]
fn string_unterminated_triple() {
    assert_errors(
        "\"\"\"abc",
        &[LexErrorKind::UnterminatedTripleString {
            form: StrForm::TripleDouble,
        }],
    );
}

#[test]
fn string_raw_newline_rejected() {
    assert_errors("\"a\nb\"", &[LexErrorKind::RawNewlineInString { line: 1 }]);
}

#[test]
fn string_multi_line_double() {
    assert_eq!(str_payload("\"\"\"a\nb\"\"\""), "a\nb");
}

#[test]
fn string_triple_single_no_interp() {
    assert_eq!(str_payload("'''a${b}'''"), "a${b}");
}

#[test]
fn string_exact_triple_detection() {
    // RULING S2: `"" ""` is two adjacent empty strings, never one triple.
    let lx = lx("\"\" \"\"");
    assert_eq!(lx.toks.len(), 3);
    assert_eq!(lx.toks[0].kind, TokenKind::Str);
    assert_eq!(lx.toks[1].kind, TokenKind::Str);
    assert_eq!(lx.str_text(&lx.toks[0]), "");
    assert_eq!(lx.str_text(&lx.toks[1]), "");
}

#[test]
fn string_no_triple_backtick() {
    // RULING S1: the backtick is excluded from triple detection, so `` ``x`` ``
    // is Str("") Ident(x) Str("") — no run of backticks lives inside a `Str`.
    let lx = lx("``x``");
    assert_eq!(
        lx.toks.iter().map(|t| t.kind).collect::<Vec<_>>(),
        vec![
            TokenKind::Str,
            TokenKind::Ident,
            TokenKind::Str,
            TokenKind::Eof
        ],
    );
    for t in &lx.toks {
        assert!(!lx.str_text(t).contains('`'), "backtick run inside a Str");
    }
}

#[test]
fn string_interpolation() {
    let lx = lx(r#""a${b}c""#);
    let i = &lx.interps[0];
    assert_eq!(i.parts.len(), 3);
    match &i.parts[0] {
        InterpPart::Text(r) => assert_eq!(r.as_str(), "a"),
        _ => panic!("part 0"),
    }
    assert_eq!(i.parts[1], InterpPart::Expr(0));
    match &i.parts[2] {
        InterpPart::Text(r) => assert_eq!(r.as_str(), "c"),
        _ => panic!("part 2"),
    }
    let sub = &i.subs[0];
    assert_eq!(sub.toks[0].kind, TokenKind::Ident);
    assert_eq!(sub.ident(&sub.toks[0]), "b");
}

#[test]
fn string_empty_interpolation_sublex_ok() {
    // "parse error in the sub-lex, not a lex error": the sub-lex completes.
    let lx = lx(r#""${}""#);
    assert_eq!(
        lx.interps[0].subs[0].toks.last().map(|t| t.kind),
        Some(TokenKind::Eof)
    );
}

#[test]
fn string_nested_interpolation() {
    // The inner `"${x}"` is itself an interpolated string, so the outer body's
    // sub-lex yields one `Interp`, not a `Str`.
    let lx = lx(r#""${ "${x}" }""#);
    let sub = &lx.interps[0].subs[0];
    let inner: Vec<TokenKind> = sub.toks.iter().map(|t| t.kind).collect();
    assert_eq!(inner, vec![TokenKind::Interp, TokenKind::Eof]);
    assert_eq!(sub.interps[0].parts.len(), 1);
}

#[test]
fn string_string_aware_interp_body() {
    let lx = lx(r#""${d["}"]}""#);
    let sub = &lx.interps[0].subs[0];
    let kinds: Vec<TokenKind> = sub.toks.iter().map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::Ident,
            TokenKind::LBracket,
            TokenKind::Str,
            TokenKind::RBracket,
            TokenKind::Eof,
        ],
    );
}

#[test]
fn string_dollar_not_interp() {
    let lx = lx(r#""$x $$""#);
    assert_eq!(lx.toks[0].kind, TokenKind::Str);
    assert_eq!(lx.str_text(&lx.toks[0]), r#"$x $$"#);
}

#[test]
fn string_unterminated_interpolation() {
    let lx = lx(r#""${a""#);
    assert!(lx
        .errors
        .iter()
        .any(|e| matches!(e.kind, LexErrorKind::UnterminatedInterpolation { .. })));
}

#[test]
fn string_multibyte_literal() {
    let text = str_payload(r#""█▀""#);
    assert_eq!(text.chars().count(), 2);
    assert_eq!(text.len(), 6);
}

// ── §11.5 Newline and statement separation ─────────────────────────────

#[test]
fn nl_plain_terminator() {
    assert_eq!(
        kinds_no_eof("a\nb"),
        vec![TokenKind::Ident, TokenKind::Newline, TokenKind::Ident],
    );
}

#[test]
fn nl_blank_lines_collapse() {
    assert_eq!(
        kinds_no_eof("a\n\n\nb"),
        vec![TokenKind::Ident, TokenKind::Newline, TokenKind::Ident],
    );
}

#[test]
fn nl_leading_blanks_dropped() {
    assert_eq!(kinds_no_eof("\n\na"), vec![TokenKind::Ident]);
}

#[test]
fn nl_trailing_plus_continues() {
    // A2: the previous token is `+`, an operator, so the newline is suppressed
    // and the two `Str`s stay in one statement.
    assert_eq!(
        kinds_no_eof("a = \"a\" +\n\"b\""),
        vec![
            TokenKind::Ident,
            TokenKind::Assign,
            TokenKind::Str,
            TokenKind::Plus,
            TokenKind::Str,
        ],
    );
}

#[test]
fn nl_trailing_minus_continues() {
    assert_eq!(
        kinds_no_eof("a = 1 -\n2 +\n3"),
        vec![
            TokenKind::Ident,
            TokenKind::Assign,
            TokenKind::Int,
            TokenKind::Minus,
            TokenKind::Int,
            TokenKind::Plus,
            TokenKind::Int,
        ],
    );
}

#[test]
fn nl_trailing_operator_not_multistatement() {
    assert_eq!(
        kinds_no_eof("a = 5\n-3"),
        vec![
            TokenKind::Ident,
            TokenKind::Assign,
            TokenKind::Int,
            TokenKind::Newline,
            TokenKind::Minus,
            TokenKind::Int,
        ],
    );
}

#[test]
fn nl_same_after_call() {
    assert_eq!(
        kinds_no_eof("f(1)\n-2"),
        vec![
            TokenKind::Ident,
            TokenKind::LParen,
            TokenKind::Int,
            TokenKind::RParen,
            TokenKind::Newline,
            TokenKind::Minus,
            TokenKind::Int,
        ],
    );
}

#[test]
fn nl_inside_paren() {
    assert_eq!(
        kinds_no_eof("max(10,\nmax(2,\n7))"),
        vec![
            TokenKind::Ident,
            TokenKind::LParen,
            TokenKind::Int,
            TokenKind::Comma,
            TokenKind::Ident,
            TokenKind::LParen,
            TokenKind::Int,
            TokenKind::Comma,
            TokenKind::Int,
            TokenKind::RParen,
            TokenKind::RParen,
        ],
    );
}

#[test]
fn nl_inside_bracket() {
    assert_eq!(
        kinds_no_eof("[\n1,\n2,\n]"),
        vec![
            TokenKind::LBracket,
            TokenKind::Int,
            TokenKind::Comma,
            TokenKind::Int,
            TokenKind::Comma,
            TokenKind::RBracket,
        ],
    );
}

#[test]
fn nl_inside_dict() {
    assert_eq!(
        kinds_no_eof("#{\n\"a\":1,\n}"),
        vec![
            TokenKind::DictStart,
            TokenKind::Str,
            TokenKind::Colon,
            TokenKind::Int,
            TokenKind::Comma,
            TokenKind::RBrace,
        ],
    );
}

#[test]
fn nl_inside_set() {
    assert_eq!(
        kinds_no_eof("%{\n1,\n2\n}"),
        vec![
            TokenKind::SetStart,
            TokenKind::Int,
            TokenKind::Comma,
            TokenKind::Int,
            TokenKind::RBrace,
        ],
    );
}

#[test]
fn nl_inside_hash_paren() {
    assert_eq!(
        kinds_no_eof("#(\n1,\n2\n)"),
        vec![
            TokenKind::TupleStart,
            TokenKind::Int,
            TokenKind::Comma,
            TokenKind::Int,
            TokenKind::RParen,
        ],
    );
}

#[test]
fn nl_lbrace_suppresses_own_newline() {
    assert_eq!(
        kinds_no_eof("if c {\na\n}"),
        vec![
            TokenKind::KwIf,
            TokenKind::Ident,
            TokenKind::LBrace,
            TokenKind::Ident,
            TokenKind::Newline,
            TokenKind::RBrace,
        ],
    );
}

#[test]
fn nl_rbrace_terminates() {
    assert_eq!(
        kinds_no_eof("f() { }\ng() { }"),
        vec![
            TokenKind::Ident,
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::Newline,
            TokenKind::Ident,
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::RBrace,
        ],
    );
}

#[test]
fn nl_else_on_next_line() {
    assert_eq!(
        kinds_no_eof("if c { a }\nelse { b }"),
        vec![
            TokenKind::KwIf,
            TokenKind::Ident,
            TokenKind::LBrace,
            TokenKind::Ident,
            TokenKind::RBrace,
            TokenKind::Newline,
            TokenKind::KwElse,
            TokenKind::LBrace,
            TokenKind::Ident,
            TokenKind::RBrace,
        ],
    );
}

#[test]
fn nl_colon_continues() {
    assert_eq!(
        kinds_no_eof("case 1:\n\"one\""),
        vec![
            TokenKind::KwCase,
            TokenKind::Int,
            TokenKind::Colon,
            TokenKind::Str,
        ],
    );
}

#[test]
fn nl_dot_continues() {
    // A2 checks the PREVIOUS significant token, so `x = 5\n.real` still emits a
    // `Newline` after the `Int` (`Int` is not an operator). `Dot` continuing a
    // line applies when `Dot` is the previous token, e.g. `x = 5.\nreal`.
    assert_eq!(
        kinds_no_eof("x = 5\n.real"),
        vec![
            TokenKind::Ident,
            TokenKind::Assign,
            TokenKind::Int,
            TokenKind::Newline,
            TokenKind::Dot,
            TokenKind::Ident,
        ],
    );
    // `Dot` as the previous significant token suppresses the newline (A2); the
    // `DotDot` range form `0..\n10` reaches that branch cleanly.
    assert_eq!(
        kinds_no_eof("0..\n10"),
        vec![TokenKind::Int, TokenKind::DotDot, TokenKind::Int],
    );
}

#[test]
fn nl_comma_continues() {
    assert_eq!(
        kinds_no_eof("f(a,\nb)"),
        vec![
            TokenKind::Ident,
            TokenKind::LParen,
            TokenKind::Ident,
            TokenKind::Comma,
            TokenKind::Ident,
            TokenKind::RParen,
        ],
    );
}

#[test]
fn nl_comment_does_not_disturb() {
    assert_eq!(
        kinds_no_eof("a = 1 +\n/* c */\n2"),
        vec![
            TokenKind::Ident,
            TokenKind::Assign,
            TokenKind::Int,
            TokenKind::Plus,
            TokenKind::Int,
        ],
    );
}

#[test]
fn nl_semi_collapse() {
    assert_eq!(
        kinds_no_eof("a = 1;\nb"),
        vec![
            TokenKind::Ident,
            TokenKind::Assign,
            TokenKind::Int,
            TokenKind::Semi,
            TokenKind::Ident,
        ],
    );
}

#[test]
fn nl_double_semi_collapse() {
    assert_eq!(
        kinds_no_eof("a = 1;;b"),
        vec![
            TokenKind::Ident,
            TokenKind::Assign,
            TokenKind::Int,
            TokenKind::Semi,
            TokenKind::Ident,
        ],
    );
}

#[test]
fn nl_return_then_newline() {
    assert_eq!(
        kinds_no_eof("return\nx"),
        vec![TokenKind::KwReturn, TokenKind::Newline, TokenKind::Ident],
    );
}

#[test]
fn nl_throw_then_newline() {
    assert_eq!(
        kinds_no_eof("throw\nx"),
        vec![TokenKind::KwThrow, TokenKind::Newline, TokenKind::Ident],
    );
}

#[test]
fn nl_block_comment_eats_newlines() {
    assert_eq!(kinds_no_eof("/* a\nb */\nx"), vec![TokenKind::Ident]);
}

#[test]
fn nl_match_arms_split() {
    assert_eq!(
        kinds_no_eof("match v {\n1: \"a\",\n_ => \"b\"\n}"),
        vec![
            TokenKind::KwMatch,
            TokenKind::Ident,
            TokenKind::LBrace,
            TokenKind::Int,
            TokenKind::Colon,
            TokenKind::Str,
            TokenKind::Comma,
            TokenKind::Newline,
            TokenKind::Ident,
            TokenKind::ThinArrow,
            TokenKind::Str,
            TokenKind::Newline,
            TokenKind::RBrace,
        ],
    );
}

// ── §11.6 Errors, recovery, totality (unit-scaled rows) ────────────────

#[test]
fn err_illegal_character() {
    let lx = lx("a § b");
    assert!(lx
        .errors
        .iter()
        .any(|e| matches!(e.kind, LexErrorKind::UnexpectedCharacter { ch: '§' })));
    reaches_eof("a § b");
    // `§` is skipped after recording the error, so it yields no token: `a`, `b`,
    // and the always-present `Eof`.
    assert_eq!(lx.toks.len(), 3, "recovered tokens are a, b, Eof");
}

#[test]
fn err_unterminated_block_comment() {
    assert_errors("/* x", &[LexErrorKind::UnterminatedBlockComment]);
    reaches_eof("/* x");
}

#[test]
fn err_nested_block_comment_ruling_h2() {
    // RULING H2: `/* */` inside is the end; the stray `*/` is a parse-time job.
    let lx = lx("/* /* */ */");
    assert_eq!(lx.errors.len(), 0, "no lex error for a closed comment");
    reaches_eof("/* /* */ */");
}

#[test]
fn err_delimiter_depth_guard() {
    let src = "(".repeat(300);
    let lx = lx(&src);
    assert!(lx
        .errors
        .iter()
        .any(|e| matches!(e.kind, LexErrorKind::NestingTooDeep { depth: 256 })));
}

#[test]
fn err_no_trailing_synthetic_newline() {
    let lx = lx("a = 1");
    let n = lx.toks.len();
    assert_eq!(lx.toks[n - 2].kind, TokenKind::Int);
    assert_eq!(lx.toks[n - 1].kind, TokenKind::Eof);
}

// There is a function-level `FileId` import to keep clippy from flagging an
// unused import when only the helper uses the name; re-export for clarity.
#[allow(unused)]
use zen_syntax::span::FileId;

#[allow(unused)]
fn _keep_fileid_in_scope() -> u32 {
    u32::try_from(FileId(7).0).unwrap_or(0)
}
/// RULING S4: every string failure stays anchored on the opening quote *except*
/// for interpolation errors, whose caret points at the `${` so the user sees
/// where the missing `}` belongs. The `span.lo` is always the opening quote.
#[test]
fn s4_failures_stay_anchored_on_the_opening_quote() {
    let cases: &[(&str, usize, usize, usize)] = &[
        // (source, expected_span_lo, expected_at_lo, expected_at_width)
        ("x = \"oops\nb = 1\nc = 2", 4usize, 4usize, 1usize),
        ("x = \"\\u{ZZZ}\" + 1\nnext()", 4usize, 4usize, 1usize),
        ("x = \"\\xZZ\" + 1\nnext()", 4usize, 4usize, 1usize),
        ("x = \"abc", 4usize, 4usize, 1usize),
        ("x = \"${a", 4usize, 5usize, 2usize), // at = the `${`
        ("x = \"\"\"abc", 4usize, 4usize, 3usize), // triple quote
    ];
    for &(src, span_lo, at_lo, at_width) in cases {
        let lx = lx(src);
        let err = lx
            .errors
            .first()
            .unwrap_or_else(|| panic!("no error for {src:?}"));
        assert_eq!(err.span.lo as usize, span_lo, "span.lo moved for {src:?}");
        assert_eq!(err.at.lo as usize, at_lo, "at.lo moved for {src:?}");
        assert_eq!(
            (err.at.lo, err.at.hi),
            (at_lo as u32, at_lo as u32 + at_width as u32),
            "caret must cover exactly the expected region for {src:?}",
        );
        assert!(
            err.span.hi >= err.at.hi,
            "span must contain its own caret: {src:?}"
        );
        assert!(
            (err.span.hi as usize) <= src.len(),
            "span past end: {src:?}"
        );
    }
}

/// The counterpart to the anchor test: widening `hi` is what keeps the
/// resynchronised bytes inside a span. If this regresses, T-P3 fails.
#[test]
fn s3a_resync_bytes_stay_inside_the_error_span() {
    let cases: &[(&str, usize)] = &[
        ("x = \"oops\nb = 1\nc = 2", 15usize),
        ("x = \"\\u{ZZZ}\" + 1\nnext()", 18usize),
        ("x = \"\\xZZ\" + 1\nnext()", 15usize),
        ("\"abc\n", 5usize),
    ];
    for &(src, min_hi) in cases {
        let lx = lx(src);
        let err = lx.errors.first().expect("error");
        assert!(
            err.span.hi as usize >= min_hi,
            "{src:?}: error span must cover the resynced line (hi={}, need>={min_hi})",
            err.span.hi
        );
        reaches_eof(src);
    }
}

#[test]
fn keyword_pymport() {
    assert_eq!(kinds("pymport"), vec![TokenKind::KwPymport, TokenKind::Eof]);
}

#[test]
fn pymport_after_dot_is_ident() {
    assert_eq!(
        kinds("a.pymport"),
        vec![
            TokenKind::Ident,
            TokenKind::Dot,
            TokenKind::Ident,
            TokenKind::Eof
        ]
    );
}
