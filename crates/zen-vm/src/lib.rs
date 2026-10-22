// The VM is single-threaded by design. `Arc` is used for cheap clone-on-share
// of protos and values, never for cross-thread sharing, so the non-`Send`/
// non-`Sync` payloads are intentional.
#![allow(clippy::arc_with_non_send_sync)]

pub mod ic;
pub mod shape;
// Zen virtual machine: value representation, bytecode, interpreter, modules.
// See `docs/04-value-and-types.md`, `docs/07-bytecode.md`, `docs/08-vm.md`.

// `unsafe` is denied crate-wide and explicitly relaxed in the three modules
// that implement the NaN-boxed representation and the refcount allocator:
// [`value`], [`object`], and [`gc`]. Every unsafe block carries a safety
// comment; nothing else in the crate may use raw pointers.

#[deny(unsafe_code)]
#[forbid(unsafe_op_in_unsafe_fn)]
#[allow(unsafe_code)]
pub mod gc;
#[allow(unsafe_code)]
pub mod object;
#[allow(unsafe_code)]
pub mod value;

pub mod builtins;
pub mod bytecode;
pub mod errors;
pub mod ffi;
pub mod jit;
#[cfg(feature = "jit")]
pub mod jit_rt;
pub mod native;
pub mod ops;
pub mod pybridge;
#[cfg(feature = "python")]
pub mod pyembed;
pub mod resource;
pub mod vm;

pub use crate::bytecode::{FunctionProto, Instruction, ModuleUnit, Opcode};
pub use crate::errors::VmError;
pub use crate::gc::{gc, Gc, GcError, GcRef, RefCountGc};
pub use crate::value::{DictKey, Tag, Value};
pub use crate::vm::Vm;
pub use crate::vm::ErrorLocation;
