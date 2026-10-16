//! Differential tests for Stage 4 OSR: a function called once with a long loop
//! must tier up through loop-back-edge hotness, transfer into native code
//! mid-loop, and agree with the interpreter bit-for-bit — including when a
//! guard fires mid-loop and execution resumes in the interpreter.
#![cfg(feature = "jit")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use zen_compiler::compile_source;
use zen_jit::{CraneliftCompiler, JitHandle, ZenJitCompiler};
use zen_vm::{RefCountGc, Value, Vm};

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

struct Outcome {
    value: Value,
    compiles: usize,
    osr_transfers: u64,
}

fn run(src: &str, jit: Option<&Arc<AtomicUsize>>) -> Outcome {
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
    let value = vm
        .run_module(&module.main, &module.children)
        .unwrap_or_else(|e| panic!("runtime error: {e}\n{src}"));
    Outcome {
        value,
        compiles: jit.map(|c| c.load(Ordering::SeqCst)).unwrap_or(0),
        osr_transfers: vm.osr_transfers,
    }
}

/// A single call with a long loop: call-count tier-up cannot fire (one call),
/// so any compilation + transfer must come from loop-back-edge hotness (OSR).
const SUM_LOOP: &str = r#"
func sum(n) {
    var s = 0
    var i = 0
    while i < n {
        s = s + i
        i = i + 1
    }
    return s
}
return sum(200000)
"#;

#[test]
fn osr_transfers_mid_loop_and_matches_interpreter() {
    let plain = run(SUM_LOOP, None);
    assert_eq!(plain.value.as_int(), Some(19999900000));
    let counter = Arc::new(AtomicUsize::new(0));
    let jitted = run(SUM_LOOP, Some(&counter));
    assert_eq!(
        format!("{:?}", jitted.value),
        format!("{:?}", plain.value),
        "OSR diverged from the interpreter"
    );
    assert!(
        jitted.compiles >= 1,
        "loop-hot function should have been compiled"
    );
    assert!(
        jitted.osr_transfers >= 1,
        "expected at least one mid-loop OSR transfer"
    );
}

/// A guard firing *after* the transfer must resume the interpreter at the
/// failing op, not restart or miscompute: the loop runs ints (specializing
/// the native code), then meets a float and must finish with float semantics.
const RESUME_LOOP: &str = r#"
func f(n) {
    var s = 0
    var i = 0
    while i < n {
        if i == 150000 {
            s = 0.5
        }
        s = s + i
        i = i + 1
    }
    return s
}
return f(200000)
"#;

#[test]
fn osr_deopt_resumes_and_matches_interpreter() {
    let plain = run(RESUME_LOOP, None);
    let counter = Arc::new(AtomicUsize::new(0));
    let jitted = run(RESUME_LOOP, Some(&counter));
    assert_eq!(
        format!("{:?}", jitted.value),
        format!("{:?}", plain.value),
        "post-OSR deopt resume diverged from the interpreter"
    );
    // The float must actually flow through the result (guards against a resume
    // that silently drops the loop state).
    assert!(format!("{:?}", plain.value).contains('.'));
    // Give the background worker a moment if compilation lagged the loop.
    let deadline = Instant::now() + Duration::from_secs(10);
    while counter.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
}
