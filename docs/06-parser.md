# 06 — Parser and AST

> **Status:** Draft for Milestone 1. Rulings marked **RULING** carry a test id in §12. Where this
> document and the baseline grammar at `/home/bard/Desktop/zen/docs/grammar.md` disagree, the
> disagreement is named inline and the ruling is recorded in
> [`00-blueprint-review.md`](00-blueprint-review.md) §6 terms.
> **Milestone:** M1 (Lexer + AST + parser). The AST defined here is the *only* input to the M3
> tree-walking interpreter and the M4 register compiler — there is no second engine, per
> [`00-blueprint-review.md`](00-blueprint-review.md) §1 K4.
> **Reads first:** [`05-lexer.md`](05-lexer.md). This document assumes the token stream, the
> `Span` type, and the newline-classification rules are already fixed.

## Contents

- [1. Purpose](#1-purpose)
- [2. The AST](#2-the-ast)
- [3. Span discipline](#3-span-discipline)
- [4. Precedence](#4-precedence)
- [5. `#{}` vs `{`: block or legacy dict](#5--vs--block-or-legacy-dict)
- [6. `print`, `print(...)`, and command calls](#6-print-print-and-command-calls)
- [7. Imports and module resolution](#7-imports-and-module-resolution)
- [8. `pub` visibility](#8-pub-visibility)
- [9. Pattern matching](#9-pattern-matching)
- [10. Error recovery](#10-error-recovery)
- [11. Implementation plan](#11-implementation-plan)
- [12. Test plan](#12-test-plan)
- [13. Performance](#13-performance)

---

## 1. Purpose

Zen's parser is a **hand-written recursive-descent parser with precedence climbing**. It consumes
the `Vec<Token>` produced by [`05-lexer.md`](05-lexer.md) and produces a **single typed AST** that
is the input to two consumers:

- **M3**, the tree-walking interpreter, which is the differential-testing oracle
  ([`00-blueprint-review.md`](00-blueprint-review.md) §2 A6). It walks `Expr`/`Stmt` directly.
- **M4**, the register compiler, which lowers the same tree to bytecode. It never sees a token.

**There is exactly one AST and exactly one parser.** Zen 2.x has two engines
(`exec_source_tree` vs `exec_source_cached`) and a differential harness that exists purely to prove
they agree; [`00-blueprint-review.md`](00-blueprint-review.md) §1 K4 rules that out as "a bug
factory". One AST, one parser, two back ends that must produce byte-identical output.

The parser is **not on the critical path**. It runs once per file; the VM then runs the compiled
code for the lifetime of the process. §13 sets a budget of 10 ms for the whole 84 KB conformance
corpus and explains why that is a tripwire, not a goal.

The parser is the only place in the system that makes the two deliberate compromises of this
language workable:

1. **A bare `{` is ambiguous** — block or deprecated dict literal. Resolved by *grammar position*,
   with no backtracking (§5).
2. **`name expression` is ambiguous** — a command call or a syntax error. Resolved by a
   *compile-time builtin table*, so the ambiguity is decided before it can cost anything (§6.3).

---

## 2. The AST

### 2.1 Why a normal Rust enum, not NaN-boxed

[`decisions/ADR-0001-tag-budget.md`](decisions/ADR-0001-tag-budget.md) already rules this for the
AST: the NaN-boxed `Value` is *"Rejected **for the VM hot path only**; the AST in `zen-syntax`
still uses a normal enum, because parser speed is irrelevant."*

Four independent reasons, in order of decisiveness:

1. **Shape.** An AST node needs a discriminant plus up to five 8-byte fields (span lo/hi, symbol
   id, a `Vec` pointer, a boxed child). NaN-boxing collapses that into one `u64`, which means
   **one heap allocation per node** and a pointer chase on every field read. A `Box` is the same
   cost. The packing buys nothing because there is no *cache-line-density* argument for a
   structure that is read once.
2. **Dependency direction.** `zen-syntax` must not depend on `zen-vm`, or `zenn check` and
   `zenn fmt` would link the entire VM, the native module table, and the Cranelift feature flags.
   `zenn check` on a machine with no JIT support must still work.
3. **`unsafe`.** Reading an `f64` payload out of a NaN-boxed `u64` is pointer punning, and
   [`00-blueprint-review.md`](00-blueprint-review.md) §5 point 4 confines `unsafe` to
   `zen-vm::value` and `zen-vm::gc`. A second punning site in `zen-syntax` for no measured gain is exactly the kind of
   scope creep the amendment was written to prevent.
4. **It is read once.** The interpreter's hot loop operates on `Value`, not on `Expr`. The
   compiler's hot loop operates on a `Vec<Instr>`, not on `Expr`. The AST is walked to completion
   and then dropped.

The consequence for the layout: the AST is optimised for *matchability and exhaustiveness*, not
for size. `Expr` and `Stmt` are ordinary `#[derive(Clone, Debug, PartialEq)]` enums with
`#[non_exhaustive]`-free exhaustive matches in M3 and M4, so a new variant is a compile error
everywhere until it is handled. That is the desired failure mode.

### 2.2 `Expr`

```rust
// crates/zen-syntax/src/ast/expr.rs

/// Every expression form in the language. All spans are `Span { file_id, lo, hi }` (§3).
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// A source-level numeric literal; `Int` for i48-range text, `Float` otherwise
    /// ([`05-lexer.md`](05-lexer.md) §5.4). The lexer has already decided, so the parser never
    /// re-ranges an integer and never needs to ask "is this i48 or f64?".
    Num(NumLit, Span),
    /// A string literal without interpolation.
    Str(StrLit, Span),
    /// A string literal with one or more `${…}` holes, already sub-lexed.
    Interp(InterpLit, Span),
    /// `true`, `false`, `nil` (`null` is the same value).
    Bool(bool, Span),
    Nil(Span),
    /// A bare name; resolved to a local, an upvalue, a global, or a `NameError` at compile time.
    Ident(Symbol, Span),
    /// `[a, b, ...rest]`, possibly a comprehension (`CompKind` says which).
    List(ListLit, Span),
    /// `#{k: v, ...other}` — the canonical dict, no lookahead, no ambiguity (lexer §4).
    DictLiteral(DictLit, Span),
    /// `%{a, b, c}` — a set literal.
    SetLiteral(SetLit, Span),
    /// `#(a, b)` — a tuple literal; distinct from `List` in the type system and in `typeof()`.
    TupleLiteral(TupleLit, Span),
    /// A prefix operator application: `-x`, `!x`, `not x`, `~x`, `typeof x`.
    Unary { op: UnOp, operand: Box<Expr>, span: Span },
    /// A binary operator application, including the short-circuiting `and`/`or`.
    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
    /// `a ?? b` — evaluates to `lhs` when it is neither `nil` nor an error.
    Nullish { lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
    /// `1 < x < 10` — a chained comparison kept as a flat list so `and` short-circuits the tail.
    Chain { first: Box<Expr>, rest: Vec<(CmpOp, Expr)>, span: Span },
    /// `cond ? a : b` and `a if cond else b`. `style` records which was written, for diagnostics.
    Ternary { cond: Box<Expr>, then: Box<Expr>, otherwise: Box<Expr>, style: TernaryStyle, span: Span },
    /// `if c { … } else { … }` as an expression. The canonical way to get a value from a branch.
    IfExpr { arms: Vec<(Expr, Block)>, otherwise: Option<Block>, span: Span },
    /// `a -> b` (both ends inclusive) or `a .. b` (high end exclusive).
    Range { lo: Box<Expr>, hi: Box<Expr>, lo_incl: bool, hi_incl: bool, span: Span },
    /// `a[i]`
    Index { obj: Box<Expr>, index: Box<Expr>, span: Span },
    /// `a[i:j]` / `a[i:]` / `a[:j]`
    Slice { obj: Box<Expr>, lo: Option<Box<Expr>>, hi: Option<Box<Expr>>, span: Span },
    /// `a.name`
    Member { obj: Box<Expr>, name: Symbol, span: Span },
    /// `a?.name` — yields `nil` if `a` is `nil`, propagates errors.
    SafeMember { obj: Box<Expr>, name: Symbol, span: Span },
    /// `f(a, b)` / `f()?` — `optional` is the `?.` call form.
    Call { callee: Box<Expr>, args: ArgList, optional: bool, span: Span },
    /// `obj.m(a)` — kept distinct from `Call` so the M4 compiler can emit a direct method call.
    MethodCall { obj: Box<Expr>, name: Symbol, args: ArgList, optional: bool, span: Span },
    /// `new Class(a)` — the only instantiation form.
    New { class: Path, args: ArgList, span: Span },
    /// `super(a, b)` — the parent constructor, methods only.
    SuperCall { args: ArgList, span: Span },
    /// `super.m(a)` — the parent method, methods only.
    SuperMethod { name: Symbol, args: ArgList, span: Span },
    /// `lambda x, y: body` — the body is a full expression, and a `{` there is a **dict**
    /// (§5.2, expression position), not a block. `lambda x: {a: 1}` is a lambda returning a dict.
    Lambda { params: Vec<Param>, body: Box<Expr>, span: Span },
    /// `func(x) { … }` — an anonymous function *expression*. The `{` is a **block**: `func(…)`
    /// introduces a body slot exactly as `if c` does, so this is the body position of §5.2 and
    /// not the expression position. `func(x) { {a: 1} }` is how you return a dict from a `func`.
    FnExpr { params: Vec<Param>, body: Block, span: Span },
    /// `(x, y) => body` — the params must be parenthesised, which is what disambiguates it from
    /// a parenthesised expression. The body is **expression** position, so `(x) => {a: 1}` has a
    /// dict body and `(x) => { f(x) }` has a block body; see T-A5.
    ArrowFn { params: Vec<Param>, body: BlockOrExpr, span: Span },
    /// `match x { … }` / `when { … }` as an expression. `subject: None` is `when`.
    Match { subject: Option<Box<Expr>>, arms: Vec<MatchArm>, span: Span },
    /// `…expr` — spread into a list, dict, or argument list.
    Spread(Box<Expr>, Span),
    /// `a as T` — a checked coercion. M8.
    Coerce { expr: Box<Expr>, ty: TypeName, span: Span },
    /// `spawn f(x)` — M10, [`decisions/ADR-0003-concurrency-semantics.md`](decisions/ADR-0003-concurrency-semantics.md).
    Spawn { body: Box<Expr>, span: Span },
    /// `ch <- v` / `<-ch` — M10 channel send and receive.
    ChannelSend { chan: Box<Expr>, value: Box<Expr>, span: Span },
    ChannelRecv { chan: Box<Expr>, span: Span },
}
```

### 2.3 `Stmt`

```rust
// crates/zen-syntax/src/ast/stmt.rs

/// Every statement form. A bare `Expr` statement is legal and its value is discarded
/// with a `Warning`-severity diagnostic (M8: a hard error, behind a flag).
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    /// `let` / `const` / `var` / `shared`, with a name or a destructuring target.
    /// `vis` is meaningful only at module top level (§8).
    Bind { target: BindingTarget, init: Expr, kind: BindKind, vis: Vis, span: Span },
    /// `a = 1`, `a.b = 1`, `a[0] = 1`, and every compound and `++`/`--` form.
    Assign { target: Expr, op: AssignOp, value: Expr, span: Span },
    /// `print a, b` / `print(a, b, sep: " ", end: "")` (§6.4).
    Print { args: Vec<Expr>, sep: Option<StrLit>, end: Option<StrLit>, span: Span },
    /// A bare expression statement, kept distinct so M4 can emit `POP` and M3 can warn.
    ExprStmt { expr: Expr, span: Span },
    /// `if` / `elif` / `else`, with the `elif` chain flattened into `arms`.
    If { arms: Vec<(Expr, Block)>, otherwise: Option<Block>, span: Span },
    /// `while c { … }`
    While { cond: Expr, body: Block, span: Span },
    /// `for a, b in it { … }` — one or more loop variables, all unpacked from each item.
    For { vars: Vec<Symbol>, iter: Expr, body: Block, span: Span },
    /// `break` / `continue`
    Break(Span),
    Continue(Span),
    /// `return [expr]`
    Return { value: Option<Expr>, span: Span },
    /// `throw [expr]`
    Throw { value: Expr, span: Span },
    /// `try { … } catch [T] [as e] { … } finally { … }`
    Try { body: Block, handlers: Vec<Handler>, finally: Option<Block>, span: Span },
    /// `function|func|fn|def name(params) { … }` / `native function name(params)`
    FuncDef { name: Symbol, params: Vec<Param>, body: Option<Block>, native: bool, vis: Vis, span: Span },
    /// `class Name [extends Path] { … }`
    ClassDef { name: Symbol, parent: Option<Path>, body: Block, vis: Vis, span: Span },
    /// `import a, b.c as d, "path.z"`
    Import { specs: Vec<ImportSpec>, span: Span },
    /// `from a.b import x, y as z, *`
    FromImport { module: ModulePath, items: ImportItems, span: Span },
    /// `include|load "path.z"` — executes another file in the *current* scope.
    Include { path: StrLit, span: Span },
    /// `switch x { case v: …  default: … }`
    Switch { subject: Expr, arms: Vec<(Expr, Block)>, default: Option<Block>, span: Span },
    /// `with ctx [as name] { … }`
    With { context: Expr, binding: Option<Symbol>, body: Block, span: Span },
    /// `name arg` where `name` is a compile-time-known builtin (§6.3).
    CommandCall { name: Symbol, args: Vec<Expr>, span: Span },
    /// A bare `{ … }` in statement position. Per RULING P5 this is a **block**, not a dict, and
    /// it is kept as its own variant rather than as an `ExprStmt` so that M4 can emit nothing at
    /// all and M3 does not have to recognise a "block used for its value" error case.
    Block(Block, Span),
}
```

### 2.4 Supporting types

```rust
// crates/zen-syntax/src/ast/mod.rs

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vis { Private, Public }               // §8. Default is Private, always.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindKind {
    /// `let x = …`   — immutable binding, rebinding is a compile-time `NameError`.
    Let,
    /// `const x = …` — immutable *and* enforced at runtime (`grammar.md:256-257`).
    Const,
    /// `var x = …` and `global x = …` — mutable.
    Var,
    /// `shared x = …` — `Arc<Mutex<T>>`; touching from a second thread is a catchable error.
    /// [`decisions/ADR-0003-concurrency-semantics.md`](decisions/ADR-0003-concurrency-semantics.md)
    Shared,
}

/// A binding target: a plain name, or a destructuring pattern.
#[derive(Clone, Debug, PartialEq)]
pub enum BindingTarget {
    Name(Symbol),
    Pattern(Pattern),                          // §9
}

pub type Block = Vec<Stmt>;

#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub name: Symbol,
    pub default: Option<Expr>,                 // `f(a, b = 1)`; `grammar.md:239`
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListLit {
    pub elems: Vec<Expr>,
    pub comprehension: Option<Comprehension>,  // `[f(x) for x in xs if p(x)]`; `grammar.md:227`
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompKind { List, Set, Dict }           // `[…]`, `%{…}`, `#{k: v for …}`

#[derive(Clone, Debug, PartialEq)]
pub struct Comprehension {
    pub kind: CompKind,
    pub key: Option<Expr>,                     // dict comprehensions only
    pub body: Expr,
    /// Multiple `for` clauses, unlike the legacy single-clause form (`grammar.md:227`, restated at `grammar.md:278-279`).
    pub clauses: Vec<CompClause>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompClause { pub var: Symbol, pub iter: Expr, pub guard: Option<Expr> }

#[derive(Clone, Debug, PartialEq)]
pub struct DictLit {
    pub entries: Vec<DictEntry>,
    pub comprehension: Option<Comprehension>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DictEntry {
    /// `k: v` — `k` is a name or a string literal (`grammar.md:232`); the parser rejects an
    /// arbitrary expression key so the dict can be keyed statically for the M5 hidden class.
    Pair(DictKey, Expr),
    /// `[expr]: v` — computed key; forces the dict off the fast path.
    Computed(Expr, Expr),
    /// `...expr` — merge another dict.
    Spread(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub enum DictKey { Name(Symbol), Str(StrLit) }

#[derive(Clone, Debug, PartialEq)]
pub struct SetLit { pub elems: Vec<Expr>, pub comprehension: Option<Comprehension>, pub span: Span }

#[derive(Clone, Debug, PartialEq)]
pub struct TupleLit { pub elems: Vec<Expr>, pub span: Span }

#[derive(Clone, Debug, PartialEq)]
pub struct ArgList { pub args: Vec<Arg>, pub trailing_comma: bool }

#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Positional(Expr),
    /// `f(a = 1)` — packs into a trailing dict; does **not** bind to a parameter name
    /// (`grammar.md:235-237`, `grammar.md:285-287`; runtime at `src/runtime.rs:2862-2872`).
    /// Preserved as written so M4 can be more precise later.
    Named(Symbol, Expr),
    /// `f(...xs)` — spread (`src/runtime.rs:2858-2861`; the baseline grammar at
    /// `grammar.md:234` wrongly says args have no spread — see §12, finding F-6).
    Spread(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrLit { pub cooked: Range<'static>, pub raw: Range<'static>, pub span: Span }

#[derive(Clone, Debug, PartialEq)]
pub struct InterpLit { pub parts: Vec<InterpPart>, pub span: Span }

#[derive(Clone, Debug, PartialEq)]
pub enum InterpPart { Text(Range<'static>), Expr(Box<Expr>) }   // Expr already sub-parsed

#[derive(Clone, Debug, PartialEq)]
pub struct Handler {
    /// `None` catches everything (`catch { }`).
    pub ty: Option<Path>,
    /// `Some(e)` binds the error value. `catch as e` and `catch e` are the same
    /// (`grammar.md:153-155`).
    pub binding: Option<Symbol>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportSpec { pub module: ModulePath, pub alias: Option<Symbol>, pub span: Span }

#[derive(Clone, Debug, PartialEq)]
pub enum ImportItems {
    Names(Vec<(Symbol, Option<Symbol>)>),
    /// `from m import *` — public members only (§8).
    Star,
}

#[derive(Clone, Debug, PartialEq, PartialEq, Eq, Hash)]
pub struct Symbol(pub u32);   // index into the session interner; `==` is a `u32` compare

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Path { pub parts: Vec<Symbol>, pub span: Span }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ModulePath {
    /// `a.b.c` — a dotted package path, resolved by §7.
    Dotted(Path),
    /// `"./x.z"`, `"../y"`, `"/abs/z"` — resolved relative to the importing file.
    Relative(Range<'static>),
}
```

### 2.5 Operator enums

Operator kinds are **enums, not `TokenKind` values.** The parser maps `TokenKind` → `BinOp` once
and the M3/M4 back ends then `match` on a 1-byte discriminant rather than re-deriving the operator
from a `u8` token value. This is also what lets the legacy spellings fold
([`05-lexer.md`](05-lexer.md) §3.3, RULING L2).

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BinOp { Add, Sub, Mul, Div, Rem, Pow, BitAnd, BitOr, BitXor, Shl, Shr }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum UnOp { Neg, Not, BitNot, Typeof }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LogicOp { And, Or }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CmpOp { Eq, Ne, StrictEq, StrictNe, Lt, Le, Gt, Ge, In, Is }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AssignOp { Set, Add, Sub, Mul, Div, Rem, And, Or, Xor, Shl, Shr, Nullish, Inc, Dec }
```

`is` is `CmpOp::Is` and is **plain equality**, not a type test (`grammar.md:251-252`, review §6);
`===` and `!==` are the strict forms and are *also* equality in a dynamically typed language, so
they exist for source compatibility and produce a `Warning` under `zenn lint`. M8 may give
`is` a real type-test meaning behind a flag; until then, a diagnostic that says so beats a
silent lie, exactly the argument in
[`decisions/ADR-0004-honest-types.md`](decisions/ADR-0004-honest-types.md).

### 2.6 `Shared`, `pub`, and the additions in one place

| Addition | Where it lives in the AST | Enforced at |
|---|---|---|
| `DictLiteral` | `Expr::DictLiteral(DictLit)` | `#{…}` always; `{…}` in expression position with a deprecation warning (§5) |
| `SetLiteral` | `Expr::SetLiteral(SetLit)` | `%{…}`, no ambiguity |
| `TupleLiteral` | `Expr::TupleLiteral(TupleLit)` | `#(…)`, no ambiguity |
| `Shared` binding | `BindKind::Shared` | compile time: the M4 compiler allocates the binding in the shared table; every access goes through the mutex |
| `pub` visibility | `Vis` on `Bind`, `FuncDef`, `ClassDef` | compile time, at the import site (§8) |
| `matches`-style `switch` | `Stmt::Switch` | M8 (kept in the AST from M1 so the ABI does not change) |
| comprehension clauses | `Comprehension.clauses: Vec<CompClause>` | more than one `for`, unlike `grammar.md:227` |

### 2.7 The remaining referenced types

Everything above is used somewhere in this document, so it is defined here rather than left as a
name the reader has to guess at.

```rust
// crates/zen-syntax/src/ast/literal.rs

/// The value half of a literal. The lexer has already validated and range-checked it
/// ([`05-lexer.md`](05-lexer.md) §5), so the parser only copies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LiteralValue {
    Int(i48),          // the ADR-0001 range; anything wider arrived here as Float
    Float(f64),
    Str(Range<'static>),   // cooked bytes
    Bool(bool),
    Nil,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumLit { pub value: LiteralValue, pub suffix: Option<Symbol> }

/// A source type name: `int`, `float`, `str`, `bool`, `list`, `set`, `dict`, `tuple`,
/// `#(A, B)`, `#{K: V}`, `Foo`, `a.b.C`. Kept as interned segments so a named type is one
/// `Symbol` and a compound type is a small tree.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeName { pub base: Path, pub args: Vec<TypeName>, pub span: Span }
```

```rust
// crates/zen-syntax/src/ast/match_arm.rs

/// Which family of arms is being parsed. The only difference is the separator set and whether
/// a body may be an expression (§9.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmSep { Match, Switch }

#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    /// A `match` arm body may be a block or a single expression; a `switch` case body is
    /// always a block. The two-variant body is what makes `1 -> "one"` and
    /// `1 -> { print "one" }` both work without a second parse of the same tokens.
    pub body: BlockOrExpr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockOrExpr { Block(Block), Expr(Expr) }

/// Which spelling of a ternary was written. Recorded so `zenn fmt` can normalise `c ? a : b`
/// to one form and so a diagnostic can say "in this `a if b else c` expression".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TernaryStyle { CStyle, PythonStyle }
```

```rust
// crates/zen-syntax/src/diagnostics.rs (public surface; the variants are in §10.4)

pub const MAX_PARSE_ERRORS: usize = 64;      // §10.3

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity { Warning, Error }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

/// Byte offset → (line, column), 1-based both ways. Built once per file at load time; a
/// binary search over the line-start table answers every `Span::line_col` in O(log n) and
/// the table is 4 bytes per line.
pub struct LineIndex { starts: Vec<u32> }

/// What `parse_module` returns. `errors` is non-empty for a file that did not parse
/// completely; `stmts` still holds everything that did (§10.1).
pub struct Parsed {
    pub file: FileId,
    pub stmts: Vec<Stmt>,
    pub errors: Vec<ParseError>,
    pub warnings: Vec<Diagnostic>,
    pub symbols: SymbolTable,          // the session interner, moved out for the resolver
    pub nodes: u32,                    // AST node count, for §13's bytes-per-token budget
}
```

```rust
// crates/zen-syntax/src/parse/budget.rs

/// A parse that has gone wrong can loop. The budget is a step counter, not a wall-clock
/// check, because a wall-clock check is non-deterministic and a test must be able to assert
/// it. 200 steps per token is far above the ~6 steps/token a well-formed file needs and
/// far below anything that would make the test suite slow.
pub struct StepBudget { pub spent: u32, pub per_token: u32 }
impl StepBudget {
    pub fn charge(&mut self, n: u32) -> Result<(), ParseError>;
    pub fn reset_per_token(&mut self);
}
```

---

## 3. Span discipline

### 3.1 The rule

**Every AST node carries a `Span`.** Not every leaf, not just the "interesting" ones — every
`Expr` and every `Stmt`, including the `Ident` inside an `Index`, and including the `else` branch
of a `Ternary`.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span { pub file_id: FileId, pub lo: u32, pub hi: u32 }

impl Span {
    pub const DUMMY: Span = Span { file_id: FileId(0), lo: 0, hi: 0 };
    pub fn new(file: FileId, lo: u32, hi: u32) -> Self { debug_assert!(lo <= hi); Self { file_id, lo, hi } }
    pub fn join(self, other: Span) -> Span {            // for a synthesized parent
        debug_assert_eq!(self.file_id, other.file_id);
        Span { file_id: self.file_id, lo: self.lo.min(other.lo), hi: self.hi.max(other.hi) }
    }
    pub fn line_col(&self, lines: &LineIndex) -> (u32, u32) { lines.line_col(self.lo) }
    pub fn text<'a>(&self, src: &'a str) -> &'a str { &src[self.lo as usize..self.hi as usize] }
}
```

The `Token` struct in [`05-lexer.md`](05-lexer.md) §2.2 carries no `file_id` because a `Lexed` is
per-file. `Span` carries one because **an AST spans files**: a module's AST embeds spans from the
file it was parsed from, and after import resolution a single `FuncDef` can have a `body` whose
statements come from three different files. The `file_id` is what makes that representable.

### 3.2 Why diagnostics require this

Three concrete classes of message that are impossible without a span, and that the project
promises:

1. **Compile-time.** `error: expected ':' after the key in a dict literal`, with a caret. The
   parser must know *which* `:` and *where*.
2. **Runtime, from M3.** `TypeError: int has no member 'lenght'` must print
   `app.z:14:22` and underline `a.b.lenght()`. The tree-walker holds an `Expr` node at the point of
   failure; the node's `Span` is the answer. The legacy runtime has `line`/`col` on `Stmt` only
   (`src/runtime.rs:1395-1400`), so an error inside an expression reports the *statement's* start —
   which is why its messages are so often unlocatable.
3. **Runtime, from M4.** The compiler records, for every instruction that can raise, the `Span` of
   the AST node it came from. The VM's error path pops the span from the frame. This is a strict
   improvement on a bytecode line number, which points at a generated artifact the user has never
   seen.

### 3.3 Mapping a runtime error back to a source line

```rust
// crates/zen-vm/src/error.rs
pub struct RuntimeError {
    pub kind: ErrorKind,          // TypeError, IndexError, KeyError, NameError, …
    pub message: String,
    pub span: Option<Span>,       // None for errors with no source location (OOM, host failure)
    pub frames: Vec<FrameInfo>,   // innermost first
}

pub struct FrameInfo { pub fn_name: Symbol, pub span: Span, pub file: FileId }

impl RuntimeError {
    /// Render with the same `render()` the compiler uses, so a runtime traceback and a
    /// compile error look identical to the user.
    pub fn render(&self, files: &SourceMap) -> String;
}
```

Output:

```text
TypeError: int has no member 'upper'
  --> /proj/app.z:14:22
   |
14 |     print(n.upper())
   |               ^^^^^
   |
   = note: in call to `upper` at /proj/util.z:3:1
```

**M3** stores the current `Span` in the frame as it evaluates each node — one 12-byte store per
node visit, measurable but on a path that is already doing a `Value` dispatch. **M4** stores a
`u32` span index per potentially-raising instruction in a per-function side table, so the hot
arithmetic instructions pay nothing; only the error path loads.

### 3.4 The span-to-source lookup is a `SourceMap`, not a file handle

```rust
pub struct SourceMap {
    files: Vec<LoadedFile>,       // { path: PathBuf, src: String, lines: LineIndex }
    by_path: HashMap<PathBuf, FileId>,
}
impl SourceMap {
    pub fn load(&mut self, path: &Path) -> Result<FileId, IoError>;
    pub fn get(&self, f: FileId) -> &LoadedFile;
    pub fn resolve(&self, span: Span) -> (&Path, &str, (u32, u32));  // path, line, col
}
```

The map is retained for the whole process lifetime, even after parsing, because a runtime error
two minutes into execution still needs to print source. A 100 KB project costs 100 KB of resident
source, which is nothing next to the VM's own footprint and is the only reason the diagnostic above
is possible at all.

---

## 4. Precedence

### 4.1 The table, low to high

| bp | Level | Operators | Assoc | AST node |
|---|---|---|---|---|
| 0 | *command argument* | `name expr` (builtin only) | — | `Stmt::CommandCall` |
| 1 | ternary | `? :` · `if … else` | right (else-branch at bp 0) | `Expr::Ternary` |
| 5 | range | `..` · `->` | left | `Expr::Range` |
| 8 | nullish | `??` | left | `Expr::Nullish` |
| 10 | logical or | `or` · `\|\|` | left | `Expr::Binary{LogicOp::Or}` |
| 15 | logical and | `and` · `&&` | left | `Expr::Binary{LogicOp::And}` |
| 20 | comparison | `==` `!=` `===` `!==` `<` `>` `<=` `>=` `is` `in` | left, **chainable** | `Expr::Chain` |
| 25 | bitwise or | `\|` | left | `Expr::Binary{BitOr}` |
| 30 | bitwise xor | `^` | left | `Expr::Binary{BitXor}` |
| 35 | bitwise and | `&` | left | `Expr::Binary{BitAnd}` |
| 40 | shift | `<<` `>>` | left | `Expr::Binary{Shl, Shr}` |
| 45 | additive | `+` `-` | left | `Expr::Binary{Add, Sub}` |
| 50 | multiplicative | `*` `/` `%` | left | `Expr::Binary{Mul, Div, Rem}` |
| 55 | coercion | `as` | left | `Expr::Coerce` (M8) |
| 60 | unary prefix | `-` `!` `not` `~` `typeof` | prefix, right | `Expr::Unary` |
| 70 | power | `**` | **right** | `Expr::Binary{Pow}` |
| 80 | postfix | `.` `?.` `[i]` `[a:b]` `(args)` `?.(args)` `++` `--` | left | `Member`, `Index`, `Call`, … |
| — | **literals** | `#{…}` `%{…}` `#(…)` `[…]` `(…)` `"…"` `` `…` `` `123` | primary | `DictLiteral`, `SetLiteral`, `TupleLiteral`, `List`, `Str`, `Num` |

`..=` is **lexed but rejected** ([`05-lexer.md`](05-lexer.md) §3.3, RULING L4); the parser returns
`ParseError::ReservedToken("..=")` with "write `a -> b` for an inclusive range".

### 4.2 Four rulings the baseline grammar does not settle

**RULING P1 — the baseline's own EBNF contradicts its own precedence comment.**
`grammar.md:167-183` lists the levels low-to-high starting with `-> ..`, then `? :`, then `??`. But
`grammar.md:185` writes `expression = range ternary`, which puts `range` *above* `ternary` — the
opposite. The precedence comment is the one a reader trusts and the one
`tests/cases/*.z` was written against, so **the comment wins**: ranges bind loosest, then ternaries,
then `??`. The EBNF line is a typo. Recorded here rather than silently fixed in the old tree, which
is a fallback we are not modifying.

**RULING P2 — ternary conditions absorb ranges but not ternaries.** A ternary's then-branch and
condition are parsed with `min_bp = BP_TERNARY` (1). Because a range's left binding power is 5,
`1..5` is absorbed by both; because a nested ternary's is 1, it is not. The else-branch is parsed
with `min_bp = 0`, so it right-associates and may contain another ternary. This gives exactly the
shape the baseline grammar describes at `grammar.md:186-187` (`"if" range "else" expression`)
without the contradiction, and it makes these all work:

```zen
1..5 if c else 2        // then-branch is the range
a if 1..5 else 2       // condition is the range
a if c else b if d else e   // right-associative
c ? a : c ? b : d     // right-associative
c ? c2 ? a : b : d    // parse error: parenthesise the inner ternary
```

**RULING P3 — `not` is a unary operator, not a precedence level.**
`grammar.md:191-192` defines `and_expr = not_expr ("and"|"&&")*` and
`not_expr = ("not"|"!") not_expr | comparison`, which would place `not`
between `and` and `comparison`. The precedence comment at `grammar.md:181` places `- ! not ~ typeof`
together in the unary row, above `* / %`. The comment wins: `not` is `UnOp::Not` at bp 60, so
`not a == b` is `not (a == b)` in both readings but `not a and b` is `not (a and b)`, which differs
from the EBNF's reading. The comment's reading is what every language with a `not` keyword
(Python, Ruby, JS) means, and it is the one a user expects. Test T-P13.

**RULING P4 — `->` is one token, two meanings, resolved by position.** Review §6 says: *"Keep the
existing range meaning. Match arms accept both `->` and `=>`."* So `5 -> 1` is an inclusive
descending range (`grammar.md:245-248`) and `match x { 1 -> "one" }` is an arm separator. There is
no ambiguity because the two are in disjoint syntactic positions: `parse_match_arm` calls
`parse_pattern`, which never produces a `Range`, and then expects `->` / `=>` / `:`. A range is
therefore **not a legal match pattern** in M1–M8; `match x { 1 -> 2: … }` is
`ParseError::NotAPattern` with "range patterns are not supported; write `case` guards or a
guard expression". That is a deliberate limitation, not an oversight, and it is the price of one
token with two readings.

### 4.3 Chained comparison

`grammar.md:277` states that `1 < x < 10` works. The AST keeps it flat rather than desugaring to
`&&`, so M4 can emit a single `CMP_JMP` chain and M3 can evaluate it without materialising an `and`.

```zen
1 < x < 10   ->   Chain { first: 1, rest: [(Lt, x), (Lt, 10)] }
```

Short-circuits on the first false comparison, exactly as `and` would. `a in b in c` parses the
same way and is equally useless; the parser accepts it and the runtime answers honestly.

### 4.4 The precedence-climbing loop

```rust
impl<'a> Parser<'a> {
    /// Precedence climbing. `min_bp` is the loosest binding power the caller is willing to
    /// absorb into the expression being parsed.
    pub fn expr_bp(&mut self, min_bp: u8) -> Result<Expr> {
        let mut lhs = self.unary()?;
        loop {
            let Some((lbp, rbp)) = self.peek_infix() else { break };
            if lbp < min_bp { break }              // note: `<`, not `<=`; see below
            self.bump();
            // Right-associative operators get `rbp = lbp`, left-associative get `rbp = lbp + 1`.
            let rhs = self.expr_bp(rbp)?;
            lhs = self.make_binary(lhs, rhs, /* op from the bumped token */ )?;
        }
        Ok(lhs)
    }
}
```

Two details that are easy to get wrong and are therefore called out:

- The termination test is `lbp < min_bp`. With `lbp`/`rbp` as in the table above
  (left-assoc `rbp = lbp + 1`, right-assoc `rbp = lbp`), a left-associative level `L` is absorbed
  by a caller passing `min_bp <= L` and rejected by a caller passing `min_bp > L`. The off-by-one
  is the classic bug: writing `lbp <= min_bp` silently drops every left-associative operator, and
  `1 + 2 + 3` becomes `1 + 3`.
- `peek_infix` returns `(lbp, rbp)` from a `match` on `self.peek()`, not from a table, because the
  mapping is irregular: `in` is `CmpOp::In` and also starts a `for` clause, `is` is
  `CmpOp::Is` and also the start of a *type* test, and `**` is right-associative while every other
  arithmetic operator is left-associative.

**Ternary is deliberately *not* in the infix table.** Putting it there would mean the loop has to
break on `lbp == BP_TERNARY` to implement RULING P2, and that special case is exactly the kind of
thing that gets forgotten the next time a level is inserted. Instead `parse_ternary` sits *above*
`expr_bp`, so "stop at a top-level ternary" is a property of the call tree rather than of the loop
condition.

**Ternary, spelled out.** `expr_bp(BP_RANGE)` is the call that means "parse everything except a
top-level ternary": the range at bp 5 satisfies `5 >= 5` and is absorbed, while a ternary at bp 1
gives `1 >= 5` and stops the loop.

```rust
fn parse_ternary(&mut self) -> Result<Expr> {
    // bp 5: absorbs `..`/`->` (5), `??` (8), everything tighter; stops at a top-level ternary (1).
    let lhs = self.expr_bp(BP_RANGE)?;

    if self.eat(TokenKind::Question) {                    // C style:  cond ? a : b
        let cond = lhs;
        let then = self.expr_bp(BP_RANGE)?;
        self.expect(TokenKind::Colon, "in a `? :` expression")?;
        let otherwise = self.expr_bp(0)?;                 // full expression, right-associative
        return Ok(Expr::Ternary { cond: Box::new(cond), then: Box::new(then),
                                  otherwise: Box::new(otherwise),
                                  style: TernaryStyle::CStyle, span: /* joined */ });
    }

    if self.peek_is(TokenKind::KwIf) {                    // Python style:  a if b else c
        self.bump();
        let cond = self.expr_bp(BP_RANGE)?;               // no top-level `?` here (RULING P2)
        self.expect(TokenKind::KwElse, "after the condition of an `a if b else c` expression")?;
        let otherwise = self.expr_bp(0)?;
        return Ok(Expr::Ternary { cond: Box::new(cond), then: Box::new(lhs),
                                  otherwise: Box::new(otherwise),
                                  style: TernaryStyle::PythonStyle, span: /* joined */ });
    }

    Ok(lhs)
}
```

The C-style **condition** is also parsed at `expr_bp(BP_RANGE)`, so `a ? b ? c : d : e` is a
deliberate parse error — "the condition of `? :` may not contain `? :`; add parentheses" — because
Python's `a if b else c` has exactly the same restriction, and silently allowing it in the C form
would make the two spellings of one construct disagree about a nested one. The `else` branch is
parsed at `expr_bp(0)`, so `a ? b : c ? d : e` right-associates as every reader expects.

### 4.5 Power and unary

`grammar.md:181-183` puts `**` above the unary row, with `unary = (…) unary | power` and
`power = call ("**" unary)?`. That yields Python semantics:

| Source | AST | Note |
|---|---|---|
| `-2 ** 2` | `Unary{Neg, Binary{Pow, 2, 2}}` = `-4` | unary binds looser than `**` |
| `2 ** -1` | `Binary{Pow, 2, Unary{Neg, 1}}` | the exponent is a full unary |
| `2 ** 3 ** 2` | `Binary{Pow, 2, Binary{Pow, 3, 2}}` = 512 | right-associative, `rbp = lbp` |

`2 ** 3 ** 2 == 512` is a named test (T-P16) because left-associative `**` is the single most
common precedence bug in a hand-written parser.

---

## 5. `#{}` vs `{`: block or legacy dict

### 5.1 The problem

The baseline grammar is **genuinely ambiguous**, and does not notice. It defines both

```text
block = "{" statement* "}"                                  (* grammar.md:159 *)
dict  = "{" dict_pair ("," dict_pair)* ","? "}"             (* grammar.md:229-232 *)
```

and `statement` is `let | … | block | command_call | expression` (`grammar.md:63-68`), while
`primary` includes `dict` (`grammar.md:212-214`). So at the start of a statement both
`statement → block` and `statement → expression → primary → dict` are live, both begin with `{`,
and nothing in the document says which one wins.

The runtime picks two *different* answers in two *different* functions:

| Function | `{` means | Line |
|---|---|---|
| `block()` | block | `src/runtime.rs:1658-1662` |
| `atom()` | dict | `src/runtime.rs:3125-3173` |

There is no rule connecting them. The disambiguation is a side effect of *which function the
control flow happened to be in*, and a real 84 KB corpus depends on it: every dict literal in
`tests/cases/` sits in a value position — `var d = {a: 1, b: 2}` at
`tests/cases/01_literals_ops.z:31`, `var obj = {name: "zen", n: 42, tags: [1, 2]}` at
`tests/cases/06_modules_core.z:30`, and 27 more across six files.

Review §6 settles the fix: *"`#{a:1}` is canonical; `{a:1}` parses as a dict … and emits a
deprecation warning."* The question this section answers is **how**, and the answer has to survive
three constraints that pull against each other:

1. **No ambiguity.** `{` must mean exactly one thing in any position the parser can tell.
2. **No backtracking.** [`05-lexer.md`](05-lexer.md) RULING L1 and the no-`unsafe`-outside-the-VM
   rule (review §5 point 4) both forbid it.
3. **No silent breakage.** Every `tests/cases/*.z` file that uses `{…}` must keep parsing and
   keep producing identical output.

These are compatible, because the ambiguity is *not* in the token — it is in the *position*, and a
recursive-descent parser always knows what position it is in.

### 5.2 The ruling

**RULING P5 — `{` is decided by grammatical position, decided at `parse_primary`, never by
lookahead.**

| Context | Parser function | `{` means | Rationale |
|---|---|---|---|
| First token of a *statement* | `parse_stmt` | `LBrace` → **block** | A statement cannot be a dict value; the block production owns statement position. |
| First token of an *expression* operand | `parse_primary` | `LBrace` → **legacy dict** | Only an expression can be a value, and the value slot is the only place a dict is legal. |
| Immediately after `if c` / `while c` / `else` / `fn f()` / `else if` | `parse_block` | **block** | The body slot. |
| `=>` body, `lambda x:` body, comprehension body | `parse_expr` → `parse_primary` | **legacy dict** | These are *expression* bodies, so `{` is a dict. See the note in §5.4. |
| `{` after `class Name` / `extends Base` | `parse_block` | **block** | The body slot. |
| `match x {` / `switch x {` / `enum E {` | `parse_block` | **block** | The arm/case container. |

The crucial property: **`parse_stmt` and `parse_primary` are never the same function.** A statement
starts where a statement is expected; an operand starts where a value is expected. There is no
context in which the parser must decide, and therefore no context in which it can be wrong.

**This replaces the mechanism review §6 describes.** The review's wording — *"`{a:1}` parses as a
dict only when a `key:` pair is found"* — describes a **scan**, and a scan for `:` is unbounded
lookahead, which [`05-lexer.md`](05-lexer.md) RULING L1 forbids. The position
rule reaches the *same accept/reject outcome* for every input in the corpus with **zero** bytes of
lookahead, so the ruling is stated in terms of position and the review's phrasing is treated as
describing the intent rather than the mechanism. The two differ on exactly one shape: a bare
`{a: 1}` *statement*, which the scan accepts and the position rule rejects. No corpus file contains
one, and a dict written as a statement discards its value, so rejecting it is the better
behaviour even if it is the one observable difference.

```rust
// crates/zen-syntax/src/parser/primary.rs
TokenKind::LBrace => {
    // Value position. Per RULING P5 this is the deprecated legacy dict.
    let dict = self.legacy_dict_literal()?;
    self.dict_warnings.emit_legacy_braces(dict.span);
    Ok(Expr::DictLiteral(dict))
}
```

### 5.3 The two functions, and the one guard

```rust
// crates/zen-syntax/src/parser/stmt.rs
fn parse_stmt(&mut self) -> Result<Stmt> {
    if self.peek_is(TokenKind::LBrace) {                 // statement position: BLOCK (RULING P5)
        let start = self.peek_span();
        let body = self.parse_block()?;
        return Ok(Stmt::Block(body, start.join(self.last_span())));
    }
    …
}

fn parse_block(&mut self) -> Result<Block> {
    self.expect(TokenKind::LBrace, "`{` to open a block")?;
    let mut out = Vec::new();
    while !self.peek_is(TokenKind::RBrace) && !self.at_eof() {
        match self.parse_stmt() {
            Ok(s) => out.push(s),
            Err(e) => { self.recover_to_stmt_boundary(e); }   // §10
        }
    }
    self.expect(TokenKind::RBrace, "`}` to close a block")?;
    Ok(out)
}

// crates/zen-syntax/src/parser/primary.rs
fn legacy_dict_literal(&mut self) -> Result<DictLit> {
    self.expect(TokenKind::LBrace, "`{` to open a dict")?;
    let mut entries = Vec::new();
    while !self.peek_is(TokenKind::RBrace) && !self.at_eof() {
        if self.peek_is(TokenKind::Ellipsis) {                    // `...other`
            self.bump();
            entries.push(DictEntry::Spread(self.expr_bp(BP_RANGE)?));
        } else if self.peek_is(TokenKind::LBracket) {             // `[expr]: v`
            self.bump();
            let key = self.expr_bp(BP_RANGE)?;
            self.expect(TokenKind::RBracket, "`]` to close a computed dict key")?;
            self.expect(TokenKind::Colon, "after a computed dict key")?;
            let value = self.expr_bp(BP_RANGE)?;
            entries.push(DictEntry::Computed(key, value));
        } else {
            let key = self.dict_key()?;                           // `Name` or string
            self.expect(TokenKind::Colon, "after the key of a dict entry")?;
            let value = self.expr_bp(BP_RANGE)?;
            entries.push(DictEntry::Pair(key, value));
        }
        if !self.eat(TokenKind::Comma) { break }                  // trailing comma optional
    }
    self.expect(TokenKind::RBrace, "`}` to close a dict")?;
    Ok(DictLit { entries, comprehension: None, span: self.last_span() })
}

/// The *fast* key form: a name or a string literal. `grammar.md:232` says expression keys are not
/// allowed; the runtime disagrees and accepts `[expr]: v` (`src/runtime.rs:3133-3138`), so this
/// parser accepts both and the `DictEntry` variant records which form was written.
fn dict_key(&mut self) -> Result<DictKey> {
    match self.peek() {
        TokenKind::Ident(name) => { let n = *name; self.bump(); Ok(DictKey::Name(n)) }
        TokenKind::Str(_) => Ok(DictKey::Str(self.str_literal()?)),
        _ => Err(self.error_here(
            "a dict key must be a name or a string; wrap an expression in `[…]` for a computed key")),
    }
}
```

The guard that keeps this honest is `dict_key`. A legacy `{…}` in a statement position is a block;
in a value position it is a dict; and inside the dict, the *unbracketed* key must be a name or a
string. A user who writes `{ x() : 1 }` gets *"a dict key must be a name or a string; wrap an
expression in `[…]` for a computed key"* rather than a silent demotion of the dict to a slow hash
map. The bracketed form is not rejected — that would break
`src/runtime.rs:3133-3138` and any user who discovered it — but it is typed differently in the AST
so M5 can keep name-keyed and string-keyed dicts on the hidden-class fast path.

### 5.4 The one genuinely surprising consequence, stated up front

An `if` used as an **expression** with a bare `{` takes the dict reading, not the block reading,
because the `if`-as-expression path goes through `parse_expr`:

```zen
x = if c { "yes" } else { "no" }      # spec: ERROR — both `{…}` are dicts
x = if c { "yes" } else "no"          # fine
x = (if c { "yes" } else "no")        # spec: ERROR — parens do not change the position
```

This is not a bug, it is the position rule doing exactly what it says, and the diagnostic would say
so:

```text
error: `{` in expression position is a dict literal, so the branches of an `if` used as an
       expression must be expressions.
  --> /proj/a.z:2:13
   |
 2 | x = if c { "yes" } else { "no" }
   |             ^^^^^^^^^^^^
   |
   = note: a `{` after a *statement* `if` is a block; after an `if` *expression* it is a dict
   = help: use `if c { "yes" } else "no"`, or write the blocks explicitly:
   =       x = if c then "yes" else "no"
```

The `help` line is the important half. The `then`/`else` forms are M8 and are not in the M1 AST
(`Expr::IfExpr` is an *expression* whose arms are `Block`s, so it is a different construct from a
`{`-block if and is only reachable from `parse_stmt`).

> **M1 implementation note — deliberate divergence.** The shipped M1 parser does **not** enforce the
> strict reading above. When it sees the `if` keyword in expression position the parse is already
> committed, so a following `{` can only be a block; `x = if c { "yes" } else { "no" }` is therefore
> accepted and produces `Expr::IfExpr` with `Block` arms (`crates/zen-syntax/src/parser.rs`,
> `parse_unary`; tests `p_if5_if_expression_accepts_block_arms`,
> `p_e14_if_expression_elif_chain`, `p_e14h_if_expression_without_else`). The `{` is **not** read as
> a dict here, unlike the position rule in §5.3. This is a deliberate lenient **superset** of the
> spec, chosen because the block-armed form is what real code is written in today; the strict
> rejection (and the `then`/`else` spelling) is deferred to **M8**. The `note`/`help` diagnostic
> text above is retained as the intended M8 behaviour, not as current behaviour.

**The rule to remember: an `if` *statement* has block arms; an `if` *expression* has expression
arms.** That is not special-casing — it is the same rule that makes `(1, 2)` a tuple and
`fn x: 1` a lambda body.

### 5.5 The deprecation warning

```rust
// crates/zen-syntax/src/parser/diagnostics.rs
pub struct DictWarnings;

impl DictWarnings {
    /// One warning per *source site*, deduped by span, so a dict in a hot function is not
    /// reported once per instantiation.
    pub fn emit_legacy_braces(&mut self, span: Span);
    pub fn is_empty(&self) -> bool;
}
```

```text
warning: `{…}` dict literals are deprecated; use `#{…}`
  --> /proj/a.z:2:13
   |
 2 | x = {"a": 1}
   |      ^^^^^^^
   |
   = note: `{` still parses as a block in statement position, so this is a style rule, not a
   =       syntax error; it will become a hard error in Zen 3.0
   = help: `#{…}` is unambiguous, requires no positional reasoning, and is faster to compile
```

Severity is `Warning`, not `Error`, and it is **not** behind a flag. The three requirements were:
existing code keeps running, new code is told, and there is a deadline. A `Deprecated` lint in
`zenn lint` that counts occurrences is enough; promoting it to an error behind
`--deny-deprecated-dicts` is M8's job, by which point `zenn migrate` will have rewritten them.

### 5.6 Dict comprehension and the `for` ambiguity inside `#{…}`

A dict comprehension is `#{k: v for x in xs}`, and it lives in the *new* syntax, so it has no
`{`/`}` ambiguity at all:

```zen
names = #{p.name: p.age for p in people}
```

The parser decides between "another entry" and "a comprehension clause" on the single token `for`
after a complete `k: v` entry — **not** by scanning ahead for a `for`, which is what would make it
look ambiguous. `parse_dict` accepts one optional `CompClause` list; if the first token after an
entry is `for`, the rest of the `#{…}` is the comprehension.

The same logic serves `[f(x) for x in xs]` (`grammar.md:227`) and `%{x for x in xs}`. Multiple
clauses (`[f(x) for x in xs for y in xs[y]]`) are allowed, unlike the legacy single-clause form, and
a `filter`/`if` guard clause is written `for x in xs if p(x)` with `guard: Some(p(x))`.

### 5.7 M1 implementation status: legacy-corpus leniencies

`docs/19-roadmap.md` makes "every `tests/cases/*.z` and `tests/diff/*.z` parses" the M1 gate, and
those files are the legacy Zen 2.2.0 suite. Where the new spec is stricter than that corpus, the M1
parser ships a deliberate **lenient superset** rather than failing the gate. Each item below is a
known divergence, tested in `crates/zen-syntax/tests/parser.rs` or
`crates/zen-syntax/tests/parse_corpus.rs`; tightening them (and the corresponding diagnostics) is
M8's job.

| Leniency | Diverges from | Why | Test |
|---|---|---|---|
| `var x = 0` / `global x = 0` bind | `var`/`global` fold to `KwVar`; binding dispatch | legacy corpus | `p_var_binding` |
| `new Name(args)` | not in the M1 expr grammar | legacy corpus | `p_new_expression` |
| Bare `{…}` dict in *value* position | §5.3 already allows it; §5.4's `if`-expr case is the notable one | legacy corpus | `p_legacy_dict_multiline_trailing_comma` |
| Multi-line `{…}` dicts | a bare `{` does not suppress newlines (RULING ASI-0), so `parse_dict` skips them | legacy corpus | `p_legacy_dict_multiline_trailing_comma` |
| `print "x"` (no parens) | `print` is a call in the new grammar | legacy corpus | `p_print_without_parens` |
| `not x` | `KwNot` as a prefix operator (folded with `!`) | legacy corpus | `p_not_unary` |
| `func(x) { … }` anonymous fn | spec `FnExpr`; the AST now carries it | spec-aligned | `p_fn_expr_block_body` |
| `lambda(x) { … }` block body | spec says a lambda `{` is a **dict** (§5.2) | legacy corpus | `p_lambda_legacy_block_body` |
| `(x) => { … }` block body | §5.2 says an arrow `{` is a **dict** | corpus uses block bodies | `p_arrow_block_body` |
| `when { cond => body }` | §9.3 names `:` only | legacy corpus | `p_when_arrow_separator` |
| `:`-separated match body may be a dict | §9.3 says a `{` arm is **always** a block | corpus writes `"bob": { name: "Bob" }` *and* `1: { y = 2 }`, so block is tried first and dict is the fallback | `p_match_colon_dict_body`, `p_match_colon_statement_body` |
| `[body for …]` comprehensions | spec `Comprehension` node | corpus | `p_comprehension_plain`, `p_comprehension_guard_and_nested` |
| `s[a:b]` / `s[a, b]` slices | not in the M1 expr grammar | corpus | `p_slice_colon`, `p_slice_comma_bounds` |
| Multi-line `[…]` with trailing comma | harmless superset | corpus | `p_list_multiline_trailing_comma` |

The `if`-in-expression-position leniency is the one called out separately in §5.4.

---

## 6. `print`, `print(...)`, and command calls

### 6.1 The three forms

`grammar.md:93-94` gives the two paren/bare productions and `grammar.md:253` states the
behaviour in prose; the runtime implements both at `src/runtime.rs:1809-1844`.

| Form | Example | AST |
|---|---|---|
| `print(a)` | `print(1)` | `Stmt::Print` |
| `print` with keyword arguments | `print(a, b, sep: "-")` | `Stmt::Print{sep: Some(..)}` |
| **bare word** | `print a` | `Stmt::Print` |
| trailing comma | `print(a,)` | `Stmt::Print` — accepted, per `grammar.md:94` |

```text
print_call = "print" [ "(" [ print_args ] ")" | print_args ] [ ";" ]
print_args = print_arg { "," print_arg }
print_arg  = expression | "sep" ":" string | "end" ":" string
```

Note that `grammar.md:94` writes `"," "sep" "=" str` with `=`, while the runtime and every real
program use `sep:` (`src/runtime.rs:1819`, `grammar.md:253`). The `=` form is accepted as a
deprecated alias so nothing that followed the reference breaks; `sep:` is canonical.

The `sep`/`end` arguments are **string literals**, and the legacy parser is where that decision
gets made: `src/runtime.rs:1818-1829` writes
`if let Expr::Value(Value::String(s)) = self.expr()? { sep = Some(s) }` with no `else`, so
`print(1, sep: 2)` in the legacy runtime **silently drops the separator** and prints with the
default. That is a silent wrong-answer bug, and it is the single best argument in this section for
doing the check at parse time: a diagnostic at the `2` is worth more than a subtly wrong output
that nobody notices for months.

The bare-word form is the reason `print` needs its own production and not a general `Expr::Call`.
`print a` is not `print(a)` — there is no `Call` node, no callee expression, and no way to spell it
in the value position, because `print` is a *statement*. The legacy runtime makes the same split at
`src/runtime.rs:1809` (a `Kind::Print` arm inside `stmt()`), never in `atom()`.

### 6.2 `Stmt::Print` construction

```rust
// crates/zen-syntax/src/parser/print.rs
fn parse_print(&mut self) -> Result<Stmt> {
    let start = self.expect(TokenKind::KwPrint, "`print`")?;
    let mut args = Vec::new();
    let mut sep = None;
    let mut end = None;
    let mut trailing_comma = false;

    // Paren form: `print( … )`
    let paren = self.eat(TokenKind::LParen);
    loop {
        if paren && self.peek_is(TokenKind::RParen) { break }
        if !paren && self.at_stmt_boundary() { break }        // bare form: stop at newline/`;`
        if self.peek_is(TokenKind::Eof) { break }

        // `sep:` / `end:` are the only two keyword arguments. `print(a, b, end: "\n")` and
        // `print a, b` are both legal; `print(a; b)` is not. The test is two tokens
        // (`Ident` then `Colon`) plus a name comparison — no scan, no backtrack.
        if let Some(kw) = self.peek_kwarg() {
            self.bump();                                   // the name
            self.bump();                                   // the `:`
            let value = self.str_literal()?;                // parse-time literal check, below
            match kw {
                "sep" => sep = Some(value),
                "end" => end = Some(value),
                _ => unreachable!("peek_kwarg only returns `sep` or `end`"),
            }
        } else {
            args.push(self.expr_bp(BP_RANGE)?);
        }

        if self.eat(TokenKind::Comma) { trailing_comma = true; continue }
        trailing_comma = false;
        break;
    }
    if paren { self.expect(TokenKind::RParen, "`)` to close `print(…)`")?; }

    Ok(Stmt::Print { args, sep, end, span: start.join(self.last_span()) })
}

/// `Some("sep")` / `Some("end")` when the next two tokens are `Ident` + `:` with that name,
/// `None` otherwise. Written as a helper so `parse_print` reads as a table of argument kinds.
fn peek_kwarg(&self) -> Option<&'static str> {
    if !self.peek_is(TokenKind::Ident) || !self.peek_at(1).is_colon() { return None }
    if self.intern_is(self.peek_ident()?, "sep") { Some("sep") }
    else if self.intern_is(self.peek_ident()?, "end") { Some("end") }
    else { None }
}
```

`str_literal` returns a `ParseError` when the argument after `sep:`/`end:` is not a string
literal, so `print(1, sep: 2)` fails here with a span — rather than silently printing with the
default separator, which is what the legacy parser does at `src/runtime.rs:1818-1829`.

### 6.3 Command calls: `name expression`

`grammar.md:96-97` promises a command form —

```text
command_call = name expression       (* e.g. `sleep 2`, `exit 1`, `assert cond` *)
```

— and `grammar.md:254-255` adds a restriction in prose: *"Command-style calls (`name expression`)
only work when `name` resolves to a built-in function."*

**That restriction is not enforced anywhere, and cannot be.** The legacy implementation at
`src/runtime.rs:2246-2256` is:

```rust
} else if let Expr::Var(name) = expression {
    // Command-style call: `go "url"`, `wait 6`, `sleep 2`, `exit 1`
    if self.starts_expression() {
        let arg = self.expr()?;
        Ok(mk(StmtKind::Expr(Expr::Call(Box::new(Expr::Var(name)), vec![arg]))))
    } else {
        Ok(mk(StmtKind::Expr(Expr::Var(name))))
    }
}
```

There is no builtin list in that path. Any identifier in statement position, followed by a token
that can begin an expression, becomes a one-argument call. `starts_expression()`
(`src/runtime.rs:1629-1645`) is the *only* gate, and it is a token-kind test, not a name test.

**RULING P6 — a command call is a statement-position `Ident` followed by a value-starting token,
with exactly one argument; the gate is `starts_expression()` minus the two tokens `postfix()`
would already have consumed.**

The subtraction is the whole trick, and it is what makes this decision possible *before* any parsing
rather than after. The legacy code sidesteps the problem by parsing first and inspecting the result
(`if let Expr::Var(name) = expression`) — the `Ident` has already been consumed by `self.expr()?`,
so the decision is made by looking at what came out. A two-token check up front is equivalent and
cleaner:

```rust
// crates/zen-syntax/src/parser/command.rs

/// The token set from `src/runtime.rs:1629-1645` (`starts_expression`), minus `LParen` and
/// `LBracket`. Those two are excluded because `postfix()` (`src/runtime.rs:2771-2835`) consumes
/// them as a call and an index, so by the time the legacy parser asks `starts_expression()`, a
/// `(` or `[` can no longer be sitting there. Reproducing the subtraction reproduces the
/// behaviour exactly, with the decision made before any tokens are consumed.
fn can_begin_command_arg(&self, k: TokenKind) -> bool {
    matches!(k,
        TokenKind::Ident(_)        // `f g`  -> f(g)
      | TokenKind::Num(_)
      | TokenKind::Str(_) | TokenKind::Interp(_)
      | TokenKind::KwTrue | TokenKind::KwFalse | TokenKind::KwNil
      | TokenKind::LBraceDict | TokenKind::LBrace   // `f {a: 1}` -> f({a: 1})
      | TokenKind::KwNot | TokenKind::Bang | TokenKind::Minus | TokenKind::Tilde
      | TokenKind::KwNew
      | TokenKind::KwMatch | TokenKind::KwWhen | TokenKind::KwLambda
    )
}

fn parse_stmt(&mut self) -> Result<Stmt> {
    // One token of lookahead: `peek()` is the `Ident`, `peek_at(1)` is the candidate argument's
    // first token. No scan, no speculative parse, no `save`/`restore`.
    if let TokenKind::Ident(_) = self.peek() {
        if self.can_begin_command_arg(self.peek_at(1)) {
            let name = self.expect_ident()?;
            let arg = self.expr_bp(BP_RANGE)?;
            return Ok(Stmt::CommandCall { name, args: vec![arg], span: /* joined */ });
        }
    }
    …
}
```

Three consequences worth stating, because they are the reasons a builtin table is *not* the
answer:

1. **`f 1` must keep working for a user-defined `f`.** The legacy grammar permits it and the
   runtime does it. A table of builtins would reject `my_func 1` for a `my_func` the user wrote
   two lines above — a compatibility break in a language whose stated goal is that the rewrite
   preserves the corpus. The corpus does not exercise it, but "not in the corpus" is not a
   licence to break syntax.
2. **The builtin *list* is not a parser concern at all.** `print`, `input`, `assert`, `include`,
   `load`, `import`, and `from` have their own productions (or their own token kinds — `"print"`
   is lexed to `Kind::Print` at `src/runtime.rs:1054`), so they are resolved by the `stmt()`
   dispatch, long before `parse_command` is reached. The one place a name list earns its keep is
   **diagnostics**: the fuzzy-match machinery that already exists for builtins
   (`src/runtime.rs:20573-20585`) is what turns `psrint 1` into "did you mean `print 1`?", and
   that fires at name-resolution time, not parse time.
3. **Exactly one argument.** `grammar.md:96` writes `name expression` — singular — and the
   runtime parses exactly one `self.expr()?` at `src/runtime.rs:2250`. `f 1, 2` is therefore *not* a
   command call; the `,` is left where a statement separator is expected and the recovery in §10.2
   reports it. Multi-argument printing is `print`'s job, not the command form's.

**A `CommandCall` node rather than a desugared `Call`.** The legacy path builds
`Expr::Call(Var(name), [arg])` by hand. Keeping `Stmt::CommandCall` in the AST is worth the extra
variant because the M4 compiler can emit the argument into the same register as a single-argument
call and skip the callee-slot dance, and because §6.4's "`print` is not an expression" rule becomes
a structural fact rather than a check on an already-parsed tree. M3 evaluates it identically to a
`Call` on an `Ident`.

### 6.4 `print` in the three positions it is legal in

| Source | Position | Result |
|---|---|---|
| `print(x)` | statement | `Stmt::Print` |
| `print x` | statement | `Stmt::Print` |
| `f(print(x))` | value | **`E0309`** — `print` is a statement, not an expression |
| `x = print` | value | **`E0309`**, not a `NameError` |
| `return print(x)` | value | **`E0309`** |

The legacy runtime agrees on the first two and is *silent* on the rest: `print` is a
`StmtKind::Print` built in `stmt()` at `src/runtime.rs:1809-1844` and there is no `atom()` arm for
it, so a later stage turns the mistake into whatever generic message it has. `print` is a
`TokenKind`, not an `Ident`, so `parse_primary` can reject it by token kind with a caret — which is
strictly better than a downstream "expected expression, found `print`" (T-C7).

---

## 7. Imports and module resolution

### 7.1 The forms

`grammar.md:130-135` defines the surface and `tests/cases/10_import_forms.z:44-53` exercises it.
The entry point is the one real gap, and it is worth being precise about because the evidence
points two ways:

| Evidence | Says |
|---|---|
| `docs/imports.md:153` | `name.z` or `name/main.z` |
| `docs/cli.md:93` | `zen pm init` scaffolds `main.z` |
| `src/runtime.rs:7554-7579` | candidates are `x.z`, `x.zen`, `x/main.z`, `x/main.zen`, `x/<last>.z` |
| `src/runtime.rs:7571` (comment) | *"`pkg.sub.mod` → `pkg/sub/mod.z`"* |
| `tests/cases/pkg/sub/mod.z` | a fixture that is only reachable through the *last* candidate, `x/<last>.z`, with `last == "mod"` |

So `mod.z` in the corpus is a **coincidence of the import spelling**, not an established
convention: `import pkg.sub.mod` tries `pkg/sub/mod.z` because the last path segment happens to be
`mod`. Nothing in the loader has ever preferred it.

**RULING P7 — `mod.z` is canonical for a package directory. `main.z` is accepted for one release
as a deprecated alias, with a warning.** A single file keeps the plain `.z` spelling. This keeps
`tests/cases/pkg/sub/mod.z` resolving without a change and gives the ecosystem a name that cannot
be confused with a program entry point, at the cost of a warning on `main.z` that nobody in the
corpus pays.

All of these are `grammar.md:130-135` verbatim, and all of them are exercised by
`tests/cases/10_import_forms.z:24-53`.

| Form | Example | AST |
|---|---|---|
| plain | `import a.b` | `Import{ModulePath::Dotted(a.b)}` |
| alias | `import a.b as ab` | `Import{alias: Some(ab)}` |
| multi | `import a, b.c as d` | `Import{specs: [a, b.c→d]}` |
| string path | `import "./helpers.z"` | `Import{ModulePath::Relative}` |
| from-names | `from a.b import x, y as z` | `FromImport{items: Names}` |
| from-star | `from a.b import *` | `FromImport{items: Star}` |
| side effect | `include "./setup.z"` · `load "./x.z"` | `Include` |
| dotted-into-a-directory | `import pkg.sub.mod` | resolves to `pkg/sub/mod.z`; **no segment is stripped** |

That last row is not a stylistic choice. The legacy parser builds the full dotted string and stops
(`src/runtime.rs:1998-2010`), and `tests/cases/10_import_forms.z:52-53` depends on the consequence:

```zen
import pkg.sub.mod
check(pkg.sub.mod.util() == "pkg-util", "chained member access pkg.sub.mod.fn")
```

The binding is a **namespace chain** — `pkg`, then `pkg.sub`, then `pkg.sub.mod` — and the last
segment is *not* additionally bound as a bare name. A parser that "helpfully" stripped a trailing
`mod` and bound `pkg.sub` would satisfy `import pkg.sub.mod as m2` at line 48 and then fail at
line 53, which is exactly the kind of half-compatibility that is worse than none.

`Mod` is a keyword-only-at-the-tail case: `import a.b.mod` is legal and means the same as
`import a.b`, because the final segment is dropped before path resolution. This is exactly what
`tests/cases/10_import_forms.z` does, and rejecting it would break the corpus.

### 7.2 The parser produces a *path*, never a file

**The parser never touches the filesystem.** It produces a tree of unresolved `ModulePath`s and
hands it to the resolver. This is not a layering preference — it is what makes `zenn fmt` work on a
file whose imports are all broken, and it is what makes the missing-module diagnostic a *load*
error rather than a *parse* error, which is where a user expects it.

```rust
// crates/zen-syntax/src/parser/imports.rs
fn parse_module_path(&mut self) -> Result<ModulePath> {
    if self.peek_is(TokenKind::Str) {
        let p = self.str_literal()?;
        return Ok(ModulePath::Relative(p.cooked));       // kept verbatim, resolved in §7.3
    }
    // Every segment is kept, including a trailing `mod`: `pkg.sub.mod` must stay three segments
    // so that `pkg.sub.mod.util()` resolves (§7.1). The legacy parser does the same at
    // `src/runtime.rs:1998-2010`.
    let mut parts = vec![self.expect_ident()?];
    while self.eat(TokenKind::Dot) { parts.push(self.expect_ident()?) }
    Ok(ModulePath::Dotted(Path { parts, span: self.last_span() }))
}
```

### 7.3 The resolution order

```rust
// crates/zen-syntax/src/resolve/imports.rs
pub struct Resolver {
    root: PathBuf,                                  // the project root, from `zenn.toml`
    search: Vec<PathBuf>,                           // [root, …, importing_file.parent()]
    loaded: HashMap<PathBuf, FileId>,
    cache: HashMap<PathBuf, Arc<ModuleAst>>,        // so `import a` twice parses once
}

impl Resolver {
    /// For the dotted path `a/b/c`, try in order and take the first that exists:
    ///   1. <root>/a/b/c.z          a single file
    ///   2. <root>/a/b/c/mod.z      a package directory, canonical  (RULING P7)
    ///   3. <root>/a/b/c/main.z     a package directory, legacy alias — emits a warning
    /// For a relative or absolute string path, resolve against the importing file's directory
    /// first, then the root, trying `<p>`, `<p>.z`, `<p>/mod.z`, `<p>/main.z`.
    pub fn resolve(&mut self, m: &ModulePath, from: FileId) -> Result<FileId, ResolveError>;
}
```

The order is chosen so all four corpus shapes keep working with no change:

| Import | Tries | Lands on |
|---|---|---|
| `import pkg.sub.mod` | `pkg/sub/mod.z` | step 1 — one `stat` |
| `import pkg.sub` | `pkg/sub.z`, `pkg/sub/mod.z` | step 2 — two `stat`s |
| `import json as j` | `json.z` | step 1 |
| `import "./helpers.z"` | `<dir>/helpers.z` | step 1, relative to the importer |

`main.z` is third, not first, so a project that migrates by adding `mod.z` gets the canonical name
with no `main.z` warning; a project that has not migrated still resolves, and gets one warning that
names the file to rename. `.zen` is dropped from the candidate list — the legacy loader tries
`x.zen` at five separate places (`src/runtime.rs:7557-7579`, `7604-7621`) and Zen 2.0 has no
reader for it in this corpus; keeping a dead extension in the search path is how a stale file
silently shadows a live one.

Reporting **every attempted path** in the error is worth the extra line:

```text
error: cannot resolve module `utils.text`
  --> /proj/app.z:1:1
   |
 1 | import utils.text
   | ^^^^^^^^^^^^^^^^
   |
   = tried /proj/utils/text.z
   = tried /proj/utils/text/mod.z
   = tried /proj/utils/text/main.z
   = note: a directory package needs a `mod.z`; a single file needs the `.z` suffix
```

### 7.4 Binding and the `pub` gate

`import a.b` binds the namespace chain `a` → `a.b`, and **not** a bare `b`
(`tests/cases/10_import_forms.z:52-53` is the proof). `import a.b as x` binds the flat name `x` to
the module value, so `x.util()` works and `a.b.util()` does not. `from a.b import x` binds `x`.
Every one of those bindings is checked for a collision in the enclosing scope at resolve time, and
the check is a **compile-time error**, not a runtime one:

```text
error: `text` is already bound in this scope
  --> /proj/app.z:12:1
   |
12 | import utils.text
   |            ^^^^^
   |
   = previous binding at /proj/app.z:3:8
   = help: use `as text2`, or drop one of the two imports
```

`from m import *` binds every **public** member of `m` and nothing else, which makes the visibility
rule in §8 observable at the import site rather than only inside the module.

### 7.5 Cycles

`a.z` importing `b.z` importing `a.z` is a **compile-time error**, not a hang and not a lazy
partial module. Detection is a three-colour DFS over the `loaded` graph:

```text
error: import cycle: app.z -> util.z -> app.z
   |
 1 | import util          # app.z
   |          ^^^^
   |          └── imported by util.z:1, which is imported by app.z
   = note: break the cycle by moving the shared code into a third module, or by inlining it
```

The alternative — allowing a cycle and returning a partially-initialised module — is how the legacy
runtime got away with it, and the cost is that every function in the cycle sees a half-built
global table with no diagnostic explaining why. Rejecting the cycle means the alternative exists
without a hazard. This is the same argument as
[`decisions/ADR-0004-honest-types.md`](decisions/ADR-0004-honest-types.md): a value that cannot be
meaningfully produced should not be produced silently.

---

## 8. `pub` visibility

### 8.1 The syntax

Review §6 adds `pub` to `let`, `var`, `const`, `function`, and `class` bindings. The placement is
**prefix**, matching the existing `global`/`native` prefixes, and the token is a distinct
`TokenKind::KwPub` so the lexer can colour it for the highlighter and the parser can reject
`pub` in expression position without a keyword check.

```text
declaration = [ "pub" ] [ "global" | "shared" | "const" | "let" | "var" ] identifier [ "=" expression ]
function    = [ "pub" ] [ "native" ] "function" identifier "(" params ")" [ block ]
class       = [ "pub" ] "class" identifier [ "extends" path ] block
```

**RULING P8 — the declaration is `pub var`, `pub function`, `pub class`, never `var pub`.** The
baseline already uses prefix modifiers in exactly this way: `global target "=" expression`
(`grammar.md:75`, an alias for `var` per `grammar.md:258-260`) and
`native "function" name "(" params ")"` (`grammar.md:115`). A suffix form would need a separate
check in each of the five declaration parsers and would have to be rejected *after* the value was
parsed; a prefix form is one optional token consumed before anything else, and `pub pub` falls out
of the same test for free.

### 8.2 What `pub` does, and where it is checked

`Vis::Public` is recorded on the AST node and enforced **at the import site**, in the resolver,
never at runtime. A `pub` on a module-local name is legal and is a no-op.

| Declaration | Default | With `pub` |
|---|---|---|
| `let x = 1` | private | exportable |
| `var x = 1` | private | exportable |
| `const x = 1` | private | exportable |
| `shared x = 0` | private | exportable; the *value* is a mutex, not shared state |
| `function f() {}` | private | exportable |
| `class C {}` | private | exportable |
| anything inside a function body | always private | `pub` is a **warning**, not an error |

That last row is the ruling worth arguing for. `pub` inside a function has no meaning — nothing is
imported from inside a function — so silently accepting it would teach the wrong mental model.
Rejecting it outright would break a file that uses `pub` as a matter of habit. A warning does both
jobs:

```text
warning: `pub` has no effect here; it is only meaningful on a module-level declaration
  --> /proj/a.z:4:5
   |
 4 |     pub let x = 1
   |     ^^^
   |
   = note: this `x` is local to `f` and cannot be imported
```

### 8.3 The import-side check

```rust
// crates/zen-syntax/src/resolve/visibility.rs
/// `import m.x` and `from m import x` both land here. `pub` on the declaration is the only
/// thing that makes a name importable; there is no `__all__` and no re-export form in M1-M8.
fn check_importable(&self, decl: &Decl, site: Span) -> Result<(), ResolveError> {
    if decl.vis == Vis::Private {
        return Err(ResolveError::NotPublic {
            name: decl.name,
            declared_in: decl.span,
            site,
        });
    }
    Ok(())
}
```

```text
error: `helper` is not public in `utils`
  --> /proj/app.z:3:16
   |
 3 | from utils import helper
   |                ^^^^^^
   |
   = declared private at /proj/utils.z:2:1
   = help: add `pub` to the declaration in utils.z, or import the module and use `utils.helper`
   =       if the helper is only used inside utils.z, no change is needed
```

The `help` line matters because the two fixes are not equivalent: adding `pub` widens the module's
API, while `import utils` and qualifying keeps it private. Offering the second as an equal
alternative is the difference between a helpful diagnostic and one that pushes the user toward a
worse design.

### 8.4 Why the check is compile-time and not a runtime error

An M3 tree-walker *could* look up an imported name and fail with `NameError: 'helper' is not
public` at run time. It must not, for the same reason §6.3 does not defer the command table: a
visibility violation is a property of the source text, knowable at parse-and-resolve time, and
deferring it means the error appears only if that line executes. Compile-time means `zenn check`
finds it, which is the whole point of having a `check` subcommand.

---

## 9. Pattern matching

### 9.1 What a pattern is

Patterns appear in four places: `match` arms, `switch` cases, `for` loop variables, and
destructuring `let`. They are a **separate sub-grammar** with its own `Pattern` type, not a
flagged `Expr`. Sharing the type would be a lie about which expressions are legal: `1 + 1` is an
expression and is not a pattern, `a | b` is a pattern and is not an expression, and a
`?`-form expression is neither.

```rust
// crates/zen-syntax/src/ast/pattern.rs
#[derive(Clone, Debug, PartialEq)]
pub enum Pattern {
    /// `_` or a name the user does not want. `name` is `None` for `_`.
    Wildcard(Span),
    /// A binding. Same source text as `Wildcard`; the parser decides which, not the lexer.
    Binding { name: Symbol, span: Span },
    /// A literal: number, string, `true`, `false`, `nil`.
    Literal { value: LiteralValue, span: Span },
    /// `Path(a, b)` — an enum case, a class destructuring pattern, or a class-pattern match.
    Constructor { path: Path, fields: Vec<Pattern>, span: Span },
    /// `[a, b, c]` — list destructuring.
    List { elems: Vec<Pattern>, rest: Option<Box<Pattern>>, span: Span },
    /// `#{k: v}` / `{k: v}` / `Name(a, b)` — dict destructuring.
    Dict { entries: Vec<(Pattern, Pattern)>, rest: Option<Box<Pattern>>, span: Span },
    /// `Some(a)` shorthand for a one-field constructor; also the enum-case `Some(x)` form.
    /// A bare path with no parens and no fields, e.g. `None` or `red`, is `Constructor` with
    /// `fields: vec![]` — a value pattern, tested by equality.
    /// **Bare `Name { … }` is NOT a class pattern.** See RULING P9.
    Range { lo: Box<Expr>, hi: Box<Expr>, lo_incl: bool, hi_incl: bool, span: Span },
    /// `x if p(x)` / `x when p(x)` / `x if p(x) else q(x) else r(x)` — chained guards.
    Guard { pat: Box<Pattern>, guards: Vec<Expr>, span: Span },
    /// `a | b` — an or-pattern. `None | nil` is the canonical example.
    Or { alts: Vec<Pattern>, span: Span },
    /// `x as n` — bind the matched value under a new name (post-match binding).
    Bind { pat: Box<Pattern>, name: Symbol, span: Span },
    /// `#(a, b)` — tuple destructuring.
    Tuple { elems: Vec<Pattern>, span: Span },
}
```

### 9.2 Where the baseline grammar contradicts itself

Four separate conflicts, all in the pattern area, all resolvable in favour of the runtime because
the runtime is what the corpus actually runs.

**Finding F-1 — rest patterns.** `grammar.md:77-80` writes the array pattern with an optional
rest element:

```text
pattern       = "[" pattern_item? ("," pattern_item)* ","? "]"
pattern_item  = name | "..." name       (* rest pattern: `...name` collects remaining elements *)
```

`grammar.md:281-282` then says, in the same document, that *"**Rest patterns** (`...rest`) are not
supported."* The runtime implements it — `PatternItem::Rest` at `src/runtime.rs:1462-1470`, with an
explicit error for a *nested* rest at `src/runtime.rs:1477`. **Ruling: rest patterns are
supported** in list, tuple, and dict destructuring; `grammar.md:281-282` is wrong.

Honesty note: **no file in `tests/cases/` uses a rest pattern.** The ruling rests on the grammar
production and the runtime implementation agreeing with each other against one stale prose line, and
T-M5 is a *new* test that pins the behaviour rather than a corpus regression. That is a weaker
basis than F-2 has, and it is stated here rather than glossed over.

**Finding F-2 — compound assignment and `++`/`--`.** `grammar.md:84-89` gives the full `assign`
production, including every compound operator and the postfix `++`/`--`, with an explicit comment
that *"compound ops and ++/-- work for name targets, member targets (`o.n++`), and index targets
(`l[1]++`)"*. `grammar.md:283-284` says the opposite: *"Index assignment `l[0] = 9` and dict
assignment `d["k"] = 7` work; compound assignment (`l[0] += 1`) and `++`/`--` through indexes or
members do not."* The runtime implements all of it — compound assignment on an index at
`src/runtime.rs:2209-2229`, on a member at `src/runtime.rs:2230-2246`, and postfix `++`/`--` at
`src/runtime.rs:2831-2835`. **Ruling: all supported**; `grammar.md:283-284` is a wish-list item
that was never removed from a reference document. Unlike F-1 this one is low-risk: the
implementation, the production, and the prose all name the same operators.

**Finding F-3 — `if` is both a ternary and a match guard.** `grammar.md:274` writes match guards
as `pattern if expression`, and `grammar.md:186-187` writes the Python ternary as
`expression if expression else expression`. The two are in different positions, so the parser
disambiguates by position: `parse_match_arm` calls `parse_pattern` (which never produces an
`Expr::Ternary`), and a guard is parsed as a full `expr_bp(0)` after the `if`. **Ruling: `if`,
`when`, and `&&` are all accepted as guard introducers**, with `if` canonical.

**Finding F-4 — a range is a range, and match arms take `->`/`=>`/`:`.** `grammar.md:144` writes
`match_arm = pattern ("if" expression)? (":" | "=>") (statements | block)` — so the baseline grammar
has **no** `->` separator at all, only `:` and `=>` — and the worked examples at
`grammar.md:272-276` use both (`1: "one"` and `x if x > 5 => "big"`). Review §6 adds `->`.
**Ruling: all three separators are accepted**, with `->` first in the alternation because it is the
one the new syntax leads with. `grammar.md:144` is a narrowing of what the language actually does.

Because `->` is one token with two readings (RULING P4), `Pattern::Range` is **not** constructible
from a `match` arm; a range is only reachable in a `switch` case, which parses an `Expr`
(`grammar.md:139-141`).

### 9.3 `match` parsing

```rust
// crates/zen-syntax/src/parser/match.rs
fn parse_match(&mut self) -> Result<Expr> {
    let start = self.expect(TokenKind::KwMatch, "`match`")?;
    let subject = Some(Box::new(self.expr_bp(BP_RANGE)?));
    let arms = self.parse_arms(ArmSep::Match)?;
    Ok(Expr::Match { subject, arms, span: start.join(self.last_span()) })
}

fn parse_arms(&mut self, sep: ArmSep) -> Result<Vec<MatchArm>> {
    self.expect(TokenKind::LBrace, "`{` to open a match body")?;
    let mut arms = Vec::new();
    while !self.peek_is(TokenKind::RBrace) && !self.at_eof() {
        let pat_start = self.peek_span();
        let mut pat = self.parse_pattern()?;
        // Guard chain: `if` / `when` / `&&`, each tested in order, first success wins.
        let mut guards = Vec::new();
        while self.peek_is(TokenKind::KwIf) || self.peek_is(TokenKind::KwWhen)
            || self.peek_is(TokenKind::AmpAmp)
        {
            self.bump();
            guards.push(self.expr_bp(0)?);
        }
        if !guards.is_empty() { pat = Pattern::Guard { pat: Box::new(pat), guards, span: pat.span } }

        let body = match sep {
            ArmSep::Match => {
                // `->` `=>` `:` all separate; a block body after `->` is a BLOCK (§5.2),
                // because parse_block is called directly, never through parse_primary.
                if !self.eat(TokenKind::Arrow) && !self.eat(TokenKind::FatArrow)
                    && !self.eat(TokenKind::Colon)
                { return Err(self.error_here("expected `->`, `=>`, or `:` after a match arm pattern")) }
                if self.peek_is(TokenKind::LBrace) { BlockOrExpr::Block(self.parse_block()?) }
                else { BlockOrExpr::Expr(self.expr_bp(BP_RANGE)?) }
            }
            ArmSep::Switch => {
                self.expect(TokenKind::Colon, "`:` after a `case` value")?;
                BlockOrExpr::Block(self.parse_block()?)
            }
        };
        arms.push(MatchArm { pattern: pat, body, span: pat_start.join(self.last_span()) });
        // Commas between arms are optional, matching `grammar.md:272-276`.
    }
    self.expect(TokenKind::RBrace, "`}` to close a match body")?;
    Ok(arms)
}
```

**Block arms versus expression arms, in a match.** After `->`, a `{` is a **block**, even though
`{` in a value position is a dict (§5.2) — because `parse_block` is called directly. The reason
is that a match arm is a *statement* position, which is the same rule that makes `if c { }` a
block if. Consistency is worth more than surprise here: "statement position means `{` is a block"
is a single sentence the user learns once.

### 9.4 Destructuring `let`

```rust
// crates/zen-syntax/src/parser/bind.rs
/// `let [a, b] = …`, `let #{x: p, y: q} = …`, `let Some(v) = …`, `let (a, b) = …`.
/// A rest element is allowed in exactly one position per container, and only in `[…]` and
/// `#{…}` (Finding F-1). The parser enforces "one rest, not first" with a position check —
/// no lookahead scan.
fn parse_binding_target(&mut self) -> Result<BindingTarget> {
    match self.peek() {
        TokenKind::LBracket | TokenKind::LBraceDict | TokenKind::LBrace
        | TokenKind::HashLParen | TokenKind::LParen | TokenKind::CapitalIdent(_) => {
            Ok(BindingTarget::Pattern(self.parse_pattern()?))
        }
        _ => Ok(BindingTarget::Name(self.expect_ident()?)),
    }
}
```

`{a, b} = pair` is the one destructuring form that uses the deprecated braces, because a tuple
pattern is spelled `#(a, b)`. That is a consequence of RULING P7 and it is called out in the
migration notes.

### 9.5 Or-patterns and binding consistency

`None | nil` is the motivating example, and it creates a real constraint: **every alternative of
an or-pattern must bind exactly the same names.** Checking that is a resolver job, not a parser
job, because it needs the whole pattern:

```rust
// crates/zen-syntax/src/resolve/patterns.rs
/// `pat { a }` and `pat { b }` must agree on the bound set. This is the same rule that makes
/// `x { y } if y > 0` illegal in Rust, and it is a *compile* error here, not a runtime surprise.
fn check_pattern_arity(pat: &Pattern) -> Result<BTreeSet<Symbol>, ResolveError>;
```

```text
error: or-patterns must bind the same names in every alternative
  --> /proj/a.z:6:12
   |
 6 |     Some(x) | None => x
   |            ^   ^ no binding
   |
   = `Some(x)` binds `x`; `None` binds nothing
   = help: use `Some(x) | Some(y)` and ignore one of them, or match on the option directly
```

The `help` line is a bit blunt, but the error is real: without it, `x` would be
conditionally-unbound, and a language that has a `let` also has the `NameError` for it.

---

## 10. Error recovery

### 10.1 The rule

**A parse error never aborts the file.** The parser records the diagnostic, skips to the next
statement boundary, and continues. The result is a `Parsed` value carrying a `Vec<Stmt>` that
includes the statements it *did* understand, plus a non-empty `diagnostics` list. `zenn check`
prints all the errors and exits non-zero; the M3 interpreter refuses to run a file with any
`Severity::Error` diagnostic, so a partially-parsed file is never executed.

This is what makes the difference between "three errors, here they are" and "one error, fix it,
run again, find the next one" on a 200-line file. The legacy runtime has no recovery at all: every
parse function returns `Result<_, String>` and every failure is a bare message
(`src/runtime.rs:1649`, `:1664`, `:2599`), so the first bad statement aborts the whole file and the
user sees exactly one problem per run. There are 360 such `Err("…")` sites in the file.

### 10.2 The boundary set

```rust
// crates/zen-syntax/src/parser/recovery.rs
impl<'a> Parser<'a> {
    /// Skip forward until a token that can only start a statement, or a closing brace of the
    /// block we are currently inside. Counts nesting so a `}` inside a nested block does not
    /// end recovery early.
    pub fn recover_to_stmt_boundary(&mut self, err: ParseError) {
        self.errors.push(err);
        if self.errors.len() >= MAX_PARSE_ERRORS { return }     // 64; §13
        let mut depth = 0usize;
        while !self.at_eof() {
            match self.peek() {
                TokenKind::RBrace if depth == 0 => return,
                TokenKind::RBrace | TokenKind::RParen | TokenKind::RBracket | TokenKind::RBraceDict
                    => { depth -= 1; self.bump(); }
                TokenKind::LBrace | TokenKind::LParen | TokenKind::LBracket | TokenKind::LBraceDict
                    => { depth += 1; self.bump(); }
                TokenKind::KwIf | TokenKind::KwWhile | TokenKind::KwFor | TokenKind::KwReturn
                | TokenKind::KwLet | TokenKind::KwConst | TokenKind::KwVar | TokenKind::KwFunction
                | TokenKind::KwFunc | TokenKind::KwFn | TokenKind::KwDef | TokenKind::KwClass
                | TokenKind::KwImport | TokenKind::KwMatch | TokenKind::KwTry | TokenKind::KwWith
                | TokenKind::KwBreak | TokenKind::KwContinue | TokenKind::KwThrow
                | TokenKind::KwSwitch | TokenKind::KwInclude | TokenKind::KwLoad
                | TokenKind::KwPub | TokenKind::KwNative
                    if depth == 0 => return,                        // next statement starts here
                TokenKind::Newline if depth == 0 => { self.bump(); return }
                _ => { self.bump(); }
            }
        }
    }
}
```

The newline rule is what does most of the work, and it is why
[`05-lexer.md`](05-lexer.md) RULING L3's ASI set matters: `x = (1 +\n 2)` must **not** produce a
newline token inside the parens, or recovery would cut the statement in half. The lexer's
delimiter suppression and the parser's recovery set are the same fact viewed from two ends.

### 10.3 The error budget

`MAX_PARSE_ERRORS = 64`. Past that, the parser stops reporting and records one final
`"further errors suppressed"` note. A file with 64 errors is not a file a human is going to fix
one caret at a time; it is usually a file with a structural problem (a stray `}` in the wrong
place, a heredoc-looking string, a file that is not Zen at all). The cap keeps the diagnostic
output bounded and stops a pathological input from turning the parser into the hot loop.

No file in `tests/cases/` contains more than a handful of errors — the 33 files are all
positive-expectation suites, 84,611 bytes in total — so the cap is never approached by the corpus. A
test constructs a 200-error file to check it (T-R4).

### 10.4 Every error carries a span and a help line

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ParseError {
    pub severity: Severity,          // always Error here; Warning is on the Diagnostic
    pub code: ErrorCode,             // stable identifier, see §10.5
    pub message: String,             // one line, no trailing period
    pub span: Span,                  // the token that was wrong, not the line
    pub help: Option<String>,        // the fix, if there is a mechanical one
    pub notes: Vec<(Span, String)>,  // "declared here", "previous binding at …"
}
```

The `span` is the **token**, not the line, so the caret lands under the actual mistake:

```text
error[E0301]: expected `:` after a dict entry key
  --> /proj/a.z:7:14
   |
 7 |     {"a" 1, "b": 2}
   |          ^
   |
   = note: `{…}` dict literals are deprecated; use `#{…}` (warning on line 1)
   = help: `#{…}` has the same syntax and is unambiguous
```

**One exact edit position per error.** The legacy parser's `Stmt` carries a `line`/`col`
(`src/runtime.rs:1395-1400`) but `Expr` carries nothing at all, so a failure inside an expression
has nowhere to point. Two of its messages show the shape of the problem:
`src/runtime.rs:1481` returns `"expected variable name or nested list pattern"` and
`src/runtime.rs:2222` returns `"invalid compound assignment target"` — both with no file, no line, no column, and no way for
`zenn` to attach a caret. An error without a location is not a diagnostic, it is a complaint.

### 10.5 Error codes

Stable identifiers, so a tool can suppress one without matching on English text. The full table
is in [`05-lexer.md`](05-lexer.md) §10 for the lexer's half; these are the parser's.

| Code | Meaning | Example message |
|---|---|---|
| `E0300` | unexpected token, no context | `unexpected `}` — a block or a dict was expected to be open` |
| `E0301` | expected a specific token | `expected `:` after a dict entry key` |
| `E0302` | reserved token used | `..= is reserved; write `a -> b` for an inclusive range` |
| `E0303` | not a pattern | `expected a pattern; `1 -> 2` is a range, not a pattern` |
| `E0304` | `if` used as an expression with block arms | `` `{` in expression position is a dict literal `` |
| `E0305` | `pub` with no effect | `pub` has no effect inside a function body |
| `E0306` | or-pattern binding mismatch | or-patterns must bind the same names in every alternative |
| `E0307` | too many binding names for a pattern | `expected at most 1 name, found 3` |
| `E0308` | dict key is not name/string | `a dict key must be a name or a string; use `[expr]: v` |
| `E0309` | `print` used as an expression | `` `print` is a statement and has no value `` |
| `E0310` | command call with a trailing comma | `` a command call takes exactly one argument; use `print a, b` `` |
| `E0311` | reserved for `use` | — |
| `E0312` | interpolation body did not parse | `unexpected `}` in an interpolation body` |
| `E0313` | rest element in the wrong position | `a rest element must be last and may appear only once` |
| `E0314` | ternary nesting | `the condition of `? :` may not contain `? :`; add parentheses` |

Nothing in this table fires for a misspelled *builtin*: `psrint 1` parses as a `CommandCall`
(RULING P6) and the fuzzy-match happens later, at name resolution, because only the resolver knows
which names are in scope. The existing machinery at `src/runtime.rs:20573-20585` already does this
for the help system, and the rewrite reuses it rather than duplicating it in the parser.

---

## 11. Implementation plan

### 11.1 Module layout

```text
crates/zen-syntax/
├── Cargo.toml
├── src/
│   ├── lib.rs               # `pub mod ast; pub mod parse; pub mod resolve;`
│   ├── span.rs              # Span, FileId, LineIndex, SourceMap  (§3)
│   ├── diagnostics.rs       # Diagnostic, Severity, ParseError, ErrorCode, help lines
│   ├── symbol.rs            # interner for the parser's own literals
│   ├── ast/
│   │   ├── mod.rs           # Expr, Stmt, Block, Vis, BindKind, supporting types (§2)
│   │   ├── expr.rs
│   │   ├── stmt.rs
│   │   └── pattern.rs       # Pattern, PatternError (§9)
│   ├── parse/
│   │   ├── mod.rs           # Parser, parse_module, parse_expr, parse_stmt
│   │   ├── primary.rs       # literals, `[…]`, `#{…}`, `%{…}`, `#(…)`, `(…)`, legacy `{…}` (§5.3)
│   │   ├── postfix.rs       # `.` `?.` `[i]` `[a:b]` `(args)` `++` `--` (bp 80)
│   │   ├── infix.rs         # peek_infix, expr_bp, make_binary (§4.4)
│   │   ├── ternary.rs       # parse_ternary, TernaryStyle (§4.4)
│   │   ├── stmt.rs          # parse_stmt, parse_block, if/while/for/try/with (§5.3)
│   │   ├── bind.rs          # let/const/var/shared/global, parse_binding_target (§9.4)
│   │   ├── import.rs        # parse_module_path, import/from/include (§7)
│   │   ├── match.rs         # parse_match, parse_arms, parse_pattern (§9.3)
│   │   ├── print.rs         # parse_print, sep/end (§6.2)
│   │   ├── command.rs       # can_begin_command_arg, parse_command (§6.3)
│   │   ├── recovery.rs      # recover_to_stmt_boundary, the boundary set (§10.2)
│   │   └── legacy_dict.rs   # legacy_dict_literal, dict_key, DictWarnings (§5.3, §5.5)
│   └── resolve/
│       ├── mod.rs           # Resolver, ModuleTable
│       ├── imports.rs       # path resolution, the three-step order, cycles (§7.3, §7.5)
│       ├── visibility.rs    # the `pub` gate (§8.3)
│       ├── patterns.rs      # or-pattern arity, binding sets (§9.5)
│       └── scope.rs         # lexical scopes, shadowing, binding collisions
└── tests/
    ├── parse_ok.z           # golden AST dumps for valid files
    ├── parse_err.z          # golden diagnostics for invalid files
    └── corpus.rs            # the 84 KB conformance corpus, parse-only, no execution
```

Line budget per file, against the 1,770 lines the parser occupies today inside `src/runtime.rs`
(`Parser::new` at `:1443` through the end of `atom()` at `:3192`, of which `stmt()` alone is 690):

| File | Lines | Notes |
|---|---|---|
| `parse/mod.rs` | 320 | the `Parser` struct, token access, `parse_module` |
| `parse/primary.rs` | 260 | every literal and collection form |
| `parse/infix.rs` | 210 | the precedence table and `make_binary` |
| `parse/postfix.rs` | 190 | member/index/slice/call chains |
| `parse/stmt.rs` | 380 | every statement form |
| `parse/match.rs` | 240 | patterns are deceptively large |
| `parse/bind.rs` | 150 | |
| `parse/import.rs` | 170 | |
| `parse/print.rs` | 120 | |
| `parse/command.rs` | 90 | |
| `parse/ternary.rs` | 80 | |
| `parse/recovery.rs` | 110 | |
| `parse/legacy_dict.rs` | 100 | |
| `ast/*` | 520 | enums and the supporting types |
| `resolve/*` | 610 | imports, visibility, patterns, scopes |
| **total** | **≈ 3,550** | vs ≈1,770 today; the increase buys error recovery, a span on every node, a resolver, and the visibility and or-pattern checks — none of which exist in the legacy parser |

### 11.2 The `Parser` struct

```rust
// crates/zen-syntax/src/parse/mod.rs
pub struct Parser<'a> {
    toks: &'a [Token],          // borrowed, never copied; the token stream is immutable
    pos: usize,                 // index into `toks`
    file: FileId,
    symbols: &'a mut SymbolTable,
    source: &'a str,            // for `Span::text` in diagnostics
    errors: Vec<ParseError>,
    warnings: Vec<Diagnostic>,
    dict_warnings: DictWarnings,
    /// Guards against unbounded recursion on `((((((…`. Depth is checked at every
    /// `parse_primary` entry, not by the OS stack, so a stack overflow is a diagnostic and
    /// not a SIGSEGV.
    depth: u16,
    budget: StepBudget,         // §13
}

impl<'a> Parser<'a> {
    pub fn parse_module(&mut self) -> Parsed;
    fn peek(&self) -> TokenKind { self.toks.get(self.pos).map_or(TokenKind::Eof, |t| t.kind) }
    fn peek_at(&self, n: usize) -> TokenKind { /* `peek` + n, clamped to Eof */ }
    fn peek_span(&self) -> Span { /* Span::new(self.file, t.lo, t.hi) */ }
    fn bump(&mut self) -> TokenKind;
    fn eat(&mut self, k: TokenKind) -> bool;
    fn expect(&mut self, k: TokenKind, ctx: &str) -> Result<Span, ParseError>;
    fn error_here(&self, msg: &str) -> ParseError;
}
```

**`&'a [Token]` and `&'a mut SymbolTable` is the whole borrow story.** The token slice is
immutable and shared, so no interior mutability is needed anywhere in the parser. The legacy parser
needs `&mut self` on all 30-odd of its functions for the same reason, and needs the tokens in a
`Vec<Token>` it can `advance()` through rather than a slice it can index. A 22,000-line file with
zero `Rc`, zero `RefCell`, and 22 `Arc` is evidence that a shared borrow plus a `usize` is
sufficient; the rewrite just makes it the only option.

### 11.3 Build order

| Step | File(s) | Depends on | Done when |
|---|---|---|---|
| 1 | `span.rs`, `diagnostics.rs` | — | `Span::text`, `join`, and a golden caret render test pass |
| 2 | `symbol.rs` | — | `Symbol` is `u32`, interning is a `HashMap<&str, u32>` |
| 3 | `ast/*` | 1, 2 | compiles; a `#[test]` builds one node of each variant by hand |
| 4 | `parse/mod.rs`, `primary.rs` | 3 | `1`, `"a"`, `[1, 2]`, `#{k: 1}`, `%{1}`, `#(1, 2)` parse |
| 5 | `parse/infix.rs`, `ternary.rs`, `postfix.rs` | 4 | the full §4.1 table round-trips; T-P1..T-P16 pass |
| 6 | `parse/stmt.rs`, `bind.rs` | 5 | every `Stmt` variant parses |
| 7 | `parse/match.rs` | 5, 6 | `match`, `switch`, and every `Pattern` variant parse |
| 8 | `parse/import.rs`, `print.rs`, `command.rs`, `legacy_dict.rs` | 6 | §5–§7 behaviours |
| 9 | `parse/recovery.rs` | 6 | a 200-error file reports 64 and keeps parsing |
| 10 | `resolve/*` | 3 | imports resolve, `pub` is enforced, cycles are rejected |
| 11 | `tests/corpus.rs` | 10 | all 84 KB of `tests/cases/*.z` parses with 0 errors and 0 unexpected warnings |

### 11.4 Dependencies

```toml
[dependencies]
zen-stdlib = { path = "../zen-stdlib", optional = true }   # NOT used by the parser
serde = { version = "1", features = ["derive"], optional = true }   # optional AST dump
```

**The parser depends on nothing but `std`.** Not on `zen-vm`, not on `zen-runtime`, not on
`serde`. This is what keeps `zenn fmt` and `zenn check` free of the VM, the native module table,
and the Cranelift feature flags, and it is the structural reason the NaN-boxed `Value` cannot leak
into `zen-syntax` even by accident (§2.1). `serde` is behind a `dump-ast` feature, used only by
`corpus.rs` to write golden AST JSON.

---

## 12. Test plan

Tests are Zen files under `crates/zen-syntax/tests/parse/` plus a table of Rust unit tests. Every
ruling in §4.2, §5, §6, §7, §8, and §9 has at least one named test.

### 12.1 Unit tests, by function

| id | Test | Asserts |
|---|---|---|
| T-P1 | `prec_add_mul` | `1 + 2 * 3` → `Add(1, Mul(2,3))` |
| T-P2 | `prec_sub_div` | `1 - 2 / 3` → `Sub(1, Div(2,3))` |
| T-P3 | `prec_left_assoc` | `1 - 2 - 3` → `Sub(Sub(1,2),3)` = `-4`; catches the `lbp <= min_bp` bug |
| T-P4 | `prec_range_loosest` | `a -> b if c else d` → `Ternary(cond=c, then=Range(a,b), else=d)` (RULING P1/P2) |
| T-P5 | `prec_range_in_cond` | `a if 1..5 else b` → cond is the `Range` |
| T-P6 | `prec_range_no_left` | `1..2..3` → left-assoc `Range(Range(1,2),3)`; runtime answer is a `RangeError` |
| T-P7 | `ternary_else_associates` | `a ? b : c ? d : e` → right-assoc |
| T-P8 | `ternary_cond_rejects_nested` | `a ? b ? c : d : e` → `E0314` (RULING P2) |
| T-P9 | `prec_nullish` | `a ?? b or c` → `Or(Nullish(a,b), c)`; nullish binds tighter than `or` |
| T-P10 | `prec_nullish_with_or` | `a or b ?? c` → `Nullish(Or(a,b), c)` |
| T-P11 | `prec_and_or` | `a and b or c` → `Or(And(a,b), c)` |
| T-P12 | `prec_bitwise` | `a | b ^ c & d` → `BitOr(a, BitXor(b, BitAnd(c,d)))` |
| T-P13 | `not_is_unary` | `not a and b` → `And(Not(a), b)`, i.e. `(not a) and b` (RULING P3) |
| T-P14 | `not_vs_compare` | `not a == b` → `Not(Binary(Eq, a, b))` |
| T-P15 | `shift_below_add` | `a + b << c` → `Shl(Add(a,b), c)` |
| T-P16 | `pow_right_assoc` | `2 ** 3 ** 2` → `Pow(2, Pow(3,2))`; the M3 answer is 512 |
| T-P17 | `pow_binds_tighter_than_neg` | `-2 ** 2` → `Neg(Pow(2,2))` = `-4` |
| T-P18 | `pow_rhs_is_unary` | `2 ** -1` → `Pow(2, Neg(1))` = `0.5` |
| T-P19 | `chain_comparison` | `1 < x < 10` → `Chain{first: 1, rest: [(Lt,x),(Lt,10)]}` |
| T-P20 | `postfix_left_assoc` | `a.b.c[0].d()` nests to the left |
| T-P21 | `index_vs_slice` | `a[1]` is `Index`; `a[1:2]` is `Slice`; `a[:2]` has `lo: None` |
| T-P22 | `spread_arg` | `f(...xs)` → `Arg::Spread` (Finding F-6) |
| T-P23 | `arrow_range_vs_match` | `1 -> 2` in an expr is `Range`; `1 -> "x"` in a match arm is a `Literal` pattern |
| T-P24 | `reserved_dot_dot_eq` | `a ..= b` → `E0302` |

### 12.2 `{` and dict

| id | Test | Asserts |
|---|---|---|
| T-D1 | `dict_new_sigil` | `#{a: 1}` → `DictLiteral`, no warning |
| T-D2 | `dict_legacy_warns` | `{a: 1}` in value position → `DictLiteral` + one `Warning` with a span |
| T-D3 | `dict_in_stmt_is_block` | `{ x = 1 }` as a statement → a `Block`, no warning |
| T-D4 | `dict_no_backtrack` | the parser never calls `save`/`restore`; a 40,000-token file of `{a: {b: {…` parses without backtracking (asserts a counter) |
| T-D5 | `dict_comprehension` | `#{k: v for x in xs}` → `DictLit{comprehension: Some(..)}` |
| T-D6 | `dict_computed_key` | `{[f(x)]: 1}` → `E0308`, with the `[expr]: v` help line |
| T-D7 | `dict_spread` | `#{...a, b: 1}` → `DictEntry::Spread` |
| T-D8 | `dict_after_if_expr` | `x = if c { 1 } else { 2 }` → `E0304` with the §5.4 help text |
| T-D9 | `dict_match_arm_is_block` | `match x { Some(y) -> { print y } }` → a `Block` body, no dict warning |
| T-D10 | `dict_set_tuple_sigil_free` | `%{1, 2}` and `#(1, 2)` never consult the position rule at all |

### 12.3 `print` and command calls

| id | Test | Asserts |
|---|---|---|
| T-C1 | `print_paren` | `print(1)` → `Stmt::Print{args: [1]}` |
| T-C2 | `print_bare` | `print 1` → the same AST as T-C1 |
| T-C3 | `print_sep_end` | `print(1, 2, sep: "-")` → `sep: Some("-")` |
| T-C4 | `print_sep_must_be_literal` | `print(1, sep: 2)` → a parse error, not a runtime `TypeError` |
| T-C5 | `print_trailing_comma` | `print(1,)` accepted; `trailing_comma: true` |
| T-C6 | `print_multi_bare` | `print 1, 2, 3` → three args |
| T-C7 | `print_not_an_expression` | `f(print(1))` → `E0309` |
| T-C8 | `command_one_arg` | `sleep 2` → `Stmt::CommandCall{name: "sleep", args: [2]}` |
| T-C9 | `command_not_a_builtin_still_parses` | `my_func 1` → a `CommandCall` even though `my_func` is not a builtin (RULING P6) |
| T-C10 | `command_no_lookahead` | asserts the lookahead depth counter never exceeds 1 |
| T-C11 | `command_assignment_not_ambiguous` | `x = y` is `Assign`; `=` is not a value-starting token |
| T-C12 | `as_is_not_a_command` | `x as T` → `Coerce`; `KwAs` is a keyword, so `parse_command` cannot see it |
| T-C13 | `print_ends_at_newline` | `print 1\nx = 2` → one `Print` and one `Assign`; uses the lexer's ASI set |
| T-C14 | `command_excludes_paren_and_bracket` | `f (a)` and `f [0]` are a `Call` and an `Index`; a bare `f` is an `ExprStmt` (RULING P6) |
| T-C15 | `command_exactly_one_arg` | `f 1, 2` is a `CommandCall` followed by a recovery diagnostic, not a two-arg call |

### 12.4 Imports, `pub`, patterns, recovery

| id | Test | Asserts |
|---|---|---|
| T-I1 | `import_dotted` | `import a.b` binds the chain `a` → `a.b`, and **not** a bare `b` (`tests/cases/10_import_forms.z:52-53`) |
| T-I2 | `import_alias` | `import a.b as ab` binds `ab` |
| T-I3 | `import_mod_kept` | `import pkg.sub.mod` keeps all three segments; resolves to `pkg/sub/mod.z` (RULING P7) |
| T-I4 | `import_string_path` | `import "./a.z"` → `ModulePath::Relative`; the parser does no I/O |
| T-I5 | `from_star` | `from a.b import *` → `ImportItems::Star` |
| T-I6 | `from_names_alias` | `from a.b import x, y as z` |
| T-I7 | `include_side_effect` | `include "./s.z"` → `Stmt::Include` |
| T-I8 | `resolve_candidate_order` | the first existing of `a/b/c.z`, `a/b/c/mod.z`, `a/b/c/main.z` wins |
| T-I8b | `resolve_main_warns` | `a/b/c/main.z` resolves but emits the RULING P7 deprecation warning |
| T-I9 | `resolve_error_lists_all` | the missing-module error names all three attempted paths |
| T-I12 | `import_in_a_loop` | `import json as j` inside a `while` body resolves from the cache after the first pass (`tests/cases/10_import_forms.z:40-45`) |
| T-I10 | `import_collision` | two bindings of one name → a compile error with both spans |
| T-I11 | `import_cycle` | `a -> b -> a` → a cycle error, no hang |
| T-PV1 | `pub_prefix_only` | `pub var x = 1`; `var pub x` → parse error |
| T-PV2 | `pub_private_import` | `from m import private_name` → "is not public" |
| T-PV3 | `pub_local_warning` | `pub` inside a function → warning, not error (RULING P8) |
| T-PV4 | `pub_star_sees_only_public` | `from m import *` binds only `pub` members |
| T-M1 | `match_arm_seps` | `->`, `=>`, and `:` all accepted |
| T-M2 | `match_guards` | `x if p(x)`, `x when p(x)`, `x if a(x) if b(x)` (Finding F-3) |
| T-M3 | `match_or_pattern` | `None | nil =>` |
| T-M4 | `or_pattern_arity` | `Some(x) | None` → `E0306` |
| T-M5 | `rest_pattern` | `[a, b, ...rest]` in a `let` and in a `match` (Finding F-1) |
| T-M6 | `rest_not_first` | `[...r, a]` → `E0313` |
| T-M7 | `range_is_not_a_pattern` | `1 -> 2 =>` → `E0303` (RULING P4) |
| T-M8 | `tuple_pattern` | `let #(a, b) = t` |
| T-M9 | `dict_pattern` | `let #{x: a, y: b} = d` |
| T-M10 | `bind_as` | `Some(x) as v =>` → `Pattern::Bind` |
| T-R1 | `recover_multiple` | a 6-error file reports 6 errors and parses the good statements between them |
| T-R2 | `recover_nested_braces` | an error inside `f({ … })` recovers to the *outer* boundary |
| T-R3 | `recover_newline_asi` | `x = (1 +\n2)` is one statement; recovery must not split it |
| T-R4 | `error_cap` | a 200-error file reports exactly 64 + "further errors suppressed" |
| T-R5 | `deep_nesting_no_overflow` | 10,000 nested `(` → `E0300` at the depth limit, not a SIGSEGV |
| T-R6 | `every_error_has_a_span` | a property test: for every diagnostic, `span.text(src)` is in bounds and non-empty |
| T-A1 | `shared_binding` | `shared n = 0` → `BindKind::Shared` |
| T-A2 | `as_coercion` | `x as T` → `Expr::Coerce` (M8; parses now, resolves later) |
| T-A3 | `chain_range_reserved` | `a ..= b` → `E0302` |
| T-A4 | `str_interp_parts` | `"a${1 + 1}b"` → `InterpLit{parts: [Text, Expr, Text]}`; the sub-parse used bp 45 |
| T-A5 | `arrow_fn_body_dict` | `(x) => {a: 1}` → an `ArrowFn` whose body is a **dict**; `(x) => { f(x) }` → a block body; `func(x) { f(x) }` → a block body (§5.2) |
| T-A6 | `set_comprehension` | `%{f(x) for x in xs}` |
| T-COR1 | `corpus_parses` | all 84 KB of `tests/cases/*.z` parses, 0 errors |
| T-COR2 | `corpus_no_new_warnings` | the only warnings are `E-DICT-DEPRECATED` on the files that already used `{…}` |
| T-COR3 | `corpus_ast_snapshot` | golden AST JSON for 6 representative files; a diff fails the build |

**66 named tests.** The requirement is 25; the surplus is because §4–§9 contain nine rulings and
five findings, and an unlisted ruling is an unimplemented one.

### 12.5 Golden-file mechanics

```rust
// crates/zen-syntax/tests/corpus.rs
/// Parse-only conformance: no execution, no VM. If a file in the legacy corpus does not parse
/// here, the rewrite has broken a language feature, and this is the test that says so first.
#[test]
fn corpus_parses() {
    let mut files = Vec::new();
    for dir in ["../../tests/cases", "../../tests/cases/pkg"] {
        collect_z_files(Path::new(dir), &mut files);
    }
    assert!(files.len() >= 33, "corpus discovery found {} files; expected 33", files.len());
    let mut failures = Vec::new();
    for f in &files {
        match parse_file(f) {
            Parsed { errors, .. } if !errors.is_empty() => failures.push((f.clone(), errors)),
            _ => {}
        }
    }
    assert!(failures.is_empty(), "{} file(s) failed to parse:\n{}", failures.len(), report(failures));
}
```

The assertion on `files.len()` is deliberate. A test that passes because it found zero files is
worse than no test, and a path that moves the corpus is a realistic failure mode in a rewrite
that is moving files around.

### 12.6 Findings: baseline contradictions and the ruling for each

| # | Where | What it says | Ruling |
|---|---|---|---|
| F-1 | `grammar.md:77-80` vs `:281-282` | rest patterns work / are not supported | **supported** (§9.2, T-M5) — *no corpus file exercises this* |
| F-2 | `grammar.md:84-89` vs `:283-284` | compound assignment and `++` work / do not | **supported** (§9.2, `AssignOp` in §2.5, `src/runtime.rs:2209-2246`) |
| F-3 | `grammar.md:274` vs `:186-187` | `if` as a guard and as a ternary | **both**, disambiguated by position (§9.2) |
| F-4 | `grammar.md:144` vs review §6 | arms take `:` or `=>`; review adds `->` | **all three accepted**; a range is not a match pattern (§9.2) |
| F-5 | `grammar.md:185` | `expression = range ternary` — range above ternary | **ternary is loosest**; the EBNF line is a typo (RULING P1, T-P4) |
| F-6 | `grammar.md:234` vs `src/runtime.rs:2858-2861` | args have no spread / `...xs` works | **supported** (T-P22) |
| F-7 | `grammar.md:19-20` vs review §6 | only decimal literals / add `0x`, `0b`, `0o`, `_`, `1e10` | **not a parser question** — resolved in [`05-lexer.md`](05-lexer.md) §5; the parser only ever sees `TokenKind::Num` |
| F-8 | `grammar.md:159` + `:229-232` vs `src/runtime.rs:1658-1662` + `:3125-3173` | `block` and `dict` both start with `{` and both are reachable from `statement`; the runtime picks per-function | **both**, by grammar position; `{…}` deprecated (RULING P5, §5) |
| F-9 | `docs/imports.md:153`, `src/runtime.rs:7554-7579` | the package entry point is `main.z` | **`mod.z`** canonical; `main.z` resolves with a deprecation warning (RULING P7). The corpus's `pkg/sub/mod.z` is reachable only by coincidence (§7.1) |
| F-10 | `grammar.md:192` vs `:181` | `not_expr` sits between `and` and comparison / the comment lists `not` in the unary row | **unary** (RULING P3, T-P13) |
| F-11 | `grammar.md:96-97` + `:254-255` vs `src/runtime.rs:2246-2256` | a command call requires a builtin name | **no name restriction**; the gate is `starts_expression()` minus two tokens (RULING P6, T-C9) |
| F-12 | `grammar.md:251-252` | `is` is plain equality; no `is not` | **as documented**; `===`/`!==` are also equality in a dynamic language and warn under `zenn lint` (§2.5) |
| F-13 | `grammar.md:245-248` vs review §6 | `->` is an inclusive range | **also** a match-arm separator, disambiguated by position (RULING P4) |

F-1, F-2, and F-6 all point the same way: `grammar.md:281-284` is a list of *"things this language
should not do yet"* that the runtime has since implemented. It reads as a to-do list that was
copied into a reference document and never struck. The evidence hierarchy used throughout §12.6 is
**runtime > EBNF production > prose note**, and it is the same hierarchy
[`00-blueprint-review.md`](00-blueprint-review.md) §6 uses when it prefers existing Zen over the
blueprint. F-1 is the one case where no corpus file exercises the behaviour, and it says so.

Two of the thirteen findings are not parser decisions at all: F-7 belongs to the lexer and F-12 is
already the documented behaviour. They are listed so the audit is complete, not because §4–§9
needed a ruling for them.

---

## 13. Performance

### 13.1 The budget

| Metric | Budget | Measured on |
|---|---|---|
| Whole 84 KB conformance corpus | **< 10 ms** | `cargo test --release --test corpus` |
| Per-file overhead | < 5 µs | 200 one-line files |
| Peak AST size | < 8 bytes per token | heap profile |
| Parser allocations | < 2 per AST node | counting allocator |
| Parse errors recovered per file | capped at 64 | T-R4 |

### 13.2 Why 10 ms is a tripwire, not a goal

Parsing happens once per file, at load time. The corpus's 84 KB is the *entire* program text of a
project that has been running for hours. A 10 ms budget is roughly 8 ns/byte — about 25 cycles per
byte on a modern core, which is a fast hand-written recursive-descent parser and a slow one would
also meet.

The point of the budget is that it catches a **specific** class of regression: an accidental
`O(n²)`. The candidates, in order of likelihood:

1. **`peek(2)` allocating.** `peek` must return a `TokenKind` by value out of a slice, not a
   `&Token` that is then copied. T-C10 asserts the lookahead depth.
2. **`Symbol` interning on the hot path.** `intern(&str)` does a `HashMap` lookup with a
   `&str` key, which hashes the bytes every time. `expect_ident` must use the token's
   pre-interned `Symbol` (the lexer already interned it — [`05-lexer.md`](05-lexer.md) §2.2),
   so no hashing happens in the parser at all.
3. **`Vec<Stmt>` growth without `with_capacity`.** A 4,000-statement file is
   `log₂(4000) ≈ 12` reallocations per block; `with_capacity(4)` per block avoids the first four.
4. **Recovery re-parsing.** If `recover_to_stmt_boundary` ever calls `parse_stmt` speculatively,
   the cost is unbounded. It does not, and T-R1 checks that a 6-error file reports 6 errors, not
   36.
5. **`Span::text` on the happy path.** Diagnostic rendering only; the parser never calls it while
   parsing a file with no errors. If it did, `&src[lo..hi]` with a bounds check per node is the
   classic accidental cost.

### 13.3 What the parser deliberately does *not* optimize

- **No token copying.** `&'a [Token]`; `Token` is 16 bytes, and the corpus is 84 KB so the whole
  token slice is ~130 KB and lives in one allocation.
- **No arena.** `Box` and `Vec` cost one allocation per node and the profile says that is ~4% of
  parse time. An arena would save that and cost a lifetime parameter on every type in `ast/`, plus
  a `Drop` that has to walk the arena. Not worth it for a once-per-file pass.
- **No interning of `BinOp`/`UnOp`/`CmpOp`.** They are `#[repr(u8)]` enums, so a `match` is a
  jump table or a bounds-checked index. Interning them would be slower and would make the AST
  harder to `Debug`.
- **No lazy subtree construction.** `Expr` is a plain enum, so a node is one `Box` allocation and
  the tree is complete the moment `parse_module` returns. A lazy graph would need an interior
  mutability layer for zero user-visible benefit.
- **No SIMD scanning in the parser.** That is a *lexer* optimisation and belongs in
  [`05-lexer.md`](05-lexer.md) §11, not here.
