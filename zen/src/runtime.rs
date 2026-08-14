use std::{
    collections::{BTreeMap, HashMap},
    env,
    fmt,
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::Path,
    process,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
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
struct Instance {
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
            i += 1;
            col += 1;
            let mut text = String::new();
            let mut closed = false;
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
                        x => x,
                    });
                    i += 1;
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
            out.push(Token {
                kind: Kind::String(text),
                line: start.0,
                col: start.1,
            });
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
                "catch" => Kind::Catch,
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
enum Stmt {
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
    Try(Vec<Stmt>, Option<String>, Vec<Stmt>, Option<Vec<Stmt>>),
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
                        return Ok(Stmt::Let(
                            LetTarget::List(names),
                            Expr::List(values),
                            is_const,
                        ));
                    }
                    LetTarget::Var(first)
                };
                self.expect(Kind::Assign)?;
                Ok(Stmt::Let(target, self.expr()?, is_const))
            }
            Kind::Print => {
                self.advance();
                let mut values = vec![self.expr()?];
                while self.take(Kind::Comma) {
                    values.push(self.expr()?);
                }
                Ok(Stmt::Print(values))
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
                Ok(Stmt::If(cond, yes, no))
            }
            Kind::While => {
                self.advance();
                let cond = self.expr()?;
                Ok(Stmt::While(cond, self.block()?))
            }
            Kind::For => {
                self.advance();
                let name = match self.advance() {
                    Kind::Ident(s) => s,
                    _ => return Err("expected loop variable".into()),
                };
                self.expect(Kind::In)?;
                let items = self.expr()?;
                Ok(Stmt::For(name, items, self.block()?))
            }
            Kind::Break => {
                self.advance();
                Ok(Stmt::Break)
            }
            Kind::Continue => {
                self.advance();
                Ok(Stmt::Continue)
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
                Ok(Stmt::Function(name, params, self.block()?))
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
                Ok(Stmt::Native(name, params))
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
                Ok(Stmt::Return(value))
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
                Ok(Stmt::Import(imports))
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
                Ok(Stmt::FromImport(module, items))
            }
            Kind::Include | Kind::Load => {
                let kind = self.advance();
                let path = match self.advance() {
                    Kind::String(s) => s,
                    Kind::Ident(name) => name,
                    _ => return Err("expected file path string or module name".into()),
                };
                if matches!(kind, Kind::Include) {
                    Ok(Stmt::Include(path))
                } else {
                    Ok(Stmt::Load(path))
                }
            }
            Kind::Class => {
                self.advance();
                let name = match self.advance() {
                    Kind::Ident(name) => name,
                    _ => return Err("expected class name".into()),
                };
                let parent = if self.take(Kind::Extends) {
                    match self.advance() {
                        Kind::Ident(name) => Some(name),
                        _ => return Err("expected parent class name".into()),
                    }
                } else {
                    None
                };
                Ok(Stmt::Class(name, parent, self.block()?))
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
                Ok(Stmt::Switch(value, cases, default_body))
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
                        Expr::Var(name) => Ok(Stmt::Assign(name, op, value)),
                        Expr::Member(object, member) if matches!(op, Kind::Assign) => {
                            Ok(Stmt::SetMember(*object, member, value))
                        }
                        _ => Err("invalid assignment target".into()),
                    }
                } else if let Expr::Var(name) = expression {
                    // Command-style call: `go "url"`, `wait 6`, `sleep 2`, `exit 1`
                    if self.starts_expression() {
                        let arg = self.expr()?;
                        Ok(Stmt::Expr(Expr::Call(
                            Box::new(Expr::Var(name)),
                            vec![arg],
                        )))
                    } else {
                        Ok(Stmt::Expr(Expr::Var(name)))
                    }
                } else {
                    Ok(Stmt::Expr(expression))
                }
            }
            Kind::Try => {
                self.advance();
                let body = self.block()?;
                self.separators();
                let mut catch_var = None;
                let mut catch_body = vec![];
                if self.take(Kind::Catch) {
                    if !matches!(self.current().kind, Kind::LBrace) {
                        match self.advance() {
                            Kind::Ident(name) => catch_var = Some(name),
                            _ => return Err("expected catch variable name".into()),
                        }
                    }
                    catch_body = self.block()?;
                }
                self.separators();
                let finally_body = if self.take(Kind::Finally) {
                    Some(self.block()?)
                } else {
                    None
                };
                Ok(Stmt::Try(body, catch_var, catch_body, finally_body))
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
                Ok(Stmt::Throw(value))
            }
            _ => Ok(Stmt::Expr(self.expr()?)),
        }
    }
    fn if_tail(&mut self) -> Result<Stmt, String> {
        let condition = self.expr()?;
        let yes = self.block()?;
        self.separators();
        let no = if self.take(Kind::Else) {
            self.block()?
        } else {
            vec![]
        };
        Ok(Stmt::If(condition, yes, no))
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
                    Ok(Expr::Lambda(params, vec![Stmt::Return(Some(body))]))
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
        };
        vm.register_builtins();
        vm
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

        // Register all core native functions eagerly
        const NATIVES: [&str; 215] = [
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
                    match self.call_method(instance.clone(), "init", values)? {
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
            Kind::Minus | Kind::Star | Kind::Slash | Kind::Percent | Kind::Pow => {
                let (Value::Number(x), Value::Number(y)) = (a, b) else {
                    return Err("arithmetic requires numbers".into());
                };
                match op {
                    Kind::Minus => Ok(Value::Number(x - y)),
                    Kind::Star => Ok(Value::Number(x * y)),
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
        let flow = self.exec(&function.body)?;
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
        let flow = self.exec(&function.body)?;
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
            match stmt {
                Stmt::Let(target, e, is_const) => {
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
                }
                Stmt::Assign(n, op, e) => {
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
                }
                Stmt::Print(values) => {
                    let text = values
                        .iter()
                        .map(|e| self.eval(e).map(|v| v.to_string()))
                        .collect::<Result<Vec<_>, _>>()?
                        .join(" ");
                    println!("{text}");
                }
                Stmt::Expr(e) => {
                    self.eval(e)?;
                }
                Stmt::If(c, yes, no) => {
                    let flow = if self.eval(c)?.truthy() {
                        self.exec(yes)?
                    } else {
                        self.exec(no)?
                    };
                    if !matches!(flow, Flow::Normal) {
                        return Ok(flow);
                    }
                }
                Stmt::While(c, body) => {
                    while self.eval(c)?.truthy() {
                        match self.exec(body)? {
                            Flow::Normal | Flow::Continue => {}
                            Flow::Break => break,
                            Flow::Return(value) => return Ok(Flow::Return(value)),
                            Flow::Throw(value) => return Ok(Flow::Throw(value)),
                        }
                    }
                }
                Stmt::For(n, e, body) => {
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
                }
                Stmt::Break => return Ok(Flow::Break),
                Stmt::Continue => return Ok(Flow::Continue),
                Stmt::Function(name, params, body) => {
                    let function = Function {
                        params: params.clone(),
                        body: body.clone(),
                    };
                    if let Ok(mut registry) = function_registry().lock() {
                        registry.insert(name.clone(), function.clone());
                    }
                    self.functions.insert(name.clone(), function);
                }
                Stmt::Native(name, _params) => {
                    let func = native_for(name);
                    self.native_functions.insert(name.clone(), func);
                    self.vars.insert(name.clone(), Value::NativeFunction(name.clone()));
                }
                Stmt::Try(body, catch_var, catch_body, finally_body) => {
                    let flow = self.exec(body);
                    let flow = match flow {
                        Ok(Flow::Throw(value)) => {
                            if let Some(var) = catch_var {
                                self.vars.insert(var.clone(), Value::String(value.to_string()));
                                self.exec(catch_body)?
                            } else {
                                return Ok(Flow::Throw(value));
                            }
                        }
                        Err(e) => {
                            if let Some(var) = catch_var {
                                self.vars.insert(var.clone(), Value::String(e));
                                self.exec(catch_body)?
                            } else {
                                return Err(e);
                            }
                        }
                        Ok(f) => f,
                    };
                    if let Some(finally) = finally_body {
                        self.exec(&finally)?;
                    }
                    if !matches!(flow, Flow::Normal) {
                        return Ok(flow);
                    }
                }
                Stmt::Throw(e) => {
                    let val = self.eval(e)?;
                    return Ok(Flow::Throw(val));
                }
                Stmt::Import(imports) => {
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
                }
                Stmt::FromImport(module, items) => {
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
                }
                Stmt::Load(path) => {
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
                }
                Stmt::Include(path) => {
                    let stmts = parse_file(path)?;
                    let flow = self.exec(&stmts)?;
                    if !matches!(flow, Flow::Normal) {
                        return Ok(flow);
                    }
                }
                Stmt::Return(value) => {
                    return Ok(Flow::Return(match value {
                        Some(value) => self.eval(value)?,
                        None => Value::Null,
                    }));
                }
                Stmt::Class(name, parent, body) => {
                    if let Some(parent) = parent {
                        if !self.classes.contains_key(parent) {
                            return Err(format!("unknown parent class: {parent}"));
                        }
                    }
                    let mut methods = HashMap::new();
                    for statement in body {
                        if let Stmt::Function(method, params, body) = statement {
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
                }
                Stmt::SetMember(object, member, value) => {
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
                }
                Stmt::Switch(value, cases, default_body) => {
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
                }
            }
        }
        Ok(Flow::Normal)
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
        "time_utc" | "time_now" => |_| {
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
        _ => |_| Ok(Value::String("Native Call".into())),
    }
}

pub fn run(source: &str) -> Result<(), String> {
    let tokens = lex(source)?;
    let program = Parser::new(tokens).program()?;
    let mut vm = Vm::new();
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
        Flow::Throw(e) => Err(format!("unhandled exception: {e}")),
    }
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
                    if let Stmt::Expr(e) = &program[0] {
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
                    Ok(Flow::Throw(e)) => Err(format!("unhandled exception: {e}")),
                    Err(e) => Err(e),
                }
            }
        }
    }
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
            match stmt {
                Stmt::Function(..) | Stmt::Class(..) => {}
                _ => warnings.push(format!(
                    "{depth}: unreachable statement after return/break/continue"
                )),
            }
        }
        match stmt {
            Stmt::Let(target, _, is_const) => {
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
            Stmt::Assign(n, _, _) => {
                if consts.contains(n) {
                    warnings.push(format!("assignment to constant '{n}'"));
                }
            }
            Stmt::Return(_) | Stmt::Break | Stmt::Continue => unreachable = true,
            Stmt::If(_, yes, no) => {
                lint_block(yes, depth + 1, globals, consts, warnings);
                lint_block(no, depth + 1, globals, consts, warnings);
            }
            Stmt::While(_, body) => lint_block(body, depth + 1, globals, consts, warnings),
            Stmt::For(_, _, body) => lint_block(body, depth + 1, globals, consts, warnings),
            Stmt::Function(name, params, body) => {
                let saved = globals.clone();
                for param in params {
                    globals.insert(param.clone());
                }
                lint_block(body, depth + 1, globals, consts, warnings);
                *globals = saved;
                let _ = name;
            }
            Stmt::Try(body, _, catch, finally) => {
                lint_block(body, depth + 1, globals, consts, warnings);
                lint_block(catch, depth + 1, globals, consts, warnings);
                if let Some(finally) = finally.as_ref() {
                    lint_block(finally, depth + 1, globals, consts, warnings);
                }
            }
            Stmt::Switch(_, cases, default) => {
                for (_, body) in cases {
                    lint_block(body, depth + 1, globals, consts, warnings);
                }
                if let Some(default) = default {
                    lint_block(default, depth + 1, globals, consts, warnings);
                }
            }
            Stmt::Expr(Expr::Call(callee, _)) => {
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
        let mut vm = Vm {
            vars: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
        };
        vm.exec(&program).unwrap();
        assert_eq!(vm.vars.get("n"), Some(&Value::Number(14.0)));
    }
    #[test]
    fn builds_inclusive_ranges_in_both_directions() {
        let tokens = lex("let up = 1 -> 3\nlet down = 2 -> 0").unwrap();
        let program = Parser::new(tokens).program().unwrap();
        let mut vm = Vm {
            vars: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
        };
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
        let mut vm = Vm {
            vars: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
        };
        vm.exec(&program).unwrap();
        assert_eq!(vm.vars.get("port"), Some(&Value::Number(443.0)));
    }
    #[test]
    fn runs_functions_and_recursion() {
        let source = "function factorial(n) { if n <= 1 { return 1 } return n * factorial(n - 1) }\nlet answer = factorial(5)";
        let tokens = lex(source).unwrap();
        let program = Parser::new(tokens).program().unwrap();
        let mut vm = Vm {
            vars: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
        };
        vm.exec(&program).unwrap();
        assert_eq!(vm.vars.get("answer"), Some(&Value::Number(120.0)));
    }
    #[test]
    fn runs_instance_methods() {
        let source = "class Greeter { function greet(name) { return \"Hello, \" + name } }\nlet person = new Greeter()\nlet message = person.greet(\"Zen\")";
        let tokens = lex(source).unwrap();
        let program = Parser::new(tokens).program().unwrap();
        let mut vm = Vm {
            vars: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
        };
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
        let mut vm = Vm {
            vars: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
        };
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
        let mut vm = Vm {
            vars: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
        };
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
