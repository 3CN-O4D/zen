//! Byte dispatch tables: `class_of`, the operator index, and the digit bitmaps.
//!
//! See `docs/05-lexer.md` §10.4. Everything here is built at compile time so the
//! scanning loop has no range comparisons and no sequential string compares.

use super::kind::TokenKind;

/// What a byte is, as far as the main loop is concerned.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    Space,
    Newline,
    Semi,
    Hash,
    Quote,
    Digit,
    IdentStart,
    Slash,
    Operator,
    Illegal,
}

/// Every operator, grouped by first byte and sorted **longest-first** within each
/// group, so `**` is tried before `*`, `<<=` before `<<` before `<`, and `===`
/// before `==` before `=`.
pub const OPERATORS: &[(&str, TokenKind)] = &[
    // '!'
    ("!==", TokenKind::StrictNe),
    ("!=", TokenKind::Ne),
    ("!", TokenKind::Bang),
    // '%'
    ("%{", TokenKind::SetStart),
    ("%=", TokenKind::PercentEq),
    ("%", TokenKind::Percent),
    // '&'
    ("&&", TokenKind::KwAnd),
    ("&=", TokenKind::AmpEq),
    ("&", TokenKind::Amp),
    // '('
    ("(", TokenKind::LParen),
    // ')'
    (")", TokenKind::RParen),
    // '*'
    ("**", TokenKind::Pow),
    ("*=", TokenKind::StarEq),
    ("*", TokenKind::Star),
    // '+'
    ("++", TokenKind::Inc),
    ("+=", TokenKind::PlusEq),
    ("+", TokenKind::Plus),
    // ','
    (",", TokenKind::Comma),
    // '-'
    ("--", TokenKind::Dec),
    ("->", TokenKind::Arrow),
    ("-=", TokenKind::MinusEq),
    ("-", TokenKind::Minus),
    // '.'
    ("...", TokenKind::Ellipsis),
    ("..=", TokenKind::DotDotEq),
    ("..", TokenKind::DotDot),
    (".", TokenKind::Dot),
    // '/'
    ("/=", TokenKind::SlashEq),
    ("/", TokenKind::Slash),
    // ':'
    (":", TokenKind::Colon),
    // '<'
    ("<<=", TokenKind::ShlEq),
    ("<<", TokenKind::Shl),
    ("<=", TokenKind::Le),
    ("<", TokenKind::Lt),
    // '='
    ("===", TokenKind::StrictEq),
    ("==", TokenKind::Eq),
    ("=>", TokenKind::ThinArrow),
    ("=", TokenKind::Assign),
    // '>'
    (">>=", TokenKind::ShrEq),
    (">>", TokenKind::Shr),
    (">=", TokenKind::Ge),
    (">", TokenKind::Gt),
    // '?'
    ("??=", TokenKind::NullishEq),
    ("??", TokenKind::Nullish),
    ("?.", TokenKind::SafeDot),
    ("?", TokenKind::Question),
    // '['
    ("[", TokenKind::LBracket),
    // ']'
    ("]", TokenKind::RBracket),
    // '^'
    ("^=", TokenKind::CaretEq),
    ("^", TokenKind::Caret),
    // '{'
    ("{", TokenKind::LBrace),
    // '|'
    ("||", TokenKind::KwOr),
    ("|=", TokenKind::PipeEq),
    ("|", TokenKind::Pipe),
    // '}'
    ("}", TokenKind::RBrace),
    // '~'
    ("~", TokenKind::Tilde),
];

/// For each first byte, the half-open slice of `OPERATORS` to try.
const fn build_op_ranges(ops: &[(&'static str, TokenKind)]) -> [(u16, u16); 256] {
    let mut out = [(0u16, 0u16); 256];
    let mut b = 0usize;
    while b < 256 {
        let mut lo: u16 = 0;
        let mut hi: u16 = 0;
        let mut found = false;
        let mut k = 0usize;
        while k < ops.len() {
            if ops[k].0.as_bytes()[0] as usize == b {
                if !found {
                    lo = k as u16;
                    found = true;
                }
                hi = (k + 1) as u16;
            }
            k += 1;
        }
        out[b] = (lo, hi);
        b += 1;
    }
    out
}

/// 256-entry first-byte index (rule **M2**): one table load, then on average
/// 1.4 prefix compares, instead of the legacy's ~48 sequential compares.
pub static OP_RANGE: [(u16, u16); 256] = build_op_ranges(OPERATORS);

const fn starts_an_operator(b: u8) -> bool {
    let mut k = 0usize;
    while k < OPERATORS.len() {
        if OPERATORS[k].0.as_bytes()[0] == b {
            return true;
        }
        k += 1;
    }
    false
}

const fn build_class_table() -> [Class; 256] {
    let mut table = [Class::Illegal; 256];
    let mut b = 0usize;
    while b < 256 {
        let byte = b as u8;
        table[b] = match byte {
            b' ' | b'\t' | b'\r' | 0x0b | 0x0c => Class::Space,
            b'\n' => Class::Newline,
            b';' => Class::Semi,
            b'#' => Class::Hash,
            b'"' | b'\'' | b'`' => Class::Quote,
            b'0'..=b'9' => Class::Digit,
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => Class::IdentStart,
            b'/' => Class::Slash,
            _ => {
                if starts_an_operator(byte) {
                    Class::Operator
                } else {
                    Class::Illegal
                }
            }
        };
        b += 1;
    }
    table
}

/// One table load replaces a 12-arm range match (rule **M1**).
pub static CLASS: [Class; 256] = build_class_table();

/// Rule **M1**.
#[inline]
pub fn class_of(b: u8) -> Class {
    CLASS.get(b as usize).copied().unwrap_or(Class::Illegal)
}

/// The UTF-8 encoding of `U+FEFF`.
pub const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// True when the three bytes at `src[at..]` are a byte-order mark.
#[inline]
pub fn is_bom_at(src: &[u8], at: usize) -> bool {
    match src.get(at..at.saturating_add(3)) {
        Some(w) => *w == BOM,
        None => false,
    }
}

/// Bytes that may continue an identifier.
///
/// Deliberately ASCII-gated: `§` must reach the illegal-character path so the
/// diagnostic names the character the user typed, and so `a § b` produces
/// exactly one error (T-E1). Non-ASCII is still allowed *inside* an identifier
/// once it starts with an ASCII letter, which is where the corpus puts it.
#[inline]
pub const fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || (b >= 0x80 && b != 0xEF)
}

const fn digit_set(lo: u8, hi: u8) -> [u8; 256] {
    let mut set = [0u8; 256];
    let mut b = lo as usize;
    while b <= hi as usize {
        set[b] = 1;
        b += 1;
    }
    set
}

/// 256-entry bitmaps, so `eat_digits` is one load and one compare per byte.
pub static DIGITS2: [u8; 256] = digit_set(b'0', b'1');
pub static DIGITS8: [u8; 256] = digit_set(b'0', b'7');
pub static DIGITS10: [u8; 256] = digit_set(b'0', b'9');

const fn hex_digit_set() -> [u8; 256] {
    let mut set = digit_set(b'0', b'9');
    let mut b = b'a' as usize;
    while b <= b'f' as usize {
        set[b] = 1;
        b += 1;
    }
    let mut b = b'A' as usize;
    while b <= b'F' as usize {
        set[b] = 1;
        b += 1;
    }
    set
}

pub static DIGITS16: [u8; 256] = hex_digit_set();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_are_as_documented() {
        assert_eq!(class_of(b' '), Class::Space);
        assert_eq!(class_of(b'\n'), Class::Newline);
        assert_eq!(class_of(b'\r'), Class::Space);
        assert_eq!(class_of(b';'), Class::Semi);
        assert_eq!(class_of(b'#'), Class::Hash);
        assert_eq!(class_of(b'"'), Class::Quote);
        assert_eq!(class_of(b'\''), Class::Quote);
        assert_eq!(class_of(b'`'), Class::Quote);
        assert_eq!(class_of(b'7'), Class::Digit);
        assert_eq!(class_of(b'_'), Class::IdentStart);
        assert_eq!(class_of(b'Z'), Class::IdentStart);
        assert_eq!(class_of(b'/'), Class::Slash);
        assert_eq!(class_of(b'+'), Class::Operator);
        assert_eq!(class_of(b'\\'), Class::Illegal);
        assert_eq!(class_of(0x00), Class::Illegal);
        assert_eq!(class_of(0x7f), Class::Illegal);
        // A multi-byte UTF-8 lead byte is not an ident start and not an operator.
        assert_eq!(class_of(0xC2), Class::Illegal);
    }

    #[test]
    fn every_operator_byte_has_a_one_byte_fallback() {
        // §10.3: this is what makes `lex_operator` total, and why it needs no
        // `unreachable!()`.
        for b in 0u16..=255 {
            let byte = u8::try_from(b).unwrap_or(0);
            if class_of(byte) != Class::Operator {
                continue;
            }
            let (lo, hi) = OP_RANGE[b as usize];
            let bucket = &OPERATORS[lo as usize..hi as usize];
            assert!(!bucket.is_empty(), "no operators for byte {byte:#04x}");
            assert!(
                bucket.iter().any(|(text, _)| text.len() == 1),
                "byte {byte:#04x} lacks a 1-byte fallback"
            );
        }
    }

    #[test]
    fn buckets_are_longest_first() {
        for (b, &(lo, hi)) in OP_RANGE.iter().enumerate() {
            let bucket = &OPERATORS[lo as usize..hi as usize];
            for pair in bucket.windows(2) {
                assert!(
                    pair[0].0.len() >= pair[1].0.len(),
                    "byte {b:#04x}: {:?} must precede {:?}",
                    pair[0].0,
                    pair[1].0
                );
            }
            for pair in bucket.windows(2) {
                assert_eq!(
                    pair[0].0.as_bytes()[0] as usize,
                    pair[1].0.as_bytes()[0] as usize,
                    "bucket {b:#04x} mixes first bytes"
                );
            }
        }
    }

    #[test]
    fn longest_match_wins() {
        // Longest-first ordering: `==` must beat `=`, `!==` must beat
        // `!=`. So within a bucket, the FIRST entry that is a prefix of
        // `text` must be `text` itself.
        for (text, kind) in OPERATORS {
            let first = text.as_bytes()[0];
            let (lo, hi) = OP_RANGE[first as usize];
            let bucket = &OPERATORS[lo as usize..hi as usize];
            let hit = bucket
                .iter()
                .find(|(t, _)| text.starts_with(t))
                .map(|(_, k)| *k);
            assert_eq!(hit, Some(*kind), "{text}");
        }
    }

    #[test]
    fn bom_detection() {
        assert!(is_bom_at(&BOM, 0));
        assert!(is_bom_at(b"let \xEF\xBB\xBF", 4));
        assert!(!is_bom_at(b"let", 0));
        assert!(!is_bom_at(b"x\xEF\xBB", 1));
    }

    #[test]
    fn ident_continuation_rules() {
        assert!(is_ident_continue(b'a'));
        assert!(is_ident_continue(b'9'));
        assert!(is_ident_continue(b'_'));
        assert!(is_ident_continue(0xC3));
        assert!(!is_ident_continue(0xEF));
        assert!(!is_ident_continue(b' '));
        assert!(!is_ident_continue(b'-'));
    }
}
