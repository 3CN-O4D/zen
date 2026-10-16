//! AST nodes for the Zen language.
use crate::span::{FileId, Span};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub file_id: FileId,
    pub items: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    Let {
        name: String,
        value: Box<Expr>,
        mutable: bool,
        span: Span,
    },
    Const {
        name: String,
        value: Box<Expr>,
        span: Span,
    },
    Assign {
        target: Box<Expr>,
        op: AssignOp,
        value: Box<Expr>,
        span: Span,
    },
    Expr(Box<Expr>),
    Print {
        args: Vec<Expr>,
        sep: Option<Expr>,
        end: Option<Expr>,
        span: Span,
    },
    If {
        cond: Box<Expr>,
        then_branch: Block,
        elifs: Vec<(Expr, Block)>,
        else_branch: Option<Block>,
        span: Span,
    },
    While {
        cond: Box<Expr>,
        body: Block,
        span: Span,
    },
    For {
        vars: Vec<String>,
        iter: Box<Expr>,
        body: Block,
        span: Span,
    },
    Function {
        name: String,
        params: Vec<Param>,
        body: Block,
        span: Span,
    },
    Return {
        value: Option<Box<Expr>>,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    Try {
        body: Block,
        catches: Vec<CatchClause>,
        finally: Option<Block>,
        span: Span,
    },
    Throw {
        value: Box<Expr>,
        span: Span,
    },
    Import {
        specs: Vec<ImportSpec>,
        span: Span,
    },
    /// A foreign Python import: `pymport pkg.sub [as alias]`. Deliberately
    /// separate from [`Stmt::Import`] so the parser, compiler, and JIT always
    /// know this crosses into CPython instead of loading Zen bytecode — no
    /// probing, no ambiguity, no slow-path checks on native imports.
    Pymport {
        path: Vec<String>,
        alias: Option<String>,
        span: Span,
    },
    FromImport {
        path: Vec<String>,
        items: ImportItems,
        span: Span,
    },
    Include {
        path: String,
        span: Span,
    },
    Class {
        name: String,
        parent: Option<String>,
        members: Vec<ClassMember>,
        span: Span,
    },
    Match {
        subject: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    When {
        arms: Vec<MatchArm>,
        span: Span,
    },
    Shared {
        name: String,
        value: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub default: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssignOp {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    NullCoalesceAssign,
    BitAndAssign,
    BitOrAssign,
    BitXorAssign,
    ShlAssign,
    ShrAssign,
    Inc,
    Dec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportSpec {
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImportItems {
    All,
    Names(Vec<ImportName>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportName {
    pub name: String,
    pub alias: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatchClause {
    pub typ: Option<String>,
    pub binding: Option<String>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    /// A `match` arm body may be a block or a single expression; a `switch`
    /// case body is always a block. The two-variant body is what lets
    /// `1 -> "one"` and `1 -> { print "one" }` both parse without re-reading
    /// the same tokens.
    pub body: BlockOrExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlockOrExpr {
    Expr(Box<Expr>),
    Block(Block),
}

/// A **separate sub-grammar** with its own type, not a flagged `Expr`
/// (`docs/06-parser.md` §9.1). Sharing the type would be a lie about which
/// expressions are legal: `1 + 1` is an expression and is not a pattern, `a | b`
/// is a pattern and is not an expression, and a `?`-form expression is neither.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    /// `_`, or a name the user does not want. The parser decides this, not the
    /// lexer: `_` and `x` have the same token type.
    Wildcard(Span),
    /// A lowercase name: binds the matched value.
    Binding {
        name: String,
        span: Span,
    },
    Literal {
        value: Literal,
        span: Span,
    },
    /// `Path(a, b)` — an enum case or a class destructuring pattern. A bare
    /// capitalised path with no parens and no fields (`None`, `red`) is a
    /// `Constructor` with `fields: vec![]`, i.e. a value pattern tested by
    /// equality.
    ///
    /// Capitalisation is what separates a `Constructor` from a `Binding`,
    /// matching `parse_binding_target`'s `CapitalIdent` arm in §9.4.
    Constructor {
        path: Vec<String>,
        fields: Vec<Pattern>,
        span: Span,
    },
    /// `[a, b, ...rest]`. `rest` is allowed in exactly one position and never
    /// first (Finding F-1: the runtime implements rest patterns, so the
    /// `grammar.md:281-282` prose claiming otherwise is stale).
    List {
        elems: Vec<Pattern>,
        rest: Option<Box<Pattern>>,
        span: Span,
    },
    /// `#{k: v}` / `{k: v}` — dict destructuring.
    Dict {
        entries: Vec<(Pattern, Pattern)>,
        rest: Option<Box<Pattern>>,
        span: Span,
    },
    /// `1 .. 10` / `1 -> 10`. **Not constructible from a `match` arm** — `->` has
    /// two readings and is resolved by position (RULING P4), so a range is only
    /// reachable from a `switch` case, which parses an `Expr`.
    Range {
        lo: Box<Expr>,
        hi: Box<Expr>,
        lo_incl: bool,
        hi_incl: bool,
        span: Span,
    },
    /// `x if p` / `x when p` / `x if a && b` — a chained guard, tested in
    /// order, first success wins (Finding F-3). Chained rather than singular so
    /// the `Vec` is what the AST carries; the resolver checks nothing here.
    Guard {
        pat: Box<Pattern>,
        guards: Vec<Expr>,
        span: Span,
    },
    /// `a | b`. Every alternative must bind the same names — a *resolver* check,
    /// because it needs the whole pattern (§9.5).
    Or {
        alts: Vec<Pattern>,
        span: Span,
    },
    /// `x as n` — bind the matched value under a new name.
    Bind {
        pat: Box<Pattern>,
        name: String,
        span: Span,
    },
    /// `#(a, b)` — tuple destructuring.
    Tuple {
        elems: Vec<Pattern>,
        span: Span,
    },
    /// A `when` arm's head. `when { v > 5: "big" }` matches on conditions, not
    /// on values, so the head is an arbitrary boolean expression rather than a
    /// pattern.
    ///
    /// **This is an extension beyond §9.1**, which never gives `when` its own
    /// arm form even though `docs/control-flow.md:181` requires the syntax.
    /// Flagged rather than smuggled in: making the head an `Expr` is the only
    /// honest shape, since no `Pattern` variant can hold `v > 5`.
    Condition {
        cond: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassMember {
    pub kind: ClassMemberKind,
    pub name: String,
    pub params: Vec<Param>,
    pub body: Block,
    pub default: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClassMemberKind {
    Init,
    Method,
    Field,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Literal(Literal),
    Ident { name: String, span: Span },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    /// `s[a:b]`, `s[a:]`, `s[:b]`, `s[a, b]` — a slice. The legacy corpus
    /// writes bounds with either `:` or `,`.
    Slice {
        object: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        span: Span,
    },
    Field {
        object: Box<Expr>,
        field: String,
        span: Span,
    },
    OptionalField {
        object: Box<Expr>,
        field: String,
        span: Span,
    },
    List(Vec<Expr>),
    /// `[body for x in xs if g(x) …]` (and future set/dict forms). Unlike the
    /// legacy single-clause comprehension, `clauses` may hold several `for`s.
    Comprehension {
        kind: CompKind,
        key: Option<Box<Expr>>,
        body: Box<Expr>,
        clauses: Vec<CompClause>,
        span: Span,
    },
    Dict(Vec<(Expr, Expr)>),
    Set(Vec<Expr>),
    Tuple(Vec<Expr>),
    DictLiteral(IndexMap<String, Expr>),
    SetLiteral(Vec<Expr>),
    TupleLiteral(Vec<Expr>),
    Lambda {
        params: Vec<Param>,
        body: Box<Expr>,
        span: Span,
    },
    /// `func(x) { … }` — an anonymous function *expression*. The `{` is a
    /// **block** (`docs/06-parser.md` §2.2 line 155).
    FnExpr {
        params: Vec<Param>,
        body: Block,
        span: Span,
    },
    /// `(x, y) => body` — the body is expression position, so `{` there is a
    /// dict and a block body is reached only when `{` opens the body slot.
    ArrowFn {
        params: Vec<Param>,
        body: BlockOrExpr,
        span: Span,
    },
    /// `if c { … } else { … }` in *expression* position. Unlike `Stmt::If` this
    /// is a value, and unlike a `? :` ternary its arms are `Block`s — the rule
    /// from docs/06-parser.md §4: an `if` **statement** has block arms, an `if`
    /// **expression** has expression arms, and `{` after an expression `if`
    /// opens a dict, so a block-armed value must be spelled with braces.
    ///
    /// `arms` is a chain so `elif` needs no separate field, and `otherwise` is
    /// an `Option` because the tail is optional. (`if c then a else b`, the
    /// other spelling, is **M8** and is not in this AST.)
    IfExpr {
        arms: Vec<(Expr, Block)>,
        otherwise: Option<Block>,
        span: Span,
    },
    Ternary {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
        span: Span,
    },
    /// Chainable comparisons: `a < b <= c` → Chain(vec![(Lt, a), (Le, b), (c)]).
    Chain {
        ops: Vec<BinaryOp>,
        operands: Vec<Expr>,
        span: Span,
    },
    NullCoalesce {
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
        span: Span,
    },
    Match {
        subject: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    When {
        arms: Vec<MatchArm>,
        span: Span,
    },
    New {
        class: String,
        args: Vec<Expr>,
        span: Span,
    },
    SuperCall {
        args: Vec<Expr>,
        span: Span,
    },
    SuperMethod {
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
    InterpolatedString(Vec<StringPart>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompKind {
    List,
    Set,
    Dict,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompClause {
    pub var: String,
    pub iter: Expr,
    pub guard: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StringPart {
    Text(String),
    Interpolation(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Nil,
    Bool(bool),
    Int(i64),
    /// Serialized as its raw `u64` bit pattern. JSON has no representation for
    /// `NaN`/`±∞`, but a literal such as `1e400` legitimately parses to `+∞`,
    /// and the round-trip gate (`docs/19-roadmap.md` M1) requires every
    /// representable `f64` to survive. Bits also preserve `-0.0` and NaN payloads.
    Float(#[serde(with = "float_bits")] f64),
    String(String),
    Bytes(Vec<u8>),
}

/// `f64` ⇄ `u64` bit-pattern serde adapter (see [`Literal::Float`]).
mod float_bits {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(v.to_bits())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        Ok(f64::from_bits(u64::deserialize(d)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Eq,
    Ne,
    StrictEq,
    StrictNe,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    In,
    NotIn,
    Is,
    IsNot,
    Nullish, // ?? operator
    Range,   // .. and -> (inclusive range)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    TypeOf,
}
