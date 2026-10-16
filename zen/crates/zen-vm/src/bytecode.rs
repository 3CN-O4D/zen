//! Bytecode instruction encoding and function prototype.
//! See `docs/07-bytecode.md`.
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    // Constants
    LoadNil,
    LoadTrue,
    LoadFalse,
    LoadConst,
    LoadConstWide,
    // Moves
    Move,
    // Arithmetic (unified with type guards)
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Neg,
    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // Logical
    And,
    Or,
    Not,
    // Jumps
    Jump,
    JumpIfFalse,
    JumpIfTrue,
    // Calls
    Call,
    CallNative,
    CallMethod,
    TailCall,
    Return,
    // Containers
    MakeList,
    MakeDict,
    MakeSet,
    MakeTuple,
    // Index / field
    GetIndex,
    SetIndex,
    GetField,
    SetField,
    // Iteration
    IterInit,
    IterNext,
    IterDone,
    // Upvalues / closures
    MakeClosure,
    GetUpval,
    SetUpval,
    CloseUpval,
    // Exceptions. `TryEnter` carries the error slot in `a` and the signed
    // 16-bit catch offset in `(b, c)`. A typed `catch` clause compares the
    // pending error's kind (`ErrorKind`) and re-raises on a mismatch
    // (`Rethrow`), so one handler can chain to the next clause outward.
    TryEnter,
    TryExit,
    Throw,
    ThrowVal,
    // Concurrency
    Spawn,
    ChanMake,
    ChanSend,
    ChanRecv,
    // JIT interface
    TypeGuard,
    Deopt,
    Safepoint,
    // Misc
    Nop,
    // ── M4 append-only extension ─────────────────────────────────────────
    // Added by the M4 compiler. Appended *after* `Nop` so every pre-existing
    // discriminant (0..=56) is unchanged; `from_u8` below stays exhaustive.
    BitAnd,
    BitOr,
    BitXor,
    BitNot,
    Shl,
    Shr,
    // ── M4 OOP extension (append-only, ABI v2) ───────────────────────────
    // Classes are built at runtime from a shared `Class` object. Property
    // reads/writes reuse `GetField`/`SetField` (which dispatch on the receiver
    // type); `GetProperty`/`SetProperty` are reserved aliases for the JIT that
    // will receive inline-cache padding in M5.
    // a=dest, b=name const, c=parent reg (0xFF = none)
    MakeClass,
    // a=class, b=name const, c=closure reg
    AddMethod,
    // a=class, b=name const, c=default reg
    AddField,
    // a=dest, b=obj, c=name const (JIT/IC alias of GetField)
    GetProperty,
    // a=obj, b=name const, c=value reg (JIT/IC alias of SetField)
    SetProperty,
    // Module-scope variable shared by every function in the module. The module
    // frame lives at base 0, so `b`/`a` index its register file directly.
    // a=dest, b=global index
    GetGlobal,
    // a=global index, b=source reg
    SetGlobal,
    // ── M4 module system (append-only, ABI v2) ───────────────────────────
    // Resolve and run a module exactly once, then write its export dictionary
    // (a `dict`) into `a`. `b` indexes a string constant holding the module
    // path. The VM's `ModuleRegistry` owns loading state and cycle detection.
    // a=dest, b=module-name constant index
    Import,
    // ── Exception handling (ABI v2, append-only) ─────────────────────────
    // `TryEnter`/`TryExit` (45/46) predate M4 and had no implementation until
    // now. `TryEnter` carries the error slot in `a` and the signed 16-bit
    // catch offset in `(b, c)`. A typed `catch` clause compares the pending
    // error's kind and re-raises on a mismatch, so a handler can chain to the
    // next clause outward exactly as the oracle's clause search does.
    // a=dest: the pending error's kind name (e.g. "TypeError")
    ErrorKind,
    // Re-raise the pending error, preserving its original kind and message.
    Rethrow,
    // ── M4 for-loop destructuring (append-only, ABI v4) ───────────────────
    // A multi-variable `for` header binds one name per element of the item the
    // iterator yields. `UnpackCheck` enforces the arity: the oracle raises
    // `ValueError` when the item is shorter than the header, so the message
    // travels as a string constant and the compiler keeps the wording on its
    // side of the ABI.
    // a=item reg, b=required count (immediate), c=error-message constant
    UnpackCheck,
    // Element extraction deliberately uses the *iteration* protocol instead of
    // `GetIndex`, so unpacking a dict yields its keys and a byte string yields
    // byte values — exactly what the oracle's `iter_get` does.
    // a=dest, b=item reg, c=position (immediate)
    IterIndex,
    // ── M4 comprehensions and ranges (append-only, ABI v5) ────────────────
    // Append one element to a list held in a register. The accumulator of a
    // comprehension is built this way: `MakeList` gives it an empty list, and
    // every surviving iteration appends to it in place. This is the same
    // copy-on-write mutation `xs.push(v)` performs, without the method-call
    // plumbing (name constant plus receiver/argument/result temporaries) that
    // a four-instruction `CallMethod` would need per append.
    // a=list reg, b=item reg
    ListAppend,
    // `lo -> hi` (and `lo .. hi`, which the parser makes indistinguishable —
    // both lower to `BinaryOp::Range`). The oracle materialises the range
    // eagerly as a list of `int_promote`d integers, inclusive of `hi`, and
    // raises `TypeError: expected int` naming the *left* operand when either
    // end is not an integer. Doing the work here instead of inlining a counter
    // loop keeps that error, and the `int_promote` overflow rule, identical to
    // the oracle's, which spells both out in one place.
    // a=dest, b=lo reg, c=hi reg
    MakeRange,
    // ── M4 destructuring patterns (append-only, ABI v6) ─────────────────
    // `len(subject)` as a register, for the arity check a tuple or list
    // pattern has to make before it binds anything. This is `ops::iter_len`
    // rather than the `len` native on purpose: `iter_len` yields 0 for a
    // value that cannot be iterated (a number, nil, a function), so a pattern
    // simply fails to match, while `len` raises `TypeError`. The oracle's
    // `pattern_matches` measures with `iter_len`, so the two agree on a
    // non-iterable subject instead of one raising and the other declining.
    // a=dest, b=item reg
    IterLen,
    // `subject[start:end]` — a slice. The bounds are two consecutive registers
    // starting at `c`, each `nil` when the source omitted it, which is the same
    // `MakeList` first/count idiom: one `nil` distinguishes "omitted" from an
    // explicit `0`, and the omitted `end` means the length.
    // a=dest, b=subject reg, c=first bound reg
    Slice,
    // ── Wide native call (ABI v8, append-only) ───────────────────────────
    // `CallNative` encodes the native id in a single `b` byte, which caps the
    // native namespace at 256 ids. This two-word form lifts the cap: the id is
    // a `u16` carried by the *next* instruction word (a `Nop` whose `a`/`b`
    // hold the high and low bytes). The id occupies the same window as
    // `CallNative`: `a`=dest, `b`=argc. Linear scanners see the payload as a
    // `Nop`, so jump verification and opcode counting stay valid.
    CallNativeW,
    // ── Read-only closure capture (append-only) ─────────────────────────
    // Same operands as `MakeClosure` (`a`=dest, `b`=child index, `c`=count),
    // but every taken capture is proven read-only by the compiler's capture
    // post-pass: no defining-frame store and no transitive `SetUpval` can
    // reach a captured slot. The VM therefore snapshots the values instead of
    // promoting locals to shared cells (no allocation beyond the closure
    // itself), and native code can capture from its spilled window without
    // needing an interpreter frame.
    MakeClosureRO,
    // ── Foreign Python import (append-only) ─────────────────────────────
    // Same shape as `Import` (`a`=dest, `b`=module-path constant, `c`
    // unused): loads a CPython module through the process Python bridge and
    // binds the foreign module handle. A distinct opcode (not a flag on
    // `Import`) so the parser, compiler, linker, and JIT always know this
    // crosses into CPython — native imports never pay a foreign check.
    PyImport,
}

/// Serialized-bytecode format version. Bumped when the opcode set or object
/// kind table grows so a loader can refuse stale `.zc` files.
pub const BYTECODE_VERSION: u16 = 10;

impl Opcode {
    /// Safe discriminant decode; `None` for a corrupt high byte.
    pub const fn from_u8(b: u8) -> Option<Opcode> {
        Some(match b {
            0 => Opcode::LoadNil,
            1 => Opcode::LoadTrue,
            2 => Opcode::LoadFalse,
            3 => Opcode::LoadConst,
            4 => Opcode::LoadConstWide,
            5 => Opcode::Move,
            6 => Opcode::Add,
            7 => Opcode::Sub,
            8 => Opcode::Mul,
            9 => Opcode::Div,
            10 => Opcode::Mod,
            11 => Opcode::Pow,
            12 => Opcode::Neg,
            13 => Opcode::Eq,
            14 => Opcode::Ne,
            15 => Opcode::Lt,
            16 => Opcode::Le,
            17 => Opcode::Gt,
            18 => Opcode::Ge,
            19 => Opcode::And,
            20 => Opcode::Or,
            21 => Opcode::Not,
            22 => Opcode::Jump,
            23 => Opcode::JumpIfFalse,
            24 => Opcode::JumpIfTrue,
            25 => Opcode::Call,
            26 => Opcode::CallNative,
            27 => Opcode::CallMethod,
            28 => Opcode::TailCall,
            29 => Opcode::Return,
            30 => Opcode::MakeList,
            31 => Opcode::MakeDict,
            32 => Opcode::MakeSet,
            33 => Opcode::MakeTuple,
            34 => Opcode::GetIndex,
            35 => Opcode::SetIndex,
            36 => Opcode::GetField,
            37 => Opcode::SetField,
            38 => Opcode::IterInit,
            39 => Opcode::IterNext,
            40 => Opcode::IterDone,
            41 => Opcode::MakeClosure,
            42 => Opcode::GetUpval,
            43 => Opcode::SetUpval,
            44 => Opcode::CloseUpval,
            45 => Opcode::TryEnter,
            46 => Opcode::TryExit,
            47 => Opcode::Throw,
            48 => Opcode::ThrowVal,
            49 => Opcode::Spawn,
            50 => Opcode::ChanMake,
            51 => Opcode::ChanSend,
            52 => Opcode::ChanRecv,
            53 => Opcode::TypeGuard,
            54 => Opcode::Deopt,
            55 => Opcode::Safepoint,
            56 => Opcode::Nop,
            57 => Opcode::BitAnd,
            58 => Opcode::BitOr,
            59 => Opcode::BitXor,
            60 => Opcode::BitNot,
            61 => Opcode::Shl,
            62 => Opcode::Shr,
            63 => Opcode::MakeClass,
            64 => Opcode::AddMethod,
            65 => Opcode::AddField,
            66 => Opcode::GetProperty,
            67 => Opcode::SetProperty,
            68 => Opcode::GetGlobal,
            69 => Opcode::SetGlobal,
            70 => Opcode::Import,
            71 => Opcode::ErrorKind,
            72 => Opcode::Rethrow,
            73 => Opcode::UnpackCheck,
            74 => Opcode::IterIndex,
            75 => Opcode::ListAppend,
            76 => Opcode::MakeRange,
            77 => Opcode::IterLen,
            78 => Opcode::Slice,
            79 => Opcode::CallNativeW,
            80 => Opcode::MakeClosureRO,
            81 => Opcode::PyImport,
            _ => return None,
        })
    }

    /// Every opcode, in discriminant order.
    ///
    /// The enum numbers its variants implicitly while `from_u8` above spells
    /// the same mapping out by hand, so appending a variant without adding its
    /// arm compiles cleanly and only fails later — `Instruction::op` panics
    /// with "invalid opcode byte" the first time the dispatch loop reaches it.
    /// `opcode_table_is_dense` below turns that into a test failure, and
    /// `debug_assert`s in `Instruction::new`/`op` catch it even earlier.
    pub const ALL: [Opcode; 82] = [
        Opcode::LoadNil,
        Opcode::LoadTrue,
        Opcode::LoadFalse,
        Opcode::LoadConst,
        Opcode::LoadConstWide,
        Opcode::Move,
        Opcode::Add,
        Opcode::Sub,
        Opcode::Mul,
        Opcode::Div,
        Opcode::Mod,
        Opcode::Pow,
        Opcode::Neg,
        Opcode::Eq,
        Opcode::Ne,
        Opcode::Lt,
        Opcode::Le,
        Opcode::Gt,
        Opcode::Ge,
        Opcode::And,
        Opcode::Or,
        Opcode::Not,
        Opcode::Jump,
        Opcode::JumpIfFalse,
        Opcode::JumpIfTrue,
        Opcode::Call,
        Opcode::CallNative,
        Opcode::CallMethod,
        Opcode::TailCall,
        Opcode::Return,
        Opcode::MakeList,
        Opcode::MakeDict,
        Opcode::MakeSet,
        Opcode::MakeTuple,
        Opcode::GetIndex,
        Opcode::SetIndex,
        Opcode::GetField,
        Opcode::SetField,
        Opcode::IterInit,
        Opcode::IterNext,
        Opcode::IterDone,
        Opcode::MakeClosure,
        Opcode::GetUpval,
        Opcode::SetUpval,
        Opcode::CloseUpval,
        Opcode::TryEnter,
        Opcode::TryExit,
        Opcode::Throw,
        Opcode::ThrowVal,
        Opcode::Spawn,
        Opcode::ChanMake,
        Opcode::ChanSend,
        Opcode::ChanRecv,
        Opcode::TypeGuard,
        Opcode::Deopt,
        Opcode::Safepoint,
        Opcode::Nop,
        Opcode::BitAnd,
        Opcode::BitOr,
        Opcode::BitXor,
        Opcode::BitNot,
        Opcode::Shl,
        Opcode::Shr,
        Opcode::MakeClass,
        Opcode::AddMethod,
        Opcode::AddField,
        Opcode::GetProperty,
        Opcode::SetProperty,
        Opcode::GetGlobal,
        Opcode::SetGlobal,
        Opcode::Import,
        Opcode::ErrorKind,
        Opcode::Rethrow,
        Opcode::UnpackCheck,
        Opcode::IterIndex,
        Opcode::ListAppend,
        Opcode::MakeRange,
        Opcode::IterLen,
        Opcode::Slice,
        Opcode::CallNativeW,
        Opcode::MakeClosureRO,
        Opcode::PyImport,
    ];
}

#[cfg(test)]
mod opcode_table_tests {
    use super::Opcode;

    /// `from_u8` is hand-written and the enum numbers itself implicitly, so a
    /// variant appended without a matching arm would otherwise only be caught
    /// by the dispatch loop's `expect("invalid opcode byte")`.
    #[test]
    fn opcode_table_is_dense() {
        for (i, op) in Opcode::ALL.iter().enumerate() {
            assert_eq!(
                *op as usize, i,
                "{op:?} sits at discriminant {i}, not its declaration position"
            );
            assert_eq!(
                Opcode::from_u8(i as u8),
                Some(*op),
                "from_u8({i}) does not decode {op:?}"
            );
        }
        assert_eq!(Opcode::from_u8(Opcode::ALL.len() as u8), None);
    }

    /// Encoding and decoding must be inverses, including operands — a jump
    /// whose offset bled into the opcode byte would decode to garbage.
    #[test]
    fn instruction_round_trips() {
        for op in Opcode::ALL {
            for (a, b, c) in [(0u8, 0u8, 0u8), (255, 255, 255), (7, 0x80, 0xff)] {
                let inst = super::Instruction::new(op, a, b, c);
                assert_eq!(inst.op(), op);
                assert_eq!((inst.a(), inst.b(), inst.c()), (a, b, c));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Instruction(pub u32);

impl Instruction {
    pub const fn new(op: Opcode, a: u8, b: u8, c: u8) -> Self {
        Self(((op as u32) << 24) | ((a as u32) << 16) | ((b as u32) << 8) | (c as u32))
    }
    #[inline]
    pub fn op(self) -> Opcode {
        Opcode::from_u8((self.0 >> 24) as u8).expect("invalid opcode byte")
    }
    #[inline]
    pub fn a(self) -> u8 {
        (self.0 >> 16) as u8
    }
    #[inline]
    pub fn b(self) -> u8 {
        (self.0 >> 8) as u8
    }
    #[inline]
    pub fn c(self) -> u8 {
        self.0 as u8
    }
    /// Payload word carrying a `u16` native id for [`Opcode::CallNativeW`]. It
    /// decodes as a `Nop` so linear scanners can walk past it safely.
    pub const fn native_wide_payload(id: u16) -> Self {
        Self::new(Opcode::Nop, (id >> 8) as u8, (id & 0xff) as u8, 0)
    }
    /// The native id stored by [`Instruction::native_wide_payload`].
    #[inline]
    pub fn native_wide_id(self) -> u16 {
        ((self.a() as u16) << 8) | self.b() as u16
    }
}

#[derive(Debug, Clone)]
pub struct FunctionProto {
    pub name: String,
    pub params: Vec<String>,
    /// Runtime module identity, assigned by the VM when a module is loaded so
    /// that a call resolves the callee's globals and prototypes against its own
    /// module (the compiler emits `0`).
    pub module_id: u32,
    /// Module-scope global slots owned by this prototype's module. Only a
    /// module's top-level `<module>` proto is non-zero; nested functions read
    /// and write those slots through `GetGlobal`/`SetGlobal`.
    pub num_globals: u16,
    pub num_locals: u16,
    /// High-water mark of `num_locals + temporaries` over the whole body. The
    /// re-entrant callback trampoline parks a callback frame's registers above
    /// this, so the frame it interrupts keeps its temporaries intact while the
    /// callback runs.
    pub max_reg: u16,
    /// Parameters that must be supplied: the declared parameters before the
    /// first one with a default value. `argc < num_required` is an arity error.
    pub num_required: u16,
    pub num_upvalues: u16,
    pub code: Vec<Instruction>,
    pub constants: Vec<crate::value::Value>,
    pub line_table: Vec<(u32, u32)>, // (pc, line)
    pub upvalue_descriptors: Vec<UpvalueDesc>,
    /// True for class methods: an implicit receiver occupies register 0, so
    /// `num_required` counts declared parameters only and the frame (and the
    /// JIT entry) supply one additional leading register.
    pub is_method: bool,
    /// Inline cache slots for property access (GetProperty/SetProperty).
    /// Indexed by the IC slot ID encoded in the instruction.
    pub ic_slots: RefCell<Vec<crate::ic::InlineCache>>,
    /// JIT tier-up state (hotness, execution state, native entry point).
    /// Lazily initialized on the first call once the JIT is enabled; ignored
    /// (and never touched) in plain interpreter builds.
    pub jit: std::sync::OnceLock<std::sync::Arc<crate::jit::JitSlot>>,
}

#[derive(Debug, Clone, Copy)]
pub struct UpvalueDesc {
    pub is_local: bool,
    pub index: u16,
}

/// A linked module ready to be registered with a [`crate::Vm`]: its top-level
/// prototype, its flat child-prototype table, and the manifest of exported
/// `(name, global_slot)` bindings used to build the import dictionary.
#[derive(Debug, Clone)]
pub struct ModuleUnit {
    pub main: FunctionProto,
    pub children: Vec<FunctionProto>,
    pub exports: Vec<(String, u16)>,
    /// Global slots that hold constant values rather than running code.
    /// Pre-filled when the module is imported so a module with hundreds of
    /// constants never needs one register per export.
    pub const_globals: Vec<(u16, crate::value::Value)>,
}
