//! Differential tests: the Cranelift JIT must agree with the interpreter for
//! float, mixed int/float, division, modulo and overflow-promotion arithmetic.
//! Each program runs its hot function through the JIT pool; reaching a
//! successful compilation is itself part of the assertion.
#![cfg(feature = "jit")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use zen_compiler::compile_source;
use zen_jit::{CraneliftCompiler, JitHandle, ZenJitCompiler};
use zen_vm::{RefCountGc, Value, Vm};

/// Wraps the Cranelift compiler and counts successful compilations.
struct Counting {
    inner: CraneliftCompiler,
    compiles: Arc<AtomicUsize>,
}

impl ZenJitCompiler for Counting {
    fn compile(
        &mut self,
        snap: &zen_vm::jit::CompileSnapshot,
    ) -> Result<zen_vm::jit::CompileOutput, String> {
        let r = self.inner.compile(snap);
        if r.is_ok() {
            self.compiles.fetch_add(1, Ordering::SeqCst);
        }
        r
    }
}

fn run(src: &str, jit: Option<&Arc<AtomicUsize>>) -> Value {
    let module = compile_source(src).unwrap_or_else(|e| panic!("compile failed: {e:?}\n{src}"));
    let mut vm = Vm::new(Box::new(RefCountGc));
    if let Some(counter) = jit {
        let counter = counter.clone();
        vm.jit = Some(
            JitHandle::start(1, move || {
                Box::new(Counting {
                    inner: CraneliftCompiler::new(),
                    compiles: counter.clone(),
                })
            })
            .expect("start jit pool"),
        );
    }
    vm.run_module(&module.main, &module.children)
        .unwrap_or_else(|e| panic!("runtime error: {e}\n{src}"))
}

/// Runs the program interpreted and jitted, asserts an exact match, and
/// requires that at least one function actually compiled (i.e. did not deopt
/// out of the JIT for the whole run).
fn assert_jit_matches(src: &str) -> Value {
    let plain = run(src, None);
    let counter = Arc::new(AtomicUsize::new(0));
    let jitted = run(src, Some(&counter));
    let deadline = Instant::now() + Duration::from_secs(10);
    while counter.load(Ordering::SeqCst) == 0 {
        assert!(Instant::now() < deadline, "jit never compiled anything\n{src}");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        format!("{jitted:?}"),
        format!("{plain:?}"),
        "JIT diverged from the interpreter\n{src}"
    );
    plain
}

const DIVMOD_FLOAT: &str = r#"
func imod(a, b) { return a % b }
func idiv(a, b) { return a / b }
var i = 0
var acc = 0.0
while i < 3000 {
    acc = acc + idiv(imod(i, 13), 2) + idiv(i, 4)
    i = i + 1
}
return acc
"#;

#[test]
fn jit_matches_interpreter_for_div_mod_float() {
    let v = assert_jit_matches(DIVMOD_FLOAT);
    assert!(v.as_float().is_some(), "expected a float result, got {v:?}");
}

const MUL_OVERFLOW: &str = r#"
func mul(a, b) { return a * b }
var i = 0
var sink = 0.0
while i < 3000 {
    sink = sink + (mul(100000000000000, 3) - 300000000000000.0)
    i = i + 1
}
return sink
"#;

#[test]
fn jit_matches_interpreter_for_overflow_promotion() {
    // `mul` leaves the 48-bit int range, so the interpreter promotes to float;
    // the JIT must produce the identical value.
    let v = assert_jit_matches(MUL_OVERFLOW);
    assert_eq!(v.as_float(), Some(0.0));
}

const MIXED_COMPARE: &str = r#"
func lt(a, b) { return a < b }
var i = 0
var t = 0
while i < 3000 {
    if lt(i, i + 0.5) { t = t + 1 }
    if lt(i + 0.5, i) { t = t + 100 }
    i = i + 1
}
return t
"#;

#[test]
fn jit_matches_interpreter_for_mixed_comparison() {
    let v = assert_jit_matches(MIXED_COMPARE);
    assert_eq!(v.as_int(), Some(3000));
}

const FLOAT_NEG: &str = r#"
func negate(a) { return -a }
var i = 0
var acc = 0.0
while i < 3000 {
    acc = acc + negate(0.25)
    i = i + 1
}
return acc
"#;

#[test]
fn jit_matches_interpreter_for_float_negation() {
    let v = assert_jit_matches(FLOAT_NEG);
    assert_eq!(v.as_float(), Some(-750.0));
}
