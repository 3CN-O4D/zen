//! Stub socket module for Windows.
//!
//! The full `socket` module exposes BSD socket operations that are not
//! available on Windows in the same form. This stub provides the same
//! API surface but returns "not supported" for every call.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::value::Value;

pub fn dispatch(_id: u16, _args: &[Value], _output: &mut String) -> VmResult<Option<Value>> {
    Err(VmError::NativeError("socket module is not supported on Windows".into()))
}

pub fn module() -> ModuleUnit {
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
