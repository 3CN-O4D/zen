//! Number scanning: the four integer radixes, decimals, floats, and the i48
//! overflow-promotes-to-float rule.
//!
//! See `docs/05-lexer.md` §5.

use super::table::{DIGITS10, DIGITS16, DIGITS2, DIGITS8};

/// The i48 range fixed by `ADR-0001`. Payloads are stored in an `i64` that is
/// always inside these bounds; the range is what the arithmetic in
/// [`finish_int`] enforces.
pub const I48_MIN: i64 = -140_737_488_355_328;
pub const I48_MAX: i64 = 140_737_488_355_327;

/// Why a numeric literal is invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumErr {
    NoDigits,
    NoDigitsAfterPrefix,
    InvalidDigit,
    EmptyExponent,
    SecondDot,
    BadSeparator,
    GluedIdentifier,
}

impl NumErr {
    pub const fn message(self) -> &'static str {
        match self {
            NumErr::NoDigits => "expected a digit",
            NumErr::NoDigitsAfterPrefix => "expected at least one digit after the radix prefix",
            NumErr::InvalidDigit => "digit is not valid for this base",
            NumErr::EmptyExponent => "expected digits after the exponent marker",
            NumErr::SecondDot => "multi-dot literals are not allowed; write `1.2` or `1..2`",
            NumErr::BadSeparator => "`_` may only appear between digits",
            NumErr::GluedIdentifier => "a literal may not be immediately followed by an identifier",
        }
    }
}

/// A literal's value, once scanned.
/// A valued literal. `Int` always holds an i48-range value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NumLit {
    Int(i64),
    Float(f64),
}

impl NumLit {
    #[inline]
    pub const fn kind_is_int(self) -> bool {
        matches!(self, NumLit::Int(_))
    }

    #[inline]
    pub fn as_i48(self) -> Option<i64> {
        match self {
            NumLit::Int(v) => Some(v),
            NumLit::Float(_) => None,
        }
    }
}

/// The outcome of one number scan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NumScan {
    /// Scanned cleanly; `end` is one past the last byte.
    Ok { end: usize, lit: NumLit },
    /// Failed at `err_pos`. `ok_end` is set when a valid prefix ended exactly
    /// there, which is what lets `1.2.3` recover as `Float(1.2)` and then
    /// rescan `.3` (rule §9.2, test T-N23).
    Err {
        err_pos: usize,
        reason: NumErr,
        ok_end: usize,
    },
}

impl NumScan {
    fn ok(end: usize, lit: NumLit) -> Self {
        NumScan::Ok { end, lit }
    }
}

/// Consume `[digit | '_']*` where `set` is a 256-entry bitmap. Underscores are
/// left in place here and dropped by the accumulator, so the error rules can
/// still see where they were.
#[inline]
fn eat_digits(src: &[u8], mut i: usize, set: &[u8; 256]) -> usize {
    while let Some(&b) = src.get(i) {
        let idx = usize::from(b);
        if set.get(idx).copied().unwrap_or(0) != 0 || b == b'_' {
            i += 1;
        } else {
            break;
        }
    }
    i
}

/// Rule **N3**: `_` only *between* digits — no leading, trailing, or doubled.
fn bad_separator(run: &[u8]) -> bool {
    if run.is_empty() {
        return false;
    }
    if run.first() == Some(&b'_') || run.last() == Some(&b'_') {
        return true;
    }
    run.windows(2).any(|w| w[0] == b'_' && w[1] == b'_')
}

fn radix_and_set(prefix: u8) -> Option<(u32, &'static [u8; 256])> {
    Some(match prefix {
        b'x' | b'X' => (16, &DIGITS16),
        b'b' | b'B' => (2, &DIGITS2),
        b'o' | b'O' => (8, &DIGITS8),
        _ => return None,
    })
}

/// Shift-accumulate a radix run into an `i128`, or `None` on overflow.
///
/// Rule §5.4(5): a per-base `match` accumulator rather than
/// `u64::from_str_radix` on a slice, which is both faster and `_`-aware.
fn eval_radix(radix: u32, run: &[u8]) -> Option<i128> {
    let base = i128::from(radix);
    let mut acc: i128 = 0;
    for &b in run {
        if b == b'_' {
            continue;
        }
        let digit = match (radix, b) {
            (_, b'0'..=b'9') => i128::from(u32::from(b - b'0')),
            (16, b'a'..=b'f') => i128::from(u32::from(b - b'a') + 10),
            (16, b'A'..=b'F') => i128::from(u32::from(b - b'A') + 10),
            _ => return None,
        };
        acc = acc.checked_mul(base)?.checked_add(digit)?;
    }
    Some(acc)
}

/// Shift-accumulate a decimal run into an `i128`, or `None` on overflow.
fn eval_decimal(run: &[u8]) -> Option<i128> {
    let mut acc: i128 = 0;
    for &b in run {
        if b == b'_' {
            continue;
        }
        let digit = i128::from(u32::from(b.wrapping_sub(b'0')));
        acc = acc.checked_mul(10)?.checked_add(digit)?;
    }
    Some(acc)
}

/// Value an integer literal, promoting to `float` when it leaves the i48 range.
///
/// Rule §5.4(4): `Int` overflow never wraps, never errors, and never produces a
/// wrong answer. `i64::MAX` becomes `9.223372036854776e18`, **not** `-1`.
#[inline]
fn finish_int(value: i128) -> NumLit {
    if value >= i128::from(I48_MIN) && value <= i128::from(I48_MAX) {
        NumLit::Int(value as i64)
    } else {
        NumLit::Float(value as f64)
    }
}

/// Value a radix run, promoting to `float` when it leaves the i48 range.
fn value_radix(radix: u32, run: &[u8]) -> NumLit {
    match eval_radix(radix, run) {
        Some(v) => finish_int(v),
        None => NumLit::Float(decimal_to_f64(run)),
    }
}

/// Value a decimal run, promoting to `float` when it leaves the i48 range.
fn value_decimal(run: &[u8]) -> NumLit {
    match eval_decimal(run) {
        Some(v) => finish_int(v),
        None => NumLit::Float(decimal_to_f64(run)),
    }
}

/// Decimal → binary float.
///
/// Fast path, used only where it provably rounds exactly once:
///
/// - `total_exp` negative and `-total_exp <= 15`: the decimal power `10^E` is
///   exactly representable (`10^15 < 2^53`), and if the mantissa also fits
///   2^53 the single IEEE division is correctly rounded.
/// - `total_exp >= 0`: `mantissa * 10^total_exp` is the exact integer value;
///   the `as f64` cast rounds exactly once, whatever its size.
///
/// Anything else (a mantissa too big to trust, or an exponent past the exact
/// powers) hands off to `str::parse`, the correctness oracle — which is also
/// what the property tests compare against.
pub fn decimal_to_f64(bytes: &[u8]) -> f64 {
    let mut i = 0usize;
    let mut negative = false;
    match bytes.first() {
        Some(b'-') => {
            negative = true;
            i = 1;
        }
        Some(b'+') => i = 1,
        _ => {}
    }

    let mut mantissa: i128 = 0;
    let mut mantissa_exact = true;
    let mut frac_exp: i64 = 0;
    let mut saw_digit = false;
    let mut past_dot = false;
    let mut exponent: Option<i64> = None;

    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'_' => {}
            b'.' => past_dot = true,
            b'0'..=b'9' => {
                saw_digit = true;
                let digit = i128::from(b - b'0');
                if mantissa_exact {
                    match mantissa.checked_mul(10).and_then(|m| m.checked_add(digit)) {
                        Some(m) => mantissa = m,
                        None => {
                            // Only the exponent continues to matter; the fast
                            // path is already off the table.
                            mantissa_exact = false;
                            if !past_dot {
                                frac_exp = frac_exp.saturating_add(1);
                            }
                        }
                    }
                } else if !past_dot {
                    frac_exp = frac_exp.saturating_add(1);
                }
                if past_dot {
                    frac_exp = frac_exp.saturating_sub(1);
                }
            }
            b'e' | b'E' => {
                i += 1;
                let mut exp_sign: i64 = 1;
                match bytes.get(i) {
                    Some(b'-') => {
                        exp_sign = -1;
                        i += 1;
                    }
                    Some(b'+') => i += 1,
                    _ => {}
                }
                let digits_start = i;
                let mut value: i64 = 0;
                while let Some(&d) = bytes.get(i) {
                    if d.is_ascii_digit() {
                        value = value.saturating_mul(10).saturating_add(i64::from(d - b'0'));
                        i += 1;
                    } else {
                        break;
                    }
                }
                if i == digits_start {
                    // No exponent digits at all: not a float we can value.
                    return float_from_str(bytes);
                }
                exponent = Some(exp_sign.saturating_mul(value));
                continue;
            }
            _ => return float_from_str(bytes),
        }
        i += 1;
    }

    if !saw_digit {
        return f64::NAN;
    }

    let total_exp = match exponent {
        Some(e) => frac_exp.saturating_add(e),
        None => frac_exp,
    };

    if mantissa_exact {
        let fits_f53 = mantissa.abs() <= (1i128 << 53);
        let fast = if total_exp < 0 {
            let e = -total_exp;
            fits_f53 && e <= 15
        } else {
            total_exp <= 308 && scale10(mantissa, total_exp).is_some()
        };
        if fast {
            let v = if total_exp < 0 {
                let e = i32::try_from(-total_exp).unwrap_or(0);
                mantissa as f64 / 10f64.powi(e)
            } else {
                scale10(mantissa, total_exp).unwrap_or(mantissa) as f64
            };
            return if negative { -v } else { v };
        }
    }

    // Anything too big for the exact paths hands off to the oracle, which
    // already reads the leading sign from the bytes itself.
    float_from_str(bytes)
}

/// `m * 10^e`, exactly, or `None` on overflow.
fn scale10(m: i128, e: i64) -> Option<i128> {
    if e < 0 {
        return None;
    }
    let mut acc = m;
    for _ in 0..e {
        acc = acc.checked_mul(10)?;
    }
    Some(acc)
}

fn float_from_str(bytes: &[u8]) -> f64 {
    // `str::parse` is the correctness oracle, but it rejects `_` separators,
    // so take them out first.
    let compact = if bytes.contains(&b'_') {
        let mut out = Vec::with_capacity(bytes.len());
        out.extend(bytes.iter().copied().filter(|&b| b != b'_'));
        String::from_utf8(out).ok()
    } else {
        std::str::from_utf8(bytes).ok().map(str::to_owned)
    };
    let Some(text) = compact else {
        return f64::NAN;
    };
    text.parse::<f64>().unwrap_or(f64::NAN)
}

/// Scan one number literal.
///
/// Precondition: `src[i]` is an ASCII digit, or `src[i] == b'.'` with
/// `src[i + 1]` a digit. A bare `.` never reaches here — `class_of` calls it an
/// operator and the main loop handles it.
/// Cap on salvage rescan depth, so a radix-prefix error (`0x` with no digits)
/// cannot recurse through `scan_inner` with unchanged bounds forever.
const SALVAGE_DEPTH: u32 = 8;

pub fn scan_number(src: &[u8], start: usize) -> NumScan {
    scan_inner(src, start, SALVAGE_DEPTH)
}

fn scan_inner(src: &[u8], start: usize, depth: u32) -> NumScan {
    let mut i = start;

    // ── S1. radix prefix ─────────────────────────────────────────────────
    if src.get(i) == Some(&b'0') {
        if let Some(&p) = src.get(i + 1) {
            if let Some((radix, set)) = radix_and_set(p) {
                i += 2;
                // N4: a separator directly after the prefix, checked *before*
                // the digit run so `0x_FF` cannot slip through.
                if src.get(i) == Some(&b'_') {
                    return NumScan::Err {
                        err_pos: i,
                        reason: NumErr::BadSeparator,
                        ok_end: salvage_ok(src, start, i, depth),
                    };
                }
                let d0 = i;
                i = eat_digits(src, i, set);
                if i == d0 {
                    // `0x` on its own is missing digits entirely, but `0b2`
                    // is a digit for some other base — N6 still applies.
                    let reason = match src.get(i) {
                        None => NumErr::NoDigitsAfterPrefix,
                        Some(&bad) if bad.is_ascii_alphanumeric() => NumErr::InvalidDigit,
                        Some(_) => NumErr::NoDigitsAfterPrefix,
                    };
                    return NumScan::Err {
                        err_pos: i,
                        reason,
                        ok_end: salvage_ok(src, start, i, depth),
                    };
                }
                if bad_separator(src.get(d0..i).unwrap_or(&[])) {
                    return NumScan::Err {
                        err_pos: i,
                        reason: NumErr::BadSeparator,
                        ok_end: salvage_ok(src, start, i, depth),
                    };
                }
                // N6: the digit set of the base is enforced, so `0b2` fails here.
                if let Some(&bad) = src.get(i) {
                    let idx = usize::from(bad);
                    let in_set = set.get(idx).copied().unwrap_or(0) != 0;
                    if bad == b'_' || (bad.is_ascii_alphanumeric() && !in_set) {
                        return NumScan::Err {
                            err_pos: i,
                            reason: NumErr::InvalidDigit,
                            ok_end: salvage_ok(src, start, i, depth),
                        };
                    }
                }
                // N2 applies to radix literals too: `0xFFz` is a mistake.
                if src
                    .get(i)
                    .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
                {
                    return NumScan::Err {
                        err_pos: i,
                        reason: NumErr::GluedIdentifier,
                        ok_end: salvage_ok(src, start, i, depth),
                    };
                }
                return NumScan::ok(i, value_radix(radix, src.get(d0..i).unwrap_or(&[])));
            }
        }
    }

    // ── S2. integer part (empty only when the literal starts with `.`) ───
    let int_start = i;
    i = eat_digits(src, i, &DIGITS10);
    let int_end = i;
    if bad_separator(src.get(int_start..int_end).unwrap_or(&[])) {
        return NumScan::Err {
            err_pos: int_end,
            reason: NumErr::BadSeparator,
            ok_end: 0,
        };
    }

    // ── S3. fraction ─────────────────────────────────────────────────────
    // The dot is taken whenever it is *not* the start of `..`, and the digits
    // after it may be empty. Both consequences are required by tests:
    //   `5.`   → Float(5.0)          (T-N6)
    //   `1..3` → Int(1) DotDot Int(3) (T-N25)
    let mut is_float = false;
    if src.get(i) == Some(&b'.') && src.get(i + 1) != Some(&b'.') {
        is_float = true;
        i += 1;
        let frac_start = i;
        i = eat_digits(src, i, &DIGITS10);
        if bad_separator(src.get(frac_start..i).unwrap_or(&[])) {
            return NumScan::Err {
                err_pos: i,
                reason: NumErr::BadSeparator,
                ok_end: 0,
            };
        }
    }

    // ── S4. exponent ─────────────────────────────────────────────────────
    if matches!(src.get(i), Some(b'e' | b'E')) {
        is_float = true;
        let mut j = i + 1;
        if matches!(src.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let d0 = j;
        j = eat_digits(src, j, &DIGITS10);
        if j == d0 {
            // N1, deliberate strictness: `1e` is an error here rather than
            // silently becoming Int(1) + Ident("e").
            return NumScan::Err {
                err_pos: i,
                reason: NumErr::EmptyExponent,
                ok_end: salvage_ok(src, start, i, depth),
            };
        }
        if bad_separator(src.get(d0..j).unwrap_or(&[])) {
            return NumScan::Err {
                err_pos: j,
                reason: NumErr::BadSeparator,
                ok_end: salvage_ok(src, start, i, depth),
            };
        }
        i = j;
    }

    // ── S5. reject a second fraction ─────────────────────────────────────
    if src.get(i) == Some(&b'.') && src.get(i + 1).is_some_and(|b| b.is_ascii_digit()) {
        return NumScan::Err {
            err_pos: i,
            reason: NumErr::SecondDot,
            ok_end: salvage_ok(src, start, i, depth),
        };
    }

    // ── S6/S7. trailing separator, or a digit run glued to an identifier ──
    // The leading `_` of the *whole* literal is re-checked here, because a
    // literal that starts with a dot skips S2's per-run check entirely
    // (`_0.5` must be a `BadSeparator`, not a float).
    if src.get(i) == Some(&b'_') || src.get(start) == Some(&b'_') {
        return NumScan::Err {
            err_pos: i,
            reason: NumErr::BadSeparator,
            ok_end: 0,
        };
    }
    if src.get(i).is_some_and(|b| b.is_ascii_alphabetic()) {
        return NumScan::Err {
            err_pos: i,
            reason: NumErr::GluedIdentifier,
            ok_end: salvage_ok(src, start, i, depth),
        };
    }

    let bytes = src.get(start..i).unwrap_or(&[]);
    let lit = if is_float {
        NumLit::Float(decimal_to_f64(bytes))
    } else {
        value_decimal(bytes)
    };
    NumScan::ok(i, lit)
}

/// When `src[start..end]` is itself a complete, valid literal, return `end` so
/// the caller can emit it and resume scanning at the failure point. This is what
/// makes `1.2.3` recover as `Float(1.2)` then `Float(0.3)` with one error.
///
/// The rescan runs at `depth - 1`, so a prefix that errors *again at the same
/// boundary* (a bare `0x`, say) terminates: either it finds a salvage point
/// strictly inside, or `depth` runs out and the prefix is treated as
/// unsalvageable.
fn salvage_ok(src: &[u8], start: usize, end: usize, depth: u32) -> usize {
    if end <= start || depth == 0 {
        return 0;
    }
    let prefix = src.get(start..end).unwrap_or(&[]);
    match scan_inner(prefix, 0, depth - 1) {
        NumScan::Ok { end: e, .. } if e == prefix.len() => end,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(text: &str) -> NumScan {
        // Leak so the returned borrow is 'static, keeping the tests readable.
        let leaked: &'static [u8] = Box::leak(text.as_bytes().to_vec().into_boxed_slice());
        scan_number(leaked, 0)
    }

    fn ok(text: &str) -> NumLit {
        match scan(text) {
            NumScan::Ok { lit, end } => {
                assert_eq!(end, text.len(), "{text}: consumed the wrong span");
                lit
            }
            NumScan::Err { reason, .. } => panic!("{text}: unexpected error {reason:?}"),
        }
    }

    fn err(text: &str) -> NumErr {
        match scan(text) {
            NumScan::Ok { lit, .. } => panic!("{text}: expected an error, got {lit:?}"),
            NumScan::Err { reason, .. } => reason,
        }
    }

    #[test]
    fn integers() {
        assert_eq!(ok("0"), NumLit::Int(0));
        assert_eq!(ok("42"), NumLit::Int(42));
        assert_eq!(ok("010"), NumLit::Int(10), "N10: no leading-zero octal");
        assert_eq!(ok("1_000_000"), NumLit::Int(1_000_000));
        assert_eq!(ok("0xff"), NumLit::Int(255));
        assert_eq!(ok("0XFF"), NumLit::Int(255));
        assert_eq!(ok("0xFF_FF"), NumLit::Int(65535));
        assert_eq!(ok("0b1010_1010"), NumLit::Int(170));
        assert_eq!(ok("0B1010"), NumLit::Int(10));
        assert_eq!(ok("0o755"), NumLit::Int(493));
        assert_eq!(ok("0O755"), NumLit::Int(493));
    }

    #[test]
    fn floats() {
        assert_eq!(
            ok("3.14"),
            NumLit::Float(f64::from_bits(0x4009_1EB8_51EB_851F))
        );
        assert_eq!(ok(".5"), NumLit::Float(0.5));
        assert_eq!(ok("5."), NumLit::Float(5.0), "T-N6");
        assert_eq!(
            ok("1e10"),
            NumLit::Float(1e10),
            "T-N7: exponent means Float"
        );
        assert_eq!(ok("1E-10"), NumLit::Float(1e-10));
        assert_eq!(ok("2.5E+3"), NumLit::Float(2500.0));
        assert_eq!(ok("1_000.5"), NumLit::Float(1000.5));
    }

    #[test]
    fn range_is_not_a_second_dot() {
        // T-N25: `..` is a range, so the scan stops after the `1`.
        assert_eq!(scan("1..3"), NumScan::ok(1, NumLit::Int(1)));
    }

    #[test]
    fn i48_boundaries_promote_rather_than_wrap() {
        assert_eq!(ok("140737488355326"), NumLit::Int(140737488355326));
        assert_eq!(ok("140737488355327"), NumLit::Int(I48_MAX));
        assert_eq!(ok("140737488355328"), NumLit::Float(140737488355328.0));
        assert_eq!(ok("0x7FFFFFFFFFFF"), NumLit::Int(I48_MAX));
        assert_eq!(ok("0x800000000000"), NumLit::Float(140737488355328.0));
        // T-N19: i64::MAX must NOT become -1.
        assert_eq!(
            ok("9223372036854775807"),
            NumLit::Float(9.223372036854776e18)
        );
        assert_eq!(ok("281474976710655"), NumLit::Float(281474976710655.0));
    }

    #[test]
    fn validity_rules() {
        assert_eq!(err("1.2.3"), NumErr::SecondDot);
        assert_eq!(err("1abc"), NumErr::GluedIdentifier);
        assert_eq!(err("1e"), NumErr::EmptyExponent);
        assert_eq!(err("1_"), NumErr::BadSeparator);
        assert_eq!(err("1__0"), NumErr::BadSeparator);
        assert_eq!(err("0x"), NumErr::NoDigitsAfterPrefix);
        assert_eq!(err("0b2"), NumErr::InvalidDigit);
        assert_eq!(err("0x_FF"), NumErr::BadSeparator);
        assert_eq!(err("0xFF_"), NumErr::BadSeparator);
        assert_eq!(err("5.foo"), NumErr::GluedIdentifier);
        assert_eq!(err("1e_1"), NumErr::BadSeparator);
        assert_eq!(err("_0.5"), NumErr::BadSeparator);
    }

    #[test]
    fn multi_dot_recovers_with_one_prefix() {
        // T-N23: the prefix `1.2` is a complete literal, so the caller can emit
        // it and rescan from the second dot.
        match scan("1.2.3") {
            NumScan::Err {
                err_pos,
                reason,
                ok_end,
            } => {
                assert_eq!(reason, NumErr::SecondDot);
                assert_eq!(err_pos, 3);
                assert_eq!(ok_end, 3, "the valid prefix ends where the error is");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn member_access_on_a_float_is_fine() {
        // T-N24: `1.2.real` — the byte after the `.` is `r`, not a digit.
        assert_eq!(scan("1.2"), NumScan::ok(3, NumLit::Float(1.2)));
    }

    #[test]
    fn fast_and_slow_float_paths_agree() {
        // T-N30, without a proptest dependency: a deterministic sweep across
        // the fast-path boundary and a set of hard-to-round values.
        let cases = [
            "0.0",
            "1.0",
            "0.1",
            "0.2",
            "0.3",
            "1.5",
            "2.5",
            "3.14",
            "9.99",
            "1e10",
            "1e-10",
            "2.5E+3",
            "1_000.5",
            "5.",
            ".5",
            "1234567890.123456789",
            "1e22",
            "1e23",
            "1e-23",
            "9007199254740993.0",
            "0.000000000000001",
            "123456789.123456789",
            "1e300",
            "1e-300",
        ];
        for text in cases {
            let bytes = text.as_bytes();
            let fast = decimal_to_f64(bytes);
            let clean: Vec<u8> = bytes.iter().copied().filter(|&b| b != b'_').collect();
            let slow = String::from_utf8(clean)
                .unwrap_or_default()
                .parse::<f64>()
                .unwrap_or(f64::NAN);
            assert_eq!(
                fast.to_bits(),
                slow.to_bits(),
                "{text}: fast {fast} != slow {slow}"
            );
        }
    }

    #[test]
    fn float_agrees_with_parse_over_a_wide_sweep() {
        // Same property, 20k generated literals, so the fast/slow boundary in
        // decimal_to_f64 is genuinely exercised rather than just sampled.
        let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..20_000 {
            let whole = next() % 1_000_000;
            let frac = next() % 1_000_000;
            let exp = (next() % 41) as i32 - 20;
            let sign = if next() & 1 == 0 { "" } else { "-" };
            let text = format!("{sign}{whole}.{frac}e{exp}");
            let fast = decimal_to_f64(text.as_bytes());
            let slow = text.parse::<f64>().unwrap_or(f64::NAN);
            assert_eq!(fast.to_bits(), slow.to_bits(), "{text}");
        }
    }
}
