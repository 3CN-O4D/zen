//! M4 fuzz target: arbitrary text is compiled and executed by the register VM.
//! Any input must either fail to compile or run to completion under the
//! instruction budget — never panic, never hang.
//!
//! Run with `cargo +nightly fuzz run vm_fuzz`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use zen_compiler::compile_source;
use zen_vm::{RefCountGc, Vm};

/// Instructions allowed per input. Bounds non-terminating programs.
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
    let Ok(module) = compile_source(src) else {
        return;
    };
    let mut vm = Vm::new(Box::new(RefCountGc));
    vm.set_step_budget(Some(BUDGET));
    let _ = vm.run_module(&module.main, &module.children);
});