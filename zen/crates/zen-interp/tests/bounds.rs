//! Allocation-bound tests: a program that would allocate an absurd string must
//! return a catchable error, not abort the process.

use zen_interp::Interp;

#[test]
fn oversized_string_repetition_errors() {
    let mut interp = Interp::new();
    let err = interp
        .run_source("var s = \"x\"\nvar n = 1000000000\nprint(s * n)\n")
        .err()
        .expect("oversized repetition should error");
    assert!(err.contains("ValueError"), "unexpected error: {err}");
}

#[test]
fn oversized_zfill_errors() {
    let mut interp = Interp::new();
    let err = interp
        .run_source("var s = \"x\"\nvar n = 1000000000\nprint(s.zfill(n))\n")
        .err()
        .expect("oversized zfill should error");
    assert!(err.contains("ValueError"), "unexpected error: {err}");
}
