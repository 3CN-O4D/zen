//! Lexical scoping, name resolution, and register/slot assignment.
//!
//! M4 follows the ruling in `docs/07-bytecode.md` §6.2: **locals are fixed** —
//! every binding gets a register at its declaration point and keeps it for the
//! rest of the function (no local-register reuse). Temporaries come from a
//! stack-discipline free list governed by [`Resolver::mark`] /
//! [`Resolver::free_to`], so expression nesting depth is the temporary count.

use std::collections::HashMap;

/// How a resolved name is stored in the current function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindKind {
    /// A `let` / `const` / hidden binding in this function.
    Local,
    /// A formal parameter in this function.
    Param,
    /// Captured from an enclosing function.
    Upvalue,
}

#[derive(Debug, Clone, Copy)]
pub struct Binding {
    pub slot: u16,
    pub kind: BindKind,
    pub is_const: bool,
}

/// One enclosing loop, for `break` / `continue` fixup.
#[derive(Debug, Clone)]
pub struct LoopCtx {
    /// Instruction index of the loop head (`continue` target).
    pub head: usize,
    /// PCs of `break` jumps, patched to the loop exit.
    pub breaks: Vec<usize>,
    /// PCs of `continue` jumps, patched to `head`.
    pub continues: Vec<usize>,
}

/// Register-file budget: `u8` operands and the reserved top six registers per
/// `docs/07-bytecode.md` §2.2 imply at most 250 usable slots.
pub const MAX_REGISTERS: u16 = 250;

/// Per-function scope and register state.
#[derive(Debug, Clone)]
pub struct Resolver {
    scopes: Vec<HashMap<String, Binding>>,
    /// Upvalues of the current function, by name.
    pub upvalue_names: HashMap<String, u16>,
    /// Upvalue descriptors, parallel to `upvalue_names` insertion order.
    pub pending_upvalues: Vec<zen_vm::bytecode::UpvalueDesc>,
    /// Number of permanently allocated registers (params + locals + hidden).
    pub num_locals: u16,
    /// Temporaries currently live, above `num_locals`.
    pub temp_count: u16,
    /// High-water mark of `num_locals + temp_count`.
    pub max_reg: u16,
    /// Enclosing loops, innermost last.
    pub loops: Vec<LoopCtx>,
    /// First register of the pool of reserved slots (see `reserve_hidden`).
    reserved_base: u16,
    /// Size of that pool.
    reserved_count: u16,
    /// Slots currently handed out, i.e. the pool's live nesting depth.
    reserved_used: u16,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            upvalue_names: HashMap::new(),
            pending_upvalues: Vec::new(),
            num_locals: 0,
            temp_count: 0,
            max_reg: 0,
            loops: Vec::new(),
            reserved_base: 0,
            reserved_count: 0,
            reserved_used: 0,
        }
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Pop the current scope's names. Registers are **not** reclaimed (M4
    /// fixed-local ruling).
    pub fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn record(&mut self, name: String, binding: Binding) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, binding);
        }
    }

    /// Allocate a permanent local slot. Fails only at the register ceiling.
    pub fn declare_local(&mut self, name: String, is_const: bool) -> Option<u16> {
        let slot = self.declare_hidden()?;
        self.record(
            name,
            Binding {
                slot,
                kind: BindKind::Local,
                is_const,
            },
        );
        Some(slot)
    }

    /// Allocate a parameter slot (never const).
    pub fn declare_param(&mut self, name: String) -> Option<u16> {
        let slot = self.declare_hidden()?;
        self.record(
            name,
            Binding {
                slot,
                kind: BindKind::Param,
                is_const: false,
            },
        );
        Some(slot)
    }

    /// Allocate a compiler-generated slot with no source name (loop iterators,
    /// match subjects, etc.).
    ///
    /// Takes `num_locals`, so it reuses the register of a temporary that the
    /// caller is about to move into it. Callers must therefore allocate the
    /// slot *before* evaluating the expression whose temp has to survive — the
    /// rule the comprehension accumulator's comment spells out.
    pub fn declare_hidden(&mut self) -> Option<u16> {
        // Never hand out a reserved slot to a local: locals and temporaries
        // both live above the pool.
        let pool_top = self.reserved_base + self.reserved_count;
        if self.num_locals < pool_top {
            self.num_locals = pool_top;
        }
        if self.num_locals >= MAX_REGISTERS {
            return None;
        }
        let slot = self.num_locals;
        self.num_locals += 1;
        self.bump_high_water();
        Some(slot)
    }

    /// Reserve a permanent slot for a construct that must hold registers across
    /// its own inner scopes — a match subject and result, a comprehension
    /// accumulator.
    ///
    /// Such a slot cannot be an ordinary local: `declare_hidden` takes
    /// `num_locals`, which an enclosing expression's temporaries are still
    /// using (`f(1) + [x for x in xs]` has `f(1)` live in exactly that
    /// register). Spilling those temporaries to locals on *every* construct is
    /// what exhausted a module root's register file, since a module root is a
    /// single capped frame and the spill cost repeated.
    ///
    /// So the slots come from a pool with stack discipline: `reserve_hidden`
    /// hands out the next free slot and `release_hidden` gives it back, which a
    /// nested construct takes and its parent returns. Sibling matches and
    /// comprehensions therefore share one register each, and the pool only ever
    /// grows to the deepest nesting actually reached. The first reservation does
    /// spill, once.
    pub fn reserve_hidden(&mut self) -> Option<u16> {
        if self.reserved_used < self.reserved_count {
            let slot = self.reserved_base + self.reserved_used;
            self.reserved_used += 1;
            return Some(slot);
        }
        if self.temp_count > 0 {
            self.num_locals += self.temp_count;
            self.temp_count = 0;
        }
        let pool_top = self.reserved_base + self.reserved_count;
        if self.num_locals < pool_top {
            self.num_locals = pool_top;
        }
        if self.num_locals >= MAX_REGISTERS {
            return None;
        }
        if self.reserved_count == 0 {
            self.reserved_base = self.num_locals;
        }
        let slot = self.num_locals;
        self.num_locals += 1;
        self.reserved_count = self.num_locals - self.reserved_base;
        self.reserved_used += 1;
        self.bump_high_water();
        Some(slot)
    }

    /// Return the most recently reserved slot to the pool.
    pub fn release_hidden(&mut self) {
        self.reserved_used = self.reserved_used.saturating_sub(1);
    }

    /// Number of active lexical scopes; `1` means the function's root scope.
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Spill every live temporary to a local slot.
    ///
    /// A construct that must keep registers alive across its own inner scopes
    /// (a match subject, a comprehension accumulator) needs them to be
    /// permanent, and `declare_hidden` takes `num_locals` — which an enclosing
    /// temporary is still using. Spilling once, on entry, gives those slots a
    /// clear base; doing it per binding would compound, turning each binding's
    /// accumulated temporaries into permanent locals.
    pub fn spill_temps(&mut self) {
        self.num_locals += self.temp_count;
        self.temp_count = 0;
    }

    /// The first register above every live temporary — the "floor" mark that
    /// `free_to` releases back down to.
    ///
    /// Constructures that need registers to survive their own inner scopes (a
    /// match subject, a comprehension accumulator) capture this after
    /// allocating those registers, then free back to it per iteration instead
    /// of calling `free_all_temps`, which would recycle the registers they are
    /// holding.
    pub fn temp_top(&self) -> u16 {
        self.num_locals + self.temp_count
    }

    pub fn resolve_local(&self, name: &str) -> Option<Binding> {
        for scope in self.scopes.iter().rev() {
            if let Some(b) = scope.get(name) {
                return Some(*b);
            }
        }
        None
    }

    pub fn resolve_upvalue(&self, name: &str) -> Option<u16> {
        self.upvalue_names.get(name).copied()
    }

    /// Register an upvalue of this function, deduplicating by source.
    pub fn add_upvalue(&mut self, name: String, is_local: bool, index: u16) -> Option<u16> {
        if let Some(slot) = self.upvalue_names.get(&name) {
            return Some(*slot);
        }
        let slot = self.upvalue_names.len() as u16;
        self.upvalue_names.insert(name, slot);
        self.upvalue_guard(slot, is_local, index);
        Some(slot)
    }

    /// Keep the descriptor list parallel to `upvalue_names` insertion order.
    fn upvalue_guard(&mut self, slot: u16, is_local: bool, index: u16) {
        self.pending_upvalues
            .push(zen_vm::bytecode::UpvalueDesc { is_local, index });
        debug_assert_eq!(self.pending_upvalues.len() as u16, slot + 1);
    }

    // ── temporaries ──────────────────────────────────────────────────────
    /// The current top of the register file; pass to [`free_to`].
    pub fn mark(&self) -> u16 {
        self.num_locals + self.temp_count
    }

    /// Release every temporary allocated at or above `mark`.
    pub fn free_to(&mut self, mark: u16) {
        let base = self.num_locals;
        self.temp_count = mark.saturating_sub(base);
    }

    /// Release every temporary at or above `reg` (which must be a temporary).
    pub fn free_since(&mut self, reg: u8) {
        let base = self.num_locals;
        let reg = u16::from(reg);
        if reg >= base {
            self.temp_count = reg - base;
        }
    }

    pub fn alloc_temp(&mut self) -> Option<u8> {
        self.alloc_temps(1)
    }

    pub fn alloc_temps(&mut self, n: u16) -> Option<u8> {
        let base = self.num_locals + self.temp_count;
        if base + n > MAX_REGISTERS {
            return None;
        }
        self.temp_count += n;
        self.bump_high_water();
        Some(base as u8)
    }

    fn bump_high_water(&mut self) {
        let top = self.num_locals + self.temp_count;
        if top > self.max_reg {
            self.max_reg = top;
        }
    }

    // ── loops ────────────────────────────────────────────────────────────
    pub fn enter_loop(&mut self, head: usize) {
        self.loops.push(LoopCtx {
            head,
            breaks: Vec::new(),
            continues: Vec::new(),
        });
    }

    pub fn exit_loop(&mut self) -> Option<LoopCtx> {
        self.loops.pop()
    }

    pub fn in_loop(&self) -> bool {
        !self.loops.is_empty()
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}
