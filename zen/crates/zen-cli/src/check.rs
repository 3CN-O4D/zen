//! `zen check`: parse, compile, and statically reject obviously-bad programs
//! without executing them.
//!
//! The compiler already rejects undefined names and structural mistakes. This
//! pass adds the checks that need only the AST and are cheap to run:
//!
//! * **Division (or remainder) by a literal zero** — `1 / 0`, `x % 0`, and
//!   `1.0 / 0.0` are almost always mistakes, and `docs/19-roadmap.md`'s M8 gate
//!   names `1/0` explicitly as something `check` must catch. A divisor that is
//!   a runtime-computed zero is *not* flagged (it is a legitimate runtime
//!   `DivideByZero`, not a typo the checker can prove).

use zen_syntax::ast::{Expr, BinaryOp, Literal, Block, MatchArm, Pattern, Stmt};
use zen_syntax::parser::Parser;
use zen_syntax::span::{FileId, LineIndex, Span};
use zen_syntax::{lex, Severity};

/// A statically-detected error, ready to render.
#[derive(Debug, Clone)]
pub struct CheckError {
    pub message: String,
    pub code: &'static str,
    pub span: Span,
    pub help: String,
}

/// Parse `source` and run the static checks. Returns the first error, if any.
pub fn check_source(path: &str, source: &str) -> Result<(), String> {
    let file_id = FileId(0);
    let lx = lex(source, file_id);
    if let Some(err) = lx.errors.iter().find(|e| e.severity == Severity::Error) {
        return Err(format!("{path}: lexical error: {:?}", err.kind));
    }
    let mut parser = Parser::new(lx);
    let ast = parser
        .parse_module()
        .map_err(|e| format!("{path}: parse error: {e:?}"))?;
    if let Some(err) = division_by_zero(&ast.items) {
        return Err(render(path, source, &err));
    }
    Ok(())
}

/// Render a check error rustc-style.
pub fn render(path: &str, source: &str, err: &CheckError) -> String {
    let idx = LineIndex::build(source);
    let (line, col) = idx.line_col(err.span.lo);
    let line_text = idx.line_text(line).unwrap_or("");
    let width = err
        .span
        .hi
        .saturating_sub(err.span.lo)
        .min(40)
        .max(1) as usize;
    format!(
        "error: {msg}[{code}]\n  --> {path}:{line}:{col}\n   |\n{lnum:>4} | {text}\n   | {pad}{carets}\n   |\n   = help: {help}\n   = note: run `zen explain {code}` for detail\n",
        msg = err.message,
        code = err.code,
        path = path,
        lnum = line,
        text = line_text,
        pad = " ".repeat(col.saturating_sub(1) as usize),
        carets = "^".repeat(width),
        help = err.help,
    )
}

fn division_by_zero(items: &[Stmt]) -> Option<CheckError> {
    let mut found = None;
    walk_stmts(items, &mut found);
    found
}

fn is_literal_zero(e: &Expr) -> bool {
    match e {
        Expr::Literal(Literal::Int(0)) => true,
        Expr::Literal(Literal::Float(f)) => *f == 0.0,
        _ => false,
    }
}

fn set_err(found: &mut Option<CheckError>, span: Span, op: &str) {
    if found.is_none() {
        *found = Some(CheckError {
            message: format!("attempt to divide by zero (`{op}` with a literal `0`)"),
            // Z0323, not Z0401: Z0401 is the *runtime* TypeError. Reusing it
            // here would give one code two meanings; static checks live in
            // the Z03 (compile-time) band.
            code: "Z0323",
            span,
            help: "replace the literal `0` with a non-zero value, or guard the operation with `if d != 0 { ... }`".to_string(),
        });
    }
}

fn walk_stmts(items: &[Stmt], found: &mut Option<CheckError>) {
    for s in items {
        walk_stmt(s, found);
    }
}

fn walk_block(b: &Block, found: &mut Option<CheckError>) {
    walk_stmts(&b.stmts, found);
}

fn walk_stmt(s: &Stmt, found: &mut Option<CheckError>) {
    match s {
        Stmt::Let { value, .. }
        | Stmt::Const { value, .. }
        | Stmt::Shared { value, .. }
        | Stmt::Expr(value) => walk_expr(value, found),
        Stmt::Assign { target, value, .. } => {
            walk_expr(target, found);
            walk_expr(value, found);
        }
        Stmt::Print { args, sep, end, .. } => {
            for a in args {
                walk_expr(a, found);
            }
            if let Some(sep) = sep {
                walk_expr(sep, found);
            }
            if let Some(end) = end {
                walk_expr(end, found);
            }
        }
        Stmt::Return { value, .. } => {
            if let Some(v) = value {
                walk_expr(v, found);
            }
        }
        Stmt::Throw { value, .. } => walk_expr(value, found),
        Stmt::If {
            cond,
            then_branch,
            elifs,
            else_branch,
            ..
        } => {
            walk_expr(cond, found);
            walk_block(then_branch, found);
            for (c, b) in elifs {
                walk_expr(c, found);
                walk_block(b, found);
            }
            if let Some(b) = else_branch {
                walk_block(b, found);
            }
        }
        Stmt::While { cond, body, .. } => {
            walk_expr(cond, found);
            walk_block(body, found);
        }
        Stmt::For { iter, body, .. } => {
            walk_expr(iter, found);
            walk_block(body, found);
        }
        Stmt::Function { params, body, .. } => {
            for p in params {
                if let Some(d) = &p.default {
                    walk_expr(d, found);
                }
            }
            walk_block(body, found);
        }
        Stmt::Try {
            body,
            catches,
            finally,
            ..
        } => {
            walk_block(body, found);
            for c in catches {
                walk_block(&c.body, found);
            }
            if let Some(f) = finally {
                walk_block(f, found);
            }
        }
        Stmt::Class { members, .. } => {
            for m in members {
                for p in &m.params {
                    if let Some(d) = &p.default {
                        walk_expr(d, found);
                    }
                }
                walk_block(&m.body, found);
            }
        }
        Stmt::Match { subject, arms, .. } => {
            walk_expr(subject, found);
            for a in arms {
                walk_arm(a, found);
            }
        }
        Stmt::When { arms, .. } => {
            for a in arms {
                walk_arm(a, found);
            }
        }
        Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Import { .. }
        | Stmt::Pymport { .. }
        | Stmt::FromImport { .. }
        | Stmt::Include { .. } => {}
    }
}

fn walk_arm(a: &MatchArm, found: &mut Option<CheckError>) {
    walk_pattern(&a.pattern, found);
    match &a.body {
        zen_syntax::ast::BlockOrExpr::Expr(e) => walk_expr(e, found),
        zen_syntax::ast::BlockOrExpr::Block(b) => walk_block(b, found),
    }
}

fn walk_pattern(p: &Pattern, found: &mut Option<CheckError>) {
    match p {
        Pattern::Constructor { fields, .. } => {
            for f in fields {
                walk_pattern(f, found);
            }
        }
        Pattern::List { elems, rest, .. } => {
            for e in elems {
                walk_pattern(e, found);
            }
            if let Some(r) = rest {
                walk_pattern(r, found);
            }
        }
        Pattern::Tuple { elems, .. } => {
            for e in elems {
                walk_pattern(e, found);
            }
        }
        Pattern::Dict { entries, rest, .. } => {
            for (k, v) in entries {
                walk_pattern(k, found);
                walk_pattern(v, found);
            }
            if let Some(r) = rest {
                walk_pattern(r, found);
            }
        }
        Pattern::Or { alts, .. } => {
            for a in alts {
                walk_pattern(a, found);
            }
        }
        Pattern::Bind { pat, .. } => walk_pattern(pat, found),
        Pattern::Guard { pat, guards, .. } => {
            walk_pattern(pat, found);
            for g in guards {
                walk_expr(g, found);
            }
        }
        Pattern::Condition { cond, .. } => walk_expr(cond, found),
        Pattern::Range { lo, hi, .. } => {
            walk_expr(lo, found);
            walk_expr(hi, found);
        }
        Pattern::Wildcard(_) | Pattern::Binding { .. } | Pattern::Literal { .. } => {}
    }
}

fn walk_expr(e: &Expr, found: &mut Option<CheckError>) {
    match e {
        Expr::Binary { op, left, right, span } => {
            walk_expr(left, found);
            walk_expr(right, found);
            if matches!(op, BinaryOp::Div | BinaryOp::Mod) && is_literal_zero(right) {
                set_err(found, *span, if matches!(op, BinaryOp::Div) { "/" } else { "%" });
            }
        }
        Expr::Unary { operand, .. } => walk_expr(operand, found),
        Expr::Call { callee, args, .. } => {
            walk_expr(callee, found);
            for a in args {
                walk_expr(a, found);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            walk_expr(receiver, found);
            for a in args {
                walk_expr(a, found);
            }
        }
        Expr::Index { object, index, .. } => {
            walk_expr(object, found);
            walk_expr(index, found);
        }
        Expr::Slice { object, start, end, .. } => {
            walk_expr(object, found);
            if let Some(s) = start {
                walk_expr(s, found);
            }
            if let Some(e) = end {
                walk_expr(e, found);
            }
        }
        Expr::Field { object, .. } | Expr::OptionalField { object, .. } => {
            walk_expr(object, found)
        }
        Expr::List(items) | Expr::Set(items) | Expr::Tuple(items) => {
            for i in items {
                walk_expr(i, found);
            }
        }
        Expr::Comprehension { key, body, clauses, .. } => {
            for c in clauses {
                walk_expr(&c.iter, found);
                if let Some(g) = &c.guard {
                    walk_expr(g, found);
                }
            }
            if let Some(k) = key {
                walk_expr(k, found);
            }
            walk_expr(body, found);
        }
        Expr::Dict(entries) => {
            for (k, v) in entries {
                walk_expr(k, found);
                walk_expr(v, found);
            }
        }
        Expr::DictLiteral(entries) => {
            for v in entries.values() {
                walk_expr(v, found);
            }
        }
        Expr::SetLiteral(items) | Expr::TupleLiteral(items) => {
            for i in items {
                walk_expr(i, found);
            }
        }
        Expr::Lambda { params, body, .. } => {
            for p in params {
                if let Some(d) = &p.default {
                    walk_expr(d, found);
                }
            }
            walk_expr(body, found);
        }
        Expr::FnExpr { params, body, .. } => {
            for p in params {
                if let Some(d) = &p.default {
                    walk_expr(d, found);
                }
            }
            walk_block(body, found);
        }
        Expr::ArrowFn { params, body, .. } => {
            for p in params {
                if let Some(d) = &p.default {
                    walk_expr(d, found);
                }
            }
            match body {
                zen_syntax::ast::BlockOrExpr::Expr(e) => walk_expr(e, found),
                zen_syntax::ast::BlockOrExpr::Block(b) => walk_block(b, found),
            }
        }
        Expr::IfExpr { arms, otherwise, .. } => {
            for (c, b) in arms {
                walk_expr(c, found);
                walk_block(b, found);
            }
            if let Some(o) = otherwise {
                walk_block(o, found);
            }
        }
        Expr::Ternary { cond, then_branch, else_branch, .. } => {
            walk_expr(cond, found);
            walk_expr(then_branch, found);
            walk_expr(else_branch, found);
        }
        Expr::Chain { operands, .. } => {
            for o in operands {
                walk_expr(o, found);
            }
        }
        Expr::NullCoalesce { left, right, .. } => {
            walk_expr(left, found);
            walk_expr(right, found);
        }
        Expr::Range { start, end, .. } => {
            walk_expr(start, found);
            walk_expr(end, found);
        }
        Expr::Match { subject, arms, .. } => {
            walk_expr(subject, found);
            for a in arms {
                walk_arm(a, found);
            }
        }
        Expr::When { arms, .. } => {
            for a in arms {
                walk_arm(a, found);
            }
        }
        Expr::New { args, .. } => {
            for a in args {
                walk_expr(a, found);
            }
        }
        Expr::SuperCall { args, .. } | Expr::SuperMethod { args, .. } => {
            for a in args {
                walk_expr(a, found);
            }
        }
        Expr::InterpolatedString(parts) => {
            for p in parts {
                if let zen_syntax::ast::StringPart::Interpolation(e) = p {
                    walk_expr(e, found);
                }
            }
        }
        Expr::Literal(_) | Expr::Ident { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(src: &str) -> Option<String> {
        check_source("test.z", src).err()
    }

    #[test]
    fn flags_integer_division_by_zero() {
        assert!(err("let x = 1 / 0\nprint(x)\n").is_some());
    }

    #[test]
    fn flags_float_division_by_zero() {
        assert!(err("let x = 1.0 / 0.0\nprint(x)\n").is_some());
    }

    #[test]
    fn flags_remainder_by_zero() {
        assert!(err("let x = 7 % 0\nprint(x)\n").is_some());
    }

    #[test]
    fn allows_nonzero_divisor() {
        assert!(err("let x = 1 / 2\nprint(x)\n").is_none());
    }

    #[test]
    fn allows_runtime_divisor() {
        assert!(err("let d = 0\nlet x = 1 / d\nprint(x)\n").is_none());
    }
}
