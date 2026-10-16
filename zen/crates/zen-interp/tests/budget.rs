//! Step-budget tests: a bound must stop a non-terminating program without
//! panicking or hanging, while leaving ordinary programs unaffected.

use zen_interp::Interp;

#[test]
fn step_budget_stops_infinite_loop() {
    let mut interp = Interp::new();
    interp.set_step_budget(10_000);
    let err = interp
        .run_source("var x = 0\nwhile true { x = x + 1 }\n")
        .err()
        .expect("infinite loop should exhaust the budget");
    assert!(err.contains("step budget"), "unexpected error: {err}");
}

#[test]
fn step_budget_does_not_trip_on_short_program() {
    let mut interp = Interp::new();
    interp.set_step_budget(100_000);
    interp
        .run_source("var x = 1\nx = x + 1\n")
        .expect("short program should finish within budget");
}
