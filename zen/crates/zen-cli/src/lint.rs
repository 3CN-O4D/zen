//! `zen lint`: source-level warnings for suspicious patterns.
//!
//! A single-file AST pass that reports problems statically analyzable without
//! running the program:
//!
//! 1. **Unused variables** — a `let`/`const`/`shared`, `for` variable, catch
//!    binding, or destructuring binding that is never read or written.
//! 2. **Unused parameters** — a function/lambda/method parameter never used in
//!    its body.
//! 3. **Unreachable code** — statements after a `return`/`break`/`continue`/
//!    `throw` in the same block.
//!
//! Names beginning with `_` (and `self`) are deliberately ignored, matching the
//! Python convention. A same-scope redefinition marks the shadowed binding as
//! used, so the common `let x = ..; let x = ..` rewrite does not produce noise.
//! Imports and top-level `fn`/`class` declarations are never reported:
//! side-effect imports and cross-module exports are legitimate uses a single
//! file cannot see.

use zen_syntax::ast::{Block, CatchClause, Expr, MatchArm, Param, Pattern, Stmt};
use zen_syntax::parser::Parser;
use zen_syntax::span::{FileId, LineIndex, Span};
use zen_syntax::{lex, Severity};

/// A single lint diagnostic.
#[derive(Debug, Clone)]
pub struct LintWarning {
    /// Stable short name, reported with a `Z050x`-style code on the CLI.
    pub code: &'static str,
    pub message: String,
    /// The statement (or parameter) the warning points at.
    pub span: Span,
}

/// Run the lint pass over `source` and return the warnings found.
pub fn lint_source(path: &str, source: &str) -> Result<Vec<LintWarning>, String> {
    let file_id = FileId(0);
    let lx = lex(source, file_id);
    if let Some(err) = lx.errors.iter().find(|e| e.severity == Severity::Error) {
        return Err(format!("{path}: lexical error: {:?}", err.kind));
    }
    let mut parser = Parser::new(lx);
    let ast = parser
        .parse_module()
        .map_err(|e| format!("{path}: parse error: {e:?}"))?;
    let mut walker = Walker {
        scopes: Vec::new(),
        warnings: Vec::new(),
    };
    walker.scopes.push(Scope { bindings: Vec::new() });
    walker.walk_statements(&ast.items, false);
    report(walker.scopes.last_mut().unwrap(), &mut walker.warnings);
    walker.scopes.pop();
    walker.warnings.sort_by_key(|w| w.span.lo);
    Ok(walker.warnings)
}

/// Run `lint_source` over a file on disk.
#[cfg(test)]
fn lint_file(path: &str) -> Result<Vec<LintWarning>, String> {
    use std::fs;
    let source = fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    lint_source(path, &source)
}

pub fn render_warning(path: &str, source: &str, w: &LintWarning) -> String {
    let idx = LineIndex::build(source);
    let (line, col) = idx.line_col(w.span.lo);
    let line_text = idx.line_text(line).unwrap_or("");
    let width = w
        .span
        .hi
        .saturating_sub(w.span.lo)
        .min(40)
        .max(1) as usize;
    format!(
        "warning[Z-lint]: {msg} ({code})\n  --> {path}:{line}:{col}\n   |\n{lnum:>4} | {text}\n   | {pad}{carets} {code}\n",
        msg = w.message,
        path = path,
        lnum = line,
        text = line_text,
        pad = " ".repeat(col.saturating_sub(1) as usize),
        carets = "^".repeat(width),
        code = w.code,
    )
}

struct Scope {
    bindings: Vec<Binding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingKind {
    Variable,
    Parameter,
}

struct Binding {
    name: String,
    kind: BindingKind,
    used: bool,
    span: Span,
    shadowed: bool,
}

struct Walker {
    scopes: Vec<Scope>,
    warnings: Vec<LintWarning>,
}

fn is_special(name: &str) -> bool {
    name.starts_with('_') || name == "self"
}

impl Walker {
    fn bind(&mut self, name: &str, kind: BindingKind, span: Span) {
        if is_special(name) {
            return;
        }
        let scope = self.scopes.last_mut().unwrap();
        // A same-scope redefinition is a real use of the shadowed binding.
        for b in scope.bindings.iter_mut().rev() {
            if b.name == name && !b.shadowed {
                b.used = true;
                b.shadowed = true;
                break;
            }
        }
        scope.bindings.push(Binding {
            name: name.to_string(),
            kind,
            used: false,
            span,
            shadowed: false,
        });
    }

    fn mark_used(&mut self, name: &str) {
        if is_special(name) {
            return;
        }
        for scope in self.scopes.iter_mut().rev() {
            for b in scope.bindings.iter_mut().rev() {
                if b.name == name {
                    b.used = true;
                    return;
                }
            }
        }
    }

    // ── statements ────────────────────────────────────────────────────────

    fn walk_statements(&mut self, stmts: &[Stmt], mut dead: bool) {
        for s in stmts {
            if dead && !matches!(
                s,
                Stmt::Function { .. }
                    | Stmt::Class { .. }
                    | Stmt::Import { .. }
                    | Stmt::Pymport { .. }
                    | Stmt::FromImport { .. }
                    | Stmt::Include { .. }
            ) {
                self.warnings.push(LintWarning {
                    code: "unreachable-code",
                    message: "statement is unreachable".into(),
                    span: stmt_span(s),
                });
            }
            let terminates = self.walk_stmt(s);
            if terminates {
                dead = true;
            }
        }
    }

    fn walk_stmt(&mut self, s: &Stmt) -> bool {
        match s {
            Stmt::Let { name, value, span, .. }
            | Stmt::Const { name, value, span, .. }
            | Stmt::Shared { name, value, span, .. } => {
                self.walk_expr(value);
                self.bind(name, BindingKind::Variable, *span);
                false
            }
            Stmt::Assign { target, value, .. } => {
                self.mark_expr_uses(target);
                self.walk_expr(value);
                false
            }
            Stmt::Expr(e) => {
                self.walk_expr(e);
                false
            }
            Stmt::Print { args, sep, end, .. } => {
                for a in args {
                    self.walk_expr(a);
                }
                if let Some(sep) = sep {
                    self.walk_expr(sep);
                }
                if let Some(end) = end {
                    self.walk_expr(end);
                }
                false
            }
            Stmt::If {
                cond,
                then_branch,
                elifs,
                else_branch,
                ..
            } => {
                self.walk_expr(cond);
                self.enter_block(then_branch);
                for (c, b) in elifs {
                    self.walk_expr(c);
                    self.enter_block(b);
                }
                if let Some(b) = else_branch {
                    self.enter_block(b);
                }
                false
            }
            Stmt::While { cond, body, .. } => {
                self.walk_expr(cond);
                self.enter_block(body);
                false
            }
            Stmt::For { vars, iter, body, .. } => {
                self.walk_expr(iter);
                self.scopes.push(Scope { bindings: Vec::new() });
                for v in vars {
                    // The whole loop is the binding's "statement".
                    self.bind(v, BindingKind::Variable, s_span(s));
                }
                self.walk_block_stmts(body, false);
                report(self.scopes.last_mut().unwrap(), &mut self.warnings);
                self.scopes.pop();
                false
            }
            Stmt::Function { name, params, body, .. } => {
                self.walk_function(name, params, body);
                false
            }
            Stmt::Return { value, .. } => {
                if let Some(v) = value {
                    self.walk_expr(v);
                }
                true
            }
            Stmt::Break { .. } | Stmt::Continue { .. } => true,
            Stmt::Try {
                body,
                catches,
                finally,
                ..
            } => {
                self.enter_block(body);
                for c in catches {
                    self.walk_catch(c);
                }
                if let Some(f) = finally {
                    self.enter_block(f);
                }
                false
            }
            Stmt::Throw { value, .. } => {
                self.walk_expr(value);
                true
            }
            Stmt::Import { .. }
            | Stmt::Pymport { .. }
            | Stmt::FromImport { .. }
            | Stmt::Include { .. } => false,
            Stmt::Class { name, parent, members, .. } => {
                // Classes are used through construction and `field()` the pass
                // cannot fully resolve; treat the name as used.
                self.mark_used(name);
                if let Some(p) = parent {
                    self.mark_used(p);
                }
                self.scopes.push(Scope { bindings: Vec::new() });
                for m in members {
                    for p in &m.params {
                        self.walk_param(p);
                    }
                    self.walk_block_stmts(&m.body, false);
                }
                report(self.scopes.last_mut().unwrap(), &mut self.warnings);
                self.scopes.pop();
                false
            }
            Stmt::Match { subject, arms, .. } => {
                self.walk_expr(subject);
                for a in arms {
                    self.walk_arm(a);
                }
                false
            }
            Stmt::When { arms, .. } => {
                for a in arms {
                    self.walk_arm(a);
                }
                false
            }
        }
    }

    fn walk_block_stmts(&mut self, block: &Block, dead: bool) {
        self.walk_statements(&block.stmts, dead);
    }

    fn enter_block(&mut self, block: &Block) {
        self.scopes.push(Scope { bindings: Vec::new() });
        self.walk_block_stmts(block, false);
        report(self.scopes.last_mut().unwrap(), &mut self.warnings);
        self.scopes.pop();
    }

    fn walk_function(&mut self, name: &str, params: &[Param], body: &Block) {
        self.mark_used(name);
        self.scopes.push(Scope { bindings: Vec::new() });
        for p in params {
            self.walk_param(p);
        }
        self.walk_block_stmts(body, false);
        report(self.scopes.last_mut().unwrap(), &mut self.warnings);
        self.scopes.pop();
    }

    fn walk_param(&mut self, p: &Param) {
        if let Some(d) = &p.default {
            self.walk_expr(d);
        }
        self.bind(&p.name, BindingKind::Parameter, p.span);
    }

    fn walk_catch(&mut self, c: &CatchClause) {
        self.scopes.push(Scope { bindings: Vec::new() });
        self.bind(c.binding.as_deref().unwrap_or("_"), BindingKind::Variable, c.span);
        self.walk_block_stmts(&c.body, false);
        report(self.scopes.last_mut().unwrap(), &mut self.warnings);
        self.scopes.pop();
    }

    fn walk_arm(&mut self, arm: &MatchArm) {
        self.scopes.push(Scope { bindings: Vec::new() });
        self.bind_pattern(&arm.pattern);
        match &arm.body {
            zen_syntax::ast::BlockOrExpr::Expr(e) => self.walk_expr(e),
            zen_syntax::ast::BlockOrExpr::Block(b) => self.walk_block_stmts(b, false),
        }
        report(self.scopes.last_mut().unwrap(), &mut self.warnings);
        self.scopes.pop();
    }

    fn bind_pattern(&mut self, pat: &Pattern) {
        match pat {
            Pattern::Binding { name, span } => {
                self.bind(name, BindingKind::Variable, *span)
            }
            Pattern::Bind { pat, name, span } => {
                self.bind_pattern(pat);
                self.bind(name, BindingKind::Variable, *span);
            }
            Pattern::Or { alts, .. } => {
                for a in alts {
                    self.bind_pattern(a);
                }
            }
            Pattern::Constructor { fields, .. } => {
                for f in fields {
                    self.bind_pattern(f);
                }
            }
            Pattern::List { elems, rest, .. } => {
                for e in elems {
                    self.bind_pattern(e);
                }
                if let Some(r) = rest {
                    self.bind_pattern(r);
                }
            }
            Pattern::Tuple { elems, .. } => {
                for e in elems {
                    self.bind_pattern(e);
                }
            }
            Pattern::Dict { entries, rest, .. } => {
                for (k, v) in entries {
                    self.bind_pattern(k);
                    self.bind_pattern(v);
                }
                if let Some(r) = rest {
                    self.bind_pattern(r);
                }
            }
            Pattern::Guard { pat, guards, .. } => {
                self.bind_pattern(pat);
                for g in guards {
                    self.walk_expr(g);
                }
            }
            Pattern::Wildcard(_) => {}
            Pattern::Literal { .. } | Pattern::Range { .. } => {}
            Pattern::Condition { cond, .. } => self.walk_expr(cond),
        }
    }

    // ── expressions ───────────────────────────────────────────────────────

    fn walk_expr(&mut self, e: &Expr) {
        match e {
            Expr::Literal(_) => {}
            Expr::Ident { name, .. } => self.mark_used(name),
            Expr::Binary { left, right, .. } => {
                self.walk_expr(left);
                self.walk_expr(right);
            }
            Expr::Unary { operand, .. } => self.walk_expr(operand),
            Expr::Call { callee, args, .. } => {
                self.walk_expr(callee);
                for a in args {
                    self.walk_expr(a);
                }
            }
            Expr::MethodCall { receiver, args, .. } => {
                self.walk_expr(receiver);
                for a in args {
                    self.walk_expr(a);
                }
            }
            Expr::Index { object, index, .. } => {
                self.walk_expr(object);
                self.walk_expr(index);
            }
            Expr::Slice { object, start, end, .. } => {
                self.walk_expr(object);
                if let Some(s) = start {
                    self.walk_expr(s);
                }
                if let Some(e) = end {
                    self.walk_expr(e);
                }
            }
            Expr::Field { object, .. } | Expr::OptionalField { object, .. } => {
                self.walk_expr(object)
            }
            Expr::List(items) | Expr::Set(items) | Expr::Tuple(items) => {
                for i in items {
                    self.walk_expr(i);
                }
            }
            Expr::Comprehension { key, body, clauses, .. } => {
                self.scopes.push(Scope { bindings: Vec::new() });
                for c in clauses {
                    self.walk_expr(&c.iter);
                    if let Some(g) = &c.guard {
                        self.walk_expr(g);
                    }
                    self.bind(&c.var, BindingKind::Variable, expr_span(&c.iter));
                }
                if let Some(k) = key {
                    self.walk_expr(k);
                }
                self.walk_expr(body);
                report(self.scopes.last_mut().unwrap(), &mut self.warnings);
                self.scopes.pop();
            }
            Expr::Dict(entries) => {
                for (k, v) in entries {
                    self.walk_expr(k);
                    self.walk_expr(v);
                }
            }
            Expr::DictLiteral(entries) => {
                for v in entries.values() {
                    self.walk_expr(v);
                }
            }
            Expr::SetLiteral(items) | Expr::TupleLiteral(items) => {
                for i in items {
                    self.walk_expr(i);
                }
            }
            Expr::Lambda { params, body, .. } => {
                self.scopes.push(Scope { bindings: Vec::new() });
                for p in params {
                    self.walk_param(p);
                }
                self.walk_expr(body);
                report(self.scopes.last_mut().unwrap(), &mut self.warnings);
                self.scopes.pop();
            }
            Expr::FnExpr { params, body, .. } => {
                self.scopes.push(Scope { bindings: Vec::new() });
                for p in params {
                    self.walk_param(p);
                }
                self.enter_block(body);
                report(self.scopes.last_mut().unwrap(), &mut self.warnings);
                self.scopes.pop();
            }
            Expr::ArrowFn { params, body, .. } => {
                self.scopes.push(Scope { bindings: Vec::new() });
                for p in params {
                    self.walk_param(p);
                }
                match body {
                    zen_syntax::ast::BlockOrExpr::Expr(e) => self.walk_expr(e),
                    zen_syntax::ast::BlockOrExpr::Block(b) => self.enter_block(b),
                }
                report(self.scopes.last_mut().unwrap(), &mut self.warnings);
                self.scopes.pop();
            }
            Expr::IfExpr { arms, otherwise, .. } => {
                for (c, b) in arms {
                    self.walk_expr(c);
                    self.enter_block(b);
                }
                if let Some(o) = otherwise {
                    self.enter_block(o);
                }
            }
            Expr::Ternary { cond, then_branch, else_branch, .. } => {
                self.walk_expr(cond);
                self.walk_expr(then_branch);
                self.walk_expr(else_branch);
            }
            Expr::Chain { operands, .. } => {
                for o in operands {
                    self.walk_expr(o);
                }
            }
            Expr::NullCoalesce { left, right, .. } => {
                self.walk_expr(left);
                self.walk_expr(right);
            }
            Expr::Range { start, end, .. } => {
                self.walk_expr(start);
                self.walk_expr(end);
            }
            Expr::Match { subject, arms, .. } => {
                self.walk_expr(subject);
                for a in arms {
                    self.walk_arm(a);
                }
            }
            Expr::When { arms, .. } => {
                for a in arms {
                    self.walk_arm(a);
                }
            }
            Expr::New { class, args, .. } => {
                self.mark_used(class);
                for a in args {
                    self.walk_expr(a);
                }
            }
            Expr::SuperCall { args, .. } | Expr::SuperMethod { args, .. } => {
                for a in args {
                    self.walk_expr(a);
                }
            }
            Expr::InterpolatedString(parts) => {
                for p in parts {
                    if let zen_syntax::ast::StringPart::Interpolation(e) = p {
                        self.walk_expr(e);
                    }
                }
            }
        }
    }

    /// Mark uses for an assignment target. A plain `x = ..` is a write; an
    /// index/field target reads the container. Either marks the binding used,
    /// which is the conservative outcome that avoids false positives.
    fn mark_expr_uses(&mut self, e: &Expr) {
        match e {
            Expr::Index { object, index, .. } => {
                self.mark_expr_uses(object);
                self.walk_expr(index);
            }
            Expr::Field { object, .. } => self.mark_expr_uses(object),
            Expr::Tuple(items) => {
                for i in items {
                    self.mark_expr_uses(i);
                }
            }
            Expr::Ident { name, .. } => self.mark_used(name),
            other => self.walk_expr(other),
        }
    }
}

/// Emit warnings for every unused binding in `scope`. `scope` and `warnings`
/// are passed separately so call sites borrow two disjoint fields of `Walker`
/// and avoid a double `&mut self`.
fn report(scope: &mut Scope, warnings: &mut Vec<LintWarning>) {
    for b in scope.bindings.iter().rev() {
        if b.shadowed || b.used || is_special(&b.name) {
            continue;
        }
        let (code, what) = match b.kind {
            BindingKind::Variable => ("unused-variable", "unused variable"),
            BindingKind::Parameter => ("unused-parameter", "unused parameter"),
        };
        warnings.push(LintWarning {
            code,
            message: format!(
                "{what} `{}` is never used; prefix with `_` to silence",
                b.name
            ),
            span: b.span,
        });
    }
}

fn stmt_span(s: &Stmt) -> Span {
    match s {
        Stmt::Let { span, .. }
        | Stmt::Const { span, .. }
        | Stmt::Assign { span, .. }
        | Stmt::Print { span, .. }
        | Stmt::If { span, .. }
        | Stmt::While { span, .. }
        | Stmt::For { span, .. }
        | Stmt::Function { span, .. }
        | Stmt::Return { span, .. }
        | Stmt::Break { span }
        | Stmt::Continue { span }
        | Stmt::Try { span, .. }
        | Stmt::Throw { span, .. }
        | Stmt::Import { span, .. }
        | Stmt::Pymport { span, .. }
        | Stmt::FromImport { span, .. }
        | Stmt::Include { span, .. }
        | Stmt::Class { span, .. }
        | Stmt::Match { span, .. }
        | Stmt::When { span, .. }
        | Stmt::Shared { span, .. } => *span,
        Stmt::Expr(e) => expr_span(e),
    }
}

fn s_span(s: &Stmt) -> Span {
    stmt_span(s)
}

/// Span of an expression, best-effort: variants with a `span` field report it;
/// container literals fall back to a zero-length span at the file start.
fn expr_span(e: &Expr) -> Span {
    match e {
        Expr::Ident { span, .. }
        | Expr::Binary { span, .. }
        | Expr::Unary { span, .. }
        | Expr::Call { span, .. }
        | Expr::MethodCall { span, .. }
        | Expr::Index { span, .. }
        | Expr::Slice { span, .. }
        | Expr::Field { span, .. }
        | Expr::OptionalField { span, .. }
        | Expr::Comprehension { span, .. }
        | Expr::Lambda { span, .. }
        | Expr::FnExpr { span, .. }
        | Expr::ArrowFn { span, .. }
        | Expr::IfExpr { span, .. }
        | Expr::Ternary { span, .. }
        | Expr::Chain { span, .. }
        | Expr::NullCoalesce { span, .. }
        | Expr::Range { span, .. }
        | Expr::Match { span, .. }
        | Expr::When { span, .. }
        | Expr::New { span, .. }
        | Expr::SuperCall { span, .. }
        | Expr::SuperMethod { span, .. } => *span,
        _ => Span::empty_at(FileId(0), 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lint(src: &str) -> Vec<LintWarning> {
        lint_source("test.z", src).unwrap()
    }

    #[test]
    fn reports_unused_local() {
        let ws = lint("let x = 1\nprint(1)\n");
        assert!(ws.iter().any(|w| w.code == "unused-variable" && w.message.contains("`x`")));
    }

    #[test]
    fn used_local_is_silent() {
        let ws = lint("let x = 1\nprint(x)\n");
        assert!(!ws.iter().any(|w| w.code == "unused-variable"));
    }

    #[test]
    fn underscore_prefix_is_silent() {
        let ws = lint("let _x = 1\nprint(1)\n");
        assert!(!ws.iter().any(|w| w.message.contains("`_x`")));
    }

    #[test]
    fn reports_unused_parameter() {
        let ws = lint("fn f(a, b) { print(a) }\n");
        assert!(ws.iter().any(|w| w.code == "unused-parameter" && w.message.contains("`b`")));
        assert!(!ws.iter().any(|w| w.code == "unused-parameter" && w.message.contains("`a`")));
    }

    #[test]
    fn reports_unreachable_code() {
        let ws = lint("fn f() { return 1\n print(2) }\n");
        assert!(ws.iter().any(|w| w.code == "unreachable-code"));
    }

    #[test]
    fn shadow_does_not_warn_outer() {
        let ws = lint("let x = 1\nlet x = x + 1\nprint(x)\n");
        assert!(!ws.iter().any(|w| w.code == "unused-variable"));
    }

    #[test]
    fn self_param_is_silent() {
        let ws = lint("class C {\n  func method(self, y) {\n    print(self)\n  }\n}\nC().method(1)\n");
        assert!(!ws.iter().any(|w| w.message.contains("`self`")));
    }
}