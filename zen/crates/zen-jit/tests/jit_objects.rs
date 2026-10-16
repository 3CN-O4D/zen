//! Differential tests for Stage 3 property access: the Cranelift JIT lowers
//! `GetField`/`SetField` through the prototype's inline cache and must agree
//! with the interpreter, both on monomorphic hits and on deopting misses.
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

/// Runs interpreted and jitted, asserts an exact match, and requires that at
/// least one function actually compiled.
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

const MONOMORPHIC_READ: &str = r#"
class Point {
    func init(x, y) {
        self.x = x
        self.y = y
    }
}
func sum(p) {
    return p.x + p.y
}
var p = new Point(3, 4)
var total = 0
var i = 0
while i < 3000 {
    total = total + sum(p)
    i = i + 1
}
return total
"#;

#[test]
fn jit_matches_interpreter_for_monomorphic_field_read() {
    let v = assert_jit_matches(MONOMORPHIC_READ);
    assert_eq!(v.as_int(), Some(21000));
}

const READ_WRITE: &str = r#"
class Counter {
    func init() {
        self.n = 0
    }
}
func bump(c) {
    c.n = c.n + 1
    return c.n
}
var c = new Counter()
var i = 0
while i < 3000 {
    bump(c)
    i = i + 1
}
return c.n
"#;

#[test]
fn jit_matches_interpreter_for_field_write() {
    let v = assert_jit_matches(READ_WRITE);
    assert_eq!(v.as_int(), Some(3000));
}

const POLYMORPHIC: &str = r#"
class Cell {
    func init() {
    }
}
func bump(x) {
    x.n = x.n + 1
    return x.n
}
var a = new Cell()
a.n = 0
var b = new Cell()
b.extra = 7
b.n = 100
var i = 0
var total = 0
while i < 3000 {
    if i % 2 == 0 {
        total = total + bump(a)
    } else {
        total = total + bump(b)
    }
    i = i + 1
}
return total
"#;

#[test]
fn jit_matches_interpreter_for_polymorphic_writes() {
    // `bump` sees two receiver shapes, so its cache goes polymorphic and the
    // native path deopts every call; the interpreter must still produce the
    // same answer.
    let v = assert_jit_matches(POLYMORPHIC);
    assert_eq!(v.as_int(), Some(2_401_500));
}

const METHOD_READ: &str = r#"
class Point {
    func init(x, y) {
        self.x = x
        self.y = y
    }
    func sum() {
        return self.x + self.y
    }
}
var p = new Point(3, 4)
var total = 0
var i = 0
while i < 3000 {
    total = total + p.sum()
    i = i + 1
}
return total
"#;

#[test]
fn jit_matches_interpreter_for_method_field_read() {
    // `CallMethod` tiers the method up like `Call`; the compiled `sum` reads
    // `self.x`/`self.y` through the inline cache.
    let v = assert_jit_matches(METHOD_READ);
    assert_eq!(v.as_int(), Some(21000));
}

const CLOSURE_CAPTURE: &str = r#"
func make_adder(n) {
    func add(x) {
        return x + n
    }
    return add
}
var add5 = make_adder(5)
var total = 0
var i = 0
while i < 3000 {
    total = total + add5(i)
    i = i + 1
}
return total
"#;

#[test]
fn jit_matches_interpreter_for_closure_capture() {
    // `add` captures `n` as an upvalue; `MakeClosure` + `GetUpval` lower to
    // native code and the hot closure tiers up like any function.
    let v = assert_jit_matches(CLOSURE_CAPTURE);
    assert_eq!(v.as_int(), Some(4_513_500));
}

const CLOSURE_MUTATE: &str = r#"
func make_counter() {
    var n = 0
    func bump() {
        n = n + 1
        return n
    }
    return bump
}
var c = make_counter()
var i = 0
while i < 3000 {
    c()
    i = i + 1
}
return c()
"#;

#[test]
fn jit_matches_interpreter_for_closure_mutation() {
    // `SetUpval`/`GetUpval` through shared cells; the compiled `bump` must
    // observe writes across calls exactly like the interpreter.
    let v = assert_jit_matches(CLOSURE_MUTATE);
    assert_eq!(v.as_int(), Some(3001));
}
