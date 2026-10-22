//! Stub sys module for non-Linux platforms.
//!
//! The full `sys` module exposes raw POSIX/Linux syscalls that are not
//! available on all platforms. This stub provides the same API surface
//! (dispatch, module) but returns "not supported" for every call.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::value::Value;

pub const ID_SCAL: u16 = 400;

pub fn dispatch(_id: u16, _args: &[Value], _output: &mut String) -> VmResult<Option<Value>> {
    Err(VmError::NativeError(
        "sys module is not supported on this platform".into(),
    ))
}

pub fn module() -> ModuleUnit {
    // Build a minimal empty module unit
    use crate::bytecode::{FunctionProto, Instruction, ModuleUnit, Opcode};
    use std::cell::RefCell;
    use std::sync::OnceLock;
    let main = FunctionProto {
        name: "<module>".to_string(),
        params: Vec::new(),
        module_id: 0,
        num_globals: 0,
        num_locals: 1,
        max_reg: 1,
        num_required: 0,
        num_upvalues: 0,
        code: vec![
            Instruction::new(Opcode::LoadNil, 0, 0, 0),
            Instruction::new(Opcode::Return, 0, 0, 0),
        ],
        constants: Vec::new(),
        line_table: Vec::new(),
        upvalue_descriptors: Vec::new(),
        ic_slots: RefCell::new(Vec::new()),
        jit: OnceLock::new(),
        is_method: false,
    };
    ModuleUnit {
        main,
        children: Vec::new(),
        exports: Vec::new(),
        const_globals: Vec::new(),
    }
}
