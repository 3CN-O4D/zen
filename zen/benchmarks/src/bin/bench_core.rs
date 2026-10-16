//! `bench_core`: the M4/M5 speed gate (`docs/08-vm.md` §12).
//!
//! Runs the `bench_core` group from `docs/08-vm.md:1736` in the Zen VM and in
//! CPython 3.13, and reports the per-benchmark ratio plus the aggregate
//! geometric mean. The gate is: aggregate ≥ 1.5× and **every** individual
//! ratio ≥ 1.0× (Zen never slower).
//!
//! The Zen side is timed in-process (VM execution only, compilation excluded),
//! matching the protocol's `./zen/bench_core` invocation which excludes
//! compile/startup. The CPython side is timed as a whole `python3` process, but
//! a measured startup baseline is subtracted so the comparison is
//! execution-only on both sides. Large iteration counts keep startup small.
//!
//! `bench_json_parse` and `bench_startup` are reported as N/A: the first needs
//! the M6 Rust JSON parser, the second measures process exec + dynamic linking,
//! which this in-process harness cannot represent.

use std::process::Command;
use std::time::{Duration, Instant};

use zen_compiler::compile_source;
use zen_vm::{RefCountGc, Vm};

/// One benchmark: identical work in Zen and CPython.
struct Bench {
    name: &'static str,
    zen: &'static str,
    py: &'static str,
    /// `docs/08-vm.md` §12.3 target ratio vs CPython.
    target: f64,
}

const BENCHES: &[Bench] = &[
    Bench {
        name: "bench_loop_sum",
        target: 8.0,
        zen: r#"
var s = 0
var i = 0
while i < 1000000 {
    s = s + i
    i = i + 1
}
s
"#,
        py: r#"
s = 0
i = 0
while i < 1000000:
    s = s + i
    i = i + 1
s
"#,
    },
    Bench {
        name: "bench_recursion",
        target: 10.0,
        zen: r#"
func fib(n) {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}
fib(30)
"#,
        py: r#"
def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)
fib(30)
"#,
    },
    Bench {
        name: "bench_list_ops",
        target: 3.0,
        zen: r#"
var l = []
var i = 0
while i < 200000 {
    l.push(i)
    i = i + 1
}
var s = 0
var j = 0
while j < 200000 {
    s = s + l[j]
    j = j + 1
}
s
"#,
        py: r#"
l = []
i = 0
while i < 200000:
    l.append(i)
    i = i + 1
s = 0
j = 0
while j < 200000:
    s = s + l[j]
    j = j + 1
s
"#,
    },
    Bench {
        name: "bench_dict_ops",
        target: 1.8,
        zen: r#"
var d = {}
var i = 0
while i < 50000 {
    d[i] = i
    i = i + 1
}
var s = 0
var j = 0
while j < 50000 {
    s = s + d[j]
    j = j + 1
}
s
"#,
        py: r#"
d = {}
i = 0
while i < 50000:
    d[i] = i
    i = i + 1
s = 0
j = 0
while j < 50000:
    s = s + d[j]
    j = j + 1
s
"#,
    },
    Bench {
        name: "bench_field_access",
        target: 6.0,
        zen: r#"
class P {
    func init() {
        self.x = 1
        self.y = 2
    }
}
var p = new P()
var s = 0
var i = 0
while i < 1000000 {
    s = s + p.x + p.y
    i = i + 1
}
s
"#,
        py: r#"
class P:
    def __init__(self):
        self.x = 1
        self.y = 2
p = P()
s = 0
i = 0
while i < 1000000:
    s = s + p.x + p.y
    i = i + 1
s
"#,
    },
    Bench {
        name: "bench_method_call",
        target: 3.0,
        zen: r#"
class Counter {
    func init() {
        self.n = 0
    }
    func inc() {
        self.n = self.n + 1
        return self.n
    }
}
var c = new Counter()
var i = 0
while i < 500000 {
    c.inc()
    i = i + 1
}
i
"#,
        py: r#"
class Counter:
    def __init__(self):
        self.n = 0
    def inc(self):
        self.n = self.n + 1
        return self.n
c = Counter()
i = 0
while i < 500000:
    c.inc()
    i = i + 1
i
"#,
    },
    Bench {
        name: "bench_closure_alloc",
        target: 1.5,
        zen: r#"
func make(x) {
    return func() { return x }
}
var s = 0
var i = 0
while i < 200000 {
    var f = make(i)
    s = s + f()
    i = i + 1
}
s
"#,
        py: r#"
def make(x):
    return lambda: x
s = 0
i = 0
while i < 200000:
    f = make(i)
    s = s + f()
    i = i + 1
s
"#,
    },
    Bench {
        name: "bench_string_build",
        target: 1.0,
        zen: r#"
var s = ""
var i = 0
while i < 5000 {
    s = s + "x"
    i = i + 1
}
s.length()
"#,
        py: r#"
s = ""
i = 0
while i < 5000:
    s = s + "x"
    i = i + 1
len(s)
"#,
    },
    Bench {
        name: "bench_iteration",
        target: 3.0,
        zen: r#"
var s = 0
for v in range(1000000) {
    s = s + v
}
s
"#,
        py: r#"
s = 0
for v in range(1000000):
    s = s + v
s
"#,
    },
];

/// Time Zen in-process: compile once, then best-of-N VM executions.
fn time_zen(src: &str, reps: usize) -> Duration {
    let module = compile_source(src).expect("benchmark must compile");
    let mut best = Duration::MAX;
    for _ in 0..reps {
        let mut vm = Vm::new(Box::new(RefCountGc));
        let start = Instant::now();
        vm.run_module(&module.main, &module.children)
            .expect("benchmark must run");
        best = best.min(start.elapsed());
    }
    best
}

/// Time Zen steady-state with the JIT attached (M11 protocol): one VM and one
/// background pool across reps, so rep 1 warms (tier-up + compile) and later
/// reps run hot. Only used with `--features jit` and `ZEN_JIT=1`; otherwise
/// `time_zen` above is the measurement.
#[cfg(feature = "jit")]
fn time_zen_jit(src: &str, reps: usize) -> Duration {
    let module = compile_source(src).expect("benchmark must compile");
    let mut vm = Vm::new(Box::new(RefCountGc));
    vm.jit = zen_jit::JitHandle::start(2, || Box::new(zen_jit::CraneliftCompiler::new()));
    let mut best = Duration::MAX;
    for _ in 0..reps {
        let start = Instant::now();
        vm.run_module(&module.main, &module.children)
            .expect("benchmark must run");
        best = best.min(start.elapsed());
    }
    best
}

/// Time CPython: best-of-N runs, timed *in-process* so interpreter startup
/// never pollutes the comparison (the old whole-process minus baseline math
/// saturated to zero on fast benches like `string_build`). The script runs
/// under `exec` with a `perf_counter` around it; bench scripts are bare
/// statement sequences, so this is exact.
fn time_python(name: &str, src: &str, reps: usize) -> Duration {
    let inner = format!("/tmp/opencode/bench_{name}_inner.py");
    std::fs::write(&inner, src).expect("write python script");
    let wrapper = format!(
        "import time;t=time.perf_counter();exec(open({inner:?}).read());print((time.perf_counter()-t)*1000.0)"
    );
    let mut best = Duration::MAX;
    for _ in 0..reps {
        let out = Command::new("python3")
            .arg("-c")
            .arg(&wrapper)
            .output()
            .expect("python3 must be runnable");
        assert!(
            out.status.success(),
            "python benchmark {name} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let ms: f64 = String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .expect("python must print milliseconds");
        best = best.min(Duration::from_secs_f64(ms / 1000.0));
    }
    best
}

fn main() {
    let reps: usize = std::env::var("REPS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    println!(
        "{:<22} {:>10} {:>10} {:>8} {:>7}",
        "benchmark", "zen(ms)", "py(ms)", "ratio", "target"
    );

    let mut log_sum = 0.0f64;
    let mut counted = 0u32;
    let mut all_fast = true;
    let mut worst = f64::INFINITY;

    for b in BENCHES {
        // Warm up both sides once so lazy IC/shape and caches populate.
        let _ = time_zen(b.zen, 1);
        let _ = time_python(b.name, b.py, 1);

        #[cfg(feature = "jit")]
        let jit_on = std::env::var_os("ZEN_JIT").is_some();
        #[cfg(not(feature = "jit"))]
        let jit_on = false;
        #[cfg(feature = "jit")]
        let zen = if jit_on {
            time_zen_jit(b.zen, reps)
        } else {
            time_zen(b.zen, reps)
        };
        #[cfg(not(feature = "jit"))]
        let zen = time_zen(b.zen, reps);
        let py = time_python(b.name, b.py, reps);

        let ratio = py.as_secs_f64() / zen.as_secs_f64();
        let ok = ratio >= 1.0;
        all_fast &= ok;
        worst = worst.min(ratio);
        log_sum += ratio.ln();
        counted += 1;

        println!(
            "{:<22} {:>10.1} {:>10.1} {:>7.2}x {:>6.1}x  {}",
            b.name,
            zen.as_secs_f64() * 1e3,
            py.as_secs_f64() * 1e3,
            ratio,
            b.target,
            if ok { "" } else { "SLOWER" }
        );
        let _ = std::io::Write::flush(&mut std::io::stdout());
    }

    let geomean = (log_sum / counted as f64).exp();
    println!(
        "{:<22} aggregate geomean: {geomean:.2}x (gate >= 1.5x)  worst: {worst:.2}x",
        ""
    );
    if geomean < 1.5 || !all_fast {
        println!(
            "FAIL: {}",
            if !all_fast {
                "an individual benchmark is slower than CPython"
            } else {
                "aggregate below 1.5x"
            }
        );
        std::process::exit(1);
    }
    println!("PASS");
}
