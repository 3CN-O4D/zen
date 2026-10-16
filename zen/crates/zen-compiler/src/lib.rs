//! Zen compiler: lowers the `zen-syntax` AST into register-based bytecode for
//! `zen-vm`.
//!
//! See `docs/07-bytecode.md` for the instruction encoding and
//! [`compiler::CompiledModule`] for the shape of a compiled unit.

pub mod capture;
pub mod chunk;
pub mod compiler;
pub mod error;
pub mod resolver;

pub use compiler::{compile_module, compile_source, CompiledModule};
pub use error::{CompileError, CompileErrorCollector, CompileResult};
pub use resolver::{BindKind, Binding, Resolver, MAX_REGISTERS};
