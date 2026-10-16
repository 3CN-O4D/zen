//! Cranelift lowering for Zen's integer subset (JIT tier 1).
//!
//! A compiled function has the [`NativeEntry`](zen_vm::jit::NativeEntry) ABI:
//! `(regs: *mut u64, base: usize, vm: *mut Vm, out: *mut u64, closure: u64)`
//! returning `0` on success or `1` to deopt. Registers live in SSA while native
//! code runs; the register *window* in memory is written only where the ABI
//! requires it (argument spill before a call, and the entry snapshot that a
//! deopt restores).
//!
//! Only operations whose result can be computed directly on `Value` bits are
//! lowered. Anything else — non-integral arithmetic, property access, closures,
//! optional parameters — makes the whole function ineligible, so it stays
//! interpreted and the result is always correct. An ineligible function is a
//! compile *error*, which marks its slot `Failed` and never retries it.

use std::collections::{BTreeMap, BTreeSet};

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    types, AbiParam, Block, BlockArg, FuncRef, InstBuilder, MemFlagsData, SigRef, Signature,
    StackSlot, StackSlotData, StackSlotKind, Type, UserFuncName, Value as ClValue,
};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, Linkage, Module};

use zen_vm::bytecode::{Instruction, Opcode};
use zen_vm::jit::{CompileSnapshot, ZenJitCompiler};
use zen_vm::value::Value;

// ── Value bit patterns (mirrors `zen_vm::value`) ────────────────────────────

const NIL: u64 = 0x7FF8_0000_0000_0000;
const FALSE: u64 = 0x7FF9_0000_0000_0000;
const TRUE: u64 = 0x7FFA_0000_0000_0000;
const INT_HDR: u64 = 0x7FFB_0000_0000_0000;
/// The `int` header as it appears in the top 16 bits (`bits >> 48`).
const INT_TAG_TOP: i64 = 0x7FFB;
/// The `heap` header's top 16 bits (`QNAN | 0b111 << 48`).
const HEAP_TAG_TOP: i64 = 0x7FFF;
const PAYLOAD_MASK: i64 = 0x0000_FFFF_FFFF_FFFF;
const INT_MAX: i64 = (1i64 << 47) - 1;
const INT_MIN: i64 = -(1i64 << 47);

/// A Cranelift JIT module shared by one worker thread.
pub struct CraneliftCompiler {
    module: JITModule,
    fbctx: FunctionBuilderContext,
    ctx: Context,
    seq: u64,
    addr_get_global: u64,
    addr_call: u64,
    addr_get_field: u64,
    addr_set_field: u64,
    addr_get_upval: u64,
    addr_set_upval: u64,
    addr_set_global: u64,
    addr_make_dict: u64,
    addr_get_index: u64,
    addr_set_index: u64,
    addr_resolve: u64,
    addr_mcall: u64,
    addr_getf_mono: u64,
    addr_setf_mono: u64,
    addr_mklist: u64,
    addr_mkclass: u64,
    addr_addm: u64,
    addr_mkclo0: u64,
    addr_callnative: u64,
    addr_iternext: u64,
    addr_iterdone: u64,
    addr_push: u64,
    addr_mkcloro: u64,
    addr_call_cached: u64,
    addr_stradd: u64,
}

impl CraneliftCompiler {
    pub fn new() -> Self {
        // `opt_level=speed` turns on cranelift's optimizer; the JIT default is
        // none, which leaves naive (spill-heavy) code. This is the difference
        // between a baseline JIT and one whose numbers are worth reporting.
        let builder = JITBuilder::with_flags(
            &[("opt_level", "speed")],
            default_libcall_names(),
        )
        .expect("create JIT builder");
        let module = JITModule::new(builder);
        Self {
            module,
            fbctx: FunctionBuilderContext::new(),
            ctx: Context::new(),
            seq: 0,
            addr_get_global: zen_vm::jit_rt::zen_jit_get_global as *const () as usize as u64,
            addr_call: zen_vm::jit_rt::zen_jit_call as *const () as usize as u64,
            addr_get_field: zen_vm::jit_rt::zen_jit_ic_get_field as *const () as usize as u64,
            addr_set_field: zen_vm::jit_rt::zen_jit_ic_set_field as *const () as usize as u64,
            addr_get_upval: zen_vm::jit_rt::zen_jit_get_upval as *const () as usize as u64,
            addr_set_upval: zen_vm::jit_rt::zen_jit_set_upval as *const () as usize as u64,
            addr_set_global: zen_vm::jit_rt::zen_jit_set_global as *const () as usize as u64,
            addr_make_dict: zen_vm::jit_rt::zen_jit_make_dict as *const () as usize as u64,
            addr_get_index: zen_vm::jit_rt::zen_jit_get_index as *const () as usize as u64,
            addr_set_index: zen_vm::jit_rt::zen_jit_set_index as *const () as usize as u64,
            addr_resolve: zen_vm::jit_rt::zen_jit_resolve_method as *const () as usize as u64,
            addr_mcall: zen_vm::jit_rt::zen_jit_method_call as *const () as usize as u64,
            addr_getf_mono: zen_vm::jit_rt::zen_jit_get_field_mono as *const () as usize as u64,
            addr_setf_mono: zen_vm::jit_rt::zen_jit_set_field_mono as *const () as usize as u64,
            addr_mklist: zen_vm::jit_rt::zen_jit_make_list as *const () as usize as u64,
            addr_mkclass: zen_vm::jit_rt::zen_jit_make_class as *const () as usize as u64,
            addr_addm: zen_vm::jit_rt::zen_jit_add_method as *const () as usize as u64,
            addr_mkclo0: zen_vm::jit_rt::zen_jit_make_closure0 as *const () as usize as u64,
            addr_callnative: zen_vm::jit_rt::zen_jit_call_native as *const () as usize as u64,
            addr_iternext: zen_vm::jit_rt::zen_jit_iter_next as *const () as usize as u64,
            addr_iterdone: zen_vm::jit_rt::zen_jit_iter_done as *const () as usize as u64,
            addr_push: zen_vm::jit_rt::zen_jit_method_push as *const () as usize as u64,
            addr_mkcloro: zen_vm::jit_rt::zen_jit_make_closure_ro as *const () as usize as u64,
            addr_call_cached: zen_vm::jit_rt::zen_jit_call_cached as *const () as usize as u64,
            addr_stradd: zen_vm::jit_rt::zen_jit_str_add as *const () as usize as u64,
        }
    }

    fn compile_impl(&mut self, snap: &CompileSnapshot) -> Result<zen_vm::jit::CompileOutput, String> {
        check_eligible(snap)?;

        let mut sig = Signature::new(self.module.isa().default_call_conv());
        for _ in 0..5 {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I32));

        // Normal entry plus one OSR entry per loop header (backup-edge
        // target). Each entry shares the same ABI and body; an OSR entry
        // loads the full register window and starts at its header block, so
        // the interpreter can transfer into native code mid-loop.
        let headers = loop_headers(snap);
        let mut entries: Vec<(Option<usize>, String)> = vec![(None, format!("zen_jit_{}", self.seq))];
        self.seq += 1;
        for h in &headers {
            entries.push((Some(*h), format!("zen_osr_{}_{}", self.seq, h)));
        }
        self.seq += 1;

        let mut out_entry: *const u8 = std::ptr::null();
        let mut out_osr: Vec<(u32, *const u8)> = Vec::new();
        // Per-`Call`-site cache cells `(callee_bits, entry_addr, max_reg)`,
        // in code order, shared by the normal and OSR entries. Leaked for
        // the life of the process, like the generated code that references
        // the cells (respecialization leaks one small box per recompile).
        let call_count = snap
            .code
            .iter()
            .filter(|i| i.op() == Opcode::Call)
            .count();
        let call_cells: u64 = if call_count == 0 {
            0
        } else {
            let cells: Box<[u64]> = vec![0u64; call_count * 3].into_boxed_slice();
            Box::leak(cells).as_mut_ptr() as u64
        };
        for (osr_pc, name) in &entries {
            let id = self
                .module
                .declare_function(name, Linkage::Local, &sig)
                .map_err(|e| e.to_string())?;

            self.ctx.clear();
            self.ctx.func.signature = sig.clone();
            self.ctx.func.name = UserFuncName::user(0, id.as_u32());
            let self_ref = self.module.declare_func_in_func(id, &mut self.ctx.func);

            {
                let mut b = FunctionBuilder::new(&mut self.ctx.func, &mut self.fbctx);
                Lower::new(
                    &mut b,
                    snap,
                    self_ref,
                    self.addr_get_global,
                    self.addr_call,
                    self.addr_get_field,
                    self.addr_set_field,
                    self.addr_get_upval,
                    self.addr_set_upval,
                    self.addr_set_global,
                    self.addr_make_dict,
                    self.addr_get_index,
                    self.addr_set_index,
                    self.addr_resolve,
                    self.addr_mcall,
                    self.addr_getf_mono,
                    self.addr_setf_mono,
                    self.addr_mklist,
                    self.addr_mkclass,
                    self.addr_addm,
                    self.addr_mkclo0,
                    self.addr_callnative,
                    self.addr_iternext,
                    self.addr_iterdone,
                    self.addr_push,
                    self.addr_mkcloro,
                    self.addr_call_cached,
                    self.addr_stradd,
                    call_cells,
                    *osr_pc,
                )?
                .run()?;
                b.seal_all_blocks();
                b.finalize(self.module.target_config());
            }

            self.module
                .define_function(id, &mut self.ctx)
                .map_err(|e| e.to_string())?;
            self.module
                .finalize_definitions()
                .map_err(|e| e.to_string())?;
            let addr = self.module.get_finalized_function(id);
            match osr_pc {
                None => out_entry = addr,
                Some(h) => out_osr.push((*h as u32, addr)),
            }
        }
        Ok(zen_vm::jit::CompileOutput {
            entry: out_entry,
            osr: out_osr,
        })
    }
}

impl Default for CraneliftCompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl ZenJitCompiler for CraneliftCompiler {
    fn compile(&mut self, snap: &CompileSnapshot) -> Result<zen_vm::jit::CompileOutput, String> {
        self.compile_impl(snap)
    }
}

fn check_eligible(snap: &CompileSnapshot) -> Result<(), String> {
    if snap.num_upvalues != 0 {
        // Allow upvalue-bearing functions; we'll lower GetUpval/SetUpval/MakeClosure
        // if they appear. Individual unsupported ops still rejected.
    }
    if snap.num_required != snap.params.len() as u16 {
        return Err("optional parameters are not lowered".into());
    }
    if snap.max_reg == 0 || snap.max_reg as usize > 256 {
        return Err("bad register window".into());
    }
    match snap.code.last().map(|i| i.op()) {
        Some(Opcode::Return) => {}
        _ => return Err("function does not end in Return".into()),
    }
    for inst in &snap.code {
        match inst.op() {
            Opcode::LoadNil
            | Opcode::LoadTrue
            | Opcode::LoadFalse
            | Opcode::LoadConst
            | Opcode::Move
            | Opcode::Add
            | Opcode::Sub
            | Opcode::Mul
            | Opcode::Div
            | Opcode::Mod
            | Opcode::Neg
            | Opcode::BitAnd
            | Opcode::BitOr
            | Opcode::BitXor
            | Opcode::BitNot
            | Opcode::Shl
            | Opcode::Shr
            | Opcode::Eq
            | Opcode::Ne
            | Opcode::Lt
            | Opcode::Le
            | Opcode::Gt
            | Opcode::Ge
            | Opcode::And
            | Opcode::Or
            | Opcode::Not
            | Opcode::Jump
            | Opcode::JumpIfFalse
            | Opcode::JumpIfTrue
            | Opcode::Call
            | Opcode::Return
            | Opcode::GetGlobal
            | Opcode::SetGlobal
            | Opcode::GetField
            | Opcode::SetField
            | Opcode::GetProperty
            | Opcode::SetProperty
            | Opcode::GetIndex
            | Opcode::SetIndex
            | Opcode::MakeDict
            | Opcode::MakeList
            | Opcode::MakeSet
            | Opcode::MakeTuple
            | Opcode::MakeClass
            | Opcode::AddMethod
            | Opcode::CallMethod
            | Opcode::IterInit
            | Opcode::IterNext
            | Opcode::IterDone
            | Opcode::CallNative
            | Opcode::GetUpval
            | Opcode::SetUpval => {}
            // `MakeClosure` without captures needs no defining frame (there is
            // nothing to capture): the module id travels as an explicit
            // parameter. Capturing definers stay interpreted, where capture is
            // exact.
            Opcode::MakeClosure if inst.c() == 0 => {}
            Opcode::MakeClosure => return Err("capturing MakeClosure stays interpreted".into()),
            Opcode::MakeClosureRO => {}
            other => return Err(format!("opcode {other:?} is not lowered")),
        }
    }
    Ok(())
}

struct Lower<'a, 'b> {
    b: &'a mut FunctionBuilder<'b>,
    code: &'a [Instruction],
    constants: &'a [Value],
    module_id: usize,
    num_globals: usize,
    max_reg: usize,
    num_required: usize,
    is_method: bool,
    self_ref: FuncRef,
    addr_gg: u64,
    addr_call: u64,
    addr_getf: u64,
    addr_setf: u64,
    addr_getu: u64,
    addr_setu: u64,
    addr_setg: u64,
    addr_mkd: u64,
    addr_geti: u64,
    addr_seti: u64,
    addr_resolve: u64,
    addr_mcall: u64,
    addr_getf_mono: u64,
    addr_setf_mono: u64,
    addr_mklist: u64,
    addr_mkclass: u64,
    addr_addm: u64,
    addr_mkclo0: u64,
    addr_callnative: u64,
    addr_iternext: u64,
    addr_iterdone: u64,
    addr_push: u64,
    addr_mkcloro: u64,
    addr_call_cached: u64,
    addr_stradd: u64,
    /// Leaked base pointer of the per-`Call`-site cache cells
    /// `(callee_bits, entry_addr, max_reg)` triples, in code order (0 when
    /// the function makes no calls). The box lives forever, like the
    /// generated code that references it.
    call_cells: u64,
    /// Next `Call` site index (code order, matching the cell layout).
    call_site: usize,
    p_regs: ClValue,
    p_base: ClValue,
    p_vm: ClValue,
    p_out: ClValue,
    p_closure: ClValue,
    sig_gg: SigRef,
    sig_call: SigRef,
    sig_ic: SigRef,
    sig_mkclo: SigRef,
    sig_mkd: SigRef,
    sig_mcall: SigRef,
    sig_native: SigRef,
    out_slot: StackSlot,
    out_slot2: StackSlot,
    blocks: BTreeMap<usize, Block>,
    deopt: Block,
    regs: Vec<ClValue>,
    cur: Option<Block>,
    terminated: bool,
    /// Per-PC feedback snapshot for specialization (Stage 3).
    feedback: &'a [zen_vm::jit::FeedbackEntry],
    /// Per-PC inline-cache snapshot for shape specialization (Stage 3).
    ic_slots: &'a [zen_vm::ic::InlineCache],
    /// OSR entry header: when `Some(h)`, load the full window and start at
    /// the loop-header block `h` instead of falling through at pc 0.
    osr_entry: Option<usize>,
}

impl<'a, 'b> Lower<'a, 'b> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        b: &'a mut FunctionBuilder<'b>,
        snap: &'a CompileSnapshot,
        self_ref: FuncRef,
        addr_gg: u64,
        addr_call: u64,
        addr_getf: u64,
        addr_setf: u64,
        addr_getu: u64,
        addr_setu: u64,
        addr_setg: u64,
        addr_mkd: u64,
        addr_geti: u64,
        addr_seti: u64,
        addr_resolve: u64,
        addr_mcall: u64,
        addr_getf_mono: u64,
        addr_setf_mono: u64,
        addr_mklist: u64,
        addr_mkclass: u64,
        addr_addm: u64,
        addr_mkclo0: u64,
        addr_callnative: u64,
        addr_iternext: u64,
        addr_iterdone: u64,
        addr_push: u64,
        addr_mkcloro: u64,
        addr_call_cached: u64,
        addr_stradd: u64,
        call_cells: u64,
        osr_entry: Option<usize>,
    ) -> Result<Self, String> {
        let max_reg = snap.max_reg as usize;
        let num_required = snap.num_required as usize;
        let is_method = snap.is_method;

        // Helper signatures.
        let conv = b.func.signature.call_conv;
        let mut sig_gg = Signature::new(conv);
        sig_gg.params = vec![AbiParam::new(types::I64); 3];
        sig_gg.returns.push(AbiParam::new(types::I64));
        let sig_gg = b.import_signature(sig_gg);
        let mut sig_call = Signature::new(conv);
        sig_call.params = vec![AbiParam::new(types::I64); 7];
        sig_call.returns.push(AbiParam::new(types::I32));
        let sig_call = b.import_signature(sig_call);
        let mut sig_ic = Signature::new(conv);
        sig_ic.params = vec![AbiParam::new(types::I64); 5];
        sig_ic.returns.push(AbiParam::new(types::I32));
        let sig_ic = b.import_signature(sig_ic);
        let mut sig_mkclo = Signature::new(conv);
        sig_mkclo.params = vec![AbiParam::new(types::I64); 4];
        sig_mkclo.returns.push(AbiParam::new(types::I32));
        let sig_mkclo = b.import_signature(sig_mkclo);
        // `zen_jit_make_dict(vm, regs, base, first, pairs, out)`: 6 args.
        let mut sig_mkd = Signature::new(conv);
        sig_mkd.params = vec![AbiParam::new(types::I64); 6];
        sig_mkd.returns.push(AbiParam::new(types::I32));
        let sig_mkd = b.import_signature(sig_mkd);
        // `zen_jit_method_call(vm, regs, base, dst, recv_first, argc, method, out)`: 8 args.
        let mut sig_mcall = Signature::new(conv);
        sig_mcall.params = vec![AbiParam::new(types::I64); 8];
        sig_mcall.returns.push(AbiParam::new(types::I32));
        let sig_mcall = b.import_signature(sig_mcall);
        // The `NativeEntry` ABI for cached direct calls:
        // `(regs, base, vm, out, closure) -> status`.
        let mut sig_native = Signature::new(conv);
        sig_native.params = vec![AbiParam::new(types::I64); 5];
        sig_native.returns.push(AbiParam::new(types::I32));
        let sig_native = b.import_signature(sig_native);

        // Stack slot: a reusable cell for helper results.
        let out_slot = b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
        // Second cell: call sites that need two live outs at once (method
        // resolve + push write-back, iterator item + cursor).
        let out_slot2 = b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3));
        // Shared deopt block: takes every live register plus the resume pc,
        // spills the window, and returns `pc + 2` (0 = ok is taken by success;
        // 1 is unused). The interpreter resumes at that pc with the window as
        // spilled — no entry snapshot needed.
        let deopt = b.create_block();
        for _ in 0..max_reg + 1 {
            b.append_block_param(deopt, types::I64);
        }

        Ok(Self {
            b,
            code: &snap.code,
            constants: &snap.constants,
            module_id: snap.module_id as usize,
            num_globals: snap.num_globals as usize,
            max_reg,
            num_required,
            is_method,
            self_ref,
            addr_gg,
            addr_call,
            addr_getf,
            addr_setf,
            addr_getu,
            addr_setu,
            addr_setg,
            addr_mkd,
            addr_geti,
            addr_seti,
            addr_resolve,
            addr_mcall,
            addr_getf_mono,
            addr_setf_mono,
            addr_mklist,
            addr_mkclass,
            addr_addm,
            addr_mkclo0,
            addr_callnative,
            addr_iternext,
            addr_iterdone,
            addr_push,
            addr_mkcloro,
            addr_call_cached,
            addr_stradd,
            call_cells,
            call_site: 0,
            p_regs: ClValue::from_u32(0),
            p_base: ClValue::from_u32(0),
            p_vm: ClValue::from_u32(0),
            p_out: ClValue::from_u32(0),
            p_closure: ClValue::from_u32(0),
            sig_gg,
            sig_call,
            sig_ic,
            sig_mkclo,
            sig_mkd,
            sig_mcall,
            sig_native,
            out_slot,
            out_slot2,
            blocks: BTreeMap::new(),
            deopt,
            regs: Vec::new(),
            cur: None,
            terminated: false,
            feedback: &snap.feedback,
            ic_slots: &snap.ic_slots,
            osr_entry,
        })
    }

    fn run(&mut self) -> Result<(), String> {
        let len = self.code.len();

        // Leaders: pc 0, every jump target, and the pc after any terminator.
        let mut leaders: BTreeSet<usize> = BTreeSet::new();
        leaders.insert(0);
        for (pc, inst) in self.code.iter().enumerate() {
            match inst.op() {
                Opcode::Jump | Opcode::JumpIfFalse | Opcode::JumpIfTrue => {
                    let t = jump_target(pc, inst.b(), inst.c(), len)?;
                    leaders.insert(t);
                    if pc + 1 < len {
                        leaders.insert(pc + 1);
                    }
                }
                Opcode::Call | Opcode::CallMethod | Opcode::Return => {
                    if pc + 1 < len {
                        leaders.insert(pc + 1);
                    }
                }
                _ => {}
            }
        }

        // Function entry.
        let entry = self.b.create_block();
        self.b.append_block_params_for_function_params(entry);
        self.b.switch_to_block(entry);
        let params = self.b.block_params(entry).to_vec();
        self.p_regs = params[0];
        self.p_base = params[1];
        self.p_vm = params[2];
        self.p_out = params[3];
        self.p_closure = params[4];
        self.cur = Some(entry);

        // Load the argument window; unsupplied locals and temporaries start nil,
        // mirroring `push_frame` (which nils `supplied..num_locals`). Methods
        // carry an implicit receiver in register 0, so they supply one extra
        // leading register beyond the declared/required parameters. An OSR
        // entry instead loads the *full* window: the interpreter transfers
        // mid-loop with every register live in memory.
        let supplied = self.num_required + usize::from(self.is_method);
        let rb = self.rbase();
        let mut regs = Vec::with_capacity(self.max_reg);
        let full = self.osr_entry.is_some();
        for i in 0..self.max_reg {
            if full || i < supplied {
                let v = self.b.ins().load(types::I64, MemFlagsData::new(), rb, (i * 8) as i32);
                regs.push(v);
            } else {
                regs.push(self.b.ins().iconst(types::I64, NIL as i64));
            }
        }
        self.regs = regs;
        // Entry transfers control to pc 0 — or, for an OSR entry, straight to
        // its loop-header block. pc 0 is *not* the entry block itself, so a
        // loop that jumps back to 0 lands on a normal parameterised block.
        let start = match self.osr_entry {
            Some(h) => self.ensure_block(h),
            None => self.ensure_block(0),
        };
        {
            let args = self.block_args();
            self.b.ins().jump(start, &args);
        }
        self.terminated = true;

        // Translate linearly; switch to a fresh parameterised block at each leader.
        let mut pc = 0;
        while pc < len {
            if leaders.contains(&pc) {
                let blk = self.ensure_block(pc);
                if self.cur != Some(blk) {
                    if !self.terminated {
                        let args = self.block_args();
                        self.b.ins().jump(blk, &args);
                    }
                    self.enter(blk);
                }
            }
            let inst = self.code[pc];
            self.lower_inst(pc, inst, len)?;
            pc += 1;
        }
        if !self.terminated {
            // Unreachable in eligible functions (they end in `Return`); resume
            // at 0 so a stray fallthrough restarts rather than trapping.
            self.spill_all();
            let two = self.b.ins().iconst(types::I32, 2);
            self.b.ins().return_(&[two]);
            self.terminated = true;
        }

        // Shared deopt epilogue: the block takes every live register plus the
        // resume pc, spills the register window, and returns `pc + 2`. The
        // interpreter resumes at that pc with the window as spilled — a guard
        // misprediction costs a fallback, never a wrong answer, and needs no
        // entry snapshot.
        self.b.switch_to_block(self.deopt);
        let dparams = self.b.block_params(self.deopt).to_vec();
        let rb = self.rbase();
        for i in 0..self.max_reg {
            self.b.ins().store(MemFlagsData::new(), dparams[i], rb, (i * 8) as i32);
        }
        let pc64 = dparams[self.max_reg];
        let pc32 = self.b.ins().ireduce(types::I32, pc64);
        let status = self.b.ins().iadd_imm_s(pc32, 2);
        self.b.ins().return_(&[status]);
        Ok(())
    }

    // ── block helpers ───────────────────────────────────────────────────

    fn ensure_block(&mut self, pc: usize) -> Block {
        if let Some(&blk) = self.blocks.get(&pc) {
            return blk;
        }
        let blk = self.new_block();
        self.blocks.insert(pc, blk);
        blk
    }

    fn new_block(&mut self) -> Block {
        let blk = self.b.create_block();
        for _ in 0..self.max_reg {
            self.b.append_block_param(blk, types::I64);
        }
        blk
    }

    fn enter(&mut self, blk: Block) {
        self.b.switch_to_block(blk);
        self.cur = Some(blk);
        self.regs = self.b.block_params(blk).to_vec();
        self.terminated = false;
    }

    fn rbase(&mut self) -> ClValue {
        let off = self.b.ins().imul_imm_s(self.p_base, 8);
        self.b.ins().iadd(self.p_regs, off)
    }

    /// Spill every live register to the window (used by the defensive
    /// fallthrough, which has no single resume pc of its own).
    fn spill_all(&mut self) {
        let rb = self.rbase();
        for i in 0..self.max_reg {
            if let Ok(v) = self.reg(i) {
                self.b.ins().store(MemFlagsData::new(), v, rb, (i * 8) as i32);
            }
        }
    }

    // ── value helpers ───────────────────────────────────────────────────

    fn iconst(&mut self, bits: u64) -> ClValue {
        self.b.ins().iconst(types::I64, bits as i64)
    }

    fn truthy(&mut self, v: ClValue) -> ClValue {
        let not_nil = self.b.ins().icmp_imm_s(IntCC::NotEqual, v, NIL as i64);
        let not_false = self.b.ins().icmp_imm_s(IntCC::NotEqual, v, FALSE as i64);
        self.b.ins().band(not_nil, not_false)
    }

    fn bool_of(&mut self, cond: ClValue) -> ClValue {
        let t = self.iconst(TRUE);
        let f = self.iconst(FALSE);
        self.b.ins().select(cond, t, f)
    }

    fn is_int(&mut self, v: ClValue) -> ClValue {
        let top = self.b.ins().ushr_imm_u(v, 48);
        self.b.ins().icmp_imm_s(IntCC::Equal, top, INT_TAG_TOP)
    }

    /// Mirrors `Tag::of`: a raw f64 whenever the quiet-NaN pattern is absent.
    fn is_float(&mut self, v: ClValue) -> ClValue {
        let mask = self.iconst(NIL);
        let m = self.b.ins().band(v, mask);
        self.b.ins().icmp_imm_s(IntCC::NotEqual, m, NIL as i64)
    }

    fn is_number(&mut self, v: ClValue) -> ClValue {
        let gi = self.is_int(v);
        let gf = self.is_float(v);
        self.b.ins().bor(gi, gf)
    }

    fn to_f64(&mut self, v: ClValue, is_int: ClValue) -> ClValue {
        let x = self.sign_extend48(v);
        let as_f = self.b.ins().fcvt_from_sint(types::F64, x);
        let raw = self.b.ins().bitcast(types::F64, MemFlagsData::new(), v);
        self.b.ins().select(is_int, as_f, raw)
    }

    fn sign_extend48(&mut self, v: ClValue) -> ClValue {
        let s = self.b.ins().ishl_imm_u(v, 16);
        self.b.ins().sshr_imm_s(s, 16)
    }

    fn encode_int(&mut self, z: ClValue) -> ClValue {
        let masked = self.b.ins().band_imm_s(z, PAYLOAD_MASK);
        let hdr = self.iconst(INT_HDR);
        self.b.ins().bor(masked, hdr)
    }

    /// True when `z` (a sign-extended integer result) left the 48-bit range.
    fn overflowed(&mut self, z: ClValue) -> ClValue {
        let hi = self.b.ins().icmp_imm_s(IntCC::SignedGreaterThan, z, INT_MAX);
        let lo = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, z, INT_MIN);
        self.b.ins().bor(hi, lo)
    }

    /// True when `z` sits inside the 48-bit int range.
    fn is_48bit(&mut self, z: ClValue) -> ClValue {
        let ovf = self.overflowed(z);
        self.b.ins().icmp_imm_s(IntCC::Equal, ovf, 0)
    }

    // ── feedback specialization (Stage 3) ─────────────────────────────
    // `Value::tag_bit`: Nil=1<<0, False=1<<1, True=1<<2, Int=1<<3,
    // Float=1<<4, String=1<<5, Bytes=1<<6, Heap=1<<7.
    // The interpreter ORs observed operand tags per pc; the snapshot carries
    // the mask. Monomorphic int sites skip float work entirely (thin guard);
    // int+float sites keep the polymorphic guard chain (int check, float
    // fallback, else deopt); unknown sites keep the generic guarded path.

    #[inline]
    fn fb_mask(&self, pc: usize) -> u32 {
        self.feedback.get(pc).map(|e| e.mask).unwrap_or(0)
    }

    #[inline]
    fn fb_int_only(&self, pc: usize) -> bool {
        let m = self.fb_mask(pc);
        m != 0 && (m & !(0x08)) == 0
    }

    #[inline]
    fn fb_float_only(&self, pc: usize) -> bool {
        let m = self.fb_mask(pc);
        m != 0 && (m & !(0x10)) == 0
    }

    /// True when `+` may see non-numeric operands: unknown sites (no feedback
    /// yet — the interpreter repopulates and respecializes) or sites that
    /// observed anything beyond int/float. Pure-numeric sites skip the string
    /// bridge entirely.
    #[inline]
    fn fb_maybe_non_numeric(&self, pc: usize) -> bool {
        let m = self.fb_mask(pc);
        m == 0 || (m & !(0x08 | 0x10)) != 0
    }

    fn reg(&self, i: usize) -> Result<ClValue, String> {
        self.regs
            .get(i)
            .copied()
            .ok_or_else(|| format!("register {i} out of range"))
    }

    fn block_args(&self) -> Vec<BlockArg> {
        self.regs.iter().map(|&v| BlockArg::from(v)).collect()
    }

    fn call_helper(&mut self, sig: SigRef, addr: u64, args: &[ClValue]) -> ClValue {
        let callee = self.iconst(addr);
        let call = self.b.ins().call_indirect(sig, callee, args);
        self.b.inst_results(call)[0]
    }

    /// Emit a guard that continues at `cont` when `ok`, and otherwise spills
    /// the live registers and returns `resume_pc + 2` so the interpreter
    /// resumes at `resume_pc` (a deopt, never a wrong answer).
    fn guard(&mut self, ok: ClValue, cont: Block, resume_pc: usize) {
        let cargs = self.block_args();
        let mut dargs = cargs.clone();
        let pcv = self.iconst(resume_pc as u64);
        dargs.push(BlockArg::from(pcv));
        self.b.ins().brif(ok, cont, &cargs, self.deopt, &dargs);
        self.terminated = true;
    }

    // ── instruction lowering ────────────────────────────────────────────

    fn lower_inst(&mut self, pc: usize, inst: Instruction, len: usize) -> Result<(), String> {
        let a = inst.a() as usize;
        let b = inst.b() as usize;
        let c = inst.c() as usize;
        let op = inst.op();
        match op {
            Opcode::LoadNil => {
                let v = self.iconst(NIL);
                self.set(a, v);
            }
            Opcode::LoadTrue => {
                let v = self.iconst(TRUE);
                self.set(a, v);
            }
            Opcode::LoadFalse => {
                let v = self.iconst(FALSE);
                self.set(a, v);
            }
            Opcode::LoadConst => {
                let bits = self
                    .constants
                    .get(b)
                    .ok_or_else(|| format!("constant {b} out of range"))?
                    .to_bits();
                let v = self.iconst(bits);
                self.set(a, v);
            }
            Opcode::Move => {
                let v = self.reg(b)?;
                self.set(a, v);
            }
            Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div | Opcode::Mod => {
                self.lower_arith(pc, a, b, c, op)?
            }
            Opcode::Neg => self.lower_neg(pc, a, b)?,
            Opcode::BitAnd | Opcode::BitOr | Opcode::BitXor | Opcode::BitNot | Opcode::Shl
            | Opcode::Shr => self.lower_bitwise(pc, a, b, c, op)?,
            Opcode::Lt | Opcode::Le | Opcode::Gt | Opcode::Ge => self.lower_cmp(pc, a, b, c, op)?,
            Opcode::Eq | Opcode::Ne => self.lower_eq(pc, a, b, c, op)?,
            Opcode::And | Opcode::Or | Opcode::Not => self.lower_logic(a, b, c, op)?,
            Opcode::Jump => {
                let t = jump_target(pc, inst.b(), inst.c(), len)?;
                let blk = self.ensure_block(t);
                let args = self.block_args();
                self.b.ins().jump(blk, &args);
                self.terminated = true;
            }
            Opcode::JumpIfFalse | Opcode::JumpIfTrue => {
                let cond = self.truthy(self.reg(a)?);
                let t = jump_target(pc, inst.b(), inst.c(), len)?;
                let target = self.ensure_block(t);
                let fall = self.ensure_block(pc + 1);
                let args = self.block_args();
                if op == Opcode::JumpIfTrue {
                    self.b.ins().brif(cond, target, &args, fall, &args);
                } else {
                    self.b.ins().brif(cond, fall, &args, target, &args);
                }
                self.terminated = true;
            }
            Opcode::GetGlobal => {
                self.lower_get_global(pc, a, b)?;
            }
            Opcode::Call => {
                self.lower_call(pc, a, b, c)?;
            }
            Opcode::GetField | Opcode::GetProperty => {
                self.lower_get_field(pc, a, b)?;
            }
            Opcode::SetField | Opcode::SetProperty => {
                self.lower_set_field(pc, a, c)?;
            }
            Opcode::GetUpval => {
                self.lower_get_upval(pc, a, b)?;
            }
            Opcode::SetUpval => {
                self.lower_set_upval(pc, a, b)?;
            }
            Opcode::MakeClosure => {
                if c == 0 {
                    self.lower_make_closure0(pc, a, b)?;
                } else {
                    return Err("capturing MakeClosure stays interpreted".into());
                }
            }
            Opcode::MakeClosureRO => {
                self.lower_make_closure_ro(pc, a, b, c)?;
            }
            Opcode::MakeList | Opcode::MakeSet | Opcode::MakeTuple => {
                self.lower_make_list(pc, a, b, c, op)?;
            }
            Opcode::MakeClass => {
                self.lower_make_class(pc, a, b, c)?;
            }
            Opcode::AddMethod => {
                self.lower_add_method(pc, a, b, c)?;
            }
            Opcode::CallMethod => {
                self.lower_call_method(pc, a, b, c)?;
            }
            Opcode::IterInit => {
                self.lower_iter_init(pc, a, b)?;
            }
            Opcode::IterNext => {
                self.lower_iter_next(pc, a, b)?;
            }
            Opcode::IterDone => {
                self.lower_iter_done(pc, a, b)?;
            }
            Opcode::CallNative => {
                self.lower_call_native(pc, a, b, c)?;
            }
            Opcode::SetGlobal => {
                self.lower_set_global(pc, a, b)?;
            }
            Opcode::MakeDict => {
                self.lower_make_dict(pc, a, b, c)?;
            }
            Opcode::GetIndex => {
                self.lower_get_index(pc, a, b, c)?;
            }
            Opcode::SetIndex => {
                self.lower_set_index(pc, a, b, c)?;
            }
            Opcode::Return => {
                let v = self.reg(a)?;
                self.b.ins().store(MemFlagsData::new(), v, self.p_out, 0);
                let zero = self.b.ins().iconst(types::I32, 0);
                self.b.ins().return_(&[zero]);
                self.terminated = true;
            }
            other => return Err(format!("opcode {other:?} is not lowered")),
        }
        Ok(())
    }

    fn set(&mut self, a: usize, v: ClValue) {
        if a < self.regs.len() {
            self.regs[a] = v;
        }
    }

    fn lower_arith(&mut self, pc: usize, a: usize, b: usize, c: usize, op: Opcode) -> Result<(), String> {
        let l = self.reg(b)?;
        let r = self.reg(c)?;
        let gl = self.is_int(l);
        let gr = self.is_int(r);
        let gnl = self.is_number(l);
        let gnr = self.is_number(r);
        let both_numbers = self.b.ins().band(gnl, gnr);
        match op {
            // Naive binary arithmetic: compute the speculative int result, fall
            // back to numeric (float or mixed) operands, else deopt.
            Opcode::Add | Opcode::Sub | Opcode::Mul => {
                // Feedback-specialized thin guard: monomorphic int sites skip
                // all float work (no fcvt/fadd/bitcast). This is the hot path
                // for integer loops and recursion (e.g. fib).
                if self.fb_int_only(pc) {
                    let x = self.sign_extend48(l);
                    let y = self.sign_extend48(r);
                    let z = match op {
                        Opcode::Add => self.b.ins().iadd(x, y),
                        Opcode::Sub => self.b.ins().isub(x, y),
                        _ => self.b.ins().imul(x, y),
                    };
                    let g_int = self.b.ins().band(gl, gr);
                    let nof = self.is_48bit(z);
                    let ok = self.b.ins().band(g_int, nof);
                    let enc = self.encode_int(z);
                    self.set(a, enc);
                    let cont = self.new_block();
                    self.guard(ok, cont, pc);
                    self.enter(cont);
                    return Ok(());
                }
                // Float-only sites skip the int computation.
                if self.fb_float_only(pc) {
                    let xf = self.to_f64(l, gl);
                    let yf = self.to_f64(r, gr);
                    let zf = match op {
                        Opcode::Add => self.b.ins().fadd(xf, yf),
                        Opcode::Sub => self.b.ins().fsub(xf, yf),
                        _ => self.b.ins().fmul(xf, yf),
                    };
                    let enc = self.b.ins().bitcast(types::I64, MemFlagsData::new(), zf);
                    self.set(a, enc);
                    let cont = self.new_block();
                    self.guard(both_numbers, cont, pc);
                    self.enter(cont);
                    return Ok(());
                }
                // Polymorphic / unknown: guard chain int -> float -> deopt.
                let x = self.sign_extend48(l);
                let y = self.sign_extend48(r);
                let z = match op {
                    Opcode::Add => self.b.ins().iadd(x, y),
                    Opcode::Sub => self.b.ins().isub(x, y),
                    _ => self.b.ins().imul(x, y),
                };
                let g_int = self.b.ins().band(gl, gr);
                let nof = self.is_48bit(z);
                let ok_int = self.b.ins().band(g_int, nof);
                let xf = self.to_f64(l, gl);
                let yf = self.to_f64(r, gr);
                let zf = match op {
                    Opcode::Add => self.b.ins().fadd(xf, yf),
                    Opcode::Sub => self.b.ins().fsub(xf, yf),
                    _ => self.b.ins().fmul(xf, yf),
                };
                let fenc = self.b.ins().bitcast(types::I64, MemFlagsData::new(), zf);
                let ienc = self.encode_int(z);
                let enc = self.b.ins().select(ok_int, ienc, fenc);
                let ok = self.b.ins().bor(ok_int, both_numbers);
                self.set(a, enc);
                // `+` also concatenates (strings, ropes, lists, bytes, sets).
                // When feedback shows (or has yet to rule out) non-numeric
                // operands, a numeric miss tries the full `+` bridge before
                // deopting — string loops stay native instead of deopting per
                // iteration. Pure-numeric sites keep the single guard.
                if op == Opcode::Add && self.fb_maybe_non_numeric(pc) {
                    let done = self.new_block();
                    let strblk = self.new_block();
                    let args = self.block_args();
                    self.b.ins().brif(ok, done, &args, strblk, &args);
                    self.terminated = true;
                    self.enter(strblk);
                    let vm = self.p_vm;
                    let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
                    let status = self.call_helper(
                        self.sig_mkclo,
                        self.addr_stradd,
                        &[vm, l, r, out_addr],
                    );
                    let ok2 = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
                    let go = self.new_block();
                    self.guard(ok2, go, pc);
                    self.enter(go);
                    let v = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
                    self.set(a, v);
                    let args = self.block_args();
                    self.b.ins().jump(done, &args);
                    self.terminated = true;
                    self.enter(done);
                } else {
                    let cont = self.new_block();
                    self.guard(ok, cont, pc);
                    self.enter(cont);
                }
            }
            // Division is always a float result (int operands are widened),
            // and dividing by zero raises, so deopt on a zero divisor.
            Opcode::Div => {
                let glf = self.is_float(l);
                let grf = self.is_float(r);
                let gnl = self.b.ins().bor(gl, glf);
                let gnr = self.b.ins().bor(gr, grf);
                let both_numbers = self.b.ins().band(gnl, gnr);
                let xf = self.to_f64(l, gl);
                let yf = self.to_f64(r, gr);
                let zf = self.b.ins().fdiv(xf, yf);
                let zero = self.b.ins().f64const(0.0);
                let nz = self.b.ins().fcmp(FloatCC::NotEqual, yf, zero);
                let ok = self.b.ins().band(both_numbers, nz);
                let enc = self.b.ins().bitcast(types::I64, MemFlagsData::new(), zf);
                self.set(a, enc);
                let cont = self.new_block();
                self.guard(ok, cont, pc);
                self.enter(cont);
            }
            // Modulo keeps the interpreter's int fast path; float/mixed mod
            // and error cases deopt. Traps are avoided by neutralizing inputs.
            _ => {
                let x = self.sign_extend48(l);
                let y = self.sign_extend48(r);
                let safe_y = self.b.ins().icmp_imm_s(IntCC::NotEqual, y, 0);
                let one = self.iconst(1);
                let safe_y = self.b.ins().select(safe_y, y, one);
                let hazard = self
                    .b
                    .ins()
                    .icmp_imm_s(IntCC::Equal, x, INT_MIN as i64);
                let hazard2 = self.b.ins().icmp_imm_s(IntCC::Equal, y, -1);
                let hazard = self.b.ins().band(hazard, hazard2);
                let zero = self.iconst(0);
                let xs = self.b.ins().select(hazard, zero, x);
                let z = self.b.ins().srem(xs, safe_y);
                let g_int = self.b.ins().band(gl, gr);
                let nz = self.b.ins().icmp_imm_s(IntCC::NotEqual, y, 0);
                let not_hazard = self.b.ins().icmp_imm_s(IntCC::Equal, hazard, 0);
                let ok = self.b.ins().band(g_int, nz);
                let ok = self.b.ins().band(ok, not_hazard);
                let enc = self.encode_int(z);
                self.set(a, enc);
                let cont = self.new_block();
                self.guard(ok, cont, pc);
                self.enter(cont);
            }
        }
        Ok(())
    }

    fn lower_neg(&mut self, pc: usize, a: usize, b: usize) -> Result<(), String> {
        let v = self.reg(b)?;
        let gi = self.is_int(v);
        let x = self.sign_extend48(v);
        let z = self.b.ins().ineg(x);
        let nof = self.is_48bit(z);
        let ok_int = self.b.ins().band(gi, nof);
        // Floating-point fallback: -f, kept in raw bits.
        let xf = self.to_f64(v, gi);
        let zf = self.b.ins().fneg(xf);
        let fenc = self.b.ins().bitcast(types::I64, MemFlagsData::new(), zf);
        let ienc = self.encode_int(z);
        let enc = self.b.ins().select(ok_int, ienc, fenc);
        let gf = self.is_float(v);
        let ok = self.b.ins().bor(ok_int, gf);
        self.set(a, enc);
        let cont = self.new_block();
        self.guard(ok, cont, pc);
        self.enter(cont);
        Ok(())
    }

    fn lower_bitwise(&mut self, pc: usize, a: usize, b: usize, c: usize, op: Opcode) -> Result<(), String> {
        let l = self.reg(b)?;
        let x = self.sign_extend48(l);
        let z = match op {
            Opcode::BitNot => self.b.ins().bnot(x),
            Opcode::Shl | Opcode::Shr => {
                let r = self.reg(c)?;
                let y = self.sign_extend48(r);
                // The interpreter masks the shift count to 0..=63.
                let s = self.b.ins().band_imm_s(y, 63);
                if op == Opcode::Shl {
                    self.b.ins().ishl(x, s)
                } else {
                    self.b.ins().sshr(x, s)
                }
            }
            _ => {
                let r = self.reg(c)?;
                let y = self.sign_extend48(r);
                match op {
                    Opcode::BitAnd => self.b.ins().band(x, y),
                    Opcode::BitOr => self.b.ins().bor(x, y),
                    _ => self.b.ins().bxor(x, y),
                }
            }
        };
        // Both operands must be ints; results outside the 48-bit range are
        // promoted to float by the interpreter, so deopt lets it finish.
        let ok = self.is_int(l);
        let ok = if op == Opcode::BitNot {
            ok
        } else {
            let gr = self.is_int(self.reg(c)?);
            self.b.ins().band(ok, gr)
        };
        let ovf = self.overflowed(z);
        let nof = self.b.ins().icmp_imm_s(IntCC::Equal, ovf, 0);
        let ok = self.b.ins().band(ok, nof);
        let enc = self.encode_int(z);
        self.set(a, enc);
        let cont = self.new_block();
        self.guard(ok, cont, pc);
        self.enter(cont);
        Ok(())
    }

    fn lower_cmp(&mut self, pc: usize, a: usize, b: usize, c: usize, op: Opcode) -> Result<(), String> {
        let l = self.reg(b)?;
        let r = self.reg(c)?;
        let gl = self.is_int(l);
        let gr = self.is_int(r);
        let x = self.sign_extend48(l);
        let y = self.sign_extend48(r);
        let cc = match op {
            Opcode::Lt => IntCC::SignedLessThan,
            Opcode::Le => IntCC::SignedLessThanOrEqual,
            Opcode::Gt => IntCC::SignedGreaterThan,
            _ => IntCC::SignedGreaterThanOrEqual,
        };
        // Feedback-specialized: monomorphic int comparisons skip float work.
        if self.fb_int_only(pc) {
            let ok_int = self.b.ins().band(gl, gr);
            let cond = self.b.ins().icmp(cc, x, y);
            let v = self.bool_of(cond);
            self.set(a, v);
            let cont = self.new_block();
            self.guard(ok_int, cont, pc);
            self.enter(cont);
            return Ok(());
        }
        // Numerics compare numerically across int/float, so fall back to a
        // widened comparison for any two numbers; everything else deopts.
        let ok_int = self.b.ins().band(gl, gr);
        let cond_i = self.b.ins().icmp(cc, x, y);
        let xf = self.to_f64(l, gl);
        let yf = self.to_f64(r, gr);
        let fcc = match op {
            Opcode::Lt => FloatCC::LessThan,
            Opcode::Le => FloatCC::LessThanOrEqual,
            Opcode::Gt => FloatCC::GreaterThan,
            _ => FloatCC::GreaterThanOrEqual,
        };
        let cond_f = self.b.ins().fcmp(fcc, xf, yf);
        let cond = self.b.ins().select(ok_int, cond_i, cond_f);
        let nl = self.is_number(l);
        let nr = self.is_number(r);
        let both_numbers = self.b.ins().band(nl, nr);
        let ok = self.b.ins().bor(ok_int, both_numbers);
        let v = self.bool_of(cond);
        self.set(a, v);
        let cont = self.new_block();
        self.guard(ok, cont, pc);
        self.enter(cont);
        Ok(())
    }

    fn lower_eq(&mut self, pc: usize, a: usize, b: usize, c: usize, op: Opcode) -> Result<(), String> {
        let l = self.reg(b)?;
        let r = self.reg(c)?;
        let gl = self.is_int(l);
        let gr = self.is_int(r);
        let x = self.sign_extend48(l);
        let y = self.sign_extend48(r);
        let cc = if op == Opcode::Eq {
            IntCC::Equal
        } else {
            IntCC::NotEqual
        };
        if self.fb_int_only(pc) {
            let ok_int = self.b.ins().band(gl, gr);
            let cond = self.b.ins().icmp(cc, x, y);
            let v = self.bool_of(cond);
            self.set(a, v);
            let cont = self.new_block();
            self.guard(ok_int, cont, pc);
            self.enter(cont);
            return Ok(());
        }
        let ok_int = self.b.ins().band(gl, gr);
        let cond_i = self.b.ins().icmp(cc, x, y);
        let xf = self.to_f64(l, gl);
        let yf = self.to_f64(r, gr);
        let fcc = if op == Opcode::Eq {
            FloatCC::Equal
        } else {
            FloatCC::NotEqual
        };
        let cond_f = self.b.ins().fcmp(fcc, xf, yf);
        let cond = self.b.ins().select(ok_int, cond_i, cond_f);
        let nl = self.is_number(l);
        let nr = self.is_number(r);
        let both_numbers = self.b.ins().band(nl, nr);
        let ok = self.b.ins().bor(ok_int, both_numbers);
        let v = self.bool_of(cond);
        self.set(a, v);
        let cont = self.new_block();
        self.guard(ok, cont, pc);
        self.enter(cont);
        Ok(())
    }

    fn lower_logic(&mut self, a: usize, b: usize, c: usize, op: Opcode) -> Result<(), String> {
        let v = match op {
            Opcode::Not => {
                let x = self.truthy(self.reg(b)?);
                let f = self.iconst(FALSE);
                let t = self.iconst(TRUE);
                self.b.ins().select(x, f, t)
            }
            _ => {
                let x = self.truthy(self.reg(b)?);
                let y = self.truthy(self.reg(c)?);
                let r = if op == Opcode::And {
                    self.b.ins().band(x, y)
                } else {
                    self.b.ins().bor(x, y)
                };
                self.bool_of(r)
            }
        };
        self.set(a, v);
        Ok(())
    }

    /// `GetGlobal dst=a, idx=b`: inlined live load from
    /// `vm.module_globals[mid][idx]` — no helper call on the hot path. Bounds
    /// misses deopt so the interpreter raises, exactly as its arm does. An
    /// index beyond the snapshotted global count falls back to the helper
    /// (which yields `nil`, matching `jit_global`).
    fn lower_get_global(&mut self, pc: usize, a: usize, b: usize) -> Result<(), String> {
        if b >= self.num_globals {
            let vm = self.p_vm;
            let mid = self.iconst(self.module_id as u64);
            let gidx = self.iconst(b as u64);
            let v = self.call_helper(self.sig_gg, self.addr_gg, &[vm, mid, gidx]);
            self.set(a, v);
            return Ok(());
        }
        let v = self.global_addr(pc, b, true)?;
        self.set(a, v);
        Ok(())
    }

    /// Load (`load=true`) or compute the address of (`load=false`) global
    /// `idx`: `vm.module_globals[mid][idx]` with live bounds guards. Emits
    /// plain loads off the `vm` pointer — always fresh, so no stale-pointer
    /// or cross-run hazard is possible; a miss deopts (never a wrong answer).
    fn global_addr(&mut self, pc: usize, idx: usize, load: bool) -> Result<ClValue, String> {
        let vm = self.p_vm;
        let off = self.iconst(zen_vm::vm::Vm::module_globals_off() as u64);
        let vm_globals = self.b.ins().iadd(vm, off);
        // Outer vec: `module_globals: Vec<Vec<Value>>` is `{len, ptr, cap}`
        // on this toolchain (pinned by `vec_value_layout`).
        let outer_len = self.b.ins().load(types::I64, MemFlagsData::new(), vm_globals, 0);
        let outer_ptr = self.b.ins().load(types::I64, MemFlagsData::new(), vm_globals, 8);
        let mid = self.iconst(self.module_id as u64);
        let mid_ok = self.b.ins().icmp(IntCC::UnsignedLessThan, mid, outer_len);
        let checked = self.new_block();
        self.guard(mid_ok, checked, pc);
        self.enter(checked);
        // Inner vec `{len, ptr, cap}` for this module.
        let inner_at = self.b.ins().iadd_imm_s(outer_ptr, (self.module_id * 24) as i64);
        let inner_len = self.b.ins().load(types::I64, MemFlagsData::new(), inner_at, 0);
        let inner_ptr = self.b.ins().load(types::I64, MemFlagsData::new(), inner_at, 8);
        let idxv = self.iconst(idx as u64);
        let idx_ok = self.b.ins().icmp(IntCC::UnsignedLessThan, idxv, inner_len);
        let ready = self.new_block();
        self.guard(idx_ok, ready, pc);
        self.enter(ready);
        let slot = self.b.ins().iadd_imm_s(inner_ptr, (idx * 8) as i64);
        if load {
            Ok(self.b.ins().load(types::I64, MemFlagsData::new(), slot, 0))
        } else {
            Ok(slot)
        }
    }

    /// `GetField`/`GetProperty`: consult the prototype's inline cache. A
    /// monomorphic hit returns the slot value; a miss deopts so the interpreter
    /// runs the slow path (and populates the cache).
    fn lower_get_field(&mut self, pc: usize, a: usize, b: usize) -> Result<(), String> {
        // Pillar 1 (inlined monomorphic IC): when the snapshot shows a stable
        // monomorphic site, bake the shape + slot and call the thin checker —
        // no `RefCell` borrow, no shape-table lookup, no IC enum walk on the
        // hot path. Anything else falls back to the resolving helper.
        if let Some((shape, offset)) = self.mono_shape(pc) {
            let obj = self.reg(b)?;
            let shapev = self.iconst(u64::from(shape));
            let offv = self.iconst(offset as u64);
            let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
            let status = self.call_helper(
                self.sig_mkclo,
                self.addr_getf_mono,
                &[obj, shapev, offv, out_addr],
            );
            let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
            let go = self.new_block();
            self.guard(ok, go, pc);
            self.enter(go);
            let v = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
            self.set(a, v);
            return Ok(());
        }
        let obj = self.reg(b)?;
        let vm = self.p_vm;
        let closure = self.p_closure;
        let pcv = self.iconst(pc as u64);
        let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let status = self.call_helper(
            self.sig_ic,
            self.addr_getf,
            &[vm, obj, pcv, closure, out_addr],
        );
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let v = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(a, v);
        Ok(())
    }

    /// The baked `(shape, offset)` for a monomorphic field site, if the
    /// snapshot's inline cache shows exactly one observed shape.
    fn mono_shape(&self, pc: usize) -> Option<(u32, usize)> {
        match self.ic_slots.get(pc) {
            Some(zen_vm::ic::InlineCache::Monomorphic { shape, offset }) => Some((*shape, *offset)),
            _ => None,
        }
    }

    /// `SetField`/`SetProperty`: write through the inline cache. The receiver is
    /// left in `a` (matching the interpreter's fast path); a miss deopts.
    fn lower_set_field(&mut self, pc: usize, a: usize, c: usize) -> Result<(), String> {
        // Same monomorphic story as `lower_get_field`: bake shape + slot.
        if let Some((shape, offset)) = self.mono_shape(pc) {
            let obj = self.reg(a)?;
            let val = self.reg(c)?;
            let shapev = self.iconst(u64::from(shape));
            let offv = self.iconst(offset as u64);
            let status = self.call_helper(
                self.sig_mkclo,
                self.addr_setf_mono,
                &[obj, shapev, offv, val],
            );
            let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
            let go = self.new_block();
            self.guard(ok, go, pc);
            self.enter(go);
            self.set(a, obj);
            return Ok(());
        }
        let obj = self.reg(a)?;
        let val = self.reg(c)?;
        let vm = self.p_vm;
        let closure = self.p_closure;
        let pcv = self.iconst(pc as u64);
        let status = self.call_helper(
            self.sig_ic,
            self.addr_setf,
            &[vm, obj, pcv, val, closure],
        );
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        self.set(a, obj);
        Ok(())
    }

    fn lower_get_upval(&mut self, pc: usize, a: usize, b: usize) -> Result<(), String> {
        let idx = self.iconst(b as u64);
        let vm = self.p_vm;
        let closure = self.p_closure;
        let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let status = self.call_helper(
            self.sig_mkclo,
            self.addr_getu,
            &[vm, closure, idx, out_addr],
        );
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let v = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(a, v);
        Ok(())
    }

    fn lower_set_upval(&mut self, pc: usize, idx: usize, src: usize) -> Result<(), String> {
        let idxv = self.iconst(idx as u64);
        let val = self.reg(src)?;
        let vm = self.p_vm;
        let closure = self.p_closure;
        let status = self.call_helper(
            self.sig_mkclo,
            self.addr_setu,
            &[vm, closure, idxv, val],
        );
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        Ok(())
    }

    /// `SetGlobal idx=a, src=b`: inlined live store to
    /// `vm.module_globals[mid][idx]`, mirroring the interpreter's plain
    /// overwrite. Out-of-range indices keep the helper path (deopt, then the
    /// interpreter raises).
    fn lower_set_global(&mut self, pc: usize, idx: usize, src: usize) -> Result<(), String> {
        let val = self.reg(src)?;
        if idx >= self.num_globals {
            let vm = self.p_vm;
            let mid = self.iconst(self.module_id as u64);
            let gidx = self.iconst(idx as u64);
            let status = self.call_helper(self.sig_mkclo, self.addr_setg, &[vm, mid, gidx, val]);
            let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
            let go = self.new_block();
            self.guard(ok, go, pc);
            self.enter(go);
            return Ok(());
        }
        let slot = self.global_addr(pc, idx, false)?;
        self.b.ins().store(MemFlagsData::new(), val, slot, 0);
        Ok(())
    }

    /// `MakeDict dst=a, pairs=b, first=c`: dict from alternating key/value
    /// window slots. Items are spilled first — the helper reads memory.
    fn lower_make_dict(&mut self, pc: usize, a: usize, pairs: usize, first: usize) -> Result<(), String> {
        if first + pairs.checked_mul(2).ok_or("bad dict size")? > self.max_reg {
            return Err("bad dict window".into());
        }
        let rb = self.rbase();
        for i in 0..pairs * 2 {
            let v = self.reg(first + i)?;
            self.b
                .ins()
                .store(MemFlagsData::new(), v, rb, ((first + i) * 8) as i32);
        }
        let vm = self.p_vm;
        let regs_ptr = self.p_regs;
        let base = self.p_base;
        let firstv = self.iconst(first as u64);
        let pairsv = self.iconst(pairs as u64);
        let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let status = self.call_helper(
            self.sig_mkd,
            self.addr_mkd,
            &[vm, regs_ptr, base, firstv, pairsv, out_addr],
        );
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let v = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(a, v);
        Ok(())
    }

    /// `GetIndex dst=a, obj=b, key=c`: a Heap-tagged `List`/`Tuple` with an
    /// `int` in-bounds key loads inline (no helper call); everything else —
    /// dicts, strings, tuples by helper, OOB, type errors — takes the shared
    /// bridge, whose errors deopt so the interpreter raises them.
    fn lower_get_index(&mut self, pc: usize, a: usize, b: usize, c: usize) -> Result<(), String> {
        let obj = self.reg(b)?;
        let key = self.reg(c)?;
        let slow = self.new_block();
        let done = self.new_block();
        // Heap tag first: only then is the payload a pointer we may read.
        let otop = self.b.ins().ushr_imm_u(obj, 48);
        let is_heap = self.b.ins().icmp_imm_s(IntCC::Equal, otop, HEAP_TAG_TOP);
        let check_kind = self.new_block();
        let args = self.block_args();
        self.b.ins().brif(is_heap, check_kind, &args, slow, &args);
        self.terminated = true;
        // Kind must be List(0) or Tuple(3): same header, same `inner` offset.
        self.enter(check_kind);
        let ptr = self.b.ins().band_imm_s(obj, PAYLOAD_MASK);
        let kind8 = self.b.ins().load(types::I8, MemFlagsData::new(), ptr, 0);
        let kind = self.b.ins().uextend(types::I64, kind8);
        let is_list = self.b.ins().icmp_imm_s(IntCC::Equal, kind, 0);
        let is_tuple = self.b.ins().icmp_imm_s(IntCC::Equal, kind, 3);
        let is_seq = self.b.ins().bor(is_list, is_tuple);
        let check_key = self.new_block();
        let args = self.block_args();
        self.b.ins().brif(is_seq, check_key, &args, slow, &args);
        self.terminated = true;
        // Key must be an int.
        self.enter(check_key);
        let key_is_int = self.is_int(key);
        let check_bounds = self.new_block();
        let args = self.block_args();
        self.b.ins().brif(key_is_int, check_bounds, &args, slow, &args);
        self.terminated = true;
        // `inner: Vec<Value>` at header+24 is `{len, ptr, cap}` here (pinned
        // by `vec_value_layout`): len@+24, data@+32. Negative wraps once, as
        // `normalize_index` does; anything still out of range is an error.
        self.enter(check_bounds);
        let len = self.b.ins().load(types::I64, MemFlagsData::new(), ptr, 24);
        let data = self.b.ins().load(types::I64, MemFlagsData::new(), ptr, 32);
        let mut idx = self.sign_extend48(key);
        let neg = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, idx, 0);
        let wrapped = self.b.ins().iadd(idx, len);
        idx = self.b.ins().select(neg, wrapped, idx);
        let in_bounds = self.b.ins().icmp(IntCC::UnsignedLessThan, idx, len);
        let hit = self.new_block();
        let args = self.block_args();
        self.b.ins().brif(in_bounds, hit, &args, slow, &args);
        self.terminated = true;
        self.enter(hit);
        let off = self.b.ins().ishl_imm_u(idx, 3);
        let slot = self.b.ins().iadd(data, off);
        let v = self.b.ins().load(types::I64, MemFlagsData::new(), slot, 0);
        self.set(a, v);
        let args = self.block_args();
        self.b.ins().jump(done, &args);
        self.terminated = true;
        // Slow path: the shared bridge (dicts, strings, errors).
        self.enter(slow);
        let vm = self.p_vm;
        let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let status = self.call_helper(self.sig_mkclo, self.addr_geti, &[vm, obj, key, out_addr]);
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let v = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(a, v);
        let args = self.block_args();
        self.b.ins().jump(done, &args);
        self.terminated = true;
        self.enter(done);
        Ok(())
    }

    /// `SetIndex obj=a, key=b, val=c`: an exclusively-owned `List` with an
    /// `int` in-bounds key stores inline; the shared case detaches, so it (and
    /// everything else) takes the bridge. Result is the object, matching the
    /// interpreter.
    fn lower_set_index(&mut self, pc: usize, a: usize, b: usize, c: usize) -> Result<(), String> {
        let obj = self.reg(a)?;
        let key = self.reg(b)?;
        let val = self.reg(c)?;
        let slow = self.new_block();
        let done = self.new_block();
        let otop = self.b.ins().ushr_imm_u(obj, 48);
        let is_heap = self.b.ins().icmp_imm_s(IntCC::Equal, otop, HEAP_TAG_TOP);
        let check_kind = self.new_block();
        let args = self.block_args();
        self.b.ins().brif(is_heap, check_kind, &args, slow, &args);
        self.terminated = true;
        self.enter(check_kind);
        let ptr = self.b.ins().band_imm_s(obj, PAYLOAD_MASK);
        let kind8 = self.b.ins().load(types::I8, MemFlagsData::new(), ptr, 0);
        let kind = self.b.ins().uextend(types::I64, kind8);
        let is_list = self.b.ins().icmp_imm_s(IntCC::Equal, kind, 0);
        let check_key = self.new_block();
        let args = self.block_args();
        self.b.ins().brif(is_list, check_key, &args, slow, &args);
        self.terminated = true;
        self.enter(check_key);
        let key_is_int = self.is_int(key);
        let check_rc = self.new_block();
        let args = self.block_args();
        self.b.ins().brif(key_is_int, check_rc, &args, slow, &args);
        self.terminated = true;
        // Exclusive ownership (`rc == 1`, header+8) means the store cannot be
        // observed through another reference — no detach needed.
        self.enter(check_rc);
        let rc = self.b.ins().load(types::I32, MemFlagsData::new(), ptr, 8);
        let excl = self.b.ins().icmp_imm_s(IntCC::Equal, rc, 1);
        let check_bounds = self.new_block();
        let args = self.block_args();
        self.b.ins().brif(excl, check_bounds, &args, slow, &args);
        self.terminated = true;
        self.enter(check_bounds);
        let len = self.b.ins().load(types::I64, MemFlagsData::new(), ptr, 24);
        let data = self.b.ins().load(types::I64, MemFlagsData::new(), ptr, 32);
        let mut idx = self.sign_extend48(key);
        let neg = self.b.ins().icmp_imm_s(IntCC::SignedLessThan, idx, 0);
        let wrapped = self.b.ins().iadd(idx, len);
        idx = self.b.ins().select(neg, wrapped, idx);
        let in_bounds = self.b.ins().icmp(IntCC::UnsignedLessThan, idx, len);
        let hit = self.new_block();
        let args = self.block_args();
        self.b.ins().brif(in_bounds, hit, &args, slow, &args);
        self.terminated = true;
        self.enter(hit);
        let off = self.b.ins().ishl_imm_u(idx, 3);
        let slot = self.b.ins().iadd(data, off);
        self.b.ins().store(MemFlagsData::new(), val, slot, 0);
        self.set(a, obj);
        let args = self.block_args();
        self.b.ins().jump(done, &args);
        self.terminated = true;
        self.enter(slow);
        let vm = self.p_vm;
        let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let status = self.call_helper(self.sig_ic, self.addr_seti, &[vm, obj, key, val, out_addr]);
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let v = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(a, v);
        let args = self.block_args();
        self.b.ins().jump(done, &args);
        self.terminated = true;
        self.enter(done);
        Ok(())
    }

    /// `MakeClosure` with zero captures: the module id is baked in, so no
    /// defining frame is touched. Capturing definers never reach here
    /// (rejected by eligibility); the arm guards anyway.
    fn lower_make_closure0(&mut self, pc: usize, a: usize, proto_idx: usize) -> Result<(), String> {
        let vm = self.p_vm;
        let mid = self.iconst(self.module_id as u64);
        let pidx = self.iconst(proto_idx as u64);
        let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let status = self.call_helper(self.sig_mkclo, self.addr_mkclo0, &[vm, mid, pidx, out_addr]);
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let v = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(a, v);
        Ok(())
    }

    /// `MakeClosureRO dst=a, proto_idx=b, count=c`: snapshot the proven
    /// read-only captures from the register window. Every live register is
    /// spilled first (capture sites are cold, so the spill is noise), then the
    /// helper reads the descriptor slots from memory — no interpreter frame,
    /// no promotion, no cells. Anything unexpected deopts to the exact
    /// interpreter capture.
    fn lower_make_closure_ro(
        &mut self,
        pc: usize,
        a: usize,
        proto_idx: usize,
        count: usize,
    ) -> Result<(), String> {
        self.spill_all();
        let vm = self.p_vm;
        let regs_ptr = self.p_regs;
        let base = self.p_base;
        let mid = self.iconst(self.module_id as u64);
        let pidx = self.iconst(proto_idx as u64);
        let cnt = self.iconst(count as u64);
        let maxreg = self.iconst(self.max_reg as u64);
        let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let status = self.call_helper(
            self.sig_mcall,
            self.addr_mkcloro,
            &[vm, regs_ptr, base, mid, pidx, cnt, maxreg, out_addr],
        );
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let v = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(a, v);
        Ok(())
    }

    /// `MakeList`/`MakeSet`/`MakeTuple dst=a, count=b, first=c`: items are
    /// spilled first — the helper reads memory, mirroring the interpreter.
    fn lower_make_list(
        &mut self,
        pc: usize,
        a: usize,
        count: usize,
        first: usize,
        op: Opcode,
    ) -> Result<(), String> {
        if first + count > self.max_reg {
            return Err("bad make-list window".into());
        }
        let kind = match op {
            Opcode::MakeList => 0,
            Opcode::MakeSet => 1,
            _ => 2,
        };
        let rb = self.rbase();
        for i in 0..count {
            let v = self.reg(first + i)?;
            self.b
                .ins()
                .store(MemFlagsData::new(), v, rb, ((first + i) * 8) as i32);
        }
        let regs_ptr = self.p_regs;
        let base = self.p_base;
        let firstv = self.iconst(first as u64);
        let countv = self.iconst(count as u64);
        let kindv = self.iconst(kind);
        let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let status = self.call_helper(
            self.sig_mkd,
            self.addr_mklist,
            &[regs_ptr, base, firstv, countv, kindv, out_addr],
        );
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let v = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(a, v);
        Ok(())
    }

    /// `MakeClass dst=a, name_const=b, parent=c|0xFF`: runs once per entry, so
    /// a faithful bridge is the right trade — it unlocks the loop below.
    fn lower_make_class(&mut self, pc: usize, a: usize, b: usize, c: usize) -> Result<(), String> {
        let closure = self.p_closure;
        let cidx = self.iconst(b as u64);
        let (parent, has) = if c == 0xFF {
            (self.iconst(NIL), self.iconst(0))
        } else {
            (self.reg(c)?, self.iconst(1))
        };
        let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let status = self.call_helper(
            self.sig_ic,
            self.addr_mkclass,
            &[closure, cidx, parent, has, out_addr],
        );
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let v = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(a, v);
        Ok(())
    }

    /// `AddMethod class=a, name_const=b, method=c`: result is the class itself,
    /// matching the interpreter.
    fn lower_add_method(&mut self, pc: usize, a: usize, b: usize, c: usize) -> Result<(), String> {
        let class = self.reg(a)?;
        let method = self.reg(c)?;
        let closure = self.p_closure;
        let cidx = self.iconst(b as u64);
        let status = self.call_helper(
            self.sig_mkclo,
            self.addr_addm,
            &[closure, cidx, class, method],
        );
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        self.set(a, class);
        Ok(())
    }

    /// `CallMethod dst=a, name_const=b, argc=c`: resolve the receiver to a
    /// method value, then invoke it through the method bridge. The receiver
    /// and arguments are spilled first (callees read memory). A resolve miss
    /// tries the `list.push` fast lane when the shape fits, else deopts so the
    /// interpreter runs the full dispatch (higher-order, user types, errors).
    fn lower_call_method(&mut self, pc: usize, a: usize, b: usize, argc: usize) -> Result<(), String> {
        let recv_first = a
            .checked_sub(argc + 1)
            .ok_or_else(|| "method receiver underflow".to_string())?;
        if recv_first + 1 + argc > self.max_reg {
            return Err("bad method window".into());
        }
        let rb = self.rbase();
        for i in 0..=argc {
            let v = self.reg(recv_first + i)?;
            self.b
                .ins()
                .store(MemFlagsData::new(), v, rb, ((recv_first + i) * 8) as i32);
        }
        let cont = self.ensure_block(pc + 1);

        // Resolve the receiver (instance IC + module-namespace dict paths).
        let recv = self.reg(recv_first)?;
        let vm = self.p_vm;
        let cidx = self.iconst(b as u64);
        let pcv = self.iconst(pc as u64);
        let closure = self.p_closure;
        let method_out = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let status = self.call_helper(
            self.sig_mkd,
            self.addr_resolve,
            &[vm, recv, cidx, pcv, closure, method_out],
        );
        let hit = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let method_blk = self.new_block();
        let miss_blk = self.new_block();
        let args = self.block_args();
        self.b.ins().brif(hit, method_blk, &args, miss_blk, &args);
        self.terminated = true;

        // Hit: invoke the resolved method (receiver stays in window slot 0
        // for `is_method` callees; the bridge computes the frame base).
        self.enter(method_blk);
        let method = self.b.ins().load(types::I64, MemFlagsData::new(), method_out, 0);
        let regs_ptr = self.p_regs;
        let base = self.p_base;
        let dstv = self.iconst(a as u64);
        let recv_firstv = self.iconst(recv_first as u64);
        let argv = self.iconst(argc as u64);
        let status = self.call_helper(
            self.sig_mcall,
            self.addr_mcall,
            &[vm, regs_ptr, base, dstv, recv_firstv, argv, method, method_out],
        );
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let res = self.b.ins().load(types::I64, MemFlagsData::new(), method_out, 0);
        self.set(a, res);
        let args = self.block_args();
        self.b.ins().jump(cont, &args);
        self.terminated = true;

        // Miss: `list.push(x)` fast lane when the static shape fits
        // (single argument + a push-like method name); otherwise deopt to the
        // interpreter's full dispatch.
        self.enter(miss_blk);
        let is_push = argc == 1 && self.is_push_name(b);
        if is_push {
            let arg = self.reg(recv_first + 1)?;
            let recv_out = self.b.ins().stack_addr(types::I64, self.out_slot2, 0);
            let status = self.call_helper(
                self.sig_mkclo,
                self.addr_push,
                &[recv, arg, recv_out, method_out],
            );
            let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
            let go = self.new_block();
            self.guard(ok, go, pc);
            self.enter(go);
            let recv_new = self.b.ins().load(types::I64, MemFlagsData::new(), recv_out, 0);
            self.set(recv_first, recv_new);
            let res = self.b.ins().load(types::I64, MemFlagsData::new(), method_out, 0);
            self.set(a, res);
            let args = self.block_args();
            self.b.ins().jump(cont, &args);
            self.terminated = true;
        } else {
            let mut dargs = self.block_args();
            dargs.push(BlockArg::from(pcv));
            self.b.ins().jump(self.deopt, &dargs);
            self.terminated = true;
        }

        self.enter(cont);
        Ok(())
    }

    /// True when constant `b` names a push-like method (`push`/`append`), so
    /// the `CallMethod` miss path can try the list fast lane.
    fn is_push_name(&self, b: usize) -> bool {
        let name = self.constants.get(b).and_then(|v| v.as_str());
        matches!(name, Some("push") | Some("append"))
    }

    /// `IterInit dst_pair=a, seq=b`: sequence stays, cursor starts at 0.
    /// Pure register moves — no helper needed.
    fn lower_iter_init(&mut self, pc: usize, a: usize, b: usize) -> Result<(), String> {
        let _ = pc;
        if a + 1 >= self.max_reg {
            return Err("bad iter window".into());
        }
        let seq = self.reg(b)?;
        let zero = self.iconst(INT_HDR);
        self.set(a, seq);
        self.set(a + 1, zero);
        Ok(())
    }

    /// `IterNext pair=a, dst=b`: item + advanced cursor via the helper;
    /// `iter_get` errors deopt so the interpreter raises them.
    fn lower_iter_next(&mut self, pc: usize, a: usize, b: usize) -> Result<(), String> {
        if a + 1 >= self.max_reg {
            return Err("bad iter window".into());
        }
        let seq = self.reg(a)?;
        let cur = self.reg(a + 1)?;
        let item_out = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let cur_out = self.b.ins().stack_addr(types::I64, self.out_slot2, 0);
        let status = self.call_helper(
            self.sig_mkclo,
            self.addr_iternext,
            &[seq, cur, item_out, cur_out],
        );
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        // Order mirrors the interpreter: cursor first, then the item (so a
        // `b == a + 1` overlap resolves identically).
        let cur_new = self.b.ins().load(types::I64, MemFlagsData::new(), cur_out, 0);
        self.set(a + 1, cur_new);
        let item = self.b.ins().load(types::I64, MemFlagsData::new(), item_out, 0);
        self.set(b, item);
        Ok(())
    }

    /// `IterDone pair=a, dst=b`: infallible done-bit, mirroring the arm.
    fn lower_iter_done(&mut self, _pc: usize, a: usize, b: usize) -> Result<(), String> {
        if a + 1 >= self.max_reg {
            return Err("bad iter window".into());
        }
        let seq = self.reg(a)?;
        let cur = self.reg(a + 1)?;
        let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let status =
            self.call_helper(self.sig_gg, self.addr_iterdone, &[seq, cur, out_addr]);
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        // `pc + 2` resume is only reachable if the helper ever fails; pass
        // through the infallible status identically to every other guard.
        self.guard(ok, go, _pc);
        self.enter(go);
        let v = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(b, v);
        Ok(())
    }

    /// `CallNative dst=a, native_id=b, argc=c`: arguments are spilled first;
    /// errors deopt so the interpreter raises them.
    fn lower_call_native(
        &mut self,
        pc: usize,
        a: usize,
        b: usize,
        argc: usize,
    ) -> Result<(), String> {
        let first = a
            .checked_sub(argc)
            .ok_or_else(|| "native arg window underflow".to_string())?;
        if first + argc > self.max_reg {
            return Err("bad native window".into());
        }
        let rb = self.rbase();
        for i in 0..argc {
            let v = self.reg(first + i)?;
            self.b
                .ins()
                .store(MemFlagsData::new(), v, rb, ((first + i) * 8) as i32);
        }
        let vm = self.p_vm;
        let regs_ptr = self.p_regs;
        let base = self.p_base;
        let firstv = self.iconst(first as u64);
        let argv = self.iconst(argc as u64);
        let nidv = self.iconst(b as u64);
        let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let status = self.call_helper(
            self.sig_call,
            self.addr_callnative,
            &[vm, regs_ptr, base, firstv, argv, nidv, out_addr],
        );
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let v = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(a, v);
        Ok(())
    }

    fn lower_call(&mut self, pc: usize, a: usize, argc: usize, first: usize) -> Result<(), String> {
        if first == 0 || first + argc > self.max_reg {
            return Err("bad call window".into());
        }
        // Spill the callee and its arguments to the register window: a callee
        // (native or interpreter) reads them from memory.
        let rb = self.rbase();
        let callee_v = self.reg(first - 1)?;
        self.b
            .ins()
            .store(MemFlagsData::new(), callee_v, rb, ((first - 1) * 8) as i32);
        for i in 0..argc {
            let v = self.reg(first + i)?;
            self.b
                .ins()
                .store(MemFlagsData::new(), v, rb, ((first + i) * 8) as i32);
        }

        let cont = self.ensure_block(pc + 1);
        let out_addr = self.b.ins().stack_addr(types::I64, self.out_slot, 0);
        let base2 = self.b.ins().iadd_imm_s(self.p_base, first as i64);

        // A recursive call passes the *same* closure value we were invoked as, so
        // comparing the callee register against our own closure detects
        // self-recursion with a single compare — no per-call global lookup.
        let self_blk = self.new_block();
        let gen_blk = self.new_block();
        let is_self = self.b.ins().icmp(IntCC::Equal, callee_v, self.p_closure);
        let args = self.block_args();
        self.b.ins().brif(is_self, self_blk, &args, gen_blk, &args);
        self.terminated = true;

        // Direct self-recursion: call our own compiled code.
        self.enter(self_blk);
        let status = self.b.ins().call(
            self.self_ref,
            &[self.p_regs, base2, self.p_vm, out_addr, self.p_closure],
        );
        let status = self.b.inst_results(status)[0];
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let res = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(a, res);
        let args = self.block_args();
        self.b.ins().jump(cont, &args);
        self.terminated = true;

        // Otherwise go through the cached general path (other functions,
        // builtins): an inline per-site entry cache, refreshed by the bridge.
        self.enter(gen_blk);
        self.emit_general_call(pc, a, argc, first, callee_v, out_addr);
        let args = self.block_args();
        self.b.ins().jump(cont, &args);
        self.terminated = true;

        self.enter(cont);
        Ok(())
    }

    /// Emit the general (non-self) call. When per-site cache cells are
    /// available, consult the inline `(callee, entry, max_reg)` triple first:
    /// a hit plus a passing window check calls the callee entry directly with
    /// no VM round-trip. A miss (or no cells) goes through
    /// `zen_jit_call_cached`, which refreshes the triple on success. A nonzero
    /// nested status degrades to our own resume at `pc`: the interpreter
    /// re-executes the call (and the callee restarts inside it).
    fn emit_general_call(
        &mut self,
        pc: usize,
        a: usize,
        argc: usize,
        first: usize,
        callee_v: ClValue,
        out_addr: ClValue,
    ) {
        let vm = self.p_vm;
        let regs_ptr = self.p_regs;
        let base = self.p_base;
        let firstv = self.iconst(first as u64);
        let argv = self.iconst(argc as u64);
        let ret = self.iconst(a as u64);
        if self.call_cells == 0 {
            let status = self.call_helper(
                self.sig_call,
                self.addr_call,
                &[vm, regs_ptr, base, firstv, argv, ret, out_addr],
            );
            let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
            let go = self.new_block();
            self.guard(ok, go, pc);
            self.enter(go);
            let res = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
            self.set(a, res);
            return;
        }
        let idx = self.call_site;
        self.call_site += 1;
        let cell = self.iconst(self.call_cells + idx as u64 * 24);
        let cached_callee = self.b.ins().load(types::I64, MemFlagsData::new(), cell, 0);
        let cached_entry = self.b.ins().load(types::I64, MemFlagsData::new(), cell, 8);
        let cached_max = self.b.ins().load(types::I64, MemFlagsData::new(), cell, 16);
        let same = self.b.ins().icmp(IntCC::Equal, callee_v, cached_callee);
        let live = self.b.ins().icmp_imm_s(IntCC::NotEqual, cached_entry, 0);
        let hit_callee = self.b.ins().band(same, live);
        let hit_blk = self.new_block();
        let miss_blk = self.new_block();
        let joined = self.new_block();
        let args = self.block_args();
        self.b.ins().brif(hit_callee, hit_blk, &args, miss_blk, &args);
        self.terminated = true;
        // Hit: the callee matches a previously resolved entry. Re-verify the
        // register window fits (the base is dynamic), then call directly.
        self.enter(hit_blk);
        let base2 = self.b.ins().iadd_imm_s(self.p_base, first as i64);
        let end = self.b.ins().iadd(base2, cached_max);
        let fits = self.b.ins().icmp_imm_s(IntCC::UnsignedLessThanOrEqual, end, 256);
        let exec_blk = self.new_block();
        let args = self.block_args();
        self.b.ins().brif(fits, exec_blk, &args, miss_blk, &args);
        self.terminated = true;
        self.enter(exec_blk);
        let call = self.b.ins().call_indirect(
            self.sig_native,
            cached_entry,
            &[regs_ptr, base2, vm, out_addr, callee_v],
        );
        let status = self.b.inst_results(call)[0];
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let res = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(a, res);
        let args = self.block_args();
        self.b.ins().jump(joined, &args);
        self.terminated = true;
        // Miss: resolve through the bridge, which refreshes the triple.
        self.enter(miss_blk);
        let status = self.call_helper(
            self.sig_mcall,
            self.addr_call_cached,
            &[vm, regs_ptr, base, firstv, argv, ret, out_addr, cell],
        );
        let ok = self.b.ins().icmp_imm_s(IntCC::Equal, status, 0);
        let go = self.new_block();
        self.guard(ok, go, pc);
        self.enter(go);
        let res = self.b.ins().load(types::I64, MemFlagsData::new(), out_addr, 0);
        self.set(a, res);
        let args = self.block_args();
        self.b.ins().jump(joined, &args);
        self.terminated = true;
        self.enter(joined);
    }
}

fn jump_target(pc: usize, b: u8, c: u8, len: usize) -> Result<usize, String> {
    let offset = i16::from_be_bytes([b, c]) as i64;
    let target = pc as i64 + 1 + offset;
    if target < 0 || target as usize > len {
        return Err(format!("jump target {target} out of range"));
    }
    Ok(target as usize)
}

/// Loop headers: targets of backward jumps. Each gets its own OSR entry so
/// the interpreter can transfer into native code mid-loop.
fn loop_headers(snap: &CompileSnapshot) -> Vec<usize> {
    let len = snap.code.len();
    let mut out: Vec<usize> = Vec::new();
    for (pc, inst) in snap.code.iter().enumerate() {
        match inst.op() {
            Opcode::Jump | Opcode::JumpIfFalse | Opcode::JumpIfTrue => {
                if let Ok(t) = jump_target(pc, inst.b(), inst.c(), len) {
                    if t <= pc && !out.contains(&t) {
                        out.push(t);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The type of a `Value` register slot, kept next to the lowering for clarity.
#[allow(dead_code)]
const REG_TY: Type = types::I64;
