//! JIT tier-up infrastructure (Stage 1).
//!
//! The interpreter records how often each [`FunctionProto`] is called. When a
//! function crosses [`HOT_THRESHOLD`] and no compilation is already in flight,
//! its state is atomically advanced to `QUEUED` and a [`CompileSnapshot`] is
//! handed to a background worker pool over a channel. Workers lower the
//! bytecode to native code on their own threads and publish the entry point via
//! [`JitSlot::entry`] (see `docs/10-jit.md`). The executing thread never blocks:
//! it keeps interpreting until an entry point appears, then jumps to it.
//!
//! A `FunctionProto` is deliberately `!Sync` (its inline-cache entries hold
//! unsynchronized interior state), so the pool never sees one. Only the
//! thread-safe [`Arc<JitSlot>`] and a plain-data [`CompileSnapshot`] cross the
//! worker boundary; the compiled entry point is published back through the slot.
//!
//! Nothing here changes observable behaviour until a real compiler publishes a
//! native entry. The default path (feature off, or `ZEN_JIT` unset) only skips a
//! single `Option` check per call.

use std::sync::atomic::{AtomicU8, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::bytecode::{FunctionProto, Instruction, UpvalueDesc};
use crate::ic::InlineCache;
use crate::value::Value;

/// No native code; the interpreter runs this function.
pub const STATE_INTERPRETED: u8 = 0;
/// A compilation task is in the worker queue; do not enqueue it again.
pub const STATE_QUEUED: u8 = 1;
/// A native entry point is published in [`JitSlot::entry`].
pub const STATE_JITTED: u8 = 2;
/// Compilation was attempted and failed (or the function is unsupported);
/// never queue it again.
pub const STATE_FAILED: u8 = 3;

/// Calls to a function before it is offered to the JIT.
pub const HOT_THRESHOLD: u32 = 1000;


/// Type feedback record for a single bytecode site.
#[derive(Clone, Debug, Default)]
pub struct FeedbackEntry {
    /// Bitmask of observed operand/output type tags (bit position by Tag ordinal or just ORed).
    /// Keep small; we can also count hits. For now store observed mask and hit count.
    pub mask: u32,
    pub hits: u32,
}

/// Per-PC feedback vector. Grows on demand to match code length.
#[derive(Clone, Debug)]
pub struct FeedbackVec {
    pub entries: Vec<FeedbackEntry>,
}

impl Default for FeedbackVec {
    fn default() -> Self {
        Self { entries: Vec::new() }
    }
}

impl FeedbackVec {
    #[inline]
    pub fn record(&mut self, pc: usize, tag_bits: u32) {
        if pc >= self.entries.len() {
            self.entries.resize(pc + 1, FeedbackEntry::default());
        }
        let e = &mut self.entries[pc];
        e.mask |= tag_bits;
        e.hits = e.hits.saturating_add(1);
    }
    #[inline]
    pub fn get(&self, pc: usize) -> Option<&FeedbackEntry> {
        self.entries.get(pc)
    }
}

/// Per-function tier-up state, shared through `FunctionProto::jit`.
#[derive(Debug)]
pub struct JitSlot {
    /// Invocation counter.
    pub hotness: AtomicU32,
    /// One of the `STATE_*` values.
    pub state: AtomicU8,
    /// Native entry point address once `state == STATE_JITTED`.
    pub entry: AtomicUsize,
    /// Type feedback collected by interpreter.
    pub feedback: Mutex<FeedbackVec>,
    /// Deopt count: native exits back to the interpreter.
    pub deopts: AtomicU32,
    /// Loop back-edge hits (OSR hotness).
    pub loop_hotness: AtomicU32,
    /// OSR entry points: `(loop-header pc, native address)` pairs published
    /// alongside `entry`. The interpreter transfers into one mid-loop.
    pub osr: Mutex<Vec<(u32, usize)>>,
}

impl Default for JitSlot {
    fn default() -> Self {
        Self {
            hotness: AtomicU32::new(0),
            state: AtomicU8::new(STATE_INTERPRETED),
            entry: AtomicUsize::new(0),
            feedback: Mutex::new(FeedbackVec::default()),
            deopts: AtomicU32::new(0),
            loop_hotness: AtomicU32::new(0),
            osr: Mutex::new(Vec::new()),
        }
    }
}

impl JitSlot {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Every field a lowerer needs to produce native code. All `Send`; the pool
/// compiles a snapshot, never the live (single-threaded) prototype.
#[derive(Debug, Clone)]
pub struct CompileSnapshot {
    pub name: String,
    pub params: Vec<String>,
    pub module_id: u32,
    pub num_globals: u16,
    pub num_locals: u16,
    pub max_reg: u16,
    pub num_required: u16,
    pub num_upvalues: u16,
    pub code: Vec<Instruction>,
    pub constants: Vec<Value>,
    pub line_table: Vec<(u32, u32)>,
    pub upvalue_descriptors: Vec<UpvalueDesc>,
    pub ic_slots: Vec<InlineCache>,
    /// Per-PC type feedback observed by the interpreter (bitmask + hits).
    pub feedback: Vec<FeedbackEntry>,
    /// Methods carry an implicit receiver in register 0 that is not part of
    /// `params`; the native entry must load one extra leading register.
    pub is_method: bool,
}

impl CompileSnapshot {
    pub fn from_proto(proto: &FunctionProto) -> Self {
        let feedback = proto
            .jit
            .get()
            .and_then(|s| s.feedback.lock().ok())
            .map(|f| f.entries.clone())
            .unwrap_or_default();
        Self {
            name: proto.name.clone(),
            params: proto.params.clone(),
            module_id: proto.module_id,
            num_globals: proto.num_globals,
            num_locals: proto.num_locals,
            max_reg: proto.max_reg,
            num_required: proto.num_required,
            num_upvalues: proto.num_upvalues,
            code: proto.code.clone(),
            constants: proto.constants.clone(),
            line_table: proto.line_table.clone(),
            upvalue_descriptors: proto.upvalue_descriptors.clone(),
            ic_slots: proto.ic_slots.borrow().clone(),
            feedback,
            is_method: proto.is_method,
        }
    }

    /// Feedback for `pc`, if any was observed.
    #[inline]
    pub fn feedback_at(&self, pc: usize) -> Option<&FeedbackEntry> {
        self.feedback.get(pc)
    }
}

/// Record one call to `proto`. Cheap and inert when the JIT is disabled.
pub fn note_call(jit: Option<&JitHandle>, proto: &Arc<FunctionProto>) {
    let Some(jit) = jit else {
        return;
    };
    let slot = proto.jit.get_or_init(|| Arc::new(JitSlot::new())).clone();
    if slot.state.load(Ordering::Relaxed) != STATE_INTERPRETED {
        return;
    }
    let n = slot.hotness.fetch_add(1, Ordering::Relaxed).saturating_add(1);
    if n < HOT_THRESHOLD {
        return;
    }
    // Exactly one thread wins the race to queue this function.
    if slot
        .state
        .compare_exchange(
            STATE_INTERPRETED,
            STATE_QUEUED,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_ok()
    {
        jit.enqueue(slot, CompileSnapshot::from_proto(proto));
    }
}

/// Loop back-edge hotness: called by the interpreter on a backward jump.
/// When a loop goes hot before the call threshold, queue the function so the
/// pool can compile the loop body (OSR trigger). `OSR_THRESHOLD` is lower
/// than [`HOT_THRESHOLD`] because a single call can execute many iterations.
pub const OSR_THRESHOLD: u32 = 500;

#[inline]
pub fn note_loop(jit: Option<&JitHandle>, proto: &Arc<FunctionProto>) {
    let Some(jit) = jit else {
        return;
    };
    let slot = proto.jit.get_or_init(|| Arc::new(JitSlot::new())).clone();
    if slot.state.load(Ordering::Relaxed) != STATE_INTERPRETED {
        return;
    }
    let n = slot
        .loop_hotness
        .fetch_add(1, Ordering::Relaxed)
        .saturating_add(1);
    if n < OSR_THRESHOLD {
        return;
    }
    if slot
        .state
        .compare_exchange(
            STATE_INTERPRETED,
            STATE_QUEUED,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_ok()
    {
        jit.enqueue(slot, CompileSnapshot::from_proto(proto));
    }
}

/// Maximum deopts before a compiled function is re-specialized from fresh
/// feedback. A thin guard that fires repeatedly means the snapshot's shapes
/// or types went stale; re-queue once with the new observations.
pub const DEOPT_RESPECIALIZE_THRESHOLD: u32 = 100;

/// Record a native→interpreter deopt. Returns `true` when the caller should
/// re-queue the function for re-specialization.
#[inline]
pub fn note_deopt(proto: &Arc<FunctionProto>) -> bool {
    let Some(slot) = proto.jit.get() else {
        return false;
    };
    let n = slot.deopts.fetch_add(1, Ordering::Relaxed).saturating_add(1);
    n == DEOPT_RESPECIALIZE_THRESHOLD
}

/// Re-queue a jitted function whose guards overflowed, using fresh feedback.
/// Only transitions `JITTED -> QUEUED`; the old entry keeps serving until the
/// new one publishes (lock-free publish via `entry` + `state`).
pub fn respecialize(jit: Option<&JitHandle>, proto: &Arc<FunctionProto>) {
    let Some(jit) = jit else {
        return;
    };
    let Some(slot) = proto.jit.get() else {
        return;
    };
    if slot
        .state
        .compare_exchange(STATE_JITTED, STATE_QUEUED, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        slot.deopts.store(0, Ordering::Relaxed);
        let snap = CompileSnapshot::from_proto(proto);
        jit.enqueue(slot.clone(), snap);
    }
}

/// The published result of compiling one function: the normal entry point
/// plus zero or more OSR entries keyed by loop-header pc. Native addresses;
/// only set `STATE_JITTED` after the code is finalized and executable.
#[derive(Debug, Clone)]
pub struct CompileOutput {
    pub entry: *const u8,
    pub osr: Vec<(u32, *const u8)>,
}

// SAFETY: addresses point at finalized executable mappings owned by the JIT
// module, which outlives any use (generated code is never freed).
unsafe impl Send for CompileOutput {}

#[cfg(feature = "jit")]
mod imp {
    use super::*;
    use crossbeam_channel::{unbounded, Sender};
    use std::thread;

    /// A bytecode-to-native lowerer. The background pool owns one instance per
    /// worker thread. Implemented by the Cranelift backend.
    pub trait ZenJitCompiler: Send {
        /// Lower `snap` and return its native entry points, or an error.
        fn compile(&mut self, snap: &CompileSnapshot) -> Result<CompileOutput, String>;
    }

    /// Placeholder compiler used until the Cranelift lowerer lands. It declines
    /// every function, so a queued function ends in `STATE_FAILED`.
    pub struct StubCompiler;

    impl ZenJitCompiler for StubCompiler {
        fn compile(&mut self, _snap: &CompileSnapshot) -> Result<CompileOutput, String> {
            Err("jit: no lowering backend linked".into())
        }
    }

    /// Handle to the background compilation pool.
    pub struct JitHandle {
        tx: Sender<(Arc<JitSlot>, CompileSnapshot)>,
    }

    fn worker_count() -> usize {
        if let Some(n) = std::env::var("ZEN_JIT_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
        {
            return n;
        }
        thread::available_parallelism()
            .map(|n| n.get().saturating_sub(1).max(1))
            .unwrap_or(1)
    }

    impl JitHandle {
        /// Start the worker pool if `ZEN_JIT` is set; otherwise return `None`.
        pub fn new() -> Option<Self> {
            if std::env::var_os("ZEN_JIT").is_none() {
                return None;
            }
            Self::start(worker_count(), || Box::new(StubCompiler))
        }

        /// Start a pool of `threads` workers, each owning a fresh compiler from
        /// `factory`. Independent of the `ZEN_JIT` environment, for embeddings
        /// and tests.
        pub fn start<F>(threads: usize, mut factory: F) -> Option<Self>
        where
            F: FnMut() -> Box<dyn ZenJitCompiler> + Send + 'static,
        {
            if threads == 0 {
                return None;
            }
            let (tx, rx): (Sender<(Arc<JitSlot>, CompileSnapshot)>, _) = unbounded();
            for _ in 0..threads {
                let rx = rx.clone();
                let compiler = factory();
                thread::Builder::new()
                    .name("zen-jit".into())
                    .spawn(move || {
                        let mut compiler = compiler;
                        while let Ok((slot, snap)) = rx.recv() {
                            match compiler.compile(&snap) {
                                Ok(out) => {
                                    slot.entry.store(out.entry as usize, Ordering::Release);
                                    if let Ok(mut m) = slot.osr.lock() {
                                        m.clear();
                                        m.extend(
                                            out.osr.iter().map(|(pc, a)| (*pc, *a as usize)),
                                        );
                                    }
                                    slot.state.store(STATE_JITTED, Ordering::Release);
                                }
                                Err(_) => {
                                    slot.state.store(STATE_FAILED, Ordering::Release);
                                }
                            }
                        }
                    })
                    .expect("spawn zen-jit worker");
            }
            Some(Self { tx })
        }

        /// Hand a compile request to a background worker. Never blocks.
        pub fn enqueue(&self, slot: Arc<JitSlot>, snap: CompileSnapshot) {
            let _ = self.tx.send((slot, snap));
        }
    }
}

#[cfg(feature = "jit")]
pub use imp::{JitHandle, StubCompiler, ZenJitCompiler};

#[cfg(not(feature = "jit"))]
mod stub {
    use super::CompileSnapshot;
    use std::sync::Arc;

    /// Inert stand-in so the VM can hold `Option<JitHandle>` unconditionally.
    pub struct JitHandle;

    impl JitHandle {
        pub fn new() -> Option<Self> {
            None
        }
        pub fn enqueue(&self, _slot: Arc<crate::jit::JitSlot>, _snap: CompileSnapshot) {}
    }
}

#[cfg(not(feature = "jit"))]
pub use stub::JitHandle;

/// The ABI of a JIT-compiled Zen function.
///
/// `regs` is the base of the register file, `base` this function's window, `vm`
/// the machine, `out` where the returned value is written, and `closure` the
/// function value that was invoked. Arguments live in the register window at
/// `regs[base..]`. Returns `0` on success, `1` to deopt.
pub type NativeEntry = unsafe extern "C" fn(
    regs: *mut u64,
    base: usize,
    vm: *mut crate::vm::Vm,
    out: *mut u64,
    closure: u64,
) -> u32;

/// Return `proto`'s published native entry point, if a compiler has provided one.
#[cfg(feature = "jit")]
#[allow(unsafe_code)]
pub fn native_entry(proto: &FunctionProto) -> Option<NativeEntry> {
    let slot = proto.jit.get()?;
    if slot.state.load(Ordering::Acquire) != STATE_JITTED {
        return None;
    }
    let addr = slot.entry.load(Ordering::Acquire);
    if addr == 0 {
        return None;
    }
    // SAFETY: a JIT compiler published a `NativeEntry`-typed pointer here and
    // only sets `STATE_JITTED` after the code is finalized and executable.
    Some(unsafe { std::mem::transmute::<usize, NativeEntry>(addr) })
}

#[cfg(not(feature = "jit"))]
pub fn native_entry(_proto: &FunctionProto) -> Option<NativeEntry> {
    None
}

/// Return `proto`'s OSR entry for loop-header `pc`, if published. An OSR entry
/// shares the [`NativeEntry`] ABI but starts at the loop header with the full
/// register window live, so the interpreter can transfer into it mid-loop.
#[cfg(feature = "jit")]
#[allow(unsafe_code)]
pub fn osr_entry(proto: &FunctionProto, pc: usize) -> Option<NativeEntry> {
    let slot = proto.jit.get()?;
    if slot.state.load(Ordering::Acquire) != STATE_JITTED {
        return None;
    }
    let map = slot.osr.lock().ok()?;
    let addr = map
        .iter()
        .find(|(p, _)| *p as usize == pc)
        .map(|(_, a)| *a)?;
    if addr == 0 {
        return None;
    }
    // SAFETY: same publication protocol as [`native_entry`].
    Some(unsafe { std::mem::transmute::<usize, NativeEntry>(addr) })
}

#[cfg(not(feature = "jit"))]
pub fn osr_entry(_proto: &FunctionProto, _pc: usize) -> Option<NativeEntry> {
    None
}

/// The outcome of entering JIT-compiled code.
pub enum InvokeOutcome {
    /// Native code ran to `Return`; the value is the result.
    Compiled(Value),
    /// A guard fired; native code spilled its registers and asked to resume
    /// interpretation at this pc.
    Resume(usize),
    /// Not compiled (or window does not fit): run the interpreter instead.
    Fallback,
}

/// If `callee_proto` has a published native entry and its register window fits,
/// run it. Native status `0` is success; any other status is `pc + 2` asking
/// to resume the interpreter at `pc` with the window as spilled.
#[cfg(feature = "jit")]
#[allow(unsafe_code)]
pub fn invoke_if_compiled(
    vm: &mut crate::vm::Vm,
    callee_proto: &Arc<FunctionProto>,
    callee: Value,
    base: usize,
    first: usize,
) -> InvokeOutcome {
    let Some(entry) = native_entry(callee_proto) else {
        return InvokeOutcome::Fallback;
    };
    if base + first + callee_proto.max_reg as usize > crate::vm::REGISTERS {
        return InvokeOutcome::Fallback;
    }
    let mut out = Value::nil();
    let regs = vm.registers.as_mut_ptr() as *mut u64;
    let vm_ptr = vm as *mut crate::vm::Vm;
    // SAFETY: `regs` addresses the fixed 256-slot register file, which never
    // reallocates; the callee window was bounds-checked. On nonzero status the
    // frame spilled its own window, so the caller may resume interpreting.
    let status = unsafe {
        entry(
            regs,
            base + first,
            vm_ptr,
            &mut out as *mut Value as *mut u64,
            callee.to_bits(),
        )
    };
    if status == 0 {
        InvokeOutcome::Compiled(out)
    } else {
        // Native guard fired: count the deopt. On overflow the shapes/types
        // went stale, so re-queue with fresh feedback (thin guard ->
        // re-specialize). The old entry keeps serving until the new one
        // publishes; meanwhile the caller resumes interpreting.
        if note_deopt(callee_proto) {
            // SAFETY: `vm_ptr` is the live VM borrowed as `vm` above; the
            // native call has returned, so reborrowing is sound.
            let jit_opt = unsafe { (&(*vm_ptr).jit).as_ref() };
            respecialize(jit_opt, callee_proto);
        }
        InvokeOutcome::Resume(status.saturating_sub(2) as usize)
    }
}

#[cfg(not(feature = "jit"))]
pub fn invoke_if_compiled(
    _vm: &mut crate::vm::Vm,
    _callee_proto: &Arc<FunctionProto>,
    _callee: Value,
    _base: usize,
    _first: usize,
) -> InvokeOutcome {
    InvokeOutcome::Fallback
}

/// Record type feedback for the given pc in the function's JIT slot.
#[inline]
pub fn record_feedback(jit: Option<&JitHandle>, proto: &Arc<FunctionProto>, pc: usize, tag_bits: u32) {
    let _ = (proto, pc, tag_bits);
    #[cfg(feature = "jit")]
    {
        if jit.is_none() {
            return;
        }
        let Some(slot) = proto.jit.get() else {
            // No slot yet: create without racing the hot path. `get_or_init`
            // is cheap but still atomic; only pay it until queued.
            let slot = std::sync::Arc::clone(proto.jit.get_or_init(|| Arc::new(JitSlot::new())));
            if slot.state.load(Ordering::Relaxed) != STATE_INTERPRETED {
                return;
            }
            if let Ok(mut f) = slot.feedback.try_lock() {
                f.record(pc, tag_bits);
            };
            return;
        };
        // Only collect while still interpreted: once queued/jitted/failed the
        // snapshot is already taken (or will never be), so skip the lock.
        if slot.state.load(Ordering::Relaxed) != STATE_INTERPRETED {
            return;
        }
        if let Ok(mut f) = slot.feedback.try_lock() {
            f.record(pc, tag_bits);
        };
    }
}
