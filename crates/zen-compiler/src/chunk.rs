//! Bytecode chunk: instruction buffer, constant pool, and line table.
//!
//! The runtime types live in [`zen_vm::bytecode`]; this is the compiler-side
//! builder. Jump offsets are stored as a signed 16-bit big-endian pair in the
//! instruction's `b` (high) and `c` (low) fields, relative to the *next*
//! instruction (`pc + 1`), matching [`zen_vm::Instruction`]'s layout.

use zen_vm::bytecode::UpvalueDesc;
use zen_vm::{FunctionProto, Instruction, Opcode, Value};

/// Raised when a chunk exceeds a hard encoding limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ChunkError {
    #[error("constant pool overflow (max 65535)")]
    ConstantPoolOverflow,
    #[error("jump offset overflow (more than +/-32767 instructions)")]
    JumpOffsetOverflow,
    #[error("register index out of range (max 250)")]
    TooManyRegisters,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub code: Vec<Instruction>,
    pub constants: Vec<Value>,
    /// `(pc, 1-based source line)` for every emitted instruction.
    pub line_table: Vec<(u32, u32)>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            line_table: Vec::new(),
        }
    }

    /// Append an instruction stamped with `line`.
    pub fn emit(&mut self, op: Opcode, a: u8, b: u8, c: u8, line: u32) -> usize {
        let pc = self.code.len();
        self.code.push(Instruction::new(op, a, b, c));
        self.line_table.push((pc as u32, line));
        pc
    }

    /// The index that the *next* instruction will occupy.
    pub fn here(&self) -> usize {
        self.code.len()
    }

    /// Append a pre-built instruction (a wide-call payload word) with its line.
    pub fn emit_raw(&mut self, ins: Instruction, line: u32) {
        let pc = self.code.len();
        self.code.push(ins);
        self.line_table.push((pc as u32, line));
    }

    /// Add a constant, deduplicating immutable scalars by value.
    pub fn add_constant(&mut self, value: Value) -> Result<u8, ChunkError> {
        if let Some(idx) = self.find_constant(&value) {
            return Ok(idx);
        }
        if self.constants.len() > u8::MAX as usize {
            return Err(ChunkError::ConstantPoolOverflow);
        }
        let idx = self.constants.len() as u8;
        self.constants.push(value);
        Ok(idx)
    }

    /// Register a constant that must never be deduplicated (e.g. a child
    /// prototype placeholder); always appends.
    pub fn push_constant(&mut self, value: Value) -> Result<u8, ChunkError> {
        if self.constants.len() > u8::MAX as usize {
            return Err(ChunkError::ConstantPoolOverflow);
        }
        let idx = self.constants.len() as u8;
        self.constants.push(value);
        Ok(idx)
    }

    fn find_constant(&self, value: &Value) -> Option<u8> {
        self.constants
            .iter()
            .position(|v| values_equal(v, value))
            .map(|i| i as u8)
    }

    /// Emit a jump with a placeholder target; returns its `pc` for patching.
    pub fn emit_jump(&mut self, op: Opcode, a: u8, line: u32) -> usize {
        self.emit(op, a, 0, 0, line)
    }

    /// Rewrite operand `a` of the instruction at `pc`, leaving the other
    /// operands (such as a jump offset in `(b, c)`) untouched.
    pub fn set_operand_a(&mut self, pc: usize, a: u8) -> Result<(), ChunkError> {
        let instr = self
            .code
            .get_mut(pc)
            .ok_or(ChunkError::JumpOffsetOverflow)?;
        instr.0 = (instr.0 & 0xFF00_FFFF) | (u32::from(a) << 16);
        Ok(())
    }

    /// Point the jump at `pc` to `target` (an instruction index).
    pub fn patch_jump(&mut self, pc: usize, target: usize) -> Result<(), ChunkError> {
        let off = target as i64 - (pc as i64 + 1);
        let off = i16::try_from(off).map_err(|_| ChunkError::JumpOffsetOverflow)?;
        let bits = off as u16;
        let instr = self
            .code
            .get_mut(pc)
            .ok_or(ChunkError::JumpOffsetOverflow)?;
        // Clear b and c (bits 0..16), keep op and a (bits 16..32). The signed
        // offset is the big-endian pair (b, c).
        instr.0 = (instr.0 & 0xFFFF_0000) | u32::from(bits);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn into_proto(
        self,
        name: String,
        params: Vec<String>,
        num_locals: u16,
        max_reg: u16,
        num_required: u16,
        num_upvalues: u16,
        upvalue_descriptors: Vec<UpvalueDesc>,
        is_method: bool,
    ) -> FunctionProto {
        FunctionProto {
            name,
            params,
            module_id: 0,
            num_globals: 0,
            num_locals,
            max_reg,
            num_required,
            num_upvalues,
            code: self.code,
            constants: self.constants,
            line_table: self.line_table,
            upvalue_descriptors,
            ic_slots: std::cell::RefCell::new(Vec::new()),
            jit: std::sync::OnceLock::new(),
            is_method,
        }
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

/// Structural value equality for constant deduplication. Heap containers are
/// never deduplicated (their address would be meaningless as a key).
fn values_equal(a: &Value, b: &Value) -> bool {
    use zen_vm::Tag;
    match a.tag() {
        Tag::Nil => b.is_nil(),
        Tag::False | Tag::True => a.as_bool() == b.as_bool(),
        Tag::Int => a.as_int() == b.as_int(),
        Tag::Float => a.to_bits() == b.to_bits(),
        Tag::String => a.as_str() == b.as_str(),
        Tag::Bytes => a.as_bytes() == b.as_bytes(),
        Tag::Heap => false,
    }
}
