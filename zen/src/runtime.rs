use std::{
    collections::{BTreeMap, HashMap},
    env,
    fmt,
    fs,
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream, UdpSocket},
    path::{Path, PathBuf},
    process::{self, Command},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

type InstanceRef = Arc<Mutex<Instance>>;

use hmac::Mac as HmacMac;
use chrono::{Datelike, Timelike};

static RESPONSE_BODIES: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<u64, String>>> =
    std::sync::OnceLock::new();

fn response_bodies() -> &'static std::sync::Mutex<std::collections::HashMap<u64, String>> {
    RESPONSE_BODIES.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

// Process-wide function registry so spawned threads can resolve user functions
// defined in the main interpreter VM.
static FUNCTION_REGISTRY: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, Function>>,
> = std::sync::OnceLock::new();

fn function_registry() -> &'static std::sync::Mutex<std::collections::HashMap<String, Function>> {
    FUNCTION_REGISTRY.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Instance {
    class_name: String,
    fields: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    List(Vec<Value>),
    Dict(BTreeMap<String, Value>),
    Instance(InstanceRef),
    Socket(Arc<Mutex<TcpStream>>),
    NativeFunction(String),
    Function(String),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Number(a), Self::Number(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::List(a), Self::List(b)) => a == b,
            (Self::Dict(a), Self::Dict(b)) => a == b,
            (Self::Instance(a), Self::Instance(b)) => Arc::ptr_eq(a, b),
            (Self::Socket(_), Self::Socket(_)) => false, // Sockets are not comparable
            _ => false,
        }
    }
}

impl Value {
    fn truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(v) => *v,
            Self::Number(v) => *v != 0.0,
            Self::String(v) => !v.is_empty(),
            Self::List(v) => !v.is_empty(),
            Self::Dict(v) => !v.is_empty(),
            Self::Instance(_) | Self::Socket(_) | Self::NativeFunction(_) | Self::Function(_) => true,
        }
    }
}
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::Number(v) if v.fract() == 0.0 => write!(f, "{v:.0}"),
            Self::Number(v) => write!(f, "{v}"),
            Self::String(v) => write!(f, "{v}"),
            Self::List(v) => {
                write!(f, "[")?;
                for (i, value) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{value}")?;
                }
                write!(f, "]")
            }
            Self::Dict(v) => {
                write!(f, "{{")?;
                for (i, (key, value)) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{key}: {value}")?;
                }
                write!(f, "}}")
            }
            Self::Instance(instance) => {
                write!(f, "<{}>", instance.lock().unwrap().class_name)
            }
            Self::Socket(_) => write!(f, "<Socket>"),
            Self::NativeFunction(name) => write!(f, "<native:{name}>"),
            Self::Function(name) => write!(f, "<function:{name}>"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Ident(String),
    Number(f64),
    String(String),
    Interp(Vec<InterpPart>),
    True,
    False,
    Null,
    Let,
    Const,
    Print,
    If,
    Else,
    While,
    For,
    In,
    Break,
    Continue,
    Function,
    Def,
    Return,
    Class,
    New,
    Extends,
    Import,
    From,
    Include,
    Load,
    As,
    Native,
    Try,
    Catch,
    Finally,
    Throw,
    Typeof,
    And,
    Or,
    Not,
    Is,
    Switch,
    Case,
    Default,
    Lambda,
    Ellipsis,
    DotDot,
    Pow,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LParen,
    RParen,
    Comma,
    Semi,
    Newline,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    PercentAssign,
    AmpAssign,
    PipeAssign,
    CaretAssign,
    LShiftAssign,
    RShiftAssign,
    Arrow,
    Nullish,
    StrictEq,
    StrictNe,
    SafeDot,
    NullishAssign,
    Inc,
    Dec,
    Amp,
    Pipe,
    Caret,
    Tilde,
    LShift,
    RShift,
    Question,
    Dot,
    Colon,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Bang,
    Eof,
}
#[derive(Clone, Debug)]
struct Token {
    kind: Kind,
    line: usize,
    col: usize,
}

#[derive(Clone, Debug, PartialEq)]
enum InterpPart {
    Text(String),
    Expr(String),
}

fn lex(source: &str) -> Result<Vec<Token>, String> {
    let bytes = source.as_bytes();
    let (mut i, mut line, mut col) = (0, 1, 1);
    let mut out = vec![];
    while i < bytes.len() {
        let c = bytes[i] as char;
        let start = (line, col);
        if c == ' ' || c == '\t' || c == '\r' {
            i += 1;
            col += 1;
            continue;
        }
        if c == '\n' {
            out.push(Token {
                kind: Kind::Newline,
                line,
                col,
            });
            i += 1;
            line += 1;
            col = 1;
            continue;
        }
        if c == '/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
                col += 1;
            }
            continue;
        }
        if c == '/' && bytes.get(i + 1) == Some(&b'*') {
            i += 2;
            col += 2;
            let mut closed = false;
            while i < bytes.len() {
                if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    i += 2;
                    col += 2;
                    closed = true;
                    break;
                }
                if bytes[i] == b'\n' {
                    i += 1;
                    line += 1;
                    col = 1;
                } else {
                    i += 1;
                    col += 1;
                }
            }
            if !closed {
                return Err(format!(
                    "{}:{}: unterminated block comment",
                    start.0, start.1
                ));
            }
            continue;
        }
        if c == '#' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
                col += 1;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            let quote = c;
            let is_interpolated = c == '"';
            i += 1;
            col += 1;
            let mut text = String::new();
            let mut closed = false;
            let mut parts: Vec<InterpPart> = Vec::new();
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch == quote {
                    i += 1;
                    col += 1;
                    closed = true;
                    break;
                }
                if ch == '\\' {
                    i += 1;
                    col += 1;
                    let escape = *bytes
                        .get(i)
                        .ok_or_else(|| format!("{line}:{col}: unfinished escape"))?
                        as char;
                    text.push(match escape {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '\\' => '\\',
                        '\'' => '\'',
                        '"' => '"',
                        '$' => '$',
                        x => x,
                    });
                    i += 1;
                    col += 1;
                } else if is_interpolated && ch == '$' && bytes.get(i + 1) == Some(&b'{') {
                    // Flush accumulated literal text, then capture the expression.
                    if !text.is_empty() {
                        parts.push(InterpPart::Text(std::mem::take(&mut text)));
                    }
                    i += 2;
                    col += 2;
                    let expr_start = i;
                    let mut depth = 1usize;
                    let mut expr_line = line;
                    let mut expr_col = col;
                    while i < bytes.len() {
                        let e = bytes[i] as char;
                        if e == '{' {
                            depth += 1;
                        } else if e == '}' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        } else if e == '\n' {
                            expr_line += 1;
                            expr_col = 1;
                            i += 1;
                            continue;
                        }
                        i += 1;
                        expr_col += 1;
                    }
                    if depth != 0 {
                        return Err(format!(
                            "{expr_line}:{expr_col}: unterminated interpolation expression"
                        ));
                    }
                    let expr_source = source[expr_start..i].to_string();
                    parts.push(InterpPart::Expr(expr_source));
                    i += 1; // consume closing }
                    col += 1;
                } else {
                    text.push(ch);
                    i += 1;
                    if ch == '\n' {
                        line += 1;
                        col = 1
                    } else {
                        col += 1
                    };
                }
            }
            if !closed {
                return Err(format!("{}:{}: unterminated string", start.0, start.1));
            }
            if !parts.is_empty() {
                if !text.is_empty() {
                    parts.push(InterpPart::Text(text));
                }
                out.push(Token {
                    kind: Kind::Interp(parts),
                    line: start.0,
                    col: start.1,
                });
            } else {
                out.push(Token {
                    kind: Kind::String(text),
                    line: start.0,
                    col: start.1,
                });
            }
            continue;
        }
        if c.is_ascii_digit()
            || (c == '.'
                && bytes
                    .get(i + 1)
                    .is_some_and(|x| (*x as char).is_ascii_digit()))
        {
            let begin = i;
            while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                i += 1;
                col += 1;
            }
            if i < bytes.len()
                && bytes[i] == b'.'
                && bytes
                    .get(i + 1)
                    .is_some_and(|x| (*x as char).is_ascii_digit())
            {
                i += 1;
                col += 1;
                while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                    i += 1;
                    col += 1;
                }
            }
            let n = source[begin..i]
                .parse()
                .map_err(|_| format!("{}:{}: invalid number", start.0, start.1))?;
            out.push(Token {
                kind: Kind::Number(n),
                line: start.0,
                col: start.1,
            });
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let begin = i;
            while i < bytes.len()
                && ((bytes[i] as char).is_ascii_alphanumeric() || bytes[i] == b'_')
            {
                i += 1;
                col += 1;
            }
            let word = &source[begin..i];
            let kind = match word {
                "let" => Kind::Let,
                "const" => Kind::Const,
                "print" => Kind::Print,
                "if" => Kind::If,
                "else" => Kind::Else,
                "while" => Kind::While,
                "for" => Kind::For,
                "in" => Kind::In,
                "break" => Kind::Break,
                "continue" => Kind::Continue,
                "function" | "func" => Kind::Function,
                "def" => Kind::Def,
                "return" => Kind::Return,
                "class" => Kind::Class,
                "new" => Kind::New,
                "extends" => Kind::Extends,
                "import" => Kind::Import,
                "from" => Kind::From,
                "include" => Kind::Include,
                "load" => Kind::Load,
                "as" => Kind::As,
                "native" => Kind::Native,
                "try" => Kind::Try,
                "catch" | "except" => Kind::Catch,
                "finally" => Kind::Finally,
                "throw" => Kind::Throw,
                "typeof" => Kind::Typeof,
                "is" => Kind::Is,
                "switch" => Kind::Switch,
                "case" => Kind::Case,
                "default" => Kind::Default,
                "lambda" => Kind::Lambda,
                "and" => Kind::And,
                "or" => Kind::Or,
                "not" => Kind::Not,
                "true" => Kind::True,
                "false" => Kind::False,
                "null" => Kind::Null,
                _ => Kind::Ident(word.into()),
            };
            out.push(Token {
                kind,
                line: start.0,
                col: start.1,
            });
            continue;
        }
        if bytes.get(i..i + 3) == Some(b"<<=" as &[u8]) {
            out.push(Token {
                kind: Kind::LShiftAssign,
                line,
                col,
            });
            i += 3;
            col += 3;
            continue;
        }
        if bytes.get(i..i + 3) == Some(b">>=" as &[u8]) {
            out.push(Token {
                kind: Kind::RShiftAssign,
                line,
                col,
            });
            i += 3;
            col += 3;
            continue;
        }
        if bytes.get(i..i + 3) == Some(b"??=" as &[u8]) {
            out.push(Token {
                kind: Kind::NullishAssign,
                line,
                col,
            });
            i += 3;
            col += 3;
            continue;
        }
        if bytes.get(i..i + 3) == Some(b"===" as &[u8]) {
            out.push(Token {
                kind: Kind::StrictEq,
                line,
                col,
            });
            i += 3;
            col += 3;
            continue;
        }
        if bytes.get(i..i + 3) == Some(b"..." as &[u8]) {
            out.push(Token {
                kind: Kind::Ellipsis,
                line,
                col,
            });
            i += 3;
            col += 3;
            continue;
        }
        if bytes.get(i..i + 3) == Some(b"!==" as &[u8]) {
            out.push(Token {
                kind: Kind::StrictNe,
                line,
                col,
            });
            i += 3;
            col += 3;
            continue;
        }
        let pair = bytes.get(i + 1).map(|v| [bytes[i], *v]);
        let kind = match pair {
            Some([b'=', b'=']) => {
                i += 1;
                col += 1;
                Kind::Eq
            }
            Some([b'.', b'.']) => {
                i += 1;
                col += 1;
                Kind::DotDot
            }
            Some([b'*', b'*']) => {
                i += 1;
                col += 1;
                Kind::Pow
            }
            Some([b'!', b'=']) => {
                i += 1;
                col += 1;
                Kind::Ne
            }
            Some([b'<', b'=']) => {
                i += 1;
                col += 1;
                Kind::Le
            }
            Some([b'>', b'=']) => {
                i += 1;
                col += 1;
                Kind::Ge
            }
            Some([b'+', b'=']) => {
                i += 1;
                col += 1;
                Kind::PlusAssign
            }
            Some([b'-', b'>']) => {
                i += 1;
                col += 1;
                Kind::Arrow
            }
            Some([b'-', b'=']) => {
                i += 1;
                col += 1;
                Kind::MinusAssign
            }
            Some([b'*', b'=']) => {
                i += 1;
                col += 1;
                Kind::StarAssign
            }
            Some([b'/', b'=']) => {
                i += 1;
                col += 1;
                Kind::SlashAssign
            }
            Some([b'%', b'=']) => {
                i += 1;
                col += 1;
                Kind::PercentAssign
            }
            Some([b'&', b'=']) => {
                i += 1;
                col += 1;
                Kind::AmpAssign
            }
            Some([b'|', b'=']) => {
                i += 1;
                col += 1;
                Kind::PipeAssign
            }
            Some([b'^', b'=']) => {
                i += 1;
                col += 1;
                Kind::CaretAssign
            }
            Some([b'&', b'&']) => {
                i += 1;
                col += 1;
                Kind::And
            }
            Some([b'|', b'|']) => {
                i += 1;
                col += 1;
                Kind::Or
            }
            Some([b'?', b'?']) => {
                i += 1;
                col += 1;
                Kind::Nullish
            }
            Some([b'?', b'.']) => {
                i += 1;
                col += 1;
                Kind::SafeDot
            }
            Some([b'+', b'+']) => {
                i += 1;
                col += 1;
                Kind::Inc
            }
            Some([b'-', b'-']) => {
                i += 1;
                col += 1;
                Kind::Dec
            }
            Some([b'<', b'<']) => {
                i += 1;
                col += 1;
                Kind::LShift
            }
            Some([b'>', b'>']) => {
                i += 1;
                col += 1;
                Kind::RShift
            }
            _ => match c {
                '{' => Kind::LBrace,
                '}' => Kind::RBrace,
                '[' => Kind::LBracket,
                ']' => Kind::RBracket,
                '(' => Kind::LParen,
                ')' => Kind::RParen,
                ',' => Kind::Comma,
                ';' => Kind::Semi,
                '+' => Kind::Plus,
                '-' => Kind::Minus,
                '*' => Kind::Star,
                '/' => Kind::Slash,
                '%' => Kind::Percent,
                '.' => Kind::Dot,
                ':' => Kind::Colon,
                '=' => Kind::Assign,
                '<' => Kind::Lt,
                '>' => Kind::Gt,
                '!' => Kind::Bang,
                '?' => Kind::Question,
                '&' => Kind::Amp,
                '|' => Kind::Pipe,
                '^' => Kind::Caret,
                '~' => Kind::Tilde,
                _ => return Err(format!("{}:{}: unexpected character {c:?}", line, col)),
            },
        };
        out.push(Token { kind, line, col });
        i += 1;
        col += 1;
    }
    out.push(Token {
        kind: Kind::Eof,
        line,
        col,
    });
    Ok(out)
}

#[derive(Clone, Debug)]
enum DictEntry {
    Pair(String, Expr),
    Spread(Expr),
}

#[derive(Clone, Debug)]
enum Expr {
    Value(Value),
    Var(String),
    List(Vec<Expr>),
    Dict(Vec<DictEntry>),
    Named(String, Box<Expr>),
    Unary(Kind, Box<Expr>),
    Binary(Box<Expr>, Kind, Box<Expr>),
    Range(Box<Expr>, Box<Expr>, bool),
    Index(Box<Expr>, Box<Expr>),
    Member(Box<Expr>, String),
    SafeMember(Box<Expr>, String),
    Call(Box<Expr>, Vec<Expr>),
    New(String, Vec<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Increment(Box<Expr>, i64),
    Lambda(Vec<String>, Vec<Stmt>),
    Spread(Box<Expr>),
}
#[derive(Clone, Debug)]
enum LetTarget {
    Var(String),
    List(Vec<String>),
    Dict(Vec<String>),
}

#[derive(Clone, Debug)]
struct Stmt {
    kind: StmtKind,
    line: usize,
    col: usize,
}

#[derive(Clone, Debug)]
enum StmtKind {
    Let(LetTarget, Expr, bool),
    Assign(String, Kind, Expr),
    Print(Vec<Expr>),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    For(String, Expr, Vec<Stmt>),
    Break,
    Continue,
    Function(String, Vec<String>, Vec<Stmt>),
    Native(String, Vec<String>),
    Try(Vec<Stmt>, Vec<CatchClause>, Option<Vec<Stmt>>),
    Throw(Expr),
    Return(Option<Expr>),
    Class(String, Option<String>, Vec<Stmt>),
    Import(Vec<(String, Option<String>)>),
    FromImport(String, Vec<(String, Option<String>)>),
    Include(String),
    Load(String),
    SetMember(Expr, String, Expr),
    Switch(Expr, Vec<(Expr, Vec<Stmt>)>, Option<Vec<Stmt>>),
    Expr(Expr),
}

#[derive(Clone, Debug)]
struct CatchClause {
    kind: Option<String>,
    var: Option<String>,
    body: Vec<Stmt>,
}
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}
impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }
    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }
    fn advance(&mut self) -> Kind {
        let kind = self.current().kind.clone();
        if !matches!(kind, Kind::Eof) {
            self.pos += 1
        }
        kind
    }
    fn same(a: &Kind, b: &Kind) -> bool {
        std::mem::discriminant(a) == std::mem::discriminant(b)
    }
    fn take(&mut self, kind: Kind) -> bool {
        if Self::same(&self.current().kind, &kind) {
            self.advance();
            true
        } else {
            false
        }
    }
    fn expect(&mut self, kind: Kind) -> Result<(), String> {
        if self.take(kind.clone()) {
            Ok(())
        } else {
            Err(format!(
                "{}:{}: expected {:?}",
                self.current().line,
                self.current().col,
                kind
            ))
        }
    }
    fn separators(&mut self) {
        while matches!(self.current().kind, Kind::Newline | Kind::Semi) {
            self.advance();
        }
    }
    fn starts_expression(&self) -> bool {
        matches!(
            self.current().kind,
            Kind::Ident(_)
                | Kind::Number(_)
                | Kind::String(_)
                | Kind::Interp(_)
                | Kind::True
                | Kind::False
                | Kind::Null
                | Kind::LParen
                | Kind::LBracket
                | Kind::LBrace
                | Kind::Not
                | Kind::Minus
                | Kind::Bang
                | Kind::Tilde
                | Kind::New
        )
    }
    fn program(&mut self) -> Result<Vec<Stmt>, String> {
        let mut list = vec![];
        self.separators();
        while !matches!(self.current().kind, Kind::Eof | Kind::RBrace) {
            list.push(self.stmt()?);
            self.separators();
        }
        Ok(list)
    }
    fn block(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect(Kind::LBrace)?;
        let body = self.program()?;
        self.expect(Kind::RBrace)?;
        Ok(body)
    }
    fn stmt(&mut self) -> Result<Stmt, String> {
        let (sl, sc) = (self.current().line, self.current().col);
        let mk = |kind: StmtKind| Stmt {
            kind,
            line: sl,
            col: sc,
        };
        match self.current().kind.clone() {
            Kind::Let | Kind::Const => {
                let is_const = matches!(self.current().kind, Kind::Const);
                self.advance();
                let target = if self.take(Kind::LBracket) {
                    let mut names = vec![];
                    if !self.take(Kind::RBracket) {
                        loop {
                            match self.advance() {
                                Kind::Ident(name) => names.push(name),
                                _ => return Err("expected variable name in list pattern".into()),
                            }
                            if self.take(Kind::Comma) {
                                if self.take(Kind::RBracket) {
                                    break;
                                }
                                continue;
                            }
                            break;
                        }
                        self.expect(Kind::RBracket)?;
                    }
                    LetTarget::List(names)
                } else if self.take(Kind::LBrace) {
                    let mut names = vec![];
                    if !self.take(Kind::RBrace) {
                        loop {
                            match self.advance() {
                                Kind::Ident(name) => names.push(name),
                                _ => return Err("expected variable name in dict pattern".into()),
                            }
                            if self.take(Kind::Comma) {
                                if self.take(Kind::RBrace) {
                                    break;
                                }
                                continue;
                            }
                            break;
                        }
                        self.expect(Kind::RBrace)?;
                    }
                    LetTarget::Dict(names)
                } else {
                    let first = match self.advance() {
                        Kind::Ident(s) => s,
                        _ => return Err("expected variable name".into()),
                    };
                    if self.take(Kind::Comma) {
                        let mut names = vec![first];
                        loop {
                            match self.advance() {
                                Kind::Ident(name) => names.push(name),
                                _ => return Err("expected variable name in list pattern".into()),
                            }
                            if !self.take(Kind::Comma) {
                                break;
                            }
                        }
                        self.expect(Kind::Assign)?;
                        let mut values = vec![self.expr()?];
                        while self.take(Kind::Comma) {
                            values.push(self.expr()?);
                        }
                        return Ok(mk(StmtKind::Let(
                            LetTarget::List(names),
                            Expr::List(values),
                            is_const,
                        )));
                    }
                    LetTarget::Var(first)
                };
                self.expect(Kind::Assign)?;
                Ok(mk(StmtKind::Let(target, self.expr()?, is_const)))
            }
            Kind::Print => {
                self.advance();
                let mut values = vec![self.expr()?];
                while self.take(Kind::Comma) {
                    values.push(self.expr()?);
                }
                Ok(mk(StmtKind::Print(values)))
            }
            Kind::If => {
                self.advance();
                let cond = self.expr()?;
                let yes = self.block()?;
                self.separators();
                let no = if self.take(Kind::Else) {
                    self.separators();
                    if self.take(Kind::If) {
                        vec![self.if_tail()?]
                    } else {
                        self.block()?
                    }
                } else {
                    vec![]
                };
                Ok(mk(StmtKind::If(cond, yes, no)))
            }
            Kind::While => {
                self.advance();
                let cond = self.expr()?;
                Ok(mk(StmtKind::While(cond, self.block()?)))
            }
            Kind::For => {
                self.advance();
                let name = match self.advance() {
                    Kind::Ident(s) => s,
                    _ => return Err("expected loop variable".into()),
                };
                self.expect(Kind::In)?;
                let items = self.expr()?;
                Ok(mk(StmtKind::For(name, items, self.block()?)))
            }
            Kind::Break => {
                self.advance();
                Ok(mk(StmtKind::Break))
            }
            Kind::Continue => {
                self.advance();
                Ok(mk(StmtKind::Continue))
            }
            Kind::Function | Kind::Def => {
                self.advance();
                let name = match self.advance() {
                    Kind::Ident(name) => name,
                    _ => return Err("expected function name".into()),
                };
                self.expect(Kind::LParen)?;
                let mut params = vec![];
                if !self.take(Kind::RParen) {
                    loop {
                        match self.advance() {
                            Kind::Ident(name) => params.push(name),
                            _ => return Err("expected parameter name".into()),
                        }
                        if !self.take(Kind::Comma) {
                            break;
                        }
                    }
                    self.expect(Kind::RParen)?;
                }
                Ok(mk(StmtKind::Function(name, params, self.block()?)))
            }
            Kind::Native => {
                self.advance();
                self.expect(Kind::Function)?;
                let name = match self.advance() {
                    Kind::Ident(name) => name,
                    _ => return Err("expected native function name".into()),
                };
                self.expect(Kind::LParen)?;
                let mut params = vec![];
                if !self.take(Kind::RParen) {
                    loop {
                        match self.advance() {
                            Kind::Ident(name) => params.push(name),
                            _ => return Err("expected parameter name".into()),
                        }
                        if !self.take(Kind::Comma) {
                            break;
                        }
                    }
                    self.expect(Kind::RParen)?;
                }
                Ok(mk(StmtKind::Native(name, params)))
            }
            Kind::Return => {
                self.advance();
                let value = if matches!(
                    self.current().kind,
                    Kind::Newline | Kind::Semi | Kind::RBrace | Kind::Eof
                ) {
                    None
                } else {
                    Some(self.expr()?)
                };
                Ok(mk(StmtKind::Return(value)))
            }
            Kind::Import => {
                self.advance();
                let mut imports = vec![];
                loop {
                    let module = match self.advance() {
                        Kind::Ident(name) => name,
                        _ => return Err("expected module name".into()),
                    };
                    let alias = if self.take(Kind::As) {
                        match self.advance() {
                            Kind::Ident(name) => Some(name),
                            _ => return Err("expected alias name".into()),
                        }
                    } else {
                        None
                    };
                    imports.push((module, alias));
                    if !self.take(Kind::Comma) {
                        break;
                    }
                }
                Ok(mk(StmtKind::Import(imports)))
            }
            Kind::From => {
                self.advance();
                let module = match self.advance() {
                    Kind::Ident(name) => name,
                    _ => return Err("expected module name".into()),
                };
                self.expect(Kind::Import)?;
                let mut items = vec![];
                loop {
                    let item = match self.advance() {
                        Kind::Ident(name) => name,
                        _ => return Err("expected item name".into()),
                    };
                    let alias = if self.take(Kind::As) {
                        match self.advance() {
                            Kind::Ident(name) => Some(name),
                            _ => return Err("expected alias name".into()),
                        }
                    } else {
                        None
                    };
                    items.push((item, alias));
                    if !self.take(Kind::Comma) {
                        break;
                    }
                }
                Ok(mk(StmtKind::FromImport(module, items)))
            }
            Kind::Include | Kind::Load => {
                let kind = self.advance();
                let path = match self.advance() {
                    Kind::String(s) => s,
                    Kind::Ident(name) => name,
                    _ => return Err("expected file path string or module name".into()),
                };
                if matches!(kind, Kind::Include) {
                    Ok(mk(StmtKind::Include(path)))
                } else {
                    Ok(mk(StmtKind::Load(path)))
                }
            }
            Kind::Class => {
                self.advance();
                let name = match self.advance() {
                    Kind::Ident(name) => name,
                    _ => return Err("expected class name".into()),
                };
                let parent = if self.take(Kind::Extends) {
                    let mut name = match self.advance() {
                        Kind::Ident(name) => name,
                        _ => return Err("expected parent class name".into()),
                    };
                    while self.take(Kind::Dot) {
                        let part = match self.advance() {
                            Kind::Ident(name) => name,
                            _ => return Err("expected parent class member after '.'".into()),
                        };
                        name = format!("{name}.{part}");
                    }
                    Some(name)
                } else {
                    None
                };
                Ok(mk(StmtKind::Class(name, parent, self.block()?)))
            }
            Kind::Switch => {
                self.advance();
                let value = self.expr()?;
                self.separators();
                self.expect(Kind::LBrace)?;
                let mut cases = vec![];
                let mut default_body = None;
                self.separators();
                while !matches!(self.current().kind, Kind::RBrace | Kind::Eof) {
                    if self.take(Kind::Case) {
                        let case_value = self.expr()?;
                        self.separators();
                        let body = if self.take(Kind::Colon) {
                            self.separators();
                            let mut body = vec![];
                            while !matches!(
                                self.current().kind,
                                Kind::Case | Kind::Default | Kind::RBrace | Kind::Eof
                            ) {
                                if matches!(
                                    self.current().kind,
                                    Kind::Newline | Kind::Semi
                                ) {
                                    self.advance();
                                    continue;
                                }
                                body.push(self.stmt()?);
                                self.separators();
                            }
                            body
                        } else {
                            self.block()?
                        };
                        cases.push((case_value, body));
                    } else if self.take(Kind::Default) {
                        self.separators();
                        default_body = if self.take(Kind::Colon) {
                            self.separators();
                            let mut body = vec![];
                            while !matches!(
                                self.current().kind,
                                Kind::Case | Kind::Default | Kind::RBrace | Kind::Eof
                            ) {
                                if matches!(
                                    self.current().kind,
                                    Kind::Newline | Kind::Semi
                                ) {
                                    self.advance();
                                    continue;
                                }
                                body.push(self.stmt()?);
                                self.separators();
                            }
                            Some(body)
                        } else {
                            Some(self.block()?)
                        };
                    } else {
                        return Err(format!(
                            "{}:{}: expected 'case' or 'default' in switch",
                            self.current().line,
                            self.current().col
                        ));
                    }
                    self.separators();
                }
                self.expect(Kind::RBrace)?;
                Ok(mk(StmtKind::Switch(value, cases, default_body)))
            }
            Kind::Ident(_) => {
                let expression = self.expr()?;
                if matches!(
                    self.current().kind,
                    Kind::Assign
                        | Kind::PlusAssign
                        | Kind::MinusAssign
                        | Kind::StarAssign
                        | Kind::SlashAssign
                        | Kind::PercentAssign
                        | Kind::AmpAssign
                        | Kind::PipeAssign
                        | Kind::CaretAssign
                        | Kind::LShiftAssign
                        | Kind::RShiftAssign
                        | Kind::NullishAssign
                ) {
                    let op = self.advance();
                    let value = self.expr()?;
                    match expression {
                        Expr::Var(name) => Ok(mk(StmtKind::Assign(name, op, value))),
                        Expr::Member(object, member) if matches!(op, Kind::Assign) => {
                            Ok(mk(StmtKind::SetMember(*object, member, value)))
                        }
                        _ => Err("invalid assignment target".into()),
                    }
                } else if let Expr::Var(name) = expression {
                    // Command-style call: `go "url"`, `wait 6`, `sleep 2`, `exit 1`
                    if self.starts_expression() {
                        let arg = self.expr()?;
                        Ok(mk(StmtKind::Expr(Expr::Call(
                            Box::new(Expr::Var(name)),
                            vec![arg],
                        ))))
                    } else {
                        Ok(mk(StmtKind::Expr(Expr::Var(name))))
                    }
                } else {
                    Ok(mk(StmtKind::Expr(expression)))
                }
            }
            Kind::Try => {
                self.advance();
                let body = self.block()?;
                self.separators();
                let mut catches: Vec<CatchClause> = vec![];
                while self.take(Kind::Catch) {
                    let mut kind = None;
                    let mut var = None;
                    // Optional error type: `catch TypeError as e`, `except errors.ValueError`
                    if !matches!(self.current().kind, Kind::LBrace | Kind::As) {
                        let mut name = match self.advance() {
                            Kind::Ident(name) => name,
                            _ => return Err("expected catch error type".into()),
                        };
                        while self.take(Kind::Dot) {
                            let part = match self.advance() {
                                Kind::Ident(name) => name,
                                _ => {
                                    return Err(
                                        "expected error type member after '.'".into()
                                    )
                                }
                            };
                            name = format!("{name}.{part}");
                        }
                        kind = Some(name);
                    }
                    // Optional binding: `catch as e`, `catch e`, `catch (e)`
                    if self.take(Kind::As) {
                        match self.advance() {
                            Kind::Ident(name) => var = Some(name),
                            _ => return Err("expected catch variable name after 'as'".into()),
                        }
                    } else if let Kind::Ident(name) = self.current().kind.clone() {
                        self.advance();
                        var = Some(name);
                    } else if self.take(Kind::LParen) {
                        match self.advance() {
                            Kind::Ident(name) => var = Some(name),
                            _ => return Err("expected catch variable name".into()),
                        }
                        self.expect(Kind::RParen)?;
                    }
                    let catch_body = self.block()?;
                    catches.push(CatchClause {
                        kind,
                        var,
                        body: catch_body,
                    });
                    self.separators();
                }
                let finally_body = if self.take(Kind::Finally) {
                    Some(self.block()?)
                } else {
                    None
                };
                Ok(mk(StmtKind::Try(body, catches, finally_body)))
            }
            Kind::Throw => {
                self.advance();
                let value = if matches!(
                    self.current().kind,
                    Kind::Newline | Kind::Semi | Kind::RBrace | Kind::Eof
                ) {
                    Expr::Value(Value::Null)
                } else {
                    self.expr()?
                };
                Ok(mk(StmtKind::Throw(value)))
            }
            _ => Ok(mk(StmtKind::Expr(self.expr()?))),
        }
    }
    fn if_tail(&mut self) -> Result<Stmt, String> {
        let (sl, sc) = (self.current().line, self.current().col);
        let condition = self.expr()?;
        let yes = self.block()?;
        self.separators();
        let no = if self.take(Kind::Else) {
            self.block()?
        } else {
            vec![]
        };
        Ok(Stmt {
            kind: StmtKind::If(condition, yes, no),
            line: sl,
            col: sc,
        })
    }
    fn expr(&mut self) -> Result<Expr, String> {
        let mut left = self.nullish()?;
        while self.take(Kind::Arrow) {
            left = Expr::Range(Box::new(left), Box::new(self.nullish()?), false);
        }
        while self.take(Kind::DotDot) {
            left = Expr::Range(Box::new(left), Box::new(self.nullish()?), true);
        }
        if self.take(Kind::Question) {
            let yes = self.expr()?;
            self.expect(Kind::Colon)?;
            let no = self.expr()?;
            left = Expr::Ternary(Box::new(left), Box::new(yes), Box::new(no));
        }
        Ok(left)
    }
    fn nullish(&mut self) -> Result<Expr, String> {
        let mut left = self.or()?;
        while self.take(Kind::Nullish) {
            left = Expr::Binary(Box::new(left), Kind::Nullish, Box::new(self.or()?));
        }
        Ok(left)
    }
    fn or(&mut self) -> Result<Expr, String> {
        let mut left = self.and()?;
        while self.take(Kind::Or) {
            left = Expr::Binary(Box::new(left), Kind::Or, Box::new(self.and()?));
        }
        Ok(left)
    }
    fn and(&mut self) -> Result<Expr, String> {
        let mut left = self.equality()?;
        while self.take(Kind::And) {
            left = Expr::Binary(Box::new(left), Kind::And, Box::new(self.equality()?));
        }
        Ok(left)
    }
    fn equality(&mut self) -> Result<Expr, String> {
        let mut left = self.comparison()?;
        while matches!(
            self.current().kind,
            Kind::Eq | Kind::Ne | Kind::StrictEq | Kind::StrictNe | Kind::Is
        ) {
            let op = self.advance();
            left = Expr::Binary(Box::new(left), op, Box::new(self.comparison()?));
        }
        Ok(left)
    }
    fn comparison(&mut self) -> Result<Expr, String> {
        let mut left = self.bit_or()?;
        while matches!(
            self.current().kind,
            Kind::Lt | Kind::Le | Kind::Gt | Kind::Ge | Kind::In
        ) {
            let op = self.advance();
            left = Expr::Binary(Box::new(left), op, Box::new(self.bit_or()?));
        }
        Ok(left)
    }
    fn bit_or(&mut self) -> Result<Expr, String> {
        let mut left = self.bit_xor()?;
        while self.take(Kind::Pipe) {
            left = Expr::Binary(Box::new(left), Kind::Pipe, Box::new(self.bit_xor()?));
        }
        Ok(left)
    }
    fn bit_xor(&mut self) -> Result<Expr, String> {
        let mut left = self.bit_and()?;
        while self.take(Kind::Caret) {
            left = Expr::Binary(Box::new(left), Kind::Caret, Box::new(self.bit_and()?));
        }
        Ok(left)
    }
    fn bit_and(&mut self) -> Result<Expr, String> {
        let mut left = self.shift()?;
        while self.take(Kind::Amp) {
            left = Expr::Binary(Box::new(left), Kind::Amp, Box::new(self.shift()?));
        }
        Ok(left)
    }
    fn shift(&mut self) -> Result<Expr, String> {
        let mut left = self.term()?;
        while matches!(self.current().kind, Kind::LShift | Kind::RShift) {
            let op = self.advance();
            left = Expr::Binary(Box::new(left), op, Box::new(self.term()?));
        }
        Ok(left)
    }
    fn term(&mut self) -> Result<Expr, String> {
        let mut left = self.factor()?;
        while matches!(self.current().kind, Kind::Plus | Kind::Minus) {
            let op = self.advance();
            left = Expr::Binary(Box::new(left), op, Box::new(self.factor()?));
        }
        Ok(left)
    }
    fn factor(&mut self) -> Result<Expr, String> {
        let left = self.unary()?;
        if self.take(Kind::Pow) {
            return Ok(Expr::Binary(
                Box::new(left),
                Kind::Pow,
                Box::new(self.factor()?),
            ));
        }
        let mut left = left;
        while matches!(
            self.current().kind,
            Kind::Star | Kind::Slash | Kind::Percent
        ) {
            let op = self.advance();
            left = Expr::Binary(Box::new(left), op, Box::new(self.unary()?));
        }
        Ok(left)
    }
    fn unary(&mut self) -> Result<Expr, String> {
        if matches!(
            self.current().kind,
            Kind::Minus | Kind::Bang | Kind::Not | Kind::Typeof | Kind::Tilde
        ) {
            let op = self.advance();
            Ok(Expr::Unary(op, Box::new(self.unary()?)))
        } else {
            self.postfix()
        }
    }
    fn postfix(&mut self) -> Result<Expr, String> {
        let mut left = self.atom()?;
        loop {
            if self.take(Kind::LBracket) {
                let index = self.expr()?;
                self.expect(Kind::RBracket)?;
                left = Expr::Index(Box::new(left), Box::new(index));
            } else if self.take(Kind::Dot) {
                let name = match self.advance() {
                    Kind::Ident(name) => name,
                    other => {
                        return Err(format!(
                            "{}:{}: expected member name, found {other:?}",
                            self.current().line,
                            self.current().col
                        ))
                    }
                };
                left = Expr::Member(Box::new(left), name);
            } else if self.take(Kind::SafeDot) {
                let name = match self.advance() {
                    Kind::Ident(name) => name,
                    other => {
                        return Err(format!(
                            "{}:{}: expected member name, found {other:?}",
                            self.current().line,
                            self.current().col
                        ))
                    }
                };
                left = Expr::SafeMember(Box::new(left), name);
            } else if self.take(Kind::Inc) {
                left = Expr::Increment(Box::new(left), 1);
            } else if self.take(Kind::Dec) {
                left = Expr::Increment(Box::new(left), -1);
            } else if self.take(Kind::LParen) {
                let mut args = vec![];
                if !self.take(Kind::RParen) {
                    args.push(self.arg()?);
                    while self.take(Kind::Comma) {
                        if self.take(Kind::RParen) {
                            break;
                        }
                        args.push(self.arg()?);
                    }
                    self.expect(Kind::RParen)?;
                }
                left = Expr::Call(Box::new(left), args);
            } else {
                break;
            }
        }
        Ok(left)
    }
    fn arg(&mut self) -> Result<Expr, String> {
        if matches!(self.current().kind, Kind::Ident(_))
            && self.tokens.get(self.pos + 1).map(|t| &t.kind) == Some(&Kind::Assign)
        {
            let name = match self.advance() {
                Kind::Ident(name) => name,
                _ => unreachable!(),
            };
            self.advance();
            let value = self.expr()?;
            return Ok(Expr::Named(name, Box::new(value)));
        }
        self.expr()
    }
    fn atom(&mut self) -> Result<Expr, String> {
        match self.advance() {
            Kind::Number(n) => Ok(Expr::Value(Value::Number(n))),
            Kind::String(s) => Ok(Expr::Value(Value::String(s))),
            Kind::Interp(parts) => {
                let mut pieces: Vec<Expr> = Vec::new();
                for part in parts {
                    let expr = match part {
                        InterpPart::Text(t) => Expr::Value(Value::String(t)),
                        InterpPart::Expr(src) => {
                            let tokens = lex(&src).map_err(|e| {
                                format!("{}:{}: in interpolation: {e}", self.current().line, self.current().col)
                            })?;
                            Parser::new(tokens).expr()?
                        }
                    };
                    pieces.push(expr);
                }
                let mut acc = pieces.remove(0);
                for piece in pieces {
                    acc = Expr::Binary(Box::new(acc), Kind::Plus, Box::new(piece));
                }
                Ok(acc)
            }
            Kind::True => Ok(Expr::Value(Value::Bool(true))),
            Kind::False => Ok(Expr::Value(Value::Bool(false))),
            Kind::Null => Ok(Expr::Value(Value::Null)),
            Kind::Ident(s) => Ok(Expr::Var(s)),
            Kind::Lambda => {
                let mut params = vec![];
                if self.take(Kind::LParen) {
                    if !self.take(Kind::RParen) {
                        loop {
                            match self.advance() {
                                Kind::Ident(name) => params.push(name),
                                _ => return Err("expected parameter name".into()),
                            }
                            if !self.take(Kind::Comma) {
                                break;
                            }
                        }
                        self.expect(Kind::RParen)?;
                    }
                } else if !matches!(self.current().kind, Kind::Colon | Kind::LBrace) {
                    match self.advance() {
                        Kind::Ident(name) => params.push(name),
                        _ => return Err("expected parameter name".into()),
                    }
                    while self.take(Kind::Comma) {
                        match self.advance() {
                            Kind::Ident(name) => params.push(name),
                            _ => return Err("expected parameter name".into()),
                        }
                    }
                }
                if self.take(Kind::LBrace) {
                    let body = self.program()?;
                    self.expect(Kind::RBrace)?;
                    Ok(Expr::Lambda(params, body))
                } else {
                    self.expect(Kind::Colon)?;
                    let body = self.expr()?;
                    Ok(Expr::Lambda(
                        params,
                        vec![Stmt {
                            kind: StmtKind::Return(Some(body)),
                            line: self.current().line,
                            col: self.current().col,
                        }],
                    ))
                }
            }
            Kind::New => {
                let mut class_name = match self.advance() {
                    Kind::Ident(name) => name,
                    _ => return Err("expected class name after 'new'".into()),
                };
                while self.take(Kind::Dot) {
                    let part = match self.advance() {
                        Kind::Ident(name) => name,
                        _ => return Err("expected class member name after '.'".into()),
                    };
                    class_name = format!("{class_name}.{part}");
                }
                self.expect(Kind::LParen)?;
                let mut args = vec![];
                if !self.take(Kind::RParen) {
                    args.push(self.expr()?);
                    while self.take(Kind::Comma) {
                        args.push(self.expr()?);
                    }
                    self.expect(Kind::RParen)?;
                }
                Ok(Expr::New(class_name, args))
            }
            Kind::LParen => {
                let e = self.expr()?;
                self.expect(Kind::RParen)?;
                Ok(e)
            }
            Kind::LBracket => {
                let mut values = vec![];
                if !self.take(Kind::RBracket) {
                    loop {
                        if self.take(Kind::Ellipsis) {
                            let spread = self.expr()?;
                            values.push(Expr::Spread(Box::new(spread)));
                        } else {
                            values.push(self.expr()?);
                        }
                        if self.take(Kind::Comma) {
                            if self.take(Kind::RBracket) {
                                break;
                            }
                            continue;
                        }
                        break;
                    }
                    self.expect(Kind::RBracket)?;
                }
                Ok(Expr::List(values))
            }
                Kind::LBrace => {
                    self.separators();
                    let mut entries = vec![];
                    if !self.take(Kind::RBrace) {
                        loop {
                            self.separators();
                            if self.take(Kind::Ellipsis) {
                                entries.push(DictEntry::Spread(self.expr()?));
                            } else {
                                let key = match self.advance() {
                                    Kind::String(key) | Kind::Ident(key) => key,
                                    other => {
                                        return Err(format!(
                                            "{}:{}: dictionary key must be a string or name, found {other:?}",
                                            self.current().line,
                                            self.current().col
                                        ))
                                    }
                                };
                                self.separators();
                                self.expect(Kind::Colon)?;
                                self.separators();
                                entries.push(DictEntry::Pair(key, self.expr()?));
                            }
                            self.separators();
                            if !self.take(Kind::Comma) {
                                break;
                            }
                            self.separators();
                            if self.take(Kind::RBrace) {
                                return Ok(Expr::Dict(entries));
                            }
                        }
                        self.expect(Kind::RBrace)?;
                    }
                    Ok(Expr::Dict(entries))
                }
            other => Err(format!(
                "{}:{}: expected expression, found {:?}",
                self.current().line,
                self.current().col,
                other
            )),
        }
    }
}

#[derive(Clone)]
struct Function {
    params: Vec<String>,
    body: Vec<Stmt>,
}
#[derive(Clone)]
struct ZenClass {
    parent: Option<String>,
    methods: HashMap<String, Function>,
}
type NativeFunc = fn(Vec<Value>) -> Result<Value, String>;

struct Vm {
    vars: HashMap<String, Value>,
    functions: HashMap<String, Function>,
    native_functions: HashMap<String, NativeFunc>,
    classes: HashMap<String, ZenClass>,
    imported_modules: HashMap<String, HashMap<String, Value>>,
    lambda_counter: u64,
    locked: std::collections::HashSet<String>,
    file: String,
    lines: Vec<String>,
    stack: Vec<String>,
}
enum Flow {
    Normal,
    Break,
    Continue,
    Return(Value),
    Throw(Value),
}
impl Vm {
    fn new() -> Vm {
        let mut vm = Vm {
            vars: HashMap::new(),
            functions: HashMap::new(),
            native_functions: HashMap::new(),
            classes: HashMap::new(),
            imported_modules: HashMap::new(),
            lambda_counter: 0,
            locked: std::collections::HashSet::new(),
            file: "<string>".into(),
            lines: Vec::new(),
            stack: vec!["<module>".into()],
        };
        vm.register_builtins();
        vm.register_error_classes();
        vm
    }

    /// Register the Python-style error hierarchy as classes. Each error class
    /// understands `new SomeError("message")` and supports custom subclasses:
    /// `class MyError extends errors.Error { }`.
    fn register_error_classes(&mut self) {
        let init = Function {
            params: vec!["message".into()],
            body: vec![Stmt {
                kind: StmtKind::SetMember(
                    Expr::Var("self".into()),
                    "message".into(),
                    Expr::Var("message".into()),
                ),
                line: 0,
                col: 0,
            }],
        };
        let mut register = |leaf: &str, parent: Option<&str>| {
            let parent_q = parent.map(|p| format!("errors.{p}"));
            let qualified = format!("errors.{leaf}");
            let mut methods = HashMap::new();
            methods.insert("init".into(), init.clone());
            let class = |methods: HashMap<String, Function>| ZenClass {
                parent: parent_q.clone(),
                methods,
            };
            self.classes
                .insert(leaf.into(), class(methods.clone()));
            self.classes.insert(qualified, class(methods));
        };
        register("Error", None);
        register("TypeError", Some("Error"));
        register("ValueError", Some("Error"));
        register("RangeError", Some("ValueError"));
        register("NameError", Some("Error"));
        register("LookupError", Some("Error"));
        register("KeyError", Some("LookupError"));
        register("IndexError", Some("LookupError"));
        register("ArithmeticError", Some("Error"));
        register("MathError", Some("ArithmeticError"));
        register("NumberError", Some("ArithmeticError"));
        register("ZeroDivisionError", Some("ArithmeticError"));
        register("OverflowError", Some("ArithmeticError"));
        register("IOError", Some("Error"));
        register("FileNotFoundError", Some("IOError"));
        register("ImportError", Some("Error"));
        register("KeyboardInterrupt", Some("Error"));
        register("RuntimeError", Some("Error"));
        register("NotImplementedError", Some("RuntimeError"));
        register("StopIteration", Some("Error"));
        register("RecursionError", Some("Error"));
        register("AssertionError", Some("Error"));
        register("SystemExit", Some("Error"));

        // The `errors` module: a dict exposing every error type so that
        // `import errors`, `print errors.ValueError`, etc. work.
        let mut errors_map = BTreeMap::new();
        for leaf in [
            "Error",
            "TypeError",
            "ValueError",
            "RangeError",
            "NameError",
            "LookupError",
            "KeyError",
            "IndexError",
            "ArithmeticError",
            "MathError",
            "NumberError",
            "ZeroDivisionError",
            "OverflowError",
            "IOError",
            "FileNotFoundError",
            "ImportError",
            "KeyboardInterrupt",
            "RuntimeError",
            "NotImplementedError",
            "StopIteration",
            "RecursionError",
            "AssertionError",
            "SystemExit",
        ] {
            errors_map.insert(leaf.into(), Value::String(leaf.into()));
        }
        self.vars.insert("errors".into(), Value::Dict(errors_map));
    }

    fn register_builtins(&mut self) {
        // str / type conversion
        self.native_functions.insert(
            "str".into(),
            |args| {
                Ok(Value::String(
                    args.first().cloned().unwrap_or(Value::Null).to_string(),
                ))
            },
        );
        self.native_functions.insert(
            "len".into(),
            |args| {
                let v = args.first().ok_or("len expects one argument")?;
                match v {
                    Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
                    Value::List(l) => Ok(Value::Number(l.len() as f64)),
                    Value::Dict(d) => Ok(Value::Number(d.len() as f64)),
                    _ => Err(format!("len() unsupported for {}", v)),
                }
            },
        );
        self.native_functions.insert(
            "range".into(),
            |args| {
                let (start, end) = match args.as_slice() {
                    [Value::Number(n)] => (0.0, *n),
                    [Value::Number(a), Value::Number(b)] => (*a, *b),
                    _ => return Err("range expects (end) or (start, end)".into()),
                };
                let mut values = Vec::new();
                let mut i = start as i64;
                let stop = end as i64;
                while i < stop {
                    values.push(Value::Number(i as f64));
                    i += 1;
                }
                Ok(Value::List(values))
            },
        );
        self.native_functions.insert(
            "int".into(),
            |args| {
                match args.first() {
                    Some(Value::Number(n)) => Ok(Value::Number(n.trunc())),
                    Some(Value::String(s)) => s
                        .trim()
                        .parse::<f64>()
                        .map(Value::Number)
                        .map_err(|_| format!("cannot parse int from {s:?}")),
                    Some(Value::Bool(b)) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
                    _ => Ok(Value::Number(0.0)),
                }
            },
        );
        self.native_functions.insert(
            "float".into(),
            |args| {
                match args.first() {
                    Some(Value::Number(n)) => Ok(Value::Number(*n)),
                    Some(Value::String(s)) => s
                        .trim()
                        .parse::<f64>()
                        .map(Value::Number)
                        .map_err(|_| format!("cannot parse float from {s:?}")),
                    _ => Ok(Value::Number(0.0)),
                }
            },
        );
        self.native_functions.insert(
            "bool".into(),
            |args| Ok(Value::Bool(args.first().map_or(false, |v| v.truthy()))),
        );
        self.native_functions.insert(
            "type".into(),
            |args| {
                Ok(Value::String(
                    match args.first().cloned().unwrap_or(Value::Null) {
                        Value::Null => "null",
                        Value::Bool(_) => "bool",
                        Value::Number(_) => "number",
                        Value::String(_) => "string",
                        Value::List(_) => "list",
                        Value::Dict(_) => "dict",
                        Value::Instance(_) => "object",
                        Value::Socket(_) => "socket",
                        Value::NativeFunction(_) | Value::Function(_) => "function",
                    }
                    .into(),
                ))
            },
        );
        self.native_functions.insert(
            "sleep".into(),
            |args| {
                let secs = match args.first() {
                    Some(Value::Number(n)) => *n,
                    _ => return Err("sleep expects a number of seconds".into()),
                };
                std::thread::sleep(std::time::Duration::from_secs_f64(secs));
                Ok(Value::Null)
            },
        );
        self.native_functions.insert(
            "input".into(),
            |args| {
                let prompt = args
                    .first()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                use std::io::Write;
                print!("{prompt}");
                std::io::stdout().flush().ok();
                let mut line = String::new();
                std::io::stdin()
                    .read_line(&mut line)
                    .map_err(|e| format!("failed to read input: {e}"))?;
                Ok(Value::String(line.trim_end().to_string()))
            },
        );
        self.native_functions.insert(
            "exit".into(),
            |args| {
                let code = match args.first() {
                    Some(Value::Number(n)) => *n as i32,
                    _ => 0,
                };
                std::process::exit(code);
            },
        );

        // json module
        let json = Value::Dict(BTreeMap::from([
            ("parse".into(), Value::NativeFunction("json_decode".into())),
            ("encode".into(), Value::NativeFunction("json_encode".into())),
            ("load".into(), Value::NativeFunction("json_load".into())),
            ("save".into(), Value::NativeFunction("json_save".into())),
        ]));
        self.vars.insert("json".into(), json);

        // fs module
        let fs = Value::Dict(BTreeMap::from([
            ("list".into(), Value::NativeFunction("fs_list_dir".into())),
            ("read".into(), Value::NativeFunction("fs_read".into())),
            ("write".into(), Value::NativeFunction("fs_write".into())),
            ("append".into(), Value::NativeFunction("fs_append".into())),
            ("read_binary".into(), Value::NativeFunction("fs_read_binary".into())),
            ("readBinary".into(), Value::NativeFunction("fs_read_binary".into())),
            ("write_binary".into(), Value::NativeFunction("fs_write_binary".into())),
            ("writeBinary".into(), Value::NativeFunction("fs_write_binary".into())),
            ("exists".into(), Value::NativeFunction("fs_exists".into())),
            ("is_file".into(), Value::NativeFunction("fs_is_file".into())),
            ("isFile".into(), Value::NativeFunction("fs_is_file".into())),
            ("is_dir".into(), Value::NativeFunction("fs_is_dir".into())),
            ("isDir".into(), Value::NativeFunction("fs_is_dir".into())),
            ("size".into(), Value::NativeFunction("fs_size".into())),
            ("mtime".into(), Value::NativeFunction("fs_mtime".into())),
            ("mkdir".into(), Value::NativeFunction("fs_mkdir".into())),
            ("mkdirs".into(), Value::NativeFunction("fs_mkdir".into())),
            ("remove".into(), Value::NativeFunction("fs_remove".into())),
            ("rmdir".into(), Value::NativeFunction("fs_rmdir".into())),
            ("rmtree".into(), Value::NativeFunction("fs_rmtree".into())),
            ("copy".into(), Value::NativeFunction("fs_copy".into())),
            ("move".into(), Value::NativeFunction("fs_move".into())),
            ("rename".into(), Value::NativeFunction("fs_move".into())),
            ("glob".into(), Value::NativeFunction("fs_glob".into())),
            ("join".into(), Value::NativeFunction("fs_join".into())),
            ("basename".into(), Value::NativeFunction("fs_basename".into())),
            ("dirname".into(), Value::NativeFunction("fs_dirname".into())),
            ("cwd".into(), Value::NativeFunction("os_cwd".into())),
            ("cd".into(), Value::NativeFunction("fs_cd".into())),
        ]));
        self.vars.insert("fs".into(), fs);

        // re module
        let re = Value::Dict(BTreeMap::from([
            ("match".into(), Value::NativeFunction("regex_match".into())),
            ("matches".into(), Value::NativeFunction("regex_match".into())),
            ("search".into(), Value::NativeFunction("regex_search".into())),
            ("find".into(), Value::NativeFunction("regex_find".into())),
            ("findall".into(), Value::NativeFunction("regex_find".into())),
            ("split".into(), Value::NativeFunction("regex_split".into())),
            ("replace".into(), Value::NativeFunction("regex_replace".into())),
            ("sub".into(), Value::NativeFunction("regex_replace".into())),
        ]));
        self.vars.insert("re".into(), re);

        // random module
        let random = Value::Dict(BTreeMap::from([
            ("random".into(), Value::NativeFunction("random_random".into())),
            ("randint".into(), Value::NativeFunction("random_randint".into())),
            ("randrange".into(), Value::NativeFunction("random_randrange".into())),
            ("choice".into(), Value::NativeFunction("random_choice".into())),
            ("choices".into(), Value::NativeFunction("random_choices".into())),
            ("sample".into(), Value::NativeFunction("random_sample".into())),
            ("shuffle".into(), Value::NativeFunction("random_shuffle".into())),
            ("uniform".into(), Value::NativeFunction("random_uniform".into())),
            ("hex".into(), Value::NativeFunction("random_hex".into())),
            ("seed".into(), Value::NativeFunction("random_seed".into())),
        ]));
        self.vars.insert("random".into(), random);

        // math module
        let math = Value::Dict(BTreeMap::from([
            ("pi".into(), Value::Number(std::f64::consts::PI)),
            ("e".into(), Value::Number(std::f64::consts::E)),
            ("inf".into(), Value::Number(f64::INFINITY)),
            ("nan".into(), Value::Number(f64::NAN)),
            ("floor".into(), Value::NativeFunction("math_floor".into())),
            ("ceil".into(), Value::NativeFunction("math_ceil".into())),
            ("trunc".into(), Value::NativeFunction("math_trunc".into())),
            ("sqrt".into(), Value::NativeFunction("math_sqrt".into())),
            ("abs".into(), Value::NativeFunction("math_abs".into())),
            ("pow".into(), Value::NativeFunction("math_pow".into())),
            ("exp".into(), Value::NativeFunction("math_exp".into())),
            ("log".into(), Value::NativeFunction("math_log".into())),
            ("log2".into(), Value::NativeFunction("math_log2".into())),
            ("log10".into(), Value::NativeFunction("math_log10".into())),
            ("sin".into(), Value::NativeFunction("math_sin".into())),
            ("cos".into(), Value::NativeFunction("math_cos".into())),
            ("tan".into(), Value::NativeFunction("math_tan".into())),
            ("asin".into(), Value::NativeFunction("math_asin".into())),
            ("acos".into(), Value::NativeFunction("math_acos".into())),
            ("atan".into(), Value::NativeFunction("math_atan".into())),
            ("atan2".into(), Value::NativeFunction("math_atan2".into())),
            ("degrees".into(), Value::NativeFunction("math_degrees".into())),
            ("radians".into(), Value::NativeFunction("math_radians".into())),
            ("hypot".into(), Value::NativeFunction("math_hypot".into())),
            ("isnan".into(), Value::NativeFunction("math_isnan".into())),
            ("isfinite".into(), Value::NativeFunction("math_isfinite".into())),
            ("isinf".into(), Value::NativeFunction("math_isinf".into())),
            ("copysign".into(), Value::NativeFunction("math_copysign".into())),
            ("gcd".into(), Value::NativeFunction("math_gcd".into())),
            ("lcm".into(), Value::NativeFunction("math_lcm".into())),
            ("factorial".into(), Value::NativeFunction("math_factorial".into())),
            ("comb".into(), Value::NativeFunction("math_comb".into())),
            ("perm".into(), Value::NativeFunction("math_perm".into())),
            ("remainder".into(), Value::NativeFunction("math_remainder".into())),
            ("fsum".into(), Value::NativeFunction("math_fsum".into())),
            ("prod".into(), Value::NativeFunction("math_prod".into())),
            ("modf".into(), Value::NativeFunction("math_modf".into())),
            ("frexp".into(), Value::NativeFunction("math_frexp".into())),
            ("ldexp".into(), Value::NativeFunction("math_ldexp".into())),
            ("round".into(), Value::NativeFunction("math_round".into())),
            ("min".into(), Value::NativeFunction("math_min".into())),
            ("max".into(), Value::NativeFunction("math_max".into())),
        ]));
        self.vars.insert("math".into(), math);

        // time module
        let time = Value::Dict(BTreeMap::from([
            ("now".into(), Value::NativeFunction("time_now".into())),
            ("unix".into(), Value::NativeFunction("time_unix".into())),
            ("utc".into(), Value::NativeFunction("time_utc".into())),
            ("date".into(), Value::NativeFunction("time_date".into())),
            ("format".into(), Value::NativeFunction("time_format".into())),
            ("parse".into(), Value::NativeFunction("time_parse".into())),
            ("sleep".into(), Value::NativeFunction("time_sleep".into())),
            ("wait".into(), Value::NativeFunction("time_wait".into())),
            ("year".into(), Value::NativeFunction("time_year".into())),
            ("month".into(), Value::NativeFunction("time_month".into())),
            ("day".into(), Value::NativeFunction("time_day".into())),
            ("hour".into(), Value::NativeFunction("time_hour".into())),
            ("minute".into(), Value::NativeFunction("time_minute".into())),
            ("second".into(), Value::NativeFunction("time_second".into())),
            ("weekday".into(), Value::NativeFunction("time_weekday".into())),
            ("timestamp".into(), Value::NativeFunction("time_unix".into())),
        ]));
        self.vars.insert("time".into(), time);

        // os module
        let os = Value::Dict(BTreeMap::from([
            ("env".into(), Value::NativeFunction("os_getenv".into())),
            ("getenv".into(), Value::NativeFunction("os_getenv".into())),
            ("setenv".into(), Value::NativeFunction("os_setenv".into())),
            ("unsetenv".into(), Value::NativeFunction("os_unsetenv".into())),
            ("exit".into(), Value::NativeFunction("exit".into())),
            ("platform".into(), Value::NativeFunction("os_platform".into())),
            ("hostname".into(), Value::NativeFunction("os_hostname".into())),
            ("pid".into(), Value::NativeFunction("os_pid".into())),
            ("cwd".into(), Value::NativeFunction("os_cwd".into())),
            ("chdir".into(), Value::NativeFunction("fs_cd".into())),
            ("name".into(), Value::NativeFunction("os_platform".into())),
            ("sep".into(), Value::String(std::path::MAIN_SEPARATOR.to_string())),
            ("linesep".into(), Value::String("\n".into())),
            ("cpu_count".into(), Value::NativeFunction("os_cpu_count".into())),
            ("system".into(), Value::NativeFunction("os_system".into())),
            ("home".into(), Value::NativeFunction("os_home".into())),
        ]));
        self.vars.insert("os".into(), os);

        // base64 module
        let base64 = Value::Dict(BTreeMap::from([
            ("encode".into(), Value::NativeFunction("b64_encode".into())),
            ("decode".into(), Value::NativeFunction("b64_decode".into())),
            ("url_encode".into(), Value::NativeFunction("b64_url_encode".into())),
            ("url_decode".into(), Value::NativeFunction("b64_url_decode".into())),
        ]));
        self.vars.insert("base64".into(), base64);

        // base32 module
        let base32 = Value::Dict(BTreeMap::from([
            ("encode".into(), Value::NativeFunction("b32_encode".into())),
            ("decode".into(), Value::NativeFunction("b32_decode".into())),
        ]));
        self.vars.insert("base32".into(), base32);

        // crypto module (hashes + hmac + aes)
        let crypto = Value::Dict(BTreeMap::from([
            ("sha256".into(), Value::NativeFunction("crypto_sha256".into())),
            ("sha1".into(), Value::NativeFunction("crypto_sha1".into())),
            ("md5".into(), Value::NativeFunction("crypto_md5".into())),
            ("sha512".into(), Value::NativeFunction("crypto_sha512".into())),
            ("sha224".into(), Value::NativeFunction("crypto_sha224".into())),
            ("sha384".into(), Value::NativeFunction("crypto_sha384".into())),
            ("sha3_256".into(), Value::NativeFunction("crypto_sha3_256".into())),
            ("sha3_512".into(), Value::NativeFunction("crypto_sha3_512".into())),
            ("blake2b".into(), Value::NativeFunction("crypto_blake2b".into())),
            ("blake2s".into(), Value::NativeFunction("crypto_blake2s".into())),
            ("hmac_sha256".into(), Value::NativeFunction("crypto_hmac_sha256".into())),
            ("hmac_sha1".into(), Value::NativeFunction("crypto_hmac_sha1".into())),
            ("hmac_md5".into(), Value::NativeFunction("crypto_hmac_md5".into())),
            ("random_bytes".into(), Value::NativeFunction("crypto_random_bytes".into())),
            ("random_hex".into(), Value::NativeFunction("crypto_random_hex".into())),
            ("pbkdf2".into(), Value::NativeFunction("crypto_pbkdf2".into())),
            ("aes_encrypt".into(), Value::NativeFunction("crypto_aes_encrypt".into())),
            ("aes_decrypt".into(), Value::NativeFunction("crypto_aes_decrypt".into())),
        ]));
        self.vars.insert("crypto".into(), crypto);

        // cryptography module (Fernet symmetric encryption)
        let cryptography = Value::Dict(BTreeMap::from([(
            "fernet".into(),
            Value::Dict(BTreeMap::from([
                ("generate_key".into(), Value::NativeFunction("fernet_generate_key".into())),
                ("encrypt".into(), Value::NativeFunction("fernet_encrypt".into())),
                ("decrypt".into(), Value::NativeFunction("fernet_decrypt".into())),
            ])),
        )]));
        self.vars.insert("cryptography".into(), cryptography);

        // datetime module
        let datetime = Value::Dict(BTreeMap::from([
            ("now".into(), Value::NativeFunction("time_now".into())),
            ("utcnow".into(), Value::NativeFunction("time_utc".into())),
            ("today".into(), Value::NativeFunction("time_date".into())),
            ("unix".into(), Value::NativeFunction("time_unix".into())),
            ("from_unix".into(), Value::NativeFunction("time_from_unix".into())),
            ("parse".into(), Value::NativeFunction("time_parse".into())),
            ("format".into(), Value::NativeFunction("time_format".into())),
            ("year".into(), Value::NativeFunction("time_year".into())),
            ("month".into(), Value::NativeFunction("time_month".into())),
            ("day".into(), Value::NativeFunction("time_day".into())),
            ("hour".into(), Value::NativeFunction("time_hour".into())),
            ("minute".into(), Value::NativeFunction("time_minute".into())),
            ("second".into(), Value::NativeFunction("time_second".into())),
            ("weekday".into(), Value::NativeFunction("time_weekday".into())),
            ("add_days".into(), Value::NativeFunction("time_add_days".into())),
            ("MONDAY".into(), Value::Number(0.0)),
            ("TUESDAY".into(), Value::Number(1.0)),
            ("WEDNESDAY".into(), Value::Number(2.0)),
            ("THURSDAY".into(), Value::Number(3.0)),
            ("FRIDAY".into(), Value::Number(4.0)),
            ("SATURDAY".into(), Value::Number(5.0)),
            ("SUNDAY".into(), Value::Number(6.0)),
        ]));
        self.vars.insert("datetime".into(), datetime);

        // uuid module
        let uuid = Value::Dict(BTreeMap::from([
            ("uuid4".into(), Value::NativeFunction("uuid_uuid4".into())),
            ("uuid1".into(), Value::NativeFunction("uuid_uuid1".into())),
            ("uuid3".into(), Value::NativeFunction("uuid_uuid3".into())),
            ("uuid5".into(), Value::NativeFunction("uuid_uuid5".into())),
            ("NAMESPACE_DNS".into(), Value::String("dns".into())),
            ("NAMESPACE_URL".into(), Value::String("url".into())),
            ("NAMESPACE_OID".into(), Value::String("oid".into())),
            ("NAMESPACE_X500".into(), Value::String("x500".into())),
        ]));
        self.vars.insert("uuid".into(), uuid);

        // color module (ANSI helpers)
        let color = Value::Dict(BTreeMap::from([
            ("reset".into(), Value::String("\x1b[0m".into())),
            ("bold".into(), Value::NativeFunction("color_style_bold".into())),
            ("dim".into(), Value::NativeFunction("color_style_dim".into())),
            ("italic".into(), Value::NativeFunction("color_style_italic".into())),
            ("underline".into(), Value::NativeFunction("color_style_underline".into())),
            ("blink".into(), Value::NativeFunction("color_style_blink".into())),
            ("reverse".into(), Value::NativeFunction("color_style_reverse".into())),
            ("hidden".into(), Value::NativeFunction("color_style_hidden".into())),
            ("strike".into(), Value::NativeFunction("color_style_strike".into())),
            ("rgb".into(), Value::NativeFunction("color_rgb".into())),
            ("bg_rgb".into(), Value::NativeFunction("color_bg_rgb".into())),
            ("hex".into(), Value::NativeFunction("color_hex".into())),
            ("strip".into(), Value::NativeFunction("color_strip".into())),
            ("black".into(), Value::NativeFunction("color_fg_black".into())),
            ("red".into(), Value::NativeFunction("color_fg_red".into())),
            ("green".into(), Value::NativeFunction("color_fg_green".into())),
            ("yellow".into(), Value::NativeFunction("color_fg_yellow".into())),
            ("blue".into(), Value::NativeFunction("color_fg_blue".into())),
            ("magenta".into(), Value::NativeFunction("color_fg_magenta".into())),
            ("cyan".into(), Value::NativeFunction("color_fg_cyan".into())),
            ("white".into(), Value::NativeFunction("color_fg_white".into())),
            ("bg_black".into(), Value::NativeFunction("color_bg_black".into())),
            ("bg_red".into(), Value::NativeFunction("color_bg_red".into())),
            ("bg_green".into(), Value::NativeFunction("color_bg_green".into())),
            ("bg_yellow".into(), Value::NativeFunction("color_bg_yellow".into())),
            ("bg_blue".into(), Value::NativeFunction("color_bg_blue".into())),
            ("bg_magenta".into(), Value::NativeFunction("color_bg_magenta".into())),
            ("bg_cyan".into(), Value::NativeFunction("color_bg_cyan".into())),
            ("bg_white".into(), Value::NativeFunction("color_bg_white".into())),
            ("bright_black".into(), Value::NativeFunction("color_fg_bright_black".into())),
            ("bright_red".into(), Value::NativeFunction("color_fg_bright_red".into())),
            ("bright_green".into(), Value::NativeFunction("color_fg_bright_green".into())),
            ("bright_yellow".into(), Value::NativeFunction("color_fg_bright_yellow".into())),
            ("bright_blue".into(), Value::NativeFunction("color_fg_bright_blue".into())),
            ("bright_magenta".into(), Value::NativeFunction("color_fg_bright_magenta".into())),
            ("bright_cyan".into(), Value::NativeFunction("color_fg_bright_cyan".into())),
            ("bright_white".into(), Value::NativeFunction("color_fg_bright_white".into())),
        ]));
        self.vars.insert("color".into(), color);

        // csv module
        let csv = Value::Dict(BTreeMap::from([
            ("read".into(), Value::NativeFunction("csv_read".into())),
            ("write".into(), Value::NativeFunction("csv_write".into())),
            ("parse".into(), Value::NativeFunction("csv_parse".into())),
            ("encode".into(), Value::NativeFunction("csv_encode".into())),
        ]));
        self.vars.insert("csv".into(), csv);

        // http module
        let http = Value::Dict(BTreeMap::from([
            ("get".into(), Value::NativeFunction("http_get".into())),
            ("post".into(), Value::NativeFunction("http_post".into())),
            ("put".into(), Value::NativeFunction("http_put".into())),
            ("del".into(), Value::NativeFunction("http_del".into())),
            ("head".into(), Value::NativeFunction("http_head".into())),
            ("patch".into(), Value::NativeFunction("http_patch".into())),
        ]));
        self.vars.insert("http".into(), http);

        // decimal module
        let decimal = Value::Dict(BTreeMap::from([
            ("Decimal".into(), Value::NativeFunction("decimal_decimal".into())),
            ("getcontext".into(), Value::NativeFunction("decimal_getcontext".into())),
            ("setcontext".into(), Value::NativeFunction("decimal_setcontext".into())),
            ("localcontext".into(), Value::NativeFunction("decimal_localcontext".into())),
            ("ROUND_HALF_UP".into(), Value::String("ROUND_HALF_UP".into())),
            ("ROUND_HALF_EVEN".into(), Value::String("ROUND_HALF_EVEN".into())),
            ("ROUND_DOWN".into(), Value::String("ROUND_DOWN".into())),
            ("ROUND_UP".into(), Value::String("ROUND_UP".into())),
            ("ROUND_CEILING".into(), Value::String("ROUND_CEILING".into())),
            ("ROUND_FLOOR".into(), Value::String("ROUND_FLOOR".into())),
            ("ROUND_HALF_DOWN".into(), Value::String("ROUND_HALF_DOWN".into())),
            ("ROUND_05UP".into(), Value::String("ROUND_05UP".into())),
        ]));
        self.vars.insert("decimal".into(), decimal);

        // threading module
        let threading = Value::Dict(BTreeMap::from([
            ("start".into(), Value::NativeFunction("threading_start".into())),
        ]));
        self.vars.insert("threading".into(), threading);

        // statistics module
        let statistics = Value::Dict(BTreeMap::from([
            ("mean".into(), Value::NativeFunction("statistics_mean".into())),
            ("median".into(), Value::NativeFunction("statistics_median".into())),
            ("mode".into(), Value::NativeFunction("statistics_mode".into())),
            ("stdev".into(), Value::NativeFunction("statistics_stdev".into())),
            ("variance".into(), Value::NativeFunction("statistics_variance".into())),
            ("min".into(), Value::NativeFunction("math_min".into())),
            ("max".into(), Value::NativeFunction("math_max".into())),
            ("sum".into(), Value::NativeFunction("statistics_sum".into())),
        ]));
        self.vars.insert("statistics".into(), statistics);

        // socket module
        let socket = Value::Dict(BTreeMap::from([
            ("open".into(), Value::NativeFunction("socket_open".into())),
            ("send".into(), Value::NativeFunction("socket_send".into())),
            ("recv".into(), Value::NativeFunction("socket_recv".into())),
            ("close".into(), Value::NativeFunction("socket_close".into())),
        ]));
        self.vars.insert("socket".into(), socket);

        // ftp module (pure-Rust FTP client)
        let ftp = Value::Dict(BTreeMap::from([
            ("connect".into(), Value::NativeFunction("ftp_connect".into())),
            ("login".into(), Value::NativeFunction("ftp_login".into())),
            ("pwd".into(), Value::NativeFunction("ftp_pwd".into())),
            ("list".into(), Value::NativeFunction("ftp_list".into())),
            ("nlist".into(), Value::NativeFunction("ftp_nlist".into())),
            ("cwd".into(), Value::NativeFunction("ftp_cwd".into())),
            ("retr".into(), Value::NativeFunction("ftp_retr".into())),
            ("stor".into(), Value::NativeFunction("ftp_stor".into())),
            ("dele".into(), Value::NativeFunction("ftp_dele".into())),
            ("mkdir".into(), Value::NativeFunction("ftp_mkdir".into())),
            ("rmdir".into(), Value::NativeFunction("ftp_rmdir".into())),
            ("rename".into(), Value::NativeFunction("ftp_rename".into())),
            ("quit".into(), Value::NativeFunction("ftp_quit".into())),
        ]));
        self.vars.insert("ftp".into(), ftp);

        // smtp module (pure-Rust SMTP client)
        let smtp = Value::Dict(BTreeMap::from([
            ("connect".into(), Value::NativeFunction("smtp_connect".into())),
            ("login".into(), Value::NativeFunction("smtp_login".into())),
            ("sendmail".into(), Value::NativeFunction("smtp_sendmail".into())),
            ("quit".into(), Value::NativeFunction("smtp_quit".into())),
            ("message".into(), Value::NativeFunction("smtp_message".into())),
        ]));
        self.vars.insert("smtp".into(), smtp);

        // pop3 module (pure-Rust POP3 client)
        let pop3 = Value::Dict(BTreeMap::from([
            ("connect".into(), Value::NativeFunction("pop3_connect".into())),
            ("stat".into(), Value::NativeFunction("pop3_stat".into())),
            ("list".into(), Value::NativeFunction("pop3_list".into())),
            ("retr".into(), Value::NativeFunction("pop3_retr".into())),
            ("dele".into(), Value::NativeFunction("pop3_dele".into())),
            ("quit".into(), Value::NativeFunction("pop3_quit".into())),
        ]));
        self.vars.insert("pop3".into(), pop3);

        // imap module (pure-Rust IMAP client)
        let imap = Value::Dict(BTreeMap::from([
            ("connect".into(), Value::NativeFunction("imap_connect".into())),
            ("select".into(), Value::NativeFunction("imap_select".into())),
            ("search".into(), Value::NativeFunction("imap_search".into())),
            ("fetch".into(), Value::NativeFunction("imap_fetch".into())),
            ("list".into(), Value::NativeFunction("imap_list".into())),
            ("logout".into(), Value::NativeFunction("imap_logout".into())),
        ]));
        self.vars.insert("imap".into(), imap);

        // telnet module (pure-Rust telnet client)
        let telnet = Value::Dict(BTreeMap::from([
            ("connect".into(), Value::NativeFunction("telnet_connect".into())),
            ("write".into(), Value::NativeFunction("telnet_write".into())),
            ("read".into(), Value::NativeFunction("telnet_read".into())),
            ("read_until".into(), Value::NativeFunction("telnet_read_until".into())),
            ("close".into(), Value::NativeFunction("telnet_close".into())),
        ]));
        self.vars.insert("telnet".into(), telnet);

        // dns module (pure-Rust DNS client)
        let dns = Value::Dict(BTreeMap::from([
            ("resolve".into(), Value::NativeFunction("dns_resolve".into())),
            ("lookup".into(), Value::NativeFunction("dns_resolve".into())),
            ("query".into(), Value::NativeFunction("dns_query".into())),
        ]));
        self.vars.insert("dns".into(), dns);

        // ssh module (wraps the system ssh/scp binaries)
        let ssh = Value::Dict(BTreeMap::from([
            ("run".into(), Value::NativeFunction("ssh_run".into())),
            ("upload".into(), Value::NativeFunction("ssh_upload".into())),
            ("download".into(), Value::NativeFunction("ssh_download".into())),
            ("available".into(), Value::NativeFunction("ssh_available".into())),
        ]));
        self.vars.insert("ssh".into(), ssh);

        // scapy module (packet crafting / sniffing)
        let scapy = Value::Dict(BTreeMap::from([
            ("checksum".into(), Value::NativeFunction("scapy_checksum".into())),
            ("ip".into(), Value::NativeFunction("scapy_ip".into())),
            ("tcp".into(), Value::NativeFunction("scapy_tcp".into())),
            ("udp".into(), Value::NativeFunction("scapy_udp".into())),
            ("icmp".into(), Value::NativeFunction("scapy_icmp".into())),
            ("raw".into(), Value::NativeFunction("scapy_raw".into())),
            ("build".into(), Value::NativeFunction("scapy_build".into())),
            ("parse".into(), Value::NativeFunction("scapy_parse".into())),
            ("send".into(), Value::NativeFunction("scapy_send".into())),
            ("sniff".into(), Value::NativeFunction("scapy_sniff".into())),
            ("ip_to_int".into(), Value::NativeFunction("scapy_ip_to_int".into())),
            ("int_to_ip".into(), Value::NativeFunction("scapy_int_to_ip".into())),
        ]));
        self.vars.insert("scapy".into(), scapy);

        // string module (Python string helpers + constants)
        let string = Value::Dict(BTreeMap::from([
            ("upper".into(), Value::NativeFunction("str_upper".into())),
            ("lower".into(), Value::NativeFunction("str_lower".into())),
            ("title".into(), Value::NativeFunction("str_title".into())),
            ("capitalize".into(), Value::NativeFunction("str_capitalize".into())),
            ("swapcase".into(), Value::NativeFunction("str_swapcase".into())),
            ("strip".into(), Value::NativeFunction("str_strip".into())),
            ("lstrip".into(), Value::NativeFunction("str_lstrip".into())),
            ("rstrip".into(), Value::NativeFunction("str_rstrip".into())),
            ("split".into(), Value::NativeFunction("str_split".into())),
            ("splitlines".into(), Value::NativeFunction("str_splitlines".into())),
            ("join".into(), Value::NativeFunction("str_join".into())),
            ("replace".into(), Value::NativeFunction("str_replace".into())),
            ("count".into(), Value::NativeFunction("str_count".into())),
            ("find".into(), Value::NativeFunction("str_find".into())),
            ("rfind".into(), Value::NativeFunction("str_rfind".into())),
            ("startswith".into(), Value::NativeFunction("str_startswith".into())),
            ("endswith".into(), Value::NativeFunction("str_endswith".into())),
            ("contains".into(), Value::NativeFunction("str_contains".into())),
            ("ljust".into(), Value::NativeFunction("str_ljust".into())),
            ("rjust".into(), Value::NativeFunction("str_rjust".into())),
            ("center".into(), Value::NativeFunction("str_center".into())),
            ("zfill".into(), Value::NativeFunction("str_zfill".into())),
            ("repeat".into(), Value::NativeFunction("str_repeat".into())),
            ("isdigit".into(), Value::NativeFunction("str_isdigit".into())),
            ("isalpha".into(), Value::NativeFunction("str_isalpha".into())),
            ("isalnum".into(), Value::NativeFunction("str_isalnum".into())),
            ("isspace".into(), Value::NativeFunction("str_isspace".into())),
            ("islower".into(), Value::NativeFunction("str_islower".into())),
            ("isupper".into(), Value::NativeFunction("str_isupper".into())),
            ("digits".into(), Value::String("0123456789".into())),
            ("hexdigits".into(), Value::String("0123456789abcdefABCDEF".into())),
            ("octdigits".into(), Value::String("01234567".into())),
            ("ascii_letters".into(), Value::String("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".into())),
            ("ascii_lowercase".into(), Value::String("abcdefghijklmnopqrstuvwxyz".into())),
            ("ascii_uppercase".into(), Value::String("ABCDEFGHIJKLMNOPQRSTUVWXYZ".into())),
            ("punctuation".into(), Value::String("!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~".into())),
            ("whitespace".into(), Value::String(" \t\n\r\x0b\x0c".into())),
            ("printable".into(), Value::String("0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~ \t\n\r\x0b\x0c".into())),
        ]));
        self.vars.insert("string".into(), string);

        // subprocess module
        let subprocess = Value::Dict(BTreeMap::from([
            ("run".into(), Value::NativeFunction("subprocess_run".into())),
            ("call".into(), Value::NativeFunction("subprocess_call".into())),
            ("check_output".into(), Value::NativeFunction("subprocess_check_output".into())),
        ]));
        self.vars.insert("subprocess".into(), subprocess);

        // struct module (binary pack/unpack)
        let struct_mod = Value::Dict(BTreeMap::from([
            ("pack".into(), Value::NativeFunction("struct_pack".into())),
            ("unpack".into(), Value::NativeFunction("struct_unpack".into())),
            ("calcsize".into(), Value::NativeFunction("struct_calcsize".into())),
        ]));
        self.vars.insert("struct".into(), struct_mod);

        // hashlib module (alias to the crypto hashes)
        let hashlib = Value::Dict(BTreeMap::from([
            ("sha256".into(), Value::NativeFunction("crypto_sha256".into())),
            ("sha1".into(), Value::NativeFunction("crypto_sha1".into())),
            ("md5".into(), Value::NativeFunction("crypto_md5".into())),
            ("sha512".into(), Value::NativeFunction("crypto_sha512".into())),
            ("sha224".into(), Value::NativeFunction("crypto_sha224".into())),
            ("sha384".into(), Value::NativeFunction("crypto_sha384".into())),
            ("sha3_256".into(), Value::NativeFunction("crypto_sha3_256".into())),
            ("sha3_512".into(), Value::NativeFunction("crypto_sha3_512".into())),
            ("blake2b".into(), Value::NativeFunction("crypto_blake2b".into())),
            ("blake2s".into(), Value::NativeFunction("crypto_blake2s".into())),
            ("pbkdf2_hmac".into(), Value::NativeFunction("crypto_pbkdf2".into())),
            ("create".into(), Value::NativeFunction("hashlib_new".into())),
            ("algorithms_available".into(), Value::List(vec![
                Value::String("md5".into()),
                Value::String("sha1".into()),
                Value::String("sha224".into()),
                Value::String("sha256".into()),
                Value::String("sha384".into()),
                Value::String("sha512".into()),
                Value::String("sha3_256".into()),
                Value::String("sha3_512".into()),
                Value::String("blake2b".into()),
                Value::String("blake2s".into()),
            ])),
        ]));
        self.vars.insert("hashlib".into(), hashlib);

        // shutil module (file utilities)
        let shutil = Value::Dict(BTreeMap::from([
            ("copy".into(), Value::NativeFunction("shutil_copy".into())),
            ("copy2".into(), Value::NativeFunction("shutil_copy2".into())),
            ("move".into(), Value::NativeFunction("shutil_move".into())),
            ("rmtree".into(), Value::NativeFunction("shutil_rmtree".into())),
            ("copytree".into(), Value::NativeFunction("shutil_copytree".into())),
            ("which".into(), Value::NativeFunction("shutil_which".into())),
            ("disk_usage".into(), Value::NativeFunction("shutil_disk_usage".into())),
        ]));
        self.vars.insert("shutil".into(), shutil);

        // pathlib module
        let pathlib = Value::Dict(BTreeMap::from([
            ("join".into(), Value::NativeFunction("pathlib_join".into())),
            ("name".into(), Value::NativeFunction("pathlib_name".into())),
            ("parent".into(), Value::NativeFunction("pathlib_parent".into())),
            ("stem".into(), Value::NativeFunction("pathlib_stem".into())),
            ("suffix".into(), Value::NativeFunction("pathlib_suffix".into())),
            ("suffixes".into(), Value::NativeFunction("pathlib_suffixes".into())),
            ("is_absolute".into(), Value::NativeFunction("pathlib_is_absolute".into())),
            ("resolve".into(), Value::NativeFunction("pathlib_resolve".into())),
            ("absolute".into(), Value::NativeFunction("pathlib_absolute".into())),
            ("exists".into(), Value::NativeFunction("pathlib_exists".into())),
            ("is_file".into(), Value::NativeFunction("pathlib_is_file".into())),
            ("is_dir".into(), Value::NativeFunction("pathlib_is_dir".into())),
            ("glob".into(), Value::NativeFunction("pathlib_glob".into())),
            ("touch".into(), Value::NativeFunction("pathlib_touch".into())),
            ("mkdir".into(), Value::NativeFunction("pathlib_mkdir".into())),
            ("rmdir".into(), Value::NativeFunction("pathlib_rmdir".into())),
            ("unlink".into(), Value::NativeFunction("pathlib_unlink".into())),
            ("rename".into(), Value::NativeFunction("pathlib_rename".into())),
            ("read_text".into(), Value::NativeFunction("pathlib_read_text".into())),
            ("write_text".into(), Value::NativeFunction("pathlib_write_text".into())),
        ]));
        self.vars.insert("pathlib".into(), pathlib);

        // glob module
        let glob = Value::Dict(BTreeMap::from([
            ("glob".into(), Value::NativeFunction("fs_glob".into())),
        ]));
        self.vars.insert("glob".into(), glob);

        // urllib module
        let urllib = Value::Dict(BTreeMap::from([
            ("urlopen".into(), Value::NativeFunction("urllib_urlopen".into())),
            ("quote".into(), Value::NativeFunction("urllib_quote".into())),
            ("unquote".into(), Value::NativeFunction("urllib_unquote".into())),
            ("urlencode".into(), Value::NativeFunction("urllib_urlencode".into())),
            ("parse".into(), Value::NativeFunction("urllib_parse".into())),
            ("parse_qs".into(), Value::NativeFunction("urllib_parse_qs".into())),
        ]));
        self.vars.insert("urllib".into(), urllib);

        // collections module
        let collections = Value::Dict(BTreeMap::from([
            ("Counter".into(), Value::NativeFunction("collections_counter".into())),
            ("chain".into(), Value::NativeFunction("collections_chain".into())),
            ("flatten".into(), Value::NativeFunction("collections_flatten".into())),
        ]));
        self.vars.insert("collections".into(), collections);

        // itertools module
        let itertools = Value::Dict(BTreeMap::from([
            ("enumerate".into(), Value::NativeFunction("itertools_enumerate".into())),
            ("zip".into(), Value::NativeFunction("itertools_zip".into())),
            ("chain".into(), Value::NativeFunction("itertools_chain".into())),
            ("repeat".into(), Value::NativeFunction("itertools_repeat".into())),
            ("product".into(), Value::NativeFunction("itertools_product".into())),
            ("permutations".into(), Value::NativeFunction("itertools_permutations".into())),
            ("combinations".into(), Value::NativeFunction("itertools_combinations".into())),
            ("accumulate".into(), Value::NativeFunction("itertools_accumulate".into())),
            ("take".into(), Value::NativeFunction("itertools_take".into())),
            ("drop".into(), Value::NativeFunction("itertools_drop".into())),
            ("range".into(), Value::NativeFunction("itertools_range".into())),
        ]));
        self.vars.insert("itertools".into(), itertools);

        // tempfile module
        let tempfile = Value::Dict(BTreeMap::from([
            ("dir".into(), Value::NativeFunction("tempfile_dir".into())),
            ("mkdtemp".into(), Value::NativeFunction("tempfile_mkdtemp".into())),
            ("mkstemp".into(), Value::NativeFunction("tempfile_mkstemp".into())),
        ]));
        self.vars.insert("tempfile".into(), tempfile);

        // binascii module
        let binascii = Value::Dict(BTreeMap::from([
            ("hexlify".into(), Value::NativeFunction("binascii_hexlify".into())),
            ("unhexlify".into(), Value::NativeFunction("binascii_unhexlify".into())),
            ("a2b_base64".into(), Value::NativeFunction("binascii_a2b_base64".into())),
            ("b2a_base64".into(), Value::NativeFunction("binascii_b2a_base64".into())),
        ]));
        self.vars.insert("binascii".into(), binascii);

        // Register all core native functions eagerly
        const NATIVES: [&str; 359] = [
            "math_sin",
            "math_cos",
            "socket_open",
            "socket_send",
            "socket_recv",
            "time_now",
            "time_unix",
            "time_utc",
            "time_date",
            "time_format",
            "time_parse",
            "time_sleep",
            "time_wait",
            "time_year",
            "time_month",
            "time_day",
            "time_hour",
            "time_minute",
            "time_second",
            "time_weekday",
            "time_from_unix",
            "time_add_days",
            "cli_args",
            "http_get",
            "http_post",
            "http_put",
            "http_del",
            "http_head",
            "http_patch",
            "__http_response_json",
            "__http_response_text",
            "sha256_hex",
            "fs_read",
            "fs_write",
            "fs_exists",
            "fs_list_dir",
            "fs_append",
            "fs_mkdir",
            "fs_remove",
            "fs_read_binary",
            "fs_write_binary",
            "fs_is_file",
            "fs_is_dir",
            "fs_size",
            "fs_mtime",
            "fs_rmdir",
            "fs_rmtree",
            "fs_copy",
            "fs_move",
            "fs_glob",
            "fs_join",
            "fs_basename",
            "fs_dirname",
            "fs_cd",
            "json_load",
            "json_save",
            "json_encode",
            "json_decode",
            "regex_match",
            "regex_search",
            "regex_find",
            "regex_split",
            "regex_replace",
            "random_random",
            "random_randint",
            "random_randrange",
            "random_choice",
            "random_choices",
            "random_sample",
            "random_shuffle",
            "random_uniform",
            "random_hex",
            "random_seed",
            "math_sqrt",
            "math_abs",
            "math_floor",
            "math_ceil",
            "math_round",
            "math_pow",
            "math_min",
            "math_max",
            "math_trunc",
            "math_exp",
            "math_log",
            "math_log2",
            "math_log10",
            "math_tan",
            "math_asin",
            "math_acos",
            "math_atan",
            "math_atan2",
            "math_degrees",
            "math_radians",
            "math_hypot",
            "math_isnan",
            "math_isfinite",
            "math_isinf",
            "math_copysign",
            "math_gcd",
            "math_lcm",
            "math_factorial",
            "math_comb",
            "math_perm",
            "math_remainder",
            "math_fsum",
            "math_prod",
            "math_modf",
            "math_frexp",
            "math_ldexp",
            "b64_encode",
            "b64_decode",
            "b64_url_encode",
            "b64_url_decode",
            "b32_encode",
            "b32_decode",
            "os_getenv",
            "os_setenv",
            "os_unsetenv",
            "os_home",
            "os_cwd",
            "os_platform",
            "os_hostname",
            "os_pid",
            "os_cpu_count",
            "os_system",
            "crypto_sha256",
            "crypto_sha1",
            "crypto_md5",
            "crypto_sha512",
            "crypto_sha224",
            "crypto_sha384",
            "crypto_sha3_256",
            "crypto_sha3_512",
            "crypto_blake2b",
            "crypto_blake2s",
            "crypto_hmac_sha256",
            "crypto_hmac_sha1",
            "crypto_hmac_md5",
            "crypto_random_bytes",
            "crypto_random_hex",
            "crypto_pbkdf2",
            "crypto_aes_encrypt",
            "crypto_aes_decrypt",
            "fernet_generate_key",
            "fernet_encrypt",
            "fernet_decrypt",
            "uuid_uuid4",
            "uuid_uuid1",
            "uuid_uuid3",
            "uuid_uuid5",
            "color_style_bold",
            "color_style_dim",
            "color_style_italic",
            "color_style_underline",
            "color_style_blink",
            "color_style_reverse",
            "color_style_hidden",
            "color_style_strike",
            "color_rgb",
            "color_bg_rgb",
            "color_hex",
            "color_strip",
            "color_fg_black",
            "color_fg_red",
            "color_fg_green",
            "color_fg_yellow",
            "color_fg_blue",
            "color_fg_magenta",
            "color_fg_cyan",
            "color_fg_white",
            "color_bg_black",
            "color_bg_red",
            "color_bg_green",
            "color_bg_yellow",
            "color_bg_blue",
            "color_bg_magenta",
            "color_bg_cyan",
            "color_bg_white",
            "color_fg_bright_black",
            "color_fg_bright_red",
            "color_fg_bright_green",
            "color_fg_bright_yellow",
            "color_fg_bright_blue",
            "color_fg_bright_magenta",
            "color_fg_bright_cyan",
            "color_fg_bright_white",
            "csv_read",
            "csv_write",
            "csv_parse",
            "csv_encode",
            "decimal_decimal",
            "decimal_getcontext",
            "decimal_setcontext",
            "decimal_localcontext",
            "threading_start",
            "statistics_sum",
            "statistics_mean",
            "statistics_median",
            "statistics_mode",
            "statistics_stdev",
            "statistics_variance",
            "browser_launch",
            "browser_connect",
            "browser_navigate",
            "browser_evaluate",
            "browser_capture_screenshot",
            "browser_get_html",
            "browser_get_title",
            "browser_get_url",
            "browser_get_text",
            "browser_click",
            "browser_fill",
            "browser_query",
            "browser_wait_for",
            "browser_close",
            "socket_close",
            "ftp_connect",
            "ftp_login",
            "ftp_pwd",
            "ftp_list",
            "ftp_nlist",
            "ftp_cwd",
            "ftp_retr",
            "ftp_stor",
            "ftp_dele",
            "ftp_mkdir",
            "ftp_rmdir",
            "ftp_rename",
            "ftp_quit",
            "smtp_connect",
            "smtp_login",
            "smtp_sendmail",
            "smtp_quit",
            "smtp_message",
            "pop3_connect",
            "pop3_stat",
            "pop3_list",
            "pop3_retr",
            "pop3_dele",
            "pop3_quit",
            "imap_connect",
            "imap_select",
            "imap_search",
            "imap_fetch",
            "imap_list",
            "imap_logout",
            "telnet_connect",
            "telnet_write",
            "telnet_read",
            "telnet_read_until",
            "telnet_close",
            "dns_resolve",
            "dns_query",
            "ssh_run",
            "ssh_upload",
            "ssh_download",
            "ssh_available",
            "scapy_checksum",
            "scapy_ip",
            "scapy_tcp",
            "scapy_udp",
            "scapy_icmp",
            "scapy_raw",
            "scapy_build",
            "scapy_parse",
            "scapy_send",
            "scapy_sniff",
            "scapy_ip_to_int",
            "scapy_int_to_ip",
            "str_upper",
            "str_lower",
            "str_title",
            "str_capitalize",
            "str_swapcase",
            "str_strip",
            "str_lstrip",
            "str_rstrip",
            "str_split",
            "str_splitlines",
            "str_join",
            "str_replace",
            "str_count",
            "str_find",
            "str_rfind",
            "str_startswith",
            "str_endswith",
            "str_contains",
            "str_ljust",
            "str_rjust",
            "str_center",
            "str_zfill",
            "str_repeat",
            "str_isdigit",
            "str_isalpha",
            "str_isalnum",
            "str_isspace",
            "str_islower",
            "str_isupper",
            "subprocess_run",
            "subprocess_call",
            "subprocess_check_output",
            "struct_pack",
            "struct_unpack",
            "struct_calcsize",
            "hashlib_new",
            "shutil_copy",
            "shutil_copy2",
            "shutil_move",
            "shutil_rmtree",
            "shutil_copytree",
            "shutil_which",
            "shutil_disk_usage",
            "pathlib_join",
            "pathlib_name",
            "pathlib_parent",
            "pathlib_stem",
            "pathlib_suffix",
            "pathlib_suffixes",
            "pathlib_is_absolute",
            "pathlib_resolve",
            "pathlib_absolute",
            "pathlib_exists",
            "pathlib_is_file",
            "pathlib_is_dir",
            "pathlib_glob",
            "pathlib_touch",
            "pathlib_mkdir",
            "pathlib_rmdir",
            "pathlib_unlink",
            "pathlib_rename",
            "pathlib_read_text",
            "pathlib_write_text",
            "urllib_urlopen",
            "urllib_quote",
            "urllib_unquote",
            "urllib_urlencode",
            "urllib_parse",
            "urllib_parse_qs",
            "collections_counter",
            "collections_chain",
            "collections_flatten",
            "itertools_enumerate",
            "itertools_zip",
            "itertools_chain",
            "itertools_repeat",
            "itertools_product",
            "itertools_permutations",
            "itertools_combinations",
            "itertools_accumulate",
            "itertools_take",
            "itertools_drop",
            "itertools_range",
            "tempfile_dir",
            "tempfile_mkdtemp",
            "tempfile_mkstemp",
            "binascii_hexlify",
            "binascii_unhexlify",
            "binascii_a2b_base64",
            "binascii_b2a_base64",
        ];
        for name in NATIVES {
            self.native_functions.insert(name.to_string(), native_for(name));
        }
        // Lock all builtin names so scripts cannot shadow the standard library.
        let builtin_names: Vec<String> = self
            .vars
            .keys()
            .cloned()
            .chain(self.native_functions.keys().cloned())
            .collect();
        for name in builtin_names {
            self.locked.insert(name);
        }
    }
    fn eval(&mut self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::Named(_, _) => Err("named arguments are only allowed inside calls".into()),
            Expr::Spread(_) => Err("spread is only allowed inside list/dict literals".into()),
            Expr::Value(v) => Ok(v.clone()),
            Expr::Var(n) => self
                .vars
                .get(n)
                .cloned()
                .or_else(|| {
                    if self.functions.contains_key(n) {
                        return Some(Value::Function(n.clone()));
                    }
                    self.imported_modules.get(n).cloned().map(|vars| {
                        if let Some(Value::Dict(module_dict)) = vars.get(n) {
                            Value::Dict(module_dict.clone())
                        } else {
                            Value::Dict(vars.into_iter().collect())
                        }
                    })
                })
                .ok_or_else(|| format!("undefined variable: {n}")),
            Expr::List(items) => {
                let mut list = Vec::new();
                for x in items {
                    match x {
                        Expr::Spread(inner) => match self.eval(inner)? {
                            Value::List(items) => list.extend(items),
                            Value::Dict(map) => list.extend(map.into_values()),
                            other => return Err(format!("cannot spread {other} into a list")),
                        },
                        other => list.push(self.eval(other)?),
                    }
                }
                Ok(Value::List(list))
            }
            Expr::Dict(entries) => {
                let mut dict = BTreeMap::new();
                for entry in entries {
                    match entry {
                        DictEntry::Pair(key, expr) => {
                            dict.insert(key.clone(), self.eval(expr)?);
                        }
                        DictEntry::Spread(expr) => match self.eval(expr)? {
                            Value::Dict(map) => {
                                for (k, v) in map {
                                    dict.insert(k, v);
                                }
                            }
                            other => return Err(format!("cannot spread {other} into a dict")),
                        },
                    }
                }
                Ok(Value::Dict(dict))
            }
            Expr::Range(start, end, exclusive) => {
                let (Value::Number(start), Value::Number(end)) =
                    (self.eval(start)?, self.eval(end)?)
                else {
                    return Err("range bounds must be numbers".into());
                };
                if start.fract() != 0.0 || end.fract() != 0.0 {
                    return Err("range bounds must be integers".into());
                }
                let step = if start <= end { 1 } else { -1 };
                let mut values = Vec::new();
                let mut value = start as i64;
                let stop = end as i64;
                while if *exclusive {
                    (step > 0 && value < stop) || (step < 0 && value > stop)
                } else {
                    (step > 0 && value <= stop) || (step < 0 && value >= stop)
                } {
                    values.push(Value::Number(value as f64));
                    value += step;
                }
                Ok(Value::List(values))
            }
            Expr::Index(object, index) => {
                let object = self.eval(object)?;
                let index = self.eval(index)?;
                match (object, index) {
                    (Value::List(values), Value::Number(index)) if index.fract() == 0.0 => {
                        let index = if index < 0.0 {
                            values.len() as i64 + index as i64
                        } else {
                            index as i64
                        };
                        values
                            .get(index as usize)
                            .cloned()
                            .ok_or_else(|| "list index out of bounds".into())
                    }
                    (Value::Dict(values), Value::String(key)) => values
                        .get(&key)
                        .cloned()
                        .ok_or_else(|| format!("dictionary has no key: {key}")),
                    (Value::String(value), Value::Number(index)) if index.fract() == 0.0 => value
                        .chars()
                        .nth(index as usize)
                        .map(|c| Value::String(c.to_string()))
                        .ok_or_else(|| "string index out of bounds".into()),
                    _ => Err("invalid index operation".into()),
                }
            }
            Expr::Member(object, name) => {
                let obj = self.eval(object)?;
                self.member(obj, name)
            }
            Expr::SafeMember(object, name) => {
                let object = self.eval(object)?;
                if matches!(object, Value::Null) {
                    Ok(Value::Null)
                } else {
                    self.member(object, name)
                }
            }
            Expr::Ternary(condition, yes, no) => {
                if self.eval(condition)?.truthy() {
                    self.eval(yes)
                } else {
                    self.eval(no)
                }
            }
            Expr::Increment(target, amount) => {
                let Expr::Var(name) = target.as_ref() else {
                    return Err("increment/decrement requires a variable".into());
                };
                let Value::Number(value) = self
                    .vars
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("undefined variable: {name}"))?
                else {
                    return Err("increment/decrement requires a number".into());
                };
                let result = Value::Number(value + *amount as f64);
                self.vars.insert(name.clone(), result.clone());
                Ok(result)
            }
            Expr::Lambda(params, body) => {
                let fname = format!("__lambda_{}", self.lambda_counter);
                self.lambda_counter += 1;
                let function = Function {
                    params: params.clone(),
                    body: body.clone(),
                };
                self.functions.insert(fname.clone(), function);
                Ok(Value::Function(fname))
            }
            Expr::Call(callee, arguments) => {
                let mut values = Vec::new();
                let mut named = BTreeMap::new();
                for argument in arguments {
                    match argument {
                        Expr::Named(name, value) => {
                            named.insert(name.clone(), self.eval(value)?);
                        }
                        other => values.push(self.eval(other)?),
                    }
                }
                if !named.is_empty() {
                    values.push(Value::Dict(named));
                }
                match callee.as_ref() {
                    Expr::Var(name) => match self.call(name, values)? {
                        Flow::Return(v) => Ok(v),
                        Flow::Throw(v) => Err(format!("unhandled exception: {v}")),
                        _ => unreachable!(),
                    },
                    Expr::Member(object, method) => {
                        // Mutating list methods on a bare variable (push/pop) update the var in place
                        if let Expr::Var(name) = &**object {
                            if matches!(method.as_str(), "push" | "pop") {
                                if let Some(Value::List(current)) = self.vars.get(name).cloned() {
                                    let mut list = current;
                                    let result = match method.as_str() {
                                        "push" => {
                                            if let Some(item) = values.first() {
                                                list.push(item.clone());
                                            }
                                            Value::Null
                                        }
                                        _ => list.pop().unwrap_or(Value::Null),
                                    };
                                    self.vars.insert(name.clone(), Value::List(list));
                                    return Ok(result);
                                }
                            }
                        }
                        let obj = self.eval(object)?;
                        match obj {
                            Value::Instance(instance) => match self.call_method(instance, method, values)? {
                                Flow::Return(v) => Ok(v),
                                Flow::Throw(v) => Err(format!("unhandled exception: {v}")),
                                _ => unreachable!(),
                            },
                            Value::Dict(dict) => {
                                if let Some(Value::NativeFunction(native_name)) = dict.get(method) {
                                    if let Some(native_fn) = self.native_functions.get(native_name).cloned() {
                                        let mut call_args = values;
                                        // Native methods prefixed with __ receive the dict as `self`.
                                        if native_name.starts_with("__") {
                                            call_args.insert(0, Value::Dict(dict.clone()));
                                        }
                                        return Ok(native_fn(call_args)?);
                                    }
                                }
                                if let Some(Value::Function(fname)) = dict.get(method) {
                                    match self.call(fname, values)? {
                                        Flow::Return(v) => return Ok(v),
                                        Flow::Throw(v) => return Err(format!("unhandled exception: {v}")),
                                        _ => unreachable!(),
                                    }
                                }
                                self.dict_method(dict, method, values)
                            }
                            Value::String(value) => self.string_method(value, method, values),
                            Value::List(list) => self.list_method(list, method, values),
                            _ => Err("only instance methods and dicts are supported currently".into()),
                        }
                    }
                    _ => {
                        return Err(
                            "only named and instance method calls are supported currently".into(),
                        )
                    }
                }
            }
            Expr::New(class_name, args) => {
                if !self.classes.contains_key(class_name) {
                    return Err(format!("undefined class: {class_name}"));
                }
                let values = args
                    .iter()
                    .map(|argument| self.eval(argument))
                    .collect::<Result<Vec<_>, _>>()?;
                let instance = Arc::new(Mutex::new(Instance {
                    class_name: class_name.clone(),
                    fields: BTreeMap::new(),
                }));
                if self.find_method(class_name, "init").is_some() {
                    let init_values = if values.is_empty() && self.is_error_class(class_name) {
                        vec![Value::String(String::new())]
                    } else {
                        values
                    };
                    match self.call_method(instance.clone(), "init", init_values)? {
                        Flow::Return(_) => {}
                        Flow::Throw(v) => return Err(format!("unhandled exception in init: {v}")),
                        _ => unreachable!(),
                    }
                } else if !values.is_empty() {
                    return Err(format!("{class_name} has no constructor"));
                }
                Ok(Value::Instance(instance))
            }
            Expr::Unary(op, x) => {
                let v = self.eval(x)?;
                match op {
                    Kind::Minus => match v {
                        Value::Number(n) => Ok(Value::Number(-n)),
                        _ => Err("cannot negate non-number".into()),
                    },
                    Kind::Bang | Kind::Not => Ok(Value::Bool(!v.truthy())),
                    Kind::Typeof => Ok(Value::String(
                        match v {
                            Value::Null => "null",
                            Value::Bool(_) => "bool",
                            Value::Number(_) => "number",
                            Value::String(_) => "string",
                            Value::List(_) => "list",
                            Value::Dict(_) => "dict",
                            Value::Instance(_) => "object",
                            Value::Socket(_) => "socket",
                        Value::NativeFunction(_) | Value::Function(_) => "function",
                    }
                    .into(),
                    )),
                    Kind::Tilde => match v {
                        Value::Number(value) if value.fract() == 0.0 => {
                            Ok(Value::Number((!(value as i64)) as f64))
                        }
                        _ => Err("bitwise not requires an integer".into()),
                    },
                    _ => unreachable!(),
                }
            }
            Expr::Binary(left, op, right) => {
                let a = self.eval(left)?;
                if matches!(op, Kind::And) {
                    return Ok(if a.truthy() { self.eval(right)? } else { a });
                }
                if matches!(op, Kind::Or) {
                    return Ok(if a.truthy() { a } else { self.eval(right)? });
                }
                if matches!(op, Kind::Nullish) {
                    return Ok(if matches!(a, Value::Null) {
                        self.eval(right)?
                    } else {
                        a
                    });
                }
                let b = self.eval(right)?;
                self.binary(a, op, b)
            }
        }
    }
    fn member(&self, object: Value, name: &str) -> Result<Value, String> {
        match object {
            Value::Dict(values) => values
                .get(name)
                .cloned()
                .or_else(|| {
                    if name == "len" {
                        Some(Value::Number(values.len() as f64))
                    } else {
                        None
                    }
                })
                .ok_or_else(|| format!("dictionary has no member: {name}")),
            Value::List(values) if name == "len" || name == "count" => {
                Ok(Value::Number(values.len() as f64))
            }
            Value::String(value) if name == "len" || name == "count" => {
                Ok(Value::Number(value.chars().count() as f64))
            }
            Value::Instance(instance) => instance
                .lock()
                .unwrap()
                .fields
                .get(name)
                .cloned()
                .ok_or_else(|| format!("object has no field: {name}")),
            value => Err(format!("{} has no member: {name}", value)),
        }
    }
    fn string_method(&mut self, value: String, method: &str, args: Vec<Value>) -> Result<Value, String> {
        let one = || -> Result<String, String> {
            match args.first() {
                Some(Value::String(s)) => Ok(s.clone()),
                _ => Err(format!("string method '{method}' expects a string argument")),
            }
        };
        match method {
            "split" => {
                let sep = one()?;
                Ok(Value::List(
                    value.split(&sep).map(|s| Value::String(s.into())).collect(),
                ))
            }
            "contains" => {
                let needle = one()?;
                Ok(Value::Bool(value.contains(&needle)))
            }
            "startsWith" | "startswith" => {
                let prefix = one()?;
                Ok(Value::Bool(value.starts_with(&prefix)))
            }
            "endsWith" | "endswith" => {
                let suffix = one()?;
                Ok(Value::Bool(value.ends_with(&suffix)))
            }
            "trim" => Ok(Value::String(value.trim().into())),
            "trimEnd" | "trimRight" => Ok(Value::String(value.trim_end().into())),
            "trimStart" | "trimLeft" => Ok(Value::String(value.trim_start().into())),
            "lower" | "toLower" | "toLowerCase" => Ok(Value::String(value.to_lowercase())),
            "upper" | "toUpper" | "toUpperCase" => Ok(Value::String(value.to_uppercase())),
            "toList" => Ok(Value::List(
                value.chars().map(|c| Value::String(c.to_string())).collect(),
            )),
            "replace" => {
                let (from, to) = match args.as_slice() {
                    [Value::String(f), Value::String(t)] => (f.clone(), t.clone()),
                    _ => return Err("replace expects (from, to)".into()),
                };
                Ok(Value::String(value.replace(&from, &to)))
            }
            "indexOf" => {
                let needle = one()?;
                match value.find(&needle) {
                    Some(i) => Ok(Value::Number(i as f64)),
                    None => Ok(Value::Number(-1.0)),
                }
            }
            "length" => Ok(Value::Number(value.chars().count() as f64)),
            "repeat" => {
                let n = match args.first() {
                    Some(Value::Number(n)) => *n as usize,
                    _ => return Err("repeat expects a number".into()),
                };
                Ok(Value::String(value.repeat(n)))
            }
            "substring" | "substr" | "slice" => {
                let (start, end) = match args.as_slice() {
                    [Value::Number(s), Value::Number(e)] => (*s as usize, Some(*e as usize)),
                    [Value::Number(s)] => (*s as usize, None),
                    _ => return Err(format!("{method} expects (start[, end])")),
                };
                let chars: Vec<char> = value.chars().collect();
                let start = start.min(chars.len());
                let end = end.unwrap_or(chars.len()).min(chars.len());
                Ok(Value::String(chars[start..end].iter().collect()))
            }
            _ => Err(format!("string has no method: {method}")),
        }
    }
    fn list_method(&mut self, list: Vec<Value>, method: &str, args: Vec<Value>) -> Result<Value, String> {
        match method {
            "push" => {
                let item = match args.first() {
                    Some(v) => v.clone(),
                    None => return Err("push expects an argument".into()),
                };
                let mut list = list;
                list.push(item);
                Ok(Value::List(list))
            }
            "pop" => {
                let mut list = list;
                let value = list.pop();
                match value {
                    Some(v) => Ok(v),
                    None => Ok(Value::Null),
                }
            }
            "join" => {
                let sep = match args.first() {
                    Some(Value::String(s)) => s.clone(),
                    _ => String::new(),
                };
                Ok(Value::String(
                    list.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(&sep),
                ))
            }
            "contains" => {
                let item = match args.first() {
                    Some(v) => v.clone(),
                    None => return Err("contains expects an argument".into()),
                };
                Ok(Value::Bool(list.contains(&item)))
            }
            "first" => match list.first() {
                Some(v) => Ok(v.clone()),
                None => Ok(Value::Null),
            },
            "last" => match list.last() {
                Some(v) => Ok(v.clone()),
                None => Ok(Value::Null),
            },
            "reverse" => {
                let mut list = list;
                list.reverse();
                Ok(Value::List(list))
            }
            "sort" => {
                let mut list = list;
                list.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
                Ok(Value::List(list))
            }
            "skip" => {
                let n = match args.first() {
                    Some(Value::Number(n)) => *n as usize,
                    _ => return Err("skip expects a number".into()),
                };
                Ok(Value::List(list.into_iter().skip(n).collect()))
            }
            "concat" => {
                let extra: Vec<Value> = args
                    .iter()
                    .flat_map(|a| match a {
                        Value::List(items) => items.clone(),
                        other => vec![other.clone()],
                    })
                    .collect();
                let mut list = list;
                list.extend(extra);
                Ok(Value::List(list))
            }
            "sum" => {
                let mut total = 0.0;
                for item in &list {
                    match item {
                        Value::Number(n) => total += n,
                        _ => return Err("sum expects a list of numbers".into()),
                    }
                }
                Ok(Value::Number(total))
            }
            "unique" => {
                let mut seen = Vec::new();
                let mut out = Vec::new();
                for item in list {
                    if !seen.contains(&item) {
                        seen.push(item.clone());
                        out.push(item);
                    }
                }
                Ok(Value::List(out))
            }
            "shift" => {
                let mut list = list;
                let value = if list.is_empty() {
                    Value::Null
                } else {
                    list.remove(0)
                };
                Ok(value)
            }
            "unshift" => {
                let item = match args.first() {
                    Some(v) => v.clone(),
                    None => return Err("unshift expects an argument".into()),
                };
                let mut list = list;
                list.insert(0, item);
                Ok(Value::List(list))
            }
            "map" => {
                let f = match args.first().cloned() {
                    Some(v) => v,
                    None => return Err("map expects a function".into()),
                };
                let mut out = Vec::new();
                for item in list {
                    out.push(self.apply_func(&f, vec![item])?);
                }
                Ok(Value::List(out))
            }
            "filter" => {
                let f = match args.first().cloned() {
                    Some(v) => v,
                    None => return Err("filter expects a function".into()),
                };
                let mut out = Vec::new();
                for item in list {
                    if self.apply_func(&f, vec![item.clone()])?.truthy() {
                        out.push(item);
                    }
                }
                Ok(Value::List(out))
            }
            "each" => {
                let f = match args.first().cloned() {
                    Some(v) => v,
                    None => return Err("each expects a function".into()),
                };
                for item in list {
                    self.apply_func(&f, vec![item])?;
                }
                Ok(Value::Null)
            }
            "length" => Ok(Value::Number(list.len() as f64)),
            _ => Err(format!("list has no method: {method}")),
        }
    }
    fn dict_method(&mut self, dict: BTreeMap<String, Value>, method: &str, args: Vec<Value>) -> Result<Value, String> {
        match method {
            "has" | "containsKey" => {
                let key = match args.first() {
                    Some(Value::String(k)) => k.clone(),
                    _ => return Err(format!("{method} expects a string key")),
                };
                Ok(Value::Bool(dict.contains_key(&key)))
            }
            "get" => {
                let key = match args.first() {
                    Some(Value::String(k)) => k.clone(),
                    _ => return Err("get expects a string key".into()),
                };
                match dict.get(&key).cloned() {
                    Some(v) => Ok(v),
                    None => match args.get(1) {
                        Some(d) => Ok(d.clone()),
                        None => Ok(Value::Null),
                    },
                }
            }
            "keys" => Ok(Value::List(
                dict.keys().map(|k| Value::String(k.clone())).collect(),
            )),
            "values" => Ok(Value::List(dict.into_values().collect())),
            "set" => {
                let (key, value) = match args.as_slice() {
                    [Value::String(k), v] => (k.clone(), v.clone()),
                    _ => return Err("set expects (key, value)".into()),
                };
                let mut dict = dict;
                dict.insert(key, value);
                Ok(Value::Dict(dict))
            }
            "length" => Ok(Value::Number(dict.len() as f64)),
            _ => Err(format!("dictionary has no method: {method}")),
        }
    }
    fn binary(&self, a: Value, op: &Kind, b: Value) -> Result<Value, String> {
        match op {
            Kind::Eq => Ok(Value::Bool(a == b)),
            Kind::Ne => Ok(Value::Bool(a != b)),
            Kind::StrictEq => Ok(Value::Bool(
                std::mem::discriminant(&a) == std::mem::discriminant(&b) && a == b,
            )),
            Kind::StrictNe => Ok(Value::Bool(
                std::mem::discriminant(&a) != std::mem::discriminant(&b) || a != b,
            )),
            Kind::Is => Ok(Value::Bool(
                std::mem::discriminant(&a) == std::mem::discriminant(&b) && a == b,
            )),
            Kind::In => match b {
                Value::List(values) => Ok(Value::Bool(values.contains(&a))),
                Value::Dict(values) => match a {
                    Value::String(key) => Ok(Value::Bool(values.contains_key(&key))),
                    _ => Err("dictionary membership requires a string key".into()),
                },
                Value::String(text) => match a {
                    Value::String(needle) => Ok(Value::Bool(text.contains(&needle))),
                    _ => Err("string membership requires a string value".into()),
                },
                _ => Err("right side of 'in' must be a list, dictionary, or string".into()),
            },
            Kind::Amp | Kind::Pipe | Kind::Caret | Kind::LShift | Kind::RShift => {
                let (Value::Number(left), Value::Number(right)) = (a, b) else {
                    return Err("bitwise operators require integers".into());
                };
                if left.fract() != 0.0 || right.fract() != 0.0 {
                    return Err("bitwise operators require integers".into());
                }
                let value = match op {
                    Kind::Amp => (left as i64) & (right as i64),
                    Kind::Pipe => (left as i64) | (right as i64),
                    Kind::Caret => (left as i64) ^ (right as i64),
                    Kind::LShift => (left as i64) << (right as u32),
                    Kind::RShift => (left as i64) >> (right as u32),
                    _ => unreachable!(),
                };
                Ok(Value::Number(value as f64))
            }
            Kind::Plus => match (a, b) {
                (Value::Number(x), Value::Number(y)) => Ok(Value::Number(x + y)),
                (Value::List(mut x), Value::List(y)) => {
                    x.extend(y);
                    Ok(Value::List(x))
                }
                (x, y) => Ok(Value::String(format!("{x}{y}"))),
            },
            Kind::Star => match (a, b) {
                (Value::String(s), Value::Number(n)) => {
                    let count = n.round() as i64;
                    if count < 0 {
                        return Err("string repetition requires a non-negative count".into());
                    }
                    if (n - n.round()).abs() > f64::EPSILON {
                        return Err("string repetition requires an integer count".into());
                    }
                    let mut out = String::new();
                    for _ in 0..count {
                        out.push_str(&s);
                    }
                    Ok(Value::String(out))
                }
                (Value::Number(n), Value::String(s)) => {
                    let count = n.round() as i64;
                    if count < 0 {
                        return Err("string repetition requires a non-negative count".into());
                    }
                    if (n - n.round()).abs() > f64::EPSILON {
                        return Err("string repetition requires an integer count".into());
                    }
                    let mut out = String::new();
                    for _ in 0..count {
                        out.push_str(&s);
                    }
                    Ok(Value::String(out))
                }
                (Value::Number(x), Value::Number(y)) => Ok(Value::Number(x * y)),
                _ => Err("string repetition requires a string and a number".into()),
            },
            Kind::Minus | Kind::Slash | Kind::Percent | Kind::Pow => {
                let (Value::Number(x), Value::Number(y)) = (a, b) else {
                    return Err("arithmetic requires numbers".into());
                };
                match op {
                    Kind::Minus => Ok(Value::Number(x - y)),
                    Kind::Slash => Ok(Value::Number(x / y)),
                    Kind::Percent => Ok(Value::Number(x % y)),
                    Kind::Pow => Ok(Value::Number(x.powf(y))),
                    _ => unreachable!(),
                }
            }
            Kind::Lt | Kind::Le | Kind::Gt | Kind::Ge => {
                let (Value::Number(x), Value::Number(y)) = (a, b) else {
                    return Err("comparison requires numbers".into());
                };
                Ok(Value::Bool(match op {
                    Kind::Lt => x < y,
                    Kind::Le => x <= y,
                    Kind::Gt => x > y,
                    Kind::Ge => x >= y,
                    _ => unreachable!(),
                }))
            }
            _ => Err("unsupported operator".into()),
        }
    }
    fn run_module(&mut self, path: &str, namespace: &str) -> Result<HashMap<String, Value>, String> {
        let stmts = parse_file(path)?;
        let mut module_vm = Vm::new();
        module_vm.functions = self.functions.clone();
        module_vm.classes = self.classes.clone();
        module_vm.file = path.into();
        if let Ok(source) = fs::read_to_string(path) {
            module_vm.lines = source.lines().map(|l| l.to_string()).collect();
        }
        module_vm.exec(&stmts)?;
        // Register the module's functions under a namespaced key in the caller so
        // `module.func(...)` calls resolve through self.functions.
        for (fname, function) in &module_vm.functions {
            let key = format!("{namespace}::{fname}");
            self.functions.insert(key, function.clone());
        }
        // Register the module's classes under a namespaced key so `new module.Class(...)` works.
        for (class, def) in &module_vm.classes {
            let key = format!("{namespace}.{class}");
            self.classes.insert(key, def.clone());
        }
        let mut exports = module_vm.vars;
        for (fname, _fn) in &module_vm.functions {
            if !exports.contains_key(fname) {
                exports.insert(
                    fname.clone(),
                    Value::Function(format!("{namespace}::{fname}")),
                );
            }
        }
        for (class, _def) in &module_vm.classes {
            if !exports.contains_key(class) {
                exports.insert(
                    class.clone(),
                    Value::Function(format!("{namespace}.{class}")),
                );
            }
        }
        Ok(exports)
    }

    fn resolve_module(&self, name: &str) -> Result<String, String> {
        let local = format!("{name}.z");
        if std::path::Path::new(&local).exists() {
            return Ok(local);
        }
        if let Some(path) = crate::pm::resolve_module_file(name) {
            return Ok(path);
        }
        if let Some(path) = find_std_file(&format!("{name}.z")) {
            return Ok(path);
        }
        Err(format!("module not found: {name}"))
    }

    fn call(&mut self, name: &str, values: Vec<Value>) -> Result<Flow, String> {
        if let Some(native_fn) = self.native_functions.get(name).cloned() {
            return Ok(Flow::Return(native_fn(values)?));
        }

        if let Some(Value::NativeFunction(native_name)) = self.vars.get(name).cloned() {
            if let Some(native_fn) = self.native_functions.get(&native_name).cloned() {
                return Ok(Flow::Return(native_fn(values)?));
            }
        }

        if let Some(Value::Function(fname)) = self.vars.get(name).cloned() {
            return self.call(&fname, values);
        }

        let function = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| format!("undefined function: {name}"))?;
        if values.len() != function.params.len() {
            return Err(format!(
                "{name} expects {} arguments, got {}",
                function.params.len(),
                values.len()
            ));
        }
        let caller_vars = self.vars.clone();
        for (parameter, value) in function.params.iter().zip(values) {
            self.vars.insert(parameter.clone(), value);
        }
        self.stack.push(name.to_string());
        let flow = self.exec(&function.body);
        self.stack.pop();
        let flow = flow?;
        self.vars = caller_vars;
        Ok(match flow {
            Flow::Return(value) => Flow::Return(value),
            Flow::Throw(value) => Flow::Throw(value),
            Flow::Normal => Flow::Return(Value::Null),
            Flow::Break | Flow::Continue => {
                return Err(format!("loop control escaped function: {name}"))
            }
        })
    }
    fn find_method(&self, class_name: &str, method: &str) -> Option<Function> {
        let mut current = Some(class_name.to_string());
        while let Some(name) = current {
            let class = self.classes.get(&name)?;
            if let Some(method) = class.methods.get(method) {
                return Some(method.clone());
            }
            current = class.parent.clone();
        }
        None
    }
    fn apply_func(&mut self, f: &Value, values: Vec<Value>) -> Result<Value, String> {
        match f {
            Value::NativeFunction(name) => match self.native_functions.get(name).cloned() {
                Some(native_fn) => native_fn(values),
                None => Err(format!("unknown native function: {name}")),
            },
            Value::Function(fname) => match self.call(fname, values)? {
                Flow::Return(v) => Ok(v),
                Flow::Throw(v) => Err(format!("unhandled exception: {v}")),
                _ => unreachable!(),
            },
            _ => Err("expected a function".into()),
        }
    }
    fn call_method(
        &mut self,
        instance: InstanceRef,
        method: &str,
        values: Vec<Value>,
    ) -> Result<Flow, String> {
        let class_name = instance.lock().unwrap().class_name.clone();
        let function = self
            .find_method(&class_name, method)
            .ok_or_else(|| format!("{class_name} has no method: {method}"))?;
        if values.len() != function.params.len() {
            return Err(format!(
                "{class_name}.{method} expects {} arguments, got {}",
                function.params.len(),
                values.len()
            ));
        }
        let caller_vars = self.vars.clone();
        self.vars.insert("self".into(), Value::Instance(instance));
        for (parameter, value) in function.params.iter().zip(values) {
            self.vars.insert(parameter.clone(), value);
        }
        self.stack.push(format!("{class_name}.{method}"));
        let flow = self.exec(&function.body);
        self.stack.pop();
        let flow = flow?;
        self.vars = caller_vars;
        Ok(match flow {
            Flow::Return(value) => Flow::Return(value),
            Flow::Throw(value) => Flow::Throw(value),
            Flow::Normal => Flow::Return(Value::Null),
            Flow::Break | Flow::Continue => {
                return Err(format!(
                    "loop control escaped method: {class_name}.{method}"
                ))
            }
        })
    }
    /// Parse a bare expression source string and evaluate it in this VM.
    fn eval_expr_source(&mut self, source: &str) -> Result<Value, String> {
        let tokens = lex(source)?;
        let expr = Parser::new(tokens).expr()?;
        self.eval(&expr)
    }
    fn exec(&mut self, stmts: &[Stmt]) -> Result<Flow, String> {
        for stmt in stmts {
            let result = self.exec_one(stmt);
            match result {
                Ok(Flow::Normal | Flow::Continue) => {}
                Ok(Flow::Break) => return Ok(Flow::Break),
                Ok(Flow::Return(value)) => return Ok(Flow::Return(value)),
                Ok(Flow::Throw(value)) => return Ok(Flow::Throw(value)),
                Err(e) => return Err(self.locate(stmt.line, stmt.col, e)),
            }
        }
        Ok(Flow::Normal)
    }
    fn exec_one(&mut self, stmt: &Stmt) -> Result<Flow, String> {
        match &stmt.kind {
                StmtKind::Let(target, e, is_const) => {
                    let v = self.eval(e)?;
                    let mut names: Vec<String> = Vec::new();
                    match target {
                        LetTarget::Var(name) => {
                            if *is_const && self.locked.contains(name) {
                                return Err(format!("cannot redefine constant: {name}"));
                            }
                            self.vars.insert(name.clone(), v);
                            names.push(name.clone());
                        }
                        LetTarget::List(patterns) => match v {
                            Value::List(items) => {
                                for (i, name) in patterns.iter().enumerate() {
                                    if *is_const && self.locked.contains(name) {
                                        return Err(format!("cannot redefine constant: {name}"));
                                    }
                                    let item = items.get(i).cloned().unwrap_or(Value::Null);
                                    self.vars.insert(name.clone(), item);
                                    names.push(name.clone());
                                }
                            }
                            other => return Err(format!("cannot destructure {other} as a list")),
                        },
                        LetTarget::Dict(patterns) => match v {
                            Value::Dict(map) => {
                                for name in patterns {
                                    if *is_const && self.locked.contains(name) {
                                        return Err(format!("cannot redefine constant: {name}"));
                                    }
                                    let item = map.get(name).cloned().unwrap_or(Value::Null);
                                    self.vars.insert(name.clone(), item);
                                    names.push(name.clone());
                                }
                            }
                            other => return Err(format!("cannot destructure {other} as a dict")),
                        },
                    }
                    if *is_const {
                        for name in names {
                            self.locked.insert(name);
                        }
                    } else {
                        for name in names {
                            self.locked.remove(&name);
                        }
                    }
                    Ok(Flow::Normal)
                }
                StmtKind::Assign(n, op, e) => {
                    if self.locked.contains(n) {
                        return Err(format!("cannot assign to constant: {n}"));
                    }
                    let rhs = self.eval(e)?;
                    let v = if matches!(op, Kind::Assign) {
                        rhs
                    } else if matches!(op, Kind::NullishAssign) {
                        let current = self
                            .vars
                            .get(n)
                            .cloned()
                            .ok_or_else(|| format!("undefined variable: {n}"))?;
                        if matches!(current, Value::Null) {
                            rhs
                        } else {
                            current
                        }
                    } else {
                        let binary_op = match op {
                            Kind::PlusAssign => Kind::Plus,
                            Kind::MinusAssign => Kind::Minus,
                            Kind::StarAssign => Kind::Star,
                            Kind::SlashAssign => Kind::Slash,
                            Kind::PercentAssign => Kind::Percent,
                            Kind::AmpAssign => Kind::Amp,
                            Kind::PipeAssign => Kind::Pipe,
                            Kind::CaretAssign => Kind::Caret,
                            Kind::LShiftAssign => Kind::LShift,
                            Kind::RShiftAssign => Kind::RShift,
                            _ => return Err("unsupported assignment operator".into()),
                        };
                        self.binary(
                            self.vars
                                .get(n)
                                .cloned()
                                .ok_or_else(|| format!("undefined variable: {n}"))?,
                            &binary_op,
                            rhs,
                        )?
                    };
                    self.vars.insert(n.clone(), v);
                    Ok(Flow::Normal)
                }
                StmtKind::Print(values) => {
                    let text = values
                        .iter()
                        .map(|e| self.eval(e).map(|v| v.to_string()))
                        .collect::<Result<Vec<_>, _>>()?
                        .join(" ");
                    println!("{text}");
                    Ok(Flow::Normal)
                }
                StmtKind::Expr(e) => {
                    self.eval(e)?;
                    Ok(Flow::Normal)
                }
                StmtKind::If(c, yes, no) => {
                    let flow = if self.eval(c)?.truthy() {
                        self.exec(yes)?
                    } else {
                        self.exec(no)?
                    };
                    if !matches!(flow, Flow::Normal) {
                        return Ok(flow);
                    }
                    Ok(Flow::Normal)
                }
                StmtKind::While(c, body) => {
                    while self.eval(c)?.truthy() {
                        match self.exec(body)? {
                            Flow::Normal | Flow::Continue => {}
                            Flow::Break => break,
                            Flow::Return(value) => return Ok(Flow::Return(value)),
                            Flow::Throw(value) => return Ok(Flow::Throw(value)),
                        }
                    }
                    Ok(Flow::Normal)
                }
                StmtKind::For(n, e, body) => {
                    let Value::List(items) = self.eval(e)? else {
                        return Err("for requires a list".into());
                    };
                    for item in items {
                        self.vars.insert(n.clone(), item);
                        match self.exec(body)? {
                            Flow::Normal | Flow::Continue => {}
                            Flow::Break => break,
                            Flow::Return(value) => return Ok(Flow::Return(value)),
                            Flow::Throw(value) => return Ok(Flow::Throw(value)),
                        }
                    }
                    Ok(Flow::Normal)
                }
                StmtKind::Break => return Ok(Flow::Break),
                StmtKind::Continue => return Ok(Flow::Continue),
                StmtKind::Function(name, params, body) => {
                    let function = Function {
                        params: params.clone(),
                        body: body.clone(),
                    };
                    if let Ok(mut registry) = function_registry().lock() {
                        registry.insert(name.clone(), function.clone());
                    }
                    self.functions.insert(name.clone(), function);
                    Ok(Flow::Normal)
                }
                StmtKind::Native(name, _params) => {
                    let func = native_for(name);
                    self.native_functions.insert(name.clone(), func);
                    self.vars.insert(name.clone(), Value::NativeFunction(name.clone()));
                    Ok(Flow::Normal)
                }
                StmtKind::Try(body, catches, finally_body) => {
                    enum Outcome {
                        Flow(Flow),
                        Error(String),
                    }
                    let outcome = match self.exec(body) {
                        Ok(Flow::Throw(value)) => {
                            match self.handle_catches(catches, value.clone()) {
                                Ok(Some(flow)) => Outcome::Flow(flow),
                                Ok(None) => Outcome::Flow(Flow::Throw(value)),
                                Err(e) => Outcome::Error(e),
                            }
                        }
                        Err(e) => {
                            let err_value = self.runtime_error(&e);
                            match self.handle_catches(catches, err_value) {
                                Ok(Some(flow)) => Outcome::Flow(flow),
                                Ok(None) => Outcome::Error(e),
                                Err(e2) => Outcome::Error(e2),
                            }
                        }
                        Ok(f) => Outcome::Flow(f),
                    };
                    if let Some(finally) = finally_body {
                        match self.exec(&finally) {
                            Ok(Flow::Normal | Flow::Continue) => {}
                            Ok(flow) => return Ok(flow),
                            Err(e) => return Err(e),
                        }
                    }
                    match outcome {
                        Outcome::Flow(flow) => {
                            if !matches!(flow, Flow::Normal) {
                                return Ok(flow);
                            }
                            Ok(Flow::Normal)
                        }
                        Outcome::Error(e) => Err(e),
                    }
                }
                StmtKind::Throw(e) => {
                    let val = self.eval(e)?;
                    return Ok(Flow::Throw(self.to_error(val, stmt.line, stmt.col)));
                }
                StmtKind::Import(imports) => {
                    for (module, alias) in imports {
                        let name = alias.clone().unwrap_or(module.clone());
                        if let Some(Value::Dict(existing)) = self.vars.get(module).cloned() {
                            let mut map = HashMap::new();
                            for (k, v) in existing {
                                map.insert(k, v);
                            }
                            self.imported_modules.insert(name, map);
                            continue;
                        }
                        let path = self.resolve_module(module)?;
                        let vars = self.run_module(&path, &name)?;
                        self.imported_modules.insert(name, vars);
                    }
                    Ok(Flow::Normal)
                }
                StmtKind::FromImport(module, items) => {
                    let vars = if let Some(Value::Dict(existing)) = self.vars.get(module).cloned()
                    {
                        existing.into_iter().collect()
                    } else {
                        let path = self.resolve_module(module)?;
                        self.run_module(&path, module)?
                    };
                    for (item, alias) in items {
                        let value = vars.get(item).cloned().ok_or_else(|| format!("item {} not found in {}", item, module))?;
                        let name = alias.clone().unwrap_or(item.clone());
                        self.vars.insert(name, value);
                    }
                    Ok(Flow::Normal)
                }
                StmtKind::Load(path) => {
                    let resolved = if path.ends_with(".z") || path.contains('/') {
                        path.clone()
                    } else {
                        self.resolve_module(path)?
                    };
                    let stem = std::path::Path::new(&resolved)
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "module".into());
                    let exports = self.run_module(&resolved, &stem)?;
                    for (name, value) in exports {
                        self.vars.insert(name, value);
                    }
                    Ok(Flow::Normal)
                }
                StmtKind::Include(path) => {
                    let stmts = parse_file(path)?;
                    let flow = self.exec(&stmts)?;
                    if !matches!(flow, Flow::Normal) {
                        return Ok(flow);
                    }
                    Ok(Flow::Normal)
                }
                StmtKind::Return(value) => {
                    return Ok(Flow::Return(match value {
                        Some(value) => self.eval(value)?,
                        None => Value::Null,
                    }));
                }
                StmtKind::Class(name, parent, body) => {
                    if let Some(parent) = parent {
                        if !self.classes.contains_key(parent) {
                            return Err(format!("unknown parent class: {parent}"));
                        }
                    }
                    let mut methods = HashMap::new();
                    for statement in body {
                        if let StmtKind::Function(method, params, body) = &statement.kind {
                            methods.insert(
                                method.clone(),
                                Function {
                                    params: params.clone(),
                                    body: body.clone(),
                                },
                            );
                        } else {
                            return Err(format!(
                                "class '{name}' may currently contain only methods"
                            ));
                        }
                    }
                    self.classes.insert(
                        name.clone(),
                        ZenClass {
                            parent: parent.clone(),
                            methods,
                        },
                    );
                    Ok(Flow::Normal)
                }
                StmtKind::SetMember(object, member, value) => {
                    match self.eval(object)? {
                        Value::Instance(instance) => {
                            instance
                                .lock()
                                .unwrap()
                                .fields
                                .insert(member.clone(), self.eval(value)?);
                        }
                        Value::Dict(dict) => {
                            let mut dict = dict;
                            dict.insert(member.clone(), self.eval(value)?);
                            // Only persist if assigned to a named variable
                            if let Expr::Var(name) = object {
                                self.vars.insert(name.clone(), Value::Dict(dict));
                            }
                        }
                        _ => return Err("member assignment requires an object".into()),
                    }
                    Ok(Flow::Normal)
                }
                StmtKind::Switch(value, cases, default_body) => {
                    let target = self.eval(value)?;
                    let mut matched = false;
                    let mut flow = Flow::Normal;
                    for (case_value, body) in cases {
                        if self.eval(case_value)? == target {
                            flow = self.exec(body)?;
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        if let Some(default) = default_body {
                            flow = self.exec(default)?;
                        }
                    }
                    if !matches!(flow, Flow::Normal) {
                        return Ok(flow);
                    }
                    Ok(Flow::Normal)
                }
            }
        }
    fn locate(&self, line: usize, col: usize, message: String) -> String {
        if line == 0 {
            return message;
        }
        if message.starts_with("Traceback (most recent call last):") {
            return message;
        }
        let mut out = String::from("Traceback (most recent call last):\n");
        out.push_str(&format!(
            "  File \"{}\", line {}, in {}\n",
            self.file,
            line,
            self.stack.last().map(|s| s.as_str()).unwrap_or("<module>")
        ));
        if let Some(src_line) = self.lines.get(line.wrapping_sub(1)) {
            let trimmed = src_line.trim();
            if !trimmed.is_empty() {
                out.push_str(&format!("    {trimmed}\n"));
                let pad = " ".repeat(4 + col.saturating_sub(1).min(trimmed.chars().count()));
                out.push_str(&format!("{pad}^\n"));
            }
        }
        out.push_str(&format!("Error: {message}\n"));
        out
    }
    fn runtime_error(&self, message: &str) -> Value {
        let msg = message
            .rsplit_once("\nError: ")
            .map(|(_, m)| m.to_string())
            .unwrap_or_else(|| message.to_string());
        let mut map = BTreeMap::new();
        map.insert("type".into(), Value::String("Error".into()));
        map.insert("message".into(), Value::String(msg));
        map.insert("file".into(), Value::String(self.file.clone()));
        map.insert("line".into(), Value::Number(0.0));
        map.insert("col".into(), Value::Number(0.0));
        Value::Dict(map)
    }
    fn to_error(&self, value: Value, line: usize, col: usize) -> Value {
        match value {
            Value::Dict(mut map) => {
                map.entry("type".into())
                    .or_insert_with(|| Value::String("Error".into()));
                map.entry("message".into())
                    .or_insert_with(|| Value::String(String::new()));
                map.entry("file".into())
                    .or_insert_with(|| Value::String(self.file.clone()));
                map.entry("line".into())
                    .or_insert_with(|| Value::Number(line as f64));
                map.entry("col".into())
                    .or_insert_with(|| Value::Number(col as f64));
                Value::Dict(map)
            }
            Value::Instance(instance) => {
                // `throw new MyError("msg")` — carry the class name and message.
                let instance = instance.lock().unwrap();
                let class_name = instance.class_name.clone();
                let message = instance
                    .fields
                    .get("message")
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| class_name.clone());
                let mut map = BTreeMap::new();
                map.insert(
                    "type".into(),
                    Value::String(class_name.rsplit('.').next().unwrap_or(&class_name).into()),
                );
                map.insert("message".into(), Value::String(message));
                map.insert("file".into(), Value::String(self.file.clone()));
                map.insert("line".into(), Value::Number(line as f64));
                map.insert("col".into(), Value::Number(col as f64));
                Value::Dict(map)
            }
            other => {
                let mut map = BTreeMap::new();
                map.insert("type".into(), Value::String("Error".into()));
                map.insert("message".into(), Value::String(other.to_string()));
                map.insert("file".into(), Value::String(self.file.clone()));
                map.insert("line".into(), Value::Number(line as f64));
                map.insert("col".into(), Value::Number(col as f64));
                Value::Dict(map)
            }
        }
    }
    fn is_error_class(&self, class_name: &str) -> bool {
        let mut current = Some(class_name.to_string());
        while let Some(name) = current {
            if name == "errors.Error" || name == "Error" {
                return true;
            }
            current = self.classes.get(&name).and_then(|c| c.parent.clone());
        }
        false
    }
    fn error_is_a(&self, err_type: &str, wanted: &str) -> bool {
        // `Error` / `errors.Error` is the universal base: it matches everything.
        if wanted == "Error" || wanted == "errors.Error" {
            return true;
        }
        if err_type == wanted {
            return true;
        }
        // Allow `catch ValueError` to match `errors.ValueError` and vice versa.
        let leaf = err_type.rsplit('.').next().unwrap_or(err_type);
        if leaf == wanted || (wanted.contains('.') && wanted.rsplit('.').next() == Some(leaf)) {
            return true;
        }
        // Walk the inheritance chain (custom subclasses of errors.Error included).
        let mut current = Some(err_type.to_string());
        while let Some(name) = current {
            if name == wanted {
                return true;
            }
            current = self.classes.get(&name).and_then(|c| c.parent.clone());
        }
        false
    }
    fn handle_catches(
        &mut self,
        catches: &[CatchClause],
        value: Value,
    ) -> Result<Option<Flow>, String> {
        let err_type = match &value {
            Value::Dict(map) => map
                .get("type")
                .map(|v| v.to_string())
                .unwrap_or_else(|| "Error".into()),
            Value::Instance(instance) => instance
                .lock()
                .unwrap()
                .class_name
                .rsplit('.')
                .next()
                .unwrap_or("Error")
                .to_string(),
            _ => "Error".into(),
        };
        for clause in catches {
            let matches = match &clause.kind {
                None => true,
                Some(kind) => self.error_is_a(&err_type, kind),
            };
            if matches {
                if let Some(var) = &clause.var {
                    let bind = match &value {
                        Value::Dict(map) => map
                            .get("message")
                            .cloned()
                            .unwrap_or(Value::String(err_type)),
                        other => other.clone(),
                    };
                    self.vars.insert(var.clone(), bind);
                }
                return self.exec(&clause.body).map(Some);
            }
        }
        Ok(None)
    }
    fn error_info(&self, value: &Value) -> (String, String) {
        match value {
            Value::Dict(map) => (
                map.get("type").map(|v| v.to_string()).unwrap_or_else(|| "Error".into()),
                map.get("message")
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            ),
            Value::Instance(instance) => {
                let instance = instance.lock().unwrap();
                (
                    instance
                        .class_name
                        .rsplit('.')
                        .next()
                        .unwrap_or("Error")
                        .to_string(),
                    instance
                        .fields
                        .get("message")
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                )
            }
            other => ("Error".into(), other.to_string()),
        }
    }
}

fn numbers_from_args(args: Vec<Value>) -> Result<Vec<f64>, String> {
    let mut values = Vec::new();
    for arg in args {
        match arg {
            Value::Number(n) => values.push(n),
            Value::List(items) => {
                for item in items {
                    match item {
                        Value::Number(n) => values.push(n),
                        _ => return Err("statistics expects numbers".into()),
                    }
                }
            }
            _ => return Err("statistics expects numbers".into()),
        }
    }
    Ok(values)
}

fn time_part_impl<F>(f: F) -> Result<Value, String>
where
    F: Fn(chrono::DateTime<chrono::Local>) -> f64,
{
    Ok(Value::Number(f(chrono::Local::now())))
}

fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn factorial(n: u64) -> u64 {
    let mut result = 1u64;
    for i in 2..=n {
        result = result.saturating_mul(i);
    }
    result
}

fn comb(n: u64, k: u64) -> u64 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut result = 1u64;
    for i in 0..k {
        result = result.saturating_mul(n - i) / (i + 1);
    }
    result
}

fn perm(n: u64, k: u64) -> u64 {
    if k > n {
        return 0;
    }
    let mut result = 1u64;
    for i in 0..k {
        result = result.saturating_mul(n - i);
    }
    result
}

fn hash_hex<D>(args: Vec<Value>) -> Result<Value, String>
where
    D: sha2::Digest + Default,
{
    let data = match args.first() {
        Some(Value::String(s)) => s.as_bytes(),
        _ => return Err("crypto hash expects a string".into()),
    };
    let mut hasher = D::default();
    hasher.update(data);
    let digest = hasher.finalize();
    Ok(Value::String(hex_encode(&digest)))
}

fn hmac_hex_sha256(args: Vec<Value>) -> Result<Value, String> {
    let (key, data) = key_data(args, "crypto.hmac_sha256 expects (key, data)")?;
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(key.as_bytes())
        .map_err(|e| format!("invalid hmac key: {e}"))?;
    mac.update(data.as_bytes());
    let result = mac.finalize();
    Ok(Value::String(hex_encode(&result.into_bytes())))
}

fn hmac_hex_sha1(args: Vec<Value>) -> Result<Value, String> {
    let (key, data) = key_data(args, "crypto.hmac_sha1 expects (key, data)")?;
    let mut mac = hmac::Hmac::<sha1::Sha1>::new_from_slice(key.as_bytes())
        .map_err(|e| format!("invalid hmac key: {e}"))?;
    mac.update(data.as_bytes());
    let result = mac.finalize();
    Ok(Value::String(hex_encode(&result.into_bytes())))
}

fn hmac_hex_md5(args: Vec<Value>) -> Result<Value, String> {
    let (key, data) = key_data(args, "crypto.hmac_md5 expects (key, data)")?;
    let mut mac = hmac::Hmac::<md5::Md5>::new_from_slice(key.as_bytes())
        .map_err(|e| format!("invalid hmac key: {e}"))?;
    mac.update(data.as_bytes());
    let result = mac.finalize();
    Ok(Value::String(hex_encode(&result.into_bytes())))
}

fn key_data(args: Vec<Value>, err: &'static str) -> Result<(String, String), String> {
    match args.as_slice() {
        [Value::String(k), Value::String(d)] => Ok((k.clone(), d.clone())),
        _ => Err(err.into()),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn to_key_bytes(key: &str) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    let key_bytes = key.as_bytes();
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = *key_bytes.get(i).unwrap_or(&0);
    }
    bytes
}

fn aes_encrypt(key: &str, data: &str, iv: Option<&str>) -> Result<String, String> {
    use aes::cipher::{BlockEncryptMut, KeyIvInit};
    use cbc::Encryptor;
    use aes::cipher::block_padding::Pkcs7;
    type Aes256Cbc = Encryptor<aes::Aes256>;

    let key_bytes = to_key_bytes(key);
    let iv_bytes = match iv {
        Some(iv) => {
            let mut bytes = [0u8; 16];
            let b = iv.as_bytes();
            for (i, byte) in bytes.iter_mut().enumerate() {
                *byte = *b.get(i).unwrap_or(&0);
            }
            bytes
        }
        None => {
            use rand::RngCore;
            let mut bytes = [0u8; 16];
            rand::rng().fill_bytes(&mut bytes);
            bytes
        }
    };
    let mut buf = vec![0u8; data.len() + 32];
    buf[..data.len()].copy_from_slice(data.as_bytes());
    let cipher = Aes256Cbc::new_from_slices(&key_bytes, &iv_bytes)
        .map_err(|e| format!("aes init failed: {e}"))?;
    let ct = cipher
        .encrypt_padded_mut::<Pkcs7>(&mut buf, data.len())
        .map_err(|e| format!("aes encrypt failed: {e}"))?;
    let mut result = Vec::with_capacity(iv_bytes.len() + ct.len());
    result.extend_from_slice(&iv_bytes);
    result.extend_from_slice(ct);
    Ok(result.iter().map(|b| format!("{b:02x}")).collect())
}

fn aes_decrypt(key: &str, data: &str, iv: Option<&str>) -> Result<String, String> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit};
    use cbc::Decryptor;
    use aes::cipher::block_padding::Pkcs7;
    type Aes256CbcDec = Decryptor<aes::Aes256>;

    let key_bytes = to_key_bytes(key);
    let raw = hex_decode(data).ok_or("aes_decrypt: invalid hex data")?;
    let (iv_bytes, ct) = match iv {
        Some(iv) => {
            let mut bytes = [0u8; 16];
            let b = iv.as_bytes();
            for (i, byte) in bytes.iter_mut().enumerate() {
                *byte = *b.get(i).unwrap_or(&0);
            }
            (bytes, raw.as_slice())
        }
        None => {
            if raw.len() < 16 {
                return Err("aes_decrypt: data too short".into());
            }
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&raw[..16]);
            (bytes, &raw[16..])
        }
    };
    let mut buf = ct.to_vec();
    let cipher = Aes256CbcDec::new_from_slices(&key_bytes, &iv_bytes)
        .map_err(|e| format!("aes init failed: {e}"))?;
    let pt = cipher
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| format!("aes decrypt failed: {e}"))?;
    Ok(String::from_utf8_lossy(pt).into_owned())
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

fn http_request_impl(args: &[Value], method: &str) -> Result<Value, String> {
    let url = match args.first() {
        Some(Value::String(s)) => s,
        _ => return Err(format!("http.{method} expects a url string")),
    };
    // Optional second arg: data (string) or dict options (headers, json, timeout)
    let mut body: Option<String> = None;
    let mut headers = BTreeMap::new();
    let mut timeout_secs: u64 = 30;
    if let Some(second) = args.get(1) {
        match second {
            Value::String(s) => body = Some(s.clone()),
            Value::Dict(opts) => {
                if let Some(Value::Dict(h)) = opts.get("headers") {
                    for (k, v) in h {
                        headers.insert(k.clone(), v.to_string());
                    }
                }
                if let Some(Value::String(j)) = opts.get("json") {
                    body = Some(j.clone());
                    headers.insert("Content-Type".into(), "application/json".into());
                }
                if let Some(Value::Number(t)) = opts.get("timeout") {
                    timeout_secs = *t as u64;
                }
            }
            _ => return Err(format!("http.{method} second argument must be data or opts")),
        }
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("http client build failed: {e}"))?;
    let mut req = client.request(
        reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|e| format!("invalid http method: {e}"))?,
        url,
    );
    for (k, v) in &headers {
        req = req.header(k, v);
    }
    if let Some(b) = &body {
        req = req.body(b.clone());
    }
    let resp = req.send().map_err(|e| format!("http {method} {url}: {e}"))?;
    let status = resp.status().as_u16() as f64;
    let header_map = resp.headers().clone();
    let body_text = resp
        .text()
        .map_err(|e| format!("http {method} {url}: {e}"))?;
    let mut header_dict = BTreeMap::new();
    for (name, value) in header_map.iter() {
        header_dict.insert(
            name.as_str().to_string(),
            Value::String(value.to_str().unwrap_or_default().to_string()),
        );
    }
    // Store the raw body in a process-wide cache so the response dict's
    // json()/text() natives (which receive the dict itself) can read it.
    let id = next_response_id();
    response_bodies()
        .lock()
        .map_err(|e| format!("response cache poisoned: {e}"))?
        .insert(id, body_text);
    let mut result = BTreeMap::new();
    result.insert("status".into(), Value::Number(status));
    result.insert("ok".into(), Value::Bool(status >= 200.0 && status < 400.0));
    result.insert("headers".into(), Value::Dict(header_dict));
    result.insert("__id".into(), Value::Number(id as f64));
    result.insert("json".into(), Value::NativeFunction("__http_response_json".into()));
    result.insert("text".into(), Value::NativeFunction("__http_response_text".into()));
    Ok(Value::Dict(result))
}

fn next_response_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn response_body_from_args(args: &[Value]) -> Result<String, String> {
    let id = match args.first() {
        Some(Value::Dict(d)) => match d.get("__id") {
            Some(Value::Number(n)) => *n as u64,
            _ => return Err("response.json/text called on non-response dict".into()),
        },
        _ => return Err("response.json/text expects the response dict".into()),
    };
    response_bodies()
        .lock()
        .map_err(|e| format!("response cache poisoned: {e}"))?
        .get(&id)
        .cloned()
        .ok_or_else(|| "response body no longer available".into())
}

fn fernet_encrypt_impl(key: &str, data: &str) -> Result<Value, String> {
    use aes::cipher::{BlockEncryptMut, KeyIvInit};
    use aes::cipher::block_padding::Pkcs7;
    use cbc::Encryptor;
    use base64::Engine;
    type Aes128CbcEnc = Encryptor<aes::Aes128>;
    use rand::RngCore;

    let key_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(key.trim())
        .map_err(|e| format!("fernet: invalid key: {e}"))?;
    if key_bytes.len() != 32 {
        return Err("fernet: key must be 32 bytes".into());
    }
    let mut iv = [0u8; 16];
    rand::rng().fill_bytes(&mut iv);
    let mut padded = vec![0u8; data.len() + 32];
    padded[..data.len()].copy_from_slice(data.as_bytes());
    let cipher = Aes128CbcEnc::new_from_slices(&key_bytes[..16], &iv)
        .map_err(|e| format!("fernet: {e}"))?;
    let ct = cipher
        .encrypt_padded_mut::<Pkcs7>(&mut padded, data.len())
        .map_err(|e| format!("fernet: {e}"))?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_be_bytes();

    let mut token = Vec::with_capacity(1 + 8 + 16 + ct.len() + 32);
    token.push(0x80);
    token.extend_from_slice(&ts);
    token.extend_from_slice(&iv);
    token.extend_from_slice(ct);

    // HMAC-SHA256 over the first part
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(&key_bytes)
        .map_err(|e| format!("fernet: {e}"))?;
    mac.update(&token);
    let mac_result = mac.finalize().into_bytes();
    token.extend_from_slice(&mac_result);

    Ok(Value::String(
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&token),
    ))
}

fn fernet_decrypt_impl(key: &str, token: &str) -> Result<Value, String> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit};
    use aes::cipher::block_padding::Pkcs7;
    use cbc::Decryptor;
    use base64::Engine;
    type Aes128CbcDec = Decryptor<aes::Aes128>;

    let key_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(key.trim())
        .map_err(|e| format!("fernet: invalid key: {e}"))?;
    if key_bytes.len() != 32 {
        return Err("fernet: key must be 32 bytes".into());
    }
    let token_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(token.trim())
        .map_err(|e| format!("fernet: invalid token: {e}"))?;
    if token_bytes.len() < 57 {
        return Err("fernet: token too short".into());
    }
    let (header, expected_mac) = token_bytes.split_at(token_bytes.len() - 32);
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(&key_bytes)
        .map_err(|e| format!("fernet: {e}"))?;
    mac.update(header);
    let computed = mac.finalize().into_bytes();
    if computed.as_slice() != expected_mac {
        return Err("fernet: invalid token (mac mismatch)".into());
    }
    if header[0] != 0x80 {
        return Err("fernet: unsupported token version".into());
    }
    let iv = &header[9..25];
    let ct = &header[25..];
    let mut buf = ct.to_vec();
    let cipher = Aes128CbcDec::new_from_slices(&key_bytes[..16], iv)
        .map_err(|e| format!("fernet: {e}"))?;
    let pt = cipher
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| format!("fernet: {e}"))?;
    Ok(Value::String(String::from_utf8_lossy(pt).into_owned()))
}

fn uuid_namespace(ns: &str) -> uuid::Uuid {
    match ns {
        "dns" => uuid::Uuid::NAMESPACE_DNS,
        "url" => uuid::Uuid::NAMESPACE_URL,
        "oid" => uuid::Uuid::NAMESPACE_OID,
        "x500" => uuid::Uuid::NAMESPACE_X500,
        other => uuid::Uuid::parse_str(other).unwrap_or(uuid::Uuid::NAMESPACE_DNS),
    }
}

fn color_style(code: u8, args: Vec<Value>) -> Result<Value, String> {
    Ok(Value::String(match args.first() {
        Some(Value::String(text)) => format!("\x1b[{code}m{text}\x1b[0m"),
        _ => format!("\x1b[{code}m"),
    }))
}

fn color_rgb_fg(args: Vec<Value>) -> Result<Value, String> {
    let (r, g, b, text) = color_rgb_args(args)?;
    Ok(Value::String(match text {
        Some(t) => format!("\x1b[38;2;{r};{g};{b}m{t}\x1b[0m"),
        None => format!("\x1b[38;2;{r};{g};{b}m"),
    }))
}

fn color_rgb_bg(args: Vec<Value>) -> Result<Value, String> {
    let (r, g, b, text) = color_rgb_args(args)?;
    Ok(Value::String(match text {
        Some(t) => format!("\x1b[48;2;{r};{g};{b}m{t}\x1b[0m"),
        None => format!("\x1b[48;2;{r};{g};{b}m"),
    }))
}

fn color_rgb_args(args: Vec<Value>) -> Result<(u32, u32, u32, Option<String>), String> {
    let (r, g, b) = match args.as_slice() {
        [Value::Number(r), Value::Number(g), Value::Number(b)] => {
            (*r as u32, *g as u32, *b as u32)
        }
        [Value::Number(r), Value::Number(g), Value::Number(b), _] => {
            (*r as u32, *g as u32, *b as u32)
        }
        [Value::String(hex)] => {
            let hex = hex.trim_start_matches('#');
            if hex.len() == 3 {
                let hex: String = hex.chars().flat_map(|c| [c, c]).collect();
                let r = u32::from_str_radix(&hex[0..2], 16).map_err(|_| "invalid hex")?;
                let g = u32::from_str_radix(&hex[2..4], 16).map_err(|_| "invalid hex")?;
                let b = u32::from_str_radix(&hex[4..6], 16).map_err(|_| "invalid hex")?;
                return Ok((r, g, b, None));
            }
            if hex.len() == 6 {
                let r = u32::from_str_radix(&hex[0..2], 16).map_err(|_| "invalid hex")?;
                let g = u32::from_str_radix(&hex[2..4], 16).map_err(|_| "invalid hex")?;
                let b = u32::from_str_radix(&hex[4..6], 16).map_err(|_| "invalid hex")?;
                return Ok((r, g, b, None));
            }
            return Err("color: invalid hex string".into());
        }
        _ => return Err("color.rgb expects (r, g, b, text?)".into()),
    };
    let text = args.get(3).map(|v| v.to_string());
    Ok((r, g, b, text))
}

fn color_hex_impl(args: Vec<Value>) -> Result<Value, String> {
    let hex = match args.first() {
        Some(Value::String(h)) => h,
        _ => return Err("color.hex expects a hex string".into()),
    };
    let text = args.get(1).map(|v| v.to_string());
    let (r, g, b, _) = color_rgb_args(vec![Value::String(hex.clone())])?;
    Ok(Value::String(match text {
        Some(t) => format!("\x1b[38;2;{r};{g};{b}m{t}\x1b[0m"),
        None => format!("\x1b[38;2;{r};{g};{b}m"),
    }))
}

fn color_named(code: u8, args: Vec<Value>) -> Result<Value, String> {
    Ok(Value::String(match args.first() {
        Some(Value::String(text)) => format!("\x1b[{code}m{text}\x1b[0m"),
        _ => format!("\x1b[{code}m"),
    }))
}

fn csv_parse_impl(text: &str) -> Value {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            ',' if !in_quotes => {
                row.push(Value::String(std::mem::take(&mut field)));
            }
            '\n' | '\r' if !in_quotes => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(Value::String(std::mem::take(&mut field)));
                if !(row.len() == 1 && row[0].to_string().is_empty()) {
                    rows.push(Value::List(std::mem::take(&mut row)));
                } else {
                    row.clear();
                }
            }
            other => field.push(other),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(Value::String(field));
        rows.push(Value::List(row));
    }
    Value::List(rows)
}

fn csv_field(v: &Value) -> String {
    let s = v.to_string();
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

fn csv_encode_impl(rows: &[Value], headers: Option<&Vec<Value>>) -> String {
    let mut lines = Vec::new();
    if let Some(headers) = headers {
        lines.push(headers.iter().map(csv_field).collect::<Vec<_>>().join(","));
    }
    for row in rows {
        match row {
            Value::List(items) => lines.push(items.iter().map(csv_field).collect::<Vec<_>>().join(",")),
            other => lines.push(csv_field(other)),
        }
    }
    lines.join("\n") + "\n"
}

fn simple_glob(pattern: &str, name: &str) -> bool {
    let mut regex_str = String::new();
    for c in pattern.chars() {
        match c {
            '*' => regex_str.push_str(".*"),
            '?' => regex_str.push('.'),
            other => {
                regex_str.push_str(&regex::escape(&other.to_string()));
            }
        }
    }
    regex::Regex::new(&format!("^{regex_str}$"))
        .map(|re| re.is_match(name))
        .unwrap_or(false)
}

fn arg_string(args: &[Value], i: usize) -> Result<String, String> {
    match args.get(i) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(Value::Number(n)) => Ok(n.to_string()),
        _ => Err(format!("argument {} must be a string", i + 1)),
    }
}

fn arg_number(args: &[Value], i: usize) -> Result<f64, String> {
    match args.get(i) {
        Some(Value::Number(n)) => Ok(*n),
        _ => Err(format!("argument {} must be a number", i + 1)),
    }
}

fn arg_dict(args: &[Value], i: usize) -> Result<BTreeMap<String, Value>, String> {
    match args.get(i) {
        Some(Value::Dict(d)) => Ok(d.clone()),
        _ => Err(format!("argument {} must be a dict", i + 1)),
    }
}

fn arg_list(args: &[Value], i: usize) -> Result<Vec<Value>, String> {
    match args.get(i) {
        Some(Value::List(l)) => Ok(l.clone()),
        _ => Err(format!("argument {} must be a list", i + 1)),
    }
}

fn session_socket(session: &Value) -> Result<Arc<Mutex<TcpStream>>, String> {
    match session {
        Value::Dict(d) => match d.get("socket") {
            Some(Value::Socket(s)) => Ok(s.clone()),
            _ => Err("session has no socket (was connect() used?)".into()),
        },
        _ => Err("expected a session dict returned by connect()".into()),
    }
}

fn stream_write_all(stream: &mut TcpStream, data: &[u8]) -> Result<(), String> {
    stream.write_all(data).map_err(|e| format!("socket write failed: {e}"))
}

fn read_line(stream: &mut TcpStream) -> Result<String, String> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte).map_err(|e| format!("socket read failed: {e}"))?;
        if n == 0 {
            break;
        }
        if byte[0] == b'\n' {
            break;
        }
        if byte[0] != b'\r' {
            buf.push(byte[0]);
        }
        if buf.len() > 65536 {
            return Err("line too long".into());
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn ftp_read_reply(stream: &mut TcpStream) -> Result<(u16, String), String> {
    let first = read_line(stream)?;
    if first.len() < 3 {
        return Err(format!("malformed FTP reply: {first}"));
    }
    let code: u16 = first[..3].parse().map_err(|_| format!("bad FTP reply: {first}"))?;
    if first.as_bytes().get(3) == Some(&b'-') {
        let mut full = first.clone();
        loop {
            let line = read_line(stream)?;
            full.push('\n');
            full.push_str(&line);
            if line.len() >= 4 && line[..3] == first[..3] && line.as_bytes().get(3) == Some(&b' ') {
                break;
            }
        }
        Ok((code, full))
    } else {
        Ok((code, first))
    }
}

fn ftp_data_connect(stream: &mut TcpStream) -> Result<TcpStream, String> {
    stream_write_all(stream, b"PASV\r\n")?;
    let (code, reply) = ftp_read_reply(stream)?;
    if code != 227 {
        return Err(format!("PASV failed ({code}): {reply}"));
    }
    let start = reply.find('(').ok_or_else(|| format!("no passive info in: {reply}"))?;
    let end = reply.find(')').ok_or_else(|| format!("no passive info in: {reply}"))?;
    let nums: Vec<u16> = reply[start + 1..end]
        .split(',')
        .map(|s| s.trim().parse().unwrap_or(0))
        .collect();
    if nums.len() != 6 {
        return Err(format!("bad PASV response: {reply}"));
    }
    let host = format!("{}.{}.{}.{}", nums[0], nums[1], nums[2], nums[3]);
    let port = nums[4] * 256 + nums[5];
    TcpStream::connect((host.as_str(), port)).map_err(|e| format!("FTP data connect failed: {e}"))
}

fn smtp_read_reply(stream: &mut TcpStream) -> Result<(u16, String), String> {
    let first = read_line(stream)?;
    if first.len() < 3 {
        return Err(format!("malformed SMTP reply: {first}"));
    }
    let code: u16 = first[..3].parse().map_err(|_| format!("bad SMTP reply: {first}"))?;
    if first.as_bytes().get(3) == Some(&b'-') {
        let mut full = first.clone();
        loop {
            let line = read_line(stream)?;
            full.push('\n');
            full.push_str(&line);
            if line.len() >= 4 && line[..3] == first[..3] && line.as_bytes().get(3) == Some(&b' ') {
                break;
            }
        }
        Ok((code, full))
    } else {
        Ok((code, first))
    }
}

fn imap_command(stream: &mut TcpStream, tag: &str, cmd: &str) -> Result<String, String> {
    stream_write_all(stream, format!("{tag} {cmd}\r\n").as_bytes())?;
    let mut response = String::new();
    loop {
        let line = read_line(stream)?;
        if line.starts_with('*') {
            response.push_str(&line);
            response.push('\n');
        } else if line.starts_with(tag) {
            if !line.contains(" OK") {
                return Err(format!("IMAP {cmd}: {line}"));
            }
            return Ok(response);
        } else {
            response.push_str(&line);
            response.push('\n');
        }
    }
}

fn strip_telnet_iac(buf: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len());
    let mut i = 0;
    while i < buf.len() {
        if buf[i] == 0xff {
            if i + 1 < buf.len() && buf[i + 1] == 0xff {
                out.push(0xff);
                i += 2;
                continue;
            }
            if i + 1 < buf.len() && buf[i + 1] == 0xfb {
                out.extend_from_slice(&[0xff, 0xfc]); // DO -> DONT
                i += 2;
                continue;
            }
            if i + 1 < buf.len() && buf[i + 1] == 0xfd {
                out.extend_from_slice(&[0xff, 0xfe]); // WILL -> WONT
                i += 2;
                continue;
            }
            if i + 2 < buf.len() {
                i += 3;
                continue;
            }
            i = buf.len();
        } else {
            out.push(buf[i]);
            i += 1;
        }
    }
    out
}

fn dns_name_to_bytes(name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for label in name.trim_end_matches('.').split('.') {
        let bytes = label.as_bytes();
        if !bytes.is_empty() {
            out.push(bytes.len() as u8);
            out.extend_from_slice(bytes);
        }
    }
    out.push(0);
    out
}

fn dns_read_name(bytes: &[u8], offset: usize) -> (String, usize) {
    let mut labels = Vec::new();
    let mut pos = offset;
    let mut jumped = false;
    let mut jump_from = offset;
    loop {
        if pos >= bytes.len() {
            break;
        }
        let len = bytes[pos];
        if len == 0 {
            pos += 1;
            if !jumped {
                jump_from = pos;
            }
            break;
        }
        if len & 0xc0 == 0xc0 {
            if pos + 1 >= bytes.len() {
                break;
            }
            let ptr = (((len & 0x3f) as usize) << 8) | bytes[pos + 1] as usize;
            if !jumped {
                jump_from = pos + 2;
            }
            pos = ptr;
            jumped = true;
            continue;
        }
        if pos + 1 + len as usize > bytes.len() {
            break;
        }
        labels.push(String::from_utf8_lossy(&bytes[pos + 1..pos + 1 + len as usize]).into_owned());
        pos += 1 + len as usize;
    }
    (labels.join("."), jump_from)
}

fn internet_checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += ((data[i] as u32) << 8) | data[i + 1] as u32;
        i += 2;
    }
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

fn ip_str_to_u32(ip: &str) -> Result<u32, String> {
    let parts: Vec<u32> = ip
        .split('.')
        .map(|s| s.parse().unwrap_or(0))
        .collect();
    if parts.len() != 4 {
        return Err(format!("bad IPv4 address: {ip}"));
    }
    Ok((parts[0] << 24) | (parts[1] << 16) | (parts[2] << 8) | parts[3])
}

fn u32_to_ip(v: u32) -> String {
    format!(
        "{}.{}.{}.{}",
        (v >> 24) & 0xff,
        (v >> 16) & 0xff,
        (v >> 8) & 0xff,
        v & 0xff
    )
}

fn layer_bytes(layer: &BTreeMap<String, Value>) -> Result<Vec<u8>, String> {
    match layer.get("type") {
        Some(Value::String(t)) if t == "Raw" => match layer.get("data") {
            Some(Value::String(d)) => Ok(d.as_bytes().to_vec()),
            Some(Value::List(l)) => Ok(l
                .iter()
                .map(|v| match v {
                    Value::Number(n) => *n as u8 as u32 as u8,
                    _ => 0,
                })
                .collect()),
            _ => Err("raw layer needs data".into()),
        },
        _ => {
            let payload = match layer.get("payload") {
                Some(Value::Dict(p)) => layer_bytes(p)?,
                Some(Value::String(s)) => s.as_bytes().to_vec(),
                _ => Vec::new(),
            };
            match layer.get("type") {
                Some(Value::String(t)) if t == "IP" => {
                    let src = match layer.get("src") {
                        Some(Value::String(s)) => s,
                        _ => return Err("IP layer needs src".into()),
                    };
                    let dst = match layer.get("dst") {
                        Some(Value::String(s)) => s,
                        _ => return Err("IP layer needs dst".into()),
                    };
                    let proto: u8 = match layer.get("proto") {
                        Some(Value::String(p)) if p == "TCP" => 6,
                        Some(Value::String(p)) if p == "UDP" => 17,
                        Some(Value::String(p)) if p == "ICMP" => 1,
                        Some(Value::Number(n)) => *n as u8,
                        _ => match payload.first() {
                            Some(b) => match b {
                                0x45..=0xff => 6,
                                _ => 17,
                            },
                            None => 1,
                        },
                    };
                    let ttl: u8 = match layer.get("ttl") {
                        Some(Value::Number(n)) => *n as u8,
                        _ => 64,
                    };
                    let id: u16 = match layer.get("id") {
                        Some(Value::Number(n)) => *n as u16,
                        _ => rand::random(),
                    };
                    let total = 20 + payload.len();
                    let mut header = Vec::with_capacity(20);
                    header.push(0x45);
                    header.push(0);
                    header.extend_from_slice(&((total as u16).to_be_bytes()));
                    header.extend_from_slice(&id.to_be_bytes());
                    header.extend_from_slice(&[0x40, 0]);
                    header.push(ttl);
                    header.push(proto);
                    header.extend_from_slice(&[0, 0]);
                    header.extend_from_slice(&ip_str_to_u32(src)?.to_be_bytes());
                    header.extend_from_slice(&ip_str_to_u32(dst)?.to_be_bytes());
                    let csum = internet_checksum(&header);
                    header[10] = (csum >> 8) as u8;
                    header[11] = (csum & 0xff) as u8;
                    header.extend_from_slice(&payload);
                    Ok(header)
                }
                Some(Value::String(t)) if t == "UDP" => {
                    let sport: u16 = match layer.get("sport") {
                        Some(Value::Number(n)) => *n as u16,
                        _ => 0,
                    };
                    let dport: u16 = match layer.get("dport") {
                        Some(Value::Number(n)) => *n as u16,
                        _ => return Err("UDP layer needs dport".into()),
                    };
                    let len = 8 + payload.len();
                    let mut seg = Vec::with_capacity(8 + payload.len());
                    seg.extend_from_slice(&sport.to_be_bytes());
                    seg.extend_from_slice(&dport.to_be_bytes());
                    seg.extend_from_slice(&(len as u16).to_be_bytes());
                    seg.extend_from_slice(&[0, 0]);
                    seg.extend_from_slice(&payload);
                    // pseudo-header checksum (optional field; 0 if not computed)
                    if let (Some(Value::String(src)), Some(Value::String(dst))) =
                        (layer.get("src"), layer.get("dst"))
                    {
                        let mut pseudo = Vec::new();
                        pseudo.extend_from_slice(&ip_str_to_u32(src)?.to_be_bytes());
                        pseudo.extend_from_slice(&ip_str_to_u32(dst)?.to_be_bytes());
                        pseudo.extend_from_slice(&[0, 17]);
                        pseudo.extend_from_slice(&(len as u16).to_be_bytes());
                        pseudo.extend_from_slice(&seg);
                        let csum = internet_checksum(&pseudo);
                        seg[6] = (csum >> 8) as u8;
                        seg[7] = (csum & 0xff) as u8;
                    }
                    Ok(seg)
                }
                Some(Value::String(t)) if t == "TCP" => {
                    let sport: u16 = match layer.get("sport") {
                        Some(Value::Number(n)) => *n as u16,
                        _ => 0,
                    };
                    let dport: u16 = match layer.get("dport") {
                        Some(Value::Number(n)) => *n as u16,
                        _ => return Err("TCP layer needs dport".into()),
                    };
                    let seq: u32 = match layer.get("seq") {
                        Some(Value::Number(n)) => *n as u32,
                        _ => 0,
                    };
                    let ack: u32 = match layer.get("ack") {
                        Some(Value::Number(n)) => *n as u32,
                        _ => 0,
                    };
                    let flags: u8 = match layer.get("flags") {
                        Some(Value::Number(n)) => *n as u8,
                        Some(Value::String(s)) => {
                            let mut f = 0u8;
                            if s.contains('S') {
                                f |= 0x02;
                            }
                            if s.contains('A') {
                                f |= 0x10;
                            }
                            if s.contains('F') {
                                f |= 0x01;
                            }
                            if s.contains('R') {
                                f |= 0x04;
                            }
                            if s.contains('P') {
                                f |= 0x08;
                            }
                            f
                        }
                        _ => 0x02,
                    };
                    let window: u16 = match layer.get("window") {
                        Some(Value::Number(n)) => *n as u16,
                        _ => 65535,
                    };
                    let mut seg = Vec::with_capacity(20 + payload.len());
                    seg.extend_from_slice(&sport.to_be_bytes());
                    seg.extend_from_slice(&dport.to_be_bytes());
                    seg.extend_from_slice(&seq.to_be_bytes());
                    seg.extend_from_slice(&ack.to_be_bytes());
                    seg.extend_from_slice(&[0x50, flags]);
                    seg.extend_from_slice(&window.to_be_bytes());
                    seg.extend_from_slice(&[0, 0]);
                    seg.extend_from_slice(&[0, 0]);
                    seg.extend_from_slice(&payload);
                    if let (Some(Value::String(src)), Some(Value::String(dst))) =
                        (layer.get("src"), layer.get("dst"))
                    {
                        let mut pseudo = Vec::new();
                        pseudo.extend_from_slice(&ip_str_to_u32(src)?.to_be_bytes());
                        pseudo.extend_from_slice(&ip_str_to_u32(dst)?.to_be_bytes());
                        pseudo.extend_from_slice(&[0, 6]);
                        pseudo.extend_from_slice(&((20 + payload.len()) as u16).to_be_bytes());
                        pseudo.extend_from_slice(&seg);
                        let csum = internet_checksum(&pseudo);
                        seg[16] = (csum >> 8) as u8;
                        seg[17] = (csum & 0xff) as u8;
                    }
                    Ok(seg)
                }
                Some(Value::String(t)) if t == "ICMP" => {
                    let icmp_type: u8 = match layer.get("icmp_type") {
                        Some(Value::Number(n)) => *n as u8,
                        _ => 8,
                    };
                    let icmp_code: u8 = match layer.get("icmp_code") {
                        Some(Value::Number(n)) => *n as u8,
                        _ => 0,
                    };
                    let mut seg = Vec::with_capacity(4 + payload.len());
                    seg.push(icmp_type);
                    seg.push(icmp_code);
                    seg.extend_from_slice(&[0, 0]);
                    seg.extend_from_slice(&payload);
                    let csum = internet_checksum(&seg);
                    seg[2] = (csum >> 8) as u8;
                    seg[3] = (csum & 0xff) as u8;
                    Ok(seg)
                }
                other => Err(format!("unknown packet layer type: {other:?}")),
            }
        }
    }
}

fn raw_socket_send(data: &[u8]) -> Result<(), String> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_RAW, libc::IPPROTO_RAW) };
    if fd < 0 {
        return Err("scapy.send: could not open raw socket (requires root / CAP_NET_RAW): check the OS error".into());
    }
    let on: libc::c_int = 1;
    unsafe {
        libc::setsockopt(
            fd,
            libc::IPPROTO_IP,
            libc::IP_HDRINCL,
            &on as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
    }
    let dst = match data.get(16..20) {
        Some(b) => IpAddr::from([b[0], b[1], b[2], b[3]]),
        None => return Err("scapy.send: packet too short".into()),
    };
    let sa = SocketAddr::new(dst, 0);
    let result = unsafe {
        libc::sendto(
            fd,
            data.as_ptr() as *const libc::c_void,
            data.len(),
            0,
            &sa as *const SocketAddr as *const libc::sockaddr,
            std::mem::size_of::<SocketAddr>() as libc::socklen_t,
        )
    };
    let err = std::io::Error::last_os_error();
    unsafe { libc::close(fd) };
    if result < 0 {
        return Err(format!("scapy.send failed: {err}"));
    }
    Ok(())
}

fn struct_size_of(fmt: &str) -> Result<usize, String> {
    let mut size = 0usize;
    let mut count = 0usize;
    for c in fmt.chars() {
        match c {
            '<' | '>' | '=' | '!' | '@' => continue,
            '0'..='9' => {
                count = count * 10 + (c as usize - '0' as usize);
            }
            _ => {
                let n = if count == 0 { 1 } else { count };
                count = 0;
                let sz = match c {
                    'x' | 'b' | 'B' | '?' => 1,
                    'h' | 'H' => 2,
                    'i' | 'I' | 'l' | 'L' | 'f' => 4,
                    'q' | 'Q' | 'd' => 8,
                    's' | 'p' => return Err("struct: s/p need an explicit size like 4s".into()),
                    _ => return Err(format!("struct: unknown format char '{c}'")),
                };
                size += sz * n;
            }
        }
    }
    Ok(size)
}

fn struct_parse_format(fmt: &str) -> Result<(bool, Vec<(char, usize)>), String> {
    let big_endian = match fmt.chars().next() {
        Some('>') | Some('!') => true,
        _ => false,
    };
    let mut codes = Vec::new();
    let mut count = 0usize;
    let mut prev: Option<char> = None;
    let mut size_checked = false;
    for c in fmt.chars() {
        if c.is_ascii_digit() {
            count = count * 10 + (c as usize - '0' as usize);
            continue;
        }
        match c {
            '>' | '!' | '<' | '=' | '@' => continue,
            _ => {}
        }
        if let Some(p) = prev.take() {
            let n = if count == 0 { 1 } else { count };
            if p == 's' || p == 'p' {
                size_checked = true;
            }
            codes.push((p, n));
            count = 0;
        }
        if c == 's' || c == 'p' {
            let n = if count == 0 { 1 } else { count };
            codes.push((c, n));
            count = 0;
        } else {
            prev = Some(c);
        }
    }
    if let Some(p) = prev {
        let n = if count == 0 { 1 } else { count };
        codes.push((p, n));
    }
    let _ = size_checked;
    Ok((big_endian, codes))
}

fn hexlify(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len() * 2);
    for b in data {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn dns_type_name(t: u16) -> &'static str {
    match t {
        1 => "A",
        2 => "NS",
        5 => "CNAME",
        15 => "MX",
        16 => "TXT",
        28 => "AAAA",
        _ => "OTHER",
    }
}

fn default_dns_server() -> String {
    if let Ok(content) = fs::read_to_string("/etc/resolv.conf") {
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("nameserver") {
                let server = rest.trim();
                if server.split('.').count() == 4 {
                    return server.to_string();
                }
            }
        }
    }
    "8.8.8.8".into()
}

fn dns_query_impl(name: &str, rtype: &str) -> Result<Vec<Value>, String> {
    let qtype: u16 = match rtype.to_uppercase().as_str() {
        "A" => 1,
        "NS" => 2,
        "CNAME" => 5,
        "MX" => 15,
        "TXT" => 16,
        "AAAA" => 28,
        _ => return Err(format!("dns: unsupported type {rtype}")),
    };
    let server = default_dns_server();
    let mut query = Vec::new();
    let id: u16 = rand::random();
    query.extend_from_slice(&id.to_be_bytes());
    query.extend_from_slice(&0x0100u16.to_be_bytes());
    query.extend_from_slice(&1u16.to_be_bytes());
    query.extend_from_slice(&0u16.to_be_bytes());
    query.extend_from_slice(&0u16.to_be_bytes());
    query.extend_from_slice(&0u16.to_be_bytes());
    query.extend_from_slice(&dns_name_to_bytes(name));
    query.extend_from_slice(&qtype.to_be_bytes());
    query.extend_from_slice(&1u16.to_be_bytes());
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("dns: bind: {e}"))?;
    sock.set_read_timeout(Some(Duration::from_secs(5))).ok();
    sock.send_to(&query, (server.as_str(), 53))
        .map_err(|e| format!("dns: send to {server}: {e}"))?;
    let mut buf = [0u8; 4096];
    let (n, _) = sock
        .recv_from(&mut buf)
        .map_err(|e| format!("dns: no response from {server}: {e}"))?;
    let resp = &buf[..n];
    if resp.len() < 12 {
        return Ok(Vec::new());
    }
    let ancount = u16::from_be_bytes([resp[6], resp[7]]) as usize;
    let mut pos = 12usize;
    let (_, npos) = dns_read_name(resp, pos);
    pos = npos + 4;
    let mut results = Vec::new();
    for _ in 0..ancount {
        if pos >= resp.len() {
            break;
        }
        let (rname, npos) = dns_read_name(resp, pos);
        pos = npos;
        if pos + 10 > resp.len() {
            break;
        }
        let rtype = u16::from_be_bytes([resp[pos], resp[pos + 1]]);
        let ttl = u32::from_be_bytes([resp[pos + 4], resp[pos + 5], resp[pos + 6], resp[pos + 7]]);
        let rdlen = u16::from_be_bytes([resp[pos + 8], resp[pos + 9]]) as usize;
        pos += 10;
        if pos + rdlen > resp.len() {
            break;
        }
        let rdata = &resp[pos..pos + rdlen];
        let data = match rtype {
            1 if rdlen == 4 => u32_to_ip(u32::from_be_bytes([
                rdata[0], rdata[1], rdata[2], rdata[3],
            ])),
            28 if rdlen == 16 => {
                let mut parts = Vec::new();
                for i in (0..16).step_by(2) {
                    parts.push(format!("{:02x}{:02x}", rdata[i], rdata[i + 1]));
                }
                parts.join(":")
            }
            15 if rdlen >= 3 => {
                let pref = u16::from_be_bytes([rdata[0], rdata[1]]);
                let (hostname, _) = dns_read_name(resp, pos + 2);
                format!("{pref} {hostname}")
            }
            16 => String::from_utf8_lossy(rdata).into_owned(),
            2 | 5 => {
                let (hostname, _) = dns_read_name(resp, pos);
                hostname
            }
            _ => hexlify(rdata),
        };
        pos += rdlen;
        let mut rec = BTreeMap::new();
        rec.insert("name".into(), Value::String(rname));
        rec.insert("type".into(), Value::String(dns_type_name(rtype).into()));
        rec.insert("ttl".into(), Value::Number(ttl as f64));
        rec.insert("data".into(), Value::String(data));
        results.push(Value::Dict(rec));
    }
    Ok(results)
}

fn parse_packet(data: &[u8]) -> Value {
    if data.len() < 20 || (data[0] >> 4) != 4 {
        return Value::Null;
    }
    let ihl = ((data[0] & 0x0f) as usize) * 4;
    if data.len() < ihl {
        return Value::Null;
    }
    let src = u32_to_ip(u32::from_be_bytes([data[12], data[13], data[14], data[15]]));
    let dst = u32_to_ip(u32::from_be_bytes([data[16], data[17], data[18], data[19]]));
    let proto = data[9];
    let payload = &data[ihl..];
    let mut ip_layer = BTreeMap::new();
    ip_layer.insert("type".into(), Value::String("IP".into()));
    ip_layer.insert("src".into(), Value::String(src));
    ip_layer.insert("dst".into(), Value::String(dst));
    let inner = match proto {
        6 => {
            ip_layer.insert("proto".into(), Value::String("TCP".into()));
            if payload.len() < 20 {
                Value::Null
            } else {
                let sport = u16::from_be_bytes([payload[0], payload[1]]) as f64;
                let dport = u16::from_be_bytes([payload[2], payload[3]]) as f64;
                let seq = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]) as f64;
                let ack = u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]]) as f64;
                let flags = payload[13];
                let data_offset = ((payload[12] >> 4) as usize) * 4;
                let mut tcp_layer = BTreeMap::new();
                tcp_layer.insert("type".into(), Value::String("TCP".into()));
                tcp_layer.insert("sport".into(), Value::Number(sport));
                tcp_layer.insert("dport".into(), Value::Number(dport));
                tcp_layer.insert("seq".into(), Value::Number(seq));
                tcp_layer.insert("ack".into(), Value::Number(ack));
                tcp_layer.insert("flags".into(), Value::Number(flags as f64));
                if payload.len() > data_offset {
                    tcp_layer.insert(
                        "payload".into(),
                        Value::String(String::from_utf8_lossy(&payload[data_offset..]).into_owned()),
                    );
                }
                Value::Dict(tcp_layer)
            }
        }
        17 => {
            ip_layer.insert("proto".into(), Value::String("UDP".into()));
            if payload.len() < 8 {
                Value::Null
            } else {
                let sport = u16::from_be_bytes([payload[0], payload[1]]) as f64;
                let dport = u16::from_be_bytes([payload[2], payload[3]]) as f64;
                let mut udp_layer = BTreeMap::new();
                udp_layer.insert("type".into(), Value::String("UDP".into()));
                udp_layer.insert("sport".into(), Value::Number(sport));
                udp_layer.insert("dport".into(), Value::Number(dport));
                if payload.len() > 8 {
                    udp_layer.insert(
                        "payload".into(),
                        Value::String(String::from_utf8_lossy(&payload[8..]).into_owned()),
                    );
                }
                Value::Dict(udp_layer)
            }
        }
        1 => {
            ip_layer.insert("proto".into(), Value::String("ICMP".into()));
            if payload.is_empty() {
                Value::Null
            } else {
                let mut icmp_layer = BTreeMap::new();
                icmp_layer.insert("type".into(), Value::String("ICMP".into()));
                icmp_layer.insert("icmp_type".into(), Value::Number(payload[0] as f64));
                icmp_layer.insert("icmp_code".into(), Value::Number(payload[1] as f64));
                if payload.len() > 4 {
                    icmp_layer.insert(
                        "payload".into(),
                        Value::String(String::from_utf8_lossy(&payload[4..]).into_owned()),
                    );
                }
                Value::Dict(icmp_layer)
            }
        }
        _ => {
            ip_layer.insert("proto".into(), Value::Number(proto as f64));
            if !payload.is_empty() {
                let mut raw = BTreeMap::new();
                raw.insert("type".into(), Value::String("Raw".into()));
                raw.insert("data".into(), Value::String(hexlify(payload)));
                Value::Dict(raw)
            } else {
                Value::Null
            }
        }
    };
    if !matches!(inner, Value::Null) {
        ip_layer.insert("payload".into(), inner);
    }
    Value::Dict(ip_layer)
}

fn sniff_packets(count: u32, timeout_secs: u64) -> Result<Value, String> {
    let fd = unsafe { libc::socket(libc::AF_PACKET, libc::SOCK_RAW, 0) };
    if fd < 0 {
        return Err(
            "scapy.sniff: could not open AF_PACKET raw socket (requires root / CAP_NET_RAW)".into(),
        );
    }
    let tv = libc::timeval {
        tv_sec: timeout_secs as libc::time_t,
        tv_usec: 0,
    };
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            &tv as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::timeval>() as libc::socklen_t,
        );
    }
    let mut packets = Vec::new();
    let deadline = SystemTime::now() + Duration::from_secs(timeout_secs);
    let mut buf = [0u8; 65535];
    while packets.len() < count as usize && SystemTime::now() < deadline {
        let n = unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0) };
        if n < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::WouldBlock
                || err.kind() == std::io::ErrorKind::TimedOut
            {
                break;
            }
            unsafe { libc::close(fd) };
            return Err(format!("scapy.sniff: recv failed: {err}"));
        }
        let frame = &buf[..n as usize];
        if frame.len() >= 34 && frame[12] == 0x08 && frame[13] == 0x00 {
            let ip = parse_packet(&frame[14..]);
            if !matches!(ip, Value::Null) {
                packets.push(ip);
            }
        }
    }
    unsafe { libc::close(fd) };
    Ok(Value::List(packets))
}

fn pack_value(values: &[Value], vi: &mut usize) -> Result<f64, String> {
    match values.get(*vi) {
        Some(Value::Number(n)) => {
            *vi += 1;
            Ok(*n)
        }
        Some(Value::String(s)) => {
            *vi += 1;
            s.parse::<f64>()
                .map_err(|_| format!("struct.pack: '{s}' is not a number"))
        }
        _ => Err("struct.pack: not enough values for format".into()),
    }
}

fn be_bytes(v: u64, n: usize) -> Vec<u8> {
    v.to_be_bytes()[8 - n..].to_vec()
}

fn le_bytes(v: u64, n: usize) -> Vec<u8> {
    v.to_le_bytes()[..n].to_vec()
}

fn pack_impl(fmt: &str, values: &[Value]) -> Result<Value, String> {
    let (big, codes) = struct_parse_format(fmt)?;
    let mut out: Vec<u8> = Vec::new();
    let mut vi = 0usize;
    for (code, count) in codes {
        match code {
            'x' => out.extend(std::iter::repeat(0u8).take(count)),
            's' | 'p' => {
                let s = match values.get(vi) {
                    Some(Value::String(s)) => {
                        vi += 1;
                        s.clone()
                    }
                    _ => return Err("struct.pack: 's' needs a string value".into()),
                };
                let bytes = s.as_bytes();
                let n = count.min(bytes.len());
                out.extend_from_slice(&bytes[..n]);
                out.extend(std::iter::repeat(0u8).take(count - n));
            }
            _ => {
                let size = match code {
                    'b' | 'B' | '?' => 1,
                    'h' | 'H' => 2,
                    'i' | 'I' | 'l' | 'L' | 'f' => 4,
                    'q' | 'Q' | 'd' => 8,
                    _ => return Err(format!("struct.pack: unsupported format char '{code}'")),
                };
                for _ in 0..count {
                    let v = pack_value(values, &mut vi)?;
                    let raw: u64 = match code {
                        'b' => {
                            out.push((v as i8) as u8);
                            continue;
                        }
                        'B' | '?' => {
                            out.push(v as u8);
                            continue;
                        }
                        'h' => (v as i16) as u16 as u64,
                        'H' => v as u16 as u64,
                        'i' | 'l' => (v as i32) as u32 as u64,
                        'I' | 'L' => v as u32 as u64,
                        'q' => v as i64 as u64,
                        'Q' => v as u64,
                        'f' => (v as f32).to_bits() as u64,
                        'd' => v.to_bits(),
                        _ => 0,
                    };
                    let bytes = if big { be_bytes(raw, size) } else { le_bytes(raw, size) };
                    out.extend_from_slice(&bytes);
                }
            }
        }
    }
    Ok(Value::String(String::from_utf8_lossy(&out).into_owned()))
}

fn unpack_impl(fmt: &str, data: &[u8]) -> Result<Value, String> {
    let (big, codes) = struct_parse_format(fmt)?;
    let mut out: Vec<Value> = Vec::new();
    let mut pos = 0usize;
    for (code, count) in codes {
        match code {
            'x' => pos += count,
            's' | 'p' => {
                if pos + count > data.len() {
                    return Err("struct.unpack: data too short".into());
                }
                out.push(Value::String(
                    String::from_utf8_lossy(&data[pos..pos + count]).into_owned(),
                ));
                pos += count;
            }
            _ => {
                let size = match code {
                    'b' | 'B' | '?' => 1,
                    'h' | 'H' => 2,
                    'i' | 'I' | 'l' | 'L' | 'f' => 4,
                    'q' | 'Q' | 'd' => 8,
                    _ => return Err(format!("struct.unpack: unsupported format char '{code}'")),
                };
                for _ in 0..count {
                    if pos + size > data.len() {
                        return Err("struct.unpack: data too short".into());
                    }
                    let mut raw = [0u8; 8];
                    if big {
                        raw[8 - size..].copy_from_slice(&data[pos..pos + size]);
                    } else {
                        raw[..size].copy_from_slice(&data[pos..pos + size]);
                    }
                    let v = match code {
                        'b' => {
                            out.push(Value::Number(data[pos] as i8 as f64));
                            pos += 1;
                            continue;
                        }
                        'B' | '?' => {
                            out.push(Value::Number(data[pos] as f64));
                            pos += 1;
                            continue;
                        }
                        'h' => u64::from_be_bytes(raw) as i16 as f64,
                        'H' => u64::from_be_bytes(raw) as u16 as f64,
                        'i' | 'l' => u64::from_be_bytes(raw) as i32 as f64,
                        'I' | 'L' => u64::from_be_bytes(raw) as u32 as f64,
                        'q' => u64::from_be_bytes(raw) as i64 as f64,
                        'Q' => u64::from_be_bytes(raw) as f64,
                        'f' => f32::from_bits(u64::from_be_bytes(raw) as u32) as f64,
                        'd' => f64::from_bits(u64::from_be_bytes(raw)),
                        _ => 0.0,
                    };
                    out.push(Value::Number(v));
                    pos += size;
                }
            }
        }
    }
    Ok(Value::List(out))
}

fn crypto_digest(data: &str, algo: &str) -> String {
    use sha2::Digest;
    let bytes = data.as_bytes();
    let digest = match algo {
        "md5" => {
            let mut h = md5::Md5::default();
            h.update(bytes);
            h.finalize().to_vec()
        }
        "sha1" => {
            let mut h = sha1::Sha1::default();
            h.update(bytes);
            h.finalize().to_vec()
        }
        "sha224" => {
            let mut h = sha2::Sha224::default();
            h.update(bytes);
            h.finalize().to_vec()
        }
        "sha256" => {
            let mut h = sha2::Sha256::default();
            h.update(bytes);
            h.finalize().to_vec()
        }
        "sha384" => {
            let mut h = sha2::Sha384::default();
            h.update(bytes);
            h.finalize().to_vec()
        }
        "sha512" => {
            let mut h = sha2::Sha512::default();
            h.update(bytes);
            h.finalize().to_vec()
        }
        "sha3_256" => {
            let mut h = sha3::Sha3_256::default();
            h.update(bytes);
            h.finalize().to_vec()
        }
        "sha3_512" => {
            let mut h = sha3::Sha3_512::default();
            h.update(bytes);
            h.finalize().to_vec()
        }
        "blake2b" => {
            let mut h = blake2::Blake2b512::default();
            h.update(bytes);
            h.finalize().to_vec()
        }
        "blake2s" => {
            let mut h = blake2::Blake2s256::default();
            h.update(bytes);
            h.finalize().to_vec()
        }
        _ => Vec::new(),
    };
    hexlify(&digest)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            fs::create_dir_all(&to).map_err(|e| e.to_string())?;
            copy_dir_recursive(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn flatten_list(list: &[Value], out: &mut Vec<Value>) {
    for item in list {
        if let Value::List(inner) = item {
            flatten_list(inner, out);
        } else {
            out.push(item.clone());
        }
    }
}

fn permutations(
    list: &[Value],
    r: usize,
    current: &mut Vec<Value>,
    used: &mut Vec<bool>,
    out: &mut Vec<Value>,
) {
    if current.len() == r {
        out.push(Value::List(current.clone()));
        return;
    }
    for i in 0..list.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        current.push(list[i].clone());
        permutations(list, r, current, used, out);
        current.pop();
        used[i] = false;
    }
}

fn combinations(list: &[Value], r: usize, start: usize, current: &mut Vec<Value>, out: &mut Vec<Value>) {
    if current.len() == r {
        out.push(Value::List(current.clone()));
        return;
    }
    for i in start..list.len() {
        current.push(list[i].clone());
        combinations(list, r, i + 1, current, out);
        current.pop();
    }
}

fn url_quote(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn url_unquote(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn native_for(name: &str) -> NativeFunc {
    match name {
        "math_sin" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math_sin expects number".into()),
            };
            Ok(Value::Number(n.sin()))
        },
        "math_cos" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math_cos expects number".into()),
            };
            Ok(Value::Number(n.cos()))
        },
        "socket_open" => |args| {
            let addr = match args.first() {
                Some(Value::String(s)) => s,
                _ => return Err("socket_open expects string address".into()),
            };
            let stream = TcpStream::connect(addr)
                .map_err(|e| format!("failed to connect: {e}"))?;
            Ok(Value::Socket(Arc::new(Mutex::new(stream))))
        },
        "socket_send" => |args| {
            let (socket, data) = match args.as_slice() {
                [Value::Socket(s), Value::String(d)] => (s, d),
                _ => return Err("socket_send expects (Socket, String)".into()),
            };
            socket.lock().unwrap().write_all(data.as_bytes())
                .map_err(|e| format!("failed to send: {e}"))?;
            Ok(Value::Bool(true))
        },
        "socket_recv" => |args| {
            let (socket, size) = match args.as_slice() {
                [Value::Socket(s), Value::Number(n)] => (s, *n as usize),
                _ => return Err("socket_recv expects (Socket, Number)".into()),
            };
            let mut buffer = vec![0u8; size];
            let n = socket.lock().unwrap().read(&mut buffer)
                .map_err(|e| format!("failed to recv: {e}"))?;
            Ok(Value::String(String::from_utf8_lossy(&buffer[..n]).into()))
        },
        "time_now" => |_| {
            let start = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64();
            Ok(Value::Number(start))
        },
        "cli_args" => |_| {
            let args: Vec<Value> = env::args().map(|s| Value::String(s)).collect();
            Ok(Value::List(args))
        },
        "fs_read" => |args| {
            let path = match args.first() {
                Some(Value::String(s)) => s,
                _ => return Err("fs_read expects string path".into()),
            };
            let content = fs::read_to_string(path)
                .map_err(|e| format!("failed to read file: {e}"))?;
            Ok(Value::String(content))
        },
        "fs_write" => |args| {
            let (path, content) = match args.as_slice() {
                [Value::String(p), Value::String(c)] => (p, c),
                _ => return Err("fs_write expects (path, content)".into()),
            };
            fs::write(path, content)
                .map_err(|e| format!("failed to write file: {e}"))?;
            Ok(Value::Bool(true))
        },
        "fs_exists" => |args| {
            let path = match args.first() {
                Some(Value::String(s)) => s,
                _ => return Err("fs_exists expects string path".into()),
            };
            Ok(Value::Bool(Path::new(path).exists()))
        },
        "fs_list_dir" => |args| {
            let path = match args.first() {
                Some(Value::String(s)) => s,
                _ => return Err("fs_list_dir expects string path".into()),
            };
            let entries = fs::read_dir(path)
                .map_err(|e| format!("failed to list dir: {e}"))?;
            let items: Vec<Value> = entries
                .filter_map(|e| e.ok())
                .map(|e| Value::String(e.file_name().to_string_lossy().into()))
                .collect();
            Ok(Value::List(items))
        },
        "fs_append" => |args| {
            let (path, content) = match args.as_slice() {
                [Value::String(p), Value::String(c)] => (p, c),
                _ => return Err("fs_append expects (path, content)".into()),
            };
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| format!("failed to open file: {e}"))?;
            file.write_all(content.as_bytes())
                .map_err(|e| format!("failed to append: {e}"))?;
            Ok(Value::Bool(true))
        },
        "fs_mkdir" => |args| {
            let path = match args.first() {
                Some(Value::String(s)) => s,
                _ => return Err("fs_mkdir expects string path".into()),
            };
            std::fs::create_dir_all(path)
                .map_err(|e| format!("failed to create dir: {e}"))?;
            Ok(Value::Bool(true))
        },
        "fs_remove" => |args| {
            let path = match args.first() {
                Some(Value::String(s)) => s,
                _ => return Err("fs_remove expects string path".into()),
            };
            let meta = std::fs::metadata(path)
                .map_err(|e| format!("failed to stat file: {e}"))?;
            if meta.is_dir() {
                std::fs::remove_dir_all(path)
                    .map_err(|e| format!("failed to remove dir: {e}"))?;
            } else {
                std::fs::remove_file(path)
                    .map_err(|e| format!("failed to remove file: {e}"))?;
            }
            Ok(Value::Bool(true))
        },
        "json_load" => |args| {
            let path = match args.first() {
                Some(Value::String(s)) => s,
                _ => return Err("json_load expects string path".into()),
            };
            let content = fs::read_to_string(path)
                .map_err(|e| format!("failed to read file: {e}"))?;
            json_decode(&content)
        },
        "json_save" => |args| {
            let (path, value) = match args.as_slice() {
                [Value::String(p), v] => (p.clone(), v.clone()),
                _ => return Err("json_save expects (path, value)".into()),
            };
            fs::write(&path, json_encode(&value, false))
                .map_err(|e| format!("failed to write file: {e}"))?;
            Ok(Value::Bool(true))
        },
        "json_encode" => |args| {
            let (value, pretty) = match args.as_slice() {
                [v] => (v, false),
                [v, Value::Dict(opts)] => {
                    let pretty = matches!(opts.get("pretty"), Some(Value::Bool(true)));
                    (v, pretty)
                }
                _ => return Err("json_encode expects a value".into()),
            };
            Ok(Value::String(json_encode(value, pretty)))
        },
        "json_decode" => |args| {
            let s = match args.first() {
                Some(Value::String(s)) => s,
                _ => return Err("json_decode expects a string".into()),
            };
            json_decode(s)
        },
        "regex_match" => |args| {
            let (pattern, text) = match args.as_slice() {
                [Value::String(p), Value::String(t)] => (p, t),
                _ => return Err("regex_match expects (pattern, text)".into()),
            };
            Ok(Value::Bool(regex_match(pattern, text)))
        },
        "regex_find" => |args| {
            let (pattern, text) = match args.as_slice() {
                [Value::String(p), Value::String(t)] => (p, t),
                _ => return Err("regex_find expects (pattern, text)".into()),
            };
            let results = regex_find_all(pattern, text);
            let items: Vec<Value> = results.into_iter().map(Value::String).collect();
            Ok(Value::List(items))
        },
        "regex_replace" => |args| {
            let (pattern, text, replacement) = match args.as_slice() {
                [Value::String(p), Value::String(t), Value::String(r)] => (p, t, r),
                _ => return Err("regex_replace expects (pattern, text, replacement)".into()),
            };
            Ok(Value::String(regex_replace(pattern, text, replacement)))
        },
        "http_get" => |args| http_request_impl(&args, "GET"),
        "http_head" => |args| http_request_impl(&args, "HEAD"),
        "http_del" => |args| http_request_impl(&args, "DELETE"),
        "http_post" => |args| http_request_impl(&args, "POST"),
        "http_put" => |args| http_request_impl(&args, "PUT"),
        "http_patch" => |args| http_request_impl(&args, "PATCH"),
        "__http_response_json" => |args| {
            let body = response_body_from_args(&args)?;
            json_decode(&body)
        },
        "__http_response_text" => |args| {
            let body = response_body_from_args(&args)?;
            Ok(Value::String(body))
        },
        "sha256_hex" => |args| {
            let data = match args.first() {
                Some(Value::String(s)) => s.as_bytes().to_vec(),
                _ => return Err("sha256_hex expects a string".into()),
            };
            Ok(Value::String(crate::pm::sha256_hex(&data)))
        },
        "random_random" => |_| {
            use rand::Rng;
            Ok(Value::Number(rand::rng().random::<f64>()))
        },
        "random_randint" => |args| {
            use rand::Rng;
            let (a, b) = match args.as_slice() {
                [Value::Number(a), Value::Number(b)] => (*a as i64, *b as i64),
                _ => return Err("random.randint expects (a, b)".into()),
            };
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            if lo == hi {
                return Ok(Value::Number(lo as f64));
            }
            Ok(Value::Number(rand::rng().random_range(lo..=hi) as f64))
        },
        "random_choice" => |args| {
            use rand::Rng;
            let items = match args.first() {
                Some(Value::List(items)) if !items.is_empty() => items,
                _ => return Err("random.choice expects a non-empty list".into()),
            };
            let idx = rand::rng().random_range(0..items.len());
            Ok(items[idx].clone())
        },
        "random_shuffle" | "shuffle" => |args| {
            use rand::seq::SliceRandom;
            let items = match args.first() {
                Some(Value::List(items)) => items.clone(),
                _ => return Err("random.shuffle expects a list".into()),
            };
            let mut items = items;
            items.shuffle(&mut rand::rng());
            Ok(Value::List(items))
        },
        "random_seed" => |_args| Ok(Value::Null),
        "random_randrange" => |args| {
            use rand::Rng;
            let (start, stop, step) = match args.as_slice() {
                [Value::Number(stop)] => (0i64, *stop as i64, 1i64),
                [Value::Number(start), Value::Number(stop)] => (*start as i64, *stop as i64, 1i64),
                [Value::Number(start), Value::Number(stop), Value::Number(step)] => {
                    (*start as i64, *stop as i64, *step as i64)
                }
                _ => return Err("random.randrange expects (stop) or (start, stop) or (start, stop, step)".into()),
            };
            if step == 0 {
                return Err("random.randrange step must be non-zero".into());
            }
            let count = ((stop - start) as f64 / step as f64).ceil().max(0.0) as u64;
            if count == 0 {
                return Err("random.randrange empty range".into());
            }
            let idx = rand::rng().random_range(0..count);
            Ok(Value::Number((start + idx as i64 * step) as f64))
        },
        "random_choices" => |args| {
            use rand::Rng;
            let (items, k) = match args.as_slice() {
                [Value::List(items)] => (items, 1usize),
                [Value::List(items), Value::Number(k)] => (items, *k as usize),
                _ => return Err("random.choices expects (sequence, k?)".into()),
            };
            if items.is_empty() {
                return Err("random.choices expects non-empty sequence".into());
            }
            let mut rng = rand::rng();
            let result: Vec<Value> = (0..k)
                .map(|_| items[rng.random_range(0..items.len())].clone())
                .collect();
            Ok(Value::List(result))
        },
        "random_sample" => |args| {
            use rand::seq::SliceRandom;
            let (items, k) = match args.as_slice() {
                [Value::List(items), Value::Number(k)] => (items, *k as usize),
                _ => return Err("random.sample expects (sequence, k)".into()),
            };
            let mut pool = items.clone();
            pool.shuffle(&mut rand::rng());
            pool.truncate(k);
            Ok(Value::List(pool))
        },
        "random_uniform" => |args| {
            use rand::Rng;
            let (a, b) = match args.as_slice() {
                [Value::Number(a), Value::Number(b)] => (*a, *b),
                _ => return Err("random.uniform expects (a, b)".into()),
            };
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            Ok(Value::Number(rand::rng().random_range(lo..=hi)))
        },
        "random_hex" => |args| {
            use rand::Rng;
            let k = match args.first() {
                Some(Value::Number(n)) => *n as usize,
                _ => 16,
            };
            const HEX: &[u8] = b"0123456789abcdef";
            let mut rng = rand::rng();
            let result: String = (0..k)
                .map(|_| HEX[rng.random_range(0..HEX.len())] as char)
                .collect();
            Ok(Value::String(result))
        },
        "math_sqrt" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.sqrt expects number".into()),
            };
            Ok(Value::Number(n.sqrt()))
        },
        "math_abs" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.abs expects number".into()),
            };
            Ok(Value::Number(n.abs()))
        },
        "math_floor" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.floor expects number".into()),
            };
            Ok(Value::Number(n.floor()))
        },
        "math_ceil" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.ceil expects number".into()),
            };
            Ok(Value::Number(n.ceil()))
        },
        "math_round" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.round expects number".into()),
            };
            Ok(Value::Number(n.round()))
        },
        "math_pow" => |args| {
            let (a, b) = match args.as_slice() {
                [Value::Number(a), Value::Number(b)] => (a, b),
                _ => return Err("math.pow expects (base, exp)".into()),
            };
            Ok(Value::Number(a.powf(*b)))
        },
        "math_min" => |args| {
            let mut nums: Vec<f64> = Vec::new();
            for v in args {
                match v {
                    Value::Number(n) => nums.push(n),
                    Value::List(items) => {
                        for item in items {
                            if let Value::Number(n) = item {
                                nums.push(n);
                            }
                        }
                    }
                    _ => {}
                }
            }
            nums.into_iter()
                .reduce(f64::min)
                .map(Value::Number)
                .ok_or("min expects at least one number".into())
        },
        "math_max" => |args| {
            let mut nums: Vec<f64> = Vec::new();
            for v in args {
                match v {
                    Value::Number(n) => nums.push(n),
                    Value::List(items) => {
                        for item in items {
                            if let Value::Number(n) = item {
                                nums.push(n);
                            }
                        }
                    }
                    _ => {}
                }
            }
            nums.into_iter()
                .reduce(f64::max)
                .map(Value::Number)
                .ok_or("max expects at least one number".into())
        },
        "math_trunc" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.trunc expects number".into()),
            };
            Ok(Value::Number(n.trunc()))
        },
        "math_exp" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.exp expects number".into()),
            };
            Ok(Value::Number(n.exp()))
        },
        "math_log" => |args| {
            let (x, base) = match args.as_slice() {
                [Value::Number(x)] => (*x, std::f64::consts::E),
                [Value::Number(x), Value::Number(b)] => (*x, *b),
                _ => return Err("math.log expects (x, base?)".into()),
            };
            Ok(Value::Number(x.log(base)))
        },
        "math_log2" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.log2 expects number".into()),
            };
            Ok(Value::Number(n.log2()))
        },
        "math_log10" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.log10 expects number".into()),
            };
            Ok(Value::Number(n.log10()))
        },
        "math_tan" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.tan expects number".into()),
            };
            Ok(Value::Number(n.tan()))
        },
        "math_asin" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.asin expects number".into()),
            };
            Ok(Value::Number(n.asin()))
        },
        "math_acos" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.acos expects number".into()),
            };
            Ok(Value::Number(n.acos()))
        },
        "math_atan" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.atan expects number".into()),
            };
            Ok(Value::Number(n.atan()))
        },
        "math_atan2" => |args| {
            let (y, x) = match args.as_slice() {
                [Value::Number(y), Value::Number(x)] => (y, x),
                _ => return Err("math.atan2 expects (y, x)".into()),
            };
            Ok(Value::Number(y.atan2(*x)))
        },
        "math_degrees" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.degrees expects number".into()),
            };
            Ok(Value::Number(n.to_degrees()))
        },
        "math_radians" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.radians expects number".into()),
            };
            Ok(Value::Number(n.to_radians()))
        },
        "math_hypot" => |args| {
            let values = numbers_from_args(args.clone())?;
            Ok(Value::Number(values.iter().map(|v| v * v).sum::<f64>().sqrt()))
        },
        "math_isnan" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.isnan expects number".into()),
            };
            Ok(Value::Bool(n.is_nan()))
        },
        "math_isfinite" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.isfinite expects number".into()),
            };
            Ok(Value::Bool(n.is_finite()))
        },
        "math_isinf" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.isinf expects number".into()),
            };
            Ok(Value::Bool(n.is_infinite()))
        },
        "math_copysign" => |args| {
            let (x, y) = match args.as_slice() {
                [Value::Number(x), Value::Number(y)] => (x, y),
                _ => return Err("math.copysign expects (x, y)".into()),
            };
            Ok(Value::Number(x.copysign(*y)))
        },
        "math_gcd" => |args| {
            let (a, b) = match args.as_slice() {
                [Value::Number(a), Value::Number(b)] => (*a as i64, *b as i64),
                _ => return Err("math.gcd expects (a, b)".into()),
            };
            Ok(Value::Number(gcd(a.abs(), b.abs()) as f64))
        },
        "math_lcm" => |args| {
            let (a, b) = match args.as_slice() {
                [Value::Number(a), Value::Number(b)] => (*a as i64, *b as i64),
                _ => return Err("math.lcm expects (a, b)".into()),
            };
            if a == 0 || b == 0 {
                return Ok(Value::Number(0.0));
            }
            Ok(Value::Number((a.abs() / gcd(a.abs(), b.abs()) * b.abs()) as f64))
        },
        "math_factorial" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => *n as u64,
                _ => return Err("math.factorial expects a number".into()),
            };
            Ok(Value::Number(factorial(n) as f64))
        },
        "math_comb" => |args| {
            let (n, k) = match args.as_slice() {
                [Value::Number(n), Value::Number(k)] => (*n as u64, *k as u64),
                _ => return Err("math.comb expects (n, k)".into()),
            };
            Ok(Value::Number(comb(n, k) as f64))
        },
        "math_perm" => |args| {
            let (n, k) = match args.as_slice() {
                [Value::Number(n)] => (*n as u64, *n as u64),
                [Value::Number(n), Value::Number(k)] => (*n as u64, *k as u64),
                _ => return Err("math.perm expects (n, k?)".into()),
            };
            Ok(Value::Number(perm(n, k) as f64))
        },
        "math_remainder" => |args| {
            let (x, y) = match args.as_slice() {
                [Value::Number(x), Value::Number(y)] => (x, y),
                _ => return Err("math.remainder expects (x, y)".into()),
            };
            Ok(Value::Number(x - (x / y).round() * y))
        },
        "math_fsum" => |args| {
            let values = numbers_from_args(args.clone())?;
            Ok(Value::Number(values.iter().sum::<f64>()))
        },
        "math_prod" => |args| {
            let values = numbers_from_args(args.clone())?;
            Ok(Value::Number(values.iter().product::<f64>()))
        },
        "math_modf" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.modf expects number".into()),
            };
            Ok(Value::List(vec![Value::Number(n.fract()), Value::Number(n.trunc())]))
        },
        "math_frexp" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.frexp expects number".into()),
            };
            if *n == 0.0 {
                return Ok(Value::List(vec![Value::Number(0.0), Value::Number(0.0)]));
            }
            let exponent = n.abs().log2().floor() as i32 + 1;
            let mantissa = n / 2f64.powi(exponent);
            Ok(Value::List(vec![Value::Number(mantissa), Value::Number(exponent as f64)]))
        },
        "math_ldexp" => |args| {
            let (x, exp) = match args.as_slice() {
                [Value::Number(x), Value::Number(e)] => (*x, *e as i32),
                _ => return Err("math.ldexp expects (x, exp)".into()),
            };
            Ok(Value::Number(x * 2f64.powi(exp)))
        },
        "b64_encode" => |args| {
            let data = match args.first() {
                Some(Value::String(s)) => s.as_bytes(),
                _ => return Err("base64.encode expects a string".into()),
            };
            use base64::Engine;
            Ok(Value::String(base64::engine::general_purpose::STANDARD.encode(data)))
        },
        "b64_decode" => |args| {
            let data = match args.first() {
                Some(Value::String(s)) => s.as_str(),
                _ => return Err("base64.decode expects a string".into()),
            };
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|e| format!("invalid base64: {e}"))?;
            Ok(Value::String(String::from_utf8_lossy(&bytes).into_owned()))
        },
        "b64_url_encode" => |args| {
            let data = match args.first() {
                Some(Value::String(s)) => s.as_bytes(),
                _ => return Err("base64.url_encode expects a string".into()),
            };
            use base64::Engine;
            Ok(Value::String(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)))
        },
        "b64_url_decode" => |args| {
            let data = match args.first() {
                Some(Value::String(s)) => s.as_str(),
                _ => return Err("base64.url_decode expects a string".into()),
            };
            use base64::Engine;
            let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(data)
                .map_err(|e| format!("invalid base64: {e}"))?;
            Ok(Value::String(String::from_utf8_lossy(&bytes).into_owned()))
        },
        "b32_encode" => |args| {
            let data = match args.first() {
                Some(Value::String(s)) => s.as_bytes(),
                _ => return Err("base32.encode expects a string".into()),
            };
            Ok(Value::String(base32::encode(base32::Alphabet::Rfc4648 { padding: false }, data)))
        },
        "b32_decode" => |args| {
            let data = match args.first() {
                Some(Value::String(s)) => s.as_str(),
                _ => return Err("base32.decode expects a string".into()),
            };
            match base32::decode(base32::Alphabet::Rfc4648 { padding: false }, data) {
                Some(bytes) => Ok(Value::String(String::from_utf8_lossy(&bytes).into_owned())),
                None => Err("invalid base32 data".into()),
            }
        },
        "os_setenv" => |args| {
            let (key, value) = match args.as_slice() {
                [Value::String(k), Value::String(v)] => (k, v),
                _ => return Err("os.setenv expects (key, value)".into()),
            };
            env::set_var(key, value);
            Ok(Value::Null)
        },
        "os_unsetenv" => |args| {
            let key = match args.first() {
                Some(Value::String(k)) => k,
                _ => return Err("os.unsetenv expects a string key".into()),
            };
            env::remove_var(key);
            Ok(Value::Null)
        },
        "os_platform" => |_| Ok(Value::String(std::env::consts::OS.into())),
        "os_hostname" => |_| {
            Ok(Value::String(
                fs::read_to_string("/etc/hostname")
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|_| "localhost".into()),
            ))
        },
        "os_pid" => |_| Ok(Value::Number(process::id() as f64)),
        "os_cpu_count" => |_| {
            Ok(Value::Number(
                std::thread::available_parallelism()
                    .map(|n| n.get() as f64)
                    .unwrap_or(1.0),
            ))
        },
        "os_system" => |args| {
            let cmd = match args.first() {
                Some(Value::String(c)) => c,
                _ => return Err("os.system expects a command string".into()),
            };
            let status = process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .status()
                .map_err(|e| format!("os.system failed: {e}"))?;
            Ok(Value::Number(status.code().unwrap_or(-1) as f64))
        },
        "time_unix" => |_| {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
            Ok(Value::Number(now.as_secs_f64()))
        },
        "time_utc" => |_| {
            let now = SystemTime::now();
            let datetime: chrono::DateTime<chrono::Utc> = now.into();
            Ok(Value::String(datetime.to_rfc3339().replace("+00:00", "Z")))
        },
        "time_date" => |_| {
            let now = SystemTime::now();
            let datetime: chrono::DateTime<chrono::Utc> = now.into();
            Ok(Value::String(datetime.format("%Y-%m-%d").to_string()))
        },
        "time_format" => |args| {
            let fmt = match args.first() {
                Some(Value::String(f)) => f,
                _ => return Err("time.format expects a format string".into()),
            };
            let now = SystemTime::now();
            let datetime: chrono::DateTime<chrono::Local> = now.into();
            Ok(Value::String(datetime.format(fmt).to_string()))
        },
        "time_parse" => |args| {
            let (s, fmt) = match args.as_slice() {
                [Value::String(s), Value::String(f)] => (s, f),
                _ => return Err("time.parse expects (text, format)".into()),
            };
            match chrono::NaiveDateTime::parse_from_str(s, fmt) {
                Ok(parsed) => Ok(Value::String(parsed.format("%Y-%m-%dT%H:%M:%S").to_string())),
                Err(_) => match chrono::NaiveDate::parse_from_str(s, fmt) {
                    Ok(date) => {
                        let dt = date.and_hms_opt(0, 0, 0).unwrap();
                        Ok(Value::String(dt.format("%Y-%m-%dT%H:%M:%S").to_string()))
                    }
                    Err(e) => Err(format!("time.parse failed: {e}")),
                },
            }
        },
        "time_from_unix" => |args| {
            let ts = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("time.from_unix expects a timestamp".into()),
            };
            let secs = *ts as i64;
            let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)
                .ok_or("invalid unix timestamp")?;
            Ok(Value::String(datetime.to_rfc3339().replace("+00:00", "Z")))
        },
        "time_sleep" => |args| {
            let secs = match args.first() {
                Some(Value::Number(n)) => *n,
                _ => return Err("time.sleep expects a number of seconds".into()),
            };
            std::thread::sleep(std::time::Duration::from_secs_f64(secs));
            Ok(Value::Null)
        },
        "time_wait" => |args| {
            let ms = match args.first() {
                Some(Value::Number(n)) => *n,
                _ => return Err("time.wait expects milliseconds".into()),
            };
            std::thread::sleep(std::time::Duration::from_secs_f64(ms / 1000.0));
            Ok(Value::Null)
        },
        "time_year" => |_| time_part_impl(|dt| dt.year() as f64),
        "time_month" => |_| time_part_impl(|dt| dt.month() as f64),
        "time_day" => |_| time_part_impl(|dt| dt.day() as f64),
        "time_hour" => |_| time_part_impl(|dt| dt.hour() as f64),
        "time_minute" => |_| time_part_impl(|dt| dt.minute() as f64),
        "time_second" => |_| time_part_impl(|dt| dt.second() as f64),
        "time_weekday" => |_| {
            let now = chrono::Local::now();
            Ok(Value::Number(now.weekday().num_days_from_monday() as f64))
        },
        "time_add_days" => |args| {
            let (date_str, days) = match args.as_slice() {
                [Value::String(d), Value::Number(n)] => (d, *n as i64),
                _ => return Err("time.add_days expects (date_string, days)".into()),
            };
            let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .map_err(|e| format!("time.add_days invalid date: {e}"))?;
            let new_date = date + chrono::Days::new(days.max(0) as u64);
            Ok(Value::String(new_date.format("%Y-%m-%d").to_string()))
        },
        "os_getenv" => |args| {
            let key = match args.first() {
                Some(Value::String(s)) => s,
                _ => return Err("os.getenv expects a string key".into()),
            };
            match env::var(key) {
                Ok(value) => Ok(Value::String(value)),
                Err(_) => Ok(Value::Null),
            }
        },
        "os_home" => |_| {
            match env::var("HOME") {
                Ok(value) => Ok(Value::String(value)),
                Err(_) => Ok(Value::Null),
            }
        },
        "os_cwd" => |_| {
            let cwd = env::current_dir().map_err(|e| e.to_string())?;
            Ok(Value::String(cwd.to_string_lossy().into_owned()))
        },
        "crypto_sha256" => |args| hash_hex::<sha2::Sha256>(args),
        "crypto_sha1" => |args| hash_hex::<sha1::Sha1>(args),
        "crypto_md5" => |args| hash_hex::<md5::Md5>(args),
        "crypto_sha512" => |args| hash_hex::<sha2::Sha512>(args),
        "crypto_sha224" => |args| hash_hex::<sha2::Sha224>(args),
        "crypto_sha384" => |args| hash_hex::<sha2::Sha384>(args),
        "crypto_sha3_256" => |args| hash_hex::<sha3::Sha3_256>(args),
        "crypto_sha3_512" => |args| hash_hex::<sha3::Sha3_512>(args),
        "crypto_blake2b" => |args| hash_hex::<blake2::Blake2b512>(args),
        "crypto_blake2s" => |args| hash_hex::<blake2::Blake2s256>(args),
        "crypto_hmac_sha256" => |args| hmac_hex_sha256(args),
        "crypto_hmac_sha1" => |args| hmac_hex_sha1(args),
        "crypto_hmac_md5" => |args| hmac_hex_md5(args),
        "crypto_random_bytes" | "crypto_random_hex" => |args| {
            use rand::RngCore;
            let n = match args.first() {
                Some(Value::Number(n)) => *n as usize,
                _ => return Err("crypto.random_bytes expects a length".into()),
            };
            let mut bytes = vec![0u8; n];
            rand::rng().fill_bytes(&mut bytes);
            let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            Ok(Value::String(hex))
        },
        "crypto_pbkdf2" => |args| {
            let (password, salt, iterations, dklen) = match args.as_slice() {
                [Value::String(p), Value::String(s)] => (p, s, 100000u32, 32usize),
                [Value::String(p), Value::String(s), Value::Number(i)] => {
                    (p, s, *i as u32, 32usize)
                }
                [Value::String(p), Value::String(s), Value::Number(i), Value::Number(d)] => {
                    (p, s, *i as u32, *d as usize)
                }
                _ => {
                    return Err(
                        "crypto.pbkdf2 expects (password, salt, iterations?, dklen?)".into()
                    )
                }
            };
            let mut derived = [0u8; 64];
            let result = pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
                password.as_bytes(),
                salt.as_bytes(),
                iterations,
                &mut derived,
            );
            let _ = result;
            let hex: String = derived[..dklen.min(derived.len())]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            Ok(Value::String(hex))
        },
        "crypto_aes_encrypt" => |args| {
            let (key, data, iv) = match args.as_slice() {
                [Value::String(k), Value::String(d)] => (k, d, None),
                [Value::String(k), Value::String(d), Value::String(iv)] => (k, d, Some(iv.as_str())),
                _ => return Err("crypto.aes_encrypt expects (key, data, iv?)".into()),
            };
            Ok(Value::String(aes_encrypt(key, data, iv)?))
        },
        "crypto_aes_decrypt" => |args| {
            let (key, data, iv) = match args.as_slice() {
                [Value::String(k), Value::String(d)] => (k, d, None),
                [Value::String(k), Value::String(d), Value::String(iv)] => (k, d, Some(iv.as_str())),
                _ => return Err("crypto.aes_decrypt expects (key, data, iv?)".into()),
            };
            Ok(Value::String(aes_decrypt(key, data, iv)?))
        },
        "statistics_mean" => |args| {
            let values = numbers_from_args(args)?;
            if values.is_empty() {
                return Err("statistics.mean expects at least one number".into());
            }
            Ok(Value::Number(values.iter().sum::<f64>() / values.len() as f64))
        },
        "statistics_median" => |args| {
            let mut values = numbers_from_args(args)?;
            if values.is_empty() {
                return Err("statistics.median expects at least one number".into());
            }
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let n = values.len();
            let median = if n % 2 == 1 {
                values[n / 2]
            } else {
                (values[n / 2 - 1] + values[n / 2]) / 2.0
            };
            Ok(Value::Number(median))
        },
        "statistics_mode" => |args| {
            let values = numbers_from_args(args)?;
            if values.is_empty() {
                return Err("statistics.mode expects at least one number".into());
            }
            let mut best = values[0];
            let mut best_count = 0;
            for candidate in &values {
                let count = values.iter().filter(|v| **v == *candidate).count();
                if count > best_count {
                    best_count = count;
                    best = *candidate;
                }
            }
            Ok(Value::Number(best))
        },
        "statistics_variance" | "statistics_pvariance" => |args| {
            let values = numbers_from_args(args)?;
            if values.len() < 2 {
                return Err("statistics.variance expects at least two numbers".into());
            }
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            let variance = values
                .iter()
                .map(|v| (v - mean) * (v - mean))
                .sum::<f64>()
                / values.len() as f64;
            Ok(Value::Number(variance))
        },
        "statistics_stdev" => |args| {
            let values = numbers_from_args(args)?;
            if values.len() < 2 {
                return Err("statistics.stdev expects at least two numbers".into());
            }
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            let variance = values
                .iter()
                .map(|v| (v - mean) * (v - mean))
                .sum::<f64>()
                / values.len() as f64;
            Ok(Value::Number(variance.sqrt()))
        },
        "statistics_sum" => |args| {
            let values = numbers_from_args(args)?;
            Ok(Value::Number(values.iter().sum::<f64>()))
        },
        "regex_search" => |args| {
            let (pattern, text) = match args.as_slice() {
                [Value::String(p), Value::String(t)] => (p, t),
                _ => return Err("re.search expects (pattern, text)".into()),
            };
            let re = regex::Regex::new(pattern)
                .map_err(|e| format!("invalid regex: {e}"))?;
            let found = re.find(text);
            Ok(match found {
                Some(m) => Value::String(m.as_str().to_string()),
                None => Value::Null,
            })
        },
        "regex_split" => |args| {
            let (pattern, text) = match args.as_slice() {
                [Value::String(p), Value::String(t)] => (p, t),
                _ => return Err("re.split expects (pattern, text)".into()),
            };
            let re = regex::Regex::new(pattern)
                .map_err(|e| format!("invalid regex: {e}"))?;
            let parts: Vec<Value> = re.split(text).map(|s| Value::String(s.to_string())).collect();
            Ok(Value::List(parts))
        },
        "fs_read_binary" => |args| {
            let path = match args.first() {
                Some(Value::String(p)) => p,
                _ => return Err("fs.read_binary expects a path".into()),
            };
            let data = fs::read(path).map_err(|e| format!("fs.read_binary {path}: {e}"))?;
            Ok(Value::String(
                data.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            ))
        },
        "fs_write_binary" => |args| {
            let (path, content) = match args.as_slice() {
                [Value::String(p), Value::String(c)] => (p, c),
                _ => return Err("fs.write_binary expects (path, hex_string)".into()),
            };
            let bytes = hex_decode(content).ok_or("fs.write_binary: invalid hex data")?;
            fs::write(path, &bytes).map_err(|e| format!("fs.write_binary {path}: {e}"))?;
            Ok(Value::Bool(true))
        },
        "fs_is_file" => |args| {
            let path = match args.first() {
                Some(Value::String(p)) => p,
                _ => return Err("fs.is_file expects a path".into()),
            };
            Ok(Value::Bool(Path::new(path).is_file()))
        },
        "fs_is_dir" => |args| {
            let path = match args.first() {
                Some(Value::String(p)) => p,
                _ => return Err("fs.is_dir expects a path".into()),
            };
            Ok(Value::Bool(Path::new(path).is_dir()))
        },
        "fs_size" => |args| {
            let path = match args.first() {
                Some(Value::String(p)) => p,
                _ => return Err("fs.size expects a path".into()),
            };
            let meta = fs::metadata(path).map_err(|e| format!("fs.size {path}: {e}"))?;
            Ok(Value::Number(meta.len() as f64))
        },
        "fs_mtime" => |args| {
            let path = match args.first() {
                Some(Value::String(p)) => p,
                _ => return Err("fs.mtime expects a path".into()),
            };
            let meta = fs::metadata(path).map_err(|e| format!("fs.mtime {path}: {e}"))?;
            let modified = meta
                .modified()
                .map_err(|e| format!("fs.mtime {path}: {e}"))?;
            let secs = modified
                .duration_since(UNIX_EPOCH)
                .map_err(|e| format!("fs.mtime {path}: {e}"))?
                .as_secs() as f64;
            Ok(Value::Number(secs))
        },
        "fs_rmdir" => |args| {
            let path = match args.first() {
                Some(Value::String(p)) => p,
                _ => return Err("fs.rmdir expects a path".into()),
            };
            fs::remove_dir(path).map_err(|e| format!("fs.rmdir {path}: {e}"))?;
            Ok(Value::Bool(true))
        },
        "fs_rmtree" => |args| {
            let path = match args.first() {
                Some(Value::String(p)) => p,
                _ => return Err("fs.rmtree expects a path".into()),
            };
            fs::remove_dir_all(path).map_err(|e| format!("fs.rmtree {path}: {e}"))?;
            Ok(Value::Bool(true))
        },
        "fs_copy" => |args| {
            let (src, dst) = match args.as_slice() {
                [Value::String(s), Value::String(d)] => (s, d),
                _ => return Err("fs.copy expects (src, dst)".into()),
            };
            fs::copy(src, dst).map_err(|e| format!("fs.copy {src} -> {dst}: {e}"))?;
            Ok(Value::Bool(true))
        },
        "fs_move" => |args| {
            let (src, dst) = match args.as_slice() {
                [Value::String(s), Value::String(d)] => (s, d),
                _ => return Err("fs.move expects (src, dst)".into()),
            };
            fs::rename(src, dst).map_err(|e| format!("fs.move {src} -> {dst}: {e}"))?;
            Ok(Value::Bool(true))
        },
        "fs_glob" => |args| {
            let pattern = match args.first() {
                Some(Value::String(p)) => p,
                _ => return Err("fs.glob expects a glob pattern".into()),
            };
            let path = Path::new(pattern);
            let dir = path.parent().and_then(|p| p.to_str()).unwrap_or(".");
            let name = path.file_name().and_then(|f| f.to_str()).unwrap_or("*");
            let mut results = Vec::new();
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let file_name = entry.file_name().to_string_lossy().into_owned();
                    if simple_glob(name, &file_name) {
                        results.push(Value::String(
                            if dir == "." {
                                file_name
                            } else {
                                format!("{dir}/{file_name}")
                            },
                        ));
                    }
                }
            }
            Ok(Value::List(results))
        },
        "fs_join" => |args| {
            let parts: Vec<String> = args
                .iter()
                .map(|v| v.to_string())
                .collect();
            let joined = parts.join(std::path::MAIN_SEPARATOR.to_string().as_str());
            Ok(Value::String(joined))
        },
        "fs_basename" => |args| {
            let path = match args.first() {
                Some(Value::String(p)) => p,
                _ => return Err("fs.basename expects a path".into()),
            };
            Ok(Value::String(
                Path::new(path)
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ))
        },
        "fs_dirname" => |args| {
            let path = match args.first() {
                Some(Value::String(p)) => p,
                _ => return Err("fs.dirname expects a path".into()),
            };
            Ok(Value::String(
                Path::new(path)
                    .parent()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ))
        },
        "fs_cd" => |args| {
            let path = match args.first() {
                Some(Value::String(p)) => p,
                _ => return Err("fs.cd expects a path".into()),
            };
            env::set_current_dir(path).map_err(|e| format!("fs.cd {path}: {e}"))?;
            Ok(Value::Bool(true))
        },
        "fernet_generate_key" => |_| {
            use rand::RngCore;
            let mut bytes = [0u8; 32];
            rand::rng().fill_bytes(&mut bytes);
            use base64::Engine;
            Ok(Value::String(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)))
        },
        "fernet_encrypt" => |args| {
            let (key, data) = match args.as_slice() {
                [Value::String(k), Value::String(d)] => (k, d),
                _ => return Err("cryptography.fernet.encrypt expects (key, data)".into()),
            };
            fernet_encrypt_impl(key, data)
        },
        "fernet_decrypt" => |args| {
            let (key, token) = match args.as_slice() {
                [Value::String(k), Value::String(t)] => (k, t),
                _ => return Err("cryptography.fernet.decrypt expects (key, token)".into()),
            };
            fernet_decrypt_impl(key, token)
        },
        "uuid_uuid4" => |_| Ok(Value::String(uuid::Uuid::new_v4().to_string())),
        "uuid_uuid1" => |_| {
            let uuid = uuid::Uuid::now_v1(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00]);
            Ok(Value::String(uuid.to_string()))
        },
        "uuid_uuid3" => |args| {
            let (ns, name) = match args.as_slice() {
                [Value::String(n), Value::String(m)] => (n, m),
                _ => return Err("uuid.uuid3 expects (namespace, name)".into()),
            };
            let namespace = uuid_namespace(ns);
            Ok(Value::String(uuid::Uuid::new_v3(&namespace, name.as_bytes()).to_string()))
        },
        "uuid_uuid5" => |args| {
            let (ns, name) = match args.as_slice() {
                [Value::String(n), Value::String(m)] => (n, m),
                _ => return Err("uuid.uuid5 expects (namespace, name)".into()),
            };
            let namespace = uuid_namespace(ns);
            Ok(Value::String(uuid::Uuid::new_v5(&namespace, name.as_bytes()).to_string()))
        },
        "color_style_bold" => |args| color_style(1, args),
        "color_style_dim" => |args| color_style(2, args),
        "color_style_italic" => |args| color_style(3, args),
        "color_style_underline" => |args| color_style(4, args),
        "color_style_blink" => |args| color_style(5, args),
        "color_style_reverse" => |args| color_style(7, args),
        "color_style_hidden" => |args| color_style(8, args),
        "color_style_strike" => |args| color_style(9, args),
        "color_rgb" => |args| color_rgb_fg(args),
        "color_bg_rgb" => |args| color_rgb_bg(args),
        "color_hex" => |args| color_hex_impl(args),
        "color_strip" => |args| {
            let text = match args.first() {
                Some(Value::String(s)) => s,
                _ => return Err("color.strip expects a string".into()),
            };
            let re = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
            Ok(Value::String(re.replace_all(text, "").into_owned()))
        },
        "color_fg_black" => |args| color_named(30, args),
        "color_fg_red" => |args| color_named(31, args),
        "color_fg_green" => |args| color_named(32, args),
        "color_fg_yellow" => |args| color_named(33, args),
        "color_fg_blue" => |args| color_named(34, args),
        "color_fg_magenta" => |args| color_named(35, args),
        "color_fg_cyan" => |args| color_named(36, args),
        "color_fg_white" => |args| color_named(37, args),
        "color_bg_black" => |args| color_named(40, args),
        "color_bg_red" => |args| color_named(41, args),
        "color_bg_green" => |args| color_named(42, args),
        "color_bg_yellow" => |args| color_named(43, args),
        "color_bg_blue" => |args| color_named(44, args),
        "color_bg_magenta" => |args| color_named(45, args),
        "color_bg_cyan" => |args| color_named(46, args),
        "color_bg_white" => |args| color_named(47, args),
        "color_fg_bright_black" => |args| color_named(90, args),
        "color_fg_bright_red" => |args| color_named(91, args),
        "color_fg_bright_green" => |args| color_named(92, args),
        "color_fg_bright_yellow" => |args| color_named(93, args),
        "color_fg_bright_blue" => |args| color_named(94, args),
        "color_fg_bright_magenta" => |args| color_named(95, args),
        "color_fg_bright_cyan" => |args| color_named(96, args),
        "color_fg_bright_white" => |args| color_named(97, args),
        "csv_read" => |args| {
            let path = match args.first() {
                Some(Value::String(p)) => p,
                _ => return Err("csv.read expects a path".into()),
            };
            let text = fs::read_to_string(path).map_err(|e| format!("csv.read {path}: {e}"))?;
            Ok(csv_parse_impl(&text))
        },
        "csv_parse" => |args| {
            let text = match args.first() {
                Some(Value::String(t)) => t,
                _ => return Err("csv.parse expects a string".into()),
            };
            Ok(csv_parse_impl(text))
        },
        "csv_write" => |args| {
            let (path, rows, headers) = match args.as_slice() {
                [Value::String(path), Value::List(rows)] => (path, rows, None),
                [Value::String(path), Value::List(rows), Value::List(headers)] => {
                    (path, rows, Some(headers))
                }
                _ => return Err("csv.write expects (path, rows, headers?)".into()),
            };
            let encoded = csv_encode_impl(rows, headers);
            fs::write(path, encoded).map_err(|e| format!("csv.write {path}: {e}"))?;
            Ok(Value::Bool(true))
        },
        "csv_encode" => |args| {
            let (rows, headers) = match args.as_slice() {
                [Value::List(rows)] => (rows, None),
                [Value::List(rows), Value::List(headers)] => (rows, Some(headers)),
                _ => return Err("csv.encode expects (rows, headers?)".into()),
            };
            Ok(Value::String(csv_encode_impl(rows, headers)))
        },
        "decimal_decimal" => |args| {
            let v = match args.first() {
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::String(s)) => s.clone(),
                _ => return Err("decimal.Decimal expects a number".into()),
            };
            Ok(Value::Dict(BTreeMap::from([
                ("value".into(), Value::String(v.clone())),
                ("__repr__".into(), Value::String(format!("Decimal({v})"))),
            ])))
        },
        "decimal_getcontext" => |_| {
            Ok(Value::Dict(BTreeMap::from([
                ("prec".into(), Value::Number(28.0)),
                ("rounding".into(), Value::String("ROUND_HALF_EVEN".into())),
            ])))
        },
        "decimal_setcontext" | "decimal_localcontext" => |_| Ok(Value::Null),
        "threading_start" => |args| {
            let name = match args.first() {
                Some(Value::Function(f)) | Some(Value::NativeFunction(f)) => f.clone(),
                _ => return Err("threading.start expects a function".into()),
            };
            let name_clone = name.clone();
            std::thread::spawn(move || {
                let mut vm = Vm::new();
                // Seed the thread VM with the function body from the registry.
                if let Ok(registry) = function_registry().lock() {
                    if let Some(function) = registry.get(&name_clone) {
                        vm.functions.insert(name_clone.clone(), function.clone());
                    }
                }
                let _ = vm.call(&name_clone, Vec::new());
            });
            Ok(Value::Dict(BTreeMap::from([
                ("name".into(), Value::String(format!("Thread-{name}"))),
                ("daemon".into(), Value::Bool(true)),
            ])))
        },
        "browser_launch" => |args| crate::state::browser_launch(&args),
        "browser_connect" => |_| crate::state::browser_connect(),
        "browser_navigate" => |args| crate::state::browser_navigate(&args),
        "browser_evaluate" => |args| crate::state::browser_evaluate(&args),
        "browser_capture_screenshot" => |args| crate::state::browser_screenshot(&args),
        "browser_get_html" => |_| crate::state::browser_get_html(),
        "browser_get_title" => |_| crate::state::browser_get_title(),
        "browser_get_url" => |_| crate::state::browser_get_url(),
        "browser_get_text" => |args| crate::state::browser_get_text(&args),
        "browser_click" => |args| crate::state::browser_click(&args),
        "browser_fill" => |args| crate::state::browser_fill(&args),
        "browser_query" => |args| crate::state::browser_query(&args),
        "browser_wait_for" => |args| crate::state::browser_wait_for(&args),
        "browser_close" => |_| crate::state::browser_close(),
        "socket_close" => |args| {
            let socket = match args.first() {
                Some(Value::Socket(s)) => s,
                _ => return Err("socket.close expects a Socket".into()),
            };
            socket
                .lock()
                .unwrap()
                .shutdown(std::net::Shutdown::Both)
                .ok();
            Ok(Value::Bool(true))
        },
        "ftp_connect" => |args| {
            let host = arg_string(&args, 0)?;
            let port = match args.get(1) {
                Some(Value::Number(n)) => *n as u16,
                _ => 21,
            };
            let stream = TcpStream::connect((host.as_str(), port))
                .map_err(|e| format!("ftp connect {host}:{port}: {e}"))?;
            stream
                .set_read_timeout(Some(Duration::from_secs(30)))
                .ok();
            let mut stream = stream;
            let (code, reply) = ftp_read_reply(&mut stream)?;
            if code != 220 {
                return Err(format!("ftp: server refused connection ({code}): {reply}"));
            }
            let mut session = BTreeMap::new();
            session.insert("socket".into(), Value::Socket(Arc::new(Mutex::new(stream))));
            session.insert("host".into(), Value::String(host));
            Ok(Value::Dict(session))
        },
        "ftp_login" => |args| {
            let stream = session_socket(&args[0])?;
            let user = arg_string(&args, 1)?;
            let pass = match args.get(2) {
                Some(Value::String(s)) => s.clone(),
                _ => String::new(),
            };
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, format!("USER {user}\r\n").as_bytes())?;
            let (code, reply) = ftp_read_reply(&mut s)?;
            if code == 331 {
                stream_write_all(&mut s, format!("PASS {pass}\r\n").as_bytes())?;
                let (code, reply) = ftp_read_reply(&mut s)?;
                if code != 230 {
                    return Err(format!("ftp login failed ({code}): {reply}"));
                }
            } else if code != 230 {
                return Err(format!("ftp login failed ({code}): {reply}"));
            }
            stream_write_all(&mut s, b"TYPE I\r\n")?;
            ftp_read_reply(&mut s)?;
            Ok(Value::Bool(true))
        },
        "ftp_pwd" => |args| {
            let stream = session_socket(&args[0])?;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, b"PWD\r\n")?;
            let (code, reply) = ftp_read_reply(&mut s)?;
            if code != 257 {
                return Err(format!("ftp pwd failed ({code}): {reply}"));
            }
            let start = reply.find('"');
            let rest = start.map(|i| &reply[i + 1..]).unwrap_or("");
            let end = rest.find('"');
            Ok(Value::String(end.map(|i| rest[..i].to_string()).unwrap_or_default()))
        },
        "ftp_list" => |args| {
            let stream = session_socket(&args[0])?;
            let path = match args.get(1) {
                Some(Value::String(s)) => s.clone(),
                _ => String::new(),
            };
            let mut s = stream.lock().unwrap();
            let data = ftp_data_connect(&mut s)?;
            stream_write_all(&mut s, format!("LIST {path}\r\n").as_bytes())?;
            let (code, reply) = ftp_read_reply(&mut s)?;
            if !(code == 150 || code == 125) {
                return Err(format!("ftp list failed ({code}): {reply}"));
            }
            let mut data = data;
            let mut content = Vec::new();
            data.read_to_end(&mut content)
                .map_err(|e| format!("ftp data read failed: {e}"))?;
            let (code, reply) = ftp_read_reply(&mut s)?;
            if code != 226 {
                return Err(format!("ftp list transfer failed ({code}): {reply}"));
            }
            Ok(Value::String(String::from_utf8_lossy(&content).into_owned()))
        },
        "ftp_nlist" => |args| {
            let stream = session_socket(&args[0])?;
            let path = match args.get(1) {
                Some(Value::String(s)) => s.clone(),
                _ => String::new(),
            };
            let mut s = stream.lock().unwrap();
            let data = ftp_data_connect(&mut s)?;
            stream_write_all(&mut s, format!("NLST {path}\r\n").as_bytes())?;
            let (code, _) = ftp_read_reply(&mut s)?;
            if !(code == 150 || code == 125) {
                return Err(format!("ftp nlist failed ({code})"));
            }
            let mut data = data;
            let mut content = String::new();
            data.read_to_string(&mut content)
                .map_err(|e| format!("ftp data read failed: {e}"))?;
            ftp_read_reply(&mut s)?;
            let names: Vec<Value> = content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| Value::String(l.to_string()))
                .collect();
            Ok(Value::List(names))
        },
        "ftp_cwd" => |args| {
            let stream = session_socket(&args[0])?;
            let dir = arg_string(&args, 1)?;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, format!("CWD {dir}\r\n").as_bytes())?;
            let (code, reply) = ftp_read_reply(&mut s)?;
            if code != 250 {
                return Err(format!("ftp cwd failed ({code}): {reply}"));
            }
            Ok(Value::Bool(true))
        },
        "ftp_retr" => |args| {
            let stream = session_socket(&args[0])?;
            let remote = arg_string(&args, 1)?;
            let mut s = stream.lock().unwrap();
            let data = ftp_data_connect(&mut s)?;
            stream_write_all(&mut s, format!("RETR {remote}\r\n").as_bytes())?;
            let (code, _) = ftp_read_reply(&mut s)?;
            if !(code == 150 || code == 125) {
                return Err(format!("ftp retr failed ({code})"));
            }
            let mut data = data;
            let mut content = Vec::new();
            data.read_to_end(&mut content)
                .map_err(|e| format!("ftp data read failed: {e}"))?;
            ftp_read_reply(&mut s)?;
            Ok(Value::String(String::from_utf8_lossy(&content).into_owned()))
        },
        "ftp_stor" => |args| {
            let stream = session_socket(&args[0])?;
            let remote = arg_string(&args, 1)?;
            let data = arg_string(&args, 2)?;
            let mut s = stream.lock().unwrap();
            let mut data_conn = ftp_data_connect(&mut s)?;
            stream_write_all(&mut s, format!("STOR {remote}\r\n").as_bytes())?;
            let (code, _) = ftp_read_reply(&mut s)?;
            if !(code == 150 || code == 125) {
                return Err(format!("ftp stor failed ({code})"));
            }
            data_conn
                .write_all(data.as_bytes())
                .map_err(|e| format!("ftp data write failed: {e}"))?;
            data_conn
                .shutdown(std::net::Shutdown::Both)
                .ok();
            let (code, _) = ftp_read_reply(&mut s)?;
            if code != 226 {
                return Err(format!("ftp stor failed ({code})"));
            }
            Ok(Value::Bool(true))
        },
        "ftp_dele" => |args| {
            let stream = session_socket(&args[0])?;
            let remote = arg_string(&args, 1)?;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, format!("DELE {remote}\r\n").as_bytes())?;
            let (code, reply) = ftp_read_reply(&mut s)?;
            if code != 250 {
                return Err(format!("ftp dele failed ({code}): {reply}"));
            }
            Ok(Value::Bool(true))
        },
        "ftp_mkdir" => |args| {
            let stream = session_socket(&args[0])?;
            let dir = arg_string(&args, 1)?;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, format!("MKD {dir}\r\n").as_bytes())?;
            let (code, reply) = ftp_read_reply(&mut s)?;
            if code != 257 {
                return Err(format!("ftp mkdir failed ({code}): {reply}"));
            }
            Ok(Value::Bool(true))
        },
        "ftp_rmdir" => |args| {
            let stream = session_socket(&args[0])?;
            let dir = arg_string(&args, 1)?;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, format!("RMD {dir}\r\n").as_bytes())?;
            let (code, reply) = ftp_read_reply(&mut s)?;
            if code != 250 {
                return Err(format!("ftp rmdir failed ({code}): {reply}"));
            }
            Ok(Value::Bool(true))
        },
        "ftp_rename" => |args| {
            let stream = session_socket(&args[0])?;
            let from = arg_string(&args, 1)?;
            let to = arg_string(&args, 2)?;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, format!("RNFR {from}\r\n").as_bytes())?;
            let (code, reply) = ftp_read_reply(&mut s)?;
            if code != 350 {
                return Err(format!("ftp rename failed ({code}): {reply}"));
            }
            stream_write_all(&mut s, format!("RNTO {to}\r\n").as_bytes())?;
            let (code, reply) = ftp_read_reply(&mut s)?;
            if code != 250 {
                return Err(format!("ftp rename failed ({code}): {reply}"));
            }
            Ok(Value::Bool(true))
        },
        "ftp_quit" => |args| {
            let stream = session_socket(&args[0])?;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, b"QUIT\r\n")?;
            ftp_read_reply(&mut s)?;
            s.shutdown(std::net::Shutdown::Both).ok();
            Ok(Value::Bool(true))
        },
        "smtp_connect" => |args| {
            let host = arg_string(&args, 0)?;
            let port = match args.get(1) {
                Some(Value::Number(n)) => *n as u16,
                _ => 25,
            };
            let stream = TcpStream::connect((host.as_str(), port))
                .map_err(|e| format!("smtp connect {host}:{port}: {e}"))?;
            let mut stream = stream;
            let (code, reply) = smtp_read_reply(&mut stream)?;
            if code != 220 {
                return Err(format!("smtp: server refused ({code}): {reply}"));
            }
            stream_write_all(&mut stream, b"EHLO localhost\r\n")?;
            let (code, reply) = smtp_read_reply(&mut stream)?;
            if code != 250 {
                return Err(format!("smtp ehlo failed ({code}): {reply}"));
            }
            let mut session = BTreeMap::new();
            session.insert("socket".into(), Value::Socket(Arc::new(Mutex::new(stream))));
            session.insert("host".into(), Value::String(host));
            Ok(Value::Dict(session))
        },
        "smtp_login" => |args| {
            let stream = session_socket(&args[0])?;
            let user = arg_string(&args, 1)?;
            let pass = arg_string(&args, 2)?;
            use base64::Engine;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, b"AUTH LOGIN\r\n")?;
            let (code, reply) = smtp_read_reply(&mut s)?;
            if code != 334 {
                return Err(format!("smtp auth failed ({code}): {reply}"));
            }
            let enc = |d: &str| base64::engine::general_purpose::STANDARD.encode(d.as_bytes());
            stream_write_all(&mut s, format!("{}\r\n", enc(&user)).as_bytes())?;
            let (code, reply) = smtp_read_reply(&mut s)?;
            if code != 334 {
                return Err(format!("smtp auth failed ({code}): {reply}"));
            }
            stream_write_all(&mut s, format!("{}\r\n", enc(&pass)).as_bytes())?;
            let (code, reply) = smtp_read_reply(&mut s)?;
            if code != 235 {
                return Err(format!("smtp auth rejected ({code}): {reply}"));
            }
            Ok(Value::Bool(true))
        },
        "smtp_sendmail" => |args| {
            let stream = session_socket(&args[0])?;
            let from = arg_string(&args, 1)?;
            let to = arg_string(&args, 2)?;
            let msg = arg_string(&args, 3)?;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, format!("MAIL FROM:<{from}>\r\n").as_bytes())?;
            let (code, reply) = smtp_read_reply(&mut s)?;
            if code != 250 {
                return Err(format!("smtp mail-from failed ({code}): {reply}"));
            }
            stream_write_all(&mut s, format!("RCPT TO:<{to}>\r\n").as_bytes())?;
            let (code, reply) = smtp_read_reply(&mut s)?;
            if code != 250 {
                return Err(format!("smtp rcpt-to failed ({code}): {reply}"));
            }
            stream_write_all(&mut s, b"DATA\r\n")?;
            let (code, reply) = smtp_read_reply(&mut s)?;
            if code != 354 {
                return Err(format!("smtp data failed ({code}): {reply}"));
            }
            let mut payload = msg.replace("\r\n.\r\n", "\r\n..\r\n");
            if !payload.ends_with("\r\n") {
                payload.push_str("\r\n");
            }
            if !payload.ends_with(".\r\n") {
                payload.push_str(".\r\n");
            }
            stream_write_all(&mut s, payload.as_bytes())?;
            let (code, reply) = smtp_read_reply(&mut s)?;
            if code != 250 {
                return Err(format!("smtp send failed ({code}): {reply}"));
            }
            Ok(Value::Bool(true))
        },
        "smtp_quit" => |args| {
            let stream = session_socket(&args[0])?;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, b"QUIT\r\n")?;
            smtp_read_reply(&mut s)?;
            s.shutdown(std::net::Shutdown::Both).ok();
            Ok(Value::Bool(true))
        },
        "smtp_message" => |args| {
            let from = arg_string(&args, 0)?;
            let to = arg_string(&args, 1)?;
            let subject = arg_string(&args, 2)?;
            let body = arg_string(&args, 3)?;
            let mut msg = String::new();
            msg.push_str(&format!("From: {from}\r\n"));
            msg.push_str(&format!("To: {to}\r\n"));
            msg.push_str(&format!("Subject: {subject}\r\n"));
            msg.push_str("MIME-Version: 1.0\r\n");
            msg.push_str("Content-Type: text/plain; charset=utf-8\r\n");
            msg.push_str("\r\n");
            msg.push_str(&body);
            Ok(Value::String(msg))
        },
        "pop3_connect" => |args| {
            let host = arg_string(&args, 0)?;
            let user = arg_string(&args, 1)?;
            let pass = arg_string(&args, 2)?;
            let port = match args.get(3) {
                Some(Value::Number(n)) => *n as u16,
                _ => 110,
            };
            let stream = TcpStream::connect((host.as_str(), port))
                .map_err(|e| format!("pop3 connect {host}:{port}: {e}"))?;
            let mut stream = stream;
            let mut greeting = read_line(&mut stream)?;
            if !greeting.starts_with("+OK") {
                return Err(format!("pop3 greeting failed: {greeting}"));
            }
            stream_write_all(&mut stream, format!("USER {user}\r\n").as_bytes())?;
            greeting = read_line(&mut stream)?;
            if !greeting.starts_with("+OK") {
                return Err(format!("pop3 user failed: {greeting}"));
            }
            stream_write_all(&mut stream, format!("PASS {pass}\r\n").as_bytes())?;
            greeting = read_line(&mut stream)?;
            if !greeting.starts_with("+OK") {
                return Err(format!("pop3 login failed: {greeting}"));
            }
            let mut session = BTreeMap::new();
            session.insert("socket".into(), Value::Socket(Arc::new(Mutex::new(stream))));
            session.insert("host".into(), Value::String(host));
            Ok(Value::Dict(session))
        },
        "pop3_stat" => |args| {
            let stream = session_socket(&args[0])?;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, b"STAT\r\n")?;
            let reply = read_line(&mut s)?;
            if !reply.starts_with("+OK") {
                return Err(format!("pop3 stat failed: {reply}"));
            }
            let parts: Vec<&str> = reply.split_whitespace().collect();
            let count = parts.get(1).and_then(|p| p.parse::<f64>().ok()).unwrap_or(0.0);
            let size = parts.get(2).and_then(|p| p.parse::<f64>().ok()).unwrap_or(0.0);
            let mut out = BTreeMap::new();
            out.insert("count".into(), Value::Number(count));
            out.insert("size".into(), Value::Number(size));
            Ok(Value::Dict(out))
        },
        "pop3_list" => |args| {
            let stream = session_socket(&args[0])?;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, b"LIST\r\n")?;
            let mut items = Vec::new();
            loop {
                let line = read_line(&mut s)?;
                if line == "." {
                    break;
                }
                if !line.starts_with("+OK") {
                    return Err(format!("pop3 list failed: {line}"));
                }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[0] != "+OK" {
                    let id = parts[0].parse::<f64>().unwrap_or(0.0);
                    let size = parts.get(1).and_then(|p| p.parse::<f64>().ok()).unwrap_or(0.0);
                    items.push(Value::List(vec![Value::Number(id), Value::Number(size)]));
                }
            }
            Ok(Value::List(items))
        },
        "pop3_retr" => |args| {
            let stream = session_socket(&args[0])?;
            let n = arg_number(&args, 1)? as u64;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, format!("RETR {n}\r\n").as_bytes())?;
            let mut content = String::new();
            loop {
                let line = read_line(&mut s)?;
                if line == "." {
                    break;
                }
                if line.starts_with("-ERR") {
                    return Err(format!("pop3 retr failed: {line}"));
                }
                if content.is_empty() && line.starts_with("+OK") {
                    continue;
                }
                content.push_str(&line);
                content.push('\n');
            }
            Ok(Value::String(content))
        },
        "pop3_dele" => |args| {
            let stream = session_socket(&args[0])?;
            let n = arg_number(&args, 1)? as u64;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, format!("DELE {n}\r\n").as_bytes())?;
            let reply = read_line(&mut s)?;
            if !reply.starts_with("+OK") {
                return Err(format!("pop3 dele failed: {reply}"));
            }
            Ok(Value::Bool(true))
        },
        "pop3_quit" => |args| {
            let stream = session_socket(&args[0])?;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, b"QUIT\r\n")?;
            read_line(&mut s)?;
            s.shutdown(std::net::Shutdown::Both).ok();
            Ok(Value::Bool(true))
        },
        "imap_connect" => |args| {
            let host = arg_string(&args, 0)?;
            let user = arg_string(&args, 1)?;
            let pass = arg_string(&args, 2)?;
            let port = match args.get(3) {
                Some(Value::Number(n)) => *n as u16,
                _ => 143,
            };
            let stream = TcpStream::connect((host.as_str(), port))
                .map_err(|e| format!("imap connect {host}:{port}: {e}"))?;
            let mut stream = stream;
            let line = read_line(&mut stream)?;
            if !line.contains("OK") {
                return Err(format!("imap greeting failed: {line}"));
            }
            let mut tag = 1u32;
            let resp = imap_command(&mut stream, &format!("a{tag}"), &format!("LOGIN {user} {pass}"))?;
            let _ = resp;
            tag += 1;
            let mut session = BTreeMap::new();
            session.insert("socket".into(), Value::Socket(Arc::new(Mutex::new(stream))));
            session.insert("host".into(), Value::String(host));
            session.insert("tag".into(), Value::Number(tag as f64));
            Ok(Value::Dict(session))
        },
        "imap_select" => |args| {
            let stream = session_socket(&args[0])?;
            let mailbox = arg_string(&args, 1)?;
            let tag = match &args[0] {
                Value::Dict(d) => match d.get("tag") {
                    Some(Value::Number(n)) => *n as u32,
                    _ => 2,
                },
                _ => 2,
            };
            let mut s = stream.lock().unwrap();
            let tag_str = format!("a{tag}");
            imap_command(&mut s, &tag_str, &format!("SELECT {mailbox}"))?;
            Ok(Value::Bool(true))
        },
        "imap_search" => |args| {
            let stream = session_socket(&args[0])?;
            let criteria = match args.get(1) {
                Some(Value::String(s)) => s.clone(),
                _ => "ALL".into(),
            };
            let tag = match &args[0] {
                Value::Dict(d) => match d.get("tag") {
                    Some(Value::Number(n)) => *n as u32,
                    _ => 3,
                },
                _ => 3,
            };
            let mut s = stream.lock().unwrap();
            let tag_str = format!("a{tag}");
            let resp = imap_command(&mut s, &tag_str, &format!("SEARCH {criteria}"))?;
            let mut ids = Vec::new();
            for line in resp.lines() {
                if let Some(rest) = line.strip_prefix("* SEARCH") {
                    for tok in rest.split_whitespace() {
                        if let Ok(n) = tok.parse::<f64>() {
                            ids.push(Value::Number(n));
                        }
                    }
                }
            }
            Ok(Value::List(ids))
        },
        "imap_fetch" => |args| {
            let stream = session_socket(&args[0])?;
            let id = arg_number(&args, 1)? as u64;
            let tag = match &args[0] {
                Value::Dict(d) => match d.get("tag") {
                    Some(Value::Number(n)) => *n as u32,
                    _ => 4,
                },
                _ => 4,
            };
            let mut s = stream.lock().unwrap();
            let tag_str = format!("a{tag}");
            let resp = imap_command(&mut s, &tag_str, &format!("FETCH {id} (FLAGS BODY[])"))?;
            let mut flags = Vec::new();
            let body = String::new();
            for line in resp.lines() {
                if let Some(rest) = line.strip_prefix("*") {
                    if rest.contains("FLAGS") {
                        let start = rest.find("FLAGS").map(|i| i + 5).unwrap_or(0);
                        let trimmed = rest[start..].trim_start_matches(|c: char| c == '(' || c == ' ' || c == ')');
                        for f in trimmed.split_whitespace() {
                            flags.push(Value::String(f.to_string()));
                        }
                    }
                }
            }
            let mut out = BTreeMap::new();
            out.insert("flags".into(), Value::List(flags));
            out.insert("body".into(), Value::String(body));
            Ok(Value::Dict(out))
        },
        "imap_list" => |args| {
            let stream = session_socket(&args[0])?;
            let tag = match &args[0] {
                Value::Dict(d) => match d.get("tag") {
                    Some(Value::Number(n)) => *n as u32,
                    _ => 5,
                },
                _ => 5,
            };
            let mut s = stream.lock().unwrap();
            let tag_str = format!("a{tag}");
            let resp = imap_command(&mut s, &tag_str, "LIST \"\" *")?;
            let mut boxes = Vec::new();
            for line in resp.lines() {
                if let Some(pos) = line.find("\"") {
                    if let Some(q2) = line[pos + 1..].find("\"") {
                        let name = line[pos + 1 + q2 + 1..].trim();
                        let name = name.trim_matches('"');
                        if !name.is_empty() {
                            boxes.push(Value::String(name.to_string()));
                        }
                    }
                }
            }
            Ok(Value::List(boxes))
        },
        "imap_logout" => |args| {
            let stream = session_socket(&args[0])?;
            let tag = match &args[0] {
                Value::Dict(d) => match d.get("tag") {
                    Some(Value::Number(n)) => *n as u32,
                    _ => 6,
                },
                _ => 6,
            };
            let mut s = stream.lock().unwrap();
            let tag_str = format!("a{tag}");
            stream_write_all(&mut s, format!("{tag_str} LOGOUT\r\n").as_bytes())?;
            let mut line = read_line(&mut s)?;
            while !line.contains("BYE") && !line.is_empty() {
                line = read_line(&mut s)?;
            }
            s.shutdown(std::net::Shutdown::Both).ok();
            Ok(Value::Bool(true))
        },
        "telnet_connect" => |args| {
            let host = arg_string(&args, 0)?;
            let port = match args.get(1) {
                Some(Value::Number(n)) => *n as u16,
                _ => 23,
            };
            let stream = TcpStream::connect((host.as_str(), port))
                .map_err(|e| format!("telnet connect {host}:{port}: {e}"))?;
            let mut session = BTreeMap::new();
            session.insert("socket".into(), Value::Socket(Arc::new(Mutex::new(stream))));
            session.insert("host".into(), Value::String(host));
            Ok(Value::Dict(session))
        },
        "telnet_write" => |args| {
            let stream = session_socket(&args[0])?;
            let data = arg_string(&args, 1)?;
            let mut s = stream.lock().unwrap();
            stream_write_all(&mut s, data.as_bytes())?;
            Ok(Value::Bool(true))
        },
        "telnet_read" => |args| {
            let stream = session_socket(&args[0])?;
            let size = match args.get(1) {
                Some(Value::Number(n)) => *n as usize,
                _ => 1024,
            };
            let mut s = stream.lock().unwrap();
            let mut buf = vec![0u8; size];
            let n = s.read(&mut buf).map_err(|e| format!("telnet read failed: {e}"))?;
            let clean = strip_telnet_iac(&buf[..n]);
            Ok(Value::String(String::from_utf8_lossy(&clean).into_owned()))
        },
        "telnet_read_until" => |args| {
            let stream = session_socket(&args[0])?;
            let marker = arg_string(&args, 1)?;
            let mut s = stream.lock().unwrap();
            let mut buf = Vec::new();
            let mut byte = [0u8; 1];
            while !String::from_utf8_lossy(&buf).ends_with(&marker) && buf.len() < 65536 {
                if s.read(&mut byte).map_err(|e| format!("telnet read failed: {e}"))? == 0 {
                    break;
                }
                buf.push(byte[0]);
            }
            let clean = strip_telnet_iac(&buf);
            Ok(Value::String(String::from_utf8_lossy(&clean).into_owned()))
        },
        "telnet_close" => |args| {
            let stream = session_socket(&args[0])?;
            let s = stream.lock().unwrap();
            s.shutdown(std::net::Shutdown::Both).ok();
            Ok(Value::Bool(true))
        },
        "dns_resolve" => |args| {
            let name = arg_string(&args, 0)?;
            let records = dns_query_impl(&name, "A")?;
            let mut ips = Vec::new();
            for rec in records {
                if let Value::Dict(d) = &rec {
                    if let Some(Value::String(data)) = d.get("data") {
                        if data.contains('.') {
                            ips.push(Value::String(data.clone()));
                        }
                    }
                }
            }
            Ok(Value::List(ips))
        },
        "dns_query" => |args| {
            let name = arg_string(&args, 0)?;
            let rtype = match args.get(1) {
                Some(Value::String(s)) => s.clone(),
                _ => "A".into(),
            };
            Ok(Value::List(dns_query_impl(&name, &rtype)?))
        },
        "ssh_run" => |args| {
            let opts = arg_dict(&args, 0)?;
            let command = arg_string(&args, 1)?;
            let mut cmd = Command::new("ssh");
            if let Some(Value::Number(p)) = opts.get("port") {
                cmd.arg("-p").arg(format!("{}", *p as u64));
            }
            if let Some(Value::String(k)) = opts.get("key") {
                cmd.arg("-i").arg(k);
            }
            if let Some(Value::String(o)) = opts.get("options") {
                cmd.arg(o);
            }
            let user = match opts.get("user") {
                Some(Value::String(u)) => u.clone(),
                _ => "root".into(),
            };
            let host = match opts.get("host") {
                Some(Value::String(h)) => h,
                _ => return Err("ssh.run: opts needs host".into()),
            };
            cmd.arg(format!("{user}@{host}")).arg(&command);
            if let Some(Value::Number(t)) = opts.get("timeout") {
                cmd.env("CONNECT_TIMEOUT", format!("{}", *t as u64));
            }
            let output = cmd
                .output()
                .map_err(|e| format!("ssh: {e} (is the ssh binary installed?)"))?;
            if !output.status.success() {
                return Err(format!(
                    "ssh command failed: {}\n{}",
                    String::from_utf8_lossy(&output.stderr),
                    String::from_utf8_lossy(&output.stdout)
                ));
            }
            Ok(Value::String(String::from_utf8_lossy(&output.stdout).into_owned()))
        },
        "ssh_upload" => |args| {
            let opts = arg_dict(&args, 0)?;
            let local = arg_string(&args, 1)?;
            let remote = arg_string(&args, 2)?;
            let mut cmd = Command::new("scp");
            if let Some(Value::Number(p)) = opts.get("port") {
                cmd.arg("-P").arg(format!("{}", *p as u64));
            }
            if let Some(Value::String(k)) = opts.get("key") {
                cmd.arg("-i").arg(k);
            }
            let user = match opts.get("user") {
                Some(Value::String(u)) => u.clone(),
                _ => "root".into(),
            };
            let host = match opts.get("host") {
                Some(Value::String(h)) => h,
                _ => return Err("ssh.upload: opts needs host".into()),
            };
            cmd.arg(&local).arg(format!("{user}@{host}:{remote}"));
            let output = cmd
                .output()
                .map_err(|e| format!("scp: {e} (is the scp binary installed?)"))?;
            if !output.status.success() {
                return Err(format!("scp failed: {}", String::from_utf8_lossy(&output.stderr)));
            }
            Ok(Value::Bool(true))
        },
        "ssh_download" => |args| {
            let opts = arg_dict(&args, 0)?;
            let remote = arg_string(&args, 1)?;
            let local = arg_string(&args, 2)?;
            let mut cmd = Command::new("scp");
            if let Some(Value::Number(p)) = opts.get("port") {
                cmd.arg("-P").arg(format!("{}", *p as u64));
            }
            if let Some(Value::String(k)) = opts.get("key") {
                cmd.arg("-i").arg(k);
            }
            let user = match opts.get("user") {
                Some(Value::String(u)) => u.clone(),
                _ => "root".into(),
            };
            let host = match opts.get("host") {
                Some(Value::String(h)) => h,
                _ => return Err("ssh.download: opts needs host".into()),
            };
            cmd.arg(format!("{user}@{host}:{remote}")).arg(&local);
            let output = cmd
                .output()
                .map_err(|e| format!("scp: {e} (is the scp binary installed?)"))?;
            if !output.status.success() {
                return Err(format!("scp failed: {}", String::from_utf8_lossy(&output.stderr)));
            }
            Ok(Value::Bool(true))
        },
        "ssh_available" => |_| {
            let found = Command::new("ssh").arg("-V").output().is_ok();
            Ok(Value::Bool(found))
        },
        "scapy_checksum" => |args| {
            let data = arg_string(&args, 0)?;
            Ok(Value::Number(internet_checksum(data.as_bytes()) as f64))
        },
        "scapy_ip" => |args| {
            let mut layer = BTreeMap::new();
            layer.insert("type".into(), Value::String("IP".into()));
            if let Some(Value::String(s)) = args.first() {
                layer.insert("src".into(), Value::String(s.clone()));
            }
            if let Some(Value::String(d)) = args.get(1) {
                layer.insert("dst".into(), Value::String(d.clone()));
            }
            if let Some(Value::String(p)) = args.get(2) {
                layer.insert("proto".into(), Value::String(p.clone()));
            }
            if let Some(Value::Dict(p)) = args.get(3) {
                layer.insert("payload".into(), Value::Dict(p.clone()));
            }
            Ok(Value::Dict(layer))
        },
        "scapy_tcp" => |args| {
            let mut layer = BTreeMap::new();
            layer.insert("type".into(), Value::String("TCP".into()));
            if let Some(Value::Number(s)) = args.get(0) {
                layer.insert("sport".into(), Value::Number(*s));
            }
            if let Some(Value::Number(d)) = args.get(1) {
                layer.insert("dport".into(), Value::Number(*d));
            }
            if let Some(Value::Dict(p)) = args.get(2) {
                layer.insert("payload".into(), Value::Dict(p.clone()));
            }
            Ok(Value::Dict(layer))
        },
        "scapy_udp" => |args| {
            let mut layer = BTreeMap::new();
            layer.insert("type".into(), Value::String("UDP".into()));
            if let Some(Value::Number(s)) = args.get(0) {
                layer.insert("sport".into(), Value::Number(*s));
            }
            if let Some(Value::Number(d)) = args.get(1) {
                layer.insert("dport".into(), Value::Number(*d));
            }
            if let Some(Value::Dict(p)) = args.get(2) {
                layer.insert("payload".into(), Value::Dict(p.clone()));
            }
            Ok(Value::Dict(layer))
        },
        "scapy_icmp" => |args| {
            let mut layer = BTreeMap::new();
            layer.insert("type".into(), Value::String("ICMP".into()));
            if let Some(Value::Number(t)) = args.get(0) {
                layer.insert("icmp_type".into(), Value::Number(*t));
            }
            if let Some(Value::Number(c)) = args.get(1) {
                layer.insert("icmp_code".into(), Value::Number(*c));
            }
            if let Some(Value::Dict(p)) = args.get(2) {
                layer.insert("payload".into(), Value::Dict(p.clone()));
            }
            Ok(Value::Dict(layer))
        },
        "scapy_raw" => |args| {
            let data = arg_string(&args, 0)?;
            let mut layer = BTreeMap::new();
            layer.insert("type".into(), Value::String("Raw".into()));
            layer.insert("data".into(), Value::String(data));
            Ok(Value::Dict(layer))
        },
        "scapy_build" => |args| {
            let layer = arg_dict(&args, 0)?;
            let bytes = layer_bytes(&layer)?;
            Ok(Value::String(String::from_utf8_lossy(&bytes).into_owned()))
        },
        "scapy_parse" => |args| {
            let data = arg_string(&args, 0)?;
            Ok(parse_packet(data.as_bytes()))
        },
        "scapy_send" => |args| {
            let layer = arg_dict(&args, 0)?;
            let bytes = layer_bytes(&layer)?;
            raw_socket_send(&bytes)?;
            Ok(Value::Bool(true))
        },
        "scapy_sniff" => |args| {
            let count = match args.get(0) {
                Some(Value::Number(n)) => *n as u32,
                _ => 1,
            };
            let timeout = match args.get(1) {
                Some(Value::Number(n)) => *n as u64,
                _ => 5,
            };
            sniff_packets(count, timeout)
        },
        "scapy_ip_to_int" => |args| {
            let ip = arg_string(&args, 0)?;
            Ok(Value::Number(ip_str_to_u32(&ip)? as f64))
        },
        "scapy_int_to_ip" => |args| {
            let n = arg_number(&args, 0)? as u32;
            Ok(Value::String(u32_to_ip(n)))
        },
        "str_upper" => |args| Ok(Value::String(arg_string(&args, 0)?.to_uppercase())),
        "str_lower" => |args| Ok(Value::String(arg_string(&args, 0)?.to_lowercase())),
        "str_title" => |args| {
            let s = arg_string(&args, 0)?;
            let mut out = String::with_capacity(s.len());
            let mut prev = ' ';
            for c in s.chars() {
                if prev.is_whitespace() {
                    out.extend(c.to_uppercase());
                } else {
                    out.extend(c.to_lowercase());
                }
                prev = c;
            }
            Ok(Value::String(out))
        },
        "str_capitalize" => |args| {
            let s = arg_string(&args, 0)?;
            let mut out = String::with_capacity(s.len());
            for (i, c) in s.chars().enumerate() {
                if i == 0 {
                    out.extend(c.to_uppercase());
                } else {
                    out.extend(c.to_lowercase());
                }
            }
            Ok(Value::String(out))
        },
        "str_swapcase" => |args| {
            let s = arg_string(&args, 0)?;
            let mut out = String::with_capacity(s.len());
            for c in s.chars() {
                if c.is_uppercase() {
                    out.extend(c.to_lowercase());
                } else if c.is_lowercase() {
                    out.extend(c.to_uppercase());
                } else {
                    out.push(c);
                }
            }
            Ok(Value::String(out))
        },
        "str_strip" => |args| {
            let s = arg_string(&args, 0)?;
            Ok(Value::String(s.trim().to_string()))
        },
        "str_lstrip" => |args| {
            let s = arg_string(&args, 0)?;
            Ok(Value::String(s.trim_start().to_string()))
        },
        "str_rstrip" => |args| {
            let s = arg_string(&args, 0)?;
            Ok(Value::String(s.trim_end().to_string()))
        },
        "str_split" => |args| {
            let s = arg_string(&args, 0)?;
            let sep = match args.get(1) {
                Some(Value::String(sep)) => sep.clone(),
                _ => " ".into(),
            };
            let parts: Vec<Value> = if sep.is_empty() {
                s.chars().map(|c| Value::String(c.to_string())).collect()
            } else {
                s.split(&sep).map(|p| Value::String(p.to_string())).collect()
            };
            Ok(Value::List(parts))
        },
        "str_splitlines" => |args| {
            let s = arg_string(&args, 0)?;
            let parts: Vec<Value> = s
                .lines()
                .map(|l| Value::String(l.to_string()))
                .collect();
            Ok(Value::List(parts))
        },
        "str_join" => |args| {
            let sep = arg_string(&args, 0)?;
            let list = arg_list(&args, 1)?;
            let parts: Vec<String> = list
                .iter()
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect();
            Ok(Value::String(parts.join(&sep)))
        },
        "str_replace" => |args| {
            let s = arg_string(&args, 0)?;
            let old = arg_string(&args, 1)?;
            let new = arg_string(&args, 2)?;
            Ok(Value::String(s.replace(&old, &new)))
        },
        "str_count" => |args| {
            let s = arg_string(&args, 0)?;
            let sub = arg_string(&args, 1)?;
            let mut count = 0usize;
            let mut rest = s.as_str();
            while let Some(pos) = rest.find(&sub) {
                count += 1;
                rest = &rest[pos + sub.len()..];
            }
            Ok(Value::Number(count as f64))
        },
        "str_find" => |args| {
            let s = arg_string(&args, 0)?;
            let sub = arg_string(&args, 1)?;
            match s.find(&sub) {
                Some(i) => Ok(Value::Number(i as f64)),
                None => Ok(Value::Number(-1.0)),
            }
        },
        "str_rfind" => |args| {
            let s = arg_string(&args, 0)?;
            let sub = arg_string(&args, 1)?;
            match s.rfind(&sub) {
                Some(i) => Ok(Value::Number(i as f64)),
                None => Ok(Value::Number(-1.0)),
            }
        },
        "str_startswith" => |args| {
            let s = arg_string(&args, 0)?;
            let prefix = arg_string(&args, 1)?;
            Ok(Value::Bool(s.starts_with(&prefix)))
        },
        "str_endswith" => |args| {
            let s = arg_string(&args, 0)?;
            let suffix = arg_string(&args, 1)?;
            Ok(Value::Bool(s.ends_with(&suffix)))
        },
        "str_contains" => |args| {
            let s = arg_string(&args, 0)?;
            let sub = arg_string(&args, 1)?;
            Ok(Value::Bool(s.contains(&sub)))
        },
        "str_ljust" => |args| {
            let s = arg_string(&args, 0)?;
            let width = arg_number(&args, 1)? as usize;
            let fill = match args.get(2) {
                Some(Value::String(f)) if !f.is_empty() => f.chars().next().unwrap(),
                _ => ' ',
            };
            let mut out = s.clone();
            while out.len() < width {
                out.push(fill);
            }
            Ok(Value::String(out))
        },
        "str_rjust" => |args| {
            let s = arg_string(&args, 0)?;
            let width = arg_number(&args, 1)? as usize;
            let fill = match args.get(2) {
                Some(Value::String(f)) if !f.is_empty() => f.chars().next().unwrap(),
                _ => ' ',
            };
            let mut out = String::new();
            while out.len() + s.len() < width {
                out.push(fill);
            }
            out.push_str(&s);
            Ok(Value::String(out))
        },
        "str_center" => |args| {
            let s = arg_string(&args, 0)?;
            let width = arg_number(&args, 1)? as usize;
            let fill = match args.get(2) {
                Some(Value::String(f)) if !f.is_empty() => f.chars().next().unwrap(),
                _ => ' ',
            };
            if s.len() >= width {
                return Ok(Value::String(s));
            }
            let total = width - s.len();
            let left = total / 2;
            let right = total - left;
            let mut out = String::new();
            for _ in 0..left {
                out.push(fill);
            }
            out.push_str(&s);
            for _ in 0..right {
                out.push(fill);
            }
            Ok(Value::String(out))
        },
        "str_zfill" => |args| {
            let s = arg_string(&args, 0)?;
            let width = arg_number(&args, 1)? as usize;
            let sign = if s.starts_with('-') || s.starts_with('+') {
                Some(s.as_bytes()[0] as char)
            } else {
                None
            };
            let body = if sign.is_some() { &s[1..] } else { &s[..] };
            let mut out = String::new();
            if let Some(sig) = sign {
                out.push(sig);
            }
            while out.len() + body.len() < width {
                out.push('0');
            }
            out.push_str(body);
            Ok(Value::String(out))
        },
        "str_repeat" => |args| {
            let s = arg_string(&args, 0)?;
            let n = arg_number(&args, 1)? as usize;
            Ok(Value::String(s.repeat(n)))
        },
        "str_isdigit" => |args| {
            let s = arg_string(&args, 0)?;
            Ok(Value::Bool(!s.is_empty() && s.chars().all(|c| c.is_ascii_digit())))
        },
        "str_isalpha" => |args| {
            let s = arg_string(&args, 0)?;
            Ok(Value::Bool(!s.is_empty() && s.chars().all(|c| c.is_alphabetic())))
        },
        "str_isalnum" => |args| {
            let s = arg_string(&args, 0)?;
            Ok(Value::Bool(!s.is_empty() && s.chars().all(|c| c.is_alphanumeric())))
        },
        "str_isspace" => |args| {
            let s = arg_string(&args, 0)?;
            Ok(Value::Bool(!s.is_empty() && s.chars().all(|c| c.is_whitespace())))
        },
        "str_islower" => |args| {
            let s = arg_string(&args, 0)?;
            Ok(Value::Bool(
                !s.is_empty() && s.chars().any(|c| c.is_lowercase()) && !s.chars().any(|c| c.is_uppercase()),
            ))
        },
        "str_isupper" => |args| {
            let s = arg_string(&args, 0)?;
            Ok(Value::Bool(
                !s.is_empty() && s.chars().any(|c| c.is_uppercase()) && !s.chars().any(|c| c.is_lowercase()),
            ))
        },
        "subprocess_run" => |args| {
            let cmd_arg = args.first().cloned().ok_or("subprocess.run: missing command")?;
            let (cmd, argv) = match &cmd_arg {
                Value::String(s) => (s.clone(), None),
                Value::List(l) => {
                    let mut parts = Vec::new();
                    for v in l {
                        parts.push(match v {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        });
                    }
                    if parts.is_empty() {
                        return Err("subprocess.run: empty command list".into());
                    }
                    (parts.remove(0), Some(parts))
                }
                _ => return Err("subprocess.run: command must be a string or list".into()),
            };
            let mut command = Command::new(&cmd);
            if let Some(argv) = &argv {
                command.args(argv);
            }
            if let Some(cwd) = args.get(1).and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Dict(d) => match d.get("cwd") {
                    Some(Value::String(s)) => Some(s.clone()),
                    _ => None,
                },
                _ => None,
            }) {
                command.current_dir(cwd);
            }
            let output = command
                .output()
                .map_err(|e| format!("subprocess.run {cmd}: {e}"))?;
            let mut result = BTreeMap::new();
            result.insert("ok".into(), Value::Bool(output.status.success()));
            result.insert("code".into(), Value::Number(output.status.code().unwrap_or(-1) as f64));
            result.insert(
                "stdout".into(),
                Value::String(String::from_utf8_lossy(&output.stdout).into_owned()),
            );
            result.insert(
                "stderr".into(),
                Value::String(String::from_utf8_lossy(&output.stderr).into_owned()),
            );
            Ok(Value::Dict(result))
        },
        "subprocess_call" => |args| {
            let cmd_arg = args.first().cloned().ok_or("subprocess.call: missing command")?;
            let (cmd, argv) = match &cmd_arg {
                Value::String(s) => (s.clone(), None),
                Value::List(l) => {
                    let mut parts: Vec<String> = l
                        .iter()
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect();
                    if parts.is_empty() {
                        return Err("subprocess.call: empty command list".into());
                    }
                    (parts.remove(0), Some(parts))
                }
                _ => return Err("subprocess.call: command must be a string or list".into()),
            };
            let mut command = Command::new(&cmd);
            if let Some(argv) = &argv {
                command.args(argv);
            }
            let status = command
                .status()
                .map_err(|e| format!("subprocess.call {cmd}: {e}"))?;
            Ok(Value::Number(status.code().unwrap_or(-1) as f64))
        },
        "subprocess_check_output" => |args| {
            let cmd_arg = args.first().cloned().ok_or("subprocess.check_output: missing command")?;
            let (cmd, argv) = match &cmd_arg {
                Value::String(s) => (s.clone(), None),
                Value::List(l) => {
                    let mut parts: Vec<String> = l
                        .iter()
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect();
                    if parts.is_empty() {
                        return Err("subprocess.check_output: empty command list".into());
                    }
                    (parts.remove(0), Some(parts))
                }
                _ => return Err("subprocess.check_output: command must be a string or list".into()),
            };
            let mut command = Command::new(&cmd);
            if let Some(argv) = &argv {
                command.args(argv);
            }
            let output = command
                .output()
                .map_err(|e| format!("subprocess.check_output {cmd}: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "subprocess.check_output: {cmd} failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            Ok(Value::String(String::from_utf8_lossy(&output.stdout).into_owned()))
        },
        "struct_pack" => |args| {
            let fmt = arg_string(&args, 0)?;
            let values = &args[1..];
            pack_impl(&fmt, values)
        },
        "struct_unpack" => |args| {
            let fmt = arg_string(&args, 0)?;
            let data = arg_string(&args, 1)?;
            unpack_impl(&fmt, data.as_bytes())
        },
        "struct_calcsize" => |args| {
            let fmt = arg_string(&args, 0)?;
            Ok(Value::Number(struct_size_of(&fmt)? as f64))
        },
        "hashlib_new" => |args| {
            let algo = arg_string(&args, 0)?.to_lowercase();
            let data = match args.get(1) {
                Some(Value::String(s)) => s.clone(),
                _ => String::new(),
            };
            let digest = match algo.as_str() {
                "md5" => crypto_digest(&data, "md5"),
                "sha1" => crypto_digest(&data, "sha1"),
                "sha224" => crypto_digest(&data, "sha224"),
                "sha256" => crypto_digest(&data, "sha256"),
                "sha384" => crypto_digest(&data, "sha384"),
                "sha512" => crypto_digest(&data, "sha512"),
                "sha3_256" => crypto_digest(&data, "sha3_256"),
                "sha3_512" => crypto_digest(&data, "sha3_512"),
                "blake2b" => crypto_digest(&data, "blake2b"),
                "blake2s" => crypto_digest(&data, "blake2s"),
                _ => return Err(format!("hashlib.new: unknown algorithm {algo}")),
            };
            Ok(Value::Dict(BTreeMap::from([
                ("hexdigest".into(), Value::String(digest)),
                ("name".into(), Value::String(algo)),
            ])))
        },
        "shutil_copy" => |args| {
            let src = arg_string(&args, 0)?;
            let dst = arg_string(&args, 1)?;
            fs::copy(&src, &dst).map_err(|e| format!("shutil.copy: {e}"))?;
            Ok(Value::Bool(true))
        },
        "shutil_copy2" => |args| {
            let src = arg_string(&args, 0)?;
            let dst = arg_string(&args, 1)?;
            fs::copy(&src, &dst).map_err(|e| format!("shutil.copy2: {e}"))?;
            let metadata = fs::metadata(&src).map_err(|e| format!("shutil.copy2: {e}"))?;
            fs::set_permissions(&dst, metadata.permissions())
                .map_err(|e| format!("shutil.copy2: {e}"))?;
            Ok(Value::Bool(true))
        },
        "shutil_move" => |args| {
            let src = arg_string(&args, 0)?;
            let dst = arg_string(&args, 1)?;
            fs::rename(&src, &dst).or_else(|_| {
                fs::copy(&src, &dst)?;
                fs::remove_file(&src)
            })
            .map_err(|e| format!("shutil.move: {e}"))?;
            Ok(Value::Bool(true))
        },
        "shutil_rmtree" => |args| {
            let path = arg_string(&args, 0)?;
            if fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false) {
                fs::remove_dir_all(&path).map_err(|e| format!("shutil.rmtree: {e}"))?;
            } else if fs::metadata(&path).is_ok() {
                fs::remove_file(&path).map_err(|e| format!("shutil.rmtree: {e}"))?;
            }
            Ok(Value::Bool(true))
        },
        "shutil_copytree" => |args| {
            let src = arg_string(&args, 0)?;
            let dst = arg_string(&args, 1)?;
            fs::create_dir_all(&dst).map_err(|e| format!("shutil.copytree: {e}"))?;
            copy_dir_recursive(Path::new(&src), Path::new(&dst))
                .map_err(|e| format!("shutil.copytree: {e}"))?;
            Ok(Value::Bool(true))
        },
        "shutil_which" => |args| {
            let name = arg_string(&args, 0)?;
            let path_env = env::var("PATH").unwrap_or_default();
            for dir in env::split_paths(&path_env) {
                let candidate = dir.join(&name);
                if candidate.is_file() {
                    return Ok(Value::String(candidate.to_string_lossy().into_owned()));
                }
            }
            Ok(Value::Null)
        },
        "shutil_disk_usage" => |args| {
            let path = arg_string(&args, 0)?;
            #[cfg(unix)]
            {
                use std::mem::MaybeUninit;
                let mut stat = MaybeUninit::<libc::statvfs>::uninit();
                let c_path = std::ffi::CString::new(path.clone())
                    .map_err(|_| "shutil.disk_usage: invalid path".to_string())?;
                if unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) } != 0 {
                    return Err(format!(
                        "shutil.disk_usage: {}",
                        std::io::Error::last_os_error()
                    ));
                }
                let stat = unsafe { stat.assume_init() };
                let bsize = stat.f_frsize as f64;
                let total = stat.f_blocks as f64 * bsize;
                let free = stat.f_bfree as f64 * bsize;
                let used = total - stat.f_bfree as f64 * bsize;
                let mut out = BTreeMap::new();
                out.insert("total".into(), Value::Number(total));
                out.insert("used".into(), Value::Number(used));
                out.insert("free".into(), Value::Number(free));
                return Ok(Value::Dict(out));
            }
            #[cfg(not(unix))]
            {
                let _ = path;
                Err("shutil.disk_usage: not supported on this platform".into())
            }
        },
        "pathlib_join" => |args| {
            let mut path = PathBuf::new();
            for v in args {
                match v {
                    Value::String(s) => path.push(s),
                    _ => return Err("pathlib.join: arguments must be strings".into()),
                }
            }
            Ok(Value::String(path.to_string_lossy().into_owned()))
        },
        "pathlib_name" => |args| {
            let s = arg_string(&args, 0)?;
            let p = Path::new(&s);
            Ok(Value::String(
                p.file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ))
        },
        "pathlib_parent" => |args| {
            let s = arg_string(&args, 0)?;
            let p = Path::new(&s);
            Ok(Value::String(
                p.parent()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ))
        },
        "pathlib_stem" => |args| {
            let s = arg_string(&args, 0)?;
            let p = Path::new(&s);
            Ok(Value::String(
                p.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ))
        },
        "pathlib_suffix" => |args| {
            let s = arg_string(&args, 0)?;
            let p = Path::new(&s);
            Ok(Value::String(
                p.extension()
                    .map(|s| format!(".{}", s.to_string_lossy()))
                    .unwrap_or_default(),
            ))
        },
        "pathlib_suffixes" => |args| {
            let s = arg_string(&args, 0)?;
            let p = Path::new(&s);
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut suffixes = Vec::new();
            let mut rest = name.as_str();
            while let Some(pos) = rest.rfind('.') {
                if pos == 0 {
                    break;
                }
                suffixes.insert(0, Value::String(rest[pos..].to_string()));
                rest = &rest[..pos];
            }
            Ok(Value::List(suffixes))
        },
        "pathlib_is_absolute" => |args| {
            let s = arg_string(&args, 0)?;
            let p = Path::new(&s);
            Ok(Value::Bool(p.is_absolute()))
        },
        "pathlib_resolve" => |args| {
            let s = arg_string(&args, 0)?;
            let p = Path::new(&s);
            let resolved = fs::canonicalize(p).map_err(|e| format!("pathlib.resolve: {e}"))?;
            Ok(Value::String(resolved.to_string_lossy().into_owned()))
        },
        "pathlib_absolute" => |args| {
            let s = arg_string(&args, 0)?;
            let p = Path::new(&s);
            let abs = if p.is_absolute() {
                p.to_path_buf()
            } else {
                env::current_dir()
                    .map_err(|e| format!("pathlib.absolute: {e}"))?
                    .join(p)
            };
            Ok(Value::String(abs.to_string_lossy().into_owned()))
        },
        "pathlib_exists" => |args| {
            let s = arg_string(&args, 0)?;
            let p = Path::new(&s);
            Ok(Value::Bool(p.exists()))
        },
        "pathlib_is_file" => |args| {
            let s = arg_string(&args, 0)?;
            let p = Path::new(&s);
            Ok(Value::Bool(p.is_file()))
        },
        "pathlib_is_dir" => |args| {
            let s = arg_string(&args, 0)?;
            let p = Path::new(&s);
            Ok(Value::Bool(p.is_dir()))
        },
        "pathlib_glob" => |args| {
            let pattern = arg_string(&args, 0)?;
            let path = Path::new(&pattern);
            let dir = path.parent().and_then(|p| p.to_str()).unwrap_or(".");
            let name = path.file_name().and_then(|f| f.to_str()).unwrap_or("*");
            let mut results = Vec::new();
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let file_name = entry.file_name().to_string_lossy().into_owned();
                    if simple_glob(name, &file_name) {
                        results.push(Value::String(
                            if dir == "." {
                                file_name
                            } else {
                                format!("{dir}/{file_name}")
                            },
                        ));
                    }
                }
            }
            Ok(Value::List(results))
        },
        "pathlib_touch" => |args| {
            let path = arg_string(&args, 0)?;
            let now = SystemTime::now();
            match fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&path)
                .and_then(|f| f.set_modified(now))
            {
                Ok(_) => Ok(Value::Bool(true)),
                Err(e) => Err(format!("pathlib.touch: {e}")),
            }
        },
        "pathlib_mkdir" => |args| {
            let path = arg_string(&args, 0)?;
            let parents = matches!(args.get(1), Some(Value::Bool(true)));
            if parents {
                fs::create_dir_all(&path).map_err(|e| format!("pathlib.mkdir: {e}"))?;
            } else {
                fs::create_dir(&path).map_err(|e| format!("pathlib.mkdir: {e}"))?;
            }
            Ok(Value::Bool(true))
        },
        "pathlib_rmdir" => |args| {
            let path = arg_string(&args, 0)?;
            fs::remove_dir(&path).map_err(|e| format!("pathlib.rmdir: {e}"))?;
            Ok(Value::Bool(true))
        },
        "pathlib_unlink" => |args| {
            let path = arg_string(&args, 0)?;
            fs::remove_file(&path).map_err(|e| format!("pathlib.unlink: {e}"))?;
            Ok(Value::Bool(true))
        },
        "pathlib_rename" => |args| {
            let src = arg_string(&args, 0)?;
            let dst = arg_string(&args, 1)?;
            fs::rename(&src, &dst).map_err(|e| format!("pathlib.rename: {e}"))?;
            Ok(Value::Bool(true))
        },
        "pathlib_read_text" => |args| {
            let path = arg_string(&args, 0)?;
            fs::read_to_string(&path).map(Value::String).map_err(|e| format!("pathlib.read_text: {e}"))
        },
        "pathlib_write_text" => |args| {
            let path = arg_string(&args, 0)?;
            let content = arg_string(&args, 1)?;
            fs::write(&path, content).map_err(|e| format!("pathlib.write_text: {e}"))?;
            Ok(Value::Bool(true))
        },
        "urllib_urlopen" => |args| {
            let url = arg_string(&args, 0)?;
            let resp = http_request_impl(&[Value::String(url)], "GET")?;
            Ok(resp)
        },
        "urllib_quote" => |args| {
            let s = arg_string(&args, 0)?;
            Ok(Value::String(url_quote(&s)))
        },
        "urllib_unquote" => |args| {
            let s = arg_string(&args, 0)?;
            Ok(Value::String(url_unquote(&s)))
        },
        "urllib_urlencode" => |args| {
            let d = arg_dict(&args, 0)?;
            let mut parts = Vec::new();
            for (k, v) in d {
                parts.push(format!("{}={}", url_quote(&k), url_quote(&v.to_string())));
            }
            Ok(Value::String(parts.join("&")))
        },
        "urllib_parse" => |args| {
            let url = arg_string(&args, 0)?;
            let mut out = BTreeMap::new();
            let rest = match url.find("://") {
                Some(i) => {
                    out.insert("scheme".into(), Value::String(url[..i].to_string()));
                    &url[i + 3..]
                }
                None => url.as_str(),
            };
            let (host_part, path_part) = match rest.find('/') {
                Some(i) => (&rest[..i], &rest[i..]),
                None => (rest, ""),
            };
            let (host, port) = match host_part.rfind(':') {
                Some(i) if host_part[i + 1..].chars().all(|c| c.is_ascii_digit()) => (
                    host_part[..i].to_string(),
                    host_part[i + 1..].parse::<f64>().unwrap_or(0.0),
                ),
                _ => (host_part.to_string(), 0.0),
            };
            out.insert("host".into(), Value::String(host));
            if port > 0.0 {
                out.insert("port".into(), Value::Number(port));
            }
            let (path, query) = match path_part.find('?') {
                Some(i) => (&path_part[..i], &path_part[i + 1..]),
                None => (path_part, ""),
            };
            out.insert("path".into(), Value::String(path.to_string()));
            if !query.is_empty() {
                out.insert("query".into(), Value::String(query.to_string()));
            }
            Ok(Value::Dict(out))
        },
        "urllib_parse_qs" => |args| {
            let query = arg_string(&args, 0)?;
            let mut out = BTreeMap::new();
            for pair in query.split('&') {
                if pair.is_empty() {
                    continue;
                }
                let (k, v) = match pair.find('=') {
                    Some(i) => (&pair[..i], &pair[i + 1..]),
                    None => (pair, ""),
                };
                out.entry(url_unquote(k))
                    .and_modify(|e| {
                        if let Value::List(l) = e {
                            l.push(Value::String(url_unquote(v)));
                        }
                    })
                    .or_insert_with(|| {
                        Value::List(vec![Value::String(url_unquote(v))])
                    });
            }
            Ok(Value::Dict(out))
        },
        "collections_counter" => |args| {
            let items = arg_list(&args, 0)?;
            let mut counts = BTreeMap::new();
            for item in items {
                let key = item.to_string();
                let entry = counts.entry(key).or_insert(0.0);
                *entry += 1.0;
            }
            let mut out = BTreeMap::new();
            for (k, v) in counts {
                out.insert(k, Value::Number(v));
            }
            Ok(Value::Dict(out))
        },
        "collections_chain" => |args| {
            let mut out = Vec::new();
            for arg in args {
                if let Value::List(l) = arg {
                    out.extend(l.iter().cloned());
                } else {
                    out.push(arg.clone());
                }
            }
            Ok(Value::List(out))
        },
        "collections_flatten" => |args| {
            let list = arg_list(&args, 0)?;
            let mut out = Vec::new();
            flatten_list(&list, &mut out);
            Ok(Value::List(out))
        },
        "itertools_enumerate" => |args| {
            let list = arg_list(&args, 0)?;
            let mut out = Vec::new();
            for (i, item) in list.into_iter().enumerate() {
                out.push(Value::List(vec![Value::Number(i as f64), item]));
            }
            Ok(Value::List(out))
        },
        "itertools_zip" => |args| {
            let a = arg_list(&args, 0)?;
            let b = arg_list(&args, 1)?;
            let mut out = Vec::new();
            let n = a.len().min(b.len());
            for i in 0..n {
                out.push(Value::List(vec![a[i].clone(), b[i].clone()]));
            }
            Ok(Value::List(out))
        },
        "itertools_chain" => |args| {
            let mut out = Vec::new();
            for arg in args {
                if let Value::List(l) = arg {
                    out.extend(l.iter().cloned());
                }
            }
            Ok(Value::List(out))
        },
        "itertools_repeat" => |args| {
            let value = args.first().cloned().ok_or("itertools.repeat: missing value")?;
            let n = match args.get(1) {
                Some(Value::Number(x)) => *x as usize,
                _ => return Err("itertools.repeat: needs a count".into()),
            };
            Ok(Value::List(vec![value; n]))
        },
        "itertools_product" => |args| {
            let a = arg_list(&args, 0)?;
            let b = arg_list(&args, 1)?;
            let mut out = Vec::new();
            for x in &a {
                for y in &b {
                    out.push(Value::List(vec![x.clone(), y.clone()]));
                }
            }
            Ok(Value::List(out))
        },
        "itertools_permutations" => |args| {
            let list = arg_list(&args, 0)?;
            let r = match args.get(1) {
                Some(Value::Number(n)) => *n as usize,
                _ => list.len(),
            };
            let mut out = Vec::new();
            permutations(&list, r, &mut vec![], &mut vec![false; list.len()], &mut out);
            Ok(Value::List(out))
        },
        "itertools_combinations" => |args| {
            let list = arg_list(&args, 0)?;
            let r = arg_number(&args, 1)? as usize;
            let mut out = Vec::new();
            combinations(&list, r, 0, &mut vec![], &mut out);
            Ok(Value::List(out))
        },
        "itertools_accumulate" => |args| {
            let list = arg_list(&args, 0)?;
            let mut out = Vec::new();
            let mut acc = 0.0;
            for item in &list {
                match item {
                    Value::Number(n) => acc += n,
                    other => {
                        if let Value::String(s) = other {
                            if let Ok(n) = s.parse::<f64>() {
                                acc += n;
                            }
                        }
                    }
                }
                out.push(Value::Number(acc));
            }
            Ok(Value::List(out))
        },
        "itertools_take" => |args| {
            let n = arg_number(&args, 0)? as usize;
            let list = arg_list(&args, 1)?;
            Ok(Value::List(list.into_iter().take(n).collect()))
        },
        "itertools_drop" => |args| {
            let n = arg_number(&args, 0)? as usize;
            let list = arg_list(&args, 1)?;
            Ok(Value::List(list.into_iter().skip(n).collect()))
        },
        "itertools_range" => |args| {
            let start = match args.first() {
                Some(Value::Number(n)) => *n,
                _ => 0.0,
            };
            let stop = match args.get(1) {
                Some(Value::Number(n)) => *n,
                _ => start,
            };
            let step = match args.get(2) {
                Some(Value::Number(n)) => *n,
                _ => 1.0,
            };
            if step == 0.0 {
                return Err("itertools.range: step cannot be 0".into());
            }
            let real_start = if args.len() >= 2 { start } else { 0.0 };
            let real_stop = if args.len() >= 2 { stop } else { start };
            let mut out = Vec::new();
            let mut i = real_start;
            if step > 0.0 {
                while i < real_stop {
                    out.push(Value::Number(i));
                    i += step;
                }
            } else {
                while i > real_stop {
                    out.push(Value::Number(i));
                    i += step;
                }
            }
            Ok(Value::List(out))
        },
        "tempfile_dir" => |_| {
            Ok(Value::String(env::temp_dir().to_string_lossy().into_owned()))
        },
        "tempfile_mkdtemp" => |args| {
            let prefix = match args.first() {
                Some(Value::String(s)) => s.clone(),
                _ => "zen".into(),
            };
            let dir = env::temp_dir().join(format!("{prefix}{}", rand::random::<u64>()));
            fs::create_dir_all(&dir).map_err(|e| format!("tempfile.mkdtemp: {e}"))?;
            Ok(Value::String(dir.to_string_lossy().into_owned()))
        },
        "tempfile_mkstemp" => |args| {
            let prefix = match args.first() {
                Some(Value::String(s)) => s.clone(),
                _ => "zen".into(),
            };
            let path = env::temp_dir().join(format!("{prefix}{}", rand::random::<u64>()));
            fs::write(&path, "").map_err(|e| format!("tempfile.mkstemp: {e}"))?;
            Ok(Value::String(path.to_string_lossy().into_owned()))
        },
        "binascii_hexlify" => |args| Ok(Value::String(hexlify(arg_string(&args, 0)?.as_bytes()))),
        "binascii_unhexlify" => |args| {
            let hex = arg_string(&args, 0)?;
            if hex.len() % 2 != 0 {
                return Err("binascii.unhexlify: odd-length string".into());
            }
            let mut out = Vec::with_capacity(hex.len() / 2);
            for i in (0..hex.len()).step_by(2) {
                let b = u8::from_str_radix(&hex[i..i + 2], 16)
                    .map_err(|_| "binascii.unhexlify: non-hex character".to_string())?;
                out.push(b);
            }
            Ok(Value::String(String::from_utf8_lossy(&out).into_owned()))
        },
        "binascii_a2b_base64" => |args| {
            use base64::Engine;
            let s = arg_string(&args, 0)?;
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(s.trim())
                .map_err(|e| format!("binascii.a2b_base64: {e}"))?;
            Ok(Value::String(String::from_utf8_lossy(&decoded).into_owned()))
        },
        "binascii_b2a_base64" => |args| {
            use base64::Engine;
            let s = arg_string(&args, 0)?;
            let encoded = base64::engine::general_purpose::STANDARD.encode(s.as_bytes());
            Ok(Value::String(encoded))
        },
        _ => |_| Ok(Value::String("Native Call".into())),
    }
}

#[allow(dead_code)]
pub fn run(source: &str) -> Result<(), String> {
    run_named(source, "<string>")
}

/// Run a script, reporting errors against a named source file.
pub fn run_named(source: &str, file: &str) -> Result<(), String> {
    let tokens = lex(source)?;
    let program = Parser::new(tokens).program()?;
    let mut vm = Vm::new();
    vm.file = file.into();
    vm.lines = source.lines().map(|l| l.to_string()).collect();
    if let Some(prelude) = find_std_file("browser.z") {
        let stmts = parse_file(&prelude)?;
        vm.exec(&stmts)?;
    }
    let flow = vm.exec(&program)?;
    match flow {
        Flow::Normal => Ok(()),
        Flow::Return(_) => Err("return used outside a function".into()),
        Flow::Break => Err("break used outside a loop".into()),
        Flow::Continue => Err("continue used outside a loop".into()),
        Flow::Throw(value) => {
            let (ty, msg) = vm.error_info(&value);
            let (file, line, col) = error_location(&value);
            Err(format_unhandled(source, &file, line, col, &ty, &msg))
        }
    }
}

fn error_location(value: &Value) -> (String, usize, usize) {
    match value {
        Value::Dict(map) => (
            map.get("file").map(|v| v.to_string()).unwrap_or_default(),
            map.get("line")
                .and_then(|v| match v {
                    Value::Number(n) => Some(*n as usize),
                    _ => None,
                })
                .unwrap_or(0),
            map.get("col")
                .and_then(|v| match v {
                    Value::Number(n) => Some(*n as usize),
                    _ => None,
                })
                .unwrap_or(0),
        ),
        _ => (String::new(), 0, 0),
    }
}

fn format_unhandled(
    source: &str,
    file: &str,
    line: usize,
    col: usize,
    ty: &str,
    msg: &str,
) -> String {
    let mut out = String::from("Traceback (most recent call last):\n");
    out.push_str(&format!("  File \"{file}\", line {line}, in <module>\n"));
    if line > 0 {
        if let Some(src_line) = source.lines().nth(line.wrapping_sub(1)) {
            let trimmed = src_line.trim();
            if !trimmed.is_empty() {
                out.push_str(&format!("    {trimmed}\n"));
                let pad = " ".repeat(4 + col.saturating_sub(1).min(trimmed.chars().count()));
                out.push_str(&format!("{pad}^\n"));
            }
        }
    }
    out.push_str(&format!("{ty}: {msg}\n"));
    out
}

/// Interactive session state that persists across evaluated lines.
pub struct Repl {
    vm: Vm,
    initialized: bool,
}

impl Repl {
    pub fn new() -> Result<Repl, String> {
        let mut vm = Vm::new();
        if let Some(prelude) = find_std_file("browser.z") {
            let stmts = parse_file(&prelude)?;
            vm.exec(&stmts)?;
        }
        Ok(Repl { vm, initialized: true })
    }

    /// Evaluate one line, keeping variables/functions defined in earlier lines.
    pub fn eval_line(&mut self, line: &str) -> Result<(), String> {
        if !self.initialized {
            return Err("repl is not initialized".into());
        }
        self.vm.file = "<repl>".into();
        self.vm.lines = line.lines().map(|l| l.to_string()).collect();
        // Try as a statement first; fall back to expression-print if it's
        // just an expression (e.g. `5 + 5`).
        match lex(line) {
            Err(e) => return Err(e),
            Ok(tokens) => {
                let program = match Parser::new(tokens).program() {
                    Ok(p) => p,
                    Err(_) => {
                        // Fall back to evaluating as a bare expression and printing.
                        match self.vm.eval_expr_source(line) {
                            Ok(value) => {
                                println!("{}", value.to_string());
                                return Ok(());
                            }
                            Err(expr_err) => return Err(expr_err),
                        }
                    }
                };
                // A single expression statement evaluates to a value we should print.
                if program.len() == 1 {
                    if let StmtKind::Expr(e) = &program[0].kind {
                        match self.vm.eval(e) {
                            Ok(value) => {
                                println!("{}", value.to_string());
                                return Ok(());
                            }
                            Err(err) => return Err(err),
                        }
                    }
                }
                match self.vm.exec(&program) {
                    Ok(Flow::Normal) => Ok(()),
                    Ok(Flow::Return(_)) => Err("return used outside a function".into()),
                    Ok(Flow::Break) => Err("break used outside a loop".into()),
                    Ok(Flow::Continue) => Err("continue used outside a loop".into()),
                    Ok(Flow::Throw(value)) => {
                        let (ty, msg) = self.vm.error_info(&value);
                        let (file, line, col) = error_location(&value);
                        let source = self
                            .vm
                            .lines
                            .get(line.saturating_sub(1))
                            .map(|s| s.as_str())
                            .unwrap_or("");
                        Err(format_unhandled(source, &file, line, col, &ty, &msg))
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }
}

/// Return general REPL help text.
pub fn repl_help() -> &'static str {
    "Zen REPL — interactive session\n\n\
     REPL Commands:\n\
       :help               show this help\n\
       :help modules       list all available modules\n\
       :help <module>      detailed help for a module\n\
       :help types         list all data types\n\
       :help functions     list all built-in functions\n\
       :help operators     list all operators\n\
       :help keywords      list all keywords\n\
       :c <expr>           shorthand for: print <expr>\n\
       :q / :quit / :exit  quit the REPL\n\n\
     Keyboard shortcuts:\n\
       Up/Down             cycle through command history\n\
       Left/Right          move cursor\n\
       Tab                 auto-complete (when available)\n\
       Ctrl-A              move to start of line\n\
       Ctrl-E              move to end of line\n\
       Ctrl-K              delete to end of line\n\
       Ctrl-U              delete to start of line\n\
       Ctrl-C              cancel current input\n\
       Ctrl-D              exit (EOF)\n\n\
     Importing modules:\n\
       All built-in modules are available as globals — no import needed.\n\
       Use them directly: string.upper(\"hello\"), hashlib.md5(\"x\"), etc.\n\
       Custom modules: save a .z file, then: import mymodule"
}

/// Return list of all module names with one-line descriptions.
pub fn list_modules() -> String {
    let modules = [
        ("errors", "Python-style error classes with inheritance and typed catch"),
        ("json", "JSON encode/decode (parse, stringify, pretty)"),
        ("fs", "Filesystem operations (read, write, exists, list_dir, mkdir, etc.)"),
        ("re", "Regular expressions (match, find, findall, replace, split)"),
        ("random", "Random number generation (int, float, choice, shuffle, seed)"),
        ("math", "Math constants and functions (sin, cos, sqrt, floor, etc.)"),
        ("time", "Time functions (now, format, parse, sleep, etc.)"),
        ("os", "OS info and process control (platform, env, execute)"),
        ("base64", "Base64 encode/decode (encode, decode)"),
        ("crypto", "Cryptographic hashes (sha256, md5, sha1, etc.)"),
        ("datetime", "Date/time objects and formatting"),
        ("uuid", "UUID generation (v1, v3, v4, v5)"),
        ("color", "ANSI color helpers (rgb, hex, 256-color, styled text)"),
        ("csv", "CSV parsing and writing"),
        ("http", "HTTP client (get, post, put, del, head, patch)"),
        ("decimal", "Arbitrary-precision decimal arithmetic"),
        ("threading", "Background function execution"),
        ("statistics", "Statistical functions (mean, median, stdev, etc.)"),
        ("socket", "Low-level TCP sockets (open, send, recv, close)"),
        ("browser", "Browser automation over CDP (Chrome DevTools Protocol)"),
        ("string", "String helpers and constants (upper, split, join, replace, etc.)"),
        ("subprocess", "Run external commands (run, call, check_output)"),
        ("hashlib", "Cryptographic hashing (sha256, md5, create, algorithms_available)"),
        ("struct", "Binary data pack/unpack (pack, unpack, calcsize)"),
        ("shutil", "High-level file operations (copy, move, rmtree, which, disk_usage)"),
        ("pathlib", "Path manipulation (join, name, parent, stem, suffix, exists, etc.)"),
        ("glob", "File pattern matching (glob)"),
        ("urllib", "URL handling (parse, quote, unquote, urlencode, urlopen)"),
        ("collections", "Data structures (Counter, chain, flatten)"),
        ("itertools", "Iterators (enumerate, zip, range, product, combinations, etc.)"),
        ("tempfile", "Temporary files/dirs (dir, mkdtemp, mkstemp)"),
        ("binascii", "Binary/ASCII encoding (hexlify, unhexlify, base64)"),
        ("socket", "Low-level networking (open, send, recv, close)"),
        ("ftp", "Pure-Rust FTP client (connect, login, list, retr, stor, etc.)"),
        ("smtp", "Pure-Rust SMTP client (connect, login, sendmail, message)"),
        ("pop3", "Pure-Rust POP3 client (connect, stat, list, retr, dele)"),
        ("imap", "Pure-Rust IMAP client (connect, select, search, fetch)"),
        ("telnet", "Pure-Rust telnet client (connect, write, read, read_until)"),
        ("dns", "DNS resolver (resolve, query)"),
        ("ssh", "System SSH/SCP wrapper (run, upload, download, available)"),
        ("scapy", "Packet crafting/sniffing (ip, tcp, udp, build, parse, send, sniff)"),
    ];
    let mut out = String::from("Available modules (all available as globals, no import needed):\n\n");
    for (name, desc) in modules {
        out.push_str(&format!("  {:<14} {}\n", name, desc));
    }
    out.push_str("\nUsage: <module>.<function>(args...)\n");
    out.push_str("Example: string.upper(\"hello\")\n");
    out.push_str("Example: hashlib.sha256(\"data\")\n");
    out
}

/// Return help for a specific module.
pub fn module_help(name: &str) -> Option<String> {
    match name {
        "errors" => Some(
            "errors — Python-style error classes with inheritance and typed catch\n\n\
             Classes: Exception, RuntimeError, ValueError, TypeError, IndexError, KeyError,\n\
             FileNotFoundError, PermissionError, ZeroDivisionError, ArithmeticError,\n\
             OverflowError, NotImplementedError, StopIteration, AssertionError,\n\
             AttributeError, SyntaxError, ImportError, RecursionError, BufferError,\n\
             OSError, ConnectionError, TimeoutError, EOFError, MemoryError,\n\
             DeprecationWarning, FutureWarning\n\n\
             Creating custom errors:\n\
             errors.define(\"MyError\", \"Exception\", \"Something went wrong\")\n\
             throw MyError(\"details\")\n\
             catch MyError as e { print e.message }\n\n\
             Typed catch:\n\
             catch ValueError as e { ... } catch TypeError as e { ... }"
                .into(),
        ),
        "json" => Some(
            "json — JSON encode/decode\n\n\
             json.parse(string)          decode JSON string to dict/list\n\
             json.stringify(value)        encode value to JSON string\n\
             json.pretty(value, indent?)  encode with indentation (default 2)\n\
             json.is_valid(string)        check if string is valid JSON"
                .into(),
        ),
        "fs" => Some(
            "fs — Filesystem operations\n\n\
             fs.read(path)               read file to string\n\
             fs.write(path, data)        write string to file\n\
             fs.read_binary(path)        read file to binary string\n\
             fs.write_binary(path, data) write binary string to file\n\
             fs.exists(path)             check if path exists\n\
             fs.is_file(path)            check if path is a file\n\
             fs.is_dir(path)             check if path is a directory\n\
             fs.list_dir(path)           list directory contents\n\
             fs.mkdir(path)              create directory\n\
             fs.rmdir(path)              remove empty directory\n\
             fs.rmtree(path)             remove directory tree\n\
             fs.remove(path)             delete file\n\
             fs.copy(src, dst)           copy file\n\
             fs.move(src, dst)           move/rename file\n\
             fs.size(path)               file size in bytes\n\
             fs.mtime(path)              modification time\n\
             fs.append(path, data)       append to file\n\
             fs.glob(pattern)            glob pattern match\n\
             fs.join(parts...)           join path components\n\
             fs.basename(path)           filename from path\n\
             fs.dirname(path)            directory from path\n\
             fs.extension(path)          file extension\n\
             fs.cwd()                    current working directory\n\
             fs.home()                   user home directory"
                .into(),
        ),
        "re" => Some(
            "re — Regular expressions\n\n\
             re.match(pattern, string)     match at start (returns list or null)\n\
             re.find(pattern, string)      find first match\n\
             re.findall(pattern, string)   find all matches (list of strings)\n\
             re.replace(pattern, str, rep) replace matches\n\
             re.split(pattern, string)     split by pattern\n\
             re.search(pattern, string)    search anywhere in string"
                .into(),
        ),
        "random" => Some(
            "random — Random number generation\n\n\
             random.int(min, max)       random integer in [min, max]\n\
             random.float(min?, max?)   random float (default 0.0-1.0)\n\
             random.choice(list)        random element from list\n\
             random.shuffle(list)       shuffle list in place\n\
             random.seed(n?)            set random seed\n\
             random.gauss(mu, sigma)    Gaussian distribution"
                .into(),
        ),
        "math" => Some(
            "math — Math functions and constants\n\n\
             Constants: pi, e, tau, inf, nan\n\n\
             Trigonometry: sin, cos, tan, asin, acos, atan, atan2\n\
             Hyperbolic: sinh, cosh, tanh\n\
             Rounding: floor, ceil, trunc, round\n\
             Roots/powers: sqrt, cbrt, pow\n\
             Logarithmic: log, log2, log10, log1p\n\
             Other: fabs, fmod, gcd, lcm, factorial, comb, perm\n\
             Comparison: isclose(a, b, rel_tol?, abs_tol?)"
                .into(),
        ),
        "time" => Some(
            "time — Time functions\n\n\
             time.now()              current timestamp (seconds since epoch)\n\
             time.unix()             same as now()\n\
             time.utc()              current UTC time string\n\
             time.date()             current date string\n\
             time.format(ts, fmt?)   format timestamp (default ISO)\n\
             time.parse(str, fmt?)   parse time string\n\
             time.sleep(sec)         sleep seconds\n\
             time.wait(ms)           sleep milliseconds\n\
             time.year()             current year\n\
             time.month()            current month\n\
             time.day()              current day\n\
             time.hour()             current hour\n\
             time.minute()           current minute\n\
             time.second()           current second\n\
             time.weekday()          day of week (0=Mon)\n\
             time.from_unix(n)       timestamp to dict\n\
             time.add_days(ts, n)    add days to timestamp"
                .into(),
        ),
        "os" => Some(
            "os — OS interaction\n\n\
             os.platform()            OS name (linux, macos, windows)\n\
             os.env(key?)             environment variable or all env vars\n\
             os.setenv(key, val)      set environment variable\n\
             os.execute(cmd)          run shell command, return {ok, code, stdout, stderr}\n\
             os.args()                command line arguments\n\
             os.cwd()                 current working directory\n\
             os.chdir(path)           change directory\n\
             os.kill(pid, signal?)    send signal to process\n\
             os.pids()                list process IDs\n\
             os.hostname()            machine hostname\n\
             os.arch()                CPU architecture"
                .into(),
        ),
        "base64" => Some(
            "base64 — Base64 encoding\n\n\
             base64.encode(data)    encode to base64 string\n\
             base64.decode(data)    decode base64 string"
                .into(),
        ),
        "crypto" => Some(
            "crypto — Cryptographic hashing\n\n\
             crypto.sha256(data)    SHA-256 hex digest\n\
             crypto.sha1(data)      SHA-1 hex digest\n\
             crypto.md5(data)       MD5 hex digest\n\
             crypto.sha512(data)    SHA-512 hex digest\n\
             crypto.sha224(data)    SHA-224 hex digest\n\
             crypto.sha384(data)    SHA-384 hex digest\n\
             crypto.sha3_256(data)  SHA3-256 hex digest\n\
             crypto.sha3_512(data)  SHA3-512 hex digest\n\
             crypto.blake2b(data)   BLAKE2b hex digest\n\
             crypto.blake2s(data)   BLAKE2s hex digest"
                .into(),
        ),
        "hashlib" => Some(
            "hashlib — Cryptographic hashing (Python-compatible)\n\n\
             hashlib.sha256(data)      SHA-256 hex digest\n\
             hashlib.md5(data)         MD5 hex digest\n\
             hashlib.sha1(data)        SHA-1 hex digest\n\
             hashlib.sha512(data)      SHA-512 hex digest\n\
             hashlib.sha224(data)      SHA-224 hex digest\n\
             hashlib.sha384(data)      SHA-384 hex digest\n\
             hashlib.sha3_256(data)    SHA3-256 hex digest\n\
             hashlib.sha3_512(data)    SHA3-512 hex digest\n\
             hashlib.blake2b(data)     BLAKE2b hex digest\n\
             hashlib.blake2s(data)     BLAKE2s hex digest\n\
             hashlib.create(algo, data) returns {hexdigest, name}\n\
             hashlib.pbkdf2_hmac(...)  key derivation\n\
             hashlib.algorithms_available  list of algorithm names"
                .into(),
        ),
        "uuid" => Some(
            "uuid — UUID generation\n\n\
             uuid.v1()     time-based UUID\n\
             uuid.v3(name) name-based UUID (MD5)\n\
             uuid.v4()     random UUID\n\
             uuid.v5(name) name-based UUID (SHA-1)\n\
             uuid.str(uuid) format UUID to string"
                .into(),
        ),
        "color" => Some(
            "color — ANSI color helpers\n\n\
             color.rgb(r, g, b, text)       24-bit RGB color\n\
             color.hex(hex, text)            hex color\n\
             color.c256(n, text)             256-color palette\n\
             color.red(text) / green / blue / yellow / cyan / magenta / white / gray\n\
             color.bold(text) / dim / italic / underline / blink / reverse\n\
             color.reset()                   reset all styling\n\
             color.strip(text)               strip ANSI escape codes"
                .into(),
        ),
        "csv" => Some(
            "csv — CSV parsing and writing\n\n\
             csv.parse(text)              parse CSV to list of dicts\n\
             csv.parse_rows(text)         parse CSV to list of lists\n\
             csv.stringify(headers, rows)  encode to CSV string\n\
             csv.read(path)               read CSV file\n\
             csv.write(path, headers, rows) write CSV file"
                .into(),
        ),
        "http" => Some(
            "http — HTTP client\n\n\
             http.get(url, headers?)       GET request\n\
             http.post(url, body, h?)      POST request\n\
             http.put(url, body, h?)       PUT request\n\
             http.del(url, h?)             DELETE request\n\
             http.head(url, h?)            HEAD request\n\
             http.patch(url, body, h?)     PATCH request\n\n\
             Response: {status, ok, body, text, json, headers}\n\
             Body types: response.json(), response.text()"
                .into(),
        ),
        "decimal" => Some(
            "decimal — Arbitrary-precision decimal arithmetic\n\n\
             decimal.make(value)     create Decimal from string/number\n\
             decimal.add(a, b)       addition\n\
             decimal.sub(a, b)       subtraction\n\
             decimal.mul(a, b)       multiplication\n\
             decimal.div(a, b)       division\n\
             decimal.cmp(a, b)       compare (-1, 0, 1)\n\
             decimal.to_str(d)       to string with precision"
                .into(),
        ),
        "threading" => Some(
            "threading — Background function execution\n\n\
             threading.run(func, args?)  run function in background thread\n\
             threading.sleep(sec)        sleep current thread\n\
             threading.yield_now()       yield to scheduler"
                .into(),
        ),
        "statistics" => Some(
            "statistics — Statistical functions\n\n\
             statistics.mean(list)        arithmetic mean\n\
             statistics.median(list)      median value\n\
             statistics.mode(list)        most common value\n\
             statistics.stdev(list)       sample standard deviation\n\
             statistics.variance(list)    sample variance\n\
             statistics.pstdev(list)      population standard deviation\n\
             statistics.pvariance(list)   population variance\n\
             statistics.quantiles(list)   quartile boundaries\n\
             statistics.correlation(x, y) Pearson correlation\n"
                .into(),
        ),
        "socket" => Some(
            "socket — Low-level TCP networking\n\n\
             socket.open(host, port)    connect to host:port, returns session\n\
             socket.send(session, data) send data (string or bytes)\n\
             socket.recv(session, n?)   receive up to n bytes (default 4096)\n\
             socket.close(session)      close connection"
                .into(),
        ),
        "browser" => Some(
            "browser — Browser automation (Chrome DevTools Protocol)\n\n\
             browser.go(url)              navigate to URL\n\
             browser.click(selector)      click element\n\
             browser.fill(sel, val)       fill input field\n\
             browser.text(sel?)           get text content\n\
             browser.attr(sel, name)      get element attribute\n\
             browser.wait_for(sel, ms?)   wait for element\n\
             browser.shot(path?)          screenshot\n\
             browser.title()              page title\n\
             browser.url()                current URL\n\
             browser.page()               page HTML\n\
             browser.eval(js)             evaluate JavaScript"
                .into(),
        ),
        "string" => Some(
            "string — String helpers and constants\n\n\
             Case:      upper(s), lower(s), title(s), capitalize(s), swapcase(s)\n\
             Trim:      strip(s), lstrip(s), rstrip(s)\n\
             Split:     split(s, sep?), splitlines(s)\n\
             Join:      join(sep, list)\n\
             Replace:   replace(s, old, new)\n\
             Search:    count(s, sub), find(s, sub), rfind(s, sub)\n\
             Test:      startswith(s, prefix), endswith(s, suffix), contains(s, sub)\n\
             Pad:       ljust(s, w, fill?), rjust(s, w, fill?), center(s, w, fill?), zfill(s, w)\n\
             Repeat:    repeat(s, n)\n\
             Check:     isdigit(s), isalpha(s), isalnum(s), isspace(s), islower(s), isupper(s)\n\n\
             Constants: digits, hexdigits, octdigits, ascii_letters, ascii_lowercase,\n\
                        ascii_uppercase, punctuation, whitespace, printable"
                .into(),
        ),
        "subprocess" => Some(
            "subprocess — Run external commands\n\n\
             subprocess.run(cmd, cwd?)         run command, returns {ok, code, stdout, stderr}\n\
             subprocess.call(cmd)              run and return exit code\n\
             subprocess.check_output(cmd)      run and return stdout (throws on error)\n\n\
             cmd can be a string (shell) or list of strings (no shell).\n\
             Example: subprocess.run([\"ls\", \"-la\"], null)\n\
             Example: subprocess.run(\"echo hello\", null)"
                .into(),
        ),
        "struct" => Some(
            "struct — Binary data packing/unpacking\n\n\
             struct.pack(fmt, values...)   pack values to binary string\n\
             struct.unpack(fmt, data)      unpack binary string to list\n\
             struct.calcsize(fmt)          size in bytes for format\n\n\
             Format characters:\n\
               b/B  signed/unsigned 8-bit integer (i8/u8)\n\
               h/H  signed/unsigned 16-bit integer (i16/u16)\n\
               i/I  signed/unsigned 32-bit integer (i32/u32)\n\
               q/Q  signed/unsigned 64-bit integer (i64/u64)\n\
               f    32-bit float\n\
               d    64-bit float\n\
               s    string (with size, e.g. 4s for 4-byte string)\n\
               x    pad byte (no output)\n\
               ?    boolean\n\n\
             Byte order prefix:\n\
               >    big-endian (network byte order)\n\
               <    little-endian\n\
               =    native byte order\n\n\
             Example: struct.pack(\">HHL\", 1, 2, 3)\n\
             Example: struct.unpack(\">HHL\", packed_data)"
                .into(),
        ),
        "shutil" => Some(
            "shutil — High-level file operations\n\n\
             shutil.copy(src, dst)           copy file\n\
             shutil.copy2(src, dst)          copy file with metadata\n\
             shutil.move(src, dst)           move/rename file\n\
             shutil.rmtree(path)             recursively remove directory tree\n\
             shutil.copytree(src, dst)       recursively copy directory\n\
             shutil.which(name)              find executable in PATH\n\
             shutil.disk_usage(path)         returns {total, used, free}"
                .into(),
        ),
        "pathlib" => Some(
            "pathlib — Path manipulation\n\n\
             Join:     join(parts...)\n\
             Parts:    name(path), parent(path), stem(path), suffix(path), suffixes(path)\n\
             Test:     is_absolute(path), exists(path), is_file(path), is_dir(path)\n\
             Resolve:  resolve(path), absolute(path)\n\
             Find:     glob(path, pattern)\n\
             Create:   touch(path), mkdir(path, parents?)\n\
             Delete:   rmdir(path), unlink(path)\n\
             Modify:   rename(src, dst)\n\
             Read:     read_text(path)\n\
             Write:    write_text(path, data)\n\n\
             Example: pathlib.name(\"/home/user/file.txt\")  =>  \"file.txt\"\n\
             Example: pathlib.join(\"/home\", \"user\", \"file.txt\")"
                .into(),
        ),
        "glob" => Some(
            "glob — File pattern matching\n\n\
             glob.glob(pattern)   match files, returns list of paths\n\n\
             Pattern syntax:\n\
               *      matches any characters except /\n\
               **     matches any characters including /\n\
               ?      matches single character\n\
               [abc]  matches one of a, b, or c\n\
               [a-z]  matches range\n\n\
             Example: glob.glob(\"*.z\")\n\
             Example: glob.glob(\"**/*.rs\")"
                .into(),
        ),
        "urllib" => Some(
            "urllib — URL handling\n\n\
             urllib.urlopen(url)              HTTP GET, returns response\n\
             urllib.parse(url)                parse URL => {scheme, host, port, path, query}\n\
             urllib.parse_qs(query)           parse query string => {key: [val, ...]}\n\
             urllib.quote(s)                  percent-encode string\n\
             urllib.unquote(s)                percent-decode string\n\
             urllib.urlencode(dict)           encode dict to query string\n\n\
             Example: urllib.parse(\"https://example.com:8080/path?q=1\")\n\
             Example: urllib.urlencode({\"name\": \"zen\", \"ver\": \"1\"})"
                .into(),
        ),
        "collections" => Some(
            "collections — Data structures\n\n\
             collections.Counter(list)        count occurrences of each element\n\
             collections.chain(a, b, ...)     concatenate lists\n\
             collections.flatten(nested)      recursively flatten nested lists\n\n\
             Example: collections.Counter([\"a\", \"a\", \"b\"])  =>  {a: 2, b: 1}\n\
             Example: collections.chain([1, 2], [3, 4])  =>  [1, 2, 3, 4]\n\
             Example: collections.flatten([[1, 2], [3, [4, 5]]])  =>  [1, 2, 3, 4, 5]"
                .into(),
        ),
        "itertools" => Some(
            "itertools — Iterator combinators\n\n\
             itertools.range(start, end?, step?)    numeric range list\n\
             itertools.enumerate(list)               [[0, a], [1, b], ...]\n\
             itertools.zip(a, b, ...)                paired elements\n\
             itertools.chain(a, b, ...)              concatenate lists\n\
             itertools.product(a, b, ...)            cartesian product\n\
             itertools.combinations(list, r)         r-element combinations\n\
             itertools.permutations(list, r?)        r-element permutations\n\
             itertools.accumulate(list)              running sum\n\
             itertools.take(n, list)                 first n elements\n\
             itertools.drop(n, list)                 skip first n elements\n\
             itertools.repeat(val, n)                repeat value n times\n\n\
             Example: itertools.range(5)  =>  [0, 1, 2, 3, 4]\n\
             Example: itertools.product([1,2], [\"a\",\"b\"])  =>  [[1,a],[1,b],[2,a],[2,b]]"
                .into(),
        ),
        "tempfile" => Some(
            "tempfile — Temporary files and directories\n\n\
             tempfile.dir()                    system temp directory\n\
             tempfile.mkdtemp(prefix?)        create temp dir, returns path\n\
             tempfile.mkstemp(prefix?)        create temp file, returns path"
                .into(),
        ),
        "binascii" => Some(
            "binascii — Binary/ASCII encoding\n\n\
             binascii.hexlify(data)           bytes to hex string\n\
             binascii.unhexlify(hex)          hex string to bytes\n\
             binascii.b2a_base64(data)        bytes to base64 string\n\
             binascii.a2b_base64(data)        base64 string to bytes\n\n\
             Example: binascii.hexlify(\"hello\")  =>  \"68656c6c6f\"\n\
             Example: binascii.unhexlify(\"68656c6c6f\")  =>  \"hello\""
                .into(),
        ),
        "ftp" => Some(
            "ftp — Pure-Rust FTP client\n\n\
             ftp.connect(host, port?)          connect to FTP server (default port 21)\n\
             ftp.login(session, user, pass)    authenticate\n\
             ftp.pwd(session)                  current directory\n\
             ftp.list(session, path?)          LIST command (full details)\n\
             ftp.nlist(session, path?)         names only\n\
             ftp.cwd(session, dir)             change directory\n\
             ftp.retr(session, file)           download file content\n\
             ftp.stor(session, file, data)     upload content to file\n\
             ftp.dele(session, file)           delete file\n\
             ftp.mkdir(session, dir)           create directory\n\
             ftp.rmdir(session, dir)           remove directory\n\
             ftp.rename(session, old, new)     rename file\n\
             ftp.quit(session)                 disconnect\n\n\
             Example:\n\
             let s = ftp.connect(\"ftp.example.com\", 21)\n\
             ftp.login(s, \"user\", \"pass\")\n\
             print ftp.nlist(s)\n\
             ftp.quit(s)"
                .into(),
        ),
        "smtp" => Some(
            "smtp — Pure-Rust SMTP client\n\n\
             smtp.connect(host, port?)          connect (default port 25)\n\
             smtp.login(session, user, pass)    authenticate (STARTTLS)\n\
             smtp.sendmail(session, from, to, msg)  send email\n\
             smtp.message(from, to, sub, body)  build MIME message string\n\
             smtp.quit(session)                 disconnect\n\n\
             Example:\n\
             let s = smtp.connect(\"smtp.gmail.com\", 587)\n\
             smtp.login(s, \"me@gmail.com\", \"app-password\")\n\
             let msg = smtp.message(\"me@gmail.com\", \"you@gmail.com\", \"Hi\", \"Hello!\")\n\
             smtp.sendmail(s, \"me@gmail.com\", \"you@gmail.com\", msg)"
                .into(),
        ),
        "pop3" => Some(
            "pop3 — Pure-Rust POP3 client\n\n\
             pop3.connect(host, user, pass, port?)  connect + login (default port 110)\n\
             pop3.stat(session)                {count, size}\n\
             pop3.list(session)                message sizes list\n\
             pop3.retr(session, id)            retrieve message by ID\n\
             pop3.dele(session, id)            mark message for deletion\n\
             pop3.quit(session)                disconnect"
                .into(),
        ),
        "imap" => Some(
            "imap — Pure-Rust IMAP client\n\n\
             imap.connect(host, user, pass, port?)  connect + login (default port 143)\n\
             imap.select(session, mailbox)     select mailbox (e.g. \"INBOX\")\n\
             imap.search(session, criteria)    search (e.g. \"ALL\", \"UNSEEN\")\n\
             imap.fetch(session, id)           fetch message => {flags, body}\n\
             imap.list(session)                list available mailboxes\n\
             imap.logout(session)              disconnect"
                .into(),
        ),
        "telnet" => Some(
            "telnet — Pure-Rust telnet client\n\n\
             telnet.connect(host, port?)       connect (default port 23)\n\
             telnet.write(session, data)       send data\n\
             telnet.read(session, size?)       read bytes (default 4096)\n\
             telnet.read_until(session, marker) read until marker found\n\
             telnet.close(session)             disconnect"
                .into(),
        ),
        "dns" => Some(
            "dns — DNS resolver (pure-Rust, no system resolver dependency)\n\n\
             dns.resolve(name)                 resolve to IP address list\n\
             dns.query(name, type?)            query records\n\n\
             Record types: A, AAAA, MX, TXT, NS, CNAME, SOA, SRV, PTR\n\n\
             Example: dns.resolve(\"example.com\")  =>  [\"93.184.216.34\"]\n\
             Example: dns.query(\"gmail.com\", \"MX\")"
                .into(),
        ),
        "ssh" => Some(
            "ssh — System SSH/SCP wrapper (requires ssh binary in PATH)\n\n\
             ssh.available()                   true if ssh is installed\n\
             ssh.run(opts, command)            run remote command\n\
             ssh.upload(opts, local, remote)   upload file via scp\n\
             ssh.download(opts, remote, local) download file via scp\n\n\
             opts: {host, user?, port?, key?, strict_host_key?: false}\n\n\
             Example:\n\
             let opts = {\"host\": \"192.168.1.1\", \"user\": \"root\"}\n\
             print ssh.run(opts, \"uname -a\")"
                .into(),
        ),
        "scapy" => Some(
            "scapy — Packet crafting and sniffing (requires root/CAP_NET_RAW)\n\n\
             Build:\n\
               scapy.ip(src, dst, proto?, ttl?, payload?)  build IP layer\n\
               scapy.tcp(sport, dport, flags?, payload?)   build TCP layer\n\
               scapy.udp(sport, dport, payload?)            build UDP layer\n\
               scapy.icmp(type?, code?, payload?)           build ICMP layer\n\
               scapy.raw(data)                              raw data layer\n\n\
             Serialize/parse:\n\
               scapy.build(layer)       serialize to bytes\n\
               scapy.parse(bytes)       parse bytes to layers\n\n\
             Send/receive:\n\
               scapy.send(layer)        send raw packet\n\
               scapy.sniff(count?, timeout?)  capture packets\n\n\
             Utilities:\n\
               scapy.checksum(data)     internet checksum\n\
               scapy.ip_to_int(ip)      IP string to integer\n\
               scapy.int_to_ip(int)     integer to IP string\n\n\
             Example:\n\
             let pkt = scapy.ip(\"10.0.0.1\", \"10.0.0.2\", \"TCP\", scapy.tcp(12345, 80, 0x02))\n\
             scapy.send(pkt)"
                .into(),
        ),
        _ => None,
    }
}

/// Return list of all data types.
pub fn help_types() -> &'static str {
    "Zen Data Types\n\n\
     Scalar types:\n\
       null        absence of value\n\
       true/false  boolean values\n\
       42          integer (i64)\n\
       3.14        float (f64)\n\
       \"hello\"     string (UTF-8)\n\n\
     Compound types:\n\
       [1, 2, 3]        list (ordered, mutable)\n\
       {a: 1, b: 2}     dict (key-value map, keys are strings)\n\
       func(x) { x }    function\n\
       class Foo {}      class (with optional inheritance)\n\
       obj               instance of a class\n\n\
     Special values:\n\
       null              null (absence of value)\n\
       true / false      booleans\n\
       <native>          built-in function\n\n\
     Type checking:\n\
       typeof x          returns type name string\n\
       type(x)           same as typeof\n\n\
     Type conversion:\n\
       str(x)            to string\n\
       int(x)            to integer (truncates float)\n\
       float(x)          to float\n\
       bool(x)           to boolean (null/false= false, 0= false, \"\"= false)\n\
       list(x)           to list\n\
       dict(pairs)       to dict from list of [key, value] pairs"
}

/// Return list of all built-in functions.
pub fn help_builtins() -> &'static str {
    "Zen Built-in Functions\n\n\
     I/O:\n\
       print(values...)      print to stdout (space-separated, newline appended)\n\
       input(prompt?)        read line from stdin, returns string\n\
       exit(code?)           terminate program\n\n\
     Type conversion:\n\
       str(x)                to string\n\
       int(x)                to integer\n\
       float(x)              to float\n\
       bool(x)               to boolean\n\
       list(x)               to list\n\
       typeof x              type name string\n\n\
     Numeric:\n\
       abs(x)                absolute value\n\
       min(a, b, ...)        minimum value\n\
       max(a, b, ...)        maximum value\n\
       round(x)              round to nearest integer\n\
       trunc(x)              truncate to integer\n\
       hex(x)                hex string (e.g. \"0xff\")\n\
       range(end)            [0..end] inclusive list\n\
       range(start, end)     [start..end] inclusive list\n\n\
     String:\n\
       len(x)                length of string/list/dict\n\
       str.repeat(s, n)      (use string.repeat)\n\n\
     Time:\n\
       sleep(sec)            pause execution seconds\n\
       wait(ms)              pause execution milliseconds\n\n\
     Data:\n\
       json.parse(s)         parse JSON\n\
       json.stringify(v)      encode JSON\n\
       base64.encode(s)      base64 encode\n\
       base64.decode(s)      base64 decode\n\n\
     Errors:\n\
       errors.define(name, base?, msg?)  define custom error class\n\
       throw value                       throw an error\n\n\
     See :help modules for the full list of available modules."
}

/// Return list of all operators.
pub fn help_operators() -> &'static str {
    "Zen Operators\n\n\
     Arithmetic:\n\
       +   addition / string concatenation\n\
       -   subtraction / negation\n\
       *   multiplication / string repetition (str * int)\n\
       /   division\n\
       %   modulo\n\
       **  exponentiation\n\n\
     Comparison:\n\
       ==  equal\n\
       !=  not equal\n\
       <   less than\n\
       >   greater than\n\
       <=  less or equal\n\
       >=  greater or equal\n\n\
     Logical:\n\
       and  logical AND\n\
       or   logical OR\n\
       not  logical NOT\n\n\
     Bitwise:\n\
       &   bitwise AND\n\
       |   bitwise OR\n\
       ^   bitwise XOR\n\
       ~   bitwise NOT\n\
       <<  left shift\n\
       >>  right shift\n\n\
     Membership:\n\
       in       element in list/string/dict\n\
       not in   element not in collection\n\n\
     Other:\n\
       =       assignment\n\
       += -= *= /= %=  compound assignment\n\
       .       member access\n\
       []      index / slice\n\
       ()      function call\n\
       =>      arrow (lambda, match arm)\n\
       ..      range (inclusive)\n\
       ??      null-coalescing\n\
       ?.      optional chaining"
}

/// Return list of all keywords.
pub fn help_keywords() -> &'static str {
    "Zen Keywords\n\n\
     Variables:\n\
       let         mutable variable\n\
       const       immutable constant\n\
       global      global variable declaration\n\n\
     Functions:\n\
       func/fn/def  define function\n\
       return        return from function\n\n\
     Classes:\n\
       class       define class\n\
       new         create instance (reserved)\n\
       inherit     inherit from parent class\n\
       this        reference to current instance\n\n\
     Control flow:\n\
       if/elif/else  conditional branching\n\
       while         while loop\n\
       for/in        for-each loop\n\
       break         exit loop\n\
       continue      skip to next iteration\n\n\
     Error handling:\n\
       throw         raise an error\n\
       try           try block\n\
       catch         catch block (typed catch supported)\n\
       finally       always-execute block\n\
       raise         alias for throw\n\
       except        alias for catch\n\n\
     Modules:\n\
       import        import a module\n\
       from          selective import (from mod import name)\n\n\
     Other:\n\
       null          null value\n\
       true/false    booleans\n\
       lambda        anonymous function\n\
       match         pattern matching\n\
       when          expression-based branching\n\
       as            type alias / import alias\n\
       is            type checking\n\
       typeof        type of expression\n\
       exit          terminate program\n\
       assert        assertion"
}

/// Return help for a specific module (alias: `help_module` for external use).
pub fn help_module(name: &str) -> String {
    module_help(name).unwrap_or_else(|| {
        let modules = [
            "errors", "json", "fs", "re", "random", "math", "time", "os",
            "base64", "crypto", "hashlib", "uuid", "color", "csv", "http",
            "decimal", "threading", "statistics", "socket", "browser",
            "string", "subprocess", "struct", "shutil", "pathlib", "glob",
            "urllib", "collections", "itertools", "tempfile", "binascii",
            "ftp", "smtp", "pop3", "imap", "telnet", "dns", "ssh", "scapy",
        ];
        let close = modules.iter()
            .filter(|m| levenshtein(name, m) <= 2)
            .cloned()
            .collect::<Vec<_>>();
        if close.is_empty() {
            format!("Unknown module: {name}\n\nRun :help modules to see all available modules.")
        } else {
            format!("Unknown module: {name}\n\nDid you mean: {}?", close.join(", "))
        }
    })
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];
    for i in 0..=a_len { matrix[i][0] = i; }
    for j in 0..=b_len { matrix[0][j] = j; }
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }
    matrix[a_len][b_len]
}

/// Parse and validate a program without executing it.
pub fn check(source: &str) -> Result<(), String> {
    let tokens = lex(source)?;
    Parser::new(tokens).program()?;
    Ok(())
}

/// Lint a program, returning a list of human-readable warnings.
pub fn lint(source: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    let Ok(tokens) = lex(source) else {
        return vec!["syntax error: unable to tokenize".into()];
    };
    let Ok(program) = Parser::new(tokens.clone()).program() else {
        return vec!["syntax error: unable to parse".into()];
    };
    // Track declared globals, consts, and builtin names so we can detect
    // undefined-variable references and const reassignment.
    let mut globals: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut consts: std::collections::HashSet<String> = std::collections::HashSet::new();
    let builtins = [
        "str", "len", "range", "int", "float", "bool", "list", "abs", "min", "max",
        "round", "trunc", "print", "input", "typeof", "exit", "json", "fs", "re",
        "math", "time", "random", "base64", "os", "crypto", "statistics", "net",
        "go", "click", "fill", "wait", "text", "attr", "wait_for", "shot", "title",
        "url", "browser", "page",
    ];
    for name in builtins {
        globals.insert(name.into());
    }
    lint_block(&program, 0, &mut globals, &mut consts, &mut warnings);
    if warnings.is_empty() {
        warnings.push("no issues found".into());
    }
    warnings
}

fn lint_block(
    body: &[Stmt],
    depth: usize,
    globals: &mut std::collections::HashSet<String>,
    consts: &mut std::collections::HashSet<String>,
    warnings: &mut Vec<String>,
) {
    let mut unreachable = false;
    for stmt in body {
        if unreachable {
            match &stmt.kind {
                StmtKind::Function(..) | StmtKind::Class(..) => {}
                _ => warnings.push(format!(
                    "{depth}: unreachable statement after return/break/continue"
                )),
            }
        }
        match &stmt.kind {
            StmtKind::Let(target, _, is_const) => {
                let names = match target {
                    LetTarget::Var(n) => vec![n.clone()],
                    LetTarget::List(names) | LetTarget::Dict(names) => names.clone(),
                };
                for name in names {
                    if *is_const {
                        consts.insert(name.clone());
                    }
                    globals.insert(name);
                }
            }
            StmtKind::Assign(n, _, _) => {
                if consts.contains(n) {
                    warnings.push(format!("assignment to constant '{n}'"));
                }
            }
            StmtKind::Return(_) | StmtKind::Break | StmtKind::Continue => unreachable = true,
            StmtKind::If(_, yes, no) => {
                lint_block(yes, depth + 1, globals, consts, warnings);
                lint_block(no, depth + 1, globals, consts, warnings);
            }
            StmtKind::While(_, body) => lint_block(body, depth + 1, globals, consts, warnings),
            StmtKind::For(_, _, body) => lint_block(body, depth + 1, globals, consts, warnings),
            StmtKind::Function(name, params, body) => {
                let saved = globals.clone();
                for param in params {
                    globals.insert(param.clone());
                }
                lint_block(body, depth + 1, globals, consts, warnings);
                *globals = saved;
                let _ = name;
            }
            StmtKind::Try(body, catches, finally) => {
                lint_block(body, depth + 1, globals, consts, warnings);
                for clause in catches {
                    lint_block(&clause.body, depth + 1, globals, consts, warnings);
                }
                if let Some(finally) = finally.as_ref() {
                    lint_block(finally, depth + 1, globals, consts, warnings);
                }
            }
            StmtKind::Switch(_, cases, default) => {
                for (_, body) in cases {
                    lint_block(body, depth + 1, globals, consts, warnings);
                }
                if let Some(default) = default {
                    lint_block(default, depth + 1, globals, consts, warnings);
                }
            }
            StmtKind::Expr(Expr::Call(callee, _)) => {
                if let Expr::Var(name) = callee.as_ref() {
                    if !globals.contains(name) {
                        warnings.push(format!("call to possibly undefined function '{name}'"));
                    }
                }
            }
            _ => {}
        }
    }
}

/// Locate a file under the zen std library directory. Checks (in order):
///  1. $ZEN_STD
///  2. ./std/<name>           (repo-relative, when running from project root)
///  3. <exe_dir>/std/<name>   (installed binary layout)
///  4. <exe_dir>/../std/<name>
fn find_std_file(name: &str) -> Option<String> {
    if let Ok(dir) = env::var("ZEN_STD") {
        let path = std::path::Path::new(&dir).join(name);
        if path.exists() {
            return Some(path.to_string_lossy().into_owned());
        }
    }
    let candidates = [
        format!("std/{name}"),
        format!("zen-rust/std/{name}"),
    ];
    for candidate in candidates {
        if std::path::Path::new(&candidate).exists() {
            return Some(candidate);
        }
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for dir in [exe_dir, exe_dir.parent().unwrap_or(exe_dir)] {
                let path = dir.join("std").join(name);
                if path.exists() {
                    return Some(path.to_string_lossy().into_owned());
                }
            }
        }
    }
    None
}

fn parse_file(path: &str) -> Result<Vec<Stmt>, String> {
    let source = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let tokens = lex(&source)?;
    Parser::new(tokens).program()
}

fn json_encode(v: &Value, pretty: bool) -> String {
    if pretty {
        return json_encode_pretty(v, 0);
    }
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            if n.fract() == 0.0 {
                format!("{n:.0}")
            } else {
                n.to_string()
            }
        }
        Value::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(|i| json_encode(i, pretty)).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Dict(map) => {
            let parts: Vec<String> = map
                .iter()
                .map(|(k, val)| format!("\"{k}\":{}", json_encode(val, pretty)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        Value::Instance(_) => "\"<object>\"".into(),
        Value::Socket(_) => "\"<socket>\"".into(),
        Value::NativeFunction(name) => format!("\"<native:{name}>\""),
        Value::Function(name) => format!("\"<function:{name}>\""),
    }
}

fn json_encode_pretty(v: &Value, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let pad_child = " ".repeat(indent + 2);
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            if n.fract() == 0.0 {
                format!("{n:.0}")
            } else {
                n.to_string()
            }
        }
        Value::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        Value::List(items) => {
            if items.is_empty() {
                "[]".into()
            } else {
                let parts: Vec<String> = items
                    .iter()
                    .map(|i| format!("{pad_child}{}", json_encode_pretty(i, indent + 2)))
                    .collect();
                format!("[\n{}\n{pad}]", parts.join(",\n"))
            }
        }
        Value::Dict(map) => {
            if map.is_empty() {
                "{}".into()
            } else {
                let parts: Vec<String> = map
                    .iter()
                    .map(|(k, val)| {
                        format!("{pad_child}\"{k}\": {}", json_encode_pretty(val, indent + 2))
                    })
                    .collect();
                format!("{{\n{}\n{pad}}}", parts.join(",\n"))
            }
        }
        Value::Instance(_) => "\"<object>\"".into(),
        Value::Socket(_) => "\"<socket>\"".into(),
        Value::NativeFunction(name) => format!("\"<native:{name}>\""),
        Value::Function(name) => format!("\"<function:{name}>\""),
    }
}

fn regex_match(pattern: &str, text: &str) -> bool {
    regex::Regex::new(pattern)
        .map(|re| re.is_match(text))
        .unwrap_or(false)
}

fn regex_find_all(pattern: &str, text: &str) -> Vec<String> {
    let Ok(re) = regex::Regex::new(pattern) else {
        return vec![];
    };
    re.find_iter(text).map(|m| m.as_str().to_string()).collect()
}

fn regex_replace(pattern: &str, text: &str, replacement: &str) -> String {
    regex::Regex::new(pattern)
        .map(|re| re.replace_all(text, replacement).into_owned())
        .unwrap_or_else(|_| text.to_string())
}

fn json_decode(s: &str) -> Result<Value, String> {
    let mut parser = JsonParser { s, pos: 0 };
    let v = parser.value()?;
    parser.skip_ws();
    if parser.pos != s.len() {
        return Err("trailing characters in JSON".into());
    }
    Ok(v)
}

struct JsonParser<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.s.len() {
            let c = self.s[self.pos..].chars().next().unwrap();
            if c.is_whitespace() {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.s[self.pos..].chars().next()
    }

    fn eat(&mut self, expected: char) -> Result<(), String> {
        self.skip_ws();
        if self.peek() == Some(expected) {
            self.pos += expected.len_utf8();
            Ok(())
        } else {
            Err(format!("expected '{expected}' in JSON"))
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        self.skip_ws();
        match self.peek() {
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Value::String(self.string()?)),
            Some('t') => {
                self.expect_literal("true")?;
                Ok(Value::Bool(true))
            }
            Some('f') => {
                self.expect_literal("false")?;
                Ok(Value::Bool(false))
            }
            Some('n') => {
                self.expect_literal("null")?;
                Ok(Value::Null)
            }
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            _ => Err("invalid JSON value".into()),
        }
    }

    fn expect_literal(&mut self, lit: &str) -> Result<(), String> {
        if self.s[self.pos..].starts_with(lit) {
            self.pos += lit.len();
            Ok(())
        } else {
            Err(format!("expected '{lit}' in JSON"))
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        self.skip_ws();
        let start = self.pos;
        let mut has_dot = false;
        while let Some(c) = self.peek() {
            if c == '-' || c == '+' || c == '.' || c.is_ascii_digit() || c == 'e' || c == 'E' {
                if c == '.' {
                    has_dot = true;
                }
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
        let text = &self.s[start..self.pos];
        if has_dot {
            text.parse::<f64>()
                .map(Value::Number)
                .map_err(|_| "invalid JSON number".into())
        } else {
            text.parse::<i64>()
                .map(|n| Value::Number(n as f64))
                .map_err(|_| "invalid JSON number".into())
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat('"')?;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err("unterminated JSON string".into()),
                Some('"') => {
                    self.pos += 1;
                    break;
                }
                Some('\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some('n') => {
                            out.push('\n');
                            self.pos += 1;
                        }
                        Some('t') => {
                            out.push('\t');
                            self.pos += 1;
                        }
                        Some('r') => {
                            out.push('\r');
                            self.pos += 1;
                        }
                        Some('\\') => {
                            out.push('\\');
                            self.pos += 1;
                        }
                        Some('"') => {
                            out.push('"');
                            self.pos += 1;
                        }
                        Some('/') => {
                            out.push('/');
                            self.pos += 1;
                        }
                        Some('u') => {
                            self.pos += 1;
                            let hex = &self.s[self.pos..self.pos + 4];
                            let code = u32::from_str_radix(hex, 16)
                                .map_err(|_| "invalid \\u escape".to_string())?;
                            out.push(char::from_u32(code).unwrap_or('?'));
                            self.pos += 4;
                        }
                        _ => return Err("invalid JSON escape".into()),
                    }
                }
                Some(c) => {
                    out.push(c);
                    self.pos += c.len_utf8();
                }
            }
        }
        Ok(out)
    }

    fn object(&mut self) -> Result<Value, String> {
        self.eat('{')?;
        self.skip_ws();
        let mut map = BTreeMap::new();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(Value::Dict(map));
        }
        loop {
            let key = self.string()?;
            self.eat(':')?;
            let val = self.value()?;
            map.insert(key, val);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                }
                Some('}') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err("expected ',' or '}}' in JSON object".into()),
            }
        }
        Ok(Value::Dict(map))
    }

    fn array(&mut self) -> Result<Value, String> {
        self.eat('[')?;
        self.skip_ws();
        let mut items = vec![];
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(Value::List(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    self.pos += 1;
                }
                Some(']') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err("expected ',' or ']' in JSON array".into()),
            }
        }
        Ok(Value::List(items))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runs_language_core() {
        run("let sum = 0\nfor n in [1, 2, 3] { sum += n }\nif sum == 6 { print sum }").unwrap();
    }
    #[test]
    fn rejects_unknown_name() {
        assert!(run("print missing")
            .unwrap_err()
            .contains("undefined variable"));
    }
    #[test]
    fn honors_precedence() {
        let tokens = lex("let n = 2 + 3 * 4").unwrap();
        let program = Parser::new(tokens).program().unwrap();
        let mut vm = Vm::new();
        vm.exec(&program).unwrap();
        assert_eq!(vm.vars.get("n"), Some(&Value::Number(14.0)));
    }
    #[test]
    fn builds_inclusive_ranges_in_both_directions() {
        let tokens = lex("let up = 1 -> 3\nlet down = 2 -> 0").unwrap();
        let program = Parser::new(tokens).program().unwrap();
        let mut vm = Vm::new();
        vm.exec(&program).unwrap();
        assert_eq!(
            vm.vars.get("up"),
            Some(&Value::List(vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0)
            ]))
        );
        assert_eq!(
            vm.vars.get("down"),
            Some(&Value::List(vec![
                Value::Number(2.0),
                Value::Number(1.0),
                Value::Number(0.0)
            ]))
        );
    }
    #[test]
    fn supports_dictionary_members_and_indexes() {
        let tokens = lex(
            "let config = {host: \"localhost\", ports: [80, 443]}\nlet port = config[\"ports\"][1]",
        )
        .unwrap();
        let program = Parser::new(tokens).program().unwrap();
        let mut vm = Vm::new();
        vm.exec(&program).unwrap();
        assert_eq!(vm.vars.get("port"), Some(&Value::Number(443.0)));
    }
    #[test]
    fn runs_functions_and_recursion() {
        let source = "function factorial(n) { if n <= 1 { return 1 } return n * factorial(n - 1) }\nlet answer = factorial(5)";
        let tokens = lex(source).unwrap();
        let program = Parser::new(tokens).program().unwrap();
        let mut vm = Vm::new();
        vm.exec(&program).unwrap();
        assert_eq!(vm.vars.get("answer"), Some(&Value::Number(120.0)));
    }
    #[test]
    fn runs_instance_methods() {
        let source = "class Greeter { function greet(name) { return \"Hello, \" + name } }\nlet person = new Greeter()\nlet message = person.greet(\"Zen\")";
        let tokens = lex(source).unwrap();
        let program = Parser::new(tokens).program().unwrap();
        let mut vm = Vm::new();
        vm.exec(&program).unwrap();
        assert_eq!(
            vm.vars.get("message"),
            Some(&Value::String("Hello, Zen".into()))
        );
    }
    #[test]
    fn supports_constructors_fields_and_inherited_methods() {
        let source = "class Person { function init(name) { self.name = name } function greet() { return \"Hi, \" + self.name } }\nclass Friendly extends Person { function salute() { return self.greet() + \"!\" } }\nlet user = new Friendly(\"Zen\")\nlet message = user.salute()";
        let tokens = lex(source).unwrap();
        let program = Parser::new(tokens).program().unwrap();
        let mut vm = Vm::new();
        vm.exec(&program).unwrap();
        assert_eq!(
            vm.vars.get("message"),
            Some(&Value::String("Hi, Zen!".into()))
        );
    }
    #[test]
    fn supports_nullish_typeof_strict_and_membership_operators() {
        let source = "let fallback = null ?? \"value\"\nlet type = typeof [1, 2]\nlet contains = 2 in [1, 2, 3]\nlet strict = 2 === 2\nlet not_strict = 2 !== \"2\"";
        let tokens = lex(source).unwrap();
        let program = Parser::new(tokens).program().unwrap();
        let mut vm = Vm::new();
        vm.exec(&program).unwrap();
        assert_eq!(
            vm.vars.get("fallback"),
            Some(&Value::String("value".into()))
        );
        assert_eq!(vm.vars.get("type"), Some(&Value::String("list".into())));
        assert_eq!(vm.vars.get("contains"), Some(&Value::Bool(true)));
        assert_eq!(vm.vars.get("strict"), Some(&Value::Bool(true)));
        assert_eq!(vm.vars.get("not_strict"), Some(&Value::Bool(true)));
    }
}
