//! Zen Cranelift JIT (optional, behind `jit` feature).
//! See `docs/10-jit.md`.
//!
//! Stage 1 exposes the tier-up infrastructure that lives in `zen-vm`
//! (hotness counters, per-function atomic execution state, and the background
//! worker pool). The Cranelift lowerer lands here in Stage 2.

pub use zen_vm::jit;

#[cfg(feature = "jit")]
pub use zen_vm::jit::{JitHandle, StubCompiler, ZenJitCompiler};

#[cfg(feature = "jit")]
mod lower;

#[cfg(feature = "jit")]
pub use lower::CraneliftCompiler;