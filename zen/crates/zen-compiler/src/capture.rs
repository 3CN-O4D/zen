//! Read-only capture optimization: `MakeClosure` → `MakeClosureRO`.
//!
//! A closure capture is *read-only* when no store can reach the captured slot:
//! the defining function never writes the local (no register-writing opcode
//! targets it), no sibling closure created in the same function writes it
//! through its own capture, and the capturing subtree contains no `SetUpval`
//! to the corresponding upvalue (transitively). Read-only captures snapshot
//! the value instead of promoting the local to a shared cell: one fewer
//! allocation, no cell indirection on reads, and — crucially — capturable
//! from native code, whose spilled window holds the current values but has no
//! interpreter frame to promote in.
//!
//! The analysis is whole-module and runs after all functions are compiled, so
//! sibling ordering cannot hide a writer (a later-created writer still
//! disqualifies an earlier-created reader). Every doubt resolves toward the
//! cell path: a missed optimization is always correct, while a missed writer
//! would be a wrong answer. In particular:
//! - only `is_local` captures of leading parameter slots qualify (nested
//!   locals / loop variables / temps are never trusted, even if they look
//!   clean — slot reuse makes flow-insensitive reasoning about them fragile);
//! - any closure-creation site storing *into* the slot (`a == slot`, including
//!   self-capture for recursion) disqualifies it;
//! - unknown shapes (truncated descriptors, exhausted fuel) disqualify.

use zen_vm::bytecode::{FunctionProto, UpvalueDesc};
use zen_vm::{Instruction, Opcode};

/// Maximum transitive capture-chain depth before giving up (conservatively).
const FUEL: u8 = 16;

/// Rewrite provably read-only `MakeClosure` sites to `MakeClosureRO` in
/// `main` and every child prototype. Same operands, no code motion — jump
/// targets and line tables are untouched.
pub fn optimize_captures(main: &mut FunctionProto, children: &mut [FunctionProto]) {
    // Definers in a fixed order: main first, then children by index.
    let definer_count = children.len() + 1;
    for d in 0..definer_count {
        // Collect candidate sites first (immutable borrows), then patch.
        let sites: Vec<(usize, u8, u8, u8)> = {
            let code = definer_code(main, children, d);
            code.iter()
                .enumerate()
                .filter_map(|(pc, inst)| {
                    if inst.op() == Opcode::MakeClosure {
                        Some((pc, inst.a(), inst.b(), inst.c()))
                    } else {
                        None
                    }
                })
                .collect()
        };
        for (pc, a, b, c) in sites {
            if c == 0 {
                continue; // nothing captured; the plain form is already optimal.
            }
            if site_is_readonly(main, children, d, a, b, c) {
                let code = definer_code_mut(main, children, d);
                code[pc] = Instruction::new(Opcode::MakeClosureRO, a, b, c);
            }
        }
    }
}

fn definer_code<'m>(
    main: &'m FunctionProto,
    children: &'m [FunctionProto],
    d: usize,
) -> &'m [Instruction] {
    if d == 0 {
        &main.code
    } else {
        &children[d - 1].code
    }
}

fn definer_code_mut<'m>(
    main: &'m mut FunctionProto,
    children: &'m mut [FunctionProto],
    d: usize,
) -> &'m mut [Instruction] {
    if d == 0 {
        &mut main.code
    } else {
        &mut children[d - 1].code
    }
}

fn definer_params(main: &FunctionProto, children: &[FunctionProto], d: usize) -> usize {
    let p = if d == 0 { &main.params } else { &children[d - 1].params };
    let is_method = if d == 0 {
        main.is_method
    } else {
        children[d - 1].is_method
    };
    // Leading slots are `self` (methods only) followed by the declared params.
    p.len() + usize::from(is_method)
}

/// Whether the `MakeClosure(a, child=b, count=c)` site in definer `d` captures
/// only read-only values.
fn site_is_readonly(
    main: &FunctionProto,
    children: &[FunctionProto],
    d: usize,
    a: u8,
    b: u8,
    c: u8,
) -> bool {
    let Some(child) = children.get(b as usize) else {
        return false;
    };
    let taken = match child.upvalue_descriptors.get(..c as usize) {
        Some(t) => t,
        None => return false,
    };
    let definer = definer_code(main, children, d);
    let params = definer_params(main, children, d);
    for (i, desc) in taken.iter().enumerate() {
        // Only leading parameter slots of the definer qualify.
        if !desc.is_local || desc.index as usize >= params {
            return false;
        }
        let Ok(slot) = u8::try_from(desc.index) else {
            return false;
        };
        // The site must not store into a slot it captures (self-capture for
        // recursion being the classic case: the cell is what makes the name
        // resolve after creation).
        if a == slot {
            return false;
        }
        // The definer itself must never write the slot.
        if definer_writes_slot(definer, slot) {
            return false;
        }
        // The capturing subtree must never write the corresponding upvalue.
        if upval_written(children, b as usize, i, FUEL) {
            return false;
        }
        // No sibling created in the same definer may write it either (a
        // later-created writer still counts — this runs post-compile).
        if sibling_writes_slot(main, children, d, b as usize, slot) {
            return false;
        }
    }
    true
}

/// Whether any instruction in `code` writes register `slot`. Conservative:
/// every opcode is assumed to write its `a` operand unless the operand names
/// something else entirely (a jump condition, an upvalue/global index).
/// `IterInit`/`IterNext` additionally write `a + 1` (the cursor);
/// `IterDone`/`ChanSend`/`ChanRecv` additionally write `b`.
fn definer_writes_slot(code: &[Instruction], slot: u8) -> bool {
    code.iter().any(|inst| {
        match inst.op() {
            Opcode::Jump | Opcode::JumpIfFalse | Opcode::JumpIfTrue => false,
            Opcode::SetUpval => false, // `a` is an upvalue index, not a register.
            Opcode::SetGlobal => false, // `a` is a global index, not a register.
            Opcode::IterInit | Opcode::IterNext => {
                inst.a() == slot || inst.a().wrapping_add(1) == slot
            }
            Opcode::IterDone | Opcode::ChanSend | Opcode::ChanRecv => {
                inst.a() == slot || inst.b() == slot
            }
            _ => inst.a() == slot,
        }
    })
}

/// Whether any *other* closure created in definer `d` captures `slot` and
/// writes through that capture (transitively). `our_child`/`our site` are
/// included harmlessly — their own subtree check is idempotent.
fn sibling_writes_slot(
    main: &FunctionProto,
    children: &[FunctionProto],
    d: usize,
    _our_child: usize,
    slot: u8,
) -> bool {
    let definer = definer_code(main, children, d);
    for inst in definer.iter() {
        let op = inst.op();
        if op != Opcode::MakeClosure && op != Opcode::MakeClosureRO {
            continue;
        }
        let (sb, sc) = (inst.b(), inst.c());
        // A creation site storing into the slot changes what a snapshot would
        // mean (including self-capture) — disqualify regardless of position.
        if inst.a() == slot {
            return true;
        }
        let Some(schild) = children.get(sb as usize) else {
            return true;
        };
        let taken = match schild.upvalue_descriptors.get(..sc as usize) {
            Some(t) => t,
            None => return true,
        };
        for (j, sdesc) in taken.iter().enumerate() {
            if sdesc.is_local
                && sdesc.index == slot as u16
                && upval_written(children, sb as usize, j, FUEL)
            {
                return true;
            }
        }
    }
    false
}

/// Whether upvalue `pos` of child `c` is written anywhere in its capturing
/// subtree: a direct `SetUpval`, any `CloseUpval` (never emitted today, but
/// conservatively fatal), or a nested capture that forwards the same upvalue
/// and writes through it.
fn upval_written(children: &[FunctionProto], c: usize, pos: usize, fuel: u8) -> bool {
    if fuel == 0 {
        return true;
    }
    let Some(child) = children.get(c) else {
        return true;
    };
    for inst in child.code.iter() {
        match inst.op() {
            Opcode::SetUpval if inst.a() as usize == pos => return true,
            Opcode::CloseUpval => return true,
            Opcode::MakeClosure | Opcode::MakeClosureRO => {
                let (gb, gc) = (inst.b() as usize, inst.c() as usize);
                let Some(gchild) = children.get(gb) else {
                    return true;
                };
                let taken = match gchild.upvalue_descriptors.get(..gc) {
                    Some(t) => t,
                    None => return true,
                };
                for (j, gdesc) in taken.iter().enumerate() {
                    if !gdesc.is_local && gdesc.index as usize == pos
                        && upval_written(children, gb, j, fuel - 1)
                    {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// For tests: the descriptors taken by a site (bounds-checked).
#[allow(dead_code)]
pub fn taken_descriptors<'c>(
    children: &'c [FunctionProto],
    b: u8,
    c: u8,
) -> Option<&'c [UpvalueDesc]> {
    children
        .get(b as usize)
        .and_then(|ch| ch.upvalue_descriptors.get(..c as usize))
}
