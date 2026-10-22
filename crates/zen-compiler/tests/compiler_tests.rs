//! Structural tests for the M4 compiler.
//!
//! The M5 interpreter does not exist yet, so these tests assert the *shape* of
//! the emitted bytecode: which opcodes appear, what the constant pool holds, and
//! where forward/backward jumps land. They deliberately avoid asserting exact
//! register numbers except where the ABI is pinned (locals start at 0).

use zen_compiler::{compile_source, CompileError};
use zen_vm::{FunctionProto, Instruction, Opcode};

fn compile(src: &str) -> FunctionProto {
    compile_source(src).expect("source should compile").main
}

fn count(proto: &FunctionProto, op: Opcode) -> usize {
    proto.code.iter().filter(|i| i.op() == op).count()
}

fn has(proto: &FunctionProto, op: Opcode) -> bool {
    count(proto, op) > 0
}

fn jump_offset(instr: Instruction) -> i16 {
    i16::from_be_bytes([instr.b(), instr.c()])
}

// ── literals and the constant pool ───────────────────────────────────────

#[test]
fn integer_literal_lands_in_constant_pool() {
    let p = compile("42");
    assert_eq!(count(&p, Opcode::LoadConst), 1);
    assert_eq!(p.constants.len(), 1);
    assert_eq!(p.constants[0].as_int(), Some(42));
}

#[test]
fn nil_true_false_use_dedicated_opcodes() {
    let p = compile("nil\ntrue\nfalse");
    assert_eq!(count(&p, Opcode::LoadNil), 1);
    assert_eq!(count(&p, Opcode::LoadTrue), 1);
    assert_eq!(count(&p, Opcode::LoadFalse), 1);
    assert!(p.constants.is_empty(), "scalars need no pool entries");
}

#[test]
fn string_constants_are_deduplicated() {
    let p = compile("\"a\"\n\"a\"\n\"b\"");
    assert_eq!(count(&p, Opcode::LoadConst), 3);
    assert_eq!(
        p.constants.len(),
        2,
        "the two `\"a\"` literals share one slot"
    );
}

// ── bindings and assignment ──────────────────────────────────────────────

#[test]
fn module_binding_allocates_global_slot_zero() {
    let p = compile("let a = 5");
    assert_eq!(p.num_globals, 1, "top-level bindings are module globals");
    assert_eq!(p.num_locals, 0, "module globals do not consume registers");
    assert!(has(&p, Opcode::SetGlobal));
    assert!(has(&p, Opcode::LoadConst));
}

#[test]
fn module_globals_are_distinct_slots() {
    let p = compile("let a = 1\nlet b = 2");
    assert_eq!(p.num_globals, 2);
}

#[test]
fn function_local_allocates_register_not_global() {
    let m = compile_source("fn f() { let a = 5 return a }").expect("compiles");
    let f = &m.children[0];
    assert_eq!(f.num_globals, 0);
    assert_eq!(f.num_locals, 1);
    assert!(f.code.iter().all(|i| i.op() != Opcode::SetGlobal));
}

#[test]
fn compound_assignment_loads_operates_and_stores() {
    let p = compile("let a = 1\na += 2");
    assert!(has(&p, Opcode::Add));
}

// ── arithmetic, comparison and bitwise operators ─────────────────────────

#[test]
fn arithmetic_emits_expected_opcodes() {
    let p = compile("let a = 6 - 2\nlet b = 3 * 4\nlet c = 8 / 2\nlet d = 7 % 2");
    assert!(has(&p, Opcode::Sub));
    assert!(has(&p, Opcode::Mul));
    assert!(has(&p, Opcode::Div));
    assert!(has(&p, Opcode::Mod));
}

#[test]
fn comparison_emits_expected_opcodes() {
    let p = compile("let a = 1 < 2\nlet b = 1 <= 2\nlet c = 1 > 2\nlet d = 1 >= 2\nlet e = 1 == 2\nlet f = 1 != 2");
    for op in [
        Opcode::Lt,
        Opcode::Le,
        Opcode::Gt,
        Opcode::Ge,
        Opcode::Eq,
        Opcode::Ne,
    ] {
        assert!(has(&p, op), "missing {op:?}");
    }
}

#[test]
fn bitwise_operators_use_m4_opcodes() {
    let p = compile(
        "let a = 5 & 3\nlet b = 5 | 3\nlet c = 5 ^ 3\nlet d = 5 << 1\nlet e = 5 >> 1\nlet f = ~5",
    );
    for op in [
        Opcode::BitAnd,
        Opcode::BitOr,
        Opcode::BitXor,
        Opcode::Shl,
        Opcode::Shr,
        Opcode::BitNot,
    ] {
        assert!(has(&p, op), "missing {op:?}");
    }
}

// ── control flow: patched jumps ──────────────────────────────────────────

#[test]
fn if_else_patches_a_forward_jump() {
    let p = compile("if true { 1 } else { 2 }");
    assert_eq!(count(&p, Opcode::JumpIfFalse), 1);
    assert_eq!(count(&p, Opcode::Jump), 1);

    let jf = p
        .code
        .iter()
        .find(|i| i.op() == Opcode::JumpIfFalse)
        .unwrap();
    assert!(
        jump_offset(*jf) > 0,
        "the else branch begins after the jump"
    );

    let over = p.code.iter().find(|i| i.op() == Opcode::Jump).unwrap();
    assert!(
        jump_offset(*over) > 0,
        "the then branch jumps over the else"
    );
}

#[test]
fn if_without_else_only_jumps_when_false() {
    let p = compile("if true { 1 }");
    assert_eq!(count(&p, Opcode::JumpIfFalse), 1);
    assert_eq!(count(&p, Opcode::Jump), 0);
}

#[test]
fn while_emits_a_backward_jump() {
    let p = compile("let i = 0\nwhile i < 3 { i += 1 }");
    assert!(has(&p, Opcode::JumpIfFalse));
    let back = p
        .code
        .iter()
        .filter(|i| i.op() == Opcode::Jump)
        .any(|i| jump_offset(*i) < 0);
    assert!(back, "the loop must jump backwards to its head");
}

#[test]
fn break_patches_to_loop_exit() {
    let p = compile("while true { break }");
    let jumps = p.code.iter().filter(|i| i.op() == Opcode::Jump).count();
    assert!(jumps >= 2, "one back-edge and one patched break");
    // Every jump target must be in range (patch_jump would have errored).
    for (pc, instr) in p.code.iter().enumerate() {
        if instr.op() == Opcode::Jump {
            let target = pc as i64 + 1 + i64::from(jump_offset(*instr));
            assert!(
                target >= 0 && target <= p.code.len() as i64,
                "pc={pc} target={target} len={}",
                p.code.len()
            );
        }
    }
}

#[test]
fn continue_targets_the_loop_head() {
    let p = compile("let i = 0\nwhile i < 3 { i += 1\ncontinue }");
    assert!(has(&p, Opcode::Jump), "back-edge plus continue");
}

#[test]
fn for_loop_uses_the_iteration_protocol() {
    let p = compile("for x in [1, 2, 3] { print(x) }");
    assert_eq!(count(&p, Opcode::IterInit), 1);
    assert_eq!(count(&p, Opcode::IterDone), 1);
    assert_eq!(count(&p, Opcode::IterNext), 1);
}

// ── short-circuiting ─────────────────────────────────────────────────────

#[test]
fn logical_and_short_circuits_on_false() {
    let p = compile("let a = true and false");
    assert!(has(&p, Opcode::JumpIfFalse));
}

#[test]
fn logical_or_short_circuits_on_true() {
    let p = compile("let a = true or false");
    assert!(has(&p, Opcode::JumpIfTrue));
}

// ── suspended values: ?. and ?? ──────────────────────────────────────────

#[test]
fn optional_field_guards_with_a_nil_check_and_jump() {
    let p = compile("let a = nil\nlet b = a?.x");
    assert!(has(&p, Opcode::GetField));
    assert!(has(&p, Opcode::JumpIfFalse));
}

#[test]
fn null_coalesce_tests_for_nil() {
    let p = compile("let a = nil\nlet b = a ?? 1");
    assert!(has(&p, Opcode::Eq));
    assert!(has(&p, Opcode::JumpIfFalse));
}

// ── containers ───────────────────────────────────────────────────────────

#[test]
fn container_literals_emit_builders() {
    let p = compile("let l = [1, 2]\nlet t = #(1, 2)\nlet s = %{1, 2}\nlet d = {a: 1}");
    assert!(has(&p, Opcode::MakeList));
    assert!(has(&p, Opcode::MakeTuple));
    assert!(has(&p, Opcode::MakeSet));
    assert!(has(&p, Opcode::MakeDict));
}

// ── functions and upvalues ───────────────────────────────────────────────

#[test]
fn function_definition_emits_a_child_prototype() {
    let module = compile_source("func f() { return 1 }").unwrap();
    assert_eq!(module.children.len(), 1);
    assert_eq!(module.children[0].name, "f");
    assert_eq!(count(&module.main, Opcode::MakeClosure), 1);
}

#[test]
fn nested_function_captures_an_upvalue() {
    let module = compile_source(
        "func outer() {\n    let x = 1\n    func inner() { return x }\n    return inner\n}",
    )
    .unwrap();
    assert_eq!(module.children.len(), 2);
    let inner = module
        .children
        .iter()
        .find(|p| p.name == "inner")
        .expect("inner prototype");
    assert_eq!(inner.num_upvalues, 1);
    assert_eq!(inner.upvalue_descriptors.len(), 1);
    assert!(has(inner, Opcode::GetUpval));
}

#[test]
fn lambda_expression_compiles_to_a_closure() {
    let p = compile("let f = func() { return 1 }");
    assert!(has(&p, Opcode::MakeClosure));
}

// ── diagnostics ──────────────────────────────────────────────────────────

#[test]
fn undefined_variable_is_rejected() {
    let err = compile_source("missing").unwrap_err();
    match err {
        CompileError::UndefinedVariable { name, .. } => assert_eq!(name, "missing"),
        other => panic!("expected UndefinedVariable, got {other:?}"),
    }
}

#[test]
fn assigning_to_a_constant_is_rejected() {
    let err = compile_source("const a = 1\na = 2").unwrap_err();
    assert!(
        matches!(err, CompileError::AssignToConst { .. }),
        "got {err:?}"
    );
}

#[test]
fn redeclaring_a_binding_shadows_the_previous_one() {
    // The oracle rebinds instead of failing, so the compiler must accept it.
    compile_source("let a = 1\nlet a = 2").expect("redeclares");
    compile_source("var a = 1\nvar a = 2").expect("redeclares");
    compile_source("func f() {\n  let a = 1\n  let a = 2\n}").expect("redeclares");
}

#[test]
fn break_outside_a_loop_is_rejected() {
    let err = compile_source("break").unwrap_err();
    assert!(
        matches!(err, CompileError::BreakOutsideLoop { .. }),
        "got {err:?}"
    );
}

#[test]
fn continue_outside_a_loop_is_rejected() {
    let err = compile_source("continue").unwrap_err();
    assert!(
        matches!(err, CompileError::ContinueOutsideLoop { .. }),
        "got {err:?}"
    );
}

#[test]
fn unsupported_constructs_report_a_compile_error() {
    for (src, what) in [
        ("include \"other.z\"", "include"),
        ("let a = 1\nlet b = 2\na is b", "is"),
    ] {
        let err = compile_source(src).unwrap_err();
        match err {
            CompileError::Unsupported { what: w, .. } => assert_eq!(w, what),
            other => panic!("expected Unsupported({what}), got {other:?}"),
        }
    }
}

/// `typeof` is a prefix operator, so both spellings compile to the same
/// native-17 call: the parenthesis belongs to the operand, not a call.
#[test]
fn typeof_compiles_in_both_spellings() {
    for src in [
        "let y = 1\nlet t = typeof y",
        "let y = 1\nlet t = typeof(y)",
    ] {
        let module = compile_source(src).unwrap_or_else(|e| panic!("{src:?} rejected: {e:?}"));
        let calls = module
            .main
            .code
            .iter()
            .filter(|i| i.op() == zen_vm::Opcode::CallNative && i.b() == 17)
            .count();
        assert_eq!(calls, 1, "{src:?} did not emit one typeof call");
    }
}

// ── read-only captures (`MakeClosureRO`) ─────────────────────────────────

fn count_all(module: &zen_compiler::CompiledModule, op: Opcode) -> usize {
    module
        .children
        .iter()
        .map(|p| count(p, op))
        .sum::<usize>()
        + count(&module.main, op)
}

#[test]
fn readonly_param_capture_flips_to_ro() {
    // The bench shape: a parameter captured once, never written.
    let module =
        compile_source("func make(x) {\n    return func() { return x }\n}").unwrap();
    assert_eq!(count_all(&module, Opcode::MakeClosureRO), 1);
    // The module-level `func make` itself takes no captures and keeps the
    // plain form.
    assert_eq!(count(&module.main, Opcode::MakeClosure), 1);
}

#[test]
fn mutated_capture_stays_cell() {
    let module = compile_source(
        "func make_counter() {\n    var n = 0\n    func bump() {\n        n = n + 1\n        return n\n    }\n    return bump\n}",
    )
    .unwrap();
    assert_eq!(count_all(&module, Opcode::MakeClosureRO), 0);
    assert!(count_all(&module, Opcode::MakeClosure) >= 1);
}

#[test]
fn reassigned_param_stays_cell() {
    let module = compile_source(
        "func make(x) {\n    x = x + 1\n    return func() { return x }\n}",
    )
    .unwrap();
    assert_eq!(count_all(&module, Opcode::MakeClosureRO), 0);
}

#[test]
fn sibling_writer_blocks_ro() {
    // `reader` is created before `writer`, but the post-pass still sees the
    // later writer: `x` must stay shared.
    let module = compile_source(
        "func outer() {\n    var x = 1\n    func reader() { return x }\n    func writer() { x = 2 }\n    writer()\n    return reader\n}",
    )
    .unwrap();
    assert_eq!(count_all(&module, Opcode::MakeClosureRO), 0);
}

#[test]
fn self_capture_for_recursion_stays_cell() {
    let module = compile_source(
        "func outer() {\n    func f(n) {\n        if n < 1 { return n }\n        return f(n - 1)\n    }\n    return f\n}",
    )
    .unwrap();
    assert_eq!(count_all(&module, Opcode::MakeClosureRO), 0);
}

#[test]
fn non_param_capture_stays_cell() {
    // `y` is a nested local, not a parameter: never trusted.
    let module = compile_source(
        "func outer() {\n    var y = 5\n    func inner() { return y }\n    return inner\n}",
    )
    .unwrap();
    assert_eq!(count_all(&module, Opcode::MakeClosureRO), 0);
}

// ── foreign imports (`pymport`) ───────────────────────────────────────────

#[test]
fn pymport_emits_pyimport_not_import() {
    let p = compile("pymport whatsapp");
    assert_eq!(count(&p, Opcode::PyImport), 1);
    assert_eq!(count(&p, Opcode::Import), 0);
    // The dotted path rides in the constant pool like `Import`.
    assert!(p.constants.iter().any(|c| c.as_str() == Some("whatsapp")));
}

#[test]
fn pymport_dotted_alias_emits_single_pyimport() {
    let p = compile("pymport a.b as c");
    assert_eq!(count(&p, Opcode::PyImport), 1);
    assert!(p.constants.iter().any(|c| c.as_str() == Some("a.b")));
}

#[test]
fn native_import_never_emits_pyimport() {
    let p = compile("import a.b\nimport c as d");
    assert_eq!(count(&p, Opcode::PyImport), 0);
    assert!(count(&p, Opcode::Import) >= 2);
}
