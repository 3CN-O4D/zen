//! Hermetic Stage 2 tests: compile a small integer function and invoke the
//! native entry directly, without the compiler or a `Vm`. Exercises integer
//! arithmetic, the type guard, overflow, and the deopt restore path.
#![cfg(feature = "jit")]

use zen_jit::{CraneliftCompiler, ZenJitCompiler};
use zen_vm::bytecode::{Instruction, Opcode};
use zen_vm::jit::{CompileSnapshot, NativeEntry};

const INT_HDR: u64 = 0x7FFB_0000_0000_0000;
const PAYLOAD: u64 = 0x0000_FFFF_FFFF_FFFF;
const INT_MAX: i64 = (1i64 << 47) - 1;
const INT_MIN: i64 = -(1i64 << 47);
const TRUE: u64 = 0x7FFA_0000_0000_0000;
const FALSE: u64 = 0x7FF9_0000_0000_0000;

fn int(n: i64) -> u64 {
    INT_HDR | (n as u64 & PAYLOAD)
}

fn float(f: f64) -> u64 {
    f.to_bits()
}

fn decode_float(bits: u64) -> f64 {
    f64::from_bits(bits)
}

fn decode_int(bits: u64) -> i64 {
    ((bits as i64) << 16) >> 16
}

/// `def add3(a, b): return a + b`  (child proto registers: them as a,b then a temp)
fn add_snapshot() -> CompileSnapshot {
    CompileSnapshot {
        name: "add3".into(),
        params: vec!["a".into(), "b".into()],
        module_id: 0,
        num_globals: 0,
        num_locals: 2,
        max_reg: 3,
        num_required: 2,
        num_upvalues: 0,
        code: vec![
            Instruction::new(Opcode::Add, 2, 0, 1),
            Instruction::new(Opcode::Return, 2, 0, 0),
        ],
        constants: vec![],
        line_table: vec![],
        upvalue_descriptors: vec![],
        ic_slots: vec![],
        feedback: vec![],
        is_method: false,
    }
}

/// `def mul2(a, b): return a * b`
fn mul_snapshot() -> CompileSnapshot {
    CompileSnapshot {
        name: "mul2".into(),
        params: vec!["a".into(), "b".into()],
        module_id: 0,
        num_globals: 0,
        num_locals: 2,
        max_reg: 3,
        num_required: 2,
        num_upvalues: 0,
        code: vec![
            Instruction::new(Opcode::Mul, 2, 0, 1),
            Instruction::new(Opcode::Return, 2, 0, 0),
        ],
        constants: vec![],
        line_table: vec![],
        upvalue_descriptors: vec![],
        ic_slots: vec![],
        feedback: vec![],
        is_method: false,
    }
}

/// `def f(a, b): return a <op> b` for a binary op.
fn bin_snapshot(name: &str, op: Opcode) -> CompileSnapshot {
    CompileSnapshot {
        name: name.into(),
        params: vec!["a".into(), "b".into()],
        module_id: 0,
        num_globals: 0,
        num_locals: 2,
        max_reg: 3,
        num_required: 2,
        num_upvalues: 0,
        code: vec![
            Instruction::new(op, 2, 0, 1),
            Instruction::new(Opcode::Return, 2, 0, 0),
        ],
        constants: vec![],
        line_table: vec![],
        upvalue_descriptors: vec![],
        ic_slots: vec![],
        feedback: vec![],
        is_method: false,
    }
}

fn div_snapshot() -> CompileSnapshot {
    bin_snapshot("div", Opcode::Div)
}

fn mod_snapshot() -> CompileSnapshot {
    bin_snapshot("mod", Opcode::Mod)
}

fn lt_snapshot() -> CompileSnapshot {
    bin_snapshot("lt", Opcode::Lt)
}

/// `def neg1(a): return -a`  (a copy of a lands in a temp register)
fn neg_snapshot() -> CompileSnapshot {
    CompileSnapshot {
        name: "neg1".into(),
        params: vec!["a".into()],
        module_id: 0,
        num_globals: 0,
        num_locals: 1,
        max_reg: 2,
        num_required: 1,
        num_upvalues: 0,
        code: vec![
            Instruction::new(Opcode::Move, 1, 0, 0),
            Instruction::new(Opcode::Neg, 1, 1, 0),
            Instruction::new(Opcode::Return, 1, 0, 0),
        ],
        constants: vec![],
        line_table: vec![],
        upvalue_descriptors: vec![],
        ic_slots: vec![],
        feedback: vec![],
        is_method: false,
    }
}

/// `def bit(a, b): return (a << b) & a`
fn bit_snapshot() -> CompileSnapshot {
    CompileSnapshot {
        name: "bit".into(),
        params: vec!["a".into(), "b".into()],
        module_id: 0,
        num_globals: 0,
        num_locals: 2,
        max_reg: 3,
        num_required: 2,
        num_upvalues: 0,
        code: vec![
            Instruction::new(Opcode::Shl, 2, 0, 1),
            Instruction::new(Opcode::BitAnd, 2, 2, 0),
            Instruction::new(Opcode::Return, 2, 0, 0),
        ],
        constants: vec![],
        line_table: vec![],
        upvalue_descriptors: vec![],
        ic_slots: vec![],
        feedback: vec![],
        is_method: false,
    }
}

#[test]
fn mul_overflow_promotes_to_float() {
    let snap = mul_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    let mut regs = [0u64; 256];
    regs[0] = int(INT_MAX);
    regs[1] = int(2);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0, "int overflow is promoted inline");
    // Mirrors `ops::overflow_float`: (a as f64) * (b as f64).
    assert_eq!(decode_float(out), (INT_MAX as f64) * 2.0);
}

#[test]
fn lowers_float_add() {
    let snap = add_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    let mut regs = [0u64; 256];
    regs[0] = float(1.5);
    regs[1] = float(2.25);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_float(out), 3.75);

    // Mixed int/float widens the int first.
    let mut regs = [0u64; 256];
    regs[0] = int(3);
    regs[1] = float(0.5);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_float(out), 3.5);

    // A non-number (here nil) deopts so the interpreter can handle it.
    let mut regs = [0u64; 256];
    regs[0] = float(1.0);
    regs[1] = 0x7FF8_0000_0000_0000; // nil
    let (status, _) = call(entry, &mut regs, 0);
    assert_eq!(status, 2);
}

#[test]
fn lowers_float_neg() {
    let snap = neg_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    let mut regs = [0u64; 256];
    regs[0] = float(2.5);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_float(out), -2.5);
}

#[test]
fn lowers_division_to_float() {
    let snap = div_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    // Integer division still yields a float, mirroring `ops::int_arith`.
    let mut regs = [0u64; 256];
    regs[0] = int(7);
    regs[1] = int(2);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_float(out), 3.5);

    let mut regs = [0u64; 256];
    regs[0] = float(1.0);
    regs[1] = float(4.0);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_float(out), 0.25);

    // Dividing by zero raises in the interpreter, so the JIT deopts.
    let mut regs = [0u64; 256];
    regs[0] = int(1);
    regs[1] = int(0);
    let (status, _) = call(entry, &mut regs, 0);
    assert_eq!(status, 2);
}

#[test]
fn lowers_modulo() {
    let snap = mod_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    let mut regs = [0u64; 256];
    regs[0] = int(17);
    regs[1] = int(5);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_int(out), 2);

    // Modulo by zero raises → deopt.
    let mut regs = [0u64; 256];
    regs[0] = int(5);
    regs[1] = int(0);
    let (status, _) = call(entry, &mut regs, 0);
    assert_eq!(status, 2);

    // INT_MIN % -1 is the interpreter's float-promotion case → deopt.
    let mut regs = [0u64; 256];
    regs[0] = int(INT_MIN);
    regs[1] = int(-1);
    let (status, _) = call(entry, &mut regs, 0);
    assert_eq!(status, 2);

    // A float operand is not lowered (interpreter handles float mod).
    let mut regs = [0u64; 256];
    regs[0] = float(5.5);
    regs[1] = int(2);
    let (status, _) = call(entry, &mut regs, 0);
    assert_eq!(status, 2);
}

#[test]
fn lowers_mixed_numeric_comparison() {
    let snap = lt_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    let mut regs = [0u64; 256];
    regs[0] = int(1);
    regs[1] = float(2.5);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(out, TRUE);

    let mut regs = [0u64; 256];
    regs[0] = float(9.5);
    regs[1] = int(2);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(out, FALSE);
}

fn call(entry: NativeEntry, regs: &mut [u64; 256], base: usize) -> (u32, u64) {
    let mut out: u64 = 0;
    let status = unsafe {
        entry(
            regs.as_mut_ptr(),
            base,
            std::ptr::null_mut(),
            &mut out as *mut u64,
            0,
        )
    };
    (status, out)
}

#[test]
fn lowers_integer_add() {
    let snap = add_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    let mut regs = [0u64; 256];
    regs[0] = int(20);
    regs[1] = int(22);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_int(out), 42);
}

#[test]
fn type_guard_deopts_and_restores_window() {
    let snap = add_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    let mut regs = [0u64; 256];
    // A non-number (nil): the guard must fail and deopt.
    regs[0] = 0x7FF8_0000_0000_0000; // nil
    regs[1] = int(1);
    let before = [regs[0], regs[1]];
    let (status, _) = call(entry, &mut regs, 0);
    assert_eq!(status, 2, "expected deopt resuming at pc 0");
    assert_eq!([regs[0], regs[1]], before, "window spilled for interpreter");
}

#[test]
fn overflow_promotes_to_float() {
    let snap = add_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    // Integer overflow promotes to float inline, so the result is a float,
    // not a deopt. Mirrors `ops::overflow_float`.
    let mut regs = [0u64; 256];
    regs[0] = int(INT_MAX);
    regs[1] = int(INT_MAX);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_float(out), (INT_MAX as f64) + (INT_MAX as f64));
}

#[test]
fn lowers_integer_mul() {
    let snap = mul_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    let mut regs = [0u64; 256];
    regs[0] = int(6);
    regs[1] = int(7);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_int(out), 42);

    let mut regs = [0u64; 256];
    regs[0] = int(-4);
    regs[1] = int(9);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_int(out), -36);
}

#[test]
fn lowers_neg() {
    let snap = neg_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    let mut regs = [0u64; 256];
    regs[0] = int(7);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_int(out), -7);

    let mut regs = [0u64; 256];
    regs[0] = int(INT_MIN as i64);
    let (status, _) = call(entry, &mut regs, 0);
    assert_eq!(status, 3, "negating INT_MIN deopts (interpreter raises)");
}

#[test]
fn lowers_bitwise_and_shl() {
    let snap = bit_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    let mut regs = [0u64; 256];
    regs[0] = int(5); // 101
    regs[1] = int(1); // (5<<1)=10 -> 1010 & 101 = 0
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_int(out), 0);

    let mut regs = [0u64; 256];
    regs[0] = int(13); // 1101
    regs[1] = int(2); // (13<<2)=52 -> 110100 & 001101 = 000100 = 4
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_int(out), 4);
}

#[test]
fn shift_count_is_masked_to_63() {
    let snap = bit_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    // b = 67 masks to 3, mirroring the interpreter's `(b & 63) as u32`.
    let mut regs = [0u64; 256];
    regs[0] = int(5);
    regs[1] = int(67);
    let (status, out) = call(entry, &mut regs, 0);
    assert_eq!(status, 0);
    assert_eq!(decode_int(out), 0); // (5<<3)=40 & 5 = 0
}

#[test]
fn bitwise_overflow_deopts() {
    let snap = bit_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    // (1<<46)-1 << 2 = 2^48-4 leaves the 48-bit range: interpreter would
    // promote to float, so the JIT must deopt.
    let mut regs = [0u64; 256];
    regs[0] = int((1i64 << 46) - 1);
    regs[1] = int(2);
    let (status, _) = call(entry, &mut regs, 0);
    assert_eq!(status, 2, "expected overflow deopt");
}

#[test]
fn uses_a_window_base() {
    let snap = add_snapshot();
    let entry = CraneliftCompiler::new().compile(&snap).expect("compiles").entry;
    let entry: NativeEntry = unsafe { std::mem::transmute(entry) };

    // Arguments live at base..base+2; the frame must read its own window.
    let mut regs = [0u64; 256];
    regs[40] = int(7);
    regs[41] = int(35);
    let (status, out) = call(entry, &mut regs, 40);
    assert_eq!(status, 0);
    assert_eq!(decode_int(out), 42);
}
