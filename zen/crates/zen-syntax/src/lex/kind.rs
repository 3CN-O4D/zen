//! `TokenKind`: the frozen token table, its canonical spellings, and the
//! keyword lookup.
//!
//! See `docs/05-lexer.md` §3. The numeric discriminants in this file are
//! **frozen**: the parser's operator dispatch, the M4 compiler's
//! opcode-selection switch, and the M5 inline-cache key all index by `u8` and
//! are compiled against these values. Adding a variant is non-breaking;
//! renumbering is breaking. Two gaps are deliberate and never assigned: **99**
//! (a retired legacy token) and **111–140** (thirty slots reserved so the three
//! new sigils cannot collide with anything added later). Test T-A18 asserts
//! that no discriminant falls outside the documented set.

use std::fmt;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // ── structural and literal (0–7) ─────────────────────────────────────
    Eof = 0,
    Newline = 1,
    Semi = 2,
    Ident = 3,
    Int = 4,
    Float = 5,
    Str = 6,
    Interp = 7,

    // ── keywords (8–57) ──────────────────────────────────────────────────
    KwLet = 8,
    KwConst = 9,
    KwVar = 10,
    KwPub = 11,
    KwShared = 12,
    KwPrint = 13,
    KwFn = 14,
    KwLambda = 15,
    KwReturn = 16,
    KwIf = 17,
    KwElif = 18,
    KwElse = 19,
    KwWhile = 20,
    KwFor = 21,
    KwIn = 22,
    KwBreak = 23,
    KwContinue = 24,
    KwClass = 25,
    KwNew = 26,
    KwExtends = 27,
    KwSuper = 28,
    KwThis = 29,
    KwImport = 30,
    KwFrom = 31,
    KwAs = 32,
    KwNative = 33,
    KwInclude = 34,
    KwTry = 35,
    KwCatch = 36,
    KwFinally = 37,
    KwThrow = 38,
    KwTypeof = 39,
    KwIs = 40,
    KwSwitch = 41,
    KwCase = 42,
    KwDefault = 43,
    KwMatch = 44,
    KwWhen = 45,
    KwAnd = 46,
    KwOr = 47,
    KwNot = 48,
    KwTrue = 49,
    KwFalse = 50,
    KwNil = 51,
    KwWith = 52,
    KwYield = 53,
    KwSpawn = 54,
    KwSend = 55,
    KwRecv = 56,
    KwSelect = 57,

    // ── punctuation (58–110, 141–143) ───────────────────────────────────
    LBrace = 58,
    RBrace = 59,
    LBracket = 60,
    RBracket = 61,
    LParen = 62,
    RParen = 63,
    Comma = 64,
    Colon = 65,
    Dot = 66,
    DotDot = 67,
    DotDotEq = 68,
    SafeDot = 69,
    Ellipsis = 70,
    ThinArrow = 71,
    Question = 72,
    Bang = 73,
    StrictEq = 74,
    StrictNe = 75,
    Plus = 76,
    Minus = 77,
    Star = 78,
    Slash = 79,
    Percent = 80,
    Amp = 81,
    Pipe = 82,
    Assign = 83,
    PlusEq = 84,
    MinusEq = 85,
    StarEq = 86,
    SlashEq = 87,
    PercentEq = 88,
    AmpEq = 89,
    PipeEq = 90,
    CaretEq = 91,
    ShlEq = 92,
    ShrEq = 93,
    NullishEq = 94,
    Arrow = 95,
    Nullish = 96,
    Eq = 97,
    Ne = 98,
    // 99 retired
    Caret = 100,
    Tilde = 101,
    Lt = 102,
    Le = 103,
    Gt = 104,
    Ge = 105,
    Pow = 106,
    Shl = 107,
    Shr = 108,
    Inc = 109,
    Dec = 110,
    // 111–140 reserved
    DictStart = 141,
    TupleStart = 142,
    SetStart = 143,

    // ── keywords, second bank (144+) ────────────────────────────────────
    // Keywords added after the 8–57 bank froze. `pymport` (foreign Python
    // imports) is the first: it must not disturb any frozen discriminant.
    KwPymport = 144,
}

impl TokenKind {
    /// The lexeme that produces this token in its canonical form.
    #[inline]
    pub fn canonical(&self) -> &'static str {
        match self {
            TokenKind::Eof => "<eof>",
            TokenKind::Newline => "\n",
            TokenKind::Semi => ";",
            TokenKind::Ident => "<ident>",
            TokenKind::Int => "<int>",
            TokenKind::Float => "<float>",
            TokenKind::Str => "<str>",
            TokenKind::Interp => "<interp>",
            TokenKind::KwLet => "let",
            TokenKind::KwConst => "const",
            TokenKind::KwVar => "var",
            TokenKind::KwPub => "pub",
            TokenKind::KwShared => "shared",
            TokenKind::KwPrint => "print",
            TokenKind::KwFn => "fn",
            TokenKind::KwLambda => "lambda",
            TokenKind::KwReturn => "return",
            TokenKind::KwIf => "if",
            TokenKind::KwElif => "elif",
            TokenKind::KwElse => "else",
            TokenKind::KwWhile => "while",
            TokenKind::KwFor => "for",
            TokenKind::KwIn => "in",
            TokenKind::KwBreak => "break",
            TokenKind::KwContinue => "continue",
            TokenKind::KwClass => "class",
            TokenKind::KwNew => "new",
            TokenKind::KwExtends => "extends",
            TokenKind::KwSuper => "super",
            TokenKind::KwThis => "this",
            TokenKind::KwImport => "import",
            TokenKind::KwFrom => "from",
            TokenKind::KwAs => "as",
            TokenKind::KwNative => "native",
            TokenKind::KwInclude => "include",
            TokenKind::KwTry => "try",
            TokenKind::KwCatch => "catch",
            TokenKind::KwFinally => "finally",
            TokenKind::KwThrow => "throw",
            TokenKind::KwTypeof => "typeof",
            TokenKind::KwIs => "is",
            TokenKind::KwSwitch => "switch",
            TokenKind::KwCase => "case",
            TokenKind::KwDefault => "default",
            TokenKind::KwMatch => "match",
            TokenKind::KwWhen => "when",
            TokenKind::KwAnd => "and",
            TokenKind::KwOr => "or",
            TokenKind::KwNot => "not",
            TokenKind::KwTrue => "true",
            TokenKind::KwFalse => "false",
            TokenKind::KwNil => "nil",
            TokenKind::KwWith => "with",
            TokenKind::KwYield => "yield",
            TokenKind::KwSpawn => "spawn",
            TokenKind::KwSend => "send",
            TokenKind::KwRecv => "recv",
            TokenKind::KwSelect => "select",
            TokenKind::KwPymport => "pymport",
            TokenKind::LBrace => "{",
            TokenKind::RBrace => "}",
            TokenKind::LBracket => "[",
            TokenKind::RBracket => "]",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::Comma => ",",
            TokenKind::Colon => ":",
            TokenKind::Dot => ".",
            TokenKind::DotDot => "..",
            TokenKind::DotDotEq => "..=",
            TokenKind::SafeDot => "?.",
            TokenKind::Ellipsis => "...",
            TokenKind::ThinArrow => "=>",
            TokenKind::Question => "?",
            TokenKind::Bang => "!",
            TokenKind::StrictEq => "===",
            TokenKind::StrictNe => "!==",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::Amp => "&",
            TokenKind::Pipe => "|",
            TokenKind::Assign => "=",
            TokenKind::PlusEq => "+=",
            TokenKind::MinusEq => "-=",
            TokenKind::StarEq => "*=",
            TokenKind::SlashEq => "/=",
            TokenKind::PercentEq => "%=",
            TokenKind::AmpEq => "&=",
            TokenKind::PipeEq => "|=",
            TokenKind::CaretEq => "^=",
            TokenKind::ShlEq => "<<=",
            TokenKind::ShrEq => ">>=",
            TokenKind::NullishEq => "??=",
            TokenKind::Arrow => "->",
            TokenKind::Nullish => "??",
            TokenKind::Eq => "==",
            TokenKind::Ne => "!=",
            TokenKind::Caret => "^",
            TokenKind::Tilde => "~",
            TokenKind::Lt => "<",
            TokenKind::Le => "<=",
            TokenKind::Gt => ">",
            TokenKind::Ge => ">=",
            TokenKind::Pow => "**",
            TokenKind::Shl => "<<",
            TokenKind::Shr => ">>",
            TokenKind::Inc => "++",
            TokenKind::Dec => "--",
            TokenKind::DictStart => "#{",
            TokenKind::TupleStart => "#(",
            TokenKind::SetStart => "%{",
        }
    }

    /// The delimiter byte a bracket token belongs to, for *both* halves of a
    /// pair.
    ///
    /// The lexer pushes `delim_byte` when it sees an opener and compares
    /// `delim_byte` when it sees a closer, so `(` and `)` must both report
    /// `b'('` — returning `None` for the openers would push a `0` that no
    /// closer can ever match, leaving the delimiter stuck on the stack and
    /// suppressing every later newline.
    #[inline]
    pub fn delim_byte(self) -> Option<u8> {
        match self {
            TokenKind::LParen | TokenKind::RParen | TokenKind::TupleStart => Some(b'('),
            TokenKind::LBracket | TokenKind::RBracket | TokenKind::SetStart => Some(b'['),
            TokenKind::LBrace | TokenKind::RBrace | TokenKind::DictStart => Some(b'{'),
            _ => None,
        }
    }

    /// Whether this opener suppresses newlines for its whole open extent (A1).
    ///
    /// **RULING ASI-0:** `#{`, `%{` and `#(` suppress, and so do `(` and `[`.
    /// A bare `{` does **not** — the legacy lexer's comment at
    /// `src/runtime.rs:677-678` says why: *"braces are not counted: blocks need
    /// newlines"*. `LBrace` instead suppresses only its **own** newline, via
    /// the A2 bit in [`TokenKind::continues_expression`], so `if c {` opens a
    /// block whose arms still split on newlines (T-C13).
    #[inline]
    pub fn opener_suppresses_newline(self) -> bool {
        matches!(
            self,
            TokenKind::LParen
                | TokenKind::LBracket
                | TokenKind::DictStart
                | TokenKind::SetStart
                | TokenKind::TupleStart
        )
    }

    /// Whether this token kind continues an expression across a newline (A2).
    #[inline]
    pub fn continues_expression(self) -> bool {
        matches!(
            self,
            TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash
                | TokenKind::Percent
                | TokenKind::Pow
                | TokenKind::Amp
                | TokenKind::Pipe
                | TokenKind::Caret
                | TokenKind::Shl
                | TokenKind::Shr
                | TokenKind::Eq
                | TokenKind::Ne
                | TokenKind::StrictEq
                | TokenKind::StrictNe
                | TokenKind::Lt
                | TokenKind::Le
                | TokenKind::Gt
                | TokenKind::Ge
                | TokenKind::KwAnd
                | TokenKind::KwOr
                | TokenKind::KwIs
                | TokenKind::KwIn
                | TokenKind::Nullish
                | TokenKind::Dot
                | TokenKind::SafeDot
                | TokenKind::DotDot
                | TokenKind::Arrow
                | TokenKind::ThinArrow
                | TokenKind::Assign
                | TokenKind::PlusEq
                | TokenKind::MinusEq
                | TokenKind::StarEq
                | TokenKind::SlashEq
                | TokenKind::PercentEq
                | TokenKind::NullishEq
                | TokenKind::AmpEq
                | TokenKind::PipeEq
                | TokenKind::CaretEq
                | TokenKind::ShlEq
                | TokenKind::ShrEq
                | TokenKind::Inc
                | TokenKind::Dec
                | TokenKind::Question
                | TokenKind::Colon
                // A bare `{` suppresses only the newline directly after it, so
                // `if c {` does not open with a blank line, while the arms of
                // the block still separate on newlines (RULING ASI-0, T-C13).
                | TokenKind::LBrace
        )
    }
}

// ── keyword lookup ──────────────────────────────────────────────────────

static KEYWORDS: phf::Map<&'static str, TokenKind> = phf::phf_map! {
    "let" => TokenKind::KwLet,
    "const" => TokenKind::KwConst,
    "var" => TokenKind::KwVar,
    "global" => TokenKind::KwVar,
    "pub" => TokenKind::KwPub,
    "shared" => TokenKind::KwShared,
    "print" => TokenKind::KwPrint,
    "fn" => TokenKind::KwFn,
    "func" => TokenKind::KwFn,
    "function" => TokenKind::KwFn,
    "procedure" => TokenKind::KwFn,
    "proc" => TokenKind::KwFn,
    "def" => TokenKind::KwFn,
    "lambda" => TokenKind::KwLambda,
    "return" => TokenKind::KwReturn,
    "if" => TokenKind::KwIf,
    "elif" => TokenKind::KwElif,
    "else" => TokenKind::KwElse,
    "while" => TokenKind::KwWhile,
    "for" => TokenKind::KwFor,
    "in" => TokenKind::KwIn,
    "break" => TokenKind::KwBreak,
    "continue" => TokenKind::KwContinue,
    "class" => TokenKind::KwClass,
    "new" => TokenKind::KwNew,
    "extends" => TokenKind::KwExtends,
    "inherit" => TokenKind::KwExtends,
    "super" => TokenKind::KwSuper,
    "this" => TokenKind::KwThis,
    "import" => TokenKind::KwImport,
    "from" => TokenKind::KwFrom,
    "as" => TokenKind::KwAs,
    "native" => TokenKind::KwNative,
    "include" => TokenKind::KwInclude,
    "load" => TokenKind::KwInclude,
    "try" => TokenKind::KwTry,
    "catch" => TokenKind::KwCatch,
    "except" => TokenKind::KwCatch,
    "finally" => TokenKind::KwFinally,
    "throw" => TokenKind::KwThrow,
    "raise" => TokenKind::KwThrow,
    "typeof" => TokenKind::KwTypeof,
    "is" => TokenKind::KwIs,
    "switch" => TokenKind::KwSwitch,
    "case" => TokenKind::KwCase,
    "default" => TokenKind::KwDefault,
    "match" => TokenKind::KwMatch,
    "when" => TokenKind::KwWhen,
    "and" => TokenKind::KwAnd,
    "or" => TokenKind::KwOr,
    "not" => TokenKind::KwNot,
    "true" => TokenKind::KwTrue,
    "false" => TokenKind::KwFalse,
    "nil" => TokenKind::KwNil,
    "null" => TokenKind::KwNil,
    "with" => TokenKind::KwWith,
    "yield" => TokenKind::KwYield,
    "spawn" => TokenKind::KwSpawn,
    "send" => TokenKind::KwSend,
    "recv" => TokenKind::KwRecv,
    "select" => TokenKind::KwSelect,
    "pymport" => TokenKind::KwPymport,
};

/// Keyword lookup used by `lex_word`.
#[inline]
pub fn keyword_of(ident: &str) -> Option<TokenKind> {
    KEYWORDS.get(ident).copied()
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.canonical())
    }
}

/// Display for debugging: shows both the canonical spelling and the discriminant.
impl fmt::Debug for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TokenKind::{self}({})", *self as u8)
    }
}
