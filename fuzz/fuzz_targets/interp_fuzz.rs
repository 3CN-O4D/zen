//! M3 fuzz target: arbitrary text is lexed, parsed and evaluated by the
//! tree-walking interpreter. Any input must either be rejected or run to
//! completion under the evaluation budget — never panic, never hang.
//!
//! Run with `cargo +nightly fuzz run interp_fuzz`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use zen_interp::Interp;

/// Evaluations allowed per input. Enough to exercise real control flow, small
/// enough that a non-terminating program (`while true {}`) cannot hang the run.
const BUDGET: u64 = 100_000;
/// Skip inputs larger than this; keeps each iteration fast and bounds allocation.
const MAX_SRC: usize = 4096;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    if src.len() > MAX_SRC {
        return;
    }
    let mut interp = Interp::new();
    interp.set_step_budget(BUDGET);
    let _ = interp.run_source(src);
});