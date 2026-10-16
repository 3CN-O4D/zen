//! End-to-end: compile Zen source, run it on the VM with the Cranelift JIT
//! attached, and check the result matches the interpreter. A counting wrapper
//! proves the hot function was actually compiled.
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

/// The pool compiles asynchronously; wait until it has compiled at least once.
fn wait_compiled(counter: &AtomicUsize) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while counter.load(Ordering::SeqCst) == 0 {
        assert!(Instant::now() < deadline, "jit never compiled anything");
        std::thread::sleep(Duration::from_millis(1));
    }
}

const FIB: &str = r#"
func fib(n) {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}
return fib(24)
"#;

#[test]
fn jit_fib_matches_interpreter() {
    let plain = run(FIB, None);
    let counter = Arc::new(AtomicUsize::new(0));
    let jitted = run(FIB, Some(&counter));
    assert_eq!(plain.as_int(), Some(46368));
    assert_eq!(jitted.as_int(), plain.as_int());
    wait_compiled(&counter);
    assert!(
        counter.load(Ordering::SeqCst) >= 1,
        "fib should have been compiled"
    );
}

// `addf` operates on floats; with the float-lowering path these operations run
// natively in the JIT. The result must still match the interpreter exactly.
const FLOAT_DEOPT: &str = r#"
func addf(a, b) { return a + b }
var i = 0
var s = 0.0
while i < 3000 {
    s = addf(s, 0.5)
    i = i + 1
}
return s
"#;

#[test]
fn jit_handles_floats_natively() {
    let plain = run(FLOAT_DEOPT, None);
    let counter = Arc::new(AtomicUsize::new(0));
    let jitted = run(FLOAT_DEOPT, Some(&counter));
    assert_eq!(plain.as_float(), Some(1500.0));
    assert_eq!(jitted.as_float(), plain.as_float());
    wait_compiled(&counter);
    assert!(
        counter.load(Ordering::SeqCst) >= 1,
        "addf should have been compiled"
    );
}
