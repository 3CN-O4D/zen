//! Parser skeleton for the Zen language, on the new [`Lexed`] API.
//!
//! This is *not* the full `docs/06-parser.md` implementation — that is a
//! later milestone. It covers the common expression and statement forms so the
//! workspace builds and the lexer can be validated end to end, and it never
//! panics: every failure path returns a [`ParseError`].
//!
//! Spans are honest but coarse: leaves (`Literal`, `Ident`) carry no span in
//! the AST, so parent nodes carry the running `lo..hi` of everything parsed so
//! far in the statement. The Milestone-2 rewrite will fix this properly.

use crate::ast::*;
use crate::lex::{InterpPart, Lexed, NumLit, Token, TokenKind};
use crate::span::Span;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("{span}: expected {expected}, found {found}")]
    ExpectedToken {
        span: Span,
        expected: String,
        found: String,
    },
    #[error("{span}: unexpected end of input")]
    UnexpectedEof { span: Span },
    #[error("{span}: invalid expression")]
    InvalidExpr { span: Span },
    #[error("{span}: {msg}")]
    Custom { span: Span, msg: String },
}

impl ParseError {
    /// The source range the error points at (exact byte, per
    /// `docs/06-parser.md` §10).
    pub fn span(&self) -> Span {
        match self {
            ParseError::ExpectedToken { span, .. }
            | ParseError::UnexpectedEof { span }
            | ParseError::InvalidExpr { span }
            | ParseError::Custom { span, .. } => *span,
        }
    }
}

type PResult<T> = Result<T, ParseError>;

/// Legacy `{…}` dict deprecation warnings (`docs/06-parser.md` §5.5).
///
/// `{` still parses as a dict in value position, so this is a *style* lint, not
/// a syntax error; it becomes a hard error in Zen 3.0. Warnings are deduped by
/// source span so a dict in a hot function is reported once per source site.
#[derive(Debug, Default, Clone)]
pub struct DictWarnings {
    spans: Vec<Span>,
}

impl DictWarnings {
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    pub fn len(&self) -> usize {
        self.spans.len()
    }

    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    /// Record a legacy-brace dict at `span`, once per distinct span.
    pub fn emit_legacy_braces(&mut self, span: Span) {
        if !self.spans.contains(&span) {
            self.spans.push(span);
        }
    }
}

/// Cursor over a fully lexed buffer.
pub struct Parser<'a> {
    lx: Lexed<'a>,
    pos: usize,
    /// Running start of the expression being parsed (`u32::MAX` while none).
    ex_lo: u32,
    /// Running end of the expression being parsed.
    ex_hi: u32,
    /// Legacy-brace dict warnings collected during this parse.
    warnings: DictWarnings,
    /// Recursion depth into `parse_binary`, so only the outermost call seeds
    /// `ex_lo`.
    ex_depth: u32,
    /// Recursion depth into `parse_stmt`, bounding nested blocks/statements.
    stmt_depth: u32,
}

/// Maximum expression nesting (`((((…))))`, nested lists, call arguments).
/// Beyond this the parser reports an error instead of overflowing the native
/// stack, which is not recoverable. The bound is deliberately conservative:
/// unoptimized parser frames are large, so it stays safe even on a default
/// 2 MiB thread stack. Revisit with the parser rewrite.
const MAX_EXPR_DEPTH: u32 = 64;
/// Maximum statement/block nesting (`if`/`while`/`fn` bodies, etc.).
const MAX_STMT_DEPTH: u32 = 64;

impl<'a> Parser<'a> {
    pub fn new(lx: Lexed<'a>) -> Self {
        Self {
            lx,
            pos: 0,
            ex_lo: u32::MAX,
            ex_hi: 0,
            warnings: DictWarnings::default(),
            ex_depth: 0,
            stmt_depth: 0,
        }
    }

    /// Legacy-brace dict warnings collected during the most recent parse
    /// (`docs/06-parser.md` §5.5). Deduped by span.
    pub fn warnings(&self) -> &DictWarnings {
        &self.warnings
    }

    // ── cursor ──────────────────────────────────────────────────────────
    #[inline]
    fn peek(&self) -> TokenKind {
        self.lx
            .toks
            .get(self.pos)
            .map(|t| t.kind)
            .unwrap_or(TokenKind::Eof)
    }

    #[inline]
    fn token(&self) -> Token {
        self.lx
            .toks
            .get(self.pos)
            .copied()
            .unwrap_or_else(|| Token::new(TokenKind::Eof, 0, 0, 0))
    }

    #[inline]
    fn at(&self, k: TokenKind) -> bool {
        self.peek() == k
    }

    /// The kind `n` tokens ahead of the cursor (0 = current). Used for the
    /// one-token `else if` lookahead.
    #[inline]
    fn nth(&self, n: usize) -> TokenKind {
        self.lx
            .toks
            .get(self.pos + n)
            .map(|t| t.kind)
            .unwrap_or(TokenKind::Eof)
    }

    #[inline]
    fn bump(&mut self) -> Token {
        let t = self.token();
        if t.kind != TokenKind::Eof || self.pos + 1 < self.lx.toks.len() {
            self.pos += 1;
        }
        self.ex_hi = self.ex_hi.max(t.hi);
        t
    }

    fn skip_separators(&mut self) {
        loop {
            match self.peek() {
                TokenKind::Newline | TokenKind::Semi => {
                    self.bump();
                }
                _ => return,
            }
        }
    }

    fn expect(&mut self, k: TokenKind, what: &str) -> PResult<Token> {
        if self.at(k) {
            Ok(self.bump())
        } else {
            let t = self.token();
            Err(ParseError::ExpectedToken {
                span: self.span(t),
                expected: what.to_string(),
                found: self.lx.describe(&t),
            })
        }
    }

    fn span(&self, t: Token) -> Span {
        Span::new(self.lx.file, t.lo, t.hi)
    }

    /// The running span of the expression under construction.
    fn ex_span(&self) -> Span {
        let lo = if self.ex_lo == u32::MAX {
            0
        } else {
            self.ex_lo
        };
        Span::new(self.lx.file, lo, self.ex_hi)
    }

    fn eof_err(&self) -> ParseError {
        // Point at the `Eof` token (its `lo`/`hi` sit at the end of the source),
        // not at byte 0, so diagnostics satisfy the "exact byte" rule.
        let t = self.token();
        ParseError::UnexpectedEof {
            span: Span::new(self.lx.file, t.lo, t.hi),
        }
    }

    // ── entry ───────────────────────────────────────────────────────────
    pub fn parse_module(&mut self) -> PResult<Module> {
        let first = self.lx.toks.first().map(|t| (t.lo, t.hi)).unwrap_or((0, 0));
        let mut items = Vec::new();
        self.skip_separators();
        while !self.at(TokenKind::Eof) {
            items.push(self.parse_stmt()?);
            self.skip_separators();
        }
        let span = match self.lx.toks.last() {
            Some(last) => Span::new(self.lx.file, first.0, last.hi),
            None => Span::new(self.lx.file, 0, 0),
        };
        Ok(Module {
            file_id: self.lx.file,
            items,
            span,
        })
    }

    // ── statements ──────────────────────────────────────────────────────
    fn parse_stmt(&mut self) -> PResult<Stmt> {
        self.stmt_depth += 1;
        if self.stmt_depth > MAX_STMT_DEPTH {
            self.stmt_depth -= 1;
            let t = self.token();
            return Err(ParseError::Custom {
                span: self.span(t),
                msg: "statement nesting too deep".into(),
            });
        }
        let result = self.parse_stmt_inner();
        self.stmt_depth -= 1;
        result
    }

    fn parse_stmt_inner(&mut self) -> PResult<Stmt> {
        let t = self.token();
        match t.kind {
            TokenKind::KwLet | TokenKind::KwConst | TokenKind::KwShared | TokenKind::KwVar => {
                self.parse_binding()
            }
            TokenKind::KwReturn => {
                let tok = self.bump();
                let value = if self.at_statement_end() {
                    None
                } else {
                    Some(Box::new(self.parse_expr()?))
                };
                Ok(Stmt::Return {
                    value,
                    span: self.span(tok),
                })
            }
            TokenKind::KwBreak => {
                let tok = self.bump();
                Ok(Stmt::Break {
                    span: self.span(tok),
                })
            }
            TokenKind::KwContinue => {
                let tok = self.bump();
                Ok(Stmt::Continue {
                    span: self.span(tok),
                })
            }
            TokenKind::KwFn => self.parse_function(),
            TokenKind::KwIf => self.parse_if(),
            TokenKind::KwWhile => self.parse_while(),
            TokenKind::KwFor => self.parse_for(),
            TokenKind::KwPrint => self.parse_print(),
            // `match`/`when` are *expressions* (`docs/control-flow.md:141`), so
            // they reach `parse_expr_stmt` and become a bare `Stmt::Expr` whose
            // value is discarded — exactly like any other expression statement.
            TokenKind::KwClass => self.parse_class(),
            TokenKind::KwImport => self.parse_import(),
            TokenKind::KwPymport => self.parse_pymport(),
            TokenKind::KwFrom => self.parse_from_import(),
            TokenKind::KwInclude => self.parse_include(),
            TokenKind::KwTry => self.parse_try(),
            TokenKind::KwThrow => self.parse_throw(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn at_statement_end(&self) -> bool {
        matches!(
            self.peek(),
            TokenKind::Semi
                | TokenKind::Newline
                | TokenKind::RBrace
                | TokenKind::Eof
                | TokenKind::RBracket
                | TokenKind::RParen
        )
    }

    fn parse_binding(&mut self) -> PResult<Stmt> {
        let kw = self.bump();
        // `shared n = 0` is a `let` whose storage the M4 compiler places in the
        // shared table (§2.6) — the parser records the binding kind and the
        // enforcement happens at compile time, not here.
        if kw.kind == TokenKind::KwShared {
            let name_tok = self.expect(TokenKind::Ident, "identifier")?;
            let name = self.lx.ident(&name_tok).to_string();
            self.expect(TokenKind::Assign, "`=` in a `shared` binding")?;
            let value = self.parse_expr()?;
            let span = self.span(kw).merge(self.ex_span());
            return Ok(Stmt::Shared {
                name,
                value: Box::new(value),
                span,
            });
        }
        let name_tok = self.expect(TokenKind::Ident, "identifier")?;
        let name = self.lx.ident(&name_tok).to_string();
        let value = if self.at(TokenKind::Assign) {
            self.bump();
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let value = value.unwrap_or_else(|| Box::new(Expr::Literal(Literal::Nil)));
        let end = self.token();
        let span = self.span(kw).merge(self.span(end));
        Ok(match kw.kind {
            TokenKind::KwConst => Stmt::Const { name, value, span },
            _ => Stmt::Let {
                name,
                value,
                mutable: true,
                span,
            },
        })
    }

    fn parse_function(&mut self) -> PResult<Stmt> {
        let kw = self.bump();
        let name_tok = self.expect(TokenKind::Ident, "function name")?;
        let name = self.lx.ident(&name_tok).to_string();
        let params = self.parse_params()?;
        let body = self.parse_block()?;
        let span = self.span(kw).merge(body.span);
        Ok(Stmt::Function {
            name,
            params,
            body,
            span,
        })
    }

    fn parse_params(&mut self) -> PResult<Vec<Param>> {
        self.expect(TokenKind::LParen, "`(`")?;
        let mut params = Vec::new();
        while !self.at(TokenKind::RParen) {
            if self.at(TokenKind::Eof) {
                return Err(self.eof_err());
            }
            let t = self.expect(TokenKind::Ident, "parameter name")?;
            let name = self.lx.ident(&t).to_string();
            let (default, span) = if self.at(TokenKind::Assign) {
                self.bump();
                let v = self.parse_expr()?;
                (Some(Box::new(v)), self.ex_span())
            } else {
                (None, self.span(t))
            };
            params.push(Param {
                name,
                default,
                span,
            });
            if !self.at(TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        self.expect(TokenKind::RParen, "`)`")?;
        Ok(params)
    }

    fn parse_block(&mut self) -> PResult<Block> {
        let open = self.expect(TokenKind::LBrace, "`{`")?;
        let mut stmts = Vec::new();
        self.skip_separators();
        while !self.at(TokenKind::RBrace) {
            if self.at(TokenKind::Eof) {
                return Err(self.eof_err());
            }
            stmts.push(self.parse_stmt()?);
            self.skip_separators();
        }
        let close = self.expect(TokenKind::RBrace, "`}`")?;
        Ok(Block {
            stmts,
            span: Span::new(self.lx.file, open.lo, close.hi),
        })
    }

    fn parse_if(&mut self) -> PResult<Stmt> {
        let kw = self.bump();
        let cond = self.parse_expr()?;
        let then_branch = self.parse_block()?;
        let mut elifs = Vec::new();
        let mut else_branch = None;
        self.skip_separators();
        // `elif` and `else if` are the same arm chain; only `else if` needs a
        // one-token lookahead to tell it apart from a plain `else`.
        loop {
            self.skip_separators();
            if self.at(TokenKind::KwElif) {
                self.bump();
            } else if self.at(TokenKind::KwElse) && self.nth(1) == TokenKind::KwIf {
                self.bump(); // `else`
                self.bump(); // `if`
            } else {
                break;
            }
            let c = self.parse_expr()?;
            let b = self.parse_block()?;
            elifs.push((c, b));
        }
        if self.at(TokenKind::KwElse) {
            self.bump();
            self.skip_separators();
            else_branch = Some(self.parse_block()?);
        }
        let span = self.span(kw).merge(then_branch.span);
        Ok(Stmt::If {
            cond: Box::new(cond),
            then_branch,
            elifs,
            else_branch,
            span,
        })
    }

    fn parse_while(&mut self) -> PResult<Stmt> {
        let kw = self.bump();
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        let span = self.span(kw).merge(body.span);
        Ok(Stmt::While {
            cond: Box::new(cond),
            body,
            span,
        })
    }

    fn parse_for(&mut self) -> PResult<Stmt> {
        let kw = self.bump();
        let first = self.expect(TokenKind::Ident, "loop variable")?;
        let mut vars = vec![self.lx.ident(&first).to_string()];
        while self.at(TokenKind::Comma) {
            self.bump();
            let t = self.expect(TokenKind::Ident, "loop variable")?;
            vars.push(self.lx.ident(&t).to_string());
        }
        self.expect(TokenKind::KwIn, "`in`")?;
        let iter = self.parse_expr()?;
        let body = self.parse_block()?;
        let span = self.span(kw).merge(body.span);
        Ok(Stmt::For {
            vars,
            iter: Box::new(iter),
            body,
            span,
        })
    }

    fn parse_class(&mut self) -> PResult<Stmt> {
        let kw = self.expect(TokenKind::KwClass, "`class`")?;
        let name_tok = self.expect(TokenKind::Ident, "class name")?;
        let name = self.lx.ident(&name_tok).to_string();
        let parent = if self.at(TokenKind::KwExtends) {
            self.bump();
            let p = self.expect(TokenKind::Ident, "parent class name")?;
            Some(self.lx.ident(&p).to_string())
        } else {
            None
        };
        self.expect(TokenKind::LBrace, "`{` to open the class body")?;
        let mut members = Vec::new();
        self.skip_separators();
        while !self.at(TokenKind::RBrace) {
            if self.at(TokenKind::Eof) {
                return Err(self.eof_err());
            }
            members.push(self.parse_class_member()?);
            self.skip_separators();
        }
        let close = self.expect(TokenKind::RBrace, "`}` to close the class body")?;
        let span = self.span(kw).merge(self.span(close));
        Ok(Stmt::Class {
            name,
            parent,
            members,
            span,
        })
    }

    /// One class-body member: `func name(params) { … }`, `var name [= default]`,
    /// or the `let`/`const`/`shared` spellings of a field. `pub` is accepted and
    /// ignored here (visibility is a compile-time concern, not a parse one).
    fn parse_class_member(&mut self) -> PResult<ClassMember> {
        if self.at(TokenKind::KwPub) {
            self.bump();
        }
        if self.at(TokenKind::KwFn) {
            let kw = self.bump();
            let name_tok = self.expect(TokenKind::Ident, "method name")?;
            let name = self.lx.ident(&name_tok).to_string();
            let params = self.parse_params()?;
            let body = self.parse_block()?;
            let span = self.span(kw).merge(body.span);
            let kind = if name == "init" {
                ClassMemberKind::Init
            } else {
                ClassMemberKind::Method
            };
            return Ok(ClassMember {
                kind,
                name,
                params,
                body,
                default: None,
                span,
            });
        }
        if self.at(TokenKind::KwVar)
            || self.at(TokenKind::KwLet)
            || self.at(TokenKind::KwConst)
            || self.at(TokenKind::KwShared)
        {
            let kw = self.bump();
            let name_tok = self.expect(TokenKind::Ident, "field name")?;
            let name = self.lx.ident(&name_tok).to_string();
            let default = if self.at(TokenKind::Assign) {
                self.bump();
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            let span = self.span(kw).merge(self.ex_span());
            let body = Block {
                stmts: Vec::new(),
                span,
            };
            return Ok(ClassMember {
                kind: ClassMemberKind::Field,
                name,
                params: Vec::new(),
                body,
                default,
                span,
            });
        }
        let t = self.token();
        Err(ParseError::ExpectedToken {
            span: self.span(t),
            expected: "a class member (`func`, `var`, `let` or `const`)".to_string(),
            found: self.lx.describe(&t),
        })
    }

    // ── imports (§7) ────────────────────────────────────────────────────

    /// `import a.b`, `import a.b as ab`, `import a, b.c as d`.
    ///
    /// The parser produces a **path**, never a file (§7.2): no segment is
    /// stripped and no candidate is probed here. `import pkg.sub.mod` keeps all
    /// three segments, because the binding is a namespace chain and the
    /// resolver — not this function — decides what that resolves to.
    fn parse_import(&mut self) -> PResult<Stmt> {
        let kw = self.expect(TokenKind::KwImport, "`import`")?;
        let mut specs = Vec::new();
        loop {
            let lo = self.token().lo;
            let path = self.parse_module_path()?;
            let alias = if self.eat(TokenKind::KwAs) {
                let t = self.expect(TokenKind::Ident, "an alias after `as`")?;
                Some(self.lx.ident(&t).to_string())
            } else {
                None
            };
            let span = self.span_of(lo, self.ex_hi);
            specs.push(ImportSpec { path, alias, span });
            if !self.eat(TokenKind::Comma) {
                break;
            }
            self.skip_separators();
        }
        let span = self.span(kw).merge(self.ex_span());
        Ok(Stmt::Import { specs, span })
    }

    /// `pymport pkg.sub [as alias]` — a foreign Python import. Single dotted
    /// path plus optional alias (no comma lists: one foreign boundary per
    /// statement keeps the bootstrap scanner trivial).
    fn parse_pymport(&mut self) -> PResult<Stmt> {
        let kw = self.expect(TokenKind::KwPymport, "`pymport`")?;
        let lo = self.token().lo;
        let path = self.parse_module_path()?;
        let alias = if self.eat(TokenKind::KwAs) {
            let t = self.expect(TokenKind::Ident, "an alias after `as`")?;
            Some(self.lx.ident(&t).to_string())
        } else {
            None
        };
        let span = self.span_of(kw.lo.min(lo), self.ex_hi);
        Ok(Stmt::Pymport {
            path,
            alias,
            span,
        })
    }

    /// A dotted module path: `a`, `a.b`, `a.b.c`.
    fn parse_module_path(&mut self) -> PResult<Vec<String>> {        let first = self.expect(TokenKind::Ident, "a module name")?;
        let mut path = vec![self.lx.ident(&first).to_string()];
        while self.at(TokenKind::Dot) {
            self.bump();
            let seg = self.expect(TokenKind::Ident, "a module name after `.`")?;
            path.push(self.lx.ident(&seg).to_string());
        }
        Ok(path)
    }

    /// `from a.b import x, y as z` / `from a.b import *`.
    fn parse_from_import(&mut self) -> PResult<Stmt> {
        let kw = self.expect(TokenKind::KwFrom, "`from`")?;
        let path = self.parse_module_path()?;
        self.expect(TokenKind::KwImport, "`import` after a `from` module")?;
        let lo = self.token().lo;
        let items = if self.eat(TokenKind::Star) {
            ImportItems::All
        } else {
            let mut names = Vec::new();
            loop {
                let nlo = self.token().lo;
                let t = self.expect(TokenKind::Ident, "a name to import")?;
                let alias = if self.eat(TokenKind::KwAs) {
                    let a = self.expect(TokenKind::Ident, "an alias after `as`")?;
                    Some(self.lx.ident(&a).to_string())
                } else {
                    None
                };
                let nspan = self.span_of(nlo, self.ex_hi);
                names.push(ImportName {
                    name: self.lx.ident(&t).to_string(),
                    alias,
                    span: nspan,
                });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
                self.skip_separators();
            }
            ImportItems::Names(names)
        };
        let span = self.span_of(kw.lo, self.ex_hi.max(lo));
        Ok(Stmt::FromImport { path, items, span })
    }

    /// `include "./setup.z"` / `load "./x.z"` — executes another file in the
    /// *current* scope, so there is no binding and no alias.
    fn parse_include(&mut self) -> PResult<Stmt> {
        let kw = self.expect(TokenKind::KwInclude, "`include`")?;
        let t = self.expect(TokenKind::Str, "a file path string after `include`")?;
        let path = self.lx.str_text(&t).to_string();
        let span = self.span(kw).merge(self.span(t));
        Ok(Stmt::Include { path, span })
    }

    // ── errors ──────────────────────────────────────────────────────────

    /// `try { … } catch [T] [as e] { … } … finally { … }`
    ///
    /// `catch_type catch_bind block` is ambiguous in a way only the *shape* of
    /// the next name resolves (`grammar.md:151-154`): `catch e { }` is a typed
    /// catch when `e` is a registered error class and a binding otherwise, but
    /// the parser cannot know the class registry, so it records the name once and
    /// lets the resolver decide. That is why `CatchClause` carries
    /// `typ: Option<String>` rather than a resolved flag.
    fn parse_try(&mut self) -> PResult<Stmt> {
        let kw = self.expect(TokenKind::KwTry, "`try`")?;
        let body = self.parse_block()?;
        let mut catches = Vec::new();
        let mut finally = None;
        loop {
            self.skip_separators();
            if !self.at(TokenKind::KwCatch) {
                break;
            }
            let ckw = self.bump();
            let (typ, binding) = self.parse_catch_head()?;
            let cbody = self.parse_block()?;
            let span = self.span(ckw).merge(cbody.span);
            catches.push(CatchClause {
                typ,
                binding,
                body: cbody,
                span,
            });
        }
        if self.at(TokenKind::KwFinally) {
            self.bump();
            finally = Some(self.parse_block()?);
        }
        let span = self.span(kw).merge(self.ex_span());
        Ok(Stmt::Try {
            body,
            catches,
            finally,
            span,
        })
    }

    /// `catch_type? catch_bind?` — both parts optional, and a bare `catch { }`
    /// catches everything with neither.
    fn parse_catch_head(&mut self) -> PResult<(Option<String>, Option<String>)> {
        // `as name` is unambiguous: the keyword makes it a binding.
        if self.at(TokenKind::KwAs) {
            self.bump();
            let t = self.expect(TokenKind::Ident, "a name after `as`")?;
            return Ok((None, Some(self.lx.ident(&t).to_string())));
        }
        if !self.at(TokenKind::Ident) {
            return Ok((None, None));
        }
        // A qualified name: `catch TypeError { }` or `catch net.HttpError { }`.
        let first = self.bump();
        let mut name = self.lx.ident(&first).to_string();
        let mut qualified = false;
        while self.at(TokenKind::Dot) {
            qualified = true;
            self.bump();
            let seg = self.expect(TokenKind::Ident, "a name after `.`")?;
            name.push('.');
            name.push_str(self.lx.ident(&seg));
        }
        // `catch Type as name` — the `as` makes the second name the binding.
        if self.at(TokenKind::KwAs) {
            self.bump();
            let b = self.expect(TokenKind::Ident, "a name after `as`")?;
            return Ok((Some(name), Some(self.lx.ident(&b).to_string())));
        }
        // A following bare name is the binding, not part of the type.
        if self.at(TokenKind::Ident) {
            let b = self.bump();
            return Ok((Some(name), Some(self.lx.ident(&b).to_string())));
        }
        // `catch e { … }` is a binding, not a typed catch, unless the name is
        // qualified or looks like an error class (conventionally capitalized);
        // see `docs/06-parser.md:347`. That mirrors `catch as e`.
        let looks_like_class = qualified || name.chars().next().is_some_and(|c| !c.is_lowercase());
        if looks_like_class {
            Ok((Some(name), None))
        } else {
            Ok((None, Some(name)))
        }
    }

    /// `throw expr` — the value is not optional, unlike `return`.
    fn parse_throw(&mut self) -> PResult<Stmt> {
        let kw = self.expect(TokenKind::KwThrow, "`throw`")?;
        let value = self.parse_expr()?;
        let span = self.span(kw).merge(self.ex_span());
        Ok(Stmt::Throw {
            value: Box::new(value),
            span,
        })
    }

    fn parse_print(&mut self) -> PResult<Stmt> {
        let kw = self.bump();
        let mut args = Vec::new();
        if self.at(TokenKind::LParen) {
            self.bump();
            if !self.at(TokenKind::RParen) {
                loop {
                    args.push(self.parse_expr()?);
                    if self.at(TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RParen, "`)`")?;
        } else if !self.at_statement_end() {
            // Legacy statement form: `print "FAIL: " + what` (no parens).
            args.push(self.parse_expr()?);
        }
        let end = self.token();
        let span = self.span(kw).merge(self.span(end));
        Ok(Stmt::Print {
            args,
            sep: None,
            end: None,
            span,
        })
    }

    fn parse_expr_stmt(&mut self) -> PResult<Stmt> {
        let expr = self.parse_expr()?;
        let t = self.token();
        if t.kind == TokenKind::Question {
            self.bump();
            let then_branch = self.parse_expr()?;
            self.expect(TokenKind::Colon, "`:`")?;
            let else_branch = self.parse_expr()?;
            let span = self.ex_span();
            return Ok(Stmt::Expr(Box::new(Expr::Ternary {
                cond: Box::new(expr),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
                span,
            })));
        }
        if let Some(op) = assign_op(t.kind) {
            self.bump();
            let value = self.parse_expr()?;
            let span = self.ex_span();
            return Ok(Stmt::Assign {
                target: Box::new(expr),
                op,
                value: Box::new(value),
                span,
            });
        }
        Ok(Stmt::Expr(Box::new(expr)))
    }

    // ── expressions ─────────────────────────────────────────────────────
    pub fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_binary(0)
    }

    fn parse_binary(&mut self, min_prec: u8) -> PResult<Expr> {
        if self.ex_depth == 0 {
            let t = self.token();
            self.ex_lo = t.lo;
            self.ex_hi = t.hi;
        }
        self.ex_depth += 1;
        if self.ex_depth > MAX_EXPR_DEPTH {
            self.ex_depth -= 1;
            return Err(ParseError::Custom {
                span: self.ex_span(),
                msg: "expression nesting too deep".into(),
            });
        }
        let mut left = self.parse_unary()?;
        loop {
            let t = self.token();
            let Some((prec, right_assoc)) = op_prec(t.kind) else {
                break;
            };
            if prec < min_prec {
                break;
            }

            // Special handling for ternary ? : (bp 1, right-assoc, three operands).
            if t.kind == TokenKind::Question {
                // (ternary handling unchanged)
                self.bump(); // consume '?'
                             // Parse then-branch at bp 1 (same precedence, so ranges etc. bind).
                let then_branch = self.parse_binary(1)?;
                self.expect(TokenKind::Colon, "`:`")?;
                // Parse else-branch at bp 0 (allows nested ternaries).
                let else_branch = self.parse_binary(0)?;
                let span = self.ex_span();
                left = Expr::Ternary {
                    cond: Box::new(left),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(else_branch),
                    span,
                };
                continue;
            }

            // Special handling for nullish coalescing ?? (bp 8, left-assoc, two operands).
            if t.kind == TokenKind::Nullish {
                let nullish_span = self.span(t);
                self.bump(); // consume '??'
                             // Parse RHS at bp 9 (one level tighter than ??, so it binds tighter).
                             // Since ?? is left-assoc, next_min = prec + 1 = 9.
                let right = self.parse_binary(9)?;
                let span = self.ex_span().merge(nullish_span);
                left = Expr::NullCoalesce {
                    left: Box::new(left),
                    right: Box::new(right),
                    span,
                };
                continue;
            }

            // Special handling for chainable comparisons (bp 20).
            // When we see a comparison operator, check if the *next* token
            // after parsing its RHS is also a comparison. If so, build a Chain.
            let is_chainable_comparison = prec == 20
                && matches!(
                    t.kind,
                    TokenKind::Eq
                        | TokenKind::Ne
                        | TokenKind::StrictEq
                        | TokenKind::StrictNe
                        | TokenKind::KwIn
                        | TokenKind::KwIs
                        | TokenKind::Lt
                        | TokenKind::Le
                        | TokenKind::Gt
                        | TokenKind::Ge
                );

            let op = binop(t.kind).ok_or_else(|| ParseError::Custom {
                span: self.span(t),
                msg: format!("{} is not a binary operator", self.lx.describe(&t)),
            })?;
            self.bump();

            if is_chainable_comparison {
                // Parse first RHS at next_min = bp + 1 (left-associative).
                let next_min = prec + 1;
                let mut operands = vec![left];
                let mut ops = Vec::new();
                let mut current = self.parse_binary(next_min)?;
                operands.push(current);
                ops.push(op);

                // Continue chain while next token is also a comparison at bp 20.
                loop {
                    let t2 = self.token();
                    let Some((p2, _)) = op_prec(t2.kind) else {
                        break;
                    };
                    if p2 != prec {
                        break;
                    }
                    let is_comp = matches!(
                        t2.kind,
                        TokenKind::Eq
                            | TokenKind::Ne
                            | TokenKind::StrictEq
                            | TokenKind::StrictNe
                            | TokenKind::KwIn
                            | TokenKind::KwIs
                            | TokenKind::Lt
                            | TokenKind::Le
                            | TokenKind::Gt
                            | TokenKind::Ge
                    );
                    if !is_comp {
                        break;
                    }
                    let op2 = binop(t2.kind).unwrap();
                    self.bump();
                    current = self.parse_binary(next_min)?;
                    operands.push(current);
                    ops.push(op2);
                }
                let span = self.ex_span();
                left = Expr::Chain {
                    ops,
                    operands,
                    span,
                };
            } else {
                // Normal binary operator (including ternary, nullish, range, etc.).
                let next_min = if right_assoc { prec } else { prec + 1 };
                let right = self.parse_binary(next_min)?;
                let span = self.ex_span();
                left = Expr::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                    span,
                };
            }
        }
        // Python-style ternary: `a if cond else b` (docs/06-parser.md §4.4).
        //
        // Handled *above* the infix loop, never inside it, so "stop at a
        // top-level ternary" is a property of the call tree rather than of the
        // loop condition — the same reason `? :` is not an `op_prec` entry.
        // `min_prec == 0` means the caller asked for a whole expression; a
        // tighter sub-expression (the left operand of `*`, say) must not
        // swallow the `if`.
        //
        // The condition is read at BP_RANGE so it cannot itself contain a
        // top-level `? :` (RULING P2), which matches the restriction Python
        // puts on the same construct. The `else` arm is read at 0, so
        // `a if b else c if d else e` chains to the right, as every reader
        // expects.
        if min_prec == 0 && self.at(TokenKind::KwIf) {
            self.bump();
            let cond = self.parse_binary(BP_RANGE)?;
            self.expect(
                TokenKind::KwElse,
                "after the condition of an `a if b else c` expression",
            )?;
            let otherwise = self.parse_binary(0)?;
            let span = self.ex_span();
            self.ex_depth -= 1;
            return Ok(Expr::Ternary {
                cond: Box::new(cond),
                then_branch: Box::new(left),
                else_branch: Box::new(otherwise),
                span,
            });
        }
        self.ex_depth -= 1;
        Ok(left)
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        let t = self.token();
        // `if` in *expression* position: `if c { … } elif c2 { … } else { … }`.
        //
        // Strictly, §5.4 says `Expr::IfExpr`'s arms are `Block`s and it is "only
        // reachable from `parse_stmt`", which would make `x = if c { … } else
        // { … }` an error (the `{` reading as a dict). We accept it anyway as a
        // deliberate lenient extension until M8 lands the `then`/`else` spelling:
        // the `if` keyword already commits the parse, so `{` here can only be a
        // block, and this is the form real code is being written in today.
        //
        // The M8 spelling, `if c then a else b`, is still M8.
        if t.kind == TokenKind::KwIf {
            let kw = self.bump();
            let mut arms = Vec::new();
            let cond = self.parse_expr()?;
            let block = self.parse_block()?;
            let first_end = block.span;
            arms.push((cond, block));
            self.skip_separators();
            // `elif` and `else if` fold into the same arm chain.
            while self.at(TokenKind::KwElif) {
                self.bump();
                let c = self.parse_expr()?;
                let b = self.parse_block()?;
                arms.push((c, b));
                self.skip_separators();
            }
            let mut otherwise = None;
            if self.at(TokenKind::KwElse) {
                self.bump();
                self.skip_separators();
                otherwise = Some(self.parse_block()?);
            }
            let tail = otherwise.as_ref().map_or(first_end, |b| b.span);
            let span = self.span(kw).merge(tail);
            return Ok(Expr::IfExpr {
                arms,
                otherwise,
                span,
            });
        }
        let op = match t.kind {
            TokenKind::Bang | TokenKind::KwNot => Some(UnaryOp::Not),
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Tilde => Some(UnaryOp::BitNot),
            TokenKind::KwTypeof => Some(UnaryOp::TypeOf),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let operand = self.parse_unary()?;
            let span = self.ex_span();
            Ok(Expr::Unary {
                op,
                operand: Box::new(operand),
                span,
            })
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut e = self.parse_primary()?;
        loop {
            match self.peek() {
                TokenKind::LParen => {
                    let args = self.parse_call_args()?;
                    let span = self.ex_span();
                    e = Expr::Call {
                        callee: Box::new(e),
                        args,
                        span,
                    };
                }
                TokenKind::LBracket => {
                    self.bump();
                    // `s[:b]`, `s[a:]`, `s[:]` — an omitted start.
                    let start = if self.at(TokenKind::Colon)
                        || self.at(TokenKind::Comma)
                        || self.at(TokenKind::RBracket)
                    {
                        None
                    } else {
                        Some(Box::new(self.parse_expr()?))
                    };
                    // `:` or `,` marks a slice; otherwise it is a plain index.
                    if self.at(TokenKind::Colon) || self.at(TokenKind::Comma) {
                        self.bump();
                        let end = if self.at(TokenKind::RBracket) {
                            None
                        } else {
                            Some(Box::new(self.parse_expr()?))
                        };
                        self.expect(TokenKind::RBracket, "`]`")?;
                        let span = self.ex_span();
                        e = Expr::Slice {
                            object: Box::new(e),
                            start,
                            end,
                            span,
                        };
                    } else {
                        let index = start.ok_or_else(|| ParseError::InvalidExpr {
                            span: self.span(self.token()),
                        })?;
                        self.expect(TokenKind::RBracket, "`]`")?;
                        let span = self.ex_span();
                        e = Expr::Index {
                            object: Box::new(e),
                            index,
                            span,
                        };
                    }
                }
                TokenKind::Dot | TokenKind::SafeDot => {
                    let safe = self.peek() == TokenKind::SafeDot;
                    self.bump();
                    let field_tok = self.expect(TokenKind::Ident, "field name")?;
                    let field = self.lx.ident(&field_tok).to_string();
                    if self.at(TokenKind::LParen) {
                        let args = self.parse_call_args()?;
                        let span = self.ex_span();
                        e = Expr::MethodCall {
                            receiver: Box::new(e),
                            method: field,
                            args,
                            span,
                        };
                    } else if safe {
                        let span = self.ex_span();
                        e = Expr::OptionalField {
                            object: Box::new(e),
                            field,
                            span,
                        };
                    } else {
                        let span = self.ex_span();
                        e = Expr::Field {
                            object: Box::new(e),
                            field,
                            span,
                        };
                    }
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn parse_call_args(&mut self) -> PResult<Vec<Expr>> {
        self.expect(TokenKind::LParen, "`(`")?;
        let mut args = Vec::new();
        if self.at(TokenKind::RParen) {
            self.bump();
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            if self.at(TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
            if self.at(TokenKind::RParen) {
                break;
            }
        }
        self.expect(TokenKind::RParen, "`)`")?;
        Ok(args)
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let t = self.token();
        match t.kind {
            TokenKind::KwNil => {
                self.bump();
                Ok(Expr::Literal(Literal::Nil))
            }
            TokenKind::KwTrue => {
                self.bump();
                Ok(Expr::Literal(Literal::Bool(true)))
            }
            TokenKind::KwFalse => {
                self.bump();
                Ok(Expr::Literal(Literal::Bool(false)))
            }
            TokenKind::Int | TokenKind::Float => {
                self.bump();
                let lit = self.lx.num(&t).unwrap_or(NumLit::Int(0));
                let v = match lit {
                    NumLit::Int(i) => Literal::Int(i),
                    NumLit::Float(f) => Literal::Float(f),
                };
                Ok(Expr::Literal(v))
            }
            TokenKind::Str => {
                self.bump();
                Ok(Expr::Literal(Literal::String(
                    self.lx.str_text(&t).to_string(),
                )))
            }
            TokenKind::Interp => {
                self.bump();
                match self.lx.interp(&t) {
                    Some(interp) => {
                        let mut parts = Vec::new();
                        for part in &interp.parts {
                            match part {
                                InterpPart::Text(r) => {
                                    parts.push(StringPart::Text(r.as_str().to_string()))
                                }
                                InterpPart::Expr(idx) => {
                                    let sub = match interp.subs.get(*idx as usize) {
                                        Some(s) => s,
                                        None => continue,
                                    };
                                    let mut sp = Parser::new(sub.clone());
                                    let e = sp
                                        .parse_expr()
                                        .unwrap_or_else(|_| Expr::Literal(Literal::Nil));
                                    parts.push(StringPart::Interpolation(Box::new(e)));
                                }
                            }
                        }
                        Ok(Expr::InterpolatedString(parts))
                    }
                    None => Err(ParseError::Custom {
                        span: self.span(t),
                        msg: "invalid interpolated string".to_string(),
                    }),
                }
            }
            TokenKind::Ident => {
                let span = self.span(t);
                self.bump();
                Ok(Expr::Ident {
                    name: self.lx.ident(&t).to_string(),
                    span,
                })
            }
            TokenKind::KwLambda => self.parse_lambda(),
            // `func(x) { … }` — an anonymous function *expression*. A named
            // `func name(…) { … }` is a statement and never reaches here.
            TokenKind::KwFn => self.parse_fn_expr(),
            // `new Name(args)` — class instantiation.
            TokenKind::KwNew => self.parse_new(),
            // `super(args)` / `super.method(args)` — only meaningful inside a
            // method body; legality is enforced by the evaluator/compiler.
            TokenKind::KwSuper => self.parse_super(),
            // `match`/`when` are expressions, so they are reachable from any
            // expression position — a `return`, an argument, a binding's value.
            TokenKind::KwMatch => self.parse_match(),
            TokenKind::KwWhen => self.parse_when(),
            TokenKind::LParen => self.parse_arrow_or_group(),
            TokenKind::LBracket => self.parse_list(),
            // A bare `{` in *expression* position is a legacy dict literal
            // (`docs/06-parser.md` §5.3): the statement path turns a leading
            // `{` into a block, so reaching here means the dict reading. It
            // still parses, but records a `{…}` deprecation warning (§5.5).
            TokenKind::LBrace => {
                let lo = self.token().lo;
                let dict = self.parse_dict()?;
                self.warnings
                    .emit_legacy_braces(self.span_of(lo, self.ex_hi));
                Ok(dict)
            }
            TokenKind::DictStart => self.parse_dict(),
            TokenKind::TupleStart => self.parse_tuple(),
            TokenKind::SetStart => self.parse_set(),
            _ => Err(ParseError::InvalidExpr { span: self.span(t) }),
        }
    }

    /// `new Name(args)` / `new module.Name(args)` — class instantiation.
    fn parse_new(&mut self) -> PResult<Expr> {
        let kw = self.bump(); // `new`
        let first = self.expect(TokenKind::Ident, "a class name after `new`")?;
        let mut class = self.lx.ident(&first).to_string();
        while self.at(TokenKind::Dot) {
            self.bump();
            let seg = self.expect(TokenKind::Ident, "a name after `.`")?;
            class.push('.');
            class.push_str(self.lx.ident(&seg));
        }
        let args = if self.at(TokenKind::LParen) {
            self.parse_call_args()?
        } else {
            Vec::new()
        };
        let span = self.span(kw).merge(self.ex_span());
        Ok(Expr::New { class, args, span })
    }

    /// `super(args)` or `super.method(args)`. The parser produces a dedicated
    /// node so the evaluator/compiler can resolve the parent class from the
    /// enclosing method's `super` binding.
    fn parse_super(&mut self) -> PResult<Expr> {
        let kw = self.bump(); // `super`
        if self.at(TokenKind::Dot) {
            self.bump();
            let m = self.expect(TokenKind::Ident, "a method name after `super.`")?;
            let method = self.lx.ident(&m).to_string();
            let args = if self.at(TokenKind::LParen) {
                self.parse_call_args()?
            } else {
                Vec::new()
            };
            let span = self.span(kw).merge(self.ex_span());
            return Ok(Expr::SuperMethod { method, args, span });
        }
        let args = if self.at(TokenKind::LParen) {
            self.parse_call_args()?
        } else {
            Vec::new()
        };
        let span = self.span(kw).merge(self.ex_span());
        Ok(Expr::SuperCall { args, span })
    }

    /// `func(x) { … }` — an anonymous function. Unlike `lambda`, the body is a
    /// **block** (§2.2 line 155), so the `{` opens a body slot and is not a
    /// dict.
    fn parse_fn_expr(&mut self) -> PResult<Expr> {
        let kw = self.bump(); // `func`/`fn`/`function`
        let params = self.parse_params()?;
        let body = self.parse_block()?;
        let span = self.span(kw).merge(body.span);
        Ok(Expr::FnExpr { params, body, span })
    }

    /// `lambda x, y = 1: body`, plus the legacy `lambda(x, y) { … }` form whose
    /// parenthesised params and `{`-block body make it an anonymous function.
    fn parse_lambda(&mut self) -> PResult<Expr> {
        let kw = self.bump(); // `lambda`
        let params = if self.at(TokenKind::LParen) {
            self.parse_params()?
        } else {
            let mut params = Vec::new();
            if !self.at(TokenKind::Colon) && !self.at(TokenKind::LBrace) {
                loop {
                    let t = self.expect(TokenKind::Ident, "parameter name")?;
                    let name = self.lx.ident(&t).to_string();
                    let (default, span) = if self.at(TokenKind::Assign) {
                        self.bump();
                        let v = self.parse_expr()?;
                        (Some(Box::new(v)), self.ex_span())
                    } else {
                        (None, self.span(t))
                    };
                    params.push(Param {
                        name,
                        default,
                        span,
                    });
                    if !self.at(TokenKind::Comma) {
                        break;
                    }
                    self.bump();
                }
            }
            params
        };
        // Legacy block body: `lambda() { … }`. The new spec spells this
        // `func() { … }`, but the `{` after a lambda's params opens a body slot
        // here, so it is a block and not a dict.
        if self.at(TokenKind::LBrace) {
            let body = self.parse_block()?;
            let span = self.span(kw).merge(body.span);
            return Ok(Expr::FnExpr { params, body, span });
        }
        self.expect(TokenKind::Colon, "`:`")?;
        let body = self.parse_expr()?;
        let span = self.span(kw).merge(self.ex_span());
        Ok(Expr::Lambda {
            params,
            body: Box::new(body),
            span,
        })
    }

    /// `(params) => body` when the arrow follows, else a parenthesised
    /// expression or tuple. The param list is parsed speculatively because
    /// `(x)` is a valid group on its own; only the `=>` decides.
    fn parse_arrow_or_group(&mut self) -> PResult<Expr> {
        let save = (self.pos, self.ex_lo, self.ex_hi);
        if let Ok(params) = self.parse_params() {
            if self.at(TokenKind::ThinArrow) {
                let open = self.lx.toks[save.0];
                self.bump(); // `=>`
                             // The body is expression position, but the corpus writes block
                             // bodies (`(x) => { … }`) that auto-return, so a `{` here opens
                             // a **block** (lenient over the strict dict reading in §5.2).
                let body = if self.at(TokenKind::LBrace) {
                    BlockOrExpr::Block(self.parse_block()?)
                } else {
                    BlockOrExpr::Expr(Box::new(self.parse_binary(BP_RANGE)?))
                };
                let span = Span::new(self.lx.file, open.lo, self.ex_hi);
                return Ok(Expr::ArrowFn { params, body, span });
            }
        }
        self.pos = save.0;
        self.ex_lo = save.1;
        self.ex_hi = save.2;
        self.parse_group()
    }

    fn parse_group(&mut self) -> PResult<Expr> {
        self.bump(); // `(`
                     // An empty group or trailing comma makes a tuple.
        let mut elems = Vec::new();
        if self.at(TokenKind::RParen) {
            self.bump();
            return Ok(Expr::Tuple(elems));
        }
        loop {
            elems.push(self.parse_expr()?);
            if self.at(TokenKind::Comma) {
                self.bump();
                if self.at(TokenKind::RParen) {
                    self.bump();
                    return Ok(Expr::Tuple(elems));
                }
            } else {
                break;
            }
        }
        self.expect(TokenKind::RParen, "`)`")?;
        if elems.len() == 1 {
            Ok(elems.pop().unwrap_or_else(|| Expr::Literal(Literal::Nil)))
        } else {
            Ok(Expr::Tuple(elems))
        }
    }

    /// `elem (, elem)* ,?` until `close`, tolerating separators and a trailing
    /// comma. The opener must already be consumed by the caller.
    fn parse_elem_list(&mut self, close: TokenKind, what: &str) -> PResult<Vec<Expr>> {
        let mut elems = Vec::new();
        loop {
            self.skip_separators();
            if self.at(close) {
                break;
            }
            if self.at(TokenKind::Eof) {
                return Err(self.eof_err());
            }
            elems.push(self.parse_expr()?);
            self.skip_separators();
            if self.at(TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        self.skip_separators();
        self.expect(close, what)?;
        Ok(elems)
    }

    fn parse_list(&mut self) -> PResult<Expr> {
        let open = self.bump(); // `[`
        self.skip_separators();
        if self.at(TokenKind::RBracket) {
            self.bump();
            return Ok(Expr::List(Vec::new()));
        }
        let first = self.parse_expr()?;
        self.skip_separators();
        // `[body for x in xs …]` — a comprehension. `for` is not an infix
        // operator, so `parse_expr` above stops cleanly before it.
        if self.at(TokenKind::KwFor) {
            let clauses = self.parse_comp_clauses()?;
            let close = self.expect(TokenKind::RBracket, "`]`")?;
            let span = Span::new(self.lx.file, open.lo, close.hi);
            return Ok(Expr::Comprehension {
                kind: CompKind::List,
                key: None,
                body: Box::new(first),
                clauses,
                span,
            });
        }
        let mut elems = vec![first];
        while self.at(TokenKind::Comma) {
            self.bump();
            self.skip_separators();
            if self.at(TokenKind::RBracket) {
                break;
            }
            elems.push(self.parse_expr()?);
            self.skip_separators();
        }
        self.skip_separators();
        self.expect(TokenKind::RBracket, "`]`")?;
        Ok(Expr::List(elems))
    }

    /// `for var in iter (if guard)?` repeated — the tail of a comprehension.
    fn parse_comp_clauses(&mut self) -> PResult<Vec<CompClause>> {
        let mut clauses = Vec::new();
        loop {
            self.expect(TokenKind::KwFor, "`for`")?;
            let var_tok = self.expect(TokenKind::Ident, "a comprehension variable")?;
            let var = self.lx.ident(&var_tok).to_string();
            self.expect(TokenKind::KwIn, "`in`")?;
            let iter = self.parse_binary(BP_RANGE)?;
            let guard = if self.eat(TokenKind::KwIf) {
                Some(self.parse_binary(BP_RANGE)?)
            } else {
                None
            };
            clauses.push(CompClause { var, iter, guard });
            self.skip_separators();
            if !self.at(TokenKind::KwFor) {
                break;
            }
        }
        Ok(clauses)
    }

    fn parse_dict(&mut self) -> PResult<Expr> {
        self.bump(); // `#{` or a legacy `{`
        let mut entries = Vec::new();
        // A bare `{` does **not** suppress newlines (RULING ASI-0), so a
        // multi-line legacy dict arrives with `Newline` tokens between its
        // entries; skip them here rather than in the lexer.
        self.skip_separators();
        if self.at(TokenKind::RBrace) {
            self.bump();
            return Ok(Expr::Dict(entries));
        }
        loop {
            self.skip_separators();
            if self.at(TokenKind::RBrace) {
                break;
            }
            let key = self.parse_expr()?;
            self.expect(TokenKind::Colon, "`:`")?;
            let value = self.parse_expr()?;
            entries.push((key, value));
            self.skip_separators();
            if self.at(TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        self.skip_separators();
        self.expect(TokenKind::RBrace, "`}`")?;
        Ok(Expr::Dict(entries))
    }

    fn parse_tuple(&mut self) -> PResult<Expr> {
        self.bump(); // `#(`
        let elems = self.parse_elem_list(TokenKind::RParen, "`)`")?;
        Ok(Expr::TupleLiteral(elems))
    }

    fn parse_set(&mut self) -> PResult<Expr> {
        self.bump(); // `%{`
        let elems = self.parse_elem_list(TokenKind::RBrace, "`}`")?;
        Ok(Expr::SetLiteral(elems))
    }

    // ── match / when ────────────────────────────────────────────────────
    //
    // `match` and `when` are *expressions* (`docs/control-flow.md:141`), so they
    // are reached from `parse_primary` and produce `Expr::Match` / `Expr::When`.
    // The statement forms just wrap that expression, mirroring how `parse_if`
    // (a `Stmt`) and the `if` primary (an `Expr`) coexist.

    /// `match <subject> { arms }`
    fn parse_match(&mut self) -> PResult<Expr> {
        let kw = self.expect(TokenKind::KwMatch, "`match`")?;
        // BP_RANGE: the subject absorbs `..` and everything tighter, and stops
        // at a top-level ternary.
        let subject = self.parse_binary(BP_RANGE)?;
        let arms = self.parse_arms(ArmSep::Match)?;
        let span = self.span(kw).merge(self.ex_span());
        Ok(Expr::Match {
            subject: Box::new(subject),
            arms,
            span,
        })
    }

    /// `when { arms }` — the subject-less `match` (`grammar.md:143`).
    fn parse_when(&mut self) -> PResult<Expr> {
        let kw = self.expect(TokenKind::KwWhen, "`when`")?;
        let arms = self.parse_arms(ArmSep::When)?;
        let span = self.span(kw).merge(self.ex_span());
        Ok(Expr::When { arms, span })
    }

    /// Which family of arms is being parsed (§9.3). The only difference is what
    /// the arm head may be and which separators are legal.
    fn parse_arms(&mut self, sep: ArmSep) -> PResult<Vec<MatchArm>> {
        self.expect(TokenKind::LBrace, "`{` to open a match body")?;
        let mut arms = Vec::new();
        self.skip_separators();
        while !self.at(TokenKind::RBrace) {
            if self.at(TokenKind::Eof) {
                return Err(self.eof_err());
            }
            arms.push(self.parse_arm(sep)?);
            // Commas between arms are optional (`grammar.md:272-276`), and
            // newlines already separate them, so both are just separators.
            self.skip_separators();
            while self.at(TokenKind::Comma) {
                self.bump();
                self.skip_separators();
            }
        }
        self.expect(TokenKind::RBrace, "`}` to close a match body")?;
        Ok(arms)
    }

    fn parse_arm(&mut self, sep: ArmSep) -> PResult<MatchArm> {
        let lo = self.token().lo;
        let pattern = self.parse_arm_head(sep)?;
        // F-3: `if`, `when` and `&&` are all accepted as guard introducers, with
        // `if` canonical. The position is what disambiguates `if` here from the
        // Python-style ternary: a guard follows a *pattern*, and `parse_pattern`
        // never produces a `Ternary`.
        let mut guards = Vec::new();
        while matches!(
            self.peek(),
            TokenKind::KwIf | TokenKind::KwWhen | TokenKind::KwAnd
        ) {
            self.bump();
            guards.push(self.parse_binary(0)?);
        }
        let pattern = if guards.is_empty() {
            pattern
        } else {
            let span = self.span_of(lo, self.ex_hi);
            Pattern::Guard {
                pat: Box::new(pattern),
                guards,
                span,
            }
        };
        // A `when` arm's head is a condition; a `match` arm separates with
        // `->`, `=>` or `:` (F-4 — the baseline grammar names only two). `when`
        // accepts `:` and the two arrows, since the legacy corpus writes
        // `cond => body` as well.
        if !self.eat(TokenKind::Colon)
            && !self.eat(TokenKind::Arrow)
            && !self.eat(TokenKind::ThinArrow)
        {
            let t = self.token();
            let expected = if sep == ArmSep::When {
                "`:` or `=>` after a `when` condition"
            } else {
                "`->`, `=>`, or `:` after a match arm pattern"
            };
            return Err(ParseError::ExpectedToken {
                span: self.span(t),
                expected: expected.to_string(),
                found: self.lx.describe(&t),
            });
        }
        // A `{` body is usually a block (`docs/06-parser.md` §9.3), but the
        // legacy corpus also writes dict-literal bodies with a `:` separator —
        // `"bob": { name: "Bob", id: 7 }` — alongside statement bodies —
        // `1: { printed = "one" }`. The two are told apart by whether the
        // contents parse as statements, so a block is tried first and the dict
        // (expression) reading is the fallback. §9.3's "always a block" holds
        // for `->`/`=>`, which the corpus never gives a dict body.
        let body = if self.at(TokenKind::LBrace) {
            let save = (self.pos, self.ex_lo, self.ex_hi);
            match self.parse_block() {
                Ok(b) => BlockOrExpr::Block(b),
                Err(_) => {
                    self.pos = save.0;
                    self.ex_lo = save.1;
                    self.ex_hi = save.2;
                    BlockOrExpr::Expr(Box::new(self.parse_binary(BP_RANGE)?))
                }
            }
        } else {
            BlockOrExpr::Expr(Box::new(self.parse_binary(BP_RANGE)?))
        };
        let span = self.span_of(lo, self.ex_hi);
        Ok(MatchArm {
            pattern,
            body,
            span,
        })
    }

    fn span_of(&self, lo: u32, hi: u32) -> Span {
        Span::new(self.lx.file, lo, hi.max(hi))
    }

    fn eat(&mut self, k: TokenKind) -> bool {
        if self.at(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// The part of an arm before the separator. A `when` head is a condition
    /// expression; a `match` head is a pattern.
    fn parse_arm_head(&mut self, sep: ArmSep) -> PResult<Pattern> {
        let lo = self.token().lo;
        if sep == ArmSep::When {
            let cond = self.parse_binary(0)?;
            return Ok(Pattern::Condition {
                cond: Box::new(cond),
                span: self.span_of(lo, self.ex_hi),
            });
        }
        self.parse_pattern()
    }

    // ── patterns (§9) ───────────────────────────────────────────────────
    //
    // Precedence, loosest first: or-pattern, then `as`-binding, then the
    // primary pattern forms. A guard is *not* handled here — `parse_arm` adds it
    // after the whole pattern is built, so `a | b if p` guards the or-pattern
    // rather than just `b`.
    fn parse_pattern(&mut self) -> PResult<Pattern> {
        let lo = self.token().lo;
        let first = self.parse_pattern_bind()?;
        if !self.at(TokenKind::Pipe) {
            return Ok(first);
        }
        let mut alts = vec![first];
        while self.eat(TokenKind::Pipe) {
            alts.push(self.parse_pattern_bind()?);
        }
        Ok(Pattern::Or {
            alts,
            span: self.span_of(lo, self.ex_hi),
        })
    }

    /// `pat as name` — bind the matched value under a new name.
    fn parse_pattern_bind(&mut self) -> PResult<Pattern> {
        let lo = self.token().lo;
        let pat = self.parse_pattern_primary()?;
        if self.eat(TokenKind::KwAs) {
            let t = self.expect(TokenKind::Ident, "a name after `as`")?;
            let name = self.lx.ident(&t).to_string();
            return Ok(Pattern::Bind {
                pat: Box::new(pat),
                name,
                span: self.span_of(lo, t.hi),
            });
        }
        Ok(pat)
    }

    fn parse_pattern_primary(&mut self) -> PResult<Pattern> {
        let t = self.token();
        let lo = t.lo;
        match t.kind {
            // `_` is not a keyword: it lexes as `Ident`, and the parser decides
            // (which is why `Wildcard` and `Binding` share a token type).
            TokenKind::Ident => {
                let name = self.lx.ident(&t).to_string();
                let is_capitalised = name.starts_with(|c: char| c.is_ascii_uppercase());
                if name == "_" {
                    self.bump();
                    return Ok(Pattern::Wildcard(self.span(t)));
                }
                // `Some(x)` / `None` — a capitalised path, with or without fields.
                if is_capitalised {
                    let mut path = vec![name];
                    while self.at(TokenKind::Dot) {
                        self.bump();
                        let seg = self.expect(TokenKind::Ident, "a name after `.`")?;
                        path.push(self.lx.ident(&seg).to_string());
                    }
                    let mut fields = Vec::new();
                    if self.at(TokenKind::LParen) {
                        self.bump();
                        while !self.at(TokenKind::RParen) {
                            if self.at(TokenKind::Eof) {
                                return Err(self.eof_err());
                            }
                            fields.push(self.parse_pattern()?);
                            if !self.eat(TokenKind::Comma) {
                                break;
                            }
                        }
                        let close = self.expect(TokenKind::RParen, "`)` to close a pattern")?;
                        return Ok(Pattern::Constructor {
                            path,
                            fields,
                            span: self.span_of(lo, close.hi),
                        });
                    }
                    return Ok(Pattern::Constructor {
                        path,
                        fields,
                        span: self.span_of(lo, t.hi),
                    });
                }
                self.bump();
                Ok(Pattern::Binding {
                    name,
                    span: self.span(t),
                })
            }
            TokenKind::KwNil => {
                self.bump();
                Ok(Pattern::Literal {
                    value: Literal::Nil,
                    span: self.span(t),
                })
            }
            TokenKind::KwTrue => {
                self.bump();
                Ok(Pattern::Literal {
                    value: Literal::Bool(true),
                    span: self.span(t),
                })
            }
            TokenKind::KwFalse => {
                self.bump();
                Ok(Pattern::Literal {
                    value: Literal::Bool(false),
                    span: self.span(t),
                })
            }
            TokenKind::Int | TokenKind::Float => {
                self.bump();
                let lit = self.lx.num(&t).unwrap_or(NumLit::Int(0));
                let value = match lit {
                    NumLit::Int(i) => Literal::Int(i),
                    NumLit::Float(f) => Literal::Float(f),
                };
                Ok(Pattern::Literal {
                    value,
                    span: self.span(t),
                })
            }
            TokenKind::Str => {
                self.bump();
                Ok(Pattern::Literal {
                    value: Literal::String(self.lx.str_text(&t).to_string()),
                    span: self.span(t),
                })
            }
            TokenKind::LBracket => self.parse_pattern_list(),
            TokenKind::DictStart => self.parse_pattern_dict(),
            TokenKind::TupleStart => self.parse_pattern_tuple(),
            _ => Err(ParseError::Custom {
                span: self.span(t),
                msg: format!("{} is not a valid pattern", self.lx.describe(&t)),
            }),
        }
    }

    /// `[a, b, ...rest]`
    ///
    /// A rest element is allowed in exactly one position and never first
    /// (Finding F-1). That is a *position* check, not a lookahead scan: if a rest
    /// has already been taken, the next `...` is an error.
    fn parse_pattern_list(&mut self) -> PResult<Pattern> {
        let open = self.expect(TokenKind::LBracket, "`[`")?;
        let mut elems = Vec::new();
        let mut rest = None;
        loop {
            if self.at(TokenKind::RBracket) {
                break;
            }
            if self.at(TokenKind::Eof) {
                return Err(self.eof_err());
            }
            if self.eat(TokenKind::Ellipsis) {
                if rest.is_some() {
                    return Err(self.rest_error("a list pattern may have only one `...rest`"));
                }
                // `...` alone is an anonymous rest; `...name` binds the tail.
                let inner = self.parse_pattern_bind()?;
                rest = Some(Box::new(inner));
            } else {
                if rest.is_some() {
                    return Err(self.rest_error("a `...rest` element must be last"));
                }
                elems.push(self.parse_pattern()?);
            }
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        let close = self.expect(TokenKind::RBracket, "`]` to close a list pattern")?;
        if elems.is_empty() && rest.is_none() {
            // `[]` destructures an empty list, which is meaningful.
        }
        Ok(Pattern::List {
            elems,
            rest,
            span: self.span_of(open.lo, close.hi),
        })
    }

    /// `#{k: v}` — keys and values are both patterns, so `#{x: p}` binds `p`
    /// under the name `x` only after the resolver pairs them up.
    fn parse_pattern_dict(&mut self) -> PResult<Pattern> {
        let open = self.expect(TokenKind::DictStart, "`#{`")?;
        let mut entries = Vec::new();
        let mut rest = None;
        loop {
            if self.at(TokenKind::RBrace) {
                break;
            }
            if self.at(TokenKind::Eof) {
                return Err(self.eof_err());
            }
            if self.eat(TokenKind::Ellipsis) {
                if rest.is_some() {
                    return Err(self.rest_error("a dict pattern may have only one `...rest`"));
                }
                let inner = self.parse_pattern_bind()?;
                rest = Some(Box::new(inner));
            } else {
                if rest.is_some() {
                    return Err(self.rest_error("a `...rest` entry must be last"));
                }
                let key = self.parse_pattern()?;
                self.expect(TokenKind::Colon, "`:` between a dict pattern key and value")?;
                let value = self.parse_pattern()?;
                entries.push((key, value));
            }
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        let close = self.expect(TokenKind::RBrace, "`}` to close a dict pattern")?;
        Ok(Pattern::Dict {
            entries,
            rest,
            span: self.span_of(open.lo, close.hi),
        })
    }

    /// `#(a, b)` — tuple destructuring. No rest: the arity is fixed, so a
    /// `...rest` inside one is always an error rather than a wider match.
    fn parse_pattern_tuple(&mut self) -> PResult<Pattern> {
        let open = self.expect(TokenKind::TupleStart, "`#(`")?;
        let mut elems = Vec::new();
        loop {
            if self.at(TokenKind::RParen) {
                break;
            }
            if self.at(TokenKind::Eof) {
                return Err(self.eof_err());
            }
            elems.push(self.parse_pattern()?);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        let close = self.expect(TokenKind::RParen, "`)` to close a tuple pattern")?;
        Ok(Pattern::Tuple {
            elems,
            span: self.span_of(open.lo, close.hi),
        })
    }

    fn rest_error(&self, msg: &str) -> ParseError {
        ParseError::Custom {
            span: self.ex_span(),
            msg: msg.to_string(),
        }
    }
}

/// Which family of arms is being parsed (§9.3). The only difference is what the
/// arm head may be and which separators are legal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ArmSep {
    /// `match v { pat -> body }` — head is a pattern, separators `->` `=>` `:`.
    Match,
    /// `when { cond: body }` — head is a condition expression, separator `:`.
    When,
}

fn assign_op(k: TokenKind) -> Option<AssignOp> {
    Some(match k {
        TokenKind::Assign => AssignOp::Assign,
        TokenKind::PlusEq => AssignOp::AddAssign,
        TokenKind::MinusEq => AssignOp::SubAssign,
        TokenKind::StarEq => AssignOp::MulAssign,
        TokenKind::SlashEq => AssignOp::DivAssign,
        TokenKind::PercentEq => AssignOp::ModAssign,
        TokenKind::NullishEq => AssignOp::NullCoalesceAssign,
        TokenKind::AmpEq => AssignOp::BitAndAssign,
        TokenKind::PipeEq => AssignOp::BitOrAssign,
        TokenKind::CaretEq => AssignOp::BitXorAssign,
        TokenKind::ShlEq => AssignOp::ShlAssign,
        TokenKind::ShrEq => AssignOp::ShrAssign,
        TokenKind::Inc => AssignOp::Inc,
        TokenKind::Dec => AssignOp::Dec,
        _ => return None,
    })
}

fn binop(k: TokenKind) -> Option<BinaryOp> {
    Some(match k {
        TokenKind::Plus => BinaryOp::Add,
        TokenKind::Minus => BinaryOp::Sub,
        TokenKind::Star => BinaryOp::Mul,
        TokenKind::Slash => BinaryOp::Div,
        TokenKind::Percent => BinaryOp::Mod,
        TokenKind::Pow => BinaryOp::Pow,
        TokenKind::Eq => BinaryOp::Eq,
        TokenKind::Ne => BinaryOp::Ne,
        TokenKind::StrictEq => BinaryOp::StrictEq,
        TokenKind::StrictNe => BinaryOp::StrictNe,
        TokenKind::Lt => BinaryOp::Lt,
        TokenKind::Le => BinaryOp::Le,
        TokenKind::Gt => BinaryOp::Gt,
        TokenKind::Ge => BinaryOp::Ge,
        TokenKind::KwAnd => BinaryOp::And,
        TokenKind::KwOr => BinaryOp::Or,
        TokenKind::Amp => BinaryOp::BitAnd,
        TokenKind::Pipe => BinaryOp::BitOr,
        TokenKind::Caret => BinaryOp::BitXor,
        TokenKind::Shl => BinaryOp::Shl,
        TokenKind::Shr => BinaryOp::Shr,
        TokenKind::KwIn => BinaryOp::In,
        TokenKind::KwIs => BinaryOp::Is,
        TokenKind::Nullish => BinaryOp::Nullish,
        TokenKind::DotDot => BinaryOp::Range,
        TokenKind::Arrow => BinaryOp::Range,
        _ => return None,
    })
}

/// Operator precedence, lowest first; `(prec, right_assoc)`.
/// Matches docs/06-parser.md §4.1 exactly.
/// Range binds loosest of the infix operators, so "everything except a
/// top-level ternary" is `parse_binary(BP_RANGE)` (docs/06-parser.md §4.4).
const BP_RANGE: u8 = 5;

fn op_prec(k: TokenKind) -> Option<(u8, bool)> {
    match k {
        // bp 1: ternary ?:
        TokenKind::Question => Some((1, true)),
        // bp 5: range .. and ->
        TokenKind::DotDot | TokenKind::Arrow => Some((5, false)),
        // bp 8: nullish ??
        TokenKind::Nullish => Some((8, false)),
        // bp 10: logical or (only `or` keyword; `||` not in lexer)
        TokenKind::KwOr => Some((10, false)),
        // bp 15: logical and (only `and` keyword; `&&` not in lexer)
        TokenKind::KwAnd => Some((15, false)),
        // bp 20: comparison (chainable)
        TokenKind::Eq
        | TokenKind::Ne
        | TokenKind::StrictEq
        | TokenKind::StrictNe
        | TokenKind::KwIn
        | TokenKind::KwIs
        | TokenKind::Lt
        | TokenKind::Le
        | TokenKind::Gt
        | TokenKind::Ge => Some((20, false)),
        // bp 25: bitwise or
        TokenKind::Pipe => Some((25, false)),
        // bp 30: bitwise xor
        TokenKind::Caret => Some((30, false)),
        // bp 35: bitwise and
        TokenKind::Amp => Some((35, false)),
        // bp 40: shift
        TokenKind::Shl | TokenKind::Shr => Some((40, false)),
        // bp 45: additive
        TokenKind::Plus | TokenKind::Minus => Some((45, false)),
        // bp 50: multiplicative
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Some((50, false)),
        // bp 55: coercion `as` (M8, not yet implemented)
        // bp 60: unary prefix (handled in parse_unary)
        // bp 70: power (right-associative)
        TokenKind::Pow => Some((70, true)),
        _ => None,
    }
}
