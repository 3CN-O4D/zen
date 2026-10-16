#![cfg(feature = "jit")]
//! Module-level (main) code in native execution: `SetGlobal`/`MakeDict`
//! `SetIndex`/`GetIndex` lowering lets loop-heavy mains tier up through OSR
//! instead of staying interpreted forever.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use zen_compiler::compile_source;
use zen_jit::{CraneliftCompiler, JitHandle, ZenJitCompiler};
use zen_vm::jit::CompileSnapshot;
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

fn snapshot_of(src: &str) -> CompileSnapshot {
    let module = compile_source(src).expect("compile");
    CompileSnapshot::from_proto(&module.main)
}

const GLOBAL_LOOP: &str = r#"
var s = 0
var i = 0
while i < 1000 {
    s = s + i
    i = i + 1
}
return s
"#;

#[test]
fn main_shaped_code_compiles_with_osr_entries() {
    let snap = snapshot_of(GLOBAL_LOOP);
    let out = CraneliftCompiler::new()
        .compile(&snap)
        .expect("global-loop main must be eligible");
    assert!(
        !out.osr.is_empty(),
        "a loop-heavy main needs OSR entries, got none"
    );
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
            JitHandle::start(2, move || {
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

/// Deterministic steady-state run: compile the main synchronously in the test
/// thread and publish it (mirroring the worker protocol), then run once with
/// no pool attached. `run_module` clones the main proto per call — a clone of
/// an initialized `OnceLock` shares the slot — so the published entries are
/// visible and the first loop back-edge transfers with no race against a
/// background compiler thread. Deopts and tier-up notes are `None`-safe
/// without a pool.
fn run_sync(src: &str) -> Outcome {
    use zen_vm::jit::{JitSlot, STATE_JITTED};
    let module = compile_source(src).unwrap_or_else(|e| panic!("compile failed: {e:?}\n{src}"));
    let slot = module
        .main
        .jit
        .get_or_init(|| Arc::new(JitSlot::new()))
        .clone();
    let snap = CompileSnapshot::from_proto(&module.main);
    let out = CraneliftCompiler::new()
        .compile(&snap)
        .unwrap_or_else(|e| panic!("sync compile failed: {e}\n{src}"));
    assert!(
        !out.osr.is_empty(),
        "a loop-heavy main needs OSR entries, got none"
    );
    slot.entry.store(out.entry as usize, Ordering::Release);
    if let Ok(mut m) = slot.osr.lock() {
        m.clear();
        m.extend(out.osr.iter().map(|(pc, a)| (*pc, *a as usize)));
    }
    slot.state.store(STATE_JITTED, Ordering::Release);
    let mut vm = Vm::new(Box::new(RefCountGc));
    // A pool must be attached for `osr_transfer` to fire at all (it bails
    // when `vm.jit` is `None`). It idles: nothing is ever queued because the
    // slot is already `JITTED`, so the run stays deterministic.
    vm.jit = JitHandle::start(2, || Box::new(CraneliftCompiler::new()));
    let value = vm
        .run_module(&module.main, &module.children)
        .unwrap_or_else(|e| panic!("runtime error: {e}\n{src}"));
    Outcome {
        value,
        compiles: 1,
        osr_transfers: vm.osr_transfers,
    }
}

const MAIN_LOOP: &str = r#"
var s = 0
var i = 0
while i < 200000 {
    s = s + i
    i = i + 1
}
return s
"#;

#[test]
fn main_loop_osrs_and_matches_interpreter() {
    let plain = run(MAIN_LOOP, None);
    assert_eq!(plain.value.as_int(), Some(19999900000));
    let jitted = run_sync(MAIN_LOOP);
    assert_eq!(
        format!("{:?}", jitted.value),
        format!("{:?}", plain.value),
        "native main loop diverged from the interpreter"
    );
    assert!(
        jitted.osr_transfers >= 1,
        "main loop should transfer into native code"
    );
}

const MAIN_DICT: &str = r#"
var d = {}
var i = 0
while i < 20000 {
    d[i] = i
    i = i + 1
}
var s = 0
var j = 0
while j < 20000 {
    s = s + d[j]
    j = j + 1
}
return s
"#;

#[test]
fn main_dict_loop_matches_interpreter() {
    let plain = run(MAIN_DICT, None);
    assert_eq!(plain.value.as_int(), Some(199990000));
    let jitted = run_sync(MAIN_DICT);
    assert_eq!(
        format!("{:?}", jitted.value),
        format!("{:?}", plain.value),
        "native dict main loop diverged from the interpreter"
    );
    assert!(
        jitted.osr_transfers >= 1,
        "dict main loop should transfer into native code"
    );
}

/// A class-definition prefix (`MakeClass` + zero-capture `MakeClosure` +
/// `AddMethod`) must not pin the main loop to the interpreter: the prefix
/// runs once through faithful bridges, then the field loop goes native.
const MAIN_FIELD: &str = r#"
class P {
    func init() {
        self.x = 1
        self.y = 2
    }
}
var p = new P()
var s = 0
var i = 0
while i < 20000 {
    s = s + p.x + p.y
    i = i + 1
}
return s
"#;

#[test]
fn main_field_loop_matches_interpreter() {
    let plain = run(MAIN_FIELD, None);
    assert_eq!(plain.value.as_int(), Some(60000));
    let jitted = run_sync(MAIN_FIELD);
    assert_eq!(
        format!("{:?}", jitted.value),
        format!("{:?}", plain.value),
        "native field main loop diverged from the interpreter"
    );
    assert!(
        jitted.compiles == 1,
        "field main should have been compiled once");
    assert!(
        jitted.osr_transfers >= 1,
        "field main loop should transfer into native code"
    );
}

/// A `CallMethod` loop body resolves through the native bridge (instance IC
/// path) instead of pinning the whole main to the interpreter.
const MAIN_METHOD: &str = r#"
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
while i < 20000 {
    c.inc()
    i = i + 1
}
return i
"#;

#[test]
fn main_method_loop_matches_interpreter() {
    let plain = run(MAIN_METHOD, None);
    assert_eq!(plain.value.as_int(), Some(20000));
    let jitted = run_sync(MAIN_METHOD);
    assert_eq!(
        format!("{:?}", jitted.value),
        format!("{:?}", plain.value),
        "native method main loop diverged from the interpreter"
    );
    assert!(
        jitted.compiles == 1,
        "method main should have been compiled once");
    assert!(
        jitted.osr_transfers >= 1,
        "method main loop should transfer into native code"
    );
}

/// `list.push` in a native loop takes the push fast lane (no per-iteration
/// deopt); the read loop uses the inlined index path.
const MAIN_LIST: &str = r#"
var l = []
var i = 0
while i < 20000 {
    l.push(i)
    i = i + 1
}
var s = 0
var j = 0
while j < 20000 {
    s = s + l[j]
    j = j + 1
}
return s
"#;

#[test]
fn main_list_loop_matches_interpreter() {
    let plain = run(MAIN_LIST, None);
    assert_eq!(plain.value.as_int(), Some(199990000));
    let jitted = run_sync(MAIN_LIST);
    assert_eq!(
        format!("{:?}", jitted.value),
        format!("{:?}", plain.value),
        "native list main loop diverged from the interpreter"
    );
    assert!(
        jitted.compiles == 1,
        "list main should have been compiled once");
    assert!(
        jitted.osr_transfers >= 1,
        "list main loop should transfer into native code"
    );
}

/// Iterator opcodes (`IterInit`/`IterNext`/`IterDone` over a `range`
/// `CallNative`) lower, so `for` loops tier up too.
const MAIN_ITER: &str = r#"
var s = 0
for v in range(20000) {
    s = s + v
}
return s
"#;

#[test]
fn main_iter_loop_matches_interpreter() {
    let plain = run(MAIN_ITER, None);
    assert_eq!(plain.value.as_int(), Some(199990000));
    let jitted = run_sync(MAIN_ITER);
    assert_eq!(
        format!("{:?}", jitted.value),
        format!("{:?}", plain.value),
        "native iterator main loop diverged from the interpreter"
    );
    assert!(
        jitted.compiles == 1,
        "iterator main should have been compiled once");
    assert!(
        jitted.osr_transfers >= 1,
        "iterator main loop should transfer into native code"
    );
}

/// Index fast paths: negative indices wrap once, tuples read inline, and an
/// out-of-bounds read deopts to the interpreter's `IndexError`.
const MAIN_INDEX_EDGE: &str = r#"
var l = [10, 20, 30]
var t = (1, 2)
var s = l[-1] + l[-3] + t[0] + t[1]
var i = 0
while i < 20000 {
    s = s + l[i % 3]
    i = i + 1
}
return s
"#;

#[test]
fn main_index_edge_cases_match_interpreter() {
    let plain = run(MAIN_INDEX_EDGE, None);
    assert_eq!(plain.value.as_int(), Some(30 + 10 + 1 + 2 + 399990));
    let jitted = run_sync(MAIN_INDEX_EDGE);
    assert_eq!(
        format!("{:?}", jitted.value),
        format!("{:?}", plain.value),
        "native index edge cases diverged from the interpreter"
    );
    assert!(
        jitted.osr_transfers >= 1,
        "index main loop should transfer into native code"
    );
}

const MAIN_INDEX_OOB: &str = r#"
var l = [1, 2]
var i = 0
while i < 20000 {
    i = i + 1
}
return l[5]
"#;

/// Read-only capture: `make`'s inner closure snapshots the parameter, so
/// `make` itself compiles (no bridge-and-resume tax per call) and the loop
/// runs fully native.
const MAIN_CLOSURE_RO: &str = r#"
func make(x) {
    return func() { return x }
}
var s = 0
var i = 0
while i < 20000 {
    var f = make(i)
    s = s + f()
    i = i + 1
}
return s
"#;

#[test]
fn main_closure_ro_matches_interpreter() {
    let plain = run(MAIN_CLOSURE_RO, None);
    assert_eq!(plain.value.as_int(), Some(199990000));
    let jitted = run_sync(MAIN_CLOSURE_RO);
    assert_eq!(
        format!("{:?}", jitted.value),
        format!("{:?}", plain.value),
        "native read-only closure loop diverged from the interpreter"
    );
    assert!(
        jitted.osr_transfers >= 1,
        "closure main loop should transfer into native code"
    );
}

/// Nested general calls through the per-site entry cache: `main -> add2 ->
/// id`, all native after tier-up, with the cache refreshing across the
/// nested frames.
const MAIN_NESTED_CALLS: &str = r#"
func id(x) { return x }
func add2(a, b) { return id(a) + id(b) }
var s = 0
var i = 0
while i < 20000 {
    s = s + add2(i, 1)
    i = i + 1
}
return s
"#;

#[test]
fn main_nested_calls_match_interpreter() {
    let plain = run(MAIN_NESTED_CALLS, None);
    assert_eq!(plain.value.as_int(), Some(200010000));
    let jitted = run_sync(MAIN_NESTED_CALLS);
    assert_eq!(
        format!("{:?}", jitted.value),
        format!("{:?}", plain.value),
        "native nested calls diverged from the interpreter"
    );
    assert!(
        jitted.osr_transfers >= 1,
        "nested-call main loop should transfer into native code"
    );
}

/// String building stays native through the `+` bridge (no per-iteration
/// deopt): appends link rope nodes instead of recopying the left side.
const MAIN_STRING: &str = r#"
var s = ""
var i = 0
while i < 5000 {
    s = s + "x"
    i = i + 1
}
return s.length()
"#;

#[test]
fn main_string_build_matches_interpreter() {
    let plain = run(MAIN_STRING, None);
    assert_eq!(plain.value.as_int(), Some(5000));
    let jitted = run_sync(MAIN_STRING);
    assert_eq!(
        format!("{:?}", jitted.value),
        format!("{:?}", plain.value),
        "native string build diverged from the interpreter"
    );
    assert!(
        jitted.osr_transfers >= 1,
        "string main loop should transfer into native code"
    );
}

#[test]
fn main_index_oob_raises_through_native() {
    // The loop transfers into native code first, so the trailing OOB read
    // executes natively, deopts, and the interpreter raises `IndexError`.
    let module = compile_source(MAIN_INDEX_OOB).expect("compile");
    let slot = module
        .main
        .jit
        .get_or_init(|| Arc::new(zen_vm::jit::JitSlot::new()))
        .clone();
    let snap = CompileSnapshot::from_proto(&module.main);
    let out = CraneliftCompiler::new().compile(&snap).expect("eligible");
    assert!(!out.osr.is_empty(), "loop main needs OSR entries");
    slot.entry.store(out.entry as usize, Ordering::Release);
    if let Ok(mut m) = slot.osr.lock() {
        m.clear();
        m.extend(out.osr.iter().map(|(pc, a)| (*pc, *a as usize)));
    }
    slot.state.store(zen_vm::jit::STATE_JITTED, Ordering::Release);
    let mut vm = Vm::new(Box::new(RefCountGc));
    vm.jit = JitHandle::start(2, || Box::new(CraneliftCompiler::new()));
    // Sanity: the loop really did run natively before the raise.
    let err = vm
        .run_module(&module.main, &module.children)
        .expect_err("OOB index must raise");
    assert!(
        vm.osr_transfers >= 1,
        "loop should have transferred before the raise"
    );
    assert!(
        format!("{err:?}").contains("IndexError") || format!("{err}").contains("Index"),
        "expected an index error, got {err:?}"
    );
}
