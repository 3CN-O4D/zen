//! Parser unit tests against `docs/06-parser.md`.

use zen_syntax::lex::lex;
use zen_syntax::parser::{ParseError, Parser};
use zen_syntax::span::FileId;

fn parse(src: &str) -> Result<zen_syntax::ast::Module, ParseError> {
    let lx = lex(src, FileId(0));
    Parser::new(lx).parse_module()
}

fn parse_expr(src: &str) -> zen_syntax::ast::Expr {
    let lx = lex(src, FileId(0));
    Parser::new(lx).parse_expr().unwrap()
}

fn first_stmt(src: &str) -> zen_syntax::ast::Stmt {
    let mut m = parse(src).unwrap();
    m.items.remove(0)
}

// ── Expression tests ─────────────────────────────────────────────────────

#[test]
fn p_e1_literal_int() {
    let e = parse_expr("42");
    assert!(matches!(
        e,
        zen_syntax::ast::Expr::Literal(zen_syntax::ast::Literal::Int(42))
    ));
}

#[test]
fn p_e2_literal_float() {
    let e = parse_expr("1.5");
    assert!(
        matches!(e, zen_syntax::ast::Expr::Literal(zen_syntax::ast::Literal::Float(f)) if f == 1.5)
    );
}

#[test]
fn p_e3_literal_string() {
    let e = parse_expr("\"hello\"");
    assert!(
        matches!(e, zen_syntax::ast::Expr::Literal(zen_syntax::ast::Literal::String(s)) if s == "hello")
    );
}

#[test]
fn p_e4_identifier() {
    let e = parse_expr("x");
    assert!(matches!(e, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "x"));
}

#[test]
fn p_e5_binary_add() {
    let e = parse_expr("a + b");
    assert!(matches!(
        e,
        zen_syntax::ast::Expr::Binary {
            op: zen_syntax::ast::BinaryOp::Add,
            ..
        }
    ));
}

#[test]
fn p_e6_binary_precedence() {
    let e = parse_expr("a + b * c");
    match e {
        zen_syntax::ast::Expr::Binary {
            op: zen_syntax::ast::BinaryOp::Add,
            right,
            ..
        } => {
            assert!(matches!(
                *right,
                zen_syntax::ast::Expr::Binary {
                    op: zen_syntax::ast::BinaryOp::Mul,
                    ..
                }
            ));
        }
        _ => panic!("expected Add with Mul right"),
    }
}

#[test]
fn p_e7_unary_neg() {
    let e = parse_expr("-x");
    assert!(matches!(
        e,
        zen_syntax::ast::Expr::Unary {
            op: zen_syntax::ast::UnaryOp::Neg,
            ..
        }
    ));
}

#[test]
fn p_e8_call() {
    let e = parse_expr("f(1, 2)");
    assert!(matches!(e, zen_syntax::ast::Expr::Call { args, .. } if args.len() == 2));
}

#[test]
fn p_e9_index() {
    let e = parse_expr("a[0]");
    assert!(matches!(e, zen_syntax::ast::Expr::Index { .. }));
}

#[test]
fn p_e10_field() {
    let e = parse_expr("x.y");
    assert!(matches!(e, zen_syntax::ast::Expr::Field { field, .. } if field == "y"));
}

#[test]
fn p_e11_optional_field() {
    let e = parse_expr("x?.y");
    assert!(matches!(e, zen_syntax::ast::Expr::OptionalField { field, .. } if field == "y"));
}

#[test]
fn p_e12_method_call() {
    let e = parse_expr("x.m(1)");
    assert!(
        matches!(e, zen_syntax::ast::Expr::MethodCall { method, args, .. } if method == "m" && args.len() == 1)
    );
}

#[test]
fn p_e13_ternary() {
    let e = parse_expr("a ? b : c");
    match e {
        zen_syntax::ast::Expr::Ternary {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            assert!(matches!(*cond, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "a"));
            assert!(matches!(*then_branch, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "b"));
            assert!(matches!(*else_branch, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "c"));
        }
        _ => panic!("expected Ternary"),
    }
}

#[test]
fn p_if5_if_expression_accepts_block_arms() {
    // Lenient extension over §5.4: because the `if` keyword commits the parse,
    // the `{` after the condition can only be a block, so a block-armed `if`
    // used as a value is accepted until M8 adds the `then`/`else` spelling.
    let stmt = first_stmt("x = if c { \"yes\" } else { \"no\" }");
    match stmt {
        zen_syntax::ast::Stmt::Assign { value, .. } => {
            assert!(matches!(
                *value,
                zen_syntax::ast::Expr::IfExpr { ref arms, otherwise: Some(_), .. } if arms.len() == 1
            ));
        }
        _ => panic!("expected an assign of an IfExpr"),
    }
}

#[test]
fn p_if6_bare_if_statement_takes_blocks() {
    // The mirror image: in *statement* position the same `{` is a block, so this
    // is a well-formed `if` statement.
    let stmt = first_stmt("if c { \"yes\" } else { \"no\" }");
    assert!(matches!(stmt, zen_syntax::ast::Stmt::If { .. }));
}

#[test]
fn p_e14_if_expression_elif_chain() {
    // `elif` needs no dedicated AST field: it is just the next arm. The `else`
    // tail is optional, so an if-expression without one is still a value.
    let e = parse_expr("if a { b } elif c { d } else { e }");
    match e {
        zen_syntax::ast::Expr::IfExpr {
            arms, otherwise, ..
        } => {
            assert_eq!(arms.len(), 2);
            assert!(matches!(&arms[1].0, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "c"));
            assert!(otherwise.is_some());
        }
        _ => panic!("expected IfExpr"),
    }
}

#[test]
fn p_e14h_if_expression_without_else() {
    let e = parse_expr("if a { b }");
    assert!(matches!(
        e,
        zen_syntax::ast::Expr::IfExpr { arms, otherwise, .. }
            if arms.len() == 1 && otherwise.is_none()
    ));
}

#[test]
fn p_s4b_if_statement_elif_chain() {
    // The `elif` chain and the optional `else` tail are what `Stmt::If`'s
    // `elifs: Vec<…>` + `else_branch: Option<…>` exist for.
    let stmt = first_stmt("if a { b } elif c { d } else { e }");
    match stmt {
        zen_syntax::ast::Stmt::If {
            cond,
            then_branch: _,
            elifs,
            else_branch,
            ..
        } => {
            assert!(matches!(*cond, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "a"));
            assert_eq!(elifs.len(), 1);
            assert!(matches!(&elifs[0].0, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "c"));
            assert!(else_branch.is_some());
        }
        _ => panic!("expected If"),
    }
}

#[test]
fn p_x1_lbrace_suppresses_only_its_own_newline() {
    // RULING ASI-0 / T-C13: a bare `{` is a *non*-suppressing opener, so the
    // arms of a block split on newlines — but the newline directly after `{` is
    // swallowed by LBrace's A2 bit, so the block does not open with a blank
    // line. (Contrast `(` / `[` / `#{`, which suppress for their whole extent.)
    let lx = lex("if c {\n  a\n}\n", FileId(0));
    let kinds: Vec<_> = lx.toks.iter().map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        vec![
            zen_syntax::lex::TokenKind::KwIf,
            zen_syntax::lex::TokenKind::Ident,
            zen_syntax::lex::TokenKind::LBrace,
            zen_syntax::lex::TokenKind::Ident,
            zen_syntax::lex::TokenKind::Newline, // the arm separator survives
            zen_syntax::lex::TokenKind::RBrace,
            zen_syntax::lex::TokenKind::Newline,
            zen_syntax::lex::TokenKind::Eof,
        ]
    );
}

#[test]
fn p_x2_delims_pair_on_the_same_byte() {
    // The delimiter stack stores one byte for both halves of a pair, so a closer
    // must report its *opener's* byte or it can never match what was pushed —
    // which would leave the opener stuck and suppress every later newline.
    for (open, close) in [
        (
            zen_syntax::lex::TokenKind::LParen,
            zen_syntax::lex::TokenKind::RParen,
        ),
        (
            zen_syntax::lex::TokenKind::LBracket,
            zen_syntax::lex::TokenKind::RBracket,
        ),
        (
            zen_syntax::lex::TokenKind::LBrace,
            zen_syntax::lex::TokenKind::RBrace,
        ),
    ] {
        assert_eq!(open.delim_byte(), close.delim_byte(), "{open:?}/{close:?}");
    }
    assert_eq!(zen_syntax::lex::TokenKind::Ident.delim_byte(), None);
}

#[test]
fn p_s4c_if_statement_without_else() {
    let stmt = first_stmt("if a { b }");
    assert!(matches!(
        stmt,
        zen_syntax::ast::Stmt::If { elifs, else_branch, .. }
            if elifs.is_empty() && else_branch.is_none()
    ));
}

#[test]
fn p_e14b_python_style_ternary() {
    // docs/06-parser.md §4.4: the second spelling of one construct is
    // `a if cond else b`, read *above* the infix loop.
    let e = parse_expr("a if b else c");
    match e {
        zen_syntax::ast::Expr::Ternary {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            assert!(matches!(*cond, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "b"));
            assert!(matches!(*then_branch, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "a"));
            assert!(matches!(*else_branch, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "c"));
        }
        _ => panic!("expected Ternary"),
    }
}

#[test]
fn p_e14c_python_style_ternary_chains_right() {
    // The `else` arm is read at bp 0, so this right-associates rather than
    // stopping after the first `else`.
    let e = parse_expr("a if b else c if d else e");
    match e {
        zen_syntax::ast::Expr::Ternary {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            assert!(matches!(*cond, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "b"));
            assert!(matches!(*then_branch, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "a"));
            assert!(matches!(
                *else_branch,
                zen_syntax::ast::Expr::Ternary { cond, then_branch, .. }
                    if matches!(&*cond, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "d")
                        && matches!(&*then_branch, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "c")
            ));
        }
        _ => panic!("expected Ternary"),
    }
}

#[test]
fn p_e14d_python_style_ternary_binds_looser_than_arithmetic() {
    // `+` is bp 45 and is absorbed into the *condition*, so the ternary is the
    // whole expression: `a if b + 1 else c`.
    let e = parse_expr("a if b + 1 else c");
    match e {
        zen_syntax::ast::Expr::Ternary {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            assert!(matches!(*cond, zen_syntax::ast::Expr::Binary { .. }));
            assert!(matches!(*then_branch, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "a"));
            assert!(matches!(*else_branch, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "c"));
        }
        _ => panic!("expected Ternary"),
    }
}

#[test]
fn p_e14e_python_style_ternary_absorbs_range() {
    // Range is bp 5 == BP_RANGE, so `5 >= min_prec` absorbs it into the
    // then-arm instead of terminating the expression early.
    let e = parse_expr("1 -> 3 if c else 9");
    match e {
        zen_syntax::ast::Expr::Ternary { then_branch, .. } => {
            assert!(matches!(
                *then_branch,
                zen_syntax::ast::Expr::Binary {
                    op: zen_syntax::ast::BinaryOp::Range,
                    ..
                }
            ));
        }
        _ => panic!("expected Ternary"),
    }
}

#[test]
fn p_e15_lambda() {
    let e = parse_expr("lambda x: x + 1");
    match e {
        zen_syntax::ast::Expr::Lambda { params, body, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert!(matches!(
                *body,
                zen_syntax::ast::Expr::Binary {
                    op: zen_syntax::ast::BinaryOp::Add,
                    ..
                }
            ));
        }
        _ => panic!("expected Lambda"),
    }
}

#[test]
fn p_e16_list() {
    let e = parse_expr("[1, 2, 3]");
    assert!(matches!(e, zen_syntax::ast::Expr::List(v) if v.len() == 3));
}

#[test]
fn p_e17_dict() {
    let e = parse_expr("#{a: 1, b: 2}");
    assert!(matches!(e, zen_syntax::ast::Expr::Dict(v) if v.len() == 2));
}

#[test]
fn p_e18_tuple() {
    let e = parse_expr("#(1, 2)");
    assert!(matches!(e, zen_syntax::ast::Expr::TupleLiteral(v) if v.len() == 2));
}

#[test]
fn p_e19_set() {
    let e = parse_expr("%{1, 2}");
    assert!(matches!(e, zen_syntax::ast::Expr::SetLiteral(v) if v.len() == 2));
}

#[test]
fn p_e20_nullish() {
    let e = parse_expr("a ?? b");
    assert!(matches!(e, zen_syntax::ast::Expr::NullCoalesce { .. }));
}

#[test]
fn p_e21_range_inclusive() {
    let e = parse_expr("1 -> 10");
    assert!(matches!(
        e,
        zen_syntax::ast::Expr::Binary {
            op: zen_syntax::ast::BinaryOp::Range,
            ..
        }
    ));
}

#[test]
fn p_e22_range_exclusive() {
    let e = parse_expr("1 .. 10");
    assert!(matches!(
        e,
        zen_syntax::ast::Expr::Binary {
            op: zen_syntax::ast::BinaryOp::Range,
            ..
        }
    ));
}

#[test]
fn p_e23_chained_comparison() {
    let e = parse_expr("1 < x <= 10");
    match e {
        zen_syntax::ast::Expr::Chain { ops, operands, .. } => {
            assert_eq!(ops.len(), 2);
            assert_eq!(operands.len(), 3);
            assert!(matches!(ops[0], zen_syntax::ast::BinaryOp::Lt));
            assert!(matches!(ops[1], zen_syntax::ast::BinaryOp::Le));
        }
        _ => panic!("expected Chain"),
    }
}

#[test]
fn p_e24_precedence_ternary_vs_range() {
    let e = parse_expr("1 .. 5 ? a : b");
    match e {
        zen_syntax::ast::Expr::Ternary { cond, .. } => {
            assert!(matches!(
                *cond,
                zen_syntax::ast::Expr::Binary {
                    op: zen_syntax::ast::BinaryOp::Range,
                    ..
                }
            ));
        }
        _ => panic!("expected Ternary with Range condition"),
    }
}

#[test]
fn p_e25_precedence_nullish_vs_ternary() {
    let e = parse_expr("a ?? b ? c : d");
    match e {
        zen_syntax::ast::Expr::Ternary { cond, .. } => {
            assert!(matches!(*cond, zen_syntax::ast::Expr::NullCoalesce { .. }));
        }
        _ => panic!("expected Ternary with NullCoalesce condition"),
    }
}

// ── Statement tests ──────────────────────────────────────────────────────

#[test]
fn p_s1_let_binding() {
    let stmt = first_stmt("let x = 1");
    match stmt {
        zen_syntax::ast::Stmt::Let {
            name,
            value,
            mutable: true,
            ..
        } => {
            assert_eq!(name, "x");
            assert!(matches!(
                *value,
                zen_syntax::ast::Expr::Literal(zen_syntax::ast::Literal::Int(1))
            ));
        }
        _ => panic!("expected Let"),
    }
}

#[test]
fn p_s2_const_binding() {
    let stmt = first_stmt("const x = 1");
    match stmt {
        zen_syntax::ast::Stmt::Const { name, .. } => assert_eq!(name, "x"),
        _ => panic!("expected Const"),
    }
}

#[test]
fn p_s3_function() {
    let stmt = first_stmt("fn f(x, y = 1) { return x + y }");
    match stmt {
        zen_syntax::ast::Stmt::Function { name, params, .. } => {
            assert_eq!(name, "f");
            assert_eq!(params.len(), 2);
            assert!(params[1].default.is_some());
        }
        _ => panic!("expected Function"),
    }
}

#[test]
fn p_s4_if_statement() {
    let stmt = first_stmt("if a { b } else { c }");
    match stmt {
        zen_syntax::ast::Stmt::If {
            cond,
            then_branch: _,
            else_branch,
            ..
        } => {
            assert!(matches!(*cond, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "a"));
            assert!(else_branch.is_some());
        }
        _ => panic!("expected If"),
    }
}

#[test]
fn p_s5_while_loop() {
    let stmt = first_stmt("while x { y }");
    assert!(matches!(stmt, zen_syntax::ast::Stmt::While { .. }));
}

#[test]
fn p_s6_for_loop() {
    let stmt = first_stmt("for i in xs { y }");
    match stmt {
        zen_syntax::ast::Stmt::For { vars, .. } => assert_eq!(vars.len(), 1),
        _ => panic!("expected For"),
    }
}

#[test]
fn p_s7_return() {
    let stmt = first_stmt("return 1");
    match stmt {
        zen_syntax::ast::Stmt::Return { value, .. } => assert!(value.is_some()),
        _ => panic!("expected Return"),
    }
}

#[test]
fn p_s8_assign() {
    let stmt = first_stmt("x = 1");
    match stmt {
        zen_syntax::ast::Stmt::Assign {
            target,
            op: zen_syntax::ast::AssignOp::Assign,
            ..
        } => {
            assert!(matches!(*target, zen_syntax::ast::Expr::Ident { name: s, .. } if s == "x"));
        }
        _ => panic!("expected Assign"),
    }
}

#[test]
fn p_s9_compound_assign() {
    let stmt = first_stmt("x += 1");
    assert!(matches!(
        stmt,
        zen_syntax::ast::Stmt::Assign {
            op: zen_syntax::ast::AssignOp::AddAssign,
            ..
        }
    ));
}

#[test]
fn p_s10_print() {
    let stmt = first_stmt("print(1, 2)");
    match stmt {
        zen_syntax::ast::Stmt::Print { args, .. } => assert_eq!(args.len(), 2),
        _ => panic!("expected Print"),
    }
}

#[test]
fn p_m1_match_simple() {
    let e = parse_expr("match x { 1 -> \"one\", _ -> \"other\" }");
    match e {
        zen_syntax::ast::Expr::Match { arms, .. } => {
            assert_eq!(arms.len(), 2);
        }
        _ => panic!("expected Match"),
    }
}

#[test]
fn p_m2_when_conditional() {
    let e = parse_expr("when { x > 5: \"big\", _: \"small\" }");
    match e {
        zen_syntax::ast::Expr::When { arms, .. } => {
            assert_eq!(arms.len(), 2);
        }
        _ => panic!("expected When"),
    }
}

#[test]
fn p_t1_try_bare_catch() {
    let stmt = first_stmt("try { f() } catch { g() }");
    match stmt {
        zen_syntax::ast::Stmt::Try {
            catches, finally, ..
        } => {
            assert_eq!(catches.len(), 1);
            assert!(catches[0].typ.is_none() && catches[0].binding.is_none());
            assert!(finally.is_none());
        }
        _ => panic!("expected Try"),
    }
}

#[test]
fn p_t2_try_typed_catch_and_finally() {
    let stmt = first_stmt("try { f() } catch TypeError e { g() } finally { h() }");
    match stmt {
        zen_syntax::ast::Stmt::Try {
            catches, finally, ..
        } => {
            assert_eq!(catches.len(), 1);
            assert_eq!(catches[0].typ.as_deref(), Some("TypeError"));
            assert_eq!(catches[0].binding.as_deref(), Some("e"));
            assert!(finally.is_some());
        }
        _ => panic!("expected Try"),
    }
}

#[test]
fn p_t3_catch_as_binding() {
    let stmt = first_stmt("try { f() } except as err { g(err) }");
    match stmt {
        zen_syntax::ast::Stmt::Try { catches, .. } => {
            assert_eq!(catches[0].binding.as_deref(), Some("err"));
        }
        _ => panic!("expected Try"),
    }
}

#[test]
fn p_t4_throw() {
    let stmt = first_stmt("throw \"boom\"");
    assert!(matches!(stmt, zen_syntax::ast::Stmt::Throw { .. }));
}

#[test]
fn p_i1_import_plain() {
    let stmt = first_stmt("import a.b.c");
    match stmt {
        zen_syntax::ast::Stmt::Import { specs, .. } => {
            assert_eq!(specs.len(), 1);
            assert_eq!(specs[0].path, vec!["a", "b", "c"]);
            assert!(specs[0].alias.is_none());
        }
        _ => panic!("expected Import"),
    }
}

#[test]
fn p_i2_import_alias_and_multi() {
    let stmt = first_stmt("import a, b.c as d");
    match stmt {
        zen_syntax::ast::Stmt::Import { specs, .. } => {
            assert_eq!(specs.len(), 2);
            assert_eq!(specs[0].path, vec!["a"]);
            assert_eq!(specs[1].path, vec!["b", "c"]);
            assert_eq!(specs[1].alias.as_deref(), Some("d"));
        }
        _ => panic!("expected Import"),
    }
}

#[test]
fn p_i3_from_import_names() {
    let stmt = first_stmt("from a.b import x, y as z");
    match stmt {
        zen_syntax::ast::Stmt::FromImport { path, items, .. } => {
            assert_eq!(path, vec!["a", "b"]);
            match items {
                zen_syntax::ast::ImportItems::Names(names) => {
                    assert_eq!(names.len(), 2);
                    assert_eq!(names[0].name, "x");
                    assert_eq!(names[1].alias.as_deref(), Some("z"));
                }
                other => panic!("expected Names, got {other:?}"),
            }
        }
        _ => panic!("expected FromImport"),
    }
}

#[test]
fn p_i4_from_import_star() {
    let stmt = first_stmt("from a import *");
    assert!(matches!(
        stmt,
        zen_syntax::ast::Stmt::FromImport {
            items: zen_syntax::ast::ImportItems::All,
            ..
        }
    ));
}

#[test]
fn p_i5_include() {
    let stmt = first_stmt("include \"./setup.z\"");
    assert!(matches!(stmt, zen_syntax::ast::Stmt::Include { path, .. } if path == "./setup.z"));
}

#[test]
fn p_c1_class_empty() {
    let stmt = first_stmt("class Foo { }");
    assert!(
        matches!(stmt, zen_syntax::ast::Stmt::Class { name, members, .. } if name == "Foo" && members.is_empty())
    );
}

#[test]
fn p_c2_class_extends() {
    let stmt = first_stmt("class Bar extends Foo { }");
    assert!(
        matches!(stmt, zen_syntax::ast::Stmt::Class { name, parent, .. } if name == "Bar" && parent.as_deref() == Some("Foo"))
    );
}

#[test]
fn p_m3_match_guard() {
    let e = parse_expr("match n { x if x > 10: true }");
    assert!(matches!(e, zen_syntax::ast::Expr::Match { .. }));
}

#[test]
fn p_s7_return_value() {
    let stmt = first_stmt("return 42");
    assert!(matches!(
        stmt,
        zen_syntax::ast::Stmt::Return { value: Some(_), .. }
    ));
}

// ── Interpolation ────────────────────────────────────────────────────────

#[test]
fn p_i1_interpolated_string() {
    let e = parse_expr("\"hello ${x}\"");
    match e {
        zen_syntax::ast::Expr::InterpolatedString(parts) => {
            assert_eq!(parts.len(), 2);
            assert!(matches!(&parts[0], zen_syntax::ast::StringPart::Text(s) if s == "hello "));
            assert!(matches!(
                &parts[1],
                zen_syntax::ast::StringPart::Interpolation(_)
            ));
        }
        _ => panic!("expected InterpolatedString"),
    }
}

// ── Legacy / corpus constructs (M1 parse gate) ───────────────────────────

#[test]
fn p_var_binding() {
    let stmt = first_stmt("var x = 1");
    assert!(matches!(
        stmt,
        zen_syntax::ast::Stmt::Let { mutable: true, .. }
    ));
}

#[test]
fn p_new_expression() {
    let e = parse_expr("new Counter(10)");
    match e {
        zen_syntax::ast::Expr::New { class, args, .. } => {
            assert_eq!(class, "Counter");
            assert_eq!(args.len(), 1);
        }
        _ => panic!("expected New"),
    }
}

#[test]
fn p_new_qualified_class() {
    let e = parse_expr("new net.Server()");
    assert!(matches!(e, zen_syntax::ast::Expr::New { class, .. } if class == "net.Server"));
}

#[test]
fn p_fn_expr_block_body() {
    let e = parse_expr("func(x) { return x }");
    assert!(matches!(e, zen_syntax::ast::Expr::FnExpr { params, .. } if params.len() == 1));
}

#[test]
fn p_arrow_expr_body() {
    let e = parse_expr("(x) => x * 2");
    assert!(matches!(
        e,
        zen_syntax::ast::Expr::ArrowFn { params, body: zen_syntax::ast::BlockOrExpr::Expr(_), .. }
            if params.len() == 1
    ));
}

#[test]
fn p_arrow_block_body() {
    let e = parse_expr("(x) => { let y = x; y + 1 }");
    assert!(matches!(
        e,
        zen_syntax::ast::Expr::ArrowFn {
            body: zen_syntax::ast::BlockOrExpr::Block(_),
            ..
        }
    ));
}

#[test]
fn p_arrow_zero_params() {
    let e = parse_expr("() => \"hi\"");
    assert!(matches!(e, zen_syntax::ast::Expr::ArrowFn { params, .. } if params.is_empty()));
}

#[test]
fn p_lambda_legacy_block_body() {
    let e = parse_expr("lambda() { return 1 }");
    assert!(matches!(e, zen_syntax::ast::Expr::FnExpr { params, .. } if params.is_empty()));
}

#[test]
fn p_legacy_dict_multiline_trailing_comma() {
    let e = parse_expr("{\n  \"a\": 1,\n  \"b\": 2,\n}");
    assert!(matches!(e, zen_syntax::ast::Expr::Dict(entries) if entries.len() == 2));
}

#[test]
fn p_list_multiline_trailing_comma() {
    let e = parse_expr("[\n  1,\n  2,\n  3,\n]");
    assert!(matches!(e, zen_syntax::ast::Expr::List(elems) if elems.len() == 3));
}

#[test]
fn p_comprehension_plain() {
    let e = parse_expr("[x for x in [1, 2, 3]]");
    assert!(matches!(
        e,
        zen_syntax::ast::Expr::Comprehension { clauses, .. } if clauses.len() == 1
    ));
}

#[test]
fn p_comprehension_guard_and_nested() {
    let e = parse_expr("[x for x in [1, 2] if x > 1 for y in [3]]");
    match e {
        zen_syntax::ast::Expr::Comprehension { clauses, .. } => {
            assert_eq!(clauses.len(), 2);
            assert!(clauses[0].guard.is_some());
        }
        _ => panic!("expected Comprehension"),
    }
}

#[test]
fn p_slice_colon() {
    let e = parse_expr("s[a:b]");
    assert!(matches!(
        e,
        zen_syntax::ast::Expr::Slice {
            start: Some(_),
            end: Some(_),
            ..
        }
    ));
}

#[test]
fn p_slice_open_ends() {
    assert!(matches!(
        parse_expr("s[:b]"),
        zen_syntax::ast::Expr::Slice {
            start: None,
            end: Some(_),
            ..
        }
    ));
    assert!(matches!(
        parse_expr("s[a:]"),
        zen_syntax::ast::Expr::Slice {
            start: Some(_),
            end: None,
            ..
        }
    ));
    assert!(matches!(
        parse_expr("s[:]"),
        zen_syntax::ast::Expr::Slice {
            start: None,
            end: None,
            ..
        }
    ));
}

#[test]
fn p_slice_comma_bounds() {
    let e = parse_expr("s[0, 1]");
    assert!(matches!(
        e,
        zen_syntax::ast::Expr::Slice {
            start: Some(_),
            end: Some(_),
            ..
        }
    ));
}

#[test]
fn p_not_unary() {
    let e = parse_expr("not x");
    assert!(matches!(
        e,
        zen_syntax::ast::Expr::Unary {
            op: zen_syntax::ast::UnaryOp::Not,
            ..
        }
    ));
}

#[test]
fn p_print_without_parens() {
    let stmt = first_stmt("print \"hi\" + name");
    assert!(matches!(stmt, zen_syntax::ast::Stmt::Print { args, .. } if args.len() == 1));
}

#[test]
fn p_else_if_folds_into_elif() {
    let stmt = first_stmt("if a { x } else if b { y } else { z }");
    match stmt {
        zen_syntax::ast::Stmt::If {
            elifs, else_branch, ..
        } => {
            assert_eq!(elifs.len(), 1);
            assert!(else_branch.is_some());
        }
        _ => panic!("expected If"),
    }
}

#[test]
fn p_when_arrow_separator() {
    let e = parse_expr("when { a => 1, b => 2 }");
    assert!(matches!(e, zen_syntax::ast::Expr::When { arms, .. } if arms.len() == 2));
}

#[test]
fn p_match_colon_dict_body() {
    let e = parse_expr("match x { \"bob\": { name: \"Bob\", id: 7 } }");
    match e {
        zen_syntax::ast::Expr::Match { arms, .. } => {
            assert_eq!(arms.len(), 1);
            assert!(matches!(
                arms[0].body,
                zen_syntax::ast::BlockOrExpr::Expr(_)
            ));
        }
        _ => panic!("expected Match"),
    }
}

#[test]
fn p_match_colon_statement_body() {
    let e = parse_expr("match x { 1: { y = 2 } }");
    match e {
        zen_syntax::ast::Expr::Match { arms, .. } => {
            assert_eq!(arms.len(), 1);
            assert!(matches!(
                arms[0].body,
                zen_syntax::ast::BlockOrExpr::Block(_)
            ));
        }
        _ => panic!("expected Match"),
    }
}

#[test]
fn p_p1_pymport_plain() {
    let stmt = first_stmt("pymport whatsapp");
    match stmt {
        zen_syntax::ast::Stmt::Pymport { path, alias, .. } => {
            assert_eq!(path, vec!["whatsapp"]);
            assert!(alias.is_none());
        }
        _ => panic!("expected Pymport"),
    }
}

#[test]
fn p_p2_pymport_dotted_alias() {
    let stmt = first_stmt("pymport a.b as c");
    match stmt {
        zen_syntax::ast::Stmt::Pymport { path, alias, .. } => {
            assert_eq!(path, vec!["a", "b"]);
            assert_eq!(alias.as_deref(), Some("c"));
        }
        _ => panic!("expected Pymport"),
    }
}

#[test]
fn p_p3_pymport_needs_name() {
    assert!(parse("pymport").is_err());
    assert!(parse("pymport 123").is_err());
}
