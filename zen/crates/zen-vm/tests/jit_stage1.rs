//! Stage 1 JIT: hotness counting, the atomic state machine, and the background
//! worker publish path. Uses an injected fake compiler so the test is hermetic
//! (no `ZEN_JIT` env, no cranelift).
#![cfg(feature = "jit")]

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use zen_vm::bytecode::{FunctionProto, Instruction, Opcode};
use zen_vm::jit::{CompileOutput, CompileSnapshot, JitHandle, JitSlot, ZenJitCompiler};

fn proto() -> Arc<FunctionProto> {
    Arc::new(FunctionProto {
        name: "g".into(),
        params: vec!["n".into()],
        module_id: 0,
        num_globals: 0,
        num_locals: 1,
        max_reg: 2,
        num_required: 1,
        num_upvalues: 0,
        code: vec![Instruction::new(Opcode::Return, 0, 0, 0)],
        constants: vec![],
        line_table: vec![],
        upvalue_descriptors: vec![],
        ic_slots: std::cell::RefCell::new(Vec::new()),
        jit: std::sync::OnceLock::new(),
        is_method: false,
    })
}

struct Fake(usize);

impl ZenJitCompiler for Fake {
    fn compile(&mut self, _snap: &CompileSnapshot) -> Result<CompileOutput, String> {
        Ok(CompileOutput {
            entry: self.0 as *const u8,
            osr: vec![],
        })
    }
}

struct Decline;

impl ZenJitCompiler for Decline {
    fn compile(&mut self, _snap: &CompileSnapshot) -> Result<CompileOutput, String> {
        Err("declined".into())
    }
}

fn wait_state(slot: &JitSlot, expect: u8) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while slot.state.load(Ordering::Acquire) != expect {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for jit state {expect}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn hotness_queues_and_background_publishes_entry() {
    let pool = JitHandle::start(2, || Box::new(Fake(0x1234))).expect("pool starts");
    let p = proto();
    for _ in 0..zen_vm::jit::HOT_THRESHOLD {
        zen_vm::jit::note_call(Some(&pool), &p);
    }
    let slot = p.jit.get().expect("slot initialized by note_call");
    wait_state(slot, zen_vm::jit::STATE_JITTED);
    assert_eq!(slot.entry.load(Ordering::Acquire), 0x1234);
    assert_eq!(
        slot.hotness.load(Ordering::Relaxed),
        zen_vm::jit::HOT_THRESHOLD
    );
}

#[test]
fn failed_compile_is_not_requeued() {
    let pool = JitHandle::start(1, || Box::new(Decline)).expect("pool starts");
    let p = proto();
    for _ in 0..zen_vm::jit::HOT_THRESHOLD {
        zen_vm::jit::note_call(Some(&pool), &p);
    }
    let slot = p.jit.get().unwrap();
    wait_state(slot, zen_vm::jit::STATE_FAILED);
    let before = slot.hotness.load(Ordering::Relaxed);
    for _ in 0..1000 {
        zen_vm::jit::note_call(Some(&pool), &p);
    }
    assert_eq!(slot.hotness.load(Ordering::Relaxed), before);
    assert_eq!(slot.state.load(Ordering::Acquire), zen_vm::jit::STATE_FAILED);
}