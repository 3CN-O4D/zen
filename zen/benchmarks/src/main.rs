//! Manual M5 gate harness: `bench_dict_lookup` and `bench_method_call`.
//!
//! Each benchmark runs a tight Zen loop both with inline caches enabled and
//! with them bypassed (`Vm::set_ic_enabled(false)`, the M4-style baseline), and
//! prints the throughput of each. The M5 gate is a ≥ 2× speed-up on both; the
//! process exits non-zero if either falls short.
//!
//! Run with `cargo run -p zen-benchmarks --release`.

use std::time::{Duration, Instant};

use zen_compiler::compile_source;
use zen_vm::{RefCountGc, Vm};

/// Field access on a monomorphic instance. The un-cached path (`ops::get_field`)
/// linearly scans the field dict by name, so a *wide* object makes the hidden
/// class offset's O(1) lookup show its worth. Each iteration reads the last of
/// sixteen fields ten times.
const DICT_LOOKUP: &str = r#"
class Wide {
    func init() {
        self.f0 = 0
        self.f1 = 0
        self.f2 = 0
        self.f3 = 0
        self.f4 = 0
        self.f5 = 0
        self.f6 = 0
        self.f7 = 0
        self.f8 = 0
        self.f9 = 0
        self.f10 = 0
        self.f11 = 0
        self.f12 = 0
        self.f13 = 0
        self.f14 = 0
        self.f15 = 7
    }
}
let p = new Wide()
let iters = 200_000
var s = 0
var i = 0
while i < iters {
    s = s + p.f15 + p.f15 + p.f15 + p.f15 + p.f15
    s = s + p.f15 + p.f15 + p.f15 + p.f15 + p.f15
    i = i + 1
}
s
"#;

/// Monomorphic method call: the call site is `CallMethod`. The class has many
/// methods, so the un-cached path (`find_class_method`) linearly scans the
/// method table while the call-site IC resolves in O(1). Ten calls per
/// iteration, each also reading and writing `self.n` through the field IC.
const METHOD_CALL: &str = r#"
class Counter {
    func init() {
        self.n = 0
    }
    func m0() { return 0 }
    func m1() { return 1 }
    func m2() { return 2 }
    func m3() { return 3 }
    func m4() { return 4 }
    func m5() { return 5 }
    func m6() { return 6 }
    func m7() { return 7 }
    func m8() { return 8 }
    func m9() { return 9 }
    func m10() { return 10 }
    func m11() { return 11 }
    func m12() { return 12 }
    func m13() { return 13 }
    func inc() {
        self.n = self.n + 1
        return self.n
    }
}
let c = new Counter()
let iters = 200_000
var i = 0
while i < iters {
    c.inc()
    c.inc()
    c.inc()
    c.inc()
    c.inc()
    c.inc()
    c.inc()
    c.inc()
    c.inc()
    c.inc()
    i = i + 1
}
i
"#;

/// Iterations each benchmark performs, parsed from the `let iters = …` line.
const ITERATIONS: u64 = 200_000;

/// Interleaved A/B timing: alternate the two modes so thermal drift and
/// scheduler noise affect both equally, keeping each best-of-N tight.
fn time_ab(src: &str, reps: usize) -> (Duration, Duration) {
    let module = compile_source(src).expect("benchmark source must compile");
    let run = |enabled: bool| {
        let mut vm = Vm::new(Box::new(RefCountGc));
        vm.set_ic_enabled(enabled);
        let start = Instant::now();
        vm.run_module(&module.main, &module.children)
            .expect("benchmark must run");
        start.elapsed()
    };
    run(true);
    run(false);
    let mut best_off = Duration::MAX;
    let mut best_on = Duration::MAX;
    for _ in 0..reps {
        best_on = best_on.min(run(true));
        best_off = best_off.min(run(false));
    }
    (best_off, best_on)
}

/// Run once and report `(duration, ic_hits, ic_misses)`.
fn run_with_stats(src: &str, ic_enabled: bool) -> (Duration, u64, u64) {
    let module = compile_source(src).expect("benchmark source must compile");
    let mut vm = Vm::new(Box::new(RefCountGc));
    vm.set_ic_enabled(ic_enabled);
    let start = Instant::now();
    vm.run_module(&module.main, &module.children)
        .expect("benchmark must run");
    let elapsed = start.elapsed();
    let (hits, misses) = vm.ic_stats();
    (elapsed, hits, misses)
}

fn main() {
    let reps = 15;
    let benchmarks = [
        ("bench_dict_lookup", DICT_LOOKUP),
        ("bench_method_call", METHOD_CALL),
    ];

    let mut all_pass = true;
    for (name, src) in benchmarks {
        let (no_ic, with_ic) = time_ab(src, reps);

        let (_, hits, misses) = run_with_stats(src, true);
        println!("  ic: {hits} hits / {misses} misses");

        let ns_ic = with_ic.as_secs_f64() * 1e9 / ITERATIONS as f64;
        let ns_no = no_ic.as_secs_f64() * 1e9 / ITERATIONS as f64;
        let speedup = no_ic.as_secs_f64() / with_ic.as_secs_f64();

        println!("{name}: {ns_no:.1} ns/op (no IC) -> {ns_ic:.1} ns/op (IC) = {speedup:.2}x");
        if speedup < 2.0 {
            println!("  FAIL: M5 gate requires >= 2.00x");
            all_pass = false;
        } else {
            println!("  PASS");
        }
    }

    if !all_pass {
        std::process::exit(1);
    }
}
