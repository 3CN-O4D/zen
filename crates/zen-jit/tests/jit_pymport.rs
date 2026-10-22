//! `pymport` never compiles: foreign imports are a runtime bridge boundary,
//! so any function containing one stays interpreted (slow path first, by
//! design — mirroring how capturing `MakeClosure` stays interpreted).
#![cfg(feature = "jit")]

use zen_compiler::compile_source;
use zen_jit::{CraneliftCompiler, ZenJitCompiler};
use zen_vm::jit::CompileSnapshot;

#[test]
fn pyimport_stays_interpreted() {
    let module = compile_source("pymport json\nreturn 1\n").expect("compile");
    let snap = CompileSnapshot::from_proto(&module.main);
    let err = CraneliftCompiler::new()
        .compile(&snap)
        .expect_err("PyImport must be ineligible");
    assert!(
        err.contains("not lowered"),
        "unexpected rejection: {err}"
    );
}
