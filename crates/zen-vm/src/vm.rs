//! VM executor.
//!
//! A stack-based register interpreter over the bytecode defined in
//! [`crate::bytecode`]. It executes the instruction set emitted by
//! `zen-compiler`; the opcode encodings and calling convention are the ABI
//! documented in `docs/07-bytecode.md` and the header of
//! `crates/zen-compiler/src/compiler.rs`.

use std::cmp::Ordering;
use std::sync::Arc;

use indexmap::IndexMap;

use crate::bytecode::{FunctionProto, ModuleUnit, Opcode};
use crate::gc::{
    new_cell, new_class, new_dict, new_error, new_func, new_instance, new_list, new_set, new_tuple,
    Gc,
};
use crate::ic::InlineCache;
use crate::object::ObjKind;
use crate::ops::{self, Arith, Bit};
use crate::value::{DictKey, Value};

pub use crate::errors::{VmError, VmResult};

/// Size of the fixed register file (`docs/07-bytecode.md` §2.2 reserves the
/// top six registers).
pub const REGISTERS: usize = 256;

/// Maximum number of nested calls before a `RecursionError`.
pub const MAX_FRAMES: usize = 1024;

/// A pending `catch` block: the resume point and the local slot that receives
/// the error value when an error unwinds into the owning frame.
#[derive(Debug, Clone, Copy)]
pub struct Handler {
    pub catch_pc: usize,
    pub err_slot: u8,
}

/// An active call frame.
#[derive(Debug, Clone)]
pub struct Frame {
    /// Prototype being executed.
    pub proto: Arc<FunctionProto>,
    /// The closure value that was called (carries captured upvalues).
    pub closure: Value,
    /// Register-file index of this frame's register 0.
    pub base: usize,
    /// Next instruction index.
    pub pc: usize,
    /// Caller register that receives this call's result.
    pub return_reg: u8,
    /// When set, the value this frame returns is replaced by this one. Used by
    /// `class(args)`: it runs `init` but yields the freshly built instance.
    pub ctor: Option<Value>,
    /// Cells for captured locals, parallel to this frame's registers: slot `i`
    /// holds the shared box for register `i` once an inner closure captured it.
    pub cells: Vec<Option<Value>>,
    /// Active `try` handlers, innermost last. `catch_pc` is the instruction the
    /// VM resumes at when an error unwinds into this frame.
    pub handlers: Vec<Handler>,
    /// Index into [`Vm::module_globals`] identifying the module whose globals
    /// this frame reads and writes via `GetGlobal`/`SetGlobal`.
    pub module_id: usize,
    /// When this frame is a module's top-level execution (pushed by `Import`),
    /// the registry index of that module. On return the VM captures the
    /// module's exports into its dictionary and marks it `Ready`.
    pub import_idx: Option<usize>,
}

/// Where a runtime error was raised: the failing function, its source line
/// (1-based, from the prototype's `line_table`), and the registry module id.
/// Recorded before `catch` unwinding so an *uncaught* error still points at
/// the exact instruction that failed.
#[derive(Debug, Clone)]
pub struct ErrorLocation {
    pub module_id: usize,
    pub func: String,
    pub line: u32,
}

/// Loading state of a registered module.
#[derive(Debug)]
enum ModuleState {
    /// Registered but never executed; the first `Import` runs it.
    Unloaded,
    /// Currently executing. The dictionary is the same reference returned to
    /// every importer, so cycles observe a live, partially-filled module.
    Loading(Value),
    /// Finished; the dictionary is fully populated.
    Ready(Value),
}

/// A registered module plus its registry state and runtime globals slot.
#[derive(Debug)]
struct ModuleInstance {
    unit: crate::bytecode::ModuleUnit,
    state: ModuleState,
    /// Index into [`Vm::module_globals`]/[`Vm::module_protos`].
    module_id: usize,
}

/// The virtual machine.
pub struct Vm {
    /// Fixed register file.
    pub registers: Vec<Value>,
    /// Active call stack.
    pub frames: Vec<Frame>,
    /// Allocator hook (retained for the GC API; not yet consulted).
    pub gc: Box<dyn Gc>,
    pub resources: crate::resource::ResourceTable,
    /// Per-module child-prototype tables indexed by `Frame::module_id` and
    /// `MakeClosure`'s `b` operand, so cross-module calls resolve prototypes
    /// against the callee's own module.
    module_protos: Vec<Vec<Arc<FunctionProto>>>,
    /// Per-module heap storage for module-scope bindings, indexed by
    /// `Frame::module_id`. Kept out of the fixed register file so imported
    /// modules cannot exhaust registers or clobber the active call stack.
    module_globals: Vec<Vec<Value>>,
    /// Registered modules by name (the linker pre-links them; the VM never
    /// performs I/O).
    modules: Vec<ModuleInstance>,
    module_index: std::collections::HashMap<String, usize>,
    /// Captured `print` output, for tests and host embedding.
    output: String,
    /// The error currently being handled, kept so a typed `catch` clause can
    /// re-raise it with its original kind and message.
    pending_error: Option<VmError>,
    /// Inline-cache hit/miss counters, exposed for benchmarks (`ic_stats`).
    ic_hits: std::cell::Cell<u64>,
    ic_misses: std::cell::Cell<u64>,
    /// When false, field inline caches are bypassed entirely so a benchmark can
    /// measure the un-cached baseline (`ZEN_NO_IC=1`).
    ic_enabled: bool,
    /// GC roots for method call-site caches: the class and resolved method held
    /// by each [`InlineCache::Method`] entry. Rooting the class keeps its
    /// address stable, so header-pointer equality is a valid identity check.
    method_roots: Vec<Value>,
    /// Whether the *current* top frame has any captured-local cells. Almost all
    /// frames never capture, so this lets `reg`/`set_reg` skip the `frames.last()`
    /// + `cells` probe on the hot path. Maintained on push/pop/promotion.
    cells_active: bool,
    /// Optional instruction budget. When `Some`, the run aborts with a
    /// `NativeError` once exhausted. `None` (the default) is unbounded. Used by
    /// embedding and fuzzing to guarantee a run terminates.
    step_budget: Option<u64>,
    /// Location of the most recent runtime error, recorded before `catch`
    /// unwinding. Read via [`Vm::error_location`] to render diagnostics.
    error_location: Option<ErrorLocation>,
    /// Background JIT worker pool. Present only when the `jit` feature is
    /// compiled in and `ZEN_JIT` is set; `None` makes the tier-up hook a
    /// single nullable check.
    pub jit: Option<crate::jit::JitHandle>,
    /// Foreign (CPython) bridge for `pymport`. `None` (the default) means no
    /// embedding is linked: `pymport` and any foreign dispatch fail with an
    /// actionable message instead of probing anything. Publish an
    /// `Arc<dyn PyBridge>` (e.g. the PyO3 embedding behind `--features
    /// python`) to enable it.
    pub py_bridge: Option<std::sync::Arc<dyn crate::pybridge::PyBridge>>,
    /// Native loop entries via OSR transfer (Stage 4). Incremented whenever
    /// the interpreter jumps into compiled code mid-loop.
    pub osr_transfers: u64,
}

/// What became of a callback frame driven by the re-entrant trampoline.
enum CallbackOutcome {
    /// The frame returned normally; its result register holds the value.
    Returned,
    /// An error escaped the callback and was handled by a `catch` *outside* it.
    /// The interrupted frame's `pc` already points into that handler, so the
    /// instruction that made the call is abandoned and writes no result.
    Aborted,
}

/// The result of offering a method name to the higher-order implementation.
enum HigherOrder {
    /// Not a higher-order method: fall through to the ordinary method table.
    NotOurs,
    /// The computed value.
    Value(Value),
    /// A `catch` outside the callback took over; write no result register.
    Aborted,
}

/// The outcome of one [`Vm::step`].
enum Step {
    /// Keep stepping.
    Continue,
    /// The frame stack emptied: this was the module's outermost `return`.
    Finished(Value),
}

impl Vm {
    /// Create a VM with an empty register file and call stack.
    pub fn new(gc: Box<dyn Gc>) -> Self {
        Self {
            registers: vec![Value::nil(); REGISTERS],
            frames: Vec::new(),
            gc,
            module_protos: Vec::new(),
            module_globals: Vec::new(),
            modules: Vec::new(),
            module_index: std::collections::HashMap::new(),
            output: String::new(),
            resources: crate::resource::ResourceTable::new(),
            ic_hits: std::cell::Cell::new(0),
            ic_misses: std::cell::Cell::new(0),
            ic_enabled: std::env::var_os("ZEN_NO_IC").is_none(),
            method_roots: Vec::new(),
            cells_active: false,
            step_budget: None,
            pending_error: None,
            error_location: None,
            jit: crate::jit::JitHandle::new(),
            py_bridge: None,
            osr_transfers: 0,
        }
    }

    /// Bound execution to at most `budget` instructions; `None` means unbounded.
    /// A run that exhausts its budget returns a `NativeError` rather than
    /// looping forever, which is what lets embedding hosts and fuzzers run
    /// untrusted programs safely.
    pub fn set_step_budget(&mut self, budget: Option<u64>) {
        self.step_budget = budget;
    }

    /// Read a module global by index, for JIT-compiled code. Out-of-range reads
    /// yield `nil`.
    #[cfg(feature = "jit")]
    pub fn jit_global(&self, module_id: usize, index: usize) -> Value {
        self.module_globals
            .get(module_id)
            .and_then(|g| g.get(index))
            .copied()
            .unwrap_or(Value::nil())
    }

    /// Write a module global by index, for JIT-compiled code. Returns `false`
    /// when out of range (the interpreter raises there; native code deopts).    #[cfg(feature = "jit")]
    pub fn jit_set_global(&mut self, module_id: usize, index: usize, val: Value) -> bool {
        match self
            .module_globals
            .get_mut(module_id)
            .and_then(|g| g.get_mut(index))
        {
            Some(slot) => {
                *slot = val;
                true
            }
            None => false,
        }
    }

    /// Byte offset of `Vm::module_globals` for JIT-compiled code, which loads
    /// globals inline (live loads off the `vm` pointer — no stale pointers,
    /// no ABI change). `Vec<Value>` layout (`ptr, len, cap`) is pinned by the
    /// `vec_value_layout` test below; any `Vm` reshuffle recomputes this
    /// automatically via `offset_of!`.
    #[cfg(feature = "jit")]
    pub fn module_globals_off() -> usize {
        std::mem::offset_of!(Vm, module_globals)
    }

    /// `(hits, misses)` since the last [`Vm::reset_ic_stats`].
    pub fn ic_stats(&self) -> (u64, u64) {
        (self.ic_hits.get(), self.ic_misses.get())
    }

    /// Zero the inline-cache counters.
    pub fn reset_ic_stats(&self) {
        self.ic_hits.set(0);
        self.ic_misses.set(0);
    }

    /// Enable or disable field inline caches (the `ZEN_NO_IC` baseline).
    pub fn set_ic_enabled(&mut self, enabled: bool) {
        self.ic_enabled = enabled;
    }

    /// Slot index of `name` under `shape`, or `None` if the shape lacks it.
    /// Served by the process-wide shape registry (see `crate::shape`).
    #[inline]
    fn shape_offset(&self, shape: u32, name: &str) -> Option<usize> {
        crate::shape::shape_offset(shape, name)
    }

    /// The shape that adds `name` to `shape`, creating (and memoising) it on
    /// first use. Served by the process-wide registry.
    fn shape_transition(&self, shape: u32, name: &str) -> u32 {
        crate::shape::shape_transition(shape, name)
    }

    /// Ensure the IC vector for `proto` has a slot for every instruction.
    #[inline]
    fn ic_reserve(proto: &FunctionProto, index: usize) -> std::cell::RefMut<'_, Vec<InlineCache>> {
        let mut ic = proto.ic_slots.borrow_mut();
        if ic.len() <= index {
            ic.resize(index + 1, InlineCache::Uninitialized);
        }
        ic
    }

    /// Inline-cache read of an instance field: a monomorphic hit loads the
    /// flat slot directly; `None` falls back to the (shape-resolving) slow
    /// path.
    pub(crate) fn ic_get_field(&self, proto: &FunctionProto, index: usize, obj: Value) -> Option<Value> {
        if !self.ic_enabled {
            return None;
        }
        let shape = obj.instance_shape()?;
        let ic = Self::ic_reserve(proto, index);
        let offset = match &ic[index] {
            InlineCache::Monomorphic { shape: s, offset } if *s == shape => *offset,
            _ => {
                drop(ic);
                self.ic_misses.set(self.ic_misses.get().wrapping_add(1));
                return None;
            }
        };
        drop(ic);
        let v = obj.as_instance()?.get(1 + offset).copied()?;
        self.ic_hits.set(self.ic_hits.get().wrapping_add(1));
        Some(v)
    }

    /// Record the shape and slot observed by a slow-path instance field read.
    fn ic_update_get(&self, proto: &FunctionProto, index: usize, obj: Value, name: Value) {
        let shape = match obj.instance_shape() {
            Some(s) => s,
            None => return,
        };
        let name = match name.as_str() {
            Some(s) => s,
            None => return,
        };
        let offset = match self.shape_offset(shape, name) {
            Some(o) => o,
            None => return,
        };
        let mut ic = Self::ic_reserve(proto, index);
        let entry = &mut ic[index];
        match entry {
            InlineCache::Uninitialized => {
                *entry = InlineCache::Monomorphic { shape, offset };
            }
            InlineCache::Monomorphic {
                shape: s,
                offset: o,
            } => {
                if *s == shape {
                    *o = offset;
                } else {
                    *entry = InlineCache::Polymorphic;
                }
            }
            _ => {}
        }
    }

    /// Inline-cache write of an instance field, mirroring [`ops::set_field`]'s
    /// instance branch. Returns `true` when the cached flat slot was written.
    pub(crate) fn ic_set_field(&self, proto: &FunctionProto, index: usize, obj: Value, val: Value) -> bool {
        if !self.ic_enabled {
            return false;
        }
        let shape = match obj.instance_shape() {
            Some(s) => s,
            None => return false,
        };
        let offset = {
            let ic = Self::ic_reserve(proto, index);
            match &ic[index] {
                InlineCache::Monomorphic { shape: s, offset } if *s == shape => *offset,
                _ => {
                    drop(ic);
                    self.ic_misses.set(self.ic_misses.get().wrapping_add(1));
                    return false;
                }
            }
        };
        let mut obj_mut = obj;
        match obj_mut
            .as_instance_mut()
            .and_then(|slots| slots.get_mut(1 + offset))
        {
            Some(v) => {
                *v = val;
                self.ic_hits.set(self.ic_hits.get().wrapping_add(1));
                true
            }
            None => false,
        }
    }

    /// Record the shape and slot observed by a slow-path instance field write.
    /// Storage is owned by `ops::set_field` (which runs first and leaves the
    /// instance at its final shape); this only folds the observation into the
    /// cache entry.
    fn ic_update_set(&self, proto: &FunctionProto, index: usize, obj: Value, name: Value) {
        let shape = match obj.instance_shape() {
            Some(s) => s,
            None => return,
        };
        let name = match name.as_str() {
            Some(s) => s,
            None => return,
        };
        let offset = match crate::shape::shape_offset(shape, name) {
            Some(o) => o,
            None => return,
        };
        let mut ic = Self::ic_reserve(proto, index);
        let entry = &mut ic[index];
        match entry {
            InlineCache::Uninitialized => {
                *entry = InlineCache::Monomorphic { shape, offset };
            }
            InlineCache::Monomorphic {
                shape: s,
                offset: o,
            } => {
                if *s == shape {
                    *o = offset;
                } else {
                    *entry = InlineCache::Polymorphic;
                }
            }
            _ => {}
        }
    }

    /// Inline-cache probe for an instance method call. Returns the rooted
    /// method on a monomorphic hit; `None` falls back to the class table. The
    /// class is rooted in `method_roots`, keeping its address stable so header
    /// pointer equality is a sound identity check.
    pub(crate) fn ic_resolve_method(
        &self,
        proto: &FunctionProto,
        index: usize,
        class: Value,
    ) -> Option<Value> {
        if !self.ic_enabled {
            return None;
        }
        let (class_root, method_root) = {
            let ic = proto.ic_slots.borrow();
            match ic.get(index) {
                Some(InlineCache::Method {
                    class_root,
                    method_root,
                }) => (*class_root, *method_root),
                _ => {
                    drop(ic);
                    self.ic_misses.set(self.ic_misses.get().wrapping_add(1));
                    return None;
                }
            }
        };
        let cached_class = self.method_roots.get(class_root as usize).copied()?;
        let class_ptr = class.header().map(|p| p.as_ptr());
        let same = cached_class.header().map(|p| p.as_ptr()) == class_ptr;
        if same {
            let v = self.method_roots.get(method_root as usize).copied()?;
            self.ic_hits.set(self.ic_hits.get().wrapping_add(1));
            Some(v)
        } else {
            self.ic_misses.set(self.ic_misses.get().wrapping_add(1));
            None
        }
    }

    /// Record a monomorphic method call site. Roots the class and method so the
    /// identity check stays sound; a second receiver class degrades the site to
    /// [`InlineCache::Polymorphic`].
    pub(crate) fn ic_update_method(
        &mut self,
        proto: &FunctionProto,
        index: usize,
        class: Value,
        method: Value,
    ) {
        let mut ic = Self::ic_reserve(proto, index);
        match &ic[index] {
            InlineCache::Uninitialized => {
                let cr = self.method_roots.len() as u32;
                self.method_roots.push(class);
                let mr = self.method_roots.len() as u32;
                self.method_roots.push(method);
                ic[index] = InlineCache::Method {
                    class_root: cr,
                    method_root: mr,
                };
            }
            InlineCache::Method { class_root, .. } => {
                let cached = self.method_roots[*class_root as usize];
                let same = match (cached.header(), class.header()) {
                    (Some(a), Some(b)) => a == b,
                    _ => false,
                };
                if !same {
                    ic[index] = InlineCache::Polymorphic;
                }
            }
            _ => {}
        }
    }
    /// Register a pre-linked module so `Import` can resolve it by name. The
    /// VM performs no I/O; the embedder (CLI linker, test harness) supplies
    /// every module's bytecode up front.
    pub fn register_module(&mut self, name: impl Into<String>, unit: ModuleUnit) {
        let name = name.into();
        if let Some(&idx) = self.module_index.get(&name) {
            self.modules[idx] = ModuleInstance {
                unit,
                state: ModuleState::Unloaded,
                module_id: 0,
            };
            return;
        }
        let idx = self.modules.len();
        self.modules.push(ModuleInstance {
            unit,
            state: ModuleState::Unloaded,
            module_id: 0,
        });
        self.module_index.insert(name, idx);
    }

    /// Output captured from the `print` native.
    pub fn output(&self) -> &str {
        &self.output
    }

    /// Take and clear captured `print` output.
    pub fn take_output(&mut self) -> String {
        std::mem::take(&mut self.output)
    }

    pub fn append_output(&mut self, s: &str) {
        self.output.push_str(s);
    }

    /// Adopt tier-up state from `old` into `next` when the bytecode is
    /// identical: same instructions plus bitwise-identical constants (native
    /// entries bake both as immediates). The JIT slot (entries, feedback,
    /// counters) and the inline-cache contents (per-VM shape ids and method
    /// roots, valid across runs on this VM) carry over; anything stale
    /// self-heals through guard → deopt → respecialize.
    fn adopt_slot(old: &FunctionProto, next: &Arc<FunctionProto>) {
        if old.code != next.code || old.constants != next.constants {
            return;
        }
        if next.jit.get().is_none() {
            if let Some(slot) = old.jit.get() {
                let _ = next.jit.set(slot.clone());
            }
        }
        if let (Ok(src), Ok(mut dst)) = (
            old.ic_slots.try_borrow(),
            next.ic_slots.try_borrow_mut(),
        ) {
            *dst = src.clone();
        }
    }

    /// Child-table carry: same index, identical bytecode (see [`Vm::adopt_slot`]).
    fn carry_slot(prev: &[Arc<FunctionProto>], i: usize, next: &Arc<FunctionProto>) {
        if next.jit.get().is_some() {
            return;
        }
        if let Some(old) = prev.get(i) {
            Self::adopt_slot(old, next);
        }
    }

    /// Main carry: an explicitly published source slot (tests, embeddings)
    /// wins; otherwise the previous runs' frames are searched (see
    /// [`Vm::adopt_slot`]).
    fn carry_main_slot(
        source: &FunctionProto,
        prev_mains: &[Arc<FunctionProto>],
        next: &Arc<FunctionProto>,
    ) {
        if next.jit.get().is_some() {
            return;
        }
        if source.jit.get().is_some() {
            Self::adopt_slot(source, next);
            if next.jit.get().is_some() {
                return;
            }
        }
        for old in prev_mains {
            Self::adopt_slot(old, next);
            if next.jit.get().is_some() {
                return;
            }
        }
    }

    /// Execute a module: `main` is the top-level prototype and `children` is
    /// the flat nested-prototype table used by `MakeClosure`.
    ///
    /// Returns the value of the module's final `return` (or `nil`).
    pub fn run_module(
        &mut self,
        main: &FunctionProto,
        children: &[FunctionProto],
    ) -> VmResult<Value> {
        // Tier-up state persists across runs of byte-identical prototypes on
        // one VM (the steady-state protocol): the previous run's frames still
        // hold its protos, so matching slots (and inline-cache contents, whose
        // shape ids and method roots are per-VM) carry over instead of
        // re-tiering from scratch every call. Anything that diverges (fresh
        // feedback, repopulated ICs) self-heals through the usual
        // guard → deopt → respecialize path, and byte-inequality (a different
        // module, edited code) simply starts cold.
        let prev_mains: Vec<Arc<FunctionProto>> =
            self.frames.iter().map(|f| f.proto.clone()).collect();
        let prev_children = std::mem::replace(
            &mut self.module_protos,
            vec![children
                .iter()
                .cloned()
                .map(|mut p| {
                    p.module_id = 0;
                    Arc::new(p)
                })
                .collect()],
        );
        if let (Some(prev_table), Some(next_table)) =
            (prev_children.first(), self.module_protos.first())
        {
            for (j, child) in next_table.iter().enumerate() {
                Vm::carry_slot(prev_table, j, child);
            }
        }
        self.module_globals = vec![vec![Value::nil(); main.num_globals as usize]];
        // A fresh top-level run re-executes imported modules from scratch.
        for m in &mut self.modules {
            m.state = ModuleState::Unloaded;
        }
        let mut main_owned = main.clone();
        main_owned.module_id = 0;
        let main_proto = Arc::new(main_owned);
        Vm::carry_main_slot(main, &prev_mains, &main_proto);
        let closure = new_func(main_proto.clone(), Vec::new());
        self.frames.clear();
        self.frames.push(Frame {
            proto: main_proto,
            closure,
            base: 0,
            pc: 0,
            return_reg: 0,
            ctor: None,
            cells: Vec::new(),
            handlers: Vec::new(),
            module_id: 0,
            import_idx: None,
        });
        self.cells_active = false;
        self.run()
    }

    /// Drive the interpreter, resuming after any error a `catch` clause
    /// handled. `run_inner` is the ordinary fetch/decode/execute loop; an error
    /// escaping it is offered to the innermost pending handler first and only
    /// propagates once no frame wants it.
    fn run(&mut self) -> VmResult<Value> {
        loop {
            match self.run_inner() {
                Ok(v) => return Ok(v),
                Err(e) => {
                    // Capture the failing site before `catch` unwinding moves
                    // frames around; the last record wins, which is the error
                    // that actually escapes `run`.
                    self.record_error_location();
                    match self.dispatch_error(e)? {
                        Ok(()) => {}
                        Err(e) => return Err(e),
                    }
                }
            }
        }
    }

    /// Snapshot the top frame's module, function, and source line into
    /// `error_location`. The failing instruction is at `pc - 1` (`step`
    /// advances past the fetch before executing).
    fn record_error_location(&mut self) {
        let Some(frame) = self.frames.last() else {
            return;
        };
        let pc = frame.pc.saturating_sub(1) as u32;
        let line = frame
            .proto
            .line_table
            .iter()
            .rev()
            .find(|(ins, _)| *ins <= pc)
            .map(|(_, line)| *line)
            .unwrap_or(0);
        self.error_location = Some(ErrorLocation {
            module_id: frame.module_id,
            func: frame.proto.name.clone(),
            line,
        });
    }

    /// Location of the most recent runtime error, if any frame was active.
    pub fn error_location(&self) -> Option<ErrorLocation> {
        self.error_location.clone()
    }

    /// Registry name for a runtime module id, or `None` for the entry module
    /// (which runs without registration) and native-only frames.
    pub fn module_name_by_id(&self, id: usize) -> Option<&str> {
        self.module_index
            .iter()
            .find(|(_, &idx)| idx == id)
            .map(|(name, _)| name.as_str())
    }

    fn run_inner(&mut self) -> VmResult<Value> {
        // The step budget is set before a run and never toggled mid-run, so the
        // common (unbudgeted) case gets a tight loop with no per-instruction
        // budget branch.
        if self.step_budget.is_none() {
            loop {
                match self.step()? {
                    Step::Continue => {}
                    Step::Finished(v) => return Ok(v),
                }
            }
        }
        loop {
            self.tick_budget()?;
            match self.step()? {
                Step::Continue => {}
                Step::Finished(v) => return Ok(v),
            }
        }
    }

    /// Consume one unit of the optional instruction budget, erroring when empty.
    #[inline]
    fn tick_budget(&mut self) -> VmResult<()> {
        if let Some(b) = self.step_budget.as_mut() {
            if *b == 0 {
                return Err(VmError::NativeError("step budget exhausted".into()));
            }
            *b -= 1;
        }
        Ok(())
    }

    /// Offer `err` to the innermost open `catch`, unwinding the frames above
    /// it. Returns the error back when no handler wants it.
    fn dispatch_error(&mut self, err: VmError) -> VmResult<Result<(), VmError>> {
        let err = err.clone();
        let mut depth = self.frames.len();
        while depth > 0 {
            match self.frames[depth - 1].handlers.last().copied() {
                Some(h) => {
                    // Discard the failing frames, restoring module bookkeeping.
                    while self.frames.len() > depth {
                        self.unwind_frame();
                    }
                    self.pending_error = Some(err.clone());
                    let value = new_error(Arc::from(err.to_string()), 0, Value::nil());
                    let base = self.frames[depth - 1].base;
                    self.set_reg(base, h.err_slot, value);
                    let frame = &mut self.frames[depth - 1];
                    frame.handlers.pop();
                    frame.pc = h.catch_pc;
                    return Ok(Ok(()));
                }
                None => depth -= 1,
            }
        }
        Ok(Err(err))
    }

    /// Drop the topmost frame without returning a value to it, abandoning an
    /// in-flight module import so a later `import` retries it cleanly.
    fn unwind_frame(&mut self) {
        let frame = match self.frames.pop() {
            Some(f) => f,
            None => return,
        };
        self.cells_active = match self.frames.last() {
            Some(f) => !f.cells.is_empty(),
            None => false,
        };
        if let Some(idx) = frame.import_idx {
            if matches!(self.modules[idx].state, ModuleState::Loading(_)) {
                let name = self.modules[idx].unit.main.name.clone();
                self.modules[idx].state = ModuleState::Unloaded;
                self.module_index.remove(&name);
            }
        }
    }

    /// One fetch/decode/execute step of the topmost frame.
    ///
    /// `run_inner` drives this until the frame stack empties; the callback
    /// trampoline drives it until the stack is back to the depth it started at.
    /// Both share this one implementation, so a nested call cannot drift away
    /// from the top-level loop it re-enters.
    #[inline(always)]
    fn step(&mut self) -> VmResult<Step> {
        let frame_idx = match self.frames.len().checked_sub(1) {
            Some(i) => i,
            None => return Ok(Step::Finished(Value::nil())),
        };
        let (base, pc, code_len) = {
            let frame = &self.frames[frame_idx];
            (frame.base, frame.pc, frame.proto.code.len())
        };

        if pc >= code_len {
            match self.do_return(Value::nil())? {
                Some(v) => return Ok(Step::Finished(v)),
                None => return Ok(Step::Continue),
            }
        }

        let inst = self.frames[frame_idx].proto.code[pc];
        self.frames[frame_idx].pc = pc + 1;
        let a = inst.a();
        let b = inst.b();
        let c = inst.c();
        let op = inst.op();

        match op {
            Opcode::LoadNil => self.set_reg(base, a, Value::nil()),
            Opcode::LoadTrue => self.set_reg(base, a, Value::bool(true)),
            Opcode::LoadFalse => self.set_reg(base, a, Value::bool(false)),
            Opcode::LoadConst => {
                let v = self.konst(b).ok_or_else(|| {
                    VmError::ValueError(format!("constant index {b} out of range"))
                })?;
                self.set_reg(base, a, v);
            }
            Opcode::Move => {
                let v = self.reg(base, b);
                self.set_reg(base, a, v);
            }

            Opcode::Add | Opcode::Sub | Opcode::Mul => {
                let l = self.reg(base, b);
                let r = self.reg(base, c);
                self.bump_feedback(frame_idx, pc, l.tag_bit() | r.tag_bit());
                // Monomorphic int fast path (the common case in numeric loops
                // and recursion); fall back to the shared, type-generic
                // implementation on overflow or a non-int operand. Kept in
                // lockstep with `ops::int_arith`'s checked/promote semantics.
                if let (Some(x), Some(y)) = (l.as_int(), r.as_int()) {
                    let fast = match op {
                        Opcode::Add => x.checked_add(y),
                        Opcode::Sub => x.checked_sub(y),
                        _ => x.checked_mul(y),
                    };
                    if let Some(z) = fast {
                        self.set_reg(base, a, Value::int_promote(z));
                        return Ok(Step::Continue);
                    }
                }
                let v = ops::arith(arith_of(op), l, r)?;
                self.set_reg(base, a, v);
            }
            Opcode::Div | Opcode::Mod | Opcode::Pow => {
                let l = self.reg(base, b);
                let r = self.reg(base, c);
                self.bump_feedback(frame_idx, pc, l.tag_bit() | r.tag_bit());
                let v = ops::arith(arith_of(op), l, r)?;
                self.set_reg(base, a, v);
            }
            Opcode::Neg => {
                let v = self.reg(base, b);
                self.bump_feedback(frame_idx, pc, v.tag_bit());
                let out = ops::negate(v)?;
                self.set_reg(base, a, out);
            }

            Opcode::Eq | Opcode::Ne => {
                let l = self.reg(base, b);
                let r = self.reg(base, c);
                self.bump_feedback(frame_idx, pc, l.tag_bit() | r.tag_bit());
                let eq = if let (Some(x), Some(y)) = (l.as_int(), r.as_int()) {
                    x == y
                } else {
                    ops::values_equal(l, r)
                };
                let res = if op == Opcode::Eq { eq } else { !eq };
                self.set_reg(base, a, Value::bool(res));
            }
            Opcode::Lt | Opcode::Le | Opcode::Gt | Opcode::Ge => {
                let l = self.reg(base, b);
                let r = self.reg(base, c);
                self.bump_feedback(frame_idx, pc, l.tag_bit() | r.tag_bit());
                if let (Some(x), Some(y)) = (l.as_int(), r.as_int()) {
                    let res = match op {
                        Opcode::Lt => x < y,
                        Opcode::Le => x <= y,
                        Opcode::Gt => x > y,
                        _ => x >= y,
                    };
                    self.set_reg(base, a, Value::bool(res));
                    return Ok(Step::Continue);
                }
                let res = compare_bool(op, l, r)?;
                self.set_reg(base, a, Value::bool(res));
            }

            Opcode::And => {
                let l = self.reg(base, b);
                let r = self.reg(base, c);
                self.set_reg(base, a, Value::bool(ops::truthy(l) && ops::truthy(r)));
            }
            Opcode::Or => {
                let l = self.reg(base, b);
                let r = self.reg(base, c);
                self.set_reg(base, a, Value::bool(ops::truthy(l) || ops::truthy(r)));
            }
            Opcode::Not => {
                let v = self.reg(base, b);
                self.set_reg(base, a, Value::bool(!ops::truthy(v)));
            }

            Opcode::BitAnd | Opcode::BitOr | Opcode::BitXor | Opcode::Shl | Opcode::Shr => {
                let l = self.reg(base, b);
                let r = self.reg(base, c);
                self.bump_feedback(frame_idx, pc, l.tag_bit() | r.tag_bit());
                let v = ops::bitwise(bit_of(op), l, r)?;
                self.set_reg(base, a, v);
            }
            Opcode::BitNot => {
                let v = self.reg(base, b);
                self.bump_feedback(frame_idx, pc, v.tag_bit());
                self.set_reg(base, a, ops::bitnot(v)?);
            }

            Opcode::Jump => {
                let target = jump_target(pc, b, c, code_len)?;
                self.tick_loop(frame_idx, pc, target);
                if target <= pc {
                    if let Some(step) = self.osr_transfer(frame_idx, base, target)? {
                        return Ok(step);
                    }
                }
                self.frames[frame_idx].pc = target;
            }
            Opcode::JumpIfFalse => {
                let v = self.reg(base, a);
                if !ops::truthy(v) {
                    let target = jump_target(pc, b, c, code_len)?;
                    self.tick_loop(frame_idx, pc, target);
                    if target <= pc {
                        if let Some(step) = self.osr_transfer(frame_idx, base, target)? {
                            return Ok(step);
                        }
                    }
                    self.frames[frame_idx].pc = target;
                }
            }
            Opcode::JumpIfTrue => {
                let v = self.reg(base, a);
                if ops::truthy(v) {
                    let target = jump_target(pc, b, c, code_len)?;
                    self.tick_loop(frame_idx, pc, target);
                    if target <= pc {
                        if let Some(step) = self.osr_transfer(frame_idx, base, target)? {
                            return Ok(step);
                        }
                    }
                    self.frames[frame_idx].pc = target;
                }
            }

            Opcode::Call => {
                let argc = b as usize;
                let first = c as usize;
                if first == 0 {
                    return Err(VmError::NativeError("Call with zero first operand".into()));
                }
                let callee = self.reg(base, (first - 1) as u8);
                self.bump_feedback(frame_idx, pc, callee.tag_bit());
                // Foreign callable: straight through the bridge (which owns
                // its own monomorphic IC keyed by handle — no VM IC traffic).
                if let Some(handle) = callee.as_foreign() {
                    use crate::pybridge::BridgeOp;
                    let args = self.py_args(base, first, argc)?;
                    let bridge = crate::pybridge::bridge_of(self).map_err(|message| {
                        VmError::ForeignCall {
                            message,
                            traceback: None,
                        }
                    })?;
                    let v = crate::pybridge::bridge_value(bridge.call(handle, args), BridgeOp::Call)?;
                    self.set_reg(base, a, v);
                    return Ok(Step::Continue);
                }
                if callee.as_class().is_some() {
                    let inst = new_instance(vec![callee]);
                    match ops::find_class_method(callee, Value::string("init")) {
                        Some(init) => {
                            let proto = init
                                .func_proto()
                                .ok_or_else(|| VmError::type_mismatch("function", init))?;
                            if argc < proto.num_required as usize || argc > proto.params.len() {
                                return Err(VmError::ArgumentCount {
                                    expected: proto.params.len(),
                                    got: argc,
                                });
                            }
                            // reg0 of the `init` frame becomes `self`.
                            self.set_reg(base, (first - 1) as u8, inst);
                            self.push_frame(proto, init, base + first - 1, a, argc + 1)?;
                            self.frames.last_mut().unwrap().ctor = Some(inst);
                        }
                        None => {
                            if argc != 0 {
                                return Err(VmError::ArgumentCount {
                                    expected: 0,
                                    got: argc,
                                });
                            }
                            self.set_reg(base, a, inst);
                        }
                    }
                    return Ok(Step::Continue);
                }
                let callee_proto = callee
                    .func_proto()
                    .ok_or_else(|| VmError::type_mismatch("function", callee))?;
                if argc < callee_proto.num_required as usize || argc > callee_proto.params.len() {
                    return Err(VmError::ArgumentCount {
                        expected: callee_proto.params.len(),
                        got: argc,
                    });
                }
                crate::jit::note_call(self.jit.as_ref(), &callee_proto);
                match crate::jit::invoke_if_compiled(self, &callee_proto, callee, base, first) {
                    crate::jit::InvokeOutcome::Compiled(result) => {
                        self.set_reg(base, a, result);
                        return Ok(Step::Continue);
                    }
                    crate::jit::InvokeOutcome::Resume(pc) => {
                        // Native code spilled its registers and asked to
                        // resume here; the window already holds the state.
                        self.push_frame_resume(callee_proto, callee, base + first, a)?;
                        self.frames.last_mut().unwrap().pc = pc;
                        return Ok(Step::Continue);
                    }
                    crate::jit::InvokeOutcome::Fallback => {}
                }
                self.push_frame(callee_proto, callee, base + first, a, argc)?;
            }
            Opcode::CallNative => {
                let argc = c as usize;
                let first = (a as usize)
                    .checked_sub(argc)
                    .ok_or_else(|| VmError::NativeError("native arg window underflow".into()))?;
                let mut args = Vec::with_capacity(argc);
                for i in 0..argc {
                    args.push(self.reg(base, (first + i) as u8));
                }
                let v = self.call_native(b as u16, &args)?;
                self.set_reg(base, a, v);
            }
            Opcode::CallNativeW => {
                let argc = b as usize;
                let payload = self.frames[frame_idx]
                    .proto
                    .code
                    .get(pc + 1)
                    .copied()
                    .ok_or_else(|| {
                        VmError::NativeError("CallNativeW missing payload word".into())
                    })?;
                // Skip the payload word so the next fetch is the following op.
                self.frames.last_mut().unwrap().pc = pc + 2;
                let first = (a as usize)
                    .checked_sub(argc)
                    .ok_or_else(|| VmError::NativeError("native arg window underflow".into()))?;
                let mut args = Vec::with_capacity(argc);
                for i in 0..argc {
                    args.push(self.reg(base, (first + i) as u8));
                }
                let v = self.call_native(payload.native_wide_id(), &args)?;
                self.set_reg(base, a, v);
            }
            Opcode::CallMethod => {
                let argc = c as usize;
                let name = self.konst(b).ok_or_else(|| {
                    VmError::ValueError(format!("method name constant {b} out of range"))
                })?;
                let name_str = name
                    .as_str()
                    .ok_or_else(|| VmError::type_mismatch("string", name))?;
                let recv_first = (a as usize)
                    .checked_sub(argc + 1)
                    .ok_or_else(|| VmError::NativeError("method receiver underflow".into()))?;
                let recv = self.reg(base, recv_first as u8);
                // Foreign dispatch: one fused bridge round-trip
                // (`call_attr` resolves and calls server-side).
                if let Some(handle) = recv.as_foreign() {
                    use crate::pybridge::BridgeOp;
                    let args = self.py_args(base, recv_first + 1, argc)?;
                    let bridge = crate::pybridge::bridge_of(self).map_err(|message| {
                        VmError::ForeignCall {
                            message,
                            traceback: None,
                        }
                    })?;
                    let v = crate::pybridge::bridge_value(
                        bridge.call_attr(handle, name_str, args),
                        BridgeOp::Call,
                    )?;
                    self.set_reg(base, a, v);
                    return Ok(Step::Continue);
                }
                if let Some(slots) = recv.as_instance() {
                    let class = slots
                        .first()
                        .copied()
                        .ok_or_else(|| VmError::KeyError(name_str.to_string()))?;
                    let method_val = match self.ic_resolve_method(
                        &self.frames[frame_idx].proto,
                        pc,
                        class,
                    ) {
                        Some(m) => m,
                        None => {
                            let m = ops::find_class_method(class, Value::string(name_str))
                                .ok_or_else(|| VmError::KeyError(name_str.to_string()))?;
                            let p = self.frames[frame_idx].proto.clone();
                            self.ic_update_method(&p, pc, class, m);
                            m
                        }
                    };
                    let proto = method_val
                        .func_proto()
                        .ok_or_else(|| VmError::type_mismatch("function", method_val))?;
                    if argc < proto.num_required as usize || argc > proto.params.len() {
                        return Err(VmError::ArgumentCount {
                            expected: proto.params.len(),
                            got: argc,
                        });
                    }
                    // reg0 of the method frame is the receiver (`self`).
                    crate::jit::note_call(self.jit.as_ref(), &proto);
                    match crate::jit::invoke_if_compiled(self, &proto, method_val, base, recv_first) {
                        crate::jit::InvokeOutcome::Compiled(result) => {
                            self.set_reg(base, a, result);
                            return Ok(Step::Continue);
                        }
                        crate::jit::InvokeOutcome::Resume(pc) => {
                            self.push_frame_resume(proto, method_val, base + recv_first, a)?;
                            self.frames.last_mut().unwrap().pc = pc;
                            return Ok(Step::Continue);
                        }
                        crate::jit::InvokeOutcome::Fallback => {}
                    }
                    self.push_frame(proto, method_val, base + recv_first, a, argc + 1)?;
                    return Ok(Step::Continue);
                }
                // Slower paths need the method name by value.
                let method = name_str.to_string();
                // Module namespaces are plain dicts, so `math.floor(x)`
                // resolves a plain function stored under the key. Such a
                // function has no `self`, so the frame starts at the first
                // argument (mirroring the method layout one slot lower).
                if let Some(map) = recv.as_dict() {
                    if let Some(f) = ops::dict_get(map, Value::string(method.clone())) {
                        if let Some(proto) = f.func_proto() {
                            if argc < proto.num_required as usize || argc > proto.params.len() {
                                return Err(VmError::ArgumentCount {
                                    expected: proto.params.len(),
                                    got: argc,
                                });
                            }
                            crate::jit::note_call(self.jit.as_ref(), &proto);
                            match crate::jit::invoke_if_compiled(
                                self,
                                &proto,
                                f,
                                base,
                                a as usize - argc,
                            ) {
                                crate::jit::InvokeOutcome::Compiled(result) => {
                                    self.set_reg(base, a, result);
                                    return Ok(Step::Continue);
                                }
                                crate::jit::InvokeOutcome::Resume(pc) => {
                                    self.push_frame_resume(
                                        proto,
                                        f,
                                        base + (a as usize - argc),
                                        a,
                                    )?;
                                    self.frames.last_mut().unwrap().pc = pc;
                                    return Ok(Step::Continue);
                                }
                                crate::jit::InvokeOutcome::Fallback => {}
                            }
                            self.push_frame(proto, f, base + (a as usize - argc), a, argc)?;
                            return Ok(Step::Continue);
                        }
                    }
                }
                let mut args = Vec::with_capacity(argc);
                for i in 0..argc {
                    args.push(self.reg(base, (a as usize - argc + i) as u8));
                }
                // After instance methods and module namespaces, exactly as the
                // oracle orders them: a user type's own `map` still wins.
                let v = match self.higher_order_method(&method, recv, &args)? {
                    HigherOrder::Value(v) => v,
                    HigherOrder::Aborted => return Ok(Step::Continue),
                    HigherOrder::NotOurs => {
                        self.call_method(base, recv_first as u8, &method, &args)?
                    }
                };
                self.set_reg(base, a, v);
            }

            Opcode::Return => {
                let v = self.reg(base, a);
                if let Some(result) = self.do_return(v)? {
                    return Ok(Step::Finished(result));
                }
            }

            Opcode::MakeList | Opcode::MakeSet | Opcode::MakeTuple => {
                let count = b as usize;
                let first = c as usize;
                let mut items = Vec::with_capacity(count);
                for i in 0..count {
                    items.push(self.reg(base, (first + i) as u8));
                }
                let v = match op {
                    Opcode::MakeList => new_list(items),
                    Opcode::MakeSet => new_set(items.into_iter().collect()),
                    _ => new_tuple(items),
                };
                self.set_reg(base, a, v);
            }
            Opcode::MakeDict => {
                let pairs = b as usize;
                let first = c as usize;
                let mut map: IndexMap<DictKey, Value> = IndexMap::new();
                for i in 0..pairs {
                    let k = self.reg(base, (first + i * 2) as u8);
                    let val = self.reg(base, (first + i * 2 + 1) as u8);
                    ops::dict_insert(&mut map, k, val);
                }
                self.set_reg(base, a, new_dict(map));
            }

            Opcode::GetIndex => {
                let obj = self.reg(base, b);
                let key = self.reg(base, c);
                let v = ops::get_index(obj, key)?;
                self.set_reg(base, a, v);
            }
            Opcode::GetField | Opcode::GetProperty => {
                let obj = self.reg(base, b);
                self.bump_feedback(frame_idx, pc, obj.tag_bit());
                let name = self.konst(c).ok_or_else(|| {
                    VmError::ValueError(format!("field constant {c} out of range"))
                })?;
                // Foreign attribute read (no VM IC traffic — the bridge
                // caches `(handle, attr)` resolutions itself).
                if let Some(handle) = obj.as_foreign() {
                    use crate::pybridge::BridgeOp;
                    let name_str = name
                        .as_str()
                        .ok_or_else(|| VmError::type_mismatch("string", name))?;
                    let bridge = crate::pybridge::bridge_of(self).map_err(|message| {
                        VmError::ForeignAttribute {
                            message,
                            traceback: None,
                        }
                    })?;
                    let v = crate::pybridge::bridge_value(
                        bridge.getattr(handle, name_str),
                        BridgeOp::GetAttr,
                    )?;
                    self.set_reg(base, a, v);
                    return Ok(Step::Continue);
                }
                let v = match self.ic_get_field(&self.frames[frame_idx].proto, pc, obj) {
                    Some(v) => v,
                    None => {
                        let v = ops::get_field(obj, name)?;
                        self.ic_update_get(&self.frames[frame_idx].proto, pc, obj, name);
                        v
                    }
                };
                self.set_reg(base, a, v);
            }
            Opcode::SetIndex => {
                let key = self.reg(base, b);
                let val = self.reg(base, c);
                let obj = self.reg(base, a);
                let obj = ops::set_index(obj, key, val)?;
                self.set_reg(base, a, obj);
            }
            Opcode::SetField | Opcode::SetProperty => {
                let name = self.konst(b).ok_or_else(|| {
                    VmError::ValueError(format!("field constant {b} out of range"))
                })?;
                let val = self.reg(base, c);
                let obj = self.reg(base, a);
                self.bump_feedback(frame_idx, pc, obj.tag_bit() | val.tag_bit());
                // Foreign attribute write; the result is the receiver,
                // matching the interpreter's fast path.
                if let Some(handle) = obj.as_foreign() {
                    use crate::pybridge::BridgeOp;
                    let name_str = name
                        .as_str()
                        .ok_or_else(|| VmError::type_mismatch("string", name))?;
                    let pv = crate::pybridge::value_to_pyvalue(val).map_err(|e| {
                        crate::pybridge::into_vm_error(e, BridgeOp::Marshal)
                    })?;
                    let bridge = crate::pybridge::bridge_of(self).map_err(|message| {
                        VmError::ForeignAttribute {
                            message,
                            traceback: None,
                        }
                    })?;
                    bridge
                        .setattr(handle, name_str, pv)
                        .map_err(|e| crate::pybridge::into_vm_error(e, BridgeOp::SetAttr))?;
                    self.set_reg(base, a, obj);
                    return Ok(Step::Continue);
                }
                let v = if self.ic_set_field(&self.frames[frame_idx].proto, pc, obj, val) {
                    obj
                } else {
                    let result = ops::set_field(obj, name, val)?;
                    let p = self.frames[frame_idx].proto.clone();
                    self.ic_update_set(&p, pc, result, name);
                    result
                };
                self.set_reg(base, a, v);
            }
            Opcode::IterInit => {
                let seq = self.reg(base, b);
                self.set_reg(base, a, seq);
                self.set_reg(base, a.wrapping_add(1), Value::int(0));
            }
            Opcode::IterNext => {
                let seq = self.reg(base, a);
                let cursor = self.reg(base, a.wrapping_add(1)).as_int().unwrap_or(0);
                let item = ops::iter_get(seq, cursor)?;
                self.set_reg(base, a.wrapping_add(1), Value::int(cursor + 1));
                self.set_reg(base, b, item);
            }
            Opcode::UnpackCheck => {
                let item = self.reg(base, a);
                if ops::iter_len(item) < b as usize {
                    let msg = self
                        .konst(c)
                        .map(|v| ops::stringify(v))
                        .unwrap_or_else(|| String::from("not enough values to unpack"));
                    return Err(VmError::ValueError(msg));
                }
            }
            Opcode::IterLen => {
                let len = ops::iter_len(self.reg(base, b));
                self.set_reg(base, a, Value::int(len as i64));
            }
            Opcode::Slice => {
                // The two bounds sit in consecutive registers from `c`, `nil`
                // for the ones the source omitted. A bound that is present but
                // not an `int` reads as `0`, which is what the oracle does when
                // it evaluates the bound with `as_int().unwrap_or(0)` — worth
                // keeping, or the two implementations would disagree on a
                // float bound.
                let bound = |reg: u8| -> Option<i64> { self.reg(base, reg).as_int() };
                let start = bound(c);
                let end = bound(c + 1);
                let out = ops::slice(self.reg(base, b), start, end)?;
                self.set_reg(base, a, out);
            }
            Opcode::ChanMake => {
                let ch = crate::gc::new_channel();
                self.set_reg(base, a, ch);
            }
            Opcode::ChanSend => {
                let ch = self.reg(base, a);
                let v = self.reg(base, b);
                match crate::gc::RefCountGc::safe_channel_send(ch, v) {
                    Ok(()) => self.set_reg(base, a, Value::bool(true)),
                    Err(_) => {
                        self.set_reg(base, b, v);
                        self.set_reg(base, a, Value::bool(false));
                    }
                }
            }
            Opcode::ChanRecv => {
                let ch = self.reg(base, a);
                match crate::gc::RefCountGc::safe_channel_recv(ch) {
                    Ok(v) => {
                        self.set_reg(base, a, Value::bool(true));
                        self.set_reg(base, b, v);
                    }
                    Err(_) => {
                        self.set_reg(base, a, Value::bool(false));
                        self.set_reg(base, b, Value::nil());
                    }
                }
            }
            Opcode::Spawn => {
                // Spawn: full implementation pending - for now, return nil
                self.set_reg(base, a, Value::nil());
            }
            Opcode::IterIndex => {
                let item = self.reg(base, b);
                let v = ops::iter_get(item, c as i64)?;
                self.set_reg(base, a, v);
            }
            Opcode::ListAppend => {
                let item = self.reg(base, b);
                let mut list = self.reg(base, a);
                if !matches!(list.kind(), Some(ObjKind::List)) {
                    return Err(VmError::type_mismatch("list", list));
                }
                crate::value::cow(&mut list);
                list.as_list_mut().push(item);
                self.set_reg(base, a, list);
            }
            Opcode::MakeRange => {
                let lo = self.reg(base, b);
                let hi = self.reg(base, c);
                // Both ends must be integers, and the error names the left
                // operand even when it is the right one that is not — the
                // oracle binds `(lv, rv)` in one `let` and reports `lv`.
                let (Some(start), Some(end)) = (lo.as_int(), hi.as_int()) else {
                    return Err(VmError::type_mismatch("int", lo));
                };
                let items: Vec<Value> = if end >= start {
                    (start..=end).map(Value::int_promote).collect()
                } else {
                    Vec::new()
                };
                self.set_reg(base, a, new_list(items));
            }
            Opcode::IterDone => {
                let seq = self.reg(base, a);
                let cursor = self.reg(base, a.wrapping_add(1)).as_int().unwrap_or(0);
                let len = ops::iter_len(seq) as i64;
                self.set_reg(base, b, Value::bool(cursor >= len));
            }

            Opcode::MakeClosure => {
                let proto_idx = b as usize;
                let count = c as usize;
                let mid = self.frames.last().map(|f| f.module_id).unwrap_or(0);
                let child = self
                    .module_protos
                    .get(mid)
                    .and_then(|c| c.get(proto_idx))
                    .cloned()
                    .ok_or_else(|| {
                        VmError::NativeError(format!(
                            "prototype index {proto_idx} out of range for module {mid}"
                        ))
                    })?;
                // Captured locals become shared *cells*: the register is
                // promoted in place, so the defining frame, every sibling
                // closure and the closure itself all observe writes. That
                // also makes recursion work without a special case, since
                // the closure is written into its own cell after this.
                let frame_idx = self.frames.len() - 1;
                let mut upvals = Vec::with_capacity(count);
                for desc in child.upvalue_descriptors.iter().take(count) {
                    if desc.is_local {
                        upvals.push(self.promote_local(frame_idx, desc.index));
                    } else {
                        let held = self.frames[frame_idx]
                            .closure
                            .func_upvals()
                            .and_then(|u| u.get(desc.index as usize).copied())
                            .ok_or_else(|| {
                                VmError::NativeError(format!(
                                    "upvalue index {} out of range",
                                    desc.index
                                ))
})?;
                        upvals.push(match held.as_cell() {
                            Some(_) => held,
                            None => new_cell(held),
                        });
                    }
                }
                let closure = new_func(child, upvals);
                self.set_reg(base, a, closure);
            }
            Opcode::MakeClosureRO => {
                // Read-only capture: the compiler proved (whole-module) that
                // no store can reach a taken slot, so values are snapshotted
                // instead of promoting locals to shared cells. One allocation
                // (the closure) instead of two, and no cell indirection.
                let proto_idx = b as usize;
                let count = c as usize;
                let mid = self.frames.last().map(|f| f.module_id).unwrap_or(0);
                let child = self
                    .module_protos
                    .get(mid)
                    .and_then(|c| c.get(proto_idx))
                    .cloned()
                    .ok_or_else(|| {
                        VmError::NativeError(format!(
                            "prototype index {proto_idx} out of range for module {mid}"
                        ))
                    })?;
                let frame_idx = self.frames.len() - 1;
                let mut upvals = Vec::with_capacity(count);
                for desc in child.upvalue_descriptors.iter().take(count) {
                    if desc.is_local {
                        upvals.push(self.local_value(frame_idx, desc.index));
                    } else {
                        // A non-local source is already a snapshot or a cell
                        // owned by an ancestor; either is safe to share, since
                        // the proof covers the transitive chain.
                        let held = self.frames[frame_idx]
                            .closure
                            .func_upvals()
                            .and_then(|u| u.get(desc.index as usize).copied())
                            .ok_or_else(|| {
                                VmError::NativeError(format!(
                                    "upvalue index {} out of range",
                                    desc.index
                                ))
                            })?;
                        upvals.push(held);
                    }
                }
                let closure = new_func(child, upvals);
                self.set_reg(base, a, closure);
            }
            Opcode::GetUpval => {
                let idx = b as usize;
                let closure = self
                    .frames
                    .last()
                    .map(|f| f.closure)
                    .unwrap_or(Value::nil());
                let held = closure
                    .func_upvals()
                    .and_then(|u| u.get(idx).copied())
                    .ok_or_else(|| {
                        VmError::NativeError(format!("upvalue index {idx} out of range"))
                    })?;
                let v = match held.as_cell() {
                    Some(cell) => *cell.borrow(),
                    None => held,
                };
                self.set_reg(base, a, v);
            }
            Opcode::SetUpval => {
                let idx = a as usize;
                let v = self.reg(base, b);
                if let Some(frame) = self.frames.last_mut() {
                    if let Some(upvals) = frame.closure.func_upvals_mut() {
                        if idx < upvals.len() {
                            match upvals[idx].as_cell() {
                                Some(cell) => *cell.borrow_mut() = v,
                                None => upvals[idx] = v,
                            }
                        } else {
                            return Err(VmError::NativeError(format!(
                                "upvalue index {idx} out of range"
                            )));
                        }
                    }
                }
            }

            Opcode::TryEnter => {
                let target = jump_target(pc, b, c, code_len)?;
                let frame = self.frames.last_mut().unwrap();
                frame.handlers.push(Handler {
                    catch_pc: target,
                    err_slot: a,
                });
            }
            Opcode::TryExit => {
                if let Some(frame) = self.frames.last_mut() {
                    frame.handlers.pop();
                }
            }
            Opcode::ErrorKind => {
                let kind = match &self.pending_error {
                    Some(e) => e.kind(),
                    None => "Error",
                };
                self.set_reg(base, a, Value::string(kind));
            }
            Opcode::Rethrow => {
                return Err(match self.pending_error.take() {
                    Some(e) => e,
                    None => VmError::UserError("rethrow without a pending error".into()),
                });
            }
            Opcode::Throw => {
                let v = self.reg(base, a);
                return Err(VmError::UserError(ops::stringify(v)));
            }
            Opcode::ThrowVal => {
                let v = self.reg(base, a);
                return Err(VmError::UserError(ops::stringify(v)));
            }
            Opcode::Nop => {}

            // Module globals live in heap-backed per-module storage
            // (`module_globals[frame.module_id]`), not the register file,
            // so a nested function shares the module's state without
            // consuming registers or capturing by value.
            Opcode::GetGlobal => {
                let mid = self.frames.last().map(|f| f.module_id).unwrap_or(0);
                let v = self
                    .module_globals
                    .get(mid)
                    .and_then(|g| g.get(b as usize))
                    .copied()
                    .ok_or_else(|| VmError::ValueError(format!("global index {b} out of range")))?;
                self.set_reg(base, a, v);
            }
            Opcode::SetGlobal => {
                let mid = self.frames.last().map(|f| f.module_id).unwrap_or(0);
                let v = self.reg(base, b);
                let slot = self
                    .module_globals
                    .get_mut(mid)
                    .and_then(|g| g.get_mut(a as usize))
                    .ok_or_else(|| VmError::ValueError(format!("global index {a} out of range")))?;
                *slot = v;
            }

            // Resolve a module by name and write its export dictionary to
            // `a`. `Unloaded` modules run once (state `Loading` guards
            // cycles); the same dictionary reference is returned to every
            // importer, so mutually recursive modules observe a live view.
            Opcode::Import => {
                let name = self
                    .konst(b)
                    .and_then(|v| v.as_str().map(str::to_string))
                    .ok_or_else(|| {
                        VmError::ValueError(format!("module name constant {b} out of range"))
                    })?;
                let idx = *self
                    .module_index
                    .get(&name)
                    .ok_or_else(|| VmError::NativeError(format!("module '{name}' not found")))?;
                match self.modules[idx].state {
                    ModuleState::Ready(dict) | ModuleState::Loading(dict) => {
                        self.set_reg(base, a, dict);
                    }
                    ModuleState::Unloaded => {
                        let num_globals = self.modules[idx].unit.main.num_globals;
                        let module_id = self.module_globals.len();
                        self.module_globals
                            .push(vec![Value::nil(); num_globals as usize]);
                        // Constant exports are pre-filled so their slots never
                        // consume a register-window slot in the module proto.
                        for (slot, value) in &self.modules[idx].unit.const_globals {
                            self.module_globals[module_id][*slot as usize] = *value;
                        }
                        let child_arcs: Vec<Arc<FunctionProto>> = self.modules[idx]
                            .unit
                            .children
                            .iter()
                            .cloned()
                            .map(|mut p| {
                                p.module_id = module_id as u32;
                                Arc::new(p)
                            })
                            .collect();
                        self.module_protos.push(child_arcs);

                        let dict = new_dict(IndexMap::new());
                        self.modules[idx].state = ModuleState::Loading(dict);
                        self.modules[idx].module_id = module_id;

                        let mut main_owned = self.modules[idx].unit.main.clone();
                        main_owned.module_id = module_id as u32;
                        let main_proto = Arc::new(main_owned);
                        let closure = new_func(main_proto.clone(), Vec::new());
                        // The module runs in the free window directly above
                        // the destination register, leaving the caller's
                        // live registers untouched.
                        let import_base = base + a as usize + 1;
                        if import_base + main_proto.max_reg as usize > REGISTERS {
                            return Err(VmError::StackOverflow(
                                "register file exhausted importing module".into(),
                            ));
                        }
                        self.frames.push(Frame {
                            proto: main_proto,
                            closure,
                            base: import_base,
                            pc: 0,
                            return_reg: a,
                            ctor: Some(dict),
                            cells: Vec::new(),
                            handlers: Vec::new(),
                            module_id,
                            import_idx: Some(idx),
                        });
                        self.cells_active = false;
                    }
                }
            }

            Opcode::PyImport => {
                // Foreign import: never touches the Zen module linker. The
                // module path rides in the constant pool exactly like
                // `Import`; the bridge owns resolution, caching, and lifetime.
                let name = self
                    .konst(b)
                    .and_then(|v| v.as_str().map(str::to_string))
                    .ok_or_else(|| {
                        VmError::ValueError(format!("module name constant {b} out of range"))
                    })?;
                let bridge = crate::pybridge::bridge_of(self).map_err(|message| {
                    VmError::PyImport {
                        message,
                        traceback: None,
                    }
                })?;
                let handle = bridge
                    .import_module(&name)
                    .map_err(|e| {
                        crate::pybridge::into_vm_error(e, crate::pybridge::BridgeOp::Import)
                    })?;
                self.set_reg(base, a, crate::gc::new_foreign(handle));
            }

            Opcode::MakeClass => {
                let name = self.konst(b).ok_or_else(|| {
                    VmError::ValueError(format!("class name constant {b} out of range"))
                })?;
                let name_str = name
                    .as_str()
                    .ok_or_else(|| VmError::type_mismatch("string", name))?
                    .to_string();
                let parent = if c == 0xFF {
                    Value::nil()
                } else {
                    self.reg(base, c)
                };
                let methods = new_dict(IndexMap::new());
                let class = new_class(Arc::from(name_str), parent, methods);
                self.set_reg(base, a, class);
            }
            Opcode::AddMethod => {
                let name = self.konst(b).ok_or_else(|| {
                    VmError::ValueError(format!("method name constant {b} out of range"))
                })?;
                let class = self.reg(base, a);
                let closure = self.reg(base, c);
                let cls = class
                    .as_class()
                    .ok_or_else(|| VmError::type_mismatch("class", class))?;
                let mut methods = cls.methods;
                if methods.as_dict().is_some() {
                    ops::dict_insert(methods.as_dict_mut(), name, closure);
                }
                self.set_reg(base, a, class);
            }
            Opcode::AddField => {
                return Err(VmError::NativeError("AddField is reserved".into()));
            }

            other => {
                return Err(VmError::NativeError(format!(
                    "unimplemented opcode {other:?}"
                )));
            }
        }
        // Every arm above mutated state and fell through to the next `step`.
        Ok(Step::Continue)
    }

    /// The cell boxing the running frame's local `idx`, if an inner closure
    /// captured it. Cells are tracked per frame rather than recognised in the
    /// register itself: register windows are reused by successive frames, so a
    /// stale cell must never be mistaken for a live capture.
    #[inline]
    fn live_cell(&self, base: usize, idx: u8) -> Option<Value> {
        let f = self.frames.last()?;
        if f.base != base {
            return None;
        }
        f.cells.get(idx as usize).copied().flatten()
    }

    /// Copy constant `idx` out of the current frame's prototype.
    ///
    /// Returns an owned [`Value`] so the immutable borrow of `frames` ends with
    /// the call, letting the caller mutate registers afterwards. This is what
    /// lets `step` read constants without cloning the frame's `Arc<FunctionProto>`
    /// on every instruction.
    #[inline]
    fn konst(&self, idx: u8) -> Option<Value> {
        self.frames
            .last()
            .and_then(|f| f.proto.constants.get(idx as usize))
            .copied()
    }

    /// Marshal `argc` window registers starting at `first` into bridge
    /// arguments. Non-marshalable values (instances, funcs, …) fail here —
    /// loudly, rather than as lossy coercions on the far side.
    fn py_args(
        &self,
        base: usize,
        first: usize,
        argc: usize,
    ) -> Result<Vec<crate::pybridge::PyValue>, VmError> {
        let mut out = Vec::with_capacity(argc);
        for i in 0..argc {
            let v = self.reg(base, (first + i) as u8);
            out.push(crate::pybridge::value_to_pyvalue(v).map_err(|e| {
                crate::pybridge::into_vm_error(e, crate::pybridge::BridgeOp::Marshal)
            })?);
        }
        Ok(out)
    }

    /// Read a register. A *captured* local reads through its shared cell, so the
    /// defining frame and every closure agree on the value.
    #[inline]
    fn reg(&self, base: usize, idx: u8) -> Value {
        let raw = self.registers[base + idx as usize];
        if !self.cells_active {
            return raw;
        }
        match self.live_cell(base, idx) {
            Some(cell) => match cell.as_cell() {
                Some(c) => *c.borrow(),
                None => raw,
            },
            None => raw,
        }
    }

    /// Write a register, writing *through* the frame's cell when the target is a
    /// captured local.
    #[inline]
    fn set_reg(&mut self, base: usize, idx: u8, v: Value) {
        if self.cells_active {
            if let Some(cell) = self.live_cell(base, idx) {
                if v.kind() != Some(ObjKind::Cell) {
                    if let Some(c) = cell.as_cell() {
                        *c.borrow_mut() = v;
                        return;
                    }
                }
            }
        }
        self.registers[base + idx as usize] = v;
    }

    /// Record per-PC type feedback for the JIT (Stage 3). Inert when the JIT
    /// is off: a single `is_some` check. `frame_idx`/`pc` identify the site,
    /// `bits` is the OR of observed [`Value::tag_bit`]s.
    #[inline]
    fn bump_feedback(&self, frame_idx: usize, pc: usize, bits: u32) {
        if self.jit.is_none() {
            return;
        }
        let Some(frame) = self.frames.get(frame_idx) else {
            return;
        };
        crate::jit::record_feedback(self.jit.as_ref(), &frame.proto, pc, bits);
    }

    /// Loop back-edge tick for OSR (Stage 4). `target <= pc` means a backward
    /// jump just executed; count it toward [`crate::jit::OSR_THRESHOLD`].
    #[inline]
    fn tick_loop(&self, frame_idx: usize, pc: usize, target: usize) {
        if target > pc {
            return;
        }
        if self.jit.is_none() {
            return;
        }
        let Some(frame) = self.frames.get(frame_idx) else {
            return;
        };
        crate::jit::note_loop(self.jit.as_ref(), &frame.proto);
    }

    /// Read one of `frame_idx`'s locals, following its cell if it has one.
    fn local_value(&self, frame_idx: usize, slot: u16) -> Value {
        let f = &self.frames[frame_idx];
        let raw = self.registers[f.base + slot as usize];
        match f.cells.get(slot as usize).copied().flatten() {
            Some(cell) => match cell.as_cell() {
                Some(c) => *c.borrow(),
                None => raw,
            },
            None => raw,
        }
    }

    /// Promote a captured local of `frame_idx` to a shared cell and return it.
    /// Repeated captures of the same local return the same cell.
    pub(crate) fn promote_local(&mut self, frame_idx: usize, slot: u16) -> Value {
        let slot = slot as usize;
        if let Some(Some(cell)) = self.frames[frame_idx].cells.get(slot) {
            return *cell;
        }
        let cell = new_cell(self.local_value(frame_idx, slot as u16));
        let cells = &mut self.frames[frame_idx].cells;
        if cells.len() <= slot {
            cells.resize(slot + 1, None);
        }
        cells[slot] = Some(cell);
        self.cells_active = true;
        cell
    }

    pub(crate) fn module_children(&self, mid: usize) -> Option<&[Arc<FunctionProto>]> {
        self.module_protos.get(mid).map(|v| v.as_slice())
    }

    /// Step the dispatch loop until the frame stack is back to `target_depth`.
    ///
    /// This is the re-entrant half of the callback trampoline. A native that
    /// must call Zen code pushes a frame and then drives *this* loop — the same
    /// `step` the top level uses, not a copy — until that frame returns. Errors
    /// are offered to the callback's own handlers first, exactly as at the top
    /// level, so a `try`/`catch` written inside the callback is handled inside
    /// it. An unhandled error discards the callback frame and propagates, so
    /// the interrupted frame finds its own handlers with a clean stack.
    fn run_until_depth(&mut self, target_depth: usize) -> VmResult<CallbackOutcome> {
        loop {
            if self.frames.len() <= target_depth {
                return Ok(CallbackOutcome::Returned);
            }
            self.tick_budget()?;
            match self.step() {
                Ok(Step::Continue) => {}
                Ok(Step::Finished(_)) => {
                    // Only an outermost `return` empties the stack, and the
                    // interrupted frame is still there, so this cannot happen.
                    return Err(VmError::NativeError(
                        "callback unwound the frame stack".into(),
                    ));
                }
                Err(e) => match self.dispatch_error(e)? {
                    // Handled. If it was handled *above* the callback the
                    // frame is already gone and the interrupted frame's `pc`
                    // now points into its `catch` block — the callback's result
                    // register is meaningless, so the caller must not touch it.
                    Ok(()) if self.frames.len() <= target_depth => {
                        return Ok(CallbackOutcome::Aborted)
                    }
                    Ok(()) => {}
                    Err(e) => {
                        while self.frames.len() > target_depth {
                            self.unwind_frame();
                        }
                        return Err(e);
                    }
                },
            }
        }
    }

    /// Call a closure value from inside a native, re-entrantly.
    ///
    /// The callback's registers are parked above everything the interrupted
    /// frame uses — locals *and* temporaries — so a native that loops (see
    /// `higher_order_method`) can call back any number of times without the
    /// callee clobbering the receiver or the values the caller is holding
    /// across the call. The result lands in the callback's own slot 0, which
    /// `do_return` writes because `return_reg` indexes the *caller's* window.
    fn call_value(&mut self, callee: Value, args: &[Value]) -> VmResult<Option<Value>> {
        let proto = callee
            .func_proto()
            .ok_or_else(|| VmError::type_mismatch("function", callee))?;
        if args.len() < proto.num_required as usize || args.len() > proto.params.len() {
            return Err(VmError::ArgumentCount {
                expected: proto.params.len(),
                got: args.len(),
            });
        }
        let target = self.frames.len();
        let (base, return_reg) = {
            let caller = self
                .frames
                .last()
                .ok_or_else(|| VmError::StackOverflow("no frame to call back from".into()))?;
            // `MAX_REGISTERS` is 250 in the compiler, so this always fits the
            // `u8` return register.
            (
                caller.base + caller.proto.max_reg as usize,
                caller.proto.max_reg as u8,
            )
        };
        if base + proto.max_reg as usize > REGISTERS {
            return Err(VmError::StackOverflow("register file exhausted".into()));
        }
        for (i, arg) in args.iter().enumerate() {
            self.registers[base + i] = *arg;
        }
        self.push_frame(proto, callee, base, return_reg, args.len())?;
        if let CallbackOutcome::Aborted = self.run_until_depth(target)? {
            return Ok(None);
        }
        Ok(Some(self.registers[base]))
    }

    /// `map`, `filter`, `reduce`, `each` and `for_each` over any iterable.
    ///
    /// These need Zen callables, so they live on the VM rather than in
    /// `ops::method_call`; the oracle keeps them in the interpreter for the same
    /// reason. Iteration goes through `iter_len`/`iter_get`, which is what makes
    /// `xs.map(f)` work on a dictionary (keys) or a string (characters) exactly
    /// as it does in the oracle. Returns `None` for every other method name so
    /// the caller can fall through to the ordinary method table.
    fn higher_order_method(
        &mut self,
        method: &str,
        seq: Value,
        args: &[Value],
    ) -> VmResult<HigherOrder> {
        if !matches!(method, "map" | "filter" | "reduce" | "each" | "for_each") {
            return Ok(HigherOrder::NotOurs);
        }
        let f = *args.first().ok_or(VmError::ArgumentCount {
            expected: 1,
            got: 0,
        })?;
        if f.func_proto().is_none() {
            return Err(VmError::type_mismatch("function", f));
        }
        let len = ops::iter_len(seq) as i64;
        match method {
            "map" => {
                let mut out = Vec::with_capacity(len.max(0) as usize);
                for i in 0..len {
                    match self.call_value(f, &[ops::iter_get(seq, i)?])? {
                        Some(v) => out.push(v),
                        None => return Ok(HigherOrder::Aborted),
                    }
                }
                Ok(HigherOrder::Value(new_list(out)))
            }
            "filter" => {
                let mut out = Vec::new();
                for i in 0..len {
                    let item = ops::iter_get(seq, i)?;
                    match self.call_value(f, &[item])? {
                        Some(keep) => {
                            if ops::truthy(keep) {
                                out.push(item);
                            }
                        }
                        None => return Ok(HigherOrder::Aborted),
                    }
                }
                Ok(HigherOrder::Value(new_list(out)))
            }
            "reduce" => {
                let (mut acc, start) = match args.get(1) {
                    Some(init) => (*init, 0),
                    None => {
                        if len == 0 {
                            return Err(VmError::ValueError(
                                "reduce of empty sequence with no initial value".into(),
                            ));
                        }
                        (ops::iter_get(seq, 0)?, 1)
                    }
                };
                for i in start..len {
                    let item = ops::iter_get(seq, i)?;
                    match self.call_value(f, &[acc, item])? {
                        Some(v) => acc = v,
                        None => return Ok(HigherOrder::Aborted),
                    }
                }
                Ok(HigherOrder::Value(acc))
            }
            _ => {
                for i in 0..len {
                    if self.call_value(f, &[ops::iter_get(seq, i)?])?.is_none() {
                        return Ok(HigherOrder::Aborted);
                    }
                }
                Ok(HigherOrder::Value(Value::nil()))
            }
        }
    }

    fn push_frame(
        &mut self,
        proto: Arc<FunctionProto>,
        closure: Value,
        base: usize,
        return_reg: u8,
        supplied: usize,
    ) -> VmResult<()> {
        if self.frames.len() >= MAX_FRAMES {
            return Err(VmError::StackOverflow("call stack exceeded".into()));
        }
        if base + proto.max_reg as usize > REGISTERS {
            return Err(VmError::StackOverflow("register file exhausted".into()));
        }
        // Register windows are reused across calls, so every local the caller
        // did not supply must start as nil. Default parameters read their own
        // slot to detect omission, and an unwritten read of a fresh local would
        // otherwise observe a previous call's leftovers. `supplied` counts the
        // leading slots the caller filled (receiver included for methods).
        for slot in supplied..proto.num_locals as usize {
            self.registers[base + slot] = Value::nil();
        }
        let module_id = proto.module_id as usize;
        // Captured-local cells are allocated lazily: the common frame never
        // captures, and pre-sizing this to `num_locals` costs a heap allocation
        // on every call (dominating recursion).
        self.frames.push(Frame {
            cells: Vec::new(),
            proto,
            closure,
            base,
            pc: 0,
            return_reg,
            ctor: None,
            handlers: Vec::new(),
            module_id,
            import_idx: None,
        });
        self.cells_active = false;
        Ok(())
    }

    /// Push a frame whose register window is already populated — used when
    /// resuming after a JIT deopt (native code spilled its registers and named
    /// the resume pc). Unlike [`Vm::push_frame`] nothing is nilled: every slot
    /// the resumed code reads was written before the guard fired.
    fn push_frame_resume(
        &mut self,
        proto: Arc<FunctionProto>,
        closure: Value,
        base: usize,
        return_reg: u8,
    ) -> VmResult<()> {
        if self.frames.len() >= MAX_FRAMES {
            return Err(VmError::StackOverflow("call stack exceeded".into()));
        }
        if base + proto.max_reg as usize > REGISTERS {
            return Err(VmError::StackOverflow("register file exhausted".into()));
        }
        let module_id = proto.module_id as usize;
        self.frames.push(Frame {
            cells: Vec::new(),
            proto,
            closure,
            base,
            pc: 0,
            return_reg,
            ctor: None,
            handlers: Vec::new(),
            module_id,
            import_idx: None,
        });
        self.cells_active = false;
        Ok(())
    }

    /// OSR transfer at a backward jump to `target` (Stage 4). When the current
    /// function has a published OSR entry for `target`, enter it with the live
    /// register window instead of interpreting the next iteration — the same
    /// frame continues, natively on success (`Return` completes it) or back in
    /// the interpreter at the resume pc on a guard failure.
    ///
    /// Returns `Ok(None)` when no transfer happened (caller sets `pc` and
    /// keeps interpreting).
    #[allow(unsafe_code)]
    fn osr_transfer(
        &mut self,
        frame_idx: usize,
        base: usize,
        target: usize,
    ) -> VmResult<Option<Step>> {
        if self.jit.is_none() {
            return Ok(None);
        }
        let (proto, closure_bits) = match self.frames.get(frame_idx) {
            Some(f) => (f.proto.clone(), f.closure.to_bits()),
            None => return Ok(None),
        };
        if base + proto.max_reg as usize > REGISTERS {
            return Ok(None);
        }
        let Some(entry) = crate::jit::osr_entry(&proto, target) else {
            return Ok(None);
        };
        self.osr_transfers += 1;
        let mut out = Value::nil();
        let regs = self.registers.as_mut_ptr() as *mut u64;
        let vm_ptr = self as *mut Vm;
        // SAFETY: same contract as `invoke_if_compiled`; the interpreter owns
        // `regs[base..]` and keeps it live, and the window was bounds-checked.
        let status = unsafe {
            entry(
                regs,
                base,
                vm_ptr,
                &mut out as *mut Value as *mut u64,
                closure_bits,
            )
        };
        if status == 0 {
            return Ok(Some(match self.do_return(out)? {
                Some(v) => Step::Finished(v),
                None => Step::Continue,
            }));
        }
        // A guard fired mid-loop: count the deopt (re-specialize on overflow)
        // and resume this same frame at the named pc — no push, the window
        // already holds the spilled registers.
        if crate::jit::note_deopt(&proto) {
            crate::jit::respecialize(self.jit.as_ref(), &proto);
        }
        let resume = status.saturating_sub(2) as usize;
        let len = self
            .frames
            .get(frame_idx)
            .map(|f| f.proto.code.len())
            .unwrap_or(0);
        self.frames[frame_idx].pc = resume.min(len);
        Ok(Some(Step::Continue))
    }

    fn do_return(&mut self, v: Value) -> VmResult<Option<Value>> {
        let frame = match self.frames.pop() {
            Some(f) => f,
            None => return Ok(Some(v)),
        };
        self.cells_active = match self.frames.last() {
            Some(f) => !f.cells.is_empty(),
            None => false,
        };
        let mut v = frame.ctor.unwrap_or(v);
        // A finished module import publishes its exports: zip the manifest
        // names with the module's heap globals into the shared dictionary.
        if let Some(idx) = frame.import_idx {
            let exports = self.modules[idx].unit.exports.clone();
            for (name, slot) in exports {
                let gv = self
                    .module_globals
                    .get(frame.module_id)
                    .and_then(|g| g.get(slot as usize))
                    .copied()
                    .unwrap_or(Value::nil());
                ops::dict_insert(v.as_dict_mut(), Value::string(name), gv);
            }
            self.modules[idx].state = ModuleState::Ready(v);
        }
        match self.frames.last() {
            Some(caller) => {
                let idx = caller.base + frame.return_reg as usize;
                self.registers[idx] = v;
                Ok(None)
            }
            None => Ok(Some(v)),
        }
    }

    pub(crate) fn call_native(&mut self, id: u16, args: &[Value]) -> VmResult<Value> {
        ops::native_call(id, args, &mut self.output)
    }

    fn call_method(
        &mut self,
        base: usize,
        receiver: u8,
        method: &str,
        args: &[Value],
    ) -> VmResult<Value> {
        let mut recv = self.reg(base, receiver);
        let v = ops::method_call(&mut recv, method, args)?;
        self.set_reg(base, receiver, recv);
        Ok(v)
    }
}

// ── operand decoding ─────────────────────────────────────────────────────────

fn jump_target(pc: usize, b: u8, c: u8, len: usize) -> VmResult<usize> {
    let offset = i16::from_be_bytes([b, c]) as i64;
    let target = pc as i64 + 1 + offset;
    if target < 0 || target as usize > len {
        return Err(VmError::NativeError(format!(
            "jump target {target} out of range"
        )));
    }
    Ok(target as usize)
}

// ── opcode → shared-op mapping ───────────────────────────────────────────────

fn arith_of(op: Opcode) -> Arith {
    match op {
        Opcode::Add => Arith::Add,
        Opcode::Sub => Arith::Sub,
        Opcode::Mul => Arith::Mul,
        Opcode::Div => Arith::Div,
        Opcode::Mod => Arith::Mod,
        Opcode::Pow => Arith::Pow,
        _ => unreachable!("arith_of called with non-arithmetic opcode"),
    }
}

fn bit_of(op: Opcode) -> Bit {
    match op {
        Opcode::BitAnd => Bit::And,
        Opcode::BitOr => Bit::Or,
        Opcode::BitXor => Bit::Xor,
        Opcode::Shl => Bit::Shl,
        Opcode::Shr => Bit::Shr,
        _ => unreachable!("bit_of called with non-bitwise opcode"),
    }
}

fn compare_bool(op: Opcode, l: Value, r: Value) -> VmResult<bool> {
    let ord = match ops::compare_order(l, r) {
        Ok(ord) => ord,
        // Ordering against NaN is unordered: every comparison is false.
        Err(_) if l.is_number() && r.is_number() => return Ok(false),
        Err(e) => return Err(e),
    };
    Ok(match op {
        Opcode::Lt => ord == Ordering::Less,
        Opcode::Le => ord != Ordering::Greater,
        Opcode::Gt => ord == Ordering::Greater,
        Opcode::Ge => ord != Ordering::Less,
        _ => unreachable!("compare_bool called with non-comparison opcode"),
    })
}

#[cfg(all(test, feature = "jit"))]
mod jit_layout_tests {
    use super::*;

    /// Pins the `Vec<Value>` word order the native global-access path depends
    /// on. If a toolchain ever reorders it, this fails loudly instead of
    /// miscompiling.
    #[test]
    fn vec_value_layout() {
        assert_eq!(std::mem::size_of::<Vec<Value>>(), 24);
        let v = vec![Value::int(1), Value::int(2), Value::int(3)];
        let base = &v as *const Vec<Value> as *const u8;
        // SAFETY: `base` points at the live vec for these reads.
        unsafe {
            // Observed order on this toolchain: `{len, ptr, cap}`.
            assert_eq!(*(base as *const usize), 3);
            let ptr = *(base.add(8) as *const *const Value);
            assert!(*(base.add(16) as *const usize) >= 3);
            assert_eq!((*ptr.add(1)).as_int(), Some(2));
        }
        assert!(Vm::module_globals_off() < std::mem::size_of::<Vm>());
        // The offset must agree with the live field address, exactly.
        let vm = Vm::new(Box::new(crate::gc::RefCountGc));
        let base = &vm as *const Vm as usize;
        let field = &vm.module_globals as *const Vec<Vec<Value>> as usize;
        assert_eq!(Vm::module_globals_off(), field - base);
    }
}
