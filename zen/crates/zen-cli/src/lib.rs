//! Zen CLI library surface.
//!
//! The binaries are thin argument parsers; everything reusable — currently the
//! module linker — lives here so tests and tooling can link programs the same
//! way `zen run` does.

pub mod explain;
pub mod help;
pub mod linker;
pub mod pydeps;
pub mod zendiag;

pub use linker::{compile_source, link, module_deps, LinkedProgram};
