use std::{
    sync::Arc,
    collections::{BTreeMap, HashMap},
    env,
    fmt,
    fs,
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream, UdpSocket},
    path::{Path, PathBuf},
    process::{self, Command},
    rc::Rc,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

// ─── Error display helpers ───────────────────────────────────────────────────

fn suggest_name(target: &str, candidates: &[&str], max_dist: usize) -> Option<String> {
    let mut best: Option<(String, usize)> = None;
    for c in candidates {
        let d = levenshtein(target, c);
        if d <= max_dist {
            if let Some((_, bd)) = &best {
                if d < *bd {
                    best = Some((c.to_string(), d));
                }
            } else {
                best = Some((c.to_string(), d));
            }
        }
    }
    best.map(|(name, _)| name)
}

fn op_symbol(op: &Kind) -> &'static str {
    match op {
        Kind::Plus => "+",
        Kind::Minus => "-",
        Kind::Star => "*",
        Kind::Slash => "/",
        Kind::Percent => "%",
        Kind::Pow => "**",
        Kind::Lt => "<",
        Kind::Gt => ">",
        Kind::Le => "<=",
        Kind::Ge => ">=",
        Kind::Eq => "==",
        Kind::Ne => "!=",
        _ => "operator",
    }
}

fn annotate_line(src_line: &str, col: usize, line_num: usize) -> String {
    let mut out = String::new();
    let width = format!("{}", line_num).len();
    let prefix = format!("{} |", " ".repeat(width));
    let line_str = format!("{} |", line_num);
    out.push_str(&format!("{} {}\n", line_str, src_line));
    let trimmed_len = src_line.chars().count();
    let arrow_col = col.saturating_sub(1).min(trimmed_len);
    out.push_str(&format!("{} {}\n", prefix, " ".repeat(arrow_col)));
    out.push_str(&format!("{} {}\n", prefix, "\x1b[1;31m^\x1b[0m"));
    out
}

/// Render a multi-line error snippet with Rust-style formatting.
/// Shows `context_lines` of source around the error line, with a colored
/// underline pointing at `col`.
fn render_context(
    source_lines: &[String],
    error_line: usize,
    col: usize,
    context_lines: usize,
) -> String {
    let mut out = String::new();
    if source_lines.is_empty() || error_line == 0 {
        return out;
    }
    let err_idx = error_line.saturating_sub(1);
    let total = source_lines.len();
    let start = err_idx.saturating_sub(context_lines);
    let end = (err_idx + context_lines + 1).min(total);
    let width = format!("{}", end).len();
    let gutter = |n: usize| -> String { format!("{:>width$} |", n + 1, width = width) };

    for i in start..end {
        let src_line = source_lines[i].trim_end();
        if src_line.is_empty() {
            continue;
        }
        if i == err_idx {
            out.push_str(&format!(" {} {}\n", gutter(i), src_line));
            let trimmed_len = src_line.chars().count();
            let arrow_col = col.saturating_sub(1).min(trimmed_len);
            let underline = "\x1b[1;31m".to_string() + &"~".repeat(1.max(trimmed_len - arrow_col)) + "\x1b[0m";
            out.push_str(&format!(
                " {} {}\x1b[1;31m{}\x1b[0m\n",
                " ".repeat(width),
                " ".repeat(arrow_col),
                format!("^{}", "~".repeat((trimmed_len - arrow_col).saturating_sub(1).max(0)))
            ));
        } else {
            out.push_str(&format!(" {} {}\n", gutter(i), src_line));
        }
    }
    out
}

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

// Pending error class definitions from errors.define() native calls.
// (name, optional_parent, optional_message)
static PENDING_ERROR_CLASSES: std::sync::OnceLock<
    std::sync::Mutex<Vec<(String, Option<String>, String)>>,
> = std::sync::OnceLock::new();

fn pending_error_classes() -> &'static std::sync::Mutex<Vec<(String, Option<String>, String)>> {
    PENDING_ERROR_CLASSES.get_or_init(|| std::sync::Mutex::new(Vec::new()))
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
    // Reference-counted with copy-on-write semantics: cloning a Value::List
    // or Value::Dict is O(1); mutations go through Arc::make_mut which clones
    // only when the container is shared.
    List(Arc<Vec<Value>>),
    Dict(Arc<BTreeMap<String, Value>>),
    Instance(InstanceRef),
    Socket(Arc<Mutex<TcpStream>>),
    UdpSocket(Arc<Mutex<std::net::UdpSocket>>),
    Listener(Arc<Mutex<std::net::TcpListener>>),
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
            (Self::Socket(_), Self::Socket(_)) => false,
            (Self::UdpSocket(_), Self::UdpSocket(_)) => false,
            (Self::Listener(_), Self::Listener(_)) => false,
            _ => false,
        }
    }
}

impl Value {
    fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Number(_) => "int",
            Self::String(_) => "string",
            Self::List(_) => "list",
            Self::Dict(_) => "dict",
            Self::Instance(_) => "object",
            Self::Socket(_) => "socket",
            Self::UdpSocket(_) => "udp_socket",
            Self::Listener(_) => "listener",
            Self::NativeFunction(_) | Self::Function(_) => "function",
        }
    }
    fn truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(v) => *v,
            Self::Number(v) => *v != 0.0,
            Self::String(v) => !v.is_empty(),
            Self::List(v) => !v.is_empty(),
            Self::Dict(v) => !v.is_empty(),
            Self::Instance(_) | Self::Socket(_) | Self::UdpSocket(_) | Self::Listener(_) | Self::NativeFunction(_) | Self::Function(_) => true,
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
            Self::UdpSocket(_) => write!(f, "<UdpSocket>"),
            Self::Listener(_) => write!(f, "<Listener>"),
            Self::NativeFunction(name) => write!(f, "<native:{name}>"),
            Self::Function(name) => write!(f, "<function:{name}>"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Kind {
    Ident(String),
    Number(f64),
    String(String),
    Interp(Vec<InterpPart>),
    True,
    False,
    Null,
    Let,
    Const,
    Var,
    Print,
    If,
    Else,
    Elif,
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
    Warn,
    Super,
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
                    "{}:{}: unterminated block comment\n  \x1b[1;33m= help:\x1b[0m add `*/` to close the comment block",
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
            let triple = bytes.get(i + 1) == Some(&(quote as u8))
                && bytes.get(i + 2) == Some(&(quote as u8));
            let is_interpolated = c == '"';
            let mut text = String::new();
            let mut closed = false;
            let mut parts: Vec<InterpPart> = Vec::new();
            if triple {
                i += 3;
                col += 3;
                // Triple-quoted string: scan until """
                while i < bytes.len() {
                    let ch = bytes[i] as char;
                    if ch == quote
                        && bytes.get(i + 1) == Some(&(quote as u8))
                        && bytes.get(i + 2) == Some(&(quote as u8))
                    {
                        i += 3;
                        col += 3;
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
                                "{}:{}: unterminated interpolation expression in triple-quoted string",
                                expr_line, expr_col
                            ));
                        }
                        let expr_source = source[expr_start..i].to_string();
                        parts.push(InterpPart::Expr(expr_source));
                        i += 1;
                        col += 1;
                    } else {
                        text.push(ch);
                        i += 1;
                        if ch == '\n' {
                            line += 1;
                            col = 1;
                        } else {
                            col += 1;
                        }
                    }
                }
                if !closed {
                    return Err(format!(
                        "{}:{}: unterminated triple-quoted string\n  \x1b[1;33m= help:\x1b[0m add `\"\"\"` to close the string",
                        start.0, start.1
                    ));
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
            i += 1;
            col += 1;
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
                    "{}:{}: unterminated interpolation expression\n  \x1b[1;33m= help:\x1b[0m add a closing `}}` to complete the interpolation\n  \x1b[1;33m= note:\x1b[0m  interpolation syntax: `\"${{expr}}\"` or `\"${{name + 1}}\"`",
                    expr_line, expr_col
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
                return Err(format!("{}:{}: unterminated string literal\n  \x1b[1;33m= help:\x1b[0m add a matching quote to close the string", start.0, start.1));
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
                "var" => Kind::Var,
                "print" => Kind::Print,
                "if" => Kind::If,
                "elif" => Kind::Elif,
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
                "inherit" => Kind::Extends,
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
                "super" => Kind::Super,
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
                _ => return Err(format!("{}:{}: unexpected character {c:?}\n  \x1b[1;33m= help:\x1b[0m Zen does not recognize `{c}` in this context\n  \x1b[1;33m= note:\x1b[0m  if you meant to use this in a string, wrap it in quotes", line, col)),
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
pub(crate) enum DictEntry {
    Pair(String, Expr),
    Spread(Expr),
}

#[derive(Clone, Debug)]
pub(crate) enum Expr {
    Value(Value),
    Var(String),
    List(Vec<Expr>),
    Dict(Vec<DictEntry>),
    Named(String, Box<Expr>),
    Unary(Kind, Box<Expr>),
    Binary(Box<Expr>, Kind, Box<Expr>),
    Range(Box<Expr>, Box<Expr>, bool),
    Index(Box<Expr>, Box<Expr>),
    Slice(Box<Expr>, Box<Expr>, Box<Expr>),
    Member(Box<Expr>, String),
    SafeMember(Box<Expr>, String),
    Call(Box<Expr>, Vec<Expr>),
    New(String, Vec<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    IfExpr(Box<Expr>, Vec<Stmt>, Vec<Stmt>),
    Increment(Box<Expr>, i64),
    Lambda(Vec<(String, Option<Expr>)>, Vec<Stmt>),
    Spread(Box<Expr>),
    Super(Vec<Expr>),
}
#[derive(Clone, Debug)]
pub(crate) enum LetTarget {
    Var(String),
    List(Vec<String>),
    Dict(Vec<String>),
}

#[derive(Clone, Debug)]
pub(crate) struct Stmt {
    pub(crate) kind: StmtKind,
    pub(crate) line: usize,
    pub(crate) col: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum StmtKind {
    Let(LetTarget, Expr, bool),
    Assign(String, Kind, Expr),
    Print(Vec<Expr>, Option<String>, Option<String>),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    For(String, Expr, Vec<Stmt>),
    Break,
    Continue,
    Function(String, Vec<(String, Option<Expr>)>, Vec<Stmt>),
    Native(String, Vec<String>),
    Field(String, Option<Expr>),
    Try(Vec<Stmt>, Vec<CatchClause>, Option<Vec<Stmt>>),
    Throw(Expr),
    Return(Option<Expr>),
    Class(String, Option<String>, Vec<Stmt>),
    Import(Vec<(String, Option<String>)>),
    FromImport(String, Vec<(String, Option<String>)>),
    StarImport(String),
    Include(String),
    Load(String),
    SetMember(Expr, String, Expr),
    SetIndex(Expr, Expr, Expr),
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
    prev: usize,
    class_depth: usize,
}
impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            prev: 0,
            class_depth: 0,
        }
    }
    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }
    /// The most recently consumed token (the one that failed a parse).
    fn previous(&self) -> &Token {
        &self.tokens[self.prev]
    }
    fn advance(&mut self) -> Kind {
        let kind = self.current().kind.clone();
        if !matches!(kind, Kind::Eof) {
            self.prev = self.pos;
            self.pos += 1;
        }
        kind
    }
    /// Human-readable rendering of a token kind for error messages, e.g.
    /// `==` for Eq, `{` for LBrace, `func` for Function.
    fn kind_text(k: &Kind) -> String {
        match k {
            Kind::Ident(name) => format!("`{name}`"),
            Kind::Eq => "`==`".into(),
            Kind::Bang => "`!`".into(),
            Kind::Ne => "`!=`".into(),
            Kind::Assign => "`=`".into(),
            Kind::NullishAssign => "`??=`".into(),
            Kind::PlusAssign => "`+=`".into(),
            Kind::MinusAssign => "`-=`".into(),
            Kind::StarAssign => "`*=`".into(),
            Kind::SlashAssign => "`/=`".into(),
            Kind::PercentAssign => "`%=`".into(),
            Kind::Arrow => "`->`".into(),
            Kind::DotDot => "`..`".into(),
            Kind::Ellipsis => "`...`".into(),
            Kind::Nullish => "`??`".into(),
            Kind::Amp => "`&`".into(),
            Kind::Pipe => "`|`".into(),
            Kind::Caret => "`^`".into(),
            Kind::LShift => "`<<`".into(),
            Kind::RShift => "`>>`".into(),
            Kind::LBrace => "`{`".into(),
            Kind::RBrace => "`}`".into(),
            Kind::LParen => "`(`".into(),
            Kind::RParen => "`)`".into(),
            Kind::LBracket => "`[`".into(),
            Kind::RBracket => "`]`".into(),
            Kind::Colon => "`:`".into(),
            Kind::Comma => "`,`".into(),
            Kind::Dot => "`.`".into(),
            Kind::Semi => "`;`".into(),
            Kind::Question => "`?`".into(),
            Kind::Plus => "`+`".into(),
            Kind::Minus => "`-`".into(),
            Kind::Star => "`*`".into(),
            Kind::Slash => "`/`".into(),
            Kind::Percent => "`%`".into(),
            Kind::Number(n) => format!("`{n}`"),
            Kind::String(_) => "<string>".into(),
            Kind::Interp(_) => "<interpolated string>".into(),
            Kind::Eof => "end of file".into(),
            Kind::Function => "`func`".into(),
            Kind::Lambda => "`lambda`".into(),
            other => format!("`{other:?}`"),
        }
    }
    fn same(a: &Kind, b: &Kind) -> bool {
        std::mem::discriminant(a) == std::mem::discriminant(b)
    }
    fn peek_ident(&self, name: &str) -> bool {
        matches!(&self.current().kind, Kind::Ident(s) if s == name)
    }
    fn peek_eq(&self) -> bool {
        self.pos + 1 < self.tokens.len() && matches!(self.tokens[self.pos + 1].kind, Kind::Assign)
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
            let found = Self::kind_text(&self.current().kind);
            let expected = Self::kind_text(&kind);
            let hint = match &kind {
                Kind::LBrace => "add `{` to start a block",
                Kind::RBrace => "add `}` to close the block",
                Kind::LParen => "add `(` to start arguments",
                Kind::RParen => "add `)` to close the parentheses",
                Kind::LBracket => "add `[` to start indexing",
                Kind::RBracket => "add `]` to close the bracket",
                Kind::Colon => "add `:` here",
                Kind::Comma => "add `,` to separate items",
                Kind::In => "add `in` keyword here",
                Kind::Assign => "use `=` for assignment",
                _ => "",
            };
            let mut msg = format!(
                "{}:{}: expected {}, found {}",
                self.current().line, self.current().col, expected, found
            );
            if !hint.is_empty() {
                msg.push_str(&format!("\n  \x1b[1;33m= help:\x1b[0m {}", hint));
            }
            Err(msg)
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
            Kind::Var if self.class_depth > 0 => {
                self.advance();
                let name = match self.advance() {
                    Kind::Ident(name) => name,
                    _ => return Err("expected field name after var/val in class body".into()),
                };
                let init = if self.take(Kind::Assign) {
                    Some(self.expr()?)
                } else {
                    None
                };
                Ok(mk(StmtKind::Field(name, init)))
            }
            Kind::Let | Kind::Const | Kind::Var => {
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
                let mut values = vec![];
                let mut sep: Option<String> = None;
                let mut end: Option<String> = None;
                if self.take(Kind::LParen) {
                    if !self.take(Kind::RParen) {
                        loop {
                            if self.peek_ident("sep") && self.peek_eq() {
                                self.advance();
                                self.advance();
                                if let Expr::Value(Value::String(s)) = self.expr()? {
                                    sep = Some(s);
                                }
                            } else if self.peek_ident("end") && self.peek_eq() {
                                self.advance();
                                self.advance();
                                if let Expr::Value(Value::String(s)) = self.expr()? {
                                    end = Some(s);
                                }
                            } else {
                                values.push(self.expr()?);
                            }
                            if !self.take(Kind::Comma) {
                                break;
                            }
                        }
                        self.expect(Kind::RParen)?;
                    }
                } else {
                    values.push(self.expr()?);
                    while self.take(Kind::Comma) {
                        values.push(self.expr()?);
                    }
                }
                Ok(mk(StmtKind::Print(values, sep, end)))
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
                } else if self.take(Kind::Elif) {
                    vec![self.if_tail()?]
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
                        let pname = match self.advance() {
                            Kind::Ident(name) => name,
                            _ => return Err(format!(
                                "{}:{}: expected parameter name, found {}",
                                self.previous().line,
                                self.previous().col,
                                Self::kind_text(&self.previous().kind)
                            )),
                        };
                        let default = if self.take(Kind::Assign) {
                            Some(self.expr()?)
                        } else {
                            None
                        };
                        params.push((pname, default));
                        if !self.take(Kind::Comma) {
                            break;
                        }
                    }
                    self.expect(Kind::RParen)?;
                }
                let saved_depth = self.class_depth;
                self.class_depth = 0;
                let body = self.block();
                self.class_depth = saved_depth;
                Ok(mk(StmtKind::Function(name, params, body?)))
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
                            _ => return Err(format!(
                                "{}:{}: expected parameter name, found {}",
                                self.previous().line,
                                self.previous().col,
                                Self::kind_text(&self.previous().kind)
                            )),
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
                        Kind::Ident(name) => {
                            let mut full = name;
                            while self.take(Kind::Dot) {
                                match self.advance() {
                                    Kind::Ident(part) => {
                                        full.push('.');
                                        full.push_str(&part);
                                    }
                                    _ => return Err("expected submodule name after '.' in import".into()),
                                }
                            }
                            full
                        }
                        Kind::String(path) => path,
                        _ => return Err("expected module name or path".into()),
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
                    Kind::Ident(name) => {
                        let mut full = name;
                        while self.take(Kind::Dot) {
                            match self.advance() {
                                Kind::Ident(part) => {
                                    full.push('.');
                                    full.push_str(&part);
                                }
                                _ => return Err("expected submodule name after '.' in import".into()),
                            }
                        }
                        full
                    }
                    _ => return Err("expected module name".into()),
                };
                self.expect(Kind::Import)?;
                // from module import *
                if self.take(Kind::Star) {
                    return Ok(mk(StmtKind::StarImport(module)));
                }
                let mut items = vec![];
                loop {
                    let item = match self.advance() {
                        Kind::Ident(name) => name,
                        _ => return Err("expected item name or *".into()),
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
                let parent = if self.take(Kind::Extends) || self.take(Kind::Lt) {
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
                self.class_depth += 1;
                let body = self.block();
                self.class_depth -= 1;
                Ok(mk(StmtKind::Class(name, parent, body?)))
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
                        Expr::Index(object, index) if matches!(op, Kind::Assign) => {
                            Ok(mk(StmtKind::SetIndex(*object, *index, value)))
                        }
                        Expr::Member(object, member) if matches!(op, Kind::Assign) => {
                            Ok(mk(StmtKind::SetMember(*object, member, value)))
                        }
                        Expr::Member(object, member) => {
                            let bin_op = match &op {
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
                                _ => return Err("invalid compound assignment target".into()),
                            };
                            let read = Expr::Member(object.clone(), member.clone());
                            let new_val = Expr::Binary(Box::new(read), bin_op, Box::new(value));
                            Ok(mk(StmtKind::SetMember(*object, member, new_val)))
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
        /// Parse `cond { .. } [elif/else if] { .. } [else] { .. }` as an expression
    /// whose value is the last expression of the chosen branch. The leading
    /// `if` token has already been consumed by `atom()`.
    fn parse_if_expr(&mut self) -> Result<Expr, String> {
        let condition = self.expr()?;
        let yes = self.block()?;
        self.separators();
        let no = if self.take(Kind::Else) {
            self.separators();
            if self.take(Kind::If) {
                let inner = self.parse_if_expr()?;
                vec![Stmt {
                    kind: StmtKind::Expr(inner),
                    line: self.current().line,
                    col: self.current().col,
                }]
            } else {
                self.block()?
            }
        } else if self.take(Kind::Elif) {
            let inner = self.parse_if_expr()?;
            vec![Stmt {
                kind: StmtKind::Expr(inner),
                line: self.current().line,
                col: self.current().col,
            }]
        } else {
            vec![]
        };
        Ok(Expr::IfExpr(Box::new(condition), yes, no))
    }

    fn if_tail(&mut self) -> Result<Stmt, String> {
        let (sl, sc) = (self.current().line, self.current().col);
        let condition = self.expr()?;
        let yes = self.block()?;
        self.separators();
        let no = if self.take(Kind::Else) {
            self.separators();
            if self.take(Kind::If) {
                vec![self.if_tail()?]
            } else {
                self.block()?
            }
        } else if self.take(Kind::Elif) {
            vec![self.if_tail()?]
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
                if self.take(Kind::Comma) {
                    let end = self.expr()?;
                    self.expect(Kind::RBracket)?;
                    left = Expr::Slice(Box::new(left), Box::new(index), Box::new(end));
                } else {
                    self.expect(Kind::RBracket)?;
                    left = Expr::Index(Box::new(left), Box::new(index));
                }
            } else if self.take(Kind::Dot) {
                let name = match self.advance() {
                    Kind::Ident(name) => name,
                    other => {
                        return Err(format!(
                            "{}:{}: expected member name, found {}",
                            self.previous().line,
                            self.previous().col,
                            Self::kind_text(&other)
                        ))
                    }
                };
                left = Expr::Member(Box::new(left), name);
            } else if self.take(Kind::SafeDot) {
                let name = match self.advance() {
                    Kind::Ident(name) => name,
                    other => {
                        return Err(format!(
                            "{}:{}: expected member name, found {}",
                            self.previous().line,
                            self.previous().col,
                            Self::kind_text(&other)
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
            Kind::If => self.parse_if_expr(),
            Kind::Function | Kind::Lambda => {
                let mut params = vec![];
                if self.take(Kind::LParen) {
                    if !self.take(Kind::RParen) {
                        loop {
                            let pname = match self.advance() {
                                Kind::Ident(name) => name,
                                _ => return Err(format!(
                                "{}:{}: expected parameter name, found {}",
                                self.previous().line,
                                self.previous().col,
                                Self::kind_text(&self.previous().kind)
                            )),
                            };
                            let default = if self.take(Kind::Assign) {
                                Some(self.expr()?)
                            } else {
                                None
                            };
                            params.push((pname, default));
                            if !self.take(Kind::Comma) {
                                break;
                            }
                        }
                        self.expect(Kind::RParen)?;
                    }
                } else if !matches!(self.current().kind, Kind::Colon | Kind::LBrace) {
                    let pname = match self.advance() {
                        Kind::Ident(name) => name,
                        _ => return Err(format!(
                        "{}:{}: expected parameter name, found {}",
                        self.previous().line,
                        self.previous().col,
                        Self::kind_text(&self.previous().kind)
                    )),
                    };
                    params.push((pname, None));
                    while self.take(Kind::Comma) {
                        let pname = match self.advance() {
                            Kind::Ident(name) => name,
                            _ => return Err(format!(
                                "{}:{}: expected parameter name, found {}",
                                self.previous().line,
                                self.previous().col,
                                Self::kind_text(&self.previous().kind)
                            )),
                        };
                        let default = if self.take(Kind::Assign) {
                            Some(self.expr()?)
                        } else {
                            None
                        };
                        params.push((pname, default));
                    }
                }
                if self.take(Kind::LBrace) {
                    let mut body = self.program()?;
                    self.expect(Kind::RBrace)?;
                    // Auto-return the last expression if it's a bare expression statement
                    if let Some(last) = body.last() {
                        if let StmtKind::Expr(e) = &last.kind {
                            let e = e.clone();
                            let line = last.line;
                            let col = last.col;
                            body.pop();
                            body.push(Stmt {
                                kind: StmtKind::Return(Some(e)),
                                line,
                                col,
                            });
                        }
                    }
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
            Kind::Super => {
                self.expect(Kind::LParen)?;
                let mut args = vec![];
                if !self.take(Kind::RParen) {
                    args.push(self.expr()?);
                    while self.take(Kind::Comma) {
                        args.push(self.expr()?);
                    }
                    self.expect(Kind::RParen)?;
                }
                Ok(Expr::Super(args))
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
            other => {
                let found = Self::kind_text(&other);
                let mut msg = format!(
                    "{}:{}: expected expression, found {}",
                    self.previous().line, self.previous().col, found
                );
                match other {
                    Kind::RBrace => msg.push_str("\n  \x1b[1;33m= help:\x1b[0m the block is already closed; remove the extra `}`"),
                    Kind::RParen => msg.push_str("\n  \x1b[1;33m= help:\x1b[0m the parentheses are already closed; remove the extra `)`"),
                    Kind::RBracket => msg.push_str("\n  \x1b[1;33m= help:\x1b[0m the bracket is already closed; remove the extra `]`"),
                    Kind::Eof => msg.push_str("\n  \x1b[1;33m= help:\x1b[0m the file ended unexpectedly; check for missing closing braces or parentheses"),
                    Kind::Newline => msg.push_str("\n  \x1b[1;33m= help:\x1b[0m expressions cannot span multiple lines without a continuation"),
                    _ => {}
                }
                Err(msg)
            }
        }
    }
}

#[derive(Clone)]
struct Function {
    /// Parameter names with optional default-value expressions (evaluated at
    /// definition time and stored in `default_values`).
    params: Vec<(String, Option<Expr>)>,
    /// Default values parallel to `params` (None where no default exists).
    default_values: Vec<Option<Value>>,
    body: Arc<Vec<Stmt>>,
    captured: HashMap<String, Value>,
    /// Pre-filtered captured vars excluding params (computed at definition time)
    effective_captured: Vec<(String, Value)>,
    /// Compiled bytecode for this function body (None = tree-walk fallback)
    bytecode: Option<Arc<crate::bytecode::CompiledFunction>>,
}

impl Function {
    fn param_names(&self) -> Vec<String> {
        self.params.iter().map(|(n, _)| n.clone()).collect()
    }
    fn required_count(&self) -> usize {
        self.params.iter().filter(|(_, d)| d.is_none()).count()
    }
    fn has_defaults(&self) -> bool {
        self.params.iter().any(|(_, d)| d.is_some())
    }
}

fn collect_free_vars_expr(expr: &Expr, params: &std::collections::HashSet<String>, free: &mut std::collections::HashSet<String>) {
    match expr {
        Expr::Var(n) => { if !params.contains(n.as_str()) { free.insert(n.clone()); } }
        Expr::Value(_) => {}
        Expr::List(items) => { for i in items { collect_free_vars_expr(i, params, free); } }
        Expr::Dict(entries) => {
            for entry in entries {
                match entry {
                    DictEntry::Pair(k, v) => { collect_free_vars_expr(v, params, free); }
                    DictEntry::Spread(e) => { collect_free_vars_expr(e, params, free); }
                }
            }
        }
        Expr::Range(s, e, _) => { collect_free_vars_expr(s, params, free); collect_free_vars_expr(e, params, free); }
        Expr::Index(o, i) => { collect_free_vars_expr(o, params, free); collect_free_vars_expr(i, params, free); }
        Expr::Slice(o, s, e) => { collect_free_vars_expr(o, params, free); collect_free_vars_expr(s, params, free); collect_free_vars_expr(e, params, free); }
        Expr::Member(o, _) => { collect_free_vars_expr(o, params, free); }
        Expr::SafeMember(o, _) => { collect_free_vars_expr(o, params, free); }
        Expr::Call(callee, args) => { collect_free_vars_expr(callee, params, free); for a in args { collect_free_vars_expr(a, params, free); } }
        Expr::Binary(l, _, r) => { collect_free_vars_expr(l, params, free); collect_free_vars_expr(r, params, free); }
        Expr::Unary(_, e) => { collect_free_vars_expr(e, params, free); }
        Expr::Named(n, e) => { collect_free_vars_expr(e, params, free); }
        Expr::Spread(e) => { collect_free_vars_expr(e, params, free); }
        Expr::Super(args) => { for a in args { collect_free_vars_expr(a, params, free); } }
        Expr::New(_, args) => { for a in args { collect_free_vars_expr(a, params, free); } }
        Expr::Ternary(c, y, n) => { collect_free_vars_expr(c, params, free); collect_free_vars_expr(y, params, free); collect_free_vars_expr(n, params, free); }
        Expr::IfExpr(c, y, n) => { collect_free_vars_expr(c, params, free); collect_free_vars_stmts(y, params, free); collect_free_vars_stmts(n, params, free); }
        Expr::Increment(e, _) => { collect_free_vars_expr(e, params, free); }
        Expr::Lambda(lambda_params, lambda_body) => {
            let mut inner = params.clone();
            for p in lambda_params { inner.insert(p.0.clone()); }
            collect_free_vars_stmts(lambda_body, &inner, free);
        }
    }
}

pub(crate) fn collect_free_vars_stmts(stmts: &[Stmt], params: &std::collections::HashSet<String>, free: &mut std::collections::HashSet<String>) {
    for stmt in stmts {
        collect_free_vars_stmt(&stmt.kind, params, free);
    }
}

fn collect_free_vars_stmt(kind: &StmtKind, params: &std::collections::HashSet<String>, free: &mut std::collections::HashSet<String>) {
    match kind {
        StmtKind::Let(target, e, _) => {
            collect_free_vars_expr(e, params, free);
            match target {
                LetTarget::Var(n) => { /* n is now in scope, but we don't remove from free since outer scope still applies */ }
                LetTarget::List(ps) | LetTarget::Dict(ps) => { for p in ps { let _ = p; } }
            }
        }
        StmtKind::Assign(n, _, e) => {
            collect_free_vars_expr(e, params, free);
            if !params.contains(n.as_str()) { free.insert(n.clone()); }
        }
        StmtKind::Print(exprs, _, _) => { for e in exprs { collect_free_vars_expr(e, params, free); } }
        StmtKind::If(c, y, n) => { collect_free_vars_expr(c, params, free); collect_free_vars_stmts(y, params, free); collect_free_vars_stmts(n, params, free); }
        StmtKind::While(c, b) => { collect_free_vars_expr(c, params, free); collect_free_vars_stmts(b, params, free); }
        StmtKind::For(n, e, b) => {
            collect_free_vars_expr(e, params, free);
            let mut inner_params = params.clone();
            inner_params.insert(n.clone());
            collect_free_vars_stmts(b, &inner_params, free);
        }
        StmtKind::Function(_, fn_params, fn_body) => {
            let mut inner_params = params.clone();
            for p in fn_params { inner_params.insert(p.0.clone()); }
            collect_free_vars_stmts(fn_body, &inner_params, free);
        }
        StmtKind::Try(t_body, catches, finally_body) => {
            collect_free_vars_stmts(t_body, params, free);
            for catch in catches {
                let mut catch_params = params.clone();
                if let Some(cv) = &catch.var { catch_params.insert(cv.clone()); }
                collect_free_vars_stmts(&catch.body, &catch_params, free);
            }
            if let Some(fb) = finally_body { collect_free_vars_stmts(fb, params, free); }
        }
        StmtKind::Throw(e) => { collect_free_vars_expr(e, params, free); }
        StmtKind::Return(Some(e)) => { collect_free_vars_expr(e, params, free); }
        StmtKind::Return(None) => {}
        StmtKind::Class(_, parent, body) => {
            if let Some(p) = parent { if !params.contains(p.as_str()) { free.insert(p.clone()); } }
            collect_free_vars_stmts(body, params, free);
        }
        StmtKind::SetMember(obj, _, val) => { collect_free_vars_expr(obj, params, free); collect_free_vars_expr(val, params, free); }
        StmtKind::SetIndex(obj, idx, val) => { collect_free_vars_expr(obj, params, free); collect_free_vars_expr(idx, params, free); collect_free_vars_expr(val, params, free); }
        StmtKind::Expr(e) => { collect_free_vars_expr(e, params, free); }
        StmtKind::Switch(e, cases, default) => {
            collect_free_vars_expr(e, params, free);
            for (case_e, case_body) in cases { collect_free_vars_expr(case_e, params, free); collect_free_vars_stmts(case_body, params, free); }
            if let Some(d) = default { collect_free_vars_stmts(d, params, free); }
        }
        _ => {}
    }
}
#[derive(Clone)]
struct ZenClass {
    parent: Option<String>,
    methods: HashMap<String, Function>,
    /// Field declarations from `var name [= expr]` in the class body, with the
    /// initializer expression to evaluate at instantiation (None => Null).
    fields: Vec<(String, Option<Expr>)>,
}
type NativeFunc = fn(Vec<Value>) -> Result<Value, String>;

pub struct Vm {
    pub vars: ahash::AHashMap<String, Value>,
    functions: HashMap<String, Function>,
    native_functions: HashMap<String, NativeFunc>,
    classes: HashMap<String, ZenClass>,
    imported_modules: HashMap<String, HashMap<String, Value>>,
    stdlib_factories: HashMap<String, fn() -> Value>,
    lambda_counter: u64,
    locked: ahash::AHashSet<String>,
    file: String,
    lines: Vec<String>,
    stack: Vec<String>,
    current_class: Option<String>,
    current_method: Option<String>,
    /// Fast local variable stack: (name, value) pairs for current function params + captured.
    /// O(1) push/pop, no hashing needed. Most-recent-first for innermost scope.
    locals: Vec<(String, Value)>,
    /// Cached function lookup: avoids HashMap lookup on repeated calls to the same function.
    /// Useful for recursive functions like fib where the same function is called millions of times.
    last_func_name: Option<String>,
    last_func_idx: Option<usize>,
    /// Compiled functions table for the currently running module (main at index 0).
    compiled_functions: Vec<Arc<crate::bytecode::CompiledFunction>>,
    /// Bumped whenever a function is defined/redefined; guards the call cache.
    fn_generation: u64,
    /// Cache of the last-resolved function for repeated calls (recursion hot path).
    call_cache: Option<CallCache>,
    /// One entry per active tree-walk function/method frame, holding the names
    /// that were CAPTURED from enclosing scopes. A non-empty stack means we are
    /// inside a function body, so `let` declarations bind to locals instead of
    /// clobbering global variables; captured names propagate writes outward.
    capture_frames: Vec<ahash::AHashSet<String>>,
    /// Start index into `locals` for each active tree-walk frame. Keeps
    /// `bind_let` from updating bindings that belong to an enclosing call.
    frame_starts: Vec<usize>,
}

/// Fast-path cache for repeated calls to the same compiled function.
struct CallCache {
    name: String,
    bc: Arc<crate::bytecode::CompiledFunction>,
    param_count: usize,
    captured: HashMap<String, Value>,
    generation: u64,
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
            vars: ahash::AHashMap::new(),
            functions: HashMap::new(),
            native_functions: HashMap::new(),
            classes: HashMap::new(),
            imported_modules: HashMap::new(),
            stdlib_factories: HashMap::new(),
            lambda_counter: 0,
            locked: ahash::AHashSet::new(),
            file: "<string>".into(),
            lines: Vec::new(),
            stack: vec!["<module>".into()],
            current_class: None,
            current_method: None,
            locals: Vec::new(),
            last_func_name: None,
            last_func_idx: None,
            compiled_functions: Vec::new(),
            fn_generation: 0,
            call_cache: None,
            capture_frames: Vec::new(),
            frame_starts: Vec::new(),
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
            params: vec![("message".into(), None)],
            default_values: vec![None],
            body: Arc::new(vec![Stmt {
                kind: StmtKind::SetMember(
                    Expr::Var("self".into()),
                    "message".into(),
                    Expr::Var("message".into()),
                ),
                line: 0,
                col: 0,
            }]),
            captured: HashMap::new(),
            effective_captured: Vec::new(),
            bytecode: None,
        };
        let mut register = |leaf: &str, parent: Option<&str>| {
            let parent_q = parent.map(|p| format!("errors.{p}"));
            let qualified = format!("errors.{leaf}");
            let mut methods = HashMap::new();
            methods.insert("init".into(), init.clone());
            let class = |methods: HashMap<String, Function>| ZenClass {
                parent: parent_q.clone(),
                methods,
                fields: Vec::new(),
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
        errors_map.insert("define".into(), Value::NativeFunction("errors_define".into()));
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
        self.vars.insert("errors".into(), Value::Dict(Arc::new(errors_map)));

        // Register errors_define as a native function so errors.define() works
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
                Ok(Value::List(Arc::new(values)))
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
                    args.first().map_or("null", |v| v.type_name()).into(),
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
        self.native_functions.insert(
            "help".into(),
            |args| {
                match args.first() {
                    None => {
                        println!("Zen built-ins: print, input, len, str, int, float, bool, type,");
                        println!(" range, dict, list, keys, values, items, has, push, pop,");
                        println!(" slice, assert, throw, exit, sleep, wait, typeof, help,");
                        println!(" min, max, abs, round, trunc, hex, chr, ord, cos, sin,");
                        println!(" tan, sqrt, pow, floor, ceil, json, fs, os, time, random,");
                        println!(" crypto, sys, re");
                        println!("Operators: + - * / % ** & | ^ ~ << >> == != < > <= >= && || ??");
                        println!("Keywords: let var const func return if elif else while for");
                        println!(" break switch case => try catch as class extends super");
                        println!(" new this self import from as default true false null");
                        println!("Tip: type help(<value>) for info on any value.");
                        Ok(Value::Null)
                    }
                    Some(val) => {
                        match val {
                            Value::Instance(inst) => {
                                let i = inst.lock().unwrap();
                                println!("instance of {}", i.class_name);
                                if !i.fields.is_empty() {
                                    println!("  fields:");
                                    for (k, v) in &i.fields {
                                        println!("    {k} = {v}");
                                    }
                                }
                            }
                            Value::Dict(d) => {
                                if d.contains_key("__doc__") {
                                    if let Some(Value::Dict(doc)) = d.get("__doc__") {
                                        if let Some(Value::String(desc)) = doc.get("description") {
                                            println!("{desc}");
                                        }
                                        if let Some(Value::Dict(funcs)) = doc.get("functions") {
                                            println!("\nfunctions:");
                                            for (name, info) in (**funcs).clone() {
                                                match info {
                                                    Value::Dict(fi) => {
                                                        let params = fi.get("params")
                                                            .map(|v| match v {
                                                                Value::String(s) => s.clone(),
                                                                _ => String::new(),
                                                            })
                                                            .unwrap_or_default();
                                                        let ret = fi.get("returns")
                                                            .map(|v| match v {
                                                                Value::String(s) => format!(" -> {s}"),
                                                                _ => String::new(),
                                                            })
                                                            .unwrap_or_default();
                                                        let desc = fi.get("description")
                                                            .and_then(|v| match v {
                                                                Value::String(s) => Some(format!("  — {s}")),
                                                                _ => None,
                                                            })
                                                            .unwrap_or_default();
                                                        println!("  {name}({params}){ret}{desc}");
                                                    }
                                                    _ => println!("  {name}"),
                                                }
                                            }
                                        }
                                        if let Some(Value::Dict(classes)) = doc.get("classes") {
                                            println!("\nclasses:");
                                            for (name, info) in (**classes).clone() {
                                                match info {
                                                    Value::Dict(ci) => {
                                                        let desc = ci.get("description")
                                                            .and_then(|v| match v {
                                                                Value::String(s) => Some(format!("  — {s}")),
                                                                _ => None,
                                                            })
                                                            .unwrap_or_default();
                                                        println!("  {name}{desc}");
                                                    }
                                                    _ => println!("  {name}"),
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    let keys: Vec<String> = d.keys().cloned().collect();
                                    println!("dict with {} keys: {}", keys.len(), keys.join(", "));
                                }
                            }
                            Value::List(lst) => {
                                println!("list with {} elements", lst.len());
                                if !lst.is_empty() {
                                    let preview: Vec<String> = lst.iter().take(5).map(|v| v.to_string()).collect();
                                    println!("  [{}{}]", preview.join(", "),
                                        if lst.len() > 5 { ", ..." } else { "" });
                                }
                            }
                            Value::String(s) => println!("string ({len}): \"{s}\"", len=s.len()),
                            Value::Number(n) => println!("number: {n}"),
                            Value::Bool(b) => println!("bool: {b}"),
                            Value::Null => println!("null"),
                            Value::NativeFunction(name) => println!("native function: {name}"),
                            Value::Function(name) => println!("function: {name}"),
                            _ => println!("{val}"),
                        }
                        Ok(Value::Null)
                    }
                }
            },
        );
        // char/chr — convert codepoint to character
        self.native_functions.insert(
            "char".into(),
            |args| {
                let n = match args.first() {
                    Some(Value::Number(n)) => *n as u32,
                    _ => return Err("char expects a number (Unicode codepoint)".into()),
                };
                let c = char::from_u32(n).ok_or_else(|| format!("invalid codepoint: {n}"))?;
                Ok(Value::String(c.to_string()))
            },
        );
        self.native_functions.insert(
            "chr".into(),
            |args| {
                let n = match args.first() {
                    Some(Value::Number(n)) => *n as u32,
                    _ => return Err("chr expects a number (Unicode codepoint)".into()),
                };
                let c = char::from_u32(n).ok_or_else(|| format!("invalid codepoint: {n}"))?;
                Ok(Value::String(c.to_string()))
            },
        );
        // ord — convert first character to codepoint
        self.native_functions.insert(
            "ord".into(),
            |args| {
                let s = match args.first() {
                    Some(Value::String(s)) => s.clone(),
                    _ => return Err("ord expects a string".into()),
                };
                let c = s.chars().next().ok_or("ord: empty string")?;
                Ok(Value::Number(c as u32 as f64))
            },
        );
        // keys / values / items — dict helpers
        self.native_functions.insert(
            "keys".into(),
            |args| {
                match args.first() {
                    Some(Value::Dict(d)) => Ok(Value::List(Arc::new(d.keys().map(|k| Value::String(k.clone())).collect::<Vec<Value>>()))),
                    _ => Err("keys expects a dict".into()),
                }
            },
        );
        self.native_functions.insert(
            "values".into(),
            |args| {
                match args.first() {
                    Some(Value::Dict(d)) => Ok(Value::List(Arc::new(d.values().cloned().collect::<Vec<Value>>()))),
                    _ => Err("values expects a dict".into()),
                }
            },
        );
        self.native_functions.insert(
            "items".into(),
            |args| {
                match args.first() {
                    Some(Value::Dict(d)) => Ok(Value::List(Arc::new(
                        d.iter().map(|(k, v)| Value::List(Arc::new(vec![Value::String(k.clone()), v.clone()]))).collect::<Vec<Value>>(),
                    ))),
                    _ => Err("items expects a dict".into()),
                }
            },
        );
        // has — check if collection contains key/element
        self.native_functions.insert(
            "has".into(),
            |args| {
                let (col, key) = match args.as_slice() {
                    [c, k] => (c, k),
                    _ => return Err("has expects (collection, key)".into()),
                };
                match col {
                    Value::Dict(d) => match key {
                        Value::String(s) => Ok(Value::Bool(d.contains_key(s.as_str()))),
                        _ => Ok(Value::Bool(false)),
                    },
                    Value::List(l) => Ok(Value::Bool(l.contains(key))),
                    Value::String(s) => match key {
                        Value::String(needle) => Ok(Value::Bool(s.contains(needle.as_str()))),
                        _ => Ok(Value::Bool(false)),
                    },
                    _ => Err(format!("has() unsupported for {}", col)),
                }
            },
        );
        // push — append to list
        self.native_functions.insert(
            "push".into(),
            |args| {
                let (list_val, item) = match args.as_slice() {
                    [l, i] => (l, i),
                    _ => return Err("push expects (list, item)".into()),
                };
                match list_val {
                    Value::List(l) => {
                        let mut l = (**l).clone();
                        l.push(item.clone());
                        Ok(Value::List(Arc::new(l)))
                    }
                    _ => Err("push expects a list as first argument".into()),
                }
            },
        );
        // pop — remove last element from list
        self.native_functions.insert(
            "pop".into(),
            |args| {
                match args.first() {
                    Some(Value::List(l)) => {
                        let mut l = (**l).clone();
                        Ok(l.pop().unwrap_or(Value::Null))
                    }
                    _ => Err("pop expects a list".into()),
                }
            },
        );
        // enumerate — list of [index, value] pairs
        self.native_functions.insert(
            "enumerate".into(),
            |args| {
                match args.first() {
                    Some(Value::List(l)) => Ok(Value::List(Arc::new(
                        l.iter().enumerate().map(|(i, v)| {
                            Value::List(Arc::new(vec![Value::Number(i as f64), v.clone()]))
                        }).collect::<Vec<Value>>(),
                    ))),
                    Some(Value::String(s)) => Ok(Value::List(Arc::new(
                        s.chars().enumerate().map(|(i, c)| {
                            Value::List(Arc::new(vec![Value::Number(i as f64), Value::String(c.to_string())]))
                        }).collect::<Vec<Value>>(),
                    ))),
                    _ => Err("enumerate expects a list or string".into()),
                }
            },
        );
        // slice — extract sub-list or sub-string
        self.native_functions.insert(
            "slice".into(),
            |args| {
                let (col, start, end) = match args.as_slice() {
                    [c, Value::Number(s)] => (c, *s as usize, None),
                    [c, Value::Number(s), Value::Number(e)] => (c, *s as usize, Some(*e as usize)),
                    _ => return Err("slice expects (collection, start[, end])".into()),
                };
                match col {
                    Value::List(l) => {
                        let start = start.min(l.len());
                        let end = end.unwrap_or(l.len()).min(l.len());
                        Ok(Value::List(Arc::new(l[start..end].to_vec())))
                    }
                    Value::String(s) => {
                        let chars: Vec<char> = s.chars().collect();
                        let start = start.min(chars.len());
                        let end = end.unwrap_or(chars.len()).min(chars.len());
                        Ok(Value::String(chars[start..end].iter().collect()))
                    }
                    _ => Err("slice expects a list or string".into()),
                }
            },
        );
        // list — convert to list
        self.native_functions.insert(
            "list".into(),
            |args| {
                match args.first() {
                    Some(Value::String(s)) => Ok(Value::List(Arc::new(s.chars().map(|c| Value::String(c.to_string())).collect::<Vec<Value>>()))),
                    Some(Value::List(l)) => Ok(Value::List(l.clone())),
                    Some(Value::Dict(d)) => Ok(Value::List(Arc::new(
                        d.iter().map(|(k, v)| Value::List(Arc::new(vec![Value::String(k.clone()), v.clone()]))).collect::<Vec<Value>>(),
                    ))),
                    Some(v) => Ok(Value::List(Arc::new(vec![v.clone()]))),
                    None => Ok(Value::List(Arc::new(vec![]))),
                }
            },
        );
        // dict — create dict from pairs
        self.native_functions.insert(
            "dict".into(),
            |args| {
                match args.first() {
                    Some(Value::List(pairs)) => {
                        let mut map = BTreeMap::new();
                        for pair in pairs.iter().cloned() {
                            if let Value::List(kv) = pair {
                                if kv.len() >= 2 {
                                    if let Value::String(k) = &kv[0] {
                                        map.insert(k.clone(), kv[1].clone());
                                    }
                                }
                            }
                        }
                        Ok(Value::Dict(Arc::new(map)))
                    }
                    Some(Value::Dict(d)) => Ok(Value::Dict(d.clone())),
                    _ => Ok(Value::Dict(Arc::new(BTreeMap::new()))),
                }
            },
        );
        // assert
        self.native_functions.insert(
            "assert".into(),
            |args| {
                let cond = args.first().map_or(false, |v| v.truthy());
                if cond {
                    Ok(Value::Null)
                } else {
                    let msg = args.get(1).map(|v| v.to_string()).unwrap_or_else(|| "assertion failed".into());
                    Err(msg)
                }
            },
        );
        // typeof
        self.native_functions.insert(
            "typeof".into(),
            |args| {
                Ok(Value::String(
                    args.first().map_or("null", |v| v.type_name()).into(),
                ))
            },
        );
        // json module
        crate::json::init_json_module(self);

        // fs module
        crate::fs::init_fs_module(self);

        // re module
        crate::re::init_re_module(self);

        // random module
        crate::random::init_random_module(self);

        // math module
        crate::math::init_math_module(self);

        // time module
        crate::time::init_time_module(self);

        // os module
        crate::os::init_os_module(self);

        // base64 module
        crate::base64::init_base64_module(self);

        // base32 module
        crate::base32::init_base32_module(self);

        // crypto module (hashes + hmac + aes)
        crate::crypto::init_crypto_module(self);

        // cryptography module (Fernet symmetric encryption)
        crate::cryptography::init_cryptography_module(self);

        // datetime module
        crate::datetime::init_datetime_module(self);

        // uuid module
        crate::uuid::init_uuid_module(self);

        // color module (ANSI helpers)
        crate::color::init_color_module(self);

        // csv module
        crate::csv::init_csv_module(self);

        // http module
        crate::http::init_http_module(self);

        // decimal module
        crate::decimal::init_decimal_module(self);

        // threading module
        crate::threading::init_threading_module(self);

        // statistics module
        crate::statistics::init_statistics_module(self);

        // socket module
        crate::socket::init_socket_module(self);

        // browser module (CDP-based browser automation)
        crate::browser::init_browser_module(self);

        // ftp module (pure-Rust FTP client)
        crate::ftp::init_ftp_module(self);

        // smtp module (pure-Rust SMTP client)
        crate::smtp::init_smtp_module(self);

        // pop3 module (pure-Rust POP3 client)
        crate::pop3::init_pop3_module(self);

        // imap module (pure-Rust IMAP client)
        crate::imap::init_imap_module(self);

        // telnet module (pure-Rust telnet client)
        crate::telnet::init_telnet_module(self);

        // dns module (pure-Rust DNS client)
        crate::dns::init_dns_module(self);

        // ssh module (wraps the system ssh/scp binaries)
        crate::ssh::init_ssh_module(self);

        // bluetooth module (wraps bluetoothctl)
        crate::bluetooth::init_bluetooth_module(self);

        // wifi module (wraps nmcli / iw)
        crate::wifi::init_wifi_module(self);

        // crunch module (Rust-native password wordlist generator)
        crate::crunch::init_crunch_module(self);

        // scapy module (packet crafting / sniffing)
        crate::scapy::init_scapy_module(self);

        // string module (Python string helpers + constants)
        crate::string::init_string_module(self);

        // subprocess module
        crate::subprocess::init_subprocess_module(self);

        // struct module (binary pack/unpack)
        crate::struct_mod::init_struct_mod_module(self);

        // hashlib module (alias to the crypto hashes)
        crate::hashlib::init_hashlib_module(self);

        // shutil module (file utilities)
        crate::shutil::init_shutil_module(self);

        // pathlib module
        crate::pathlib::init_pathlib_module(self);

        // glob module
        crate::glob::init_glob_module(self);

        // urllib module
        crate::urllib::init_urllib_module(self);

        // collections module
        crate::collections::init_collections_module(self);

        // itertools module
        crate::itertools::init_itertools_module(self);

        // tempfile module
        crate::tempfile::init_tempfile_module(self);

        // binascii module
        crate::binascii::init_binascii_module(self);

        // Register all core native functions eagerly
          const NATIVES: [&str; 402] = [
            "math_sin",
            "math_cos",
            "socket_open",
            "socket_send",
            "socket_recv",
            "socket_recv_text",
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
            "os_arch",
            "os_execute",
            "os_run",
            "os_popen",
            "os_args",
            "os_pids",
            "os_kill",
            "errors_define",
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
            "browser_attr",
            "browser_page_text",
            "browser_wait_for_ms",
            "socket_close",
            "socket_listen",
            "socket_accept",
            "socket_open_udp",
            "socket_send_to",
            "socket_recv_from",
            "socket_recv_all",
            "socket_scan",
            "socket_set_timeout",
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
            "bt_status",
            "bt_power",
            "bt_scan",
            "bt_scan_stop",
            "bt_devices",
            "bt_pair",
            "bt_unpair",
            "bt_connect",
            "bt_disconnect",
            "bt_trust",
            "bt_send",
            "wifi_scan",
            "wifi_status",
            "wifi_connect",
            "wifi_disconnect",
            "wifi_forget",
            "wifi_interfaces",
            "wifi_list",
            "crunch_charset",
            "crunch_generate",
            "crunch_pattern",
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
            "scapy_cidr_expand",
            "scapy_subnet_hosts",
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
            Expr::Value(v) => Ok(v.clone()),
            Expr::Var(n) => {
                let resolved;
                let n: &String = if n == "this" {
                    resolved = "self".to_string();
                    &resolved
                } else {
                    n
                };
                // Fast path: check local stack (innermost last)
                // Most common case in fib: "n" is at known position in locals
                let mut found = None;
                for i in (0..self.locals.len()).rev() {
                    if self.locals[i].0.as_str() == n {
                        found = Some(i);
                        break;
                    }
                }
                if let Some(i) = found {
                    return Ok(self.locals[i].1.clone());
                }
                if let Some(v) = self.vars.get(n) {
                    return Ok(v.clone());
                }
                if let Some(f) = self.functions.get(n) {
                    return Ok(Value::Function(n.clone()));
                }
                if let Some(vars) = self.imported_modules.get(n) {
                    return Ok(if let Some(Value::Dict(module_dict)) = vars.get(n) {
                        Value::Dict(module_dict.clone())
                    } else {
                        Value::Dict(Arc::new(vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<BTreeMap<String, Value>>()))
                    });
                }
                if let Some(v) = self.self_field_get(n) {
                    return Ok(v);
                }
                let mut candidates: Vec<&str> = self.vars.keys().map(|s| s.as_str()).collect();
                candidates.extend(self.functions.keys().map(|s| s.as_str()).filter(|k| !k.contains("::")));
                let hint = suggest_name(n, &candidates, 3)
                    .map(|s| format!("\n  \x1b[1;33m= help:\x1b[0m a variable named `{}` is in scope\n  \x1b[1;33m= help:\x1b[0m did you mean `{}` instead of `{}`?", s, s, n))
                    .unwrap_or_else(|| format!("\n  \x1b[1;33m= note:\x1b[0m  `{n}` has not been defined yet. Use `let {n} = ...` to declare it."));
                Err(format!("undefined variable: `{n}`{hint}"))
            }
            Expr::Named(_, _) => Err("named arguments are only allowed inside calls".into()),
            Expr::Spread(_) => Err("spread is only allowed inside list/dict literals".into()),
            Expr::List(items) => {
                let mut list = Vec::new();
                for x in items {
                    match x {
                        Expr::Spread(inner) => match self.eval(inner)? {
                            Value::List(items) => list.extend(items.iter().cloned()),
                            Value::Dict(map) => list.extend(Arc::unwrap_or_clone(map).into_values()),
                            other => return Err(format!("cannot spread {other} into a list")),
                        },
                        other => list.push(self.eval(other)?),
                    }
                }
                Ok(Value::List(Arc::new(list)))
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
                                for (k, v) in Arc::unwrap_or_clone(map) {
                                    dict.insert(k, v);
                                }
                            }
                            other => return Err(format!("cannot spread {other} into a dict")),
                        },
                    }
                }
                Ok(Value::Dict(Arc::new(dict)))
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
                Ok(Value::List(Arc::new(values)))
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
            Expr::Slice(object, start, end) => {
                let object = self.eval(object)?;
                let Value::Number(s) = self.eval(start)? else {
                    return Err("slice start must be a number".into());
                };
                let Value::Number(e) = self.eval(end)? else {
                    return Err("slice end must be a number".into());
                };
                let s = s as i64;
                let e = e as i64;
                match object {
                    Value::String(value) => {
                        let chars: Vec<char> = value.chars().collect();
                        let len = chars.len() as i64;
                        let a = if s < 0 { len + s } else { s };
                        let b = if e < 0 { len + e } else { e };
                        let a = a.max(0).min(len) as usize;
                        let b = b.max(0).min(len) as usize;
                        Ok(Value::String(
                            chars.get(a..b).unwrap_or(&[]).iter().collect(),
                        ))
                    }
                    Value::List(values) => {
                        let len = values.len() as i64;
                        let a = if s < 0 { len + s } else { s };
                        let b = if e < 0 { len + e } else { e };
                        let a = a.max(0).min(len) as usize;
                        let b = b.max(0).min(len) as usize;
                        Ok(Value::List(Arc::new(values.get(a..b).unwrap_or(&[]).to_vec())))
                    }
                    _ => Err("slice requires a string or list".into()),
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
            Expr::IfExpr(condition, yes, no) => {
                if self.eval(condition)?.truthy() {
                    self.eval_block_expr(yes)
                } else {
                    self.eval_block_expr(no)
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
                    .ok_or_else(|| {
                        let mut candidates: Vec<&str> = self.vars.keys().map(|s| s.as_str()).collect();
                        candidates.extend(self.functions.keys().map(|s| s.as_str()).filter(|k| !k.contains("::")));
                        let hint = suggest_name(name, &candidates, 3)
                            .map(|s| format!("\n  \x1b[1;33m= help:\x1b[0m did you mean `{s}` instead of `{name}`?"))
                            .unwrap_or_else(|| format!("\n  \x1b[1;33m= note:\x1b[0m  `{name}` has not been defined yet"));
                        format!("undefined variable: `{name}`{hint}")
                    })?
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
                let names: Vec<String> = params.iter().map(|(n, _)| n.clone()).collect();
                let param_set: std::collections::HashSet<String> = names.iter().cloned().collect();
                let mut free = std::collections::HashSet::new();
                collect_free_vars_stmts(body, &param_set, &mut free);
                let captured: HashMap<String, Value> = free.iter()
                    .filter_map(|k| {
                        if let Some(v) = self.vars.get(k) {
                            return Some((k.clone(), v.clone()));
                        }
                        // Enclosing function params/captures live on the locals
                        // stack; grab the nearest binding so closures can close
                        // over parameters of the defining function.
                        self.locals
                            .iter()
                            .rev()
                            .find(|(n, _)| n == k)
                            .map(|(n, v)| (n.clone(), v.clone()))
                    })
                    .collect();
                let effective_captured: Vec<(String, Value)> = captured.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                let mut captured_names: Vec<String> = captured
                    .keys()
                    .filter(|k| !param_set.contains(k.as_str()))
                    .cloned()
                    .collect();
                captured_names.sort();
                let has_defaults = params.iter().any(|(_, d)| d.is_some());
                let default_values = self.eval_default_values(params)?;
                let bytecode = if has_defaults {
                    None
                } else {
                    std::env::set_var("ZEN_DBG_FN", format!("eval:{fname}"));
                    let bc = match crate::bytecode::compile_function(&fname, &names, &captured_names, body) {
                        Ok(b) => Some(b),
                        Err(_) => None
                    };
                    std::env::remove_var("ZEN_DBG_FN");
                    bc
                };
                let function = Function {
                    params: params.clone(),
                    default_values,
                    body: Arc::new(body.clone()),
                    captured,
                    effective_captured,
                    bytecode,
                };
                self.register_function(fname.clone(), function);
                Ok(Value::Function(fname))
            }
            Expr::Super(args) => {
                let class_name = self.current_class.clone()
                    .ok_or("super() can only be used inside a class method")?;
                let method_name = self.current_method.clone()
                    .ok_or("super() can only be used inside a class method")?;
                let class = self.classes.get(&class_name)
                    .ok_or_else(|| format!("unknown class: {class_name}"))?;
                let parent_name = class.parent.clone()
                    .ok_or_else(|| format!("{class_name} has no parent class"))?;
                let parent_class = self.classes.get(&parent_name)
                    .ok_or_else(|| format!("unknown parent class: {parent_name}"))?;
                let parent_func = parent_class.methods.get(&method_name)
                    .ok_or_else(|| format!("{parent_name} has no method: {method_name}"))?;
                let function = parent_func.clone();
                let mut values = Vec::new();
                for arg in args {
                    values.push(self.eval(arg)?);
                }
                let total = function.params.len();
                if values.len() > total || values.len() < function.required_count() {
                    return Err(format!(
                        "{parent_name}.{method_name} expects {} arguments, got {}",
                        total, values.len()
                    ));
                }
                for i in values.len()..total {
                    values.push(function.default_values[i].clone().unwrap_or(Value::Null));
                }
                let instance = self.eval(&Expr::Var("self".into()))?;
                let saved_len = self.locals.len();
                self.frame_starts.push(saved_len);
                self.locals.reserve(1 + function.params.len());
                self.locals.push(("self".into(), instance));
                for (param, val) in function.params.iter().zip(values) {
                    self.locals.push((param.0.clone(), val));
                }
                let prev_class = self.current_class.take();
                let prev_method = self.current_method.take();
                self.current_class = Some(parent_name.clone());
                self.current_method = Some(method_name.clone());
                self.stack.push(format!("{parent_name}.{method_name}"));
                let flow = self.exec(&function.body);
                self.stack.pop();
                self.locals.truncate(saved_len);
                self.frame_starts.pop();
                let result = match flow? {
                    Flow::Return(v) => Ok(v),
                    Flow::Throw(v) => Err(format!("unhandled exception: {v}")),
                    Flow::Normal => Ok(Value::Null),
                    _ => Err("loop control escaped super call".into()),
                };
                self.current_class = prev_class;
                self.current_method = prev_method;
                result
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
                    values.push(Value::Dict(Arc::new(named)));
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
                                // The target may be a global or a function-local.
                                let local_idx = self.locals.iter().rposition(|(n, _)| n == name);
                                let current = match local_idx {
                                    Some(i) => match &self.locals[i].1 {
                                        Value::List(_) => Some(self.locals[i].1.clone()),
                                        _ => None,
                                    },
                                    None => self.vars.get(name).cloned(),
                                };
                                if let Some(Value::List(mut list)) = current {
                                    let slot = Arc::make_mut(&mut list);
                                    let result = match method.as_str() {
                                        "push" => {
                                            if let Some(item) = values.first() {
                                                slot.push(item.clone());
                                            }
                                            Value::Null
                                        }
                                        _ => slot.pop().unwrap_or(Value::Null),
                                    };
                                    let updated = Value::List(list);
                                    match local_idx {
                                        Some(i) => self.locals[i].1 = updated,
                                        None => {
                                            self.vars.insert(name.clone(), updated);
                                        }
                                    }
                                    return Ok(result);
                                }
                            }
                        }
                        let obj = self.eval(object)?;
                        let v = self.invoke_member(obj, method, values, Some(object))?;
                        Ok(v)
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
                    return Err(format!("undefined class: {class_name}\n  \x1b[1;33m= help:\x1b[0m check that the class is defined before using `new`\n  \x1b[1;33m= help:\x1b[0m class definitions must appear before the `new` statement"));
                }
                let values = args
                    .iter()
                    .map(|argument| self.eval(argument))
                    .collect::<Result<Vec<_>, _>>()?;
                let instance = Arc::new(Mutex::new(Instance {
                    class_name: class_name.clone(),
                    fields: BTreeMap::new(),
                }));
                {
                    let mut seen = std::collections::HashSet::new();
                    let mut cur = Some(class_name.clone());
                    while let Some(cname) = cur {
                        let (fields, parent): (Vec<(String, Option<Expr>)>, Option<String>) =
                            match self.classes.get(&cname) {
                                Some(c) => (c.fields.clone(), c.parent.clone()),
                                None => break,
                            };
                        for (fname, init) in fields {
                            if seen.insert(fname.clone()) {
                                let val = match init {
                                    Some(e) => self.eval(&e)?,
                                    None => Value::Null,
                                };
                                instance.lock().unwrap().fields.insert(fname, val);
                            }
                        }
                        cur = parent;
                    }
                }
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
                        _ => Err(format!(
                            "cannot negate `{}`\nnote: negation (-) only works on numbers\n      to make a number negative, use: int(\"{}\") or float(\"{}\")",
                            v.type_name(), v, v
                        )),
                    },
                    Kind::Bang | Kind::Not => Ok(Value::Bool(!v.truthy())),
                    Kind::Typeof => Ok(Value::String(
                        v.type_name().to_string(),
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
                .ok_or_else(|| {
                    // Hide compiler-internal namespaced keys (`mod::func`)
                    // from user-facing suggestions; they are not callable syntax.
                    let mut keys: Vec<&str> = values.keys().map(|s| s.as_str())
                        .filter(|k| !k.contains("::"))
                        .collect();
                    let hint = suggest_name(name, &keys, 4)
                        .map(|s| format!("\nnote: did you mean `{}`? available: {}", s, keys.iter().take(8).map(|s| *s).collect::<Vec<_>>().join(", ")))
                        .unwrap_or_else(|| {
                            if !keys.is_empty() {
                                format!("\nnote: available members: {}", keys.iter().take(8).map(|s| *s).collect::<Vec<_>>().join(", "))
                            } else {
                                String::new()
                            }
                        });
                    format!("dictionary has no member: `{}`{}", name, hint)
                }),
            Value::List(values) if name == "len" || name == "count" || name == "length" => {
                Ok(Value::Number(values.len() as f64))
            }
            Value::String(value) if name == "len" || name == "count" || name == "length" => {
                Ok(Value::Number(value.chars().count() as f64))
            }
            Value::Instance(instance) => instance
                .lock()
                .unwrap()
                .fields
                .get(name)
                .cloned()
                .ok_or_else(|| {
                    let hint = format!("\nnote: use `self.{}` in __init__ to define fields", name);
                    format!("object has no field: `{}`{}", name, hint)
                }),
            value => Err(format!("{} has no member: {name}", value)),
        }
    }
    fn number_method(&mut self, n: f64, method: &str, args: Vec<Value>) -> Result<Value, String> {
        match method {
            "floor" => Ok(Value::Number(n.floor())),
            "ceil" => Ok(Value::Number(n.ceil())),
            "round" => Ok(Value::Number(n.round())),
            "abs" => Ok(Value::Number(n.abs())),
            "toInt" | "to_int" => Ok(Value::Number(n.trunc())),
            "toString" | "to_string" | "str" => Ok(Value::String(n.to_string())),
            "toFixed" | "to_fixed" => {
                let decimals = match args.first() {
                    Some(Value::Number(d)) => *d as usize,
                    _ => 0,
                };
                Ok(Value::String(format!("{:.prec$}", n, prec = decimals)))
            }
            "sqrt" => Ok(Value::Number(n.sqrt())),
            "pow" => {
                let exp = match args.first() {
                    Some(Value::Number(e)) => *e,
                    _ => return Err("pow expects an exponent".into()),
                };
                Ok(Value::Number(n.powf(exp)))
            }
            "isNaN" | "is_nan" => Ok(Value::Bool(n.is_nan())),
            "isFinite" | "is_finite" => Ok(Value::Bool(n.is_finite())),
            "isInfinite" | "is_infinite" => Ok(Value::Bool(n.is_infinite())),
            "isInteger" | "is_integer" => Ok(Value::Bool(n.fract() == 0.0)),
            _ => Err(format!("number has no method: {method}")),
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
            "toNum" | "to_num" | "toNumber" | "to_number" => {
                let trimmed = value.trim();
                match trimmed.parse::<f64>() {
                    Ok(n) => Ok(Value::Number(n)),
                    Err(_) => Err(format!(
                        "cannot convert '{trimmed}' to a number\n  \x1b[1;33m= help:\x1b[0m the string must contain a valid numeric literal (e.g. \"42\", \"3.14\", \"-7\")"
                    )),
                }
            }
            "split" => {
                let sep = one()?;
                Ok(Value::List(Arc::new(
                    value.split(&sep).map(|s| Value::String(s.into())).collect::<Vec<Value>>(),
                )))
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
            "format" => {
                let mut result = String::new();
                let mut auto_idx = 0usize;
                let chars: Vec<char> = value.chars().collect();
                let mut i = 0;
                while i < chars.len() {
                    if chars[i] == '{' && i + 1 < chars.len() && chars[i + 1] == '}' {
                        if auto_idx < args.len() {
                            result.push_str(&args[auto_idx].to_string());
                            auto_idx += 1;
                        }
                        i += 2;
                    } else if chars[i] == '{' {
                        let start = i + 1;
                        i += 1;
                        while i < chars.len() && chars[i] != '}' {
                            i += 1;
                        }
                        let key: String = chars[start..i].iter().collect();
                        let key = key.trim();
                        if let Ok(idx) = key.parse::<usize>() {
                            if idx < args.len() {
                                result.push_str(&args[idx].to_string());
                            }
                        } else {
                            let mut found = false;
                            for arg in &args {
                                if let Value::Dict(map) = arg {
                                    if let Some(val) = map.get(key) {
                                        result.push_str(&val.to_string());
                                        found = true;
                                        break;
                                    }
                                }
                            }
                            if !found {
                                result.push_str(&format!("{{{key}}}"));
                            }
                        }
                        if i < chars.len() {
                            i += 1;
                        }
                    } else {
                        result.push(chars[i]);
                        i += 1;
                    }
                }
                Ok(Value::String(result))
            }
            "find" => {
                let needle = one()?;
                match value.find(&needle) {
                    Some(i) => Ok(Value::Number(i as f64)),
                    None => Ok(Value::Number(-1.0)),
                }
            }
            "includes" | "contains" => {
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
            "charAt" | "char" => {
                let idx = match args.first() {
                    Some(Value::Number(n)) => *n as usize,
                    _ => return Err("charAt expects a number index".into()),
                };
                let ch = value.chars().nth(idx).unwrap_or('\0');
                Ok(Value::String(ch.to_string()))
            }
            "ord" => {
                let ch = value.chars().next().unwrap_or('\0');
                Ok(Value::Number(ch as u32 as f64))
            }
            "trim" | "strip" => Ok(Value::String(value.trim().into())),
            "trimEnd" | "trimRight" | "trim_right" => Ok(Value::String(value.trim_end().into())),
            "trimStart" | "trimLeft" | "trim_left" => Ok(Value::String(value.trim_start().into())),
            "lower" | "toLower" | "toLowerCase" => Ok(Value::String(value.to_lowercase())),
            "upper" | "toUpper" | "toUpperCase" => Ok(Value::String(value.to_uppercase())),
            "reverse" => Ok(Value::String(value.chars().rev().collect())),
            "length" => Ok(Value::Number(value.chars().count() as f64)),
            "repeat" => {
                let n = match args.first() {
                    Some(Value::Number(n)) => *n as usize,
                    _ => return Err("repeat expects a number".into()),
                };
                Ok(Value::String(value.repeat(n)))
            }
            "concat" => {
                let mut out = value;
                for arg in &args {
                    out.push_str(&arg.to_string());
                }
                Ok(Value::String(out))
            }
            "split" => {
                let sep = one()?;
                Ok(Value::List(Arc::new(
                    value.split(&sep).map(|s| Value::String(s.into())).collect::<Vec<Value>>(),
                )))
            }
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
            "toList" => Ok(Value::List(Arc::new(
                value.chars().map(|c| Value::String(c.to_string())).collect::<Vec<Value>>(),
            ))),
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
                Ok(Value::List(Arc::new(list)))
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
                Ok(Value::List(Arc::new(list)))
            }
            "sort" => {
                let mut list = list;
                list.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
                Ok(Value::List(Arc::new(list)))
            }
            "skip" => {
                let n = match args.first() {
                    Some(Value::Number(n)) => *n as usize,
                    _ => return Err("skip expects a number".into()),
                };
                Ok(Value::List(Arc::new(list.iter().cloned().skip(n).collect::<Vec<Value>>())))
            }
            "concat" => {
                let extra: Vec<Value> = args
                    .iter()
                    .flat_map(|a| match a {
                        Value::List(items) => items.iter().cloned().collect::<Vec<Value>>(),
                        other => vec![other.clone()],
                    })
                    .collect();
                let mut list = list;
                list.extend(extra);
                Ok(Value::List(Arc::new(list)))
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
                Ok(Value::List(Arc::new(out)))
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
                Ok(Value::List(Arc::new(list)))
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
                Ok(Value::List(Arc::new(out)))
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
                Ok(Value::List(Arc::new(out)))
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
            "includes" | "indexOf" | "index_of" => {
                let item = match args.first() {
                    Some(v) => v.clone(),
                    None => return Err(format!("{method} expects an argument")),
                };
                match list.iter().position(|x| x == &item) {
                    Some(i) => Ok(Value::Number(i as f64)),
                    None => Ok(Value::Number(-1.0)),
                }
            }
            "flat" | "flatten" => {
                let mut out = Vec::new();
                for item in list {
                    if let Value::List(sub) = item {
                        out.extend(sub.iter().cloned());
                    } else {
                        out.push(item);
                    }
                }
                Ok(Value::List(Arc::new(out)))
            }
            "compact" => {
                Ok(Value::List(Arc::new(list.iter().cloned().filter(|v| v.truthy()).collect::<Vec<Value>>())))
            }
            "uniq" | "unique" => {
                let mut seen = Vec::new();
                let mut out = Vec::new();
                for item in list {
                    if !seen.contains(&item) {
                        seen.push(item.clone());
                        out.push(item);
                    }
                }
                Ok(Value::List(Arc::new(out)))
            }
            "shuffle" => {
                let mut list = list;
                for i in (1..list.len()).rev() {
                    let j = (rand::random::<f64>() * (i + 1) as f64) as usize;
                    list.swap(i, j);
                }
                Ok(Value::List(Arc::new(list)))
            }
            "sample" => {
                if list.is_empty() {
                    Ok(Value::Null)
                } else {
                    let idx = (rand::random::<f64>() * list.len() as f64) as usize;
                    Ok(list[idx].clone())
                }
            }
            "take" => {
                let n = match args.first() {
                    Some(Value::Number(n)) => *n as usize,
                    _ => return Err("take expects a number".into()),
                };
                Ok(Value::List(Arc::new(list.iter().cloned().take(n).collect::<Vec<Value>>())))
            }
            "drop" => {
                let n = match args.first() {
                    Some(Value::Number(n)) => *n as usize,
                    _ => return Err("drop expects a number".into()),
                };
                Ok(Value::List(Arc::new(list.iter().cloned().skip(n).collect::<Vec<Value>>())))
            }
            "chunk" => {
                let size = match args.first() {
                    Some(Value::Number(n)) => *n as usize,
                    _ => return Err("chunk expects a number".into()),
                };
                let size = size.max(1);
                let mut out = Vec::new();
                let mut i = 0;
                while i < list.len() {
                    let end = (i + size).min(list.len());
                    out.push(Value::List(Arc::new(list[i..end].to_vec())));
                    i += size;
                }
                Ok(Value::List(Arc::new(out)))
            }
            "copy" => Ok(Value::List(Arc::new(list))),
            "slice" => {
                let (start, end) = match args.as_slice() {
                    [Value::Number(s), Value::Number(e)] => (*s as usize, Some(*e as usize)),
                    [Value::Number(s)] => (*s as usize, None),
                    _ => return Err("slice expects (start[, end])".into()),
                };
                let start = start.min(list.len());
                let end = end.unwrap_or(list.len()).min(list.len());
                Ok(Value::List(Arc::new(list[start..end].to_vec())))
            }
            "splice" => {
                let (start, delete_count) = match args.as_slice() {
                    [Value::Number(s), Value::Number(d)] => (*s as usize, *d as usize),
                    [Value::Number(s)] => (*s as usize, 0),
                    _ => return Err("splice expects (start[, delete_count])".into()),
                };
                let mut list = list;
                let start = start.min(list.len());
                let delete_count = delete_count.min(list.len() - start);
                let removed: Vec<Value> = list.drain(start..start + delete_count).collect();
                let insert: Vec<Value> = args.into_iter().skip(2).collect();
                for (i, item) in insert.into_iter().enumerate() {
                    list.insert(start + i, item);
                }
                Ok(Value::List(Arc::new(removed)))
            }
            "zip" => {
                let other = match args.first() {
                    Some(Value::List(l)) => l,
                    _ => return Err("zip expects a list".into()),
                };
                let len = list.len().min(other.len());
                let out: Vec<Value> = (0..len)
                    .map(|i| Value::List(Arc::new(vec![list[i].clone(), other[i].clone()])))
                    .collect();
                Ok(Value::List(Arc::new(out)))
            }
            "reduce" => {
                let f = match args.first().cloned() {
                    Some(v) => v,
                    None => return Err("reduce expects a function".into()),
                };
                let has_init = args.len() > 1;
                let init = args.get(1).cloned();
                let mut acc = match init {
                    Some(v) => v,
                    None => {
                        if list.is_empty() {
                            return Err("reduce of empty list with no initial value".into());
                        }
                        let mut l = list.clone();
                        l.remove(0)
                    }
                };
                let items: Vec<Value> = if has_init {
                    list
                } else {
                    list.into_iter().skip(1).collect()
                };
                for item in items {
                    acc = self.apply_func(&f, vec![acc, item])?;
                }
                Ok(acc)
            }
            "find" => {
                let f = match args.first().cloned() {
                    Some(v) => v,
                    None => return Err("find expects a function".into()),
                };
                for item in &list {
                    if self.apply_func(&f, vec![item.clone()])?.truthy() {
                        return Ok(item.clone());
                    }
                }
                Ok(Value::Null)
            }
            "some" => {
                let f = match args.first().cloned() {
                    Some(v) => v,
                    None => return Err("some expects a function".into()),
                };
                for item in &list {
                    if self.apply_func(&f, vec![item.clone()])?.truthy() {
                        return Ok(Value::Bool(true));
                    }
                }
                Ok(Value::Bool(false))
            }
            "every" => {
                let f = match args.first().cloned() {
                    Some(v) => v,
                    None => return Err("every expects a function".into()),
                };
                for item in &list {
                    if !self.apply_func(&f, vec![item.clone()])?.truthy() {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
            "fill" => {
                let val = match args.first() {
                    Some(v) => v.clone(),
                    None => return Err("fill expects a value".into()),
                };
                Ok(Value::List(Arc::new(vec![val; list.len()])))
            }
            _ => Err(format!("list has no method: {method}")),
        }
    }
    fn dict_method(&mut self, dict: BTreeMap<String, Value>, method: &str, args: Vec<Value>) -> Result<Value, String> {
        match method {
            "has" | "containsKey" | "has_key" | "contains" => {
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
            "keys" => Ok(Value::List(Arc::new(
                dict.keys().map(|k| Value::String(k.clone())).collect::<Vec<Value>>(),
            ))),
            "values" => Ok(Value::List(Arc::new(dict.into_values().collect::<Vec<Value>>()))),
            "items" => Ok(Value::List(Arc::new(
                dict.iter().map(|(k, v)| Value::List(Arc::new(vec![Value::String(k.clone()), v.clone()]))).collect::<Vec<Value>>(),
            ))),
            "set" => {
                let (key, value) = match args.as_slice() {
                    [Value::String(k), v] => (k.clone(), v.clone()),
                    _ => return Err("set expects (key, value)".into()),
                };
                let mut dict = dict;
                dict.insert(key, value);
                Ok(Value::Dict(Arc::new(dict)))
            }
            "delete" | "remove" => {
                let key = match args.first() {
                    Some(Value::String(k)) => k.clone(),
                    _ => return Err("{method} expects a string key".into()),
                };
                let mut dict = dict;
                dict.remove(&key);
                Ok(Value::Dict(Arc::new(dict)))
            }
            "update" | "merge" => {
                let mut dict = dict;
                for arg in args {
                    match arg {
                        Value::Dict(other) => {
                            for (k, v) in Arc::unwrap_or_clone(other) {
                                dict.insert(k, v);
                            }
                        }
                        _ => return Err(format!("{method} expects a dictionary argument")),
                    }
                }
                Ok(Value::Dict(Arc::new(dict)))
            }
            "length" => Ok(Value::Number(dict.len() as f64)),
            _ => Err(format!("dictionary has no method: {method}")),
        }
    }
    #[inline(always)]
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
                    Arc::make_mut(&mut x).extend(y.iter().cloned());
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
                if let (Value::Number(x), Value::Number(y)) = (&a, &b) {
                    let x = *x;
                    let y = *y;
                    match op {
                        Kind::Minus => Ok(Value::Number(x - y)),
                        Kind::Slash => {
                            if y == 0.0 {
                                return Err("division by zero\n  \x1b[1;33m= help:\x1b[0m check if the divisor is zero before dividing".into());
                            }
                            Ok(Value::Number(x / y))
                        },
                        Kind::Percent => {
                            if y == 0.0 {
                                return Err("modulo by zero\n  \x1b[1;33m= help:\x1b[0m check if the divisor is zero before modulo".into());
                            }
                            Ok(Value::Number(x % y))
                        },
                        Kind::Pow => Ok(Value::Number(x.powf(y))),
                        _ => unreachable!(),
                    }
                } else {
                    let at = a.type_name();
                    let bt = b.type_name();
                    let hint = if (at == "string" || bt == "string") && matches!(op, Kind::Minus | Kind::Slash | Kind::Percent) {
                        "\n  \x1b[1;33m= help:\x1b[0m to concatenate strings, use `+` instead"
                    } else if matches!(op, Kind::Slash) && matches!(b, Value::Number(n) if n == 0.0) {
                        "\n  \x1b[1;33m= help:\x1b[0m cannot divide by zero — check if the divisor is zero before dividing"
                    } else {
                        ""
                    };
                    Err(format!("unsupported operand type(s) for {}: `{}` and `{}`{}", op_symbol(op), at, bt, hint))
                }
            }
            Kind::Lt | Kind::Le | Kind::Gt | Kind::Ge => {
                match (&a, &b) {
                    (Value::Number(x), Value::Number(y)) => {
                        Ok(Value::Bool(match op {
                            Kind::Lt => x < y,
                            Kind::Le => x <= y,
                            Kind::Gt => x > y,
                            Kind::Ge => x >= y,
                            _ => unreachable!(),
                        }))
                    }
                    _ => {
                        let at = a.type_name();
                        let bt = b.type_name();
                        let hint = if at == "string" && bt == "string" {
                            ""
                        } else if at != "int" && at != "float" && bt != "int" && bt != "float" {
                            "\nnote: '<', '>', '<=', '>=' only work on numbers\n      for string comparison, use == or !="
                        } else {
                            ""
                        };
                        Err(format!("unsupported operand type(s) for {}: `{}` and `{}`{}", op_symbol(op), at, bt, hint))
                    }
                }
            }
            _ => Err("unsupported operator".into()),
        }
    }
    fn run_module(&mut self, path: &str, namespace: &str) -> Result<HashMap<String, Value>, String> {
        let stmts = parse_file(path).map_err(|e| format!("\x1b[1;31merror\x1b[0m\x1b[1m[{}]\x1b[0m\n \x1b[1;34m-->\x1b[0m {}:1\n  \x1b[1;34m|\x1b[0m\n  \x1b[1;31m= {}\x1b[0m", e, path, e))?;
        let mut module_vm = Vm::new();
        // Snapshot the pristine baseline BEFORE seeding from the parent, so
        // names the parent happens to share with this module (e.g. plain-name
        // function registrations left by an earlier `from mod import fn`) are
        // not mistaken for pre-existing builtins and filtered out of exports.
        let initial_keys: std::collections::HashSet<String> = module_vm.vars.keys().cloned().collect();
        let initial_fns: std::collections::HashSet<String> = module_vm.functions.keys().cloned().collect();
        let initial_classes: std::collections::HashSet<String> = module_vm.classes.keys().cloned().collect();
        module_vm.functions = self.functions.clone();
        module_vm.native_functions = self.native_functions.clone();
        module_vm.classes = self.classes.clone();
        module_vm.file = path.into();
        if let Ok(source) = fs::read_to_string(path) {
            module_vm.lines = source.lines().map(|l| l.to_string()).collect();
        }
        module_vm.exec_module(&stmts)?;
        // Register the module's functions under a namespaced key in the caller so
        // `module.func(...)` calls resolve through self.functions.
        for (fname, function) in &module_vm.functions {
            if !initial_fns.contains(fname) {
                let key = format!("{namespace}::{fname}");
                self.register_function(key, function.clone());
                // Also expose under the plain name so closures escaping the module
                // (e.g. logging handlers) can resolve module helpers by name.
                self.register_function(fname.clone(), function.clone());
            }
        }
        // Register the module's classes under a namespaced key so `new module.Class(...)` works.
        for (class, def) in &module_vm.classes {
            if !initial_classes.contains(class) {
                let key = format!("{namespace}.{class}");
                self.classes.insert(key, def.clone());
                // Also expose under the plain name so module factories that do
                // `new Class(...)` internally resolve.
                self.classes.insert(class.clone(), def.clone());
            }
        }
        let mut exports: HashMap<String, Value> = module_vm.vars.into_iter()
            .filter(|(k, _)| !initial_keys.contains(k))
            .collect();
        for (fname, _fn) in &module_vm.functions {
            if !initial_fns.contains(fname) && !exports.contains_key(fname) {
                exports.insert(
                    fname.clone(),
                    Value::Function(format!("{namespace}::{fname}")),
                );
            }
        }
        for (class, _def) in &module_vm.classes {
            if !initial_classes.contains(class) && !exports.contains_key(class) {
                exports.insert(
                    class.clone(),
                    Value::Function(format!("{namespace}.{class}")),
                );
            }
        }
        Ok(exports)
    }

    fn resolve_module(&self, name: &str) -> Result<String, String> {
        // Absolute path: /path/to/module.z or /path/to/module
        if name.starts_with('/') || name.starts_with("./") || name.starts_with("../") {
            let candidates = [
                name.to_string(),
                format!("{name}.z"),
                format!("{name}.zen"),
                format!("{name}/main.z"),
                format!("{name}/main.zen"),
            ];
            for c in &candidates {
                if std::path::Path::new(c).exists() {
                    return Ok(c.clone());
                }
            }
            return Err(format!("module not found: {name}\n  \x1b[1;33m= note:\x1b[0m  searched for `{name}.z` and `{name}/main.z` in the current directory and std/"));
        }
        // Dotted names: pkg.sub.mod -> pkg/sub/mod.z, pkg/sub/main.z, etc.
        let parts: Vec<&str> = name.split('.').collect();
        if parts.len() > 1 {
            let path_name = parts.join("/");
            let base_candidates = [
                format!("{path_name}.z"),
                format!("{path_name}.zen"),
                format!("{path_name}/main.z"),
                format!("{path_name}/main.zen"),
                format!("{path_name}/{}", parts.last().unwrap_or(&"")),
            ];
            // Try local first
            for c in &base_candidates {
                if std::path::Path::new(c).exists() {
                    return Ok(c.clone());
                }
            }
            // Try PM modules dir
            let pm_dir = crate::pm::modules_dir();
            for c in &base_candidates {
                let pm_path = pm_dir.join(c);
                if pm_path.is_file() {
                    return Ok(pm_path.to_string_lossy().into_owned());
                }
            }
        }
        // Simple name: try local, pm, std
        let local = format!("{name}.z");
        if std::path::Path::new(&local).exists() {
            return Ok(local);
        }
        let local_zen = format!("{name}.zen");
        if std::path::Path::new(&local_zen).exists() {
            return Ok(local_zen);
        }
        // Package directory: name/main.z or name/name.z
        let pkg_main = format!("{name}/main.z");
        let pkg_self = format!("{name}/{}.z", name);
        if std::path::Path::new(&pkg_main).exists() {
            return Ok(pkg_main);
        }
        if std::path::Path::new(&pkg_self).exists() {
            return Ok(pkg_self);
        }
        let pkg_main_zen = format!("{name}/main.zen");
        let pkg_self_zen = format!("{name}/{}.zen", name);
        if std::path::Path::new(&pkg_main_zen).exists() {
            return Ok(pkg_main_zen);
        }
        if std::path::Path::new(&pkg_self_zen).exists() {
            return Ok(pkg_self_zen);
        }
        if let Some(path) = crate::pm::resolve_module_file(name) {
            return Ok(path);
        }
        if let Some(path) = find_std_file(&format!("{name}.z")) {
            return Ok(path);
        }
        if let Some(path) = find_std_file(&format!("{name}.zen")) {
            return Ok(path);
        }
        Err(format!("module not found: {name}"))
    }

    fn register_function(&mut self, name: String, function: Function) {
        self.fn_generation = self.fn_generation.wrapping_add(1);
        if let Ok(mut registry) = function_registry().lock() {
            registry.insert(name.clone(), function.clone());
        }
        self.functions.insert(name, function);
    }

    /// Evaluate default-value expressions for a function's parameters at
    /// definition time, producing a value parallel to `params`.
    fn eval_default_values(
        &mut self,
        params: &[(String, Option<Expr>)],
    ) -> Result<Vec<Option<Value>>, String> {
        let mut out = Vec::with_capacity(params.len());
        for (_, default) in params {
            out.push(match default {
                Some(e) => Some(self.eval(e)?),
                None => None,
            });
        }
        Ok(out)
    }

    /// Evaluate a block used as an expression (`if`-expression branch).
    /// Returns the value of the last expression statement, or the result of an
    /// explicit `return`, or Null.
    fn eval_block_expr(&mut self, body: &[Stmt]) -> Result<Value, String> {
        let mut result = Value::Null;
        for stmt in body {
            match &stmt.kind {
                StmtKind::Expr(e) => {
                    result = self.eval(e)?;
                }
                StmtKind::Let(_, e, _) => {
                    let v = self.eval(e)?;
                    match &stmt.kind {
                        StmtKind::Let(LetTarget::Var(n), _, _) => {
                            self.vars.insert(n.clone(), v.clone());
                        }
                        StmtKind::Let(LetTarget::List(names), _, _) => {
                            if let Value::List(items) = v.clone() {
                                for (i, name) in names.iter().enumerate() {
                                    if let Some(item) = items.get(i) {
                                        self.vars.insert(name.clone(), item.clone());
                                    }
                                }
                            }
                        }
                        StmtKind::Let(LetTarget::Dict(names), _, _) => {
                            if let Value::Dict(dict) = v.clone() {
                                for name in names {
                                    if let Some(item) = dict.get(name) {
                                        self.vars.insert(name.clone(), item.clone());
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    result = v;
                }
                _ => {
                    match self.exec(std::slice::from_ref(stmt))? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Return(v) => return Ok(v),
                        Flow::Break => break,
                        Flow::Throw(v) => return Err(format!("unhandled exception: {v}")),
                    }
                }
            }
        }
        Ok(result)
    }

    fn call(&mut self, name: &str, values: Vec<Value>) -> Result<Flow, String> {
        // Fast path: repeated calls to the same compiled function (recursion).
        if let Some(c) = &self.call_cache {
            if c.generation == self.fn_generation
                && c.name == name
                && values.len() == c.param_count
            {
                let bc = Arc::clone(&c.bc);
                let captured = if c.captured.is_empty() {
                    HashMap::new()
                } else {
                    c.captured.clone()
                };
                let flow = self.run_bytecode(&bc, values, &captured)?;
                return Ok(match flow {
                    Flow::Return(value) => Flow::Return(value),
                    Flow::Throw(value) => Flow::Throw(value),
                    Flow::Normal => Flow::Return(Value::Null),
                    Flow::Break | Flow::Continue => {
                        return Err(format!("loop control escaped function: {name}"))
                    }
                });
            }
        }
        // Check native functions first (most common fast path for builtins)
        if name == "help" {
            return Ok(Flow::Return(self.help(values.first())?));
        }
        if let Some(&native_fn) = self.native_functions.get(name) {
            return Ok(Flow::Return(native_fn(values)?));
        }

        // Check registered functions directly (hot path for user-defined functions)
        if let Some(function) = self.functions.get(name) {
            let argc = values.len();
            let required = function.required_count();
            if argc > function.params.len() || argc < required {
                return Err(format!(
                    "{name} expects {} arguments, got {}",
                    function.params.len(),
                    argc
                ));
            }
            let bind_args = |function: &Function, values: &[Value]| -> Vec<Value> {
                let mut bound = values.to_vec();
                for i in bound.len()..function.params.len() {
                    bound.push(
                        function.default_values[i]
                            .clone()
                            .unwrap_or(Value::Null),
                    );
                }
                bound
            };
            // Bytecode fast path: run the compiled body if available.
            if let Some(bc) = &function.bytecode {
                let bc = Arc::clone(bc);
                let captured = if function.captured.is_empty() {
                    HashMap::new()
                } else {
                    function.captured.clone()
                };
                let bound = bind_args(&function, &values);
                let param_count = function.params.len();
                let cache_matches = self
                    .call_cache
                    .as_ref()
                    .is_some_and(|c| c.name == name && c.generation == self.fn_generation);
                if !cache_matches {
                    self.call_cache = Some(CallCache {
                        name: name.to_string(),
                        bc: Arc::clone(&bc),
                        param_count,
                        captured: captured.clone(),
                        generation: self.fn_generation,
                    });
                }
                let flow = self.run_bytecode(&bc, bound, &captured)?;
                return Ok(match flow {
                    Flow::Return(value) => Flow::Return(value),
                    Flow::Throw(value) => Flow::Throw(value),
                    Flow::Normal => Flow::Return(Value::Null),
                    Flow::Break | Flow::Continue => {
                        return Err(format!("loop control escaped function: {name}"))
                    }
                });
            }
            let body = Arc::clone(&function.body);
            let cap_count = function.effective_captured.len();
            let bound = bind_args(&function, &values);
            // Save locals stack position, push params + captured onto fast local stack
            let saved_len = self.locals.len();
            self.frame_starts.push(saved_len);
            self.locals.reserve(function.params.len() + cap_count);
            for (parameter, value) in function.params.iter().zip(bound) {
                self.locals.push((parameter.0.clone(), value));
            }
            for (k, v) in &function.effective_captured {
                self.locals.push((k.clone(), v.clone()));
            }
            self.capture_frames.push(function.effective_captured.iter().map(|(k, _)| k.clone()).collect());
            let flow = self.exec(&body);
            self.capture_frames.pop();
            // Restore locals stack
            self.locals.truncate(saved_len);
            self.frame_starts.pop();
            let flow = flow?;
            return Ok(match flow {
                Flow::Return(value) => Flow::Return(value),
                Flow::Throw(value) => Flow::Throw(value),
                Flow::Normal => Flow::Return(Value::Null),
                Flow::Break | Flow::Continue => {
                    return Err(format!("loop control escaped function: {name}"))
                }
            });
        }

        // Fall back to variable-based function lookup (for function pointers stored in vars)
        if let Some(val) = self.vars.get(name) {
            match val {
                Value::NativeFunction(native_name) => {
                    if let Some(&native_fn) = self.native_functions.get(native_name) {
                        return Ok(Flow::Return(native_fn(values)?));
                    }
                }
                Value::Function(fname) => {
                    let fname = fname.clone();
                    return self.call(&fname, values);
                }
                _ => {}
            }
        }

        // Implicit-self method call: a bare `find_index(...)` inside a method
        // resolves to `self.find_index(...)`.
        if self.current_class.is_some() {
            if let Some((_, Value::Instance(instance))) = self
                .locals
                .iter()
                .rev()
                .find(|(n, _)| n == "self")
            {
                let instance = instance.clone();
                let class_name = instance.lock().unwrap().class_name.clone();
                if self.find_method(&class_name, name).is_some() {
                    return self.call_method(instance, name, values);
                }
            }
        }

        Err(format!("undefined function: {name}"))
    }
    /// Execute a compiled function body. Returns a Flow like `exec` does so
    /// `Flow::Throw` propagates to tree-walk try/catch callers.
    fn run_bytecode(
        &mut self,
        bc: &std::sync::Arc<crate::bytecode::CompiledFunction>,
        args: Vec<Value>,
        captured: &HashMap<String, Value>,
    ) -> Result<Flow, String> {
        use crate::bytecode::Opcode;
        let mut cur = Arc::clone(bc);
        let mut locals: Vec<Value> = Vec::new();
        locals.resize(cur.local_count as usize, Value::Null);
        for (i, v) in args.into_iter().enumerate() {
            if i < cur.param_count as usize {
                locals[i] = v;
            }
        }
        for (i, name) in cur.captured_names.iter().enumerate() {
            if let Some(v) = captured.get(name) {
                let idx = cur.param_count as usize + i;
                if idx < locals.len() {
                    locals[idx] = v.clone();
                }
            }
        }
        // Pre-size the operand stack: avoids repeated reallocation in hot loops.
        let mut stack: Vec<Value> = Vec::with_capacity(64);
        let mut ip: usize = 0;
        let mut frames: Vec<(
            Arc<crate::bytecode::CompiledFunction>,
            usize,
            usize,
            usize,
            usize,
        )> = Vec::new();
        let mut base: usize = 0;
        loop {
            if ip >= cur.instructions.len() {
                if let Some((fbc, fip, fbase, fnew_base, fstack_len)) = frames.pop() {
                    locals.truncate(fnew_base);
                    stack.truncate(fstack_len);
                    stack.push(Value::Null);
                    cur = fbc;
                    ip = fip;
                    base = fbase;
                    continue;
                }
                return Ok(Flow::Normal);
            }
            let inst = cur.instructions[ip];
            ip += 1;
            match inst.opcode {
                Opcode::Pop => {
                    stack.pop();
                }
                Opcode::Dup => {
                    if let Some(top) = stack.last().cloned() {
                        stack.push(top);
                    }
                }
                Opcode::Const => {
                    let c = cur.constants[inst.arg1 as usize].clone();
                    stack.push(c);
                }
                Opcode::True => stack.push(Value::Bool(true)),
                Opcode::False => stack.push(Value::Bool(false)),
                Opcode::Null => stack.push(Value::Null),
                Opcode::LoadLocal => {
                    let idx = base + inst.arg1 as usize;
                    let v = locals.get(idx).cloned().unwrap_or(Value::Null);
                    stack.push(v);
                }
                Opcode::StoreLocal => {
                    if let Some(v) = stack.pop() {
                        let idx = base + inst.arg1 as usize;
                        if let Some(slot) = locals.get_mut(idx) {
                            *slot = v.clone();
                        }
                        // Propagate captured variable mutations to enclosing scope
                        if idx >= cur.param_count as usize
                            && idx < cur.param_count as usize + cur.captured_names.len()
                        {
                            let captured_idx = idx - cur.param_count as usize;
                            if let Some(cname) = cur.captured_names.get(captured_idx) {
                                self.vars.insert(cname.clone(), v.clone());
                                if let Some(f) = self.functions.get_mut(&cur.name) {
                                    f.captured.insert(cname.clone(), v.clone());
                                }
                                // Invalidate call cache so next call uses updated captures
                                self.call_cache = None;
                            }
                        }
                    }
                }
                Opcode::LoadGlobal => {
                    let Value::String(name) = &cur.constants[inst.arg1 as usize] else {
                        return Err("bad constant in LoadGlobal".into());
                    };
                    if let Some(v) = self.vars.get(name.as_str()) {
                        stack.push(v.clone());
                    } else if let Some(_f) = self.functions.get(name.as_str()) {
                        stack.push(Value::Function(name.clone()));
                    } else if let Some(vars) = self.imported_modules.get(name.as_str()) {
                        let val = if let Some(Value::Dict(module_dict)) = vars.get(name.as_str()) {
                            Value::Dict(module_dict.clone())
                        } else {
                            Value::Dict(Arc::new(vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect::<BTreeMap<String, Value>>()))
                        };
                        stack.push(val);
                    } else {
                        return Err(format!("undefined variable: `{name}`"));
                    }
                }
                Opcode::StoreGlobal => {
                    let Value::String(name) = &cur.constants[inst.arg1 as usize] else {
                        return Err("bad constant in StoreGlobal".into());
                    };
                    let v = stack.pop().unwrap_or(Value::Null);
                    self.vars.insert(name.clone(), v);
                }
                Opcode::CheckLockedAssign => {
                    let Value::String(name) = &cur.constants[inst.arg1 as usize] else {
                        return Err("bad constant in CheckLockedAssign".into());
                    };
                    if self.locked.contains(name.as_str()) {
                        return Err(format!(
                            "cannot assign to constant: {name}\n  \x1b[1;33m= note:\x1b[0m  `{name}` was declared with `const` and cannot be reassigned\n  \x1b[1;33m= help:\x1b[0m use `let` instead of `const` if you need a mutable variable"
                        ));
                    }
                }
                Opcode::CheckLockedRedefine => {
                    let Value::String(name) = &cur.constants[inst.arg1 as usize] else {
                        return Err("bad constant in CheckLockedRedefine".into());
                    };
                    if self.locked.contains(name.as_str()) {
                        return Err(format!(
                            "cannot redefine constant: {name}\n  \x1b[1;33m= note:\x1b[0m  `{name}` was declared with `const` and cannot be changed\n  \x1b[1;33m= help:\x1b[0m use `let {name} = ...` if you need a mutable variable"
                        ));
                    }
                }
                Opcode::LockGlobal => {
                    let Value::String(name) = &cur.constants[inst.arg1 as usize] else {
                        return Err("bad constant in LockGlobal".into());
                    };
                    self.locked.insert(name.clone());
                }
                Opcode::UnlockGlobal => {
                    let Value::String(name) = &cur.constants[inst.arg1 as usize] else {
                        return Err("bad constant in UnlockGlobal".into());
                    };
                    self.locked.remove(name.as_str());
                }
                Opcode::AddGlobal
                | Opcode::SubGlobal
                | Opcode::MulGlobal
                | Opcode::DivGlobal
                | Opcode::ModGlobal => {
                    let Value::String(name) = &cur.constants[inst.arg1 as usize] else {
                        return Err("bad constant in compound assign".into());
                    };
                    if self.locked.contains(name.as_str()) {
                        return Err(format!(
                            "cannot assign to constant: {name}\n  \x1b[1;33m= note:\x1b[0m  `{name}` was declared with `const` and cannot be reassigned\n  \x1b[1;33m= help:\x1b[0m use `let` instead of `const` if you need a mutable variable"
                        ));
                    }
                    let rhs = stack.pop().unwrap_or(Value::Null);
                    let current = self.vars.get(name.as_str()).cloned().unwrap_or(Value::Null);
                    let v = match (inst.opcode, current, rhs) {
                        (Opcode::AddGlobal, Value::Number(x), Value::Number(y)) => {
                            Value::Number(x + y)
                        }
                        (Opcode::SubGlobal, Value::Number(x), Value::Number(y)) => {
                            Value::Number(x - y)
                        }
                        (Opcode::MulGlobal, Value::Number(x), Value::Number(y)) => {
                            Value::Number(x * y)
                        }
                        (Opcode::DivGlobal, Value::Number(x), Value::Number(y)) => {
                            Value::Number(x / y)
                        }
                        (Opcode::ModGlobal, Value::Number(x), Value::Number(y)) => {
                            Value::Number(x % y)
                        }
                        (op, current, rhs) => {
                            let k = match op {
                                Opcode::AddGlobal => &Kind::Plus,
                                Opcode::SubGlobal => &Kind::Minus,
                                Opcode::MulGlobal => &Kind::Star,
                                Opcode::DivGlobal => &Kind::Slash,
                                Opcode::ModGlobal => &Kind::Percent,
                                _ => unreachable!(),
                            };
                            self.binary(current, k, rhs)?
                        }
                    };
                    self.vars.insert(name.clone(), v);
                }
                Opcode::AddLocal
                | Opcode::SubLocal
                | Opcode::MulLocal
                | Opcode::DivLocal
                | Opcode::ModLocal => {
                    let slot = base + inst.arg1 as usize;
                    let rhs = stack.pop().unwrap_or(Value::Null);
                    let current = locals.get(slot).cloned().unwrap_or(Value::Null);
                    let v = match (inst.opcode, current, rhs) {
                        (Opcode::AddLocal, Value::Number(x), Value::Number(y)) => {
                            Value::Number(x + y)
                        }
                        (Opcode::SubLocal, Value::Number(x), Value::Number(y)) => {
                            Value::Number(x - y)
                        }
                        (Opcode::MulLocal, Value::Number(x), Value::Number(y)) => {
                            Value::Number(x * y)
                        }
                        (Opcode::DivLocal, Value::Number(x), Value::Number(y)) => {
                            Value::Number(x / y)
                        }
                        (Opcode::ModLocal, Value::Number(x), Value::Number(y)) => {
                            Value::Number(x % y)
                        }
                        (op, current, rhs) => {
                            let k = match op {
                                Opcode::AddLocal => &Kind::Plus,
                                Opcode::SubLocal => &Kind::Minus,
                                Opcode::MulLocal => &Kind::Star,
                                Opcode::DivLocal => &Kind::Slash,
                                Opcode::ModLocal => &Kind::Percent,
                                _ => unreachable!(),
                            };
                            self.binary(current, k, rhs)?
                        }
                    };
                    if let Some(slot_v) = locals.get_mut(slot) {
                        *slot_v = v.clone();
                    }
                    // Propagate captured variable mutations to enclosing scope
                    if slot >= cur.param_count as usize
                        && slot < cur.param_count as usize + cur.captured_names.len()
                    {
                        let captured_idx = slot - cur.param_count as usize;
                        if let Some(cname) = cur.captured_names.get(captured_idx) {
                            self.vars.insert(cname.clone(), v.clone());
                            if let Some(f) = self.functions.get_mut(&cur.name) {
                                f.captured.insert(cname.clone(), v.clone());
                            }
                            // Invalidate call cache so next call uses updated captures
                            self.call_cache = None;
                        }
                    }
                }
                Opcode::Add => {
                    let b = stack.pop().unwrap_or(Value::Null);
                    let a = stack.pop().unwrap_or(Value::Null);
                    let v = match (a, b) {
                        (Value::Number(x), Value::Number(y)) => Value::Number(x + y),
                        (a, b) => self.binary(a, &Kind::Plus, b)?,
                    };
                    stack.push(v);
                }
                Opcode::Sub => {
                    let b = stack.pop().unwrap_or(Value::Null);
                    let a = stack.pop().unwrap_or(Value::Null);
                    let v = match (a, b) {
                        (Value::Number(x), Value::Number(y)) => Value::Number(x - y),
                        (a, b) => self.binary(a, &Kind::Minus, b)?,
                    };
                    stack.push(v);
                }
                Opcode::Mul => {
                    let b = stack.pop().unwrap_or(Value::Null);
                    let a = stack.pop().unwrap_or(Value::Null);
                    let v = match (a, b) {
                        (Value::Number(x), Value::Number(y)) => Value::Number(x * y),
                        (a, b) => self.binary(a, &Kind::Star, b)?,
                    };
                    stack.push(v);
                }
                Opcode::Div => {
                    let b = stack.pop().unwrap_or(Value::Null);
                    let a = stack.pop().unwrap_or(Value::Null);
                    let v = match (a, b) {
                        (Value::Number(x), Value::Number(y)) => Value::Number(x / y),
                        (a, b) => self.binary(a, &Kind::Slash, b)?,
                    };
                    stack.push(v);
                }
                Opcode::Mod => {
                    let b = stack.pop().unwrap_or(Value::Null);
                    let a = stack.pop().unwrap_or(Value::Null);
                    let v = match (a, b) {
                        (Value::Number(x), Value::Number(y)) => Value::Number(x % y),
                        (a, b) => self.binary(a, &Kind::Percent, b)?,
                    };
                    stack.push(v);
                }
                Opcode::Pow => {
                    let b = stack.pop().unwrap_or(Value::Null);
                    let a = stack.pop().unwrap_or(Value::Null);
                    let v = match (a, b) {
                        (Value::Number(x), Value::Number(y)) => Value::Number(x.powf(y)),
                        (a, b) => self.binary(a, &Kind::Pow, b)?,
                    };
                    stack.push(v);
                }
                Opcode::Neg => {
                    let a = stack.pop().unwrap_or(Value::Null);
                    match a {
                        Value::Number(n) => stack.push(Value::Number(-n)),
                        _ => return Err(format!(
                            "cannot negate `{}`\nnote: negation (-) only works on numbers",
                            a.type_name()
                        )),
                    }
                }
                Opcode::Eq => {
                    let b = stack.pop().unwrap_or(Value::Null);
                    let a = stack.pop().unwrap_or(Value::Null);
                    stack.push(Value::Bool(a == b));
                }
                Opcode::Ne => {
                    let b = stack.pop().unwrap_or(Value::Null);
                    let a = stack.pop().unwrap_or(Value::Null);
                    stack.push(Value::Bool(a != b));
                }
                Opcode::Lt => {
                    let b = stack.pop().unwrap_or(Value::Null);
                    let a = stack.pop().unwrap_or(Value::Null);
                    let v = match (a, b) {
                        (Value::Number(x), Value::Number(y)) => Value::Bool(x < y),
                        (a, b) => self.binary(a, &Kind::Lt, b)?,
                    };
                    stack.push(v);
                }
                Opcode::Le => {
                    let b = stack.pop().unwrap_or(Value::Null);
                    let a = stack.pop().unwrap_or(Value::Null);
                    let v = match (a, b) {
                        (Value::Number(x), Value::Number(y)) => Value::Bool(x <= y),
                        (a, b) => self.binary(a, &Kind::Le, b)?,
                    };
                    stack.push(v);
                }
                Opcode::Gt => {
                    let b = stack.pop().unwrap_or(Value::Null);
                    let a = stack.pop().unwrap_or(Value::Null);
                    let v = match (a, b) {
                        (Value::Number(x), Value::Number(y)) => Value::Bool(x > y),
                        (a, b) => self.binary(a, &Kind::Gt, b)?,
                    };
                    stack.push(v);
                }
                Opcode::Ge => {
                    let b = stack.pop().unwrap_or(Value::Null);
                    let a = stack.pop().unwrap_or(Value::Null);
                    let v = match (a, b) {
                        (Value::Number(x), Value::Number(y)) => Value::Bool(x >= y),
                        (a, b) => self.binary(a, &Kind::Ge, b)?,
                    };
                    stack.push(v);
                }
                Opcode::Not => {
                    let a = stack.pop().unwrap_or(Value::Null);
                    stack.push(Value::Bool(!a.truthy()));
                }
                Opcode::Jmp => {
                    ip = inst.arg1 as usize;
                }
                Opcode::JmpLtLocal | Opcode::JmpLeLocal => {
                    let ia = base + inst.arg2 as usize;
                    let ib = base + inst.arg3 as usize;
                    let taken = match (locals.get(ia), locals.get(ib)) {
                        (Some(Value::Number(x)), Some(Value::Number(y))) => {
                            if inst.opcode == Opcode::JmpLtLocal { x >= y } else { x > y }
                        }
                        (Some(a), Some(b)) => {
                            let k = if inst.opcode == Opcode::JmpLtLocal { Kind::Lt } else { Kind::Le };
                            match self.binary(a.clone(), &k, b.clone())? {
                                Value::Bool(t) => !t,
                                other => !other.truthy(),
                            }
                        }
                        _ => return Err("bad slots in fused compare".into()),
                    };
                    if taken {
                        ip = inst.arg1 as usize;
                    }
                }
                Opcode::AddLocalImm | Opcode::SubLocalImm => {
                    let idx = base + inst.arg1 as usize;
                    let imm = match cur.constants.get(inst.arg2 as usize) {
                        Some(Value::Number(n)) => *n,
                        _ => return Err("bad constant in Add/SubLocalImm".into()),
                    };
                    let imm = if inst.opcode == Opcode::AddLocalImm { imm } else { -imm };
                    match locals.get_mut(idx) {
                        Some(Value::Number(x)) => *x += imm,
                        Some(slot_val) => {
                            let old = std::mem::replace(slot_val, Value::Null);
                            let nv = self.binary(old, &Kind::Plus, Value::Number(imm))?;
                            locals[idx] = nv;
                        }
                        None => return Err("bad slot in Add/SubLocalImm".into()),
                    }
                }
                Opcode::JmpIfFalse => {
                    let top = stack.pop().unwrap_or(Value::Null);
                    if !top.truthy() {
                        ip = inst.arg1 as usize;
                    }
                }
                Opcode::JmpIfTrue => {
                    let top = stack.pop().unwrap_or(Value::Null);
                    if top.truthy() {
                        ip = inst.arg1 as usize;
                    }
                }
                Opcode::JmpIfNotNull => {
                    let top = stack.pop().unwrap_or(Value::Null);
                    if !matches!(top, Value::Null) {
                        ip = inst.arg1 as usize;
                    }
                }
                Opcode::Call => {
                    let Value::String(name) = &cur.constants[inst.arg1 as usize] else {
                        return Err("bad constant in Call".into());
                    };
                    let argc = inst.arg2 as usize;
                    let start = stack.len().checked_sub(argc).unwrap_or(0);
                    // 1. Cache fast path: repeated calls to the same compiled
                    // function (recursion).
                    if let Some(c) = &self.call_cache {
                        if c.generation == self.fn_generation
                            && c.name.as_str() == name
                            && c.param_count == argc
                        {
                            let cbc = Arc::clone(&c.bc);
                            let captured = if c.captured.is_empty() {
                                HashMap::new()
                            } else {
                                c.captured.clone()
                            };
                            let new_base = locals.len();
                            frames.push((Arc::clone(&cur), ip, base, new_base, start));
                            for i in 0..argc {
                                locals.push(stack[start + i].clone());
                            }
                            stack.truncate(start);
                            locals.resize(new_base + cbc.local_count as usize, Value::Null);
                            for (i, cn) in cbc.captured_names.iter().enumerate() {
                                if let Some(v) = captured.get(cn) {
                                    locals[new_base + cbc.param_count as usize + i] = v.clone();
                                }
                            }
                            cur = cbc;
                            ip = 0;
                            base = new_base;
                            continue;
                        }
                    }
                    // 2. Native functions.
                    if let Some(&native_fn) = self.native_functions.get(name.as_str()) {
                        let mut vals = Vec::with_capacity(argc);
                        for i in 0..argc {
                            vals.push(stack[start + i].clone());
                        }
                        stack.truncate(start);
                        stack.push(native_fn(vals)?);
                        continue;
                    }
                    // 3. Registered user functions.
                    if let Some(function) = self.functions.get(name.as_str()) {
                        let total = function.params.len();
                        if argc > total || argc < function.required_count() {
                            return Err(format!(
                                "{name} expects {} arguments, got {}",
                                total, argc
                            ));
                        }
                        if let Some(cbc) = &function.bytecode {
                            let cbc = Arc::clone(cbc);
                            let captured = if function.captured.is_empty() {
                                HashMap::new()
                            } else {
                                function.captured.clone()
                            };
                            self.call_cache = Some(CallCache {
                                name: name.to_string(),
                                bc: Arc::clone(&cbc),
                                param_count: total,
                                captured: captured.clone(),
                                generation: self.fn_generation,
                            });
                            let new_base = locals.len();
                            frames.push((Arc::clone(&cur), ip, base, new_base, start));
                            for i in 0..argc {
                                locals.push(stack[start + i].clone());
                            }
                            for i in argc..total {
                                locals.push(
                                    function.default_values[i]
                                        .clone()
                                        .unwrap_or(Value::Null),
                                );
                            }
                            stack.truncate(start);
                            locals.resize(new_base + cbc.local_count as usize, Value::Null);
                            for (i, cn) in cbc.captured_names.iter().enumerate() {
                                if let Some(v) = captured.get(cn) {
                                    locals[new_base + cbc.param_count as usize + i] = v.clone();
                                }
                            }
                            cur = cbc;
                            ip = 0;
                            base = new_base;
                            continue;
                        }
                        let mut vals = Vec::with_capacity(argc);
                        for i in 0..argc {
                            vals.push(stack[start + i].clone());
                        }
                        stack.truncate(start);
                        let flow = self.call(name, vals)?;
                        match flow {
                            Flow::Return(v) => stack.push(v),
                            Flow::Throw(v) => return Ok(Flow::Throw(v)),
                            _ => stack.push(Value::Null),
                        }
                        continue;
                    }
                    // 4. Function pointers stored in variables.
                    if let Some(val) = self.vars.get(name.as_str()) {
                        match val {
                            Value::NativeFunction(n) => {
                                if let Some(&native_fn) = self.native_functions.get(n) {
                                    let mut vals = Vec::with_capacity(argc);
                                    for i in 0..argc {
                                        vals.push(stack[start + i].clone());
                                    }
                                    stack.truncate(start);
                                    stack.push(native_fn(vals)?);
                                    continue;
                                }
                            }
                            Value::Function(fname) => {
                                let fname = fname.clone();
                                let mut vals = Vec::with_capacity(argc);
                                for i in 0..argc {
                                    vals.push(stack[start + i].clone());
                                }
                                stack.truncate(start);
                                let flow = self.call(&fname, vals)?;
                                match flow {
                                    Flow::Return(v) => stack.push(v),
                                    Flow::Throw(v) => return Ok(Flow::Throw(v)),
                                    _ => stack.push(Value::Null),
                                }
                                continue;
                            }
                            _ => {}
                        }
                    }
                    return Err(format!("undefined function: {name}"));
                }
                Opcode::Return => {
                    let v = stack.pop().unwrap_or(Value::Null);
                    if cur.name == "f" {
                    }
if let Some((fbc, fip, fbase, fnew_base, fstack_len)) = frames.pop() {
                        locals.truncate(fnew_base);
                        stack.truncate(fstack_len);
                        stack.push(v);
                        cur = fbc;
                        ip = fip;
                        base = fbase;
                    } else {
                        return Ok(Flow::Return(v));
                    }
                }
                Opcode::Print => {
                    let count = inst.arg1 as usize;
                    let sep = if inst.arg2 == u16::MAX {
                        " ".to_string()
                    } else {
                        match &cur.constants[inst.arg2 as usize] {
                            Value::String(s) => s.clone(),
                            _ => " ".to_string(),
                        }
                    };
                    let end = if inst.arg3 == u16::MAX {
                        "\n".to_string()
                    } else {
                        match &cur.constants[inst.arg3 as usize] {
                            Value::String(s) => s.clone(),
                            _ => "\n".to_string(),
                        }
                    };
                    let mut vals = Vec::with_capacity(count);
                    for _ in 0..count {
                        vals.push(stack.pop().unwrap_or(Value::Null));
                    }
                    vals.reverse();
                    let text = vals
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(&sep);
                    print!("{text}{end}");
                }
                Opcode::BuildList => {
                    let count = inst.arg1 as usize;
                    let mut list = Vec::with_capacity(count);
                    for _ in 0..count {
                        list.push(stack.pop().unwrap_or(Value::Null));
                    }
                    list.reverse();
                    stack.push(Value::List(Arc::new(list)));
                }
                Opcode::BuildDict => {
                    let count = inst.arg1 as usize;
                    let mut dict = BTreeMap::new();
                    for _ in 0..count {
                        let val = stack.pop().unwrap_or(Value::Null);
                        let key = stack.pop().unwrap_or(Value::Null);
                        if let Value::String(k) = key {
                            dict.insert(k, val);
                        }
                    }
                    stack.push(Value::Dict(Arc::new(dict)));
                }
                Opcode::Index => {
                    let index = stack.pop().unwrap_or(Value::Null);
                    let collection = stack.pop().unwrap_or(Value::Null);
                    let v = match (collection, index) {
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
                    }?;
                    stack.push(v);
                }
                Opcode::GetMember => {
                    let obj = stack.pop().unwrap_or(Value::Null);
                    let Value::String(name) = &cur.constants[inst.arg1 as usize] else {
                        return Err("bad constant in GetMember".into());
                    };
                    let v = self.member(obj, name)?;
                    stack.push(v);
                }
                Opcode::PushSlot => {
                    let idx = base + inst.arg1 as usize;
                    let v = stack.pop().unwrap_or(Value::Null);
                    match locals.get_mut(idx) {
                        Some(Value::List(list)) => Arc::make_mut(list).push(v),
                        Some(Value::Null) => {
                            locals[idx] = Value::List(Arc::new(vec![v]));
                        }
                        _ => return Err("push target is not a list".into()),
                    }
                }
                Opcode::PopSlot => {
                    let idx = base + inst.arg1 as usize;
                    match locals.get_mut(idx) {
                        Some(Value::List(list)) => {
                            let popped = Arc::make_mut(list).pop().unwrap_or(Value::Null);
                            stack.push(popped);
                        }
                        _ => return Err("pop target is not a list".into()),
                    }
                }
                Opcode::PushGlobal => {
                    let Value::String(name) = &cur.constants[inst.arg1 as usize] else {
                        return Err("bad constant in PushGlobal".into());
                    };
                    let v = stack.pop().unwrap_or(Value::Null);
                    match self.vars.get_mut(name.as_str()) {
                        Some(Value::List(list)) => Arc::make_mut(list).push(v),
                        _ => {
                            let name = name.clone();
                            self.vars.insert(name, Value::List(Arc::new(vec![v])));
                        }
                    }
                }
                Opcode::PopGlobal => {
                    let Value::String(name) = &cur.constants[inst.arg1 as usize] else {
                        return Err("bad constant in PopGlobal".into());
                    };
                    match self.vars.get_mut(name.as_str()) {
                        Some(Value::List(list)) => {
                            let popped = Arc::make_mut(list).pop().unwrap_or(Value::Null);
                            stack.push(popped);
                        }
                        _ => return Err("pop target is not a list".into()),
                    }
                }
                Opcode::CallMethod => {
                    let argc = inst.arg2 as usize;
                    if stack.len() < argc + 1 {
                        return Err("stack underflow in CallMethod".into());
                    }
                    let args: Vec<Value> = stack.split_off(stack.len() - argc);
                    let target = stack.pop().unwrap_or(Value::Null);
                    let Value::String(name) = &cur.constants[inst.arg1 as usize] else {
                        return Err("bad constant in CallMethod".into());
                    };
                    let name = name.clone();
                    let v = self.invoke_member(target, &name, args, None)?;
                    stack.push(v);
                }
                Opcode::Typeof => {
                    let a = stack.pop().unwrap_or(Value::Null);
                    let s = a.type_name();
                    stack.push(Value::String(s.into()));
                }
                Opcode::Len => {
                    let a = stack.pop().unwrap_or(Value::Null);
                    match a {
                        Value::List(values) => stack.push(Value::Number(values.len() as f64)),
                        _ => return Err("for requires a list".into()),
                    }
                }
                Opcode::DefineFunction | Opcode::Closure => {
                    let func_idx = if inst.opcode == Opcode::DefineFunction {
                        inst.arg2 as usize
                    } else {
                        inst.arg1 as usize
                    };
                    let cf = self.compiled_functions.get(func_idx).cloned().ok_or_else(|| {
                        format!("compiled function table missing index {func_idx}")
                    })?;
                    let name = cf.name.clone();
                    let mut captured_map = HashMap::new();
                    for cn in &cf.captured_names {
                        if let Some(v) = self.vars.get(cn) {
                            captured_map.insert(cn.clone(), v.clone());
                        }
                    }
                    let effective_captured: Vec<(String, Value)> = captured_map
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    let function = Function {
                        params: cf.params.iter().cloned().map(|n| (n, None)).collect(),
                        default_values: Vec::new(),
                        body: Arc::new(Vec::new()),
                        captured: captured_map,
                        effective_captured,
                        bytecode: Some(cf),
                    };
                    self.register_function(name.clone(), function);
                    if inst.opcode == Opcode::Closure {
                        stack.push(Value::Function(name));
                    }
                }
            }
        }
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
    /// Look up `name` as an instance field of the current method's `self`.
    /// Returns Some(field value) when the name is a declared field (and not a
    /// method name) of the instance's class or one of its ancestors.
    /// Locals-first read: prefer the current frame's locals (params/captured),
    /// then the global scope.
    fn get_var(&mut self, name: &str) -> Option<Value> {
        let resolve = if name == "this" { "self" } else { name };
        if let Some((_, v)) = self.locals.iter().rev().find(|(n, _)| n == resolve) {
            return Some(v.clone());
        }
        self.vars.get(resolve).cloned()
    }
    /// Locals-first write: update the nearest local binding if one exists,
    /// otherwise store in the global scope.
    fn set_var(&mut self, name: &str, value: Value) {
        let resolve = if name == "this" { "self" } else { name };
        let mut found_in_locals = false;
        if let Some(idx) = self.locals.iter().rposition(|(n, _)| n == resolve) {
            self.locals[idx].1 = value.clone();
            found_in_locals = true;
        }
        // Propagate captured variable mutations back to the enclosing scope.
        // Only names actually CAPTUREED by this frame propagate; a local that
        // merely shadows an identically-named global stays isolated so module
        // function locals cannot clobber caller variables.
        let captured_here = self.capture_frames.last().is_some_and(|f| f.contains(resolve));
        if found_in_locals && (captured_here || self.capture_frames.is_empty()) && self.vars.contains_key(resolve) {
            self.vars.insert(resolve.to_string(), value);
        } else if !found_in_locals {
            self.vars.insert(resolve.to_string(), value);
        }
    }

    /// Bind a `let`-declared name. Inside a function body the declaration is
    /// function-local (update the innermost binding within THIS frame or push
    /// a new one); at top level it writes to global variables as before.
    fn bind_let(&mut self, name: &str, value: Value) {
        if !self.capture_frames.is_empty() {
            let start = self.frame_starts.last().copied().unwrap_or(0);
            if let Some(rel) = self.locals[start..].iter().rposition(|(n, _)| n == name) {
                self.locals[start + rel].1 = value;
            } else {
                self.locals.push((name.to_string(), value));
            }
        } else {
            self.vars.insert(name.to_string(), value);
        }
    }
    fn self_field_get(&mut self, name: &str) -> Option<Value> {
        let inst = {
            let mut found = None;
            for (k, v) in self.locals.iter().rev() {
                if k == "self" {
                    if let Value::Instance(i) = v {
                        found = Some(Arc::clone(i));
                    }
                    break;
                }
            }
            found?
        };
        let class_name = inst.lock().unwrap().class_name.clone();
        let mut cur = Some(class_name);
        while let Some(c) = cur {
            let class = self.classes.get(&c)?;
            if class.methods.contains_key(name) {
                return None;
            }
            if let Some(v) = inst.lock().unwrap().fields.get(name) {
                return Some(v.clone());
            }
            cur = class.parent.clone();
        }
        None
    }
    /// True when `name` is a declared field of the current method's `self`
    /// (checking the instance's class and its ancestors).
    fn is_self_field(&self, name: &str) -> bool {
        let Some(inst) = self.locals.iter().rev().find_map(|(k, v)| {
            if k == "self" {
                if let Value::Instance(i) = v {
                    Some(Arc::clone(i))
                } else {
                    None
                }
            } else {
                None
            }
        }) else {
            return false;
        };
        let class_name = inst.lock().unwrap().class_name.clone();
        let mut cur = Some(class_name);
        while let Some(c) = cur {
            let class = match self.classes.get(&c) {
                Some(c) => c,
                None => return false,
            };
            if class.methods.contains_key(name) {
                return false;
            }
            if class.fields.iter().any(|(f, _)| f == name) {
                return true;
            }
            cur = class.parent.clone();
        }
        false
    }
    /// Assign `value` to the instance field `name` on the current method's
    /// `self` if it is a declared field. Returns true when handled.
    fn self_field_set(&mut self, name: &str, value: Value) -> bool {
        let Some(inst) = (self.locals.iter().rev().find_map(|(k, v)| {
            if k == "self" {
                if let Value::Instance(i) = v {
                    Some(Arc::clone(i))
                } else {
                    None
                }
            } else {
                None
            }
        })) else {
            return false;
        };
        let class_name = inst.lock().unwrap().class_name.clone();
        let mut cur = Some(class_name);
        while let Some(c) = cur {
            let class = match self.classes.get(&c) {
                Some(c) => c,
                None => return false,
            };
            if class.methods.contains_key(name) {
                return false;
            }
            if class.fields.iter().any(|(f, _)| f == name) {
                inst.lock().unwrap().fields.insert(name.to_string(), value);
                return true;
            }
            cur = class.parent.clone();
        }
        false
    }
    /// If `object` refers to an instance field (a bare field name in a method,
    /// or `self.field`, or `var.field` where var holds an instance), return the
    /// instance and field name so list mutations can be written back.
    fn list_target_field(&self, object: &Expr) -> Option<(InstanceRef, String)> {
        match object {
            Expr::Member(inner, field) => {
                let inst = match &**inner {
                    Expr::Var(name) => {
                        let v = self
                            .locals
                            .iter()
                            .rev()
                            .find(|(k, _)| k == name)
                            .map(|(_, v)| v.clone())
                            .or_else(|| self.vars.get(name).cloned())?;
                        match v {
                            Value::Instance(i) => i,
                            _ => return None,
                        }
                    }
                    _ => return None,
                };
                Some((inst, field.clone()))
            }
            Expr::Var(name) => {
                if !self.is_self_field(name) {
                    return None;
                }
                let inst = self.locals.iter().rev().find_map(|(k, v)| {
                    if k == "self" {
                        if let Value::Instance(i) = v {
                            Some(Arc::clone(i))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })?;
                Some((inst, name.clone()))
            }
            _ => None,
        }
    }
    fn help(&mut self, val: Option<&Value>) -> Result<Value, String> {
        match val {
            None => {
                println!("Zen built-ins: print, input, len, str, int, float, bool, type,");
                println!(" range, dict, list, keys, values, items, has, push, pop,");
                println!(" slice, assert, throw, exit, sleep, wait, typeof, help,");
                println!(" min, max, abs, round, trunc, hex, chr, ord, cos, sin,");
                println!(" tan, sqrt, pow, floor, ceil, json, fs, os, time, random,");
                println!(" crypto, sys, re");
                println!("Operators: + - * / % ** & | ^ ~ << >> == != < > <= >= && || ??");
                println!("Keywords: let var const func return if elif else while for");
                println!(" break switch case => try catch as class extends super");
                println!(" new this self import from as default true false null");
                println!("Tip: type help(<value>) for info on any value.");
                Ok(Value::Null)
            }
            Some(val) => {
                match val {
                    Value::String(s) if self.classes.contains_key(s.as_str()) => {
                        let class_name = s.as_str();
                        let class = self.classes.get(class_name).unwrap();
                        println!("class {class_name}");
                        if let Some(p) = &class.parent {
                            println!("  extends {p}");
                        }
                        if !class.methods.is_empty() {
                            println!("  methods:");
                            let mut method_names: Vec<_> = class.methods.iter().collect();
                            method_names.sort_by_key(|(n, _)| n.clone());
                            for (name, func) in method_names {
                                let ps: Vec<String> = func.params.iter().map(|(p, _)| p.clone()).collect();
                                println!("    {name}({})", ps.join(", "));
                            }
                        }
                        if !class.fields.is_empty() {
                            println!("  fields:");
                            for (name, _) in &class.fields {
                                println!("    {name}");
                            }
                        }
                    }
                    Value::Instance(inst) => {
                        let i = inst.lock().unwrap();
                        println!("instance of {}", i.class_name);
                        if let Some(class) = self.classes.get(i.class_name.as_str()) {
                            if !class.methods.is_empty() {
                                println!("  methods:");
                                let mut method_names: Vec<_> = class.methods.iter().collect();
                                method_names.sort_by_key(|(n, _)| n.clone());
                                for (name, func) in method_names {
                                    let ps: Vec<String> = func.params.iter().map(|(p, _)| p.clone()).collect();
                                    println!("    {name}({})", ps.join(", "));
                                }
                            }
                        }
                        if !i.fields.is_empty() {
                            println!("  fields:");
                            for (k, v) in &i.fields {
                                println!("    {k} = {v}");
                            }
                        }
                    }
                    Value::Dict(d) => {
                        if d.contains_key("__doc__") {
                            if let Some(Value::Dict(doc)) = d.get("__doc__") {
                                if let Some(Value::String(desc)) = doc.get("description") {
                                    println!("{desc}");
                                }
                                if let Some(Value::Dict(funcs)) = doc.get("functions") {
                                    println!("\nfunctions:");
                                    let mut fnames: Vec<_> = funcs.iter().collect();
                                    fnames.sort_by_key(|(n, _)| n.clone());
                                    for (name, info) in fnames {
                                        match info {
                                            Value::Dict(fi) => {
                                                let params = fi.get("params")
                                                    .map(|v| match v {
                                                        Value::String(s) => s.clone(),
                                                        _ => String::new(),
                                                    })
                                                    .unwrap_or_default();
                                                let ret = fi.get("returns")
                                                    .map(|v| match v {
                                                        Value::String(s) => format!(" -> {s}"),
                                                        _ => String::new(),
                                                    })
                                                    .unwrap_or_default();
                                                let desc = fi.get("description")
                                                    .and_then(|v| match v {
                                                        Value::String(s) => Some(format!("  — {s}")),
                                                        _ => None,
                                                    })
                                                    .unwrap_or_default();
                                                println!("  {name}({params}){ret}{desc}");
                                            }
                                            _ => println!("  {name}"),
                                        }
                                    }
                                }
                                if let Some(Value::Dict(classes)) = doc.get("classes") {
                                    println!("\nclasses:");
                                    let mut cnames: Vec<_> = classes.iter().collect();
                                    cnames.sort_by_key(|(n, _)| n.clone());
                                    for (name, info) in cnames {
                                        match info {
                                            Value::Dict(ci) => {
                                                let desc = ci.get("description")
                                                    .and_then(|v| match v {
                                                        Value::String(s) => Some(format!("  — {s}")),
                                                        _ => None,
                                                    })
                                                    .unwrap_or_default();
                                                println!("  {name}{desc}");
                                            }
                                            _ => println!("  {name}"),
                                        }
                                    }
                                }
                            }
                        } else {
                            let keys: Vec<String> = d.keys().cloned().collect();
                            println!("dict with {} keys: {}", keys.len(), keys.join(", "));
                        }
                    }
                    Value::List(lst) => {
                        println!("list with {} elements", lst.len());
                        if !lst.is_empty() {
                            let preview: Vec<String> = lst.iter().take(5).map(|v| v.to_string()).collect();
                            println!("  [{}{}]", preview.join(", "),
                                if lst.len() > 5 { ", ..." } else { "" });
                        }
                    }
                    Value::String(s) => println!("string ({len}): \"{s}\"", len=s.len()),
                    Value::Number(n) => println!("number: {n}"),
                    Value::Bool(b) => println!("bool: {b}"),
                    Value::Null => println!("null"),
                    Value::NativeFunction(name) => println!("native function: {name}"),
                    Value::Function(name) => {
                        if let Some(class) = self.classes.get(name.as_str()) {
                            println!("class {name}");
                            if let Some(p) = &class.parent {
                                println!("  extends {p}");
                            }
                            if !class.methods.is_empty() {
                                println!("  methods:");
                                let mut method_names: Vec<_> = class.methods.iter().collect();
                                method_names.sort_by_key(|(n, _)| n.clone());
                                for (mname, func) in method_names {
                                    let ps: Vec<String> = func.params.iter().map(|(p, _)| p.clone()).collect();
                                    println!("    {mname}({})", ps.join(", "));
                                }
                            }
                            if !class.fields.is_empty() {
                                println!("  fields:");
                                for (fname, _) in &class.fields {
                                    println!("    {fname}");
                                }
                            }
                        } else {
                            println!("function: {name}");
                        }
                    }
                    _ => println!("{val}"),
                }
                Ok(Value::Null)
            }
        }
    }
    fn apply_func(&mut self, f: &Value, values: Vec<Value>) -> Result<Value, String> {
        match f {
            Value::NativeFunction(name) if name == "help" => {
                self.help(values.first())
            }
            Value::NativeFunction(name) => match self.native_functions.get(name.as_str()) {
                Some(&native_fn) => native_fn(values),
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

    /// Dispatch `obj.method(args)` for any value type. Shared by the tree-walk
    /// interpreter and the bytecode VM's CallMethod opcode. `object_expr` is the
    /// original expression when available (needed for push/pop field mutation).
    fn invoke_member(
        &mut self,
        obj: Value,
        method: &str,
        values: Vec<Value>,
        object_expr: Option<&Expr>,
    ) -> Result<Value, String> {
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
                        return native_fn(call_args);
                    }
                }
                if let Some(Value::Function(fname)) = dict.get(method) {
                    return match self.call(fname, values)? {
                        Flow::Return(v) => Ok(v),
                        Flow::Throw(v) => Err(format!("unhandled exception: {v}")),
                        _ => unreachable!(),
                    };
                }
                match method {
                    "length" | "len" => return Ok(Value::Number(dict.len() as f64)),
                    "has" | "containsKey" | "has_key" | "contains" => {
                        let key = values.first().cloned().unwrap_or(Value::Null);
                        let hit = match key {
                            Value::String(k) => dict.contains_key(&k),
                            _ => false,
                        };
                        return Ok(Value::Bool(hit));
                    }
                    "get" => {
                        let key = values.first().cloned().unwrap_or(Value::Null);
                        if let Value::String(k) = key {
                            return Ok(dict.get(&k).cloned().unwrap_or(Value::Null));
                        }
                        return Ok(Value::Null);
                    }
                    "keys" => {
                        return Ok(Value::List(Arc::new(
                            dict.keys().map(|k| Value::String(k.clone())).collect::<Vec<Value>>(),
                        )));
                    }
                    "values" => {
                        return Ok(Value::List(Arc::new(
                            dict.values().cloned().collect::<Vec<Value>>(),
                        )));
                    }
                    _ => {}
                }
                self.dict_method(Arc::unwrap_or_clone(dict), method, values)
            }
            Value::String(value) => self.string_method(value, method, values),
            Value::List(list) => {
                // Borrowing fast paths for hot read-only methods: avoids
                // deep-cloning shared containers on every call.
                match method {
                    "length" | "len" => return Ok(Value::Number(list.len() as f64)),
                    "isEmpty" | "is_empty" => return Ok(Value::Bool(list.is_empty())),
                    "first" => return Ok(list.first().cloned().unwrap_or(Value::Null)),
                    "last" => return Ok(list.last().cloned().unwrap_or(Value::Null)),
                    "contains" => {
                        let needle = values.first().cloned().unwrap_or(Value::Null);
                        return Ok(Value::Bool(list.iter().any(|v| *v == needle)));
                    }
                    _ => {}
                }
                if matches!(method, "push" | "pop") {
                    if let (Some(expr), Some((inst, field))) =
                        (object_expr, object_expr.and_then(|e2| self.list_target_field(e2)))
                    {
                        let result = self.list_method(list.as_ref().clone(), method, values.clone())?;
                        let mut new_list = Arc::unwrap_or_clone(list);
                        match method {
                            "push" => {
                                if let Some(item) = values.first() {
                                    new_list.push(item.clone());
                                }
                            }
                            _ => {
                                new_list.pop();
                            }
                        }
                        inst.lock().unwrap().fields.insert(field, Value::List(Arc::new(new_list)));
                        return Ok(result);
                    }
                }
                self.list_method(Arc::unwrap_or_clone(list), method, values)
            }
            Value::Number(n) => self.number_method(n, method, values),
            Value::Bool(b) => match method {
                "toString" | "to_string" => Ok(Value::String(b.to_string())),
                _ => Err(format!("bool has no method: {method}")),
            },
            other => Err(format!(
                "{} has no method: {method}",
                other.type_name()
            )),
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
        let total = function.params.len();
        let argc = values.len();
        if argc > total || argc < function.required_count() {
            return Err(format!(
                "{class_name}.{method} expects {} arguments, got {}",
                total, argc
            ));
        }
        let mut bound = values;
        for i in bound.len()..total {
            bound.push(function.default_values[i].clone().unwrap_or(Value::Null));
        }
        let body = Arc::clone(&function.body);
        let prev_class = self.current_class.take();
        let prev_method = self.current_method.take();
        self.current_class = Some(class_name.clone());
        self.current_method = Some(method.to_string());
        // Bytecode fast path for compiled methods.
        if let Some(bc) = &function.bytecode {
            let bc = Arc::clone(bc);
            let mut args = Vec::with_capacity(1 + bound.len());
            args.push(Value::Instance(instance));
            args.extend(bound);
            let captured = if function.captured.is_empty() {
                HashMap::new()
            } else {
                function.captured.clone()
            };
            let flow = self.run_bytecode(&bc, args, &captured)?;
            self.current_class = prev_class;
            self.current_method = prev_method;
            return Ok(match flow {
                Flow::Return(value) => Flow::Return(value),
                Flow::Throw(value) => Flow::Throw(value),
                Flow::Normal => Flow::Return(Value::Null),
                Flow::Break | Flow::Continue => {
                    return Err(format!(
                        "loop control escaped method: {class_name}.{method}"
                    ))
                }
            });
        }
        // Use local stack: self + params + captured
        let saved_len = self.locals.len();
        self.frame_starts.push(saved_len);
        self.locals.reserve(1 + function.params.len() + function.effective_captured.len());
        self.locals.push(("self".into(), Value::Instance(instance)));
        for (parameter, value) in function.params.iter().zip(bound) {
            self.locals.push((parameter.0.clone(), value));
        }
        for (k, v) in &function.effective_captured {
            self.locals.push((k.clone(), v.clone()));
        }
        self.capture_frames.push(function.effective_captured.iter().map(|(k, _)| k.clone()).collect());
        let flow = self.exec(&body);
        self.capture_frames.pop();
        self.locals.truncate(saved_len);
        self.frame_starts.pop();
        let flow = flow?;
        self.current_class = prev_class;
        self.current_method = prev_method;
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
        let mut prev_flow = Flow::Normal;
        for stmt in stmts {
            if !matches!(prev_flow, Flow::Normal) {
                return Ok(prev_flow);
            }
            prev_flow = self.exec_one(stmt)?;
        }
        Ok(prev_flow)
    }
    fn exec_module(&mut self, stmts: &[Stmt]) -> Result<Flow, String> {
        // Try to compile and run the module via the bytecode VM. Falls back to
        // tree-walk if any construct is unsupported.
        if let Ok(funcs) = crate::bytecode::compile_program(stmts) {
            self.compiled_functions = funcs;
            if let Some(main) = self.compiled_functions.first().cloned() {
                let flow = self.run_bytecode(&main, Vec::new(), &HashMap::new());
                self.drain_pending_error_classes();
                return flow;
            }
        }
        let result = self.exec(stmts);
        self.drain_pending_error_classes();
        result
    }
    fn drain_pending_error_classes(&mut self) {
        let pending = if let Ok(mut lock) = pending_error_classes().lock() {
            std::mem::take(&mut *lock)
        } else {
            return;
        };
        for (name, parent, message) in pending {
            // Register as a class
            let parent_q = parent.clone().map(|p| format!("errors.{p}"));
            let qualified = format!("errors.{name}");
            let init = Function {
                params: vec![("message".into(), None)],
                default_values: vec![None],
                body: Arc::new(vec![Stmt {
                    kind: StmtKind::SetMember(
                        Expr::Var("self".into()),
                        "message".into(),
                        Expr::Var("message".into()),
                    ),
                    line: 0,
                    col: 0,
                }]),
                captured: HashMap::new(),
                effective_captured: Vec::new(),
                bytecode: None,
            };
            let mut methods = HashMap::new();
            methods.insert("init".into(), init);
            let class = ZenClass {
                parent: parent_q.clone(),
                methods: methods.clone(),
                fields: Vec::new(),
            };
            self.classes.insert(name.clone(), class.clone());
            self.classes.insert(qualified, class);
            // Also add to the errors dict so errors.MyErr works
            if let Some(Value::Dict(errors_dict)) = self.vars.get_mut("errors") {
                let errors_dict = Arc::make_mut(errors_dict);
                errors_dict.insert(name, Value::String(message));
            }
        }
    }
    fn exec_one(&mut self, stmt: &Stmt) -> Result<Flow, String> {
        match &stmt.kind {
                StmtKind::Let(target, e, is_const) => {
                    let v = self.eval(e)?;
                    let mut names: Vec<String> = Vec::new();
                    match target {
                        LetTarget::Var(name) => {
                            if *is_const && self.locked.contains(name) {
                                return Err(format!("cannot redefine constant: {name}\n  \x1b[1;33m= note:\x1b[0m  `{name}` was declared with `const` and cannot be changed\n  \x1b[1;33m= help:\x1b[0m use `let {name} = ...` if you need a mutable variable"));
                            }
                            self.bind_let(name, v);
                            names.push(name.clone());
                        }
                        LetTarget::List(patterns) => match v {
                            Value::List(items) => {
                                for (i, name) in patterns.iter().enumerate() {
                                    if *is_const && self.locked.contains(name) {
                                        return Err(format!("cannot redefine constant: {name}\n  \x1b[1;33m= note:\x1b[0m  `{name}` was declared with `const` and cannot be changed\n  \x1b[1;33m= help:\x1b[0m use `let {name} = ...` if you need a mutable variable"));
                                    }
                                    let item = items.get(i).cloned().unwrap_or(Value::Null);
                                    self.bind_let(name, item);
                                    names.push(name.clone());
                                }
                            }
                            other => return Err(format!("cannot destructure {other} as a list")),
                        },
                        LetTarget::Dict(patterns) => match v {
                            Value::Dict(map) => {
                                for name in patterns {
                                    if *is_const && self.locked.contains(name) {
                                        return Err(format!("cannot redefine constant: {name}\n  \x1b[1;33m= note:\x1b[0m  `{name}` was declared with `const` and cannot be changed\n  \x1b[1;33m= help:\x1b[0m use `let {name} = ...` if you need a mutable variable"));
                                    }
                                    let item = map.get(name).cloned().unwrap_or(Value::Null);
                                    self.bind_let(name, item);
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
                    if self.is_self_field(n) {
                        let v = if matches!(op, Kind::Assign) {
                            self.eval(e)?
                        } else if matches!(op, Kind::NullishAssign) {
                            let current = self.self_field_get(n).unwrap_or(Value::Null);
                            if matches!(current, Value::Null) {
                                self.eval(e)?
                            } else {
                                current
                            }
                        } else {
                            let current = self.self_field_get(n).unwrap_or(Value::Null);
                            let rhs = self.eval(e)?;
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
                            self.binary(current, &binary_op, rhs)?
                        };
                        self.self_field_set(n, v);
                        return Ok(Flow::Normal);
                    }
                    let v = if matches!(op, Kind::Assign) {
                        self.eval(e)?
                    } else {
                        if self.locked.contains(n) {
                            return Err(format!("cannot assign to constant: {n}\n  \x1b[1;33m= note:\x1b[0m  `{n}` was declared with `const` and cannot be reassigned\n  \x1b[1;33m= help:\x1b[0m use `let` instead of `const` if you need a mutable variable"));
                        }
                        let rhs = self.eval(e)?;
                        if matches!(op, Kind::NullishAssign) {
                            let current = self
                                .get_var(n)
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
                            let current = self
                                .get_var(n)
                                .ok_or_else(|| format!("undefined variable: {n}"))?;
                            self.binary(current, &binary_op, rhs)?
                        }
                    };
                    self.set_var(n, v);
                    Ok(Flow::Normal)
                }
                StmtKind::Print(values, sep, end) => {
                    let sep_str = sep.clone().unwrap_or_else(|| " ".to_string());
                    let end_str = end.clone().unwrap_or_else(|| "\n".to_string());
                    let text = values
                        .iter()
                        .map(|e| self.eval(e).map(|v| v.to_string()))
                        .collect::<Result<Vec<_>, _>>()?
                        .join(&sep_str);
                    print!("{text}{end_str}");
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
                    for item in items.iter().cloned() {
                        self.bind_let(n, item);
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
                    let names: Vec<String> = params.iter().map(|(n, _)| n.clone()).collect();
                    let param_set: std::collections::HashSet<String> = names.iter().cloned().collect();
                    let mut free = std::collections::HashSet::new();
                    collect_free_vars_stmts(body, &param_set, &mut free);
                    let captured: HashMap<String, Value> = free.iter()
                        .filter_map(|k| {
                            if let Some(v) = self.vars.get(k) {
                                return Some((k.clone(), v.clone()));
                            }
                            self.locals
                                .iter()
                                .rev()
                                .find(|(n, _)| n == k)
                                .map(|(n, v)| (n.clone(), v.clone()))
                        })
                        .collect();
                    let effective_captured: Vec<(String, Value)> = captured.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                    let mut captured_names: Vec<String> = captured
                        .keys()
                        .filter(|k| !param_set.contains(k.as_str()))
                        .cloned()
                        .collect();
                    captured_names.sort();
                    let has_defaults = params.iter().any(|(_, d)| d.is_some());
                    let default_values = self.eval_default_values(params)?;
                    let bytecode = if has_defaults {
                        None
                    } else {
                        std::env::set_var("ZEN_DBG_FN", format!("stmt:{name}"));
                        match crate::bytecode::compile_function(name, &names, &captured_names, body) {
                            Ok(b) => Some(b),
                            Err(_) => None
                        }
                    };
                    let function = Function {
                        params: params.clone(),
                        default_values,
                        body: Arc::new(body.clone()),
                        captured,
                        effective_captured,
                        bytecode,
                    };
                    self.register_function(name.clone(), function);
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
                Ok(Flow::Normal) => {}
                Ok(Flow::Continue) => return Ok(Flow::Continue),
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
                        // Modules execute exactly once; repeated imports reuse
                        // the cached exports so module-level side effects do not
                        // re-run and exports stay stable.
                        if self.imported_modules.contains_key(&name) {
                            continue;
                        }
                        // Check if the module is already loaded as a dict in
                        // vars under its REAL name (`import string as st`
                        // must find vars["string"], not vars["st"]).
                        if let Some(Value::Dict(existing)) = self.vars.get(module.as_str()).cloned() {
                            let mut map: HashMap<String, Value> = HashMap::new();
                            for (k, v) in Arc::unwrap_or_clone(existing) {
                                map.insert(k, v);
                            }
                            // Also try to load the .zen file and merge exports
                            if let Ok(path) = self.resolve_module(&module) {
                                if let Ok(module_vars) = self.run_module(&path, &name) {
                                    for (k, v) in module_vars {
                                        map.entry(k).or_insert(v);
                                    }
                                }
                            }
                            self.imported_modules.insert(name.clone(), map.clone());
                            let btree: BTreeMap<String, Value> = map.into_iter().collect();
                            self.vars.insert(name, Value::Dict(Arc::new(btree)));
                            continue;
                        }
                        // Check if it's a dotted submodule (e.g. pkg.sub -> parent.sub)
                        if module.contains('.') {
                            let parts: Vec<&str> = module.splitn(2, '.').collect();
                            let parent = parts[0];
                            let child = parts[1];
                            if let Some(parent_mod) = self.imported_modules.get(parent).cloned() {
                                if let Some(child_val) = parent_mod.get(child) {
                                    if let Value::Dict(d) = child_val {
                                        let mut map = HashMap::new();
                                        for (k, v) in (**d).clone() {
                                            map.insert(k, v);
                                        }
                                        self.imported_modules.insert(name, map);
                                        continue;
                                    }
                                }
                            }
                        }
                        // Check stdlib lazy registry
                        if let Some(factory) = self.stdlib_factories.get(module.as_str()).cloned() {
                            let mod_val = factory();
                            if let Value::Dict(d) = &mod_val {
                                let mut map = HashMap::new();
                                for (k, v) in (**d).clone() {
                                    map.insert(k.clone(), v.clone());
                                }
                                self.imported_modules.insert(name.clone(), map);
                            }
                            self.vars.insert(name, mod_val);
                            continue;
                        }
                        // Modules execute exactly once; repeated imports reuse
                        // the cached exports so module-level side effects do not
                        // re-run and exports stay stable.
                        if self.imported_modules.contains_key(&name) {
                            continue;
                        }
                        // Resolve as file
                        let path = self.resolve_module(&module)?;
                        let vars = match self.run_module(&path, &name) {
                            Ok(v) => {
                                v
                            }
                            Err(e) => {
                                return Err(e);
                            }
                        };
                        if module.contains('.') {
                            // Register nested dicts so chained member access
                            // `pkg.sub.mod.func` resolves through vars.
                            let parts: Vec<&str> = module.split('.').collect();
                            let leaf: BTreeMap<String, Value> =
                                vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                            let mut acc = Value::Dict(Arc::new(leaf));
                            // Nest every segment after the root var name,
                            // left-to-right: pkg.sub.mod -> {sub: {mod: exports}}.
                            for p in parts.iter().skip(1).rev() {
                                let mut m = BTreeMap::new();
                                m.insert(p.to_string(), acc);
                                acc = Value::Dict(Arc::new(m));
                            }
                            let root = parts[0];
                            match self.vars.get(root).cloned() {
                                Some(Value::Dict(existing)) => {
                                    let mut merged = Arc::unwrap_or_clone(existing);
                                    if let Value::Dict(newm) = &acc {
                                        for (k, v) in (**newm).clone() {
                                            merged.entry(k.clone()).or_insert(v.clone());
                                        }
                                    }
                                    self.vars.insert(root.to_string(), Value::Dict(Arc::new(merged)));
                                }
                                _ => {
                                    self.vars.insert(root.to_string(), acc);
                                }
                            }
                            if let Some(a) = &alias {
                                let dict: BTreeMap<String, Value> =
                                    vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                                self.vars.insert(a.clone(), Value::Dict(Arc::new(dict)));
                            }
                        } else if alias.is_some() {
                            // Bind the loaded module under the alias so direct
                            // member access (`alias.func()`) works.
                            let dict: BTreeMap<String, Value> =
                                vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                            self.vars.insert(name.clone(), Value::Dict(Arc::new(dict)));
                        }
                        self.imported_modules.insert(name, vars);
                    }
                    Ok(Flow::Normal)
                }
                StmtKind::FromImport(module, items) => {
                    let vars = if let Some(Value::Dict(existing)) = self.vars.get(module.as_str()).cloned()
                    {
                        Arc::unwrap_or_clone(existing).into_iter().collect()
                    } else if let Some(map) = self.imported_modules.get(module.as_str()).cloned() {
                        map.into_iter().collect()
                    } else if let Some(factory) = self.stdlib_factories.get(module.as_str()).cloned() {
                        let mod_val = factory();
                        if let Value::Dict(d) = &mod_val {
                            (**d).clone().into_iter().collect()
                        } else {
                            HashMap::new()
                        }
                    } else {
                        let path = self.resolve_module(&module)?;
                        self.run_module(&path, &module)?
                    };
                    for (item, alias) in items {
                        let value = if let Some(v) = vars.get(item).cloned() {
                            v
                        } else {
                            // Item not in module dict — try as a submodule file:
                            // from pkg import sub  ->  pkg/sub.z or pkg/sub/main.z
                            let sub_name = if module.contains('.') {
                                format!("{}.{}", module, item)
                            } else {
                                format!("{}.{}", module, item)
                            };
                            match self.resolve_module(&sub_name) {
                                Ok(path) => {
                                    let sub_vars = self.run_module(&path, &sub_name)?;
                                    // Cache the submodule dict in imported_modules
                                    let mut map = HashMap::new();
                                    for (k, v) in &sub_vars {
                                        map.insert(k.clone(), v.clone());
                                    }
                                    self.imported_modules.insert(sub_name.clone(), map);
                                    Value::Dict(Arc::new(sub_vars.into_iter().collect::<BTreeMap<String, Value>>()))
                                }
                                Err(_) => {
                                    return Err(format!("item '{}' not found in module '{}'\n  \x1b[1;33m= help:\x1b[0m check the module's available exports with `from {} import *`", item, module, module));
                                }
                            }
                        };
                        let name = alias.clone().unwrap_or(item.clone());
                        self.bind_let(&name, value);
                    }
                    Ok(Flow::Normal)
                }
                StmtKind::StarImport(module) => {
                    let vars = if let Some(Value::Dict(existing)) = self.vars.get(module.as_str()).cloned()
                    {
                        Arc::unwrap_or_clone(existing).into_iter().collect()
                    } else if let Some(map) = self.imported_modules.get(module.as_str()).cloned() {
                        map.into_iter().collect()
                    } else if let Some(factory) = self.stdlib_factories.get(module.as_str()).cloned() {
                        let mod_val = factory();
                        if let Value::Dict(d) = &mod_val {
                            (**d).clone().into_iter().collect()
                        } else {
                            HashMap::new()
                        }
                    } else {
                        let path = self.resolve_module(&module)?;
                        self.run_module(&path, &module)?
                    };
                    for (name, value) in vars {
                        if !name.starts_with('_') {
                            self.bind_let(&name, value);
                        }
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
                            return Err(format!("unknown parent class: {parent}\n  \x1b[1;33m= help:\x1b[0m make sure the parent class is defined before the child class"));
                        }
                    }
                    let mut methods = HashMap::new();
                    let mut fields = Vec::new();
                    for statement in body {
                        match &statement.kind {
                            StmtKind::Function(method, params, body) => {
                                let names: Vec<String> = params.iter().map(|(n, _)| n.clone()).collect();
                                let has_defaults = params.iter().any(|(_, d)| d.is_some());
                                let default_values = self.eval_default_values(params)?;
                                // Methods run via tree-walk so implicit-self
                                // field access (`var _x` -> `self._x`) resolves
                                // correctly. Bytecode for methods is disabled.
                                let bytecode = None;
                                let _ = names;
                                methods.insert(
                                    method.clone(),
                                    Function {
                                        params: params.clone(),
                                        default_values,
                                        body: Arc::new(body.clone()),
                                        captured: HashMap::new(),
                                        effective_captured: Vec::new(),
                                        bytecode,
                                    },
                                );
                            }
                            StmtKind::Field(fname, init) => {
                                fields.push((fname.clone(), init.clone()));
                            }
                            _ => {
                                return Err(format!(
                                    "class '{name}' may currently contain only methods"
                                ));
                            }
                        }
                    }
                    self.classes.insert(
                        name.clone(),
                        ZenClass {
                            parent: parent.clone(),
                            methods,
                            fields,
                        },
                    );
                    self.vars.insert(name.clone(), Value::Function(name.clone()));
                    Ok(Flow::Normal)
                }
                StmtKind::SetMember(object, member, value) => {
                    match self.eval(object)? {
                        Value::Instance(instance) => {
                            let new_val = self.eval(value)?;
                            instance
                                .lock()
                                .unwrap()
                                .fields
                                .insert(member.clone(), new_val);
                        }
                        Value::Dict(dict) => {
                            let mut dict = dict;
                            Arc::make_mut(&mut dict).insert(member.clone(), self.eval(value)?);
                            // Only persist if assigned to a named variable
                            if let Expr::Var(name) = object {
                                self.vars.insert(name.clone(), Value::Dict(dict));
                            }
                        }
                        _ => return Err("member assignment requires an object".into()),
                    }
                    Ok(Flow::Normal)
                }
                StmtKind::SetIndex(object, index, value) => {
                    let obj = self.eval(object)?;
                    let idx = self.eval(index)?;
                    let new_val = self.eval(value)?;
                    match obj {
                        Value::Dict(mut dict) => {
                            let key = match idx {
                                Value::String(s) => s,
                                _ => return Err("dictionary index must be a string".into()),
                            };
                            Arc::make_mut(&mut dict).insert(key, new_val);
                            if let Expr::Var(name) = object {
                                self.vars.insert(name.clone(), Value::Dict(dict));
                            }
                        }
                        Value::List(mut list) => {
                            let Value::Number(n) = idx else {
                                return Err("list index must be a number".into());
                            };
                            let mut i = n as i64;
                            if i < 0 {
                                i += list.len() as i64;
                            }
                            if i < 0 || i as usize >= list.len() {
                                return Err("list index out of bounds".into());
                            }
                            Arc::make_mut(&mut list)[i as usize] = new_val;
                            if let Expr::Var(name) = object {
                                self.vars.insert(name.clone(), Value::List(list));
                            }
                        }
                        _ => return Err("index assignment requires a dict or list".into()),
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
                StmtKind::Field(..) => unreachable!("field declarations are handled by the class handler"),
            }
        }
    fn locate(&self, line: usize, col: usize, message: String) -> String {
        if line == 0 {
            return message;
        }
        // Don't double-wrap already formatted errors — just add a stack frame
        if message.contains("\x1b[1;34m-->\x1b[0m") {
            // Already has our format. Prepend a new frame.
            let inner = &message;
            // Extract just the innermost error type + message for the header
            let inner_summary = inner.lines()
                .find(|l| l.starts_with("  \x1b[1;31m= ") || l.starts_with("\x1b[1;31merror\x1b[0m"))
                .map(|l| {
                    let cleaned = l.trim_start()
                        .trim_start_matches("\x1b[1;31m= ")
                        .trim_start_matches("\x1b[1;31merror\x1b[0m\x1b[1m[")
                        .trim_end_matches("\x1b[0m")
                        .trim_end_matches("]");
                    cleaned.split('\n').next().unwrap_or(cleaned).to_string()
                })
                .unwrap_or_else(|| "error".into());
            let mut out = format!(
                "\x1b[1;31merror\x1b[0m\x1b[1m[{}]\x1b[0m\n",
                inner_summary
            );
            out.push_str(&format!(
                " \x1b[1;34m-->\x1b[0m {}:{}:{}\n",
                self.file, line, col
            ));
            out.push_str(&format!("  \x1b[1;34m|\x1b[0m\n"));
            if line > 0 && !self.lines.is_empty() {
                out.push_str(&render_context(&self.lines, line, col, 2));
            }
            out.push_str(&format!("  \x1b[1;34m|\x1b[0m\n"));
            // Append inner traceback indented
            for ln in inner.lines() {
                out.push_str(&format!("  {}\n", ln));
            }
            return out;
        }
        let mut out = format!(
            "\x1b[1;31merror\x1b[0m\x1b[1m[{}]\x1b[0m\n",
            message.split('\n').next().unwrap_or(&message)
        );
        out.push_str(&format!(
            " \x1b[1;34m-->\x1b[0m {}:{}:{}\n",
            self.file, line, col
        ));
        out.push_str(&format!("  \x1b[1;34m|\x1b[0m\n"));
        if line > 0 && !self.lines.is_empty() {
            out.push_str(&render_context(&self.lines, line, col, 2));
        }
        out.push_str(&format!("  \x1b[1;34m|\x1b[0m\n"));
        // Split message into main + notes
        let parts: Vec<&str> = message.splitn(2, "\nnote: ").collect();
        let main_msg = parts[0];
        out.push_str(&format!(
            "  \x1b[1;31m= {}\x1b[0m {}\n",
            if main_msg.contains(':') {
                ""
            } else {
                "error: "
            },
            main_msg
        ));
        if let Some(note) = parts.get(1) {
            out.push_str(&format!(
                "  \x1b[1;33m= note:\x1b[0m {}\n",
                note
            ));
        }
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
        Value::Dict(Arc::new(map))
    }
    fn to_error(&self, value: Value, line: usize, col: usize) -> Value {
        match value {
            Value::Dict(mut map) => {
                let mut map = (*map).clone();
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
                Value::Dict(Arc::new(map))
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
                Value::Dict(Arc::new(map))
            }
            other => {
                let mut map = BTreeMap::new();
                map.insert("type".into(), Value::String("Error".into()));
                map.insert("message".into(), Value::String(other.to_string()));
                map.insert("file".into(), Value::String(self.file.clone()));
                map.insert("line".into(), Value::Number(line as f64));
                map.insert("col".into(), Value::Number(col as f64));
                Value::Dict(Arc::new(map))
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
                for item in items.iter().cloned() {
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
                    for (k, v) in (**h).clone() {
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
    // Bodies are evicted on read (.json()/.text()) to prevent memory leaks.
    // Cap at 256 entries to bound growth if responses are never consumed.
    let id = next_response_id();
    {
        let mut cache = response_bodies()
            .lock()
            .map_err(|e| format!("response cache poisoned: {e}"))?;
        cache.insert(id, body_text);
        if cache.len() > 256 {
            let cutoff = id.saturating_sub(256);
            cache.retain(|&k, _| k >= cutoff);
        }
    }
    let mut result = BTreeMap::new();
    result.insert("status".into(), Value::Number(status));
    result.insert("ok".into(), Value::Bool(status >= 200.0 && status < 400.0));
    result.insert("headers".into(), Value::Dict(Arc::new(header_dict)));
    result.insert("__id".into(), Value::Number(id as f64));
    result.insert("json".into(), Value::NativeFunction("__http_response_json".into()));
    result.insert("text".into(), Value::NativeFunction("__http_response_text".into()));
    Ok(Value::Dict(Arc::new(result)))
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
        .remove(&id)
        .ok_or_else(|| "response body no longer available (already consumed)".into())
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
                    rows.push(Value::List(Arc::new(std::mem::take(&mut row))));
                } else {
                    row.clear();
                }
            }
            other => field.push(other),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(Value::String(field));
        rows.push(Value::List(Arc::new(row)));
    }
    Value::List(Arc::new(rows))
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
        Some(Value::Dict(d)) => Ok((**d).clone()),
        _ => Err(format!("argument {} must be a dict", i + 1)),
    }
}

fn arg_list(args: &[Value], i: usize) -> Result<Vec<Value>, String> {
    match args.get(i) {
        Some(Value::List(l)) => Ok((**l).clone()),
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
            // Handle IMAP literal {N}\r\n — read N bytes of literal data
            if let Some(brace_pos) = line.find('{') {
                if let Some(close_pos) = line[brace_pos..].find('}') {
                    let num_str = &line[brace_pos + 1..brace_pos + close_pos];
                    if let Ok(literal_len) = num_str.parse::<usize>() {
                        let mut literal_buf = vec![0u8; literal_len];
                        let mut total_read = 0;
                        while total_read < literal_len {
                            match stream.read(&mut literal_buf[total_read..]) {
                                Ok(0) => break,
                                Ok(n) => total_read += n,
                                Err(e) => return Err(format!("IMAP literal read error: {e}")),
                            }
                        }
                        response.push_str(&String::from_utf8_lossy(&literal_buf));
                        // Read the \r\n after the literal
                        let _ = read_line(stream);
                    }
                }
            }
        } else if line.starts_with(tag) {
            if !line.contains("OK") && !line.contains("NO") && !line.contains("BAD") {
                return Err(format!("IMAP {cmd}: {line}"));
            }
            return Ok(response);
        } else {
            response.push_str(&line);
            response.push('\n');
        }
    }
}

fn imap_next_tag(session: &Value) -> Result<u32, String> {
    // Use a global registry so tags persist across cloned session dicts
    static IMAP_TAGS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<u64, u32>>> =
        std::sync::OnceLock::new();
    let tags = IMAP_TAGS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let id = match session {
        Value::Dict(d) => match d.get("__id") {
            Some(Value::Number(n)) => *n as u64,
            _ => 0,
        },
        _ => 0,
    };
    let mut map = tags.lock().unwrap();
    let tag = map.entry(id).or_insert(2);
    let current = *tag;
    *tag += 1;
    Ok(current)
}

fn strip_telnet_iac(buf: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let mut out = Vec::with_capacity(buf.len());
    let mut replies = Vec::new();
    let mut i = 0;
    while i < buf.len() {
        if buf[i] == 0xff {
            if i + 1 < buf.len() && buf[i + 1] == 0xff {
                out.push(0xff);
                i += 2;
                continue;
            }
            if i + 1 < buf.len() && buf[i + 1] == 0xfb {
                replies.extend_from_slice(&[0xff, 0xfc]); // DO -> send WONT
                i += 2;
                continue;
            }
            if i + 1 < buf.len() && buf[i + 1] == 0xfd {
                replies.extend_from_slice(&[0xff, 0xfe]); // WILL -> send DONT
                i += 2;
                continue;
            }
            if i + 2 < buf.len() {
                replies.extend_from_slice(&[0xff, buf[i + 1], buf[i + 2]]);
                i += 3;
                continue;
            }
            i = buf.len();
        } else {
            out.push(buf[i]);
            i += 1;
        }
    }
    (out, replies)
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

fn crunch_generate_len(len: usize, chars: &[char], prefix: &mut String, result: &mut Vec<String>) {
    if len == 0 {
        result.push(prefix.clone());
        return;
    }
    for &c in chars {
        prefix.push(c);
        crunch_generate_len(len - 1, chars, prefix, result);
        prefix.pop();
    }
}

#[derive(Clone)]
enum CrunchElem { Lit(String), Slot(Vec<char>), Range(String, usize, usize) }

fn crunch_pattern_impl(template: &str) -> Result<Value, String> {
    let resolve = |token: &str| -> Vec<char> {
        match token {
            "a" | "lower" => "abcdefghijklmnopqrstuvwxyz".chars().collect(),
            "A" | "upper" => "ABCDEFGHIJKLMNOPQRSTUVWXYZ".chars().collect(),
            "d" | "n" | "digits" | "numeric" => "0123456789".chars().collect(),
            "s" | "symbols" => "!@#$%^&*()-_=+[]{}|;:',.<>?/".chars().collect(),
            "h" | "hex" => "0123456789abcdef".chars().collect(),
            "x" | "alnum" => "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars().collect(),
            _ => token.chars().collect(),
        }
    };

    // CrunchElement: literal string, fixed slot, or range slot
    let mut elems: Vec<CrunchElem> = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = template.chars().collect();
    let clen = chars.len();
    let mut i = 0;

    while i < clen {
        if chars[i] == ':' && i + 1 < clen {
            if !current.is_empty() {
                elems.push(CrunchElem::Lit(current.clone()));
                current.clear();
            }
            i += 1;
            let mut token = String::new();
            while i < clen && chars[i] != '{' && chars[i] != ':' && chars[i] != '?' {
                token.push(chars[i]);
                i += 1;
            }
            if i < clen && chars[i] == '{' {
                i += 1;
                let mut num_str = String::new();
                while i < clen && chars[i] != ',' && chars[i] != '}' {
                    num_str.push(chars[i]);
                    i += 1;
                }
                let n: usize = num_str.parse().unwrap_or(1);
                if i < clen && chars[i] == ',' {
                    i += 1;
                    let mut m_str = String::new();
                    while i < clen && chars[i] != '}' {
                        m_str.push(chars[i]);
                        i += 1;
                    }
                    let m: usize = m_str.parse().unwrap_or(n);
                    i += 1;
                    elems.push(CrunchElem::Range(token, n, m));
                } else {
                    i += 1;
                    let cs = resolve(&token);
                    for _ in 0..n {
                        elems.push(CrunchElem::Slot(cs.clone()));
                    }
                }
            } else {
                elems.push(CrunchElem::Slot(resolve(&token)));
            }
        } else if chars[i] == '?' {
            if !current.is_empty() {
                elems.push(CrunchElem::Lit(current.clone()));
                current.clear();
            }
            elems.push(CrunchElem::Slot("abcdefghijklmnopqrstuvwxyz".chars().collect()));
            i += 1;
        } else {
            current.push(chars[i]);
            i += 1;
        }
    }
    if !current.is_empty() {
        elems.push(CrunchElem::Lit(current));
    }

    if elems.is_empty() {
        return Ok(Value::List(Arc::new(vec![Value::String(template.to_string())])));
    }

    // Find first Range, expand it into sub-templates, recurse
    if let Some(idx) = elems.iter().position(|e| matches!(e, CrunchElem::Range(_, _, _))) {
        if let CrunchElem::Range(token, n, m) = elems[idx].clone() {
            let mut all = Vec::new();
            for rep in n..=m {
                // Build sub-template: expand this range into `rep` fixed slots
                let mut sub_elems = elems[..idx].to_vec();
                let cs = resolve(&token);
                for _ in 0..rep {
                    sub_elems.push(CrunchElem::Slot(cs.clone()));
                }
                sub_elems.extend_from_slice(&elems[idx + 1..]);
                // Recurse on the sub-template (which may have more ranges)
                let sub = crunch_expand_elems(&sub_elems)?;
                all.extend(sub);
            }
            return Ok(Value::List(Arc::new(all)));
        }
    }

    // No ranges — single cartesian product
    Ok(Value::List(Arc::new(crunch_expand_elems(&elems)?)))
}

fn crunch_expand_elems(elems: &[CrunchElem]) -> Result<Vec<Value>, String> {
    // Flat cartesian product of all slots with literals interleaved
    let mut result: Vec<String> = vec![String::new()];
    for elem in elems {
        match elem {
            CrunchElem::Lit(s) => {
                for r in &mut result {
                    r.push_str(s);
                }
            }
            CrunchElem::Slot(cs) => {
                let mut next = Vec::new();
                for r in &result {
                    for &c in cs {
                        let mut s = r.clone();
                        s.push(c);
                        next.push(s);
                    }
                }
                result = next;
            }
            CrunchElem::Range(_, _, _) => {
                return Err("internal: unresolved range in crunch_expand_elems".into());
            }
        }
    }
    Ok(result.into_iter().map(Value::String).collect())
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
    let mut resp = buf[..n].to_vec();
    // Check TC (truncation) bit — byte 2, bit 1
    if resp.len() >= 3 && (resp[2] & 0x02) != 0 {
        // Retry over TCP for truncated response
        let tcp_sock = TcpStream::connect_timeout(
            &format!("{server}:53").parse().map_err(|e: std::net::AddrParseError| format!("{e}"))?,
            Duration::from_secs(5),
        ).map_err(|e| format!("dns tcp connect: {e}"))?;
        // TCP DNS: prefix with 2-byte length
        let len = query.len() as u16;
        let mut tcp_query = Vec::with_capacity(2 + query.len());
        tcp_query.extend_from_slice(&len.to_be_bytes());
        tcp_query.extend_from_slice(&query);
        let mut stream = tcp_sock;
        stream.write_all(&tcp_query).map_err(|e| format!("dns tcp write: {e}"))?;
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        let mut len_buf = [0u8; 2];
        stream.read_exact(&mut len_buf).map_err(|e| format!("dns tcp read len: {e}"))?;
        let tcp_len = u16::from_be_bytes(len_buf) as usize;
        resp.resize(tcp_len, 0);
        stream.read_exact(&mut resp).map_err(|e| format!("dns tcp read: {e}"))?;
    }
    if resp.len() < 12 {
        return Ok(Vec::new());
    }
    // Validate response ID matches query ID to prevent spoofing
    let resp_id = u16::from_be_bytes([resp[0], resp[1]]);
    if resp_id != id {
        return Err(format!("dns: response ID {resp_id:#06x} does not match query ID {id:#06x}"));
    }
    let ancount = u16::from_be_bytes([resp[6], resp[7]]) as usize;
    let mut pos = 12usize;
    let (_, npos) = dns_read_name(&resp, pos);
    pos = npos + 4;
    let mut results = Vec::new();
    for _ in 0..ancount {
        if pos >= resp.len() {
            break;
        }
        let (rname, npos) = dns_read_name(&resp, pos);
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
                let (hostname, _) = dns_read_name(&resp, pos + 2);
                format!("{pref} {hostname}")
            }
            16 => String::from_utf8_lossy(rdata).into_owned(),
            2 | 5 => {
                let (hostname, _) = dns_read_name(&resp, pos);
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
        results.push(Value::Dict(Arc::new(rec)));
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
                Value::Dict(Arc::new(tcp_layer))
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
                Value::Dict(Arc::new(udp_layer))
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
                Value::Dict(Arc::new(icmp_layer))
            }
        }
        _ => {
            ip_layer.insert("proto".into(), Value::Number(proto as f64));
            if !payload.is_empty() {
                let mut raw = BTreeMap::new();
                raw.insert("type".into(), Value::String("Raw".into()));
                raw.insert("data".into(), Value::String(hexlify(payload)));
                Value::Dict(Arc::new(raw))
            } else {
                Value::Null
            }
        }
    };
    if !matches!(inner, Value::Null) {
        ip_layer.insert("payload".into(), inner);
    }
    Value::Dict(Arc::new(ip_layer))
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
    Ok(Value::List(Arc::new(packets)))
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
    Ok(Value::List(Arc::new(out)))
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
        out.push(Value::List(Arc::new(current.clone())));
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
        out.push(Value::List(Arc::new(current.clone())));
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
            let addr = match (args.first(), args.get(1)) {
                (Some(Value::String(host)), Some(Value::Number(port))) => {
                    format!("{host}:{port}")
                }
                (Some(Value::String(s)), _) => s.clone(),
                _ => return Err("socket.open expects (host, port) or (\"host:port\")\n  \x1b[1;33m= help:\x1b[0m usage: socket.open(\"192.168.1.10\", 80) or socket.open(\"192.168.1.10:80\")".into()),
            };
            let stream = TcpStream::connect(&addr)
                .map_err(|e| format!("failed to connect to {addr}: {e}"))?;
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
                [Value::Socket(s)] => (s, 4096),
                _ => return Err("socket.recv expects (Socket, size?)".into()),
            };
            let mut buffer = vec![0u8; size];
            let n = socket.lock().unwrap().read(&mut buffer)
                .map_err(|e| format!("failed to recv: {e}"))?;
            let data = buffer[..n].to_vec();
            Ok(Value::List(Arc::new(data.iter().map(|b| Value::Number(*b as f64)).collect::<Vec<Value>>())))
        },
        "socket_recv_text" => |args| {
            let (socket, size) = match args.as_slice() {
                [Value::Socket(s), Value::Number(n)] => (s, *n as usize),
                [Value::Socket(s)] => (s, 4096),
                _ => return Err("socket.recv_text expects (Socket, size?)".into()),
            };
            let mut buffer = vec![0u8; size];
            let n = socket.lock().unwrap().read(&mut buffer)
                .map_err(|e| format!("failed to recv: {e}"))?;
            Ok(Value::String(String::from_utf8_lossy(&buffer[..n]).into_owned()))
        },
        "socket_open_udp" => |args| {
            let addr = match (args.first(), args.get(1)) {
                (Some(Value::String(host)), Some(Value::Number(port))) => {
                    format!("{host}:{port}")
                }
                (Some(Value::String(s)), _) => s.clone(),
                _ => return Err("socket.open_udp expects (host, port) or (\"host:port\")".into()),
            };
            let socket = std::net::UdpSocket::bind("0.0.0.0:0")
                .map_err(|e| format!("failed to bind UDP socket: {e}"))?;
            socket.connect(&addr)
                .map_err(|e| format!("failed to connect UDP to {addr}: {e}"))?;
            socket.set_nonblocking(false).ok();
            Ok(Value::UdpSocket(Arc::new(Mutex::new(socket))))
        },
        "socket_send_to" => |args| {
            match args.as_slice() {
                [Value::UdpSocket(s), Value::String(data)] => {
                    s.lock().unwrap().send(data.as_bytes())
                        .map_err(|e| format!("failed to send: {e}"))?;
                    Ok(Value::Bool(true))
                }
                [Value::UdpSocket(s), Value::String(data), Value::String(addr)] => {
                    let sock = s.lock().unwrap();
                    sock.send_to(data.as_bytes(), addr.as_str())
                        .map_err(|e| format!("failed to send_to: {e}"))?;
                    Ok(Value::Bool(true))
                }
                [Value::Dict(d), Value::String(data), Value::String(addr)] => {
                    let socket = std::net::UdpSocket::bind("0.0.0.0:0")
                        .map_err(|e| format!("failed to bind UDP: {e}"))?;
                    socket.send_to(data.as_bytes(), addr.as_str())
                        .map_err(|e| format!("failed to send_to: {e}"))?;
                    Ok(Value::Bool(true))
                }
                _ => return Err("socket_send_to expects (UdpSocket, data, addr?)".into()),
            }
        },
        "socket_recv_from" => |args| {
            let (socket, size) = match args.as_slice() {
                [Value::UdpSocket(s), Value::Number(n)] => (s.clone(), *n as usize),
                [Value::UdpSocket(s)] => (s.clone(), 4096),
                _ => return Err("socket_recv_from expects (UdpSocket, size?)".into()),
            };
            let mut buf = vec![0u8; size];
            let (n, addr) = socket.lock().unwrap().recv_from(&mut buf)
                .map_err(|e| format!("failed to recv_from: {e}"))?;
            let data = buf[..n].to_vec();
            let bytes: Vec<Value> = data.iter().map(|b| Value::Number(*b as f64)).collect();
            Ok(Value::Dict(Arc::new(BTreeMap::from([
                ("data".into(), Value::List(Arc::new(bytes))),
                ("addr".into(), Value::String(addr.to_string())),
                ("text".into(), Value::String(String::from_utf8_lossy(&data).into_owned())),
            ]))))
        },
        "socket_recv_all" => |args| {
            let socket = match &args[0] {
                Value::Socket(s) => s,
                _ => return Err("socket_recv_all expects a Socket".into()),
            };
            let timeout = match args.get(1) {
                Some(Value::Number(n)) => std::time::Duration::from_secs_f64(*n),
                _ => std::time::Duration::from_secs(5),
            };
            let mut s = socket.lock().unwrap();
            s.set_read_timeout(Some(timeout)).ok();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            loop {
                match s.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut => break,
                    Err(e) => return Err(format!("socket_recv_all error: {e}")),
                }
            }
            Ok(Value::String(String::from_utf8_lossy(&buf).into_owned()))
        },
        "socket_set_timeout" => |args| {
            let socket = match &args[0] {
                Value::Socket(s) => s,
                _ => return Err("socket_set_timeout expects a Socket".into()),
            };
            let secs = match args.get(1) {
                Some(Value::Number(n)) => *n,
                _ => return Err("socket_set_timeout expects (Socket, seconds)".into()),
            };
            let dur = std::time::Duration::from_secs_f64(secs);
            let mut s = socket.lock().unwrap();
            s.set_read_timeout(Some(dur)).map_err(|e| format!("set timeout error: {e}"))?;
            s.set_write_timeout(Some(dur)).map_err(|e| format!("set timeout error: {e}"))?;
            Ok(Value::Bool(true))
        },
        "socket_scan" => |args| {
            let host = arg_string(&args, 0)?;
            let (start_port, end_port) = match args.as_slice() {
                [_, Value::Number(s), Value::Number(e)] => (*s as u16, *e as u16),
                [_, Value::Number(p)] => (*p as u16, *p as u16),
                _ => (1, 1024),
            };
            let timeout_ms = match args.get(3) {
                Some(Value::Number(n)) => *n as u64,
                _ => 200,
            };
            // Resolve hostname to IP address
            use std::net::ToSocketAddrs;
            let ip_addr = format!("{host}:0")
                .to_socket_addrs()
                .map_err(|e| format!("DNS resolution failed for '{host}': {e}"))?
                .next()
                .ok_or_else(|| format!("no addresses found for '{host}'"))?
                .ip();
            let timeout = std::time::Duration::from_millis(timeout_ms);
            let mut open_ports = Vec::new();
            for port in start_port..=end_port {
                let addr = std::net::SocketAddr::new(ip_addr, port);
                match TcpStream::connect_timeout(&addr, timeout) {
                    Ok(stream) => {
                        drop(stream);
                        open_ports.push(Value::Number(port as f64));
                    }
                    Err(_) => {}
                }
            }
            Ok(Value::List(Arc::new(open_ports)))
        },
        "time_now" => |_| {
            let start = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64();
            Ok(Value::Number(start))
        },
        "cli_args" => |_| {
            let args: Vec<Value> = env::args().map(|s| Value::String(s)).collect();
            Ok(Value::List(Arc::new(args)))
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
            Ok(Value::List(Arc::new(items)))
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
            Ok(Value::List(Arc::new(items)))
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
            Arc::make_mut(&mut items).shuffle(&mut rand::rng());
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
            Ok(Value::List(Arc::new(result)))
        },
        "random_sample" => |args| {
            use rand::seq::SliceRandom;
            let (items, k) = match args.as_slice() {
                [Value::List(items), Value::Number(k)] => (items, *k as usize),
                _ => return Err("random.sample expects (sequence, k)".into()),
            };
            let mut pool = (**items).clone();
            pool.shuffle(&mut rand::rng());
            pool.truncate(k);
            Ok(Value::List(Arc::new(pool)))
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
                        for item in items.iter().cloned() {
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
                        for item in items.iter().cloned() {
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
            Ok(Value::List(Arc::new(vec![Value::Number(n.fract()), Value::Number(n.trunc())])))
        },
        "math_frexp" => |args| {
            let n = match args.first() {
                Some(Value::Number(n)) => n,
                _ => return Err("math.frexp expects number".into()),
            };
            if *n == 0.0 {
                return Ok(Value::List(Arc::new(vec![Value::Number(0.0), Value::Number(0.0)])));
            }
            let exponent = n.abs().log2().floor() as i32 + 1;
            let mantissa = n / 2f64.powi(exponent);
            Ok(Value::List(Arc::new(vec![Value::Number(mantissa), Value::Number(exponent as f64)])))
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
        "os_arch" => |_| Ok(Value::String(std::env::consts::ARCH.into())),
        "errors_define" => |args| {
            let name = match args.first() {
                Some(Value::String(s)) => s.clone(),
                _ => return Err("errors.define expects (name [, parent, message])".into()),
            };
            let parent = args.get(1).and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            });
            let message = args.get(2).and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            }).unwrap_or_default();
            // Store the definition; the VM will drain and register after the statement
            if let Ok(mut pending) = pending_error_classes().lock() {
                pending.push((name, parent, message));
            }
            Ok(Value::Null)
        },
        "os_execute" => |args| {
            let cmd = match args.first() {
                Some(Value::String(c)) => c,
                _ => return Err("os.execute expects a command string".into()),
            };
            let output = process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .output()
                .map_err(|e| format!("os.execute failed: {e}"))?;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let mut result = BTreeMap::new();
            result.insert("ok".into(), Value::Bool(output.status.success()));
            result.insert("code".into(), Value::Number(output.status.code().unwrap_or(-1) as f64));
            result.insert("stdout".into(), Value::String(stdout));
            result.insert("stderr".into(), Value::String(stderr));
            Ok(Value::Dict(Arc::new(result)))
        },
        "os_run" => |args| {
            let cmd = match args.first() {
                Some(Value::String(c)) => c.clone(),
                _ => return Err("os.run expects a command string".into()),
            };
            let output = process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output()
                .map_err(|e| format!("os.run failed: {e}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let code = output.status.code().unwrap_or(-1);
                return Err(format!("command failed (exit {code}): {stderr}"));
            }
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(Value::String(stdout))
        },
        "os_popen" => |args| {
            let cmd = match args.first() {
                Some(Value::String(c)) => c.clone(),
                _ => return Err("os.popen expects a command string".into()),
            };
            let output = process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .stdout(process::Stdio::piped())
                .stderr(process::Stdio::piped())
                .stdin(process::Stdio::null())
                .output()
                .map_err(|e| format!("os.popen failed: {e}"))?;
            let mut result = BTreeMap::new();
            result.insert("ok".into(), Value::Bool(output.status.success()));
            result.insert("code".into(), Value::Number(output.status.code().unwrap_or(-1) as f64));
            result.insert("stdout".into(), Value::String(String::from_utf8_lossy(&output.stdout).to_string()));
            result.insert("stderr".into(), Value::String(String::from_utf8_lossy(&output.stderr).to_string()));
            Ok(Value::Dict(Arc::new(result)))
        },
        "os_args" => |_| {
            let args: Vec<Value> = std::env::args().skip(1)
                .map(|a| Value::String(a))
                .collect();
            Ok(Value::List(Arc::new(args)))
        },
        "os_pids" => |_| {
            let mut pids = Vec::new();
            if let Ok(entries) = fs::read_dir("/proc") {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.chars().all(|c| c.is_ascii_digit()) {
                            if let Ok(pid) = name.parse::<f64>() {
                                pids.push(Value::Number(pid));
                            }
                        }
                    }
                }
            }
            Ok(Value::List(Arc::new(pids)))
        },
        "os_kill" => |args| {
            let pid = arg_number(&args, 0)? as i32;
            let signal = arg_number(&args, 1).unwrap_or(15.0) as i32;
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(pid, signal);
                }
                Ok(Value::Null)
            }
            #[cfg(not(unix))]
            {
                Err("os.kill is only supported on Unix".into())
            }
        },
        "os_home" => |_| {
            Ok(Value::String(
                std::env::var("HOME")
                    .or_else(|_| std::env::var("USERPROFILE"))
                    .unwrap_or_else(|_| "/".into()),
            ))
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
            enum Ts {
                Now,
                Unix(f64),
                Str(String),
            }
            let (fmt, ts) = match args.as_slice() {
                [Value::String(f)] => (f.clone(), Ts::Now),
                [Value::Number(unix), Value::String(f)] => (f.clone(), Ts::Unix(*unix)),
                [Value::String(ts), Value::String(f)] => (f.clone(), Ts::Str(ts.clone())),
                _ => return Err("time.format expects a format string".into()),
            };
            match ts {
                Ts::Unix(unix) => {
                    let datetime: chrono::DateTime<chrono::Local> =
                        chrono::DateTime::from_timestamp(unix as i64, 0)
                            .unwrap_or_default()
                            .into();
                    Ok(Value::String(datetime.format(&fmt).to_string()))
                }
                Ts::Str(ts) => match chrono::DateTime::parse_from_rfc3339(&ts) {
                    Ok(dt) => Ok(Value::String(dt.format(&fmt).to_string())),
                    Err(_) => match chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%dT%H:%M:%S") {
                        Ok(parsed) => Ok(Value::String(parsed.format(&fmt).to_string())),
                        Err(_) => Err("time.format could not parse timestamp".into()),
                    },
                },
                Ts::Now => {
                    let now = SystemTime::now();
                    let datetime: chrono::DateTime<chrono::Local> = now.into();
                    Ok(Value::String(datetime.format(&fmt).to_string()))
                }
            }
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
            Ok(Value::List(Arc::new(parts)))
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
            Ok(Value::List(Arc::new(results)))
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
            let encoded = csv_encode_impl(rows.as_ref(), headers.map(|h| h.as_ref()));
            fs::write(path, encoded).map_err(|e| format!("csv.write {path}: {e}"))?;
            Ok(Value::Bool(true))
        },
        "csv_encode" => |args| {
            let (rows, headers) = match args.as_slice() {
                [Value::List(rows)] => (rows, None),
                [Value::List(rows), Value::List(headers)] => (rows, Some(headers)),
                _ => return Err("csv.encode expects (rows, headers?)".into()),
            };
            Ok(Value::String(csv_encode_impl(rows.as_ref(), headers.map(|h| h.as_ref()))))
        },
        "decimal_decimal" => |args| {
            let v = match args.first() {
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::String(s)) => s.clone(),
                _ => return Err("decimal.Decimal expects a number".into()),
            };
            Ok(Value::Dict(Arc::new(BTreeMap::from([
                ("value".into(), Value::String(v.clone())),
                ("__repr__".into(), Value::String(format!("Decimal({v})"))),
            ]))))
        },
        "decimal_getcontext" => |_| {
            Ok(Value::Dict(Arc::new(BTreeMap::from([
                ("prec".into(), Value::Number(28.0)),
                ("rounding".into(), Value::String("ROUND_HALF_EVEN".into())),
            ]))))
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
            Ok(Value::Dict(Arc::new(BTreeMap::from([
                ("name".into(), Value::String(format!("Thread-{name}"))),
                ("daemon".into(), Value::Bool(true)),
            ]))))
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
        "browser_attr" => |args| crate::state::browser_attr(&args),
        "browser_page_text" => |_| crate::state::browser_page_text(),
        "browser_wait_for_ms" => |args| crate::state::browser_wait_for_ms(&args),
        "browser_close" => |_| crate::state::browser_close(),
        "socket_close" => |args| {
            match args.first() {
                Some(Value::Socket(s)) => {
                    s.lock().unwrap().shutdown(std::net::Shutdown::Both).ok();
                }
                Some(Value::UdpSocket(_)) => {}
                Some(Value::Listener(_)) => {}
                _ => return Err("socket.close expects a Socket, UdpSocket, or Listener".into()),
            }
            Ok(Value::Bool(true))
        },
        "socket_listen" => |args| {
            let addr = arg_string(&args, 0)?;
            let backlog = match args.get(1) {
                Some(Value::Number(n)) => *n as u32,
                _ => 128,
            };
            let listener = std::net::TcpListener::bind(&addr)
                .map_err(|e| format!("failed to bind listener on {addr}: {e}"))?;
            listener.set_nonblocking(false).ok();
            // TcpListener has no set backlog method after bind; use OS default
            Ok(Value::Listener(Arc::new(Mutex::new(listener))))
        },
        "socket_accept" => |args| {
            let listener = match args.first() {
                Some(Value::Listener(l)) => l.clone(),
                _ => return Err("socket_accept expects a Listener".into()),
            };
            let timeout_ms = match args.get(1) {
                Some(Value::Number(n)) => *n as u64,
                _ => 0,
            };
            if timeout_ms > 0 {
                listener.lock().unwrap().set_nonblocking(true)
                    .map_err(|e| format!("set nonblocking: {e}"))?;
                let start = std::time::Instant::now();
                let dur = std::time::Duration::from_millis(timeout_ms);
                loop {
                    match listener.lock().unwrap().accept() {
                        Ok((stream, addr)) => {
                            stream.set_nonblocking(false).ok();
                            let mut result = BTreeMap::new();
                            result.insert("socket".into(), Value::Socket(Arc::new(Mutex::new(stream))));
                            result.insert("addr".into(), Value::String(addr.to_string()));
                            return Ok(Value::Dict(Arc::new(result)));
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            if start.elapsed() >= dur {
                                return Ok(Value::Null);
                            }
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                        Err(e) => return Err(format!("accept error: {e}")),
                    }
                }
            }
            let (stream, addr) = listener.lock().unwrap().accept()
                .map_err(|e| format!("accept error: {e}"))?;
            let mut result = BTreeMap::new();
            result.insert("socket".into(), Value::Socket(Arc::new(Mutex::new(stream))));
            result.insert("addr".into(), Value::String(addr.to_string()));
            Ok(Value::Dict(Arc::new(result)))
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
            Ok(Value::Dict(Arc::new(session)))
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
            Ok(Value::List(Arc::new(names)))
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
            Ok(Value::Dict(Arc::new(session)))
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
            Ok(Value::Dict(Arc::new(session)))
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
            Ok(Value::Dict(Arc::new(out)))
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
                    items.push(Value::List(Arc::new(vec![Value::Number(id), Value::Number(size)])));
                }
            }
            Ok(Value::List(Arc::new(items)))
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
                // POP3 dot-unstuffing: ".." at start of line means literal "."
                let actual = if line.starts_with("..") { &line[1..] } else { &line };
                content.push_str(actual);
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
            let session_id = next_response_id();
            session.insert("__id".into(), Value::Number(session_id as f64));
            session.insert("socket".into(), Value::Socket(Arc::new(Mutex::new(stream))));
            session.insert("host".into(), Value::String(host));
            session.insert("tag".into(), Value::Number(2.0));
            Ok(Value::Dict(Arc::new(session)))
        },
        "imap_select" => |args| {
            let stream = session_socket(&args[0])?;
            let mailbox = arg_string(&args, 1)?;
            let tag = imap_next_tag(&args[0])?;
            let mut s = stream.lock().unwrap();
            let tag_str = format!("a{tag}");
            imap_command(&mut s, &tag_str, &format!("SELECT {mailbox}"))?;
            drop(s);
            Ok(Value::Bool(true))
        },
        "imap_search" => |args| {
            let stream = session_socket(&args[0])?;
            let criteria = match args.get(1) {
                Some(Value::String(s)) => s.clone(),
                _ => "ALL".into(),
            };
            let tag = imap_next_tag(&args[0])?;
            let mut s = stream.lock().unwrap();
            let tag_str = format!("a{tag}");
            let resp = imap_command(&mut s, &tag_str, &format!("SEARCH {criteria}"))?;
            drop(s);
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
            Ok(Value::List(Arc::new(ids)))
        },
        "imap_fetch" => |args| {
            let stream = session_socket(&args[0])?;
            let id = arg_number(&args, 1)? as u64;
            let tag = imap_next_tag(&args[0])?;
            let mut s = stream.lock().unwrap();
            let tag_str = format!("a{tag}");
            let resp = imap_command(&mut s, &tag_str, &format!("FETCH {id} (FLAGS BODY[])"))?;
            drop(s);
            let mut flags = Vec::new();
            let mut body = String::new();
            for line in resp.lines() {
                if let Some(rest) = line.strip_prefix("*") {
                    if rest.contains("FLAGS") {
                        if let Some(start_idx) = rest.find('(') {
                            let flags_str = &rest[start_idx + 1..];
                            if let Some(end_idx) = flags_str.find(')') {
                                for f in flags_str[..end_idx].split_whitespace() {
                                    flags.push(Value::String(f.trim_matches('\\').to_string()));
                                }
                            }
                        }
                    }
                }
            }
            if let Some(paren_end) = resp.find(") BODY[]") {
                let literal_section = &resp[paren_end + 9..];
                if let Some(nl) = literal_section.find('\n') {
                    body = literal_section[nl + 1..].trim_end().to_string();
                } else {
                    body = literal_section.trim_end().to_string();
                }
            }
            let mut out = BTreeMap::new();
            out.insert("flags".into(), Value::List(Arc::new(flags)));
            out.insert("body".into(), Value::String(body));
            Ok(Value::Dict(Arc::new(out)))
        },
        "imap_list" => |args| {
            let stream = session_socket(&args[0])?;
            let tag = imap_next_tag(&args[0])?;
            let mut s = stream.lock().unwrap();
            let tag_str = format!("a{tag}");
            let resp = imap_command(&mut s, &tag_str, "LIST \"\" *")?;
            drop(s);
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
            Ok(Value::List(Arc::new(boxes)))
        },
        "imap_logout" => |args| {
            let stream = session_socket(&args[0])?;
            let tag = imap_next_tag(&args[0])?;
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
            Ok(Value::Dict(Arc::new(session)))
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
            let (clean, replies) = strip_telnet_iac(&buf[..n]);
            if !replies.is_empty() {
                let _ = s.write_all(&replies);
            }
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
            let (clean, replies) = strip_telnet_iac(&buf);
            if !replies.is_empty() {
                let _ = s.write_all(&replies);
            }
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
            Ok(Value::List(Arc::new(ips)))
        },
        "dns_query" => |args| {
            let name = arg_string(&args, 0)?;
            let rtype = match args.get(1) {
                Some(Value::String(s)) => s.clone(),
                _ => "A".into(),
            };
            Ok(Value::List(Arc::new(dns_query_impl(&name, &rtype)?)))
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
            Ok(Value::Dict(Arc::new(layer)))
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
            Ok(Value::Dict(Arc::new(layer)))
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
            Ok(Value::Dict(Arc::new(layer)))
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
            Ok(Value::Dict(Arc::new(layer)))
        },
        "scapy_raw" => |args| {
            let data = arg_string(&args, 0)?;
            let mut layer = BTreeMap::new();
            layer.insert("type".into(), Value::String("Raw".into()));
            layer.insert("data".into(), Value::String(data));
            Ok(Value::Dict(Arc::new(layer)))
        },
        "scapy_build" => |args| {
            let layer = arg_dict(&args, 0)?;
            let bytes = layer_bytes(&layer)?;
            Ok(Value::List(Arc::new(bytes.iter().map(|b| Value::Number(*b as f64)).collect::<Vec<Value>>())))
        },
        "scapy_parse" => |args| {
            let data: Vec<u8> = match args.first() {
                Some(Value::List(items)) => {
                    let mut bytes = Vec::with_capacity(items.len());
                    for item in items.iter().cloned() {
                        match item {
                            Value::Number(n) => {
                                if n < 0.0 || n > 255.0 || n.fract() != 0.0 {
                                    return Err(format!("scapy.parse byte out of range (0-255): {n}"));
                                }
                                bytes.push(n as u8);
                            }
                            other => {
                                return Err(format!("scapy.parse expects a list of byte numbers, got {other:?}"))
                            }
                        }
                    }
                    bytes
                }
                _ => arg_string(&args, 0)?.into_bytes(),
            };
            Ok(parse_packet(&data))
        },
        "scapy_send" => |args| {
            let is_root = unsafe { libc::geteuid() == 0 };
            if !is_root {
                return Err("scapy send/sniff requires root privileges. Run with: sudo zen <script>".into());
            }
            let bytes: Vec<u8> = match args.first() {
                Some(Value::Dict(_)) => layer_bytes(&arg_dict(&args, 0)?)?,
                Some(Value::List(items)) => {
                    let mut bytes = Vec::with_capacity(items.len());
                    for item in items.iter().cloned() {
                        match item {
                            Value::Number(n) => {
                                if n < 0.0 || n > 255.0 || n.fract() != 0.0 {
                                    return Err(format!("scapy.send byte out of range (0-255): {n}"));
                                }
                                bytes.push(n as u8);
                            }
                            other => {
                                return Err(format!("scapy.send expects a packet dict or a list of bytes, got {other:?}"))
                            }
                        }
                    }
                    bytes
                }
                other => return Err(format!("scapy.send expects a packet dict or a list of bytes, got {other:?}")),
            };
            raw_socket_send(&bytes)?;
            Ok(Value::Bool(true))
        },
        "scapy_sniff" => |args| {
            let is_root = unsafe { libc::geteuid() == 0 };
            if !is_root {
                return Err("scapy send/sniff requires root privileges. Run with: sudo zen <script>".into());
            }
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
        "scapy_cidr_expand" => |args| {
            let cidr = arg_string(&args, 0)?;
            let (ip_part, prefix_part) = match cidr.split_once('/') {
                Some(p) => p,
                None => return Err(format!("scapy.cidr_expand expects a CIDR like 192.168.1.0/24, got: {cidr}")),
            };
            let prefix: u32 = prefix_part
                .trim()
                .parse()
                .map_err(|_| format!("bad prefix length: {prefix_part}"))?;
            if prefix > 32 {
                return Err(format!("prefix length must be 0-32, got: {prefix}"));
            }
            let base = ip_str_to_u32(ip_part)?;
            let mask: u32 = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
            let network = base & mask;
            let count: u64 = 1 << (32 - prefix);
            if count > 1_048_576 {
                return Err(format!("CIDR range too large ({count} addresses); use a larger prefix"));
            }
            let mut out = Vec::with_capacity(count as usize);
            for i in 0..count {
                out.push(Value::String(u32_to_ip(network.wrapping_add(i as u32))));
            }
            Ok(Value::List(Arc::new(out)))
        },
        "scapy_subnet_hosts" => |args| {
            let network = arg_string(&args, 0)?;
            let netmask = arg_string(&args, 1)?;
            let net = ip_str_to_u32(&network)?;
            let mask = ip_str_to_u32(&netmask)?;
            let start = net & mask;
            let end = start | !mask;
            let total = (end - start) as u64 + 1;
            // Conventional hosts(): exclude network and broadcast when possible
            let (first, last) = if total > 2 { (start + 1, end - 1) } else { (start, end) };
            let count = (last - first) as u64 + 1;
            if count > 1_048_576 {
                return Err(format!("subnet too large ({count} hosts); use a narrower netmask"));
            }
            let mut out = Vec::with_capacity(count as usize);
            for i in first..=last {
                out.push(Value::String(u32_to_ip(i)));
            }
            Ok(Value::List(Arc::new(out)))
        },

        // ── bluetooth module ──────────────────────────────────────────
        "bt_status" => |_| {
            let out = Command::new("hciconfig").arg("hci0").output()
                .map_err(|e| format!("hciconfig not found: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            let up = stdout.contains("UP RUNNING");
            let addr = stdout.lines()
                .find(|l| l.contains("BD Address:"))
                .and_then(|l| l.split("BD Address: ").nth(1))
                .map(|s| s.split_whitespace().next().unwrap_or("").to_string())
                .unwrap_or_default();
            let name = stdout.lines()
                .find(|l| l.contains("Name:"))
                .and_then(|l| l.split("Name: '").nth(1))
                .and_then(|s| s.split('\'').next())
                .unwrap_or("")
                .to_string();
            let mut result = BTreeMap::new();
            result.insert("adapter".into(), Value::String("hci0".into()));
            result.insert("up".into(), Value::Bool(up));
            result.insert("address".into(), Value::String(addr));
            result.insert("name".into(), Value::String(name));
            Ok(Value::Dict(Arc::new(result)))
        },
        "bt_power" => |args| {
            let on = match args.first() {
                Some(Value::Bool(b)) => *b,
                Some(Value::Number(n)) => *n != 0.0,
                _ => true,
            };
            let state = if on { "on" } else { "off" };
            let out = Command::new("bluetoothctl").arg("power").arg(state).output()
                .map_err(|e| format!("bluetoothctl not found: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(Value::Bool(stdout.contains("succeeded") || on))
        },
        "bt_scan" => |args| {
            let timeout_s = match args.first() {
                Some(Value::Number(n)) => *n as u64,
                _ => 10,
            };
            Command::new("bluetoothctl").arg("scan").arg("on").output()
                .map_err(|e| format!("bluetoothctl scan: {e}"))?;
            std::thread::sleep(std::time::Duration::from_secs(timeout_s));
            Command::new("bluetoothctl").arg("scan").arg("off").output().ok();
            let out = Command::new("bluetoothctl").arg("devices").output()
                .map_err(|e| format!("bluetoothctl devices: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut devices = Vec::new();
            for line in stdout.lines() {
                if let Some(rest) = line.strip_prefix("Device ") {
                    let parts: Vec<&str> = rest.splitn(2, ' ').collect();
                    if parts.len() >= 2 {
                        let mut d = BTreeMap::new();
                        d.insert("address".into(), Value::String(parts[0].to_string()));
                        d.insert("name".into(), Value::String(parts[1].trim().to_string()));
                        devices.push(Value::Dict(Arc::new(d)));
                    }
                }
            }
            Ok(Value::List(Arc::new(devices)))
        },
        "bt_scan_stop" => |_| {
            Command::new("bluetoothctl").arg("scan").arg("off").output()
                .map_err(|e| format!("bluetoothctl scan off: {e}"))?;
            Ok(Value::Bool(true))
        },
        "bt_devices" => |args| {
            let filter = match args.first() {
                Some(Value::String(s)) => s.clone(),
                _ => "all".into(),
            };
            let subcmd = match filter.as_str() {
                "paired" => "paired-devices",
                _ => "devices",
            };
            let out = Command::new("bluetoothctl").arg(subcmd).output()
                .map_err(|e| format!("bluetoothctl {subcmd}: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut devices = Vec::new();
            for line in stdout.lines() {
                let stripped = line.strip_prefix("Device ").unwrap_or(line);
                let parts: Vec<&str> = stripped.splitn(2, ' ').collect();
                if parts.len() >= 2 {
                    let addr = parts[0].trim();
                    let name = parts[1].trim();
                    if !addr.contains(':') { continue; }
                    let mut d = BTreeMap::new();
                    d.insert("address".into(), Value::String(addr.to_string()));
                    d.insert("name".into(), Value::String(name.to_string()));
                    devices.push(Value::Dict(Arc::new(d)));
                }
            }
            Ok(Value::List(Arc::new(devices)))
        },
        "bt_pair" => |args| {
            let addr = arg_string(&args, 0)?;
            let out = Command::new("bluetoothctl").arg("pair").arg(&addr).output()
                .map_err(|e| format!("bluetoothctl pair: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(Value::Bool(stdout.contains("succeeded") || stdout.contains("Pairing successful")))
        },
        "bt_unpair" => |args| {
            let addr = arg_string(&args, 0)?;
            let out = Command::new("bluetoothctl").arg("remove").arg(&addr).output()
                .map_err(|e| format!("bluetoothctl remove: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(Value::Bool(stdout.contains("succeeded") || stdout.contains("removed")))
        },
        "bt_connect" => |args| {
            let addr = arg_string(&args, 0)?;
            let out = Command::new("bluetoothctl").arg("connect").arg(&addr).output()
                .map_err(|e| format!("bluetoothctl connect: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(Value::Bool(stdout.contains("succeeded") || stdout.contains("Connection successful")))
        },
        "bt_disconnect" => |args| {
            let addr = arg_string(&args, 0)?;
            let out = Command::new("bluetoothctl").arg("disconnect").arg(&addr).output()
                .map_err(|e| format!("bluetoothctl disconnect: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(Value::Bool(stdout.contains("succeeded") || true))
        },
        "bt_trust" => |args| {
            let addr = arg_string(&args, 0)?;
            let out = Command::new("bluetoothctl").arg("trust").arg(&addr).output()
                .map_err(|e| format!("bluetoothctl trust: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(Value::Bool(stdout.contains("succeeded") || stdout.contains("trust")))
        },
        "bt_send" => |args| {
            let addr = arg_string(&args, 0)?;
            let data = arg_string(&args, 1)?;
            let out = Command::new("bluetoothctl").arg("send").arg(&addr).arg(&data).output()
                .map_err(|e| format!("bluetoothctl send: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(Value::Bool(stdout.contains("succeeded") || true))
        },

        // ── wifi module ───────────────────────────────────────────────
        "wifi_scan" => |args| {
            let iface = match args.first() {
                Some(Value::String(s)) => s.clone(),
                _ => "wlan0".into(),
            };
            let out = Command::new("nmcli").args(["-t", "-f", "SSID,BSSID,MODE,FREQ,RATE,SIGNAL,SECURITY", "device", "wifi", "list", "ifname", &iface]).output()
                .map_err(|e| format!("nmcli wifi list: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut networks = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for line in stdout.lines() {
                // nmcli -t uses : as separator; BSSID contains colons escaped as \:
                // Reassemble by joining everything after first 6 fields
                let raw: Vec<&str> = line.split(':').collect();
                if raw.len() < 7 { continue; }
                let ssid = raw[0].replace("\\:", ":");
                if ssid.is_empty() || seen.contains(&ssid) { continue; }
                seen.insert(ssid.clone());
                // BSSID is fields 1..6 joined with unescaped colons
                let bssid: String = raw[1..6].iter().map(|s| s.replace("\\:", ":")).collect::<Vec<String>>().join(":");
                let mode = raw[6].replace("\\:", ":");
                let freq = if raw.len() > 7 { raw[7].replace("\\:", ":") } else { String::new() };
                let rate = if raw.len() > 8 { raw[8].replace("\\:", ":") } else { String::new() };
                let signal = if raw.len() > 9 { raw[9].replace("\\:", ":") } else { String::new() };
                let security = raw[raw.len()-1..].join("").replace("\\:", ":");
                let mut net = BTreeMap::new();
                net.insert("ssid".into(), Value::String(ssid));
                net.insert("bssid".into(), Value::String(bssid));
                net.insert("mode".into(), Value::String(mode));
                net.insert("frequency".into(), Value::String(freq));
                net.insert("speed".into(), Value::String(rate));
                net.insert("signal".into(), Value::String(signal));
                net.insert("security".into(), Value::String(security));
                networks.push(Value::Dict(Arc::new(net)));
            }
            Ok(Value::List(Arc::new(networks)))
        },
        "wifi_status" => |_| {
            let out = Command::new("nmcli").args(["-t", "-f", "WIFI", "general"]).output()
                .map_err(|e| format!("nmcli general: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            let enabled = stdout.trim() == "enabled";
            let conn = Command::new("nmcli").args(["-t", "-f", "NAME,DEVICE,TYPE", "connection", "show", "--active"]).output()
                .map_err(|e| format!("nmcli connection show: {e}"))?;
            let conn_out = String::from_utf8_lossy(&conn.stdout);
            let mut result = BTreeMap::new();
            result.insert("wifi_enabled".into(), Value::Bool(enabled));
            result.insert("connected".into(), Value::Bool(false));
            for line in conn_out.lines() {
                if line.contains(":wifi:") || line.contains(":802-11-wireless:") {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 2 {
                        result.insert("ssid".into(), Value::String(parts[0].to_string()));
                        result.insert("device".into(), Value::String(parts[1].to_string()));
                        result.insert("connected".into(), Value::Bool(true));
                    }
                }
            }
            Ok(Value::Dict(Arc::new(result)))
        },
        "wifi_connect" => |args| {
            let ssid = arg_string(&args, 0)?;
            let password = match args.get(1) {
                Some(Value::String(s)) => s.clone(),
                _ => String::new(),
            };
            let mut cmd = Command::new("nmcli");
            cmd.args(["device", "wifi", "connect", &ssid]);
            if !password.is_empty() {
                cmd.args(["password", &password]);
            }
            let out = cmd.output().map_err(|e| format!("nmcli wifi connect: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(Value::Bool(stdout.contains("successfully activated") || stdout.contains("connected")))
        },
        "wifi_disconnect" => |args| {
            let iface = match args.first() {
                Some(Value::String(s)) => s.clone(),
                _ => "wlan0".into(),
            };
            let out = Command::new("nmcli").args(["device", "disconnect", &iface]).output()
                .map_err(|e| format!("nmcli device disconnect: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(Value::Bool(stdout.contains("successfully disconnected") || true))
        },
        "wifi_forget" => |args| {
            let ssid = arg_string(&args, 0)?;
            let out = Command::new("nmcli").args(["connection", "delete", &ssid]).output()
                .map_err(|e| format!("nmcli connection delete: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            Ok(Value::Bool(stdout.contains("successfully deleted") || true))
        },
        "wifi_interfaces" => |_| {
            let out = Command::new("nmcli").args(["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device"]).output()
                .map_err(|e| format!("nmcli device: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut ifaces = Vec::new();
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() < 4 { continue; }
                if !parts[1].contains("wireless") && parts[1] != "wifi" { continue; }
                let mut d = BTreeMap::new();
                d.insert("device".into(), Value::String(parts[0].to_string()));
                d.insert("type".into(), Value::String(parts[1].to_string()));
                d.insert("state".into(), Value::String(parts[2].to_string()));
                d.insert("connection".into(), Value::String(parts[3].to_string()));
                ifaces.push(Value::Dict(Arc::new(d)));
            }
            Ok(Value::List(Arc::new(ifaces)))
        },
        "wifi_list" => |_| {
            let out = Command::new("nmcli").args(["-t", "-f", "NAME,UUID,TYPE,DEVICE", "connection", "show"]).output()
                .map_err(|e| format!("nmcli connection show: {e}"))?;
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut conns = Vec::new();
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() < 4 { continue; }
                if !parts[2].contains("wireless") && parts[2] != "802-11-wireless" { continue; }
                let mut d = BTreeMap::new();
                d.insert("name".into(), Value::String(parts[0].to_string()));
                d.insert("uuid".into(), Value::String(parts[1].to_string()));
                d.insert("device".into(), Value::String(parts[3].to_string()));
                conns.push(Value::Dict(Arc::new(d)));
            }
            Ok(Value::List(Arc::new(conns)))
        },

        // ── crunch module (Rust-native for speed) ─────────────────────
        "crunch_charset" => |args| {
            let name = arg_string(&args, 0)?;
            let cs = match name.as_str() {
                "a" | "lower" => "abcdefghijklmnopqrstuvwxyz",
                "A" | "upper" => "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
                "d" | "n" | "digits" | "numeric" => "0123456789",
                "s" | "symbols" => "!@#$%^&*()-_=+[]{}|;:',.<>?/",
                "h" | "hex" => "0123456789abcdef",
                "x" | "alnum" => "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
                "p" | "print" => "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+[]{}|;:',.<>?/ ",
                "all" => "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+[]{}|;:',.<>?/",
                _ => &name,
            };
            Ok(Value::String(cs.to_string()))
        },
        "crunch_generate" => |args| {
            let min_len = arg_number(&args, 0)? as usize;
            let max_len = arg_number(&args, 1)? as usize;
            let charset_name = arg_string(&args, 2)?;
            let charset = match charset_name.as_str() {
                "a" | "lower" => "abcdefghijklmnopqrstuvwxyz",
                "A" | "upper" => "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
                "d" | "n" | "digits" | "numeric" => "0123456789",
                "s" | "symbols" => "!@#$%^&*()-_=+[]{}|;:',.<>?/",
                "h" | "hex" => "0123456789abcdef",
                "x" | "alnum" => "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
                _ => &charset_name,
            };
            let chars: Vec<char> = charset.chars().collect();
            let mut result = Vec::new();
            for len in min_len..=max_len {
                crunch_generate_len(len, &chars, &mut String::new(), &mut result);
                if result.len() > 1_000_000 {
                    break;
                }
            }
            Ok(Value::List(Arc::new(result.into_iter().map(|s| Value::String(s)).collect::<Vec<Value>>())))
        },
        "crunch_pattern" => |args| {
            let template = arg_string(&args, 0)?;
            crunch_pattern_impl(&template)
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
            Ok(Value::List(Arc::new(parts)))
        },
        "str_splitlines" => |args| {
            let s = arg_string(&args, 0)?;
            let parts: Vec<Value> = s
                .lines()
                .map(|l| Value::String(l.to_string()))
                .collect();
            Ok(Value::List(Arc::new(parts)))
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
                    for v in l.iter().cloned() {
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
            Ok(Value::Dict(Arc::new(result)))
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
            Ok(Value::Dict(Arc::new(BTreeMap::from([
                ("hexdigest".into(), Value::String(digest)),
                ("name".into(), Value::String(algo)),
            ]))))
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
                return Ok(Value::Dict(Arc::new(out)));
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
            Ok(Value::List(Arc::new(suffixes)))
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
            Ok(Value::List(Arc::new(results)))
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
            Ok(Value::Dict(Arc::new(out)))
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
                            Arc::make_mut(l).push(Value::String(url_unquote(v)));
                        }
                    })
                    .or_insert_with(|| {
                        Value::List(Arc::new(vec![Value::String(url_unquote(v))]))
                    });
            }
            Ok(Value::Dict(Arc::new(out)))
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
            Ok(Value::Dict(Arc::new(out)))
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
            Ok(Value::List(Arc::new(out)))
        },
        "collections_flatten" => |args| {
            let list = arg_list(&args, 0)?;
            let mut out = Vec::new();
            flatten_list(&list, &mut out);
            Ok(Value::List(Arc::new(out)))
        },
        "itertools_enumerate" => |args| {
            let list = arg_list(&args, 0)?;
            let mut out = Vec::new();
            for (i, item) in list.into_iter().enumerate() {
                out.push(Value::List(Arc::new(vec![Value::Number(i as f64), item])));
            }
            Ok(Value::List(Arc::new(out)))
        },
        "itertools_zip" => |args| {
            let a = arg_list(&args, 0)?;
            let b = arg_list(&args, 1)?;
            let mut out = Vec::new();
            let n = a.len().min(b.len());
            for i in 0..n {
                out.push(Value::List(Arc::new(vec![a[i].clone(), b[i].clone()])));
            }
            Ok(Value::List(Arc::new(out)))
        },
        "itertools_chain" => |args| {
            let mut out = Vec::new();
            for arg in args {
                if let Value::List(l) = arg {
                    out.extend(l.iter().cloned());
                }
            }
            Ok(Value::List(Arc::new(out)))
        },
        "itertools_repeat" => |args| {
            let value = args.first().cloned().ok_or("itertools.repeat: missing value")?;
            let n = match args.get(1) {
                Some(Value::Number(x)) => *x as usize,
                _ => return Err("itertools.repeat: needs a count".into()),
            };
            Ok(Value::List(Arc::new(vec![value; n])))
        },
        "itertools_product" => |args| {
            let a = arg_list(&args, 0)?;
            let b = arg_list(&args, 1)?;
            let mut out = Vec::new();
            for x in &a {
                for y in &b {
                    out.push(Value::List(Arc::new(vec![x.clone(), y.clone()])));
                }
            }
            Ok(Value::List(Arc::new(out)))
        },
        "itertools_permutations" => |args| {
            let list = arg_list(&args, 0)?;
            let r = match args.get(1) {
                Some(Value::Number(n)) => *n as usize,
                _ => list.len(),
            };
            let mut out = Vec::new();
            permutations(&list, r, &mut vec![], &mut vec![false; list.len()], &mut out);
            Ok(Value::List(Arc::new(out)))
        },
        "itertools_combinations" => |args| {
            let list = arg_list(&args, 0)?;
            let r = arg_number(&args, 1)? as usize;
            let mut out = Vec::new();
            combinations(&list, r, 0, &mut vec![], &mut out);
            Ok(Value::List(Arc::new(out)))
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
            Ok(Value::List(Arc::new(out)))
        },
        "itertools_take" => |args| {
            let n = arg_number(&args, 0)? as usize;
            let list = arg_list(&args, 1)?;
            Ok(Value::List(Arc::new(list.into_iter().take(n).collect::<Vec<Value>>())))
        },
        "itertools_drop" => |args| {
            let n = arg_number(&args, 0)? as usize;
            let list = arg_list(&args, 1)?;
            Ok(Value::List(Arc::new(list.into_iter().skip(n).collect::<Vec<Value>>())))
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
            Ok(Value::List(Arc::new(out)))
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
        vm.exec_module(&stmts)?;
    }
    let flow = vm.exec_module(&program)?;
    match flow {
        Flow::Normal => Ok(()),
        Flow::Return(_) => Err("return used outside a function\n  \x1b[1;33m= help:\x1b[0m `return` can only be used inside a function body defined with `function` or `def`".into()),
        Flow::Break => Err("break used outside a loop\n  \x1b[1;33m= help:\x1b[0m `break` can only be used inside a `while` or `for` loop".into()),
        Flow::Continue => Err("continue used outside a loop\n  \x1b[1;33m= help:\x1b[0m `continue` can only be used inside a `while` or `for` loop".into()),
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
    let lines: Vec<String> = source.lines().map(|l| l.to_string()).collect();
    let mut out = String::new();

    // ── Header ───────────────────────────────────────────────────────────────
    out.push_str(&format!(
        "\x1b[1;31merror\x1b[0m\x1b[1m[{}]\x1b[0m: {}\n",
        ty, msg
    ));
    out.push_str(&format!(
        " \x1b[1;34m-->\x1b[0m {}:{}:{}\n",
        file, line, col
    ));

    // ── Source context ───────────────────────────────────────────────────────
    out.push_str(&format!("  \x1b[1;34m|\x1b[0m\n"));
    if line > 0 && !lines.is_empty() {
        out.push_str(&render_context(&lines, line, col, 2));
    }

    // ── Annotation footer ────────────────────────────────────────────────────
    out.push_str(&format!("  \x1b[1;34m|\x1b[0m\n"));

    // Suggestion for common error patterns
    let lower_msg = msg.to_lowercase();
    if lower_msg.contains("undefined") || lower_msg.contains("not defined") || lower_msg.contains("unknown variable") {
        if let Some(name) = msg.split_whitespace().find(|w| w.chars().all(|c| c.is_alphanumeric() || c == '_')) {
            let all_names: Vec<&str> = lines.iter()
                .flat_map(|l| l.split_whitespace())
                .filter(|w| w.chars().all(|c| c.is_alphanumeric() || c == '_') && w.len() > 1)
                .collect();
            if let Some(suggestion) = suggest_name(name, &all_names, 3) {
                out.push_str(&format!(
                    "  \x1b[1;33m= help:\x1b[0m a variable named `{suggestion}` is in scope\n"
                ));
                out.push_str(&format!(
                    "  \x1b[1;34m|       \x1b[0m did you mean `{suggestion}` instead of `{name}`?\n"
                ));
            }
        }
    }

    // Type mismatch suggestions
    if lower_msg.contains("cannot") && (lower_msg.contains("add") || lower_msg.contains("multiply") || lower_msg.contains("compare")) {
        out.push_str(&format!(
            "  \x1b[1;33m= help:\x1b[0m try converting the operands to a common type first\n"
        ));
        out.push_str(&format!(
            "  \x1b[1;34m|       \x1b[0m e.g.  str(num) + \" items\"  or  int(str_val)\n"
        ));
    }

    // Index/key error suggestions
    if lower_msg.contains("index") || lower_msg.contains("key") {
        out.push_str(&format!(
            "  \x1b[1;33m= note:\x1b[0m  list indices start at 0, not 1\n"
        ));
        if lower_msg.contains("out of range") {
            out.push_str(&format!(
                "  \x1b[1;33m= help:\x1b[0m check the length with len(collection) before indexing\n"
            ));
        }
    }

    // Null/None dereference
    if lower_msg.contains("null") || lower_msg.contains("none") {
        out.push_str(&format!(
            "  \x1b[1;33m= note:\x1b[0m  the value is null — check if a function returned null unexpectedly\n"
        ));
    }

    // Division by zero
    if lower_msg.contains("divide") || lower_msg.contains("division") || lower_msg.contains("zero") {
        out.push_str(&format!(
            "  \x1b[1;33m= help:\x1b[0m check that the divisor is not zero before dividing\n"
        ));
    }

    // Type error for missing method
    if lower_msg.contains("has no method") || lower_msg.contains("no attribute") {
        out.push_str(&format!(
            "  \x1b[1;33m= help:\x1b[0m verify the type with typeof(value) before calling methods\n"
        ));
    }

    out.push_str(&format!(
        "  \x1b[1;31m= {}: {}\x1b[0m\n",
        ty, msg
    ));
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
            vm.exec_module(&stmts)?;
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
                match self.vm.exec_module(&program) {
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
        ("socket", "TCP/UDP networking (open, send, recv, listen, accept, scan)"),
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
        ("ftp", "Pure-Rust FTP client (connect, login, list, retr, stor, etc.)"),
        ("smtp", "Pure-Rust SMTP client (connect, login, sendmail, message)"),
        ("pop3", "Pure-Rust POP3 client (connect, stat, list, retr, dele)"),
        ("imap", "Pure-Rust IMAP client (connect, select, search, fetch)"),
        ("telnet", "Pure-Rust telnet client (connect, write, read, read_until)"),
        ("dns", "DNS resolver (resolve, query)"),
        ("bluetooth", "Bluetooth via bluetoothctl (scan, pair, connect, send)"),
        ("wifi", "WiFi via nmcli (scan, connect, disconnect, status)"),
        ("crunch", "Password wordlist generator (generate, pattern, charset)"),
        ("hydra", "Brute-force password tester (SSH, FTP, HTTP, Telnet, SMTP)"),
        ("browser", "Browser automation via CDP (Chromium only)"),
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
             Built-in: Error, TypeError, ValueError, IndexError, KeyError,\n\
             FileNotFoundError, ZeroDivisionError, ArithmeticError, RuntimeError,\n\
             NotImplementedError, StopIteration, AssertionError, ImportError,\n\
             RecursionError, OSError, SystemExit\n\n\
             Custom errors:\n\
             errors.define(\"MyError\", \"Error\", \"default message\")\n\
             throw new MyError(\"details\")\n\n\
             Catch syntax:\n\
             try { ... } catch as e { print e }\n\
             try { ... } catch TypeError as e { ... }\n\
             try { ... } catch MyError as e { ... } catch as e { ... }"
                .into(),
        ),
        "json" => Some(
            "json — JSON encode/decode\n\n\
             json.parse(string)          decode JSON string to dict/list\n\
             json.encode(value)          encode value to JSON string\n\
             json.stringify(value)       alias for encode\n\
             json.load(path)             read and parse JSON file\n\
             json.save(path, value)      encode and write JSON file"
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
             os.name                  OS name (constant)\n\
             os.arch()                CPU architecture\n\
             os.pid()                 current process ID\n\
             os.pids()                list all process IDs\n\
             os.kill(pid, signal?)    send signal to process (default SIGTERM)\n\
             os.env(key?)             environment variable or all env vars\n\
             os.setenv(key, val)      set environment variable\n\
             os.unsetenv(key)         remove environment variable\n\
             os.execute(cmd)          run command, return {ok, code, stdout, stderr}\n\
             os.run(cmd)              run command, return stdout or throw on failure\n\
             os.popen(cmd)            run command (alias for execute)\n\
             os.system(cmd)           run command, return exit code\n\
             os.cwd()                 current working directory\n\
             os.chdir(path)           change directory\n\
             os.home()                home directory\n\
             os.hostname()            machine hostname\n\
             os.cpu_count()           number of CPUs\n\
             os.exit(code?)           exit program"
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
             uuid.uuid1()  or uuid.v1()   time-based UUID\n\
             uuid.uuid3(n) or uuid.v3(n)  name-based UUID (MD5)\n\
             uuid.uuid4()  or uuid.v4()   random UUID\n\
             uuid.uuid5(n) or uuid.v5(n)  name-based UUID (SHA-1)"
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
            "socket — Low-level TCP/UDP networking\n\n\
             socket.open(host, port)         TCP connect\n\
             socket.send(session, data)      send string data\n\
             socket.recv(session, n?)        receive up to n bytes (default 4096), returns byte list\n\
             socket.listen(addr, backlog?)   bind TCP server, returns listener\n\
             socket.accept(listener, ms?)    accept connection, returns {socket, addr}\n\
             socket.open_udp(addr)           UDP connect\n\
             socket.send_to(udp, data, addr?)  send UDP data\n\
             socket.recv_from(udp, n?)       receive UDP, returns {data, addr, text}\n\
             socket.scan(host, start?, end?, timeout?)  TCP port scan\n\
             socket.set_timeout(session, ms?)  set read/write timeout\n\
             socket.close(session)           close connection"
                .into(),
        ),
        "browser" => Some(
            "browser — Browser automation (Chrome DevTools Protocol, Chromium/Chrome only)\n\n\
             Navigation:\n\
               browser.go(url)              navigate + wait for load\n\
               browser.launch(headless?, port?)  explicit launch\n\
               browser.connect()            launch headful (visible)\n\
               browser.close()              close and kill process\n\n\
             Reading:\n\
               browser.title()              page title\n\
               browser.url()                current URL\n\
               browser.page()               full HTML\n\
               browser.page_text()          full page text\n\
               browser.text(selector?)      element text\n\
               browser.attr(sel, name)      element attribute value\n\
               browser.query(selector)      list of texts for all matches\n\n\
             Interaction:\n\
               browser.click(sel)           click element\n\
               browser.fill(sel, val)       fill input + fire events\n\
               browser.eval(js)             evaluate JavaScript\n\n\
             Waiting:\n\
               browser.wait_for(sel)         wait up to 20s\n\
               browser.wait_for_ms(sel, ms)  wait up to ms milliseconds\n\n\
             Note: Firefox is not supported (CDP protocol limitation)."
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
            "smtp — Pure-Rust SMTP client (plaintext)\n\n\
             smtp.connect(host, port?)          connect (default port 25)\n\
             smtp.login(session, user, pass)    authenticate (AUTH LOGIN, plaintext)\n\
             smtp.sendmail(session, from, to, msg)  send email\n\
             smtp.message(from, to, sub, body)  build MIME message string\n\
             smtp.quit(session)                 disconnect\n\n\
             Note: No TLS/STARTTLS. Use port 465 with TLS or ensure network security."
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
             Record types: A, AAAA, MX, TXT, NS, CNAME\n\n\
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
        "bluetooth" => Some(
            "bluetooth — Bluetooth via bluetoothctl (Linux/BlueZ)\n\n\
             bluetooth.status()                adapter status {up, address, name}\n\
             bluetooth.power(on?)              power adapter on/off\n\
             bluetooth.scan(timeout?)          scan for devices (default 10s)\n\
             bluetooth.scan_stop()             stop scanning\n\
             bluetooth.devices(filter?)        list devices (filter: 'all', 'paired')\n\
             bluetooth.pair(addr)              pair with device\n\
             bluetooth.unpair(addr)            remove device\n\
             bluetooth.connect(addr)           connect to device\n\
             bluetooth.disconnect(addr)        disconnect from device\n\
             bluetooth.trust(addr)             trust device\n\
             bluetooth.send(addr, data)        send data\n\n\
             Requires bluetoothd running.\n\
             Example: bluetooth.scan(5)"
                .into(),
        ),
        "wifi" => Some(
            "wifi — WiFi via NetworkManager (nmcli)\n\n\
             wifi.scan(iface?)                 scan networks (default wlan0)\n\
             wifi.status()                     connection status {wifi_enabled, connected, ssid}\n\
             wifi.connect(ssid, password?)     connect to network\n\
             wifi.disconnect(iface?)           disconnect (default wlan0)\n\
             wifi.forget(ssid)                 forget saved network\n\
             wifi.interfaces()                 list wireless interfaces\n\
             wifi.list()                       list saved connections\n\n\
             Example: wifi.connect(\"MyNetwork\", \"password\")"
                .into(),
        ),
        "crunch" => Some(
            "crunch — Password wordlist generator (Rust-native, fast)\n\n\
             crunch.charset(name)              get charset string\n\
             crunch.generate(min, max, charset) generate all combos\n\
             crunch.pattern(template)          pattern-based generation\n\n\
             Charset shortcuts:\n\
               :a = lower   :A = upper   :d = digits   :s = symbols\n\
               :n = digits  :x = alnum   :h = hex      :p = printable\n\n\
             Pattern syntax:\n\
               :X     = single char from charset X\n\
               :X{N}  = repeat charset X exactly N times\n\
               :X{N,M} = repeat X between N and M times\n\
               ?      = single lowercase char (legacy)\n\n\
             Examples:\n\
               crunch.pattern(\"admin:d:d:d:d\")  => admin0000..admin9999\n\
               crunch.pattern(\":A:a:a:d{3}\")    => Aaa000..Zzz999\n\
               crunch.pattern(\"pass:s:d{2,4}\")   => pass!00..pass!9999\n\
                crunch.generate(4, 6, \"digits\")    => 0000..999999"
                .into(),
        ),
        "hydra" => Some(
            "hydra — Brute-force password tester (SSH, FTP, HTTP, Telnet, SMTP)\n\n\
             Single tests:\n\
               hydra.ssh(host, user, pass)             test SSH credential\n\
               hydra.ftp(host, user, pass)             test FTP credential\n\
               hydra.http(host, user, pass, url?)      test HTTP basic auth\n\
               hydra.telnet(host, user, pass)           test telnet login\n\
               hydra.smtp(host, user, pass)             test SMTP auth\n\
               hydra.test(proto, host, user, pass)      test by protocol name\n\n\
             Brute-force:\n\
               hydra.run(proto, host, user, passwords)  try password list\n\
               hydra.run_file(proto, host, user, path)  try passwords from file\n\n\
             Protocols: ssh, ftp, http, telnet, smtp\n\n\
             Results: {success, password, attempts, errors, elapsed, results}\n\n\
             Examples:\n\
               hydra.ssh(\"10.0.0.1\", \"root\", \"toor\")\n\
               hydra.run(\"ftp\", \"10.0.0.1\", \"admin\", [\"1234\",\"pass\",\"test\"])\n\
               hydra.run_file(\"ssh\", \"10.0.0.1\", \"root\", \"passwords.txt\")"
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
       \"hello\"     string (UTF-8)\n\
     \"\"\"hello\n  world\"\"\"  triple-quoted string (multiline, preserves newlines)\n\
     'hello'      string (single-quoted, no interpolation)\n\
     '''hello\n  world'''  triple-quoted single string (no interpolation)\n\n\
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
        print(values..., sep?, end?)  print to stdout (space-separated, newline appended)\n\
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
       import        import a module or package\n\
       from          selective import (from mod import name)\n\
       as            alias (import mod as m, from mod import f as g)\n\
       load          load and execute a file\n\n\
     Packages:\n\
       import pkg              load pkg/main.z or pkg/pkg.z\n\
       import pkg.sub          load pkg/sub.z\n\
       import /path/to/mod     load by absolute path\n\
       from pkg import sub     load pkg/sub.z as sub\n\
       from pkg.sub import f   load f from pkg/sub.z\n\
       zen pm init [name]      create zen.json + main.z\n\
       zen pm install <spec>   install from repo/url/file/directory\n\n\
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

/// Return help for a specific built-in function or constant.
pub fn help_builtin(name: &str) -> Option<String> {
    match name {
        // I/O
        "help" => Some(
            "help() / help(x)  — show help information\n\
             No args: lists all builtins and keywords.\n\
             With a value: shows structure (class methods, dict keys, list length, etc).\n\n\
             Example: help()           — list all builtins\n\
             Example: help(print)      — show print signature\n\
             Example: help(MyClass)    — show class methods\n\
             Example: help(mydict)     — show dict keys or __doc__ info"
                .into(),
        ),
        "print" => Some(
            "print(values..., sep?, end?)  — print values to stdout\n\
             Prints each argument separated by spaces (or custom sep), followed by\n\
             a newline (or custom end).\n\n\
             Example: print(\"hello\", 42, true)  =>  hello 42 true\n\
             Example: print(1 + 1)  =>  2\n\
             Example: print(\"a\", \"b\", sep=\"-\")  =>  a-b\n\
             Example: print(\"hi\", end=\"\")  =>  hi (no newline)"
                .into(),
        ),
        "input" => Some(
            "input(prompt?)  — read a line from stdin\n\
             Shows the prompt string (if given), reads one line, returns it as a string.\n\n\
             Example: let name = input(\"Name: \")\n\
             Example: let line = input()"
                .into(),
        ),
        "exit" => Some(
            "exit(code?)  — terminate the program\n\
             Exits with the given exit code (default 0).\n\n\
             Example: exit(0)\n\
             Example: exit(1)"
                .into(),
        ),
        "len" => Some(
            "len(x)  — length of a string, list, or dict\n\
             Returns the number of characters (string), elements (list), or keys (dict).\n\n\
             Example: len(\"hello\")  =>  5\n\
             Example: len([1, 2, 3])  =>  3\n\
             Example: len({a: 1, b: 2})  =>  2"
                .into(),
        ),

        // Type conversion
        "str" => Some(
            "str(x)  — convert to string\n\
             Converts any value to its string representation.\n\n\
             Example: str(42)  =>  \"42\"\n\
             Example: str(true)  =>  \"true\"\n\
             Example: str([1,2])  =>  \"[1, 2]\""
                .into(),
        ),
        "int" => Some(
            "int(x)  — convert to integer\n\
             Converts a value to an integer. Floats are truncated (toward zero).\n\
             Strings are parsed as base-10 integers.\n\n\
             Example: int(3.9)  =>  3\n\
             Example: int(\"42\")  =>  42\n\
             Example: int(true)  =>  1"
                .into(),
        ),
        "float" => Some(
            "float(x)  — convert to float\n\
             Converts a value to a floating-point number.\n\n\
             Example: float(42)  =>  42.0\n\
             Example: float(\"3.14\")  =>  3.14"
                .into(),
        ),
        "bool" => Some(
            "bool(x)  — convert to boolean\n\
             Converts a value to a boolean.\n\
             Falsy: null, false, 0, 0.0, \"\"\n\
             Everything else is truthy.\n\n\
             Example: bool(0)  =>  false\n\
             Example: bool(\"hello\")  =>  true\n\
             Example: bool(null)  =>  false"
                .into(),
        ),
        "list" => Some(
            "list(x)  — convert to list\n\
             Converts a value to a list.\n\
             Strings become list of characters. Dicts become list of [key, value] pairs.\n\n\
             Example: list(\"abc\")  =>  [a, b, c]\n\
             Example: list({a: 1})  =>  [[a, 1]]"
                .into(),
        ),
        "dict" => Some(
            "dict(pairs)  — create dict from pairs\n\
             Creates a dict from a list of [key, value] pairs.\n\n\
             Example: dict([[\"a\", 1], [\"b\", 2]])  =>  {a: 1, b: 2}"
                .into(),
        ),
        "typeof" => Some(
            "typeof x  — type name of a value\n\
             Returns a string naming the type: \"null\", \"bool\", \"int\", \"float\",\n\
             \"string\", \"list\", \"dict\", \"function\", \"class\", \"instance\", etc.\n\
             Can also be written as: type(x)\n\n\
             Example: typeof 42  =>  \"int\"\n\
             Example: typeof \"hello\"  =>  \"string\"\n\
             Example: typeof [1]  =>  \"list\""
                .into(),
        ),
        "type" => Some(
            "type(x)  — alias for typeof x\n\
             Returns the type name as a string.\n\n\
             Example: type(3.14)  =>  \"float\""
                .into(),
        ),

        // Numeric
        "abs" => Some(
            "abs(x)  — absolute value\n\
             Returns the absolute (non-negative) value.\n\n\
             Example: abs(-5)  =>  5\n\
             Example: abs(3.14)  =>  3.14"
                .into(),
        ),
        "min" => Some(
            "min(a, b, ...)  — minimum value\n\
             Returns the smallest of the given values.\n\
             For a single list argument, finds the minimum element.\n\n\
             Example: min(3, 1, 2)  =>  1\n\
             Example: min([5, 2, 8])  =>  2"
                .into(),
        ),
        "max" => Some(
            "max(a, b, ...)  — maximum value\n\
             Returns the largest of the given values.\n\
             For a single list argument, finds the maximum element.\n\n\
             Example: max(3, 1, 2)  =>  3\n\
             Example: max([5, 2, 8])  =>  8"
                .into(),
        ),
        "round" => Some(
            "round(x)  — round to nearest integer\n\
             Rounds a number to the nearest integer.\n\n\
             Example: round(3.6)  =>  4\n\
             Example: round(3.2)  =>  3\n\
             Example: round(-1.5)  =>  -2"
                .into(),
        ),
        "trunc" => Some(
            "trunc(x)  — truncate to integer\n\
             Truncates toward zero (drops the decimal part).\n\n\
             Example: trunc(3.9)  =>  3\n\
             Example: trunc(-3.9)  =>  -3"
                .into(),
        ),
        "hex" => Some(
            "hex(x)  — hex string representation\n\
             Converts an integer to a hex string with \"0x\" prefix.\n\n\
             Example: hex(255)  =>  \"0xff\"\n\
             Example: hex(16)  =>  \"0x10\""
                .into(),
        ),
        "range" => Some(
            "range(end) / range(start, end)  — inclusive range list\n\
             Returns a list of integers.\n\
             One arg: [0, 1, ..., end].\n\
             Two args: [start, start+1, ..., end].\n\n\
             Example: range(5)  =>  [0, 1, 2, 3, 4, 5]\n\
             Example: range(2, 5)  =>  [2, 3, 4, 5]"
                .into(),
        ),

        // Time
        "sleep" => Some(
            "sleep(sec)  — pause execution for N seconds\n\
             Blocks the current thread for the given number of seconds (fractional OK).\n\n\
             Example: sleep(1)     — pause 1 second\n\
             Example: sleep(0.5)   — pause 500 milliseconds"
                .into(),
        ),
        "wait" => Some(
            "wait(ms)  — pause execution for N milliseconds\n\
             Blocks the current thread for the given number of milliseconds.\n\n\
             Example: wait(1000)   — pause 1 second\n\
             Example: wait(500)    — pause 500 milliseconds"
                .into(),
        ),

        // Error handling
        "throw" => Some(
            "throw value  — raise an error\n\
             Throws a value as an error. Can be caught with try/catch.\n\
             Values can be strings, dicts, or instances of error classes.\n\n\
             Example: throw \"something went wrong\"\n\
             Example: throw {type: \"Error\", message: \"oops\"}\n\
             Example: errors.define(\"MyError\", \"Exception\", \"msg\")\n\
             throw MyError(\"details\")"
                .into(),
        ),
        "raise" => Some(
            "raise value  — alias for throw\n\
             Same as throw. Raises an error that can be caught with try/catch.\n\n\
             Example: raise \"oops\""
                .into(),
        ),
        "try" => Some(
            "try { ... } catch <var> { ... } finally { ... }  — error handling\n\
             try: code that might throw\n\
             catch: handles the error (supports typed catch)\n\
             finally: always runs, whether or not there was an error\n\n\
             Example:\n\
             try {\n\
               let x = 1 / 0\n\
             } catch e {\n\
               print \"Error:\", e\n\
             }\n\n\
             Typed catch:\n\
             try {\n\
               throw ValueError(\"bad\")\n\
             } catch ValueError as e {\n\
               print e.message\n\
             } catch TypeError as e {\n\
               print \"wrong type\"\n\
             }"
                .into(),
        ),
        "catch" => Some(
            "catch [as var] { ... }  — handle error in try block\n\
             Catches an error thrown by throw/raise. Without a type, catches all.\n\
             With a type name, catches only that error type.\n\n\
             Example: catch e { print e }\n\
             Example: catch ValueError as e { print e.message }\n\
             Example: catch TypeError as e { ... }"
                .into(),
        ),
        "finally" => Some(
            "finally { ... }  — always-execute block\n\
             Used after try/catch. Runs regardless of whether an error was thrown.\n\n\
             Example:\n\
             try { risky_call() }\n\
             catch e { handle(e) }\n\
             finally { cleanup() }"
                .into(),
        ),
        "assert" => Some(
            "assert condition  — assertion check\n\
             Throws an error if the condition is falsy.\n\n\
             Example: assert 1 == 1\n\
             Example: assert len([1, 2]) == 2"
                .into(),
        ),

        // Variables
        "let" => Some(
            "let name = value  — declare a mutable variable\n\
             Creates a new variable in the current scope.\n\n\
             Example: let x = 42\n\
             Example: let name = \"zen\""
                .into(),
        ),
        "const" => Some(
            "const NAME = value  — declare an immutable constant\n\
             Creates a constant that cannot be reassigned.\n\
             Convention: UPPER_CASE names.\n\n\
             Example: const PI = 3.14159\n\
             Example: const MAX_RETRIES = 3"
                .into(),
        ),
        "global" => Some(
            "global name  — declare a global variable\n\
             Makes a variable accessible across all scopes.\n\n\
             Example: global counter = 0"
                .into(),
        ),

        // Functions
        "func" | "def" => Some(
            "func name(args) { body }  — define a named function\n\
             'def' and 'fn' are aliases for 'func'.\n\
             Functions can have default arguments and rest parameters.\n\n\
             Example:\n\
             func add(a, b) {\n\
               return a + b\n\
             }\n\
             print add(2, 3)  // 5\n\n\
             Default args:\n\
             func greet(name = \"world\") {\n\
               print \"hello \" + name\n\
             }\n\n\
             Rest parameters:\n\
             func sum(nums...) {\n\
               let total = 0\n\
               for n in nums { total += n }\n\
               return total\n\
             }"
                .into(),
        ),
        "fn" => Some(
            "fn(args) { body }  — define a named function (alias for func)\n\
             Same as func. Can also be used as a lambda keyword.\n\n\
             Example: fn double(x) { x * 2 }"
                .into(),
        ),
        "lambda" => Some(
            "lambda(args) { body }  — anonymous function\n\
             Creates an unnamed function value. Can be assigned to a variable.\n\n\
             Example: let f = lambda(x) { x * 2 }\n\
             Example: [1, 2, 3].map(lambda(x) { x * 2 })\n\
             Example: [1, 2, 3].filter(lambda(x) { x > 1 })"
                .into(),
        ),
        "return" => Some(
            "return value  — return from a function\n\
             Exits the current function, returning the given value.\n\
             Without a value, returns null.\n\n\
             Example:\n\
             func abs(x) {\n\
               if x < 0 { return -x }\n\
               return x\n\
             }"
                .into(),
        ),

        // Control flow
        "if" => Some(
            "if condition { body } elif cond { body } else { body }  — conditional\n\
             Branches based on a condition. elif and else are optional.\n\n\
             Example:\n\
             if x > 10 {\n\
               print \"big\"\n\
             } elif x > 5 {\n\
               print \"medium\"\n\
             } else {\n\
               print \"small\"\n\
             }\n\n\
             As expression:\n\
             let size = if x > 10 { \"big\" } else { \"small\" }"
                .into(),
        ),
        "elif" => Some(
            "elif condition { body }  — else-if branch\n\
             Used after if or another elif. Adds another condition to check.\n\n\
             Example:\n\
             if x > 0 { print \"positive\" }\n\
             elif x < 0 { print \"negative\" }\n\
             else { print \"zero\" }"
                .into(),
        ),
        "else" => Some(
            "else { body }  — default branch\n\
             Executes when no preceding if/elif condition is true.\n\n\
             Example:\n\
             if x > 0 { print \"positive\" }\n\
             else { print \"non-positive\" }"
                .into(),
        ),
        "while" => Some(
            "while condition { body }  — while loop\n\
             Repeats the body while the condition is true.\n\n\
             Example:\n\
             let i = 0\n\
             while i < 5 {\n\
               print i\n\
               i += 1\n\
             }\n\n\
             Use break to exit early, continue to skip to next iteration."
                .into(),
        ),
        "for" => Some(
            "for var in iterable { body }  — for-each loop\n\
             Iterates over each element in a list, string, or dict.\n\n\
             Example:\n\
             for x in [1, 2, 3] {\n\
               print x\n\
             }\n\n\
             Dict iteration:\n\
             for key in {a: 1, b: 2} {\n\
               print key, \":\", {a: 1, b: 2}[key]\n\
             }\n\n\
             String iteration:\n\
             for ch in \"hello\" {\n\
               print ch\n\
             }"
                .into(),
        ),
        "in" => Some(
            "x in collection  — membership test\n\
             Returns true if x is an element of the collection.\n\
             For strings: true if x is a substring.\n\
             For dicts: true if x is a key.\n\n\
             Example: 2 in [1, 2, 3]  =>  true\n\
             Example: \"l\" in \"hello\"  =>  true\n\
             Example: \"a\" in {a: 1}  =>  true"
                .into(),
        ),
        "break" => Some(
            "break  — exit a loop immediately\n\
             Used inside while or for loops to stop looping.\n\n\
             Example:\n\
             for x in [1, 2, 3, 4, 5] {\n\
               if x == 3 { break }\n\
               print x\n\
             }\n\
             // prints 1, 2"
                .into(),
        ),
        "continue" => Some(
            "continue  — skip to next loop iteration\n\
             Used inside while or for loops to skip the rest of the body.\n\n\
             Example:\n\
             for x in [1, 2, 3, 4, 5] {\n\
               if x % 2 == 0 { continue }\n\
               print x\n\
             }\n\
             // prints 1, 3, 5"
                .into(),
        ),

        // Classes
        "class" => Some(
            "class Name { ... } / class Name < Parent { ... }  — define a class\n\
             Creates a class with optional inheritance.\n\
             'inherit' keyword is an alias for '<'.\n\n\
             Example:\n\
             class Animal {\n\
               func init(name) {\n\
                 this.name = name\n\
               }\n\
               func speak() {\n\
                 return \"...\"\n\
               }\n\
             }\n\
             class Dog < Animal {\n\
               func speak() {\n\
                 return \"woof!\"\n\
               }\n\
             }\n\
             let d = Dog(\"Rex\")\n\
             print d.speak()  // woof!"
                .into(),
        ),
        "inherit" => Some(
            "inherit  — inheritance keyword (alias for <)\n\
             Used in class declarations to inherit from a parent class.\n\n\
             Example: class Dog < Animal { ... }\n\
             Example: class Dog inherit Animal { ... }"
                .into(),
        ),
        "this" => Some(
            "this  — reference to the current instance\n\
             Inside a class method, 'this' refers to the instance being used.\n\
             Used to access fields and methods of the current object.\n\n\
             Example:\n\
             class Person {\n\
               func init(name, age) {\n\
                 this.name = name\n\
                 this.age = age\n\
               }\n\
               func greet() {\n\
                 return \"Hi, I'm \" + this.name\n\
               }\n\
             }"
                .into(),
        ),
        "new" => Some(
            "new is a reserved keyword in Zen.\n\
             Class instances are created by calling the class name directly:\n\n\
             Example:\n\
             class Foo { func init(x) { this.x = x } }\n\
             let f = Foo(42)  // calls Foo.new(42) internally\n\
             print f.x  // 42\n\n\
             Note: You cannot use 'new' as a variable or function name."
                .into(),
        ),

        // Modules
        "import" => Some(
            "import module  — import a module\n\
             Loads a .z file or makes a built-in module available.\n\
             All built-in modules are already available as globals —\n\
             import is mainly for .z files.\n\n\
             Example: import mylib\n\
             Example: import mylib.z"
                .into(),
        ),
        "from" => Some(
            "from module import name  — selective import\n\
             Imports specific names from a module.\n\n\
             Example: from string import upper, lower\n\
             Example: from itertools import range, enumerate"
                .into(),
        ),

        // Match/When
        "match" => Some(
            "match value { pattern => expr, ... }  — pattern matching\n\
             Matches a value against patterns and returns the matching expression.\n\n\
             Example:\n\
             match x {\n\
               1 => \"one\",\n\
               2 => \"two\",\n\
               _ => \"other\"\n\
             }\n\n\
             With guards:\n\
             match x {\n\
               n if n > 0 => \"positive\",\n\
               0 => \"zero\",\n\
               _ => \"negative\"\n\
             }"
                .into(),
        ),
        "when" => Some(
            "when { condition => expr, ... }  — expression-based branching\n\
             Like match, but evaluates conditions (like acond). Returns the value\n\
             for the first true condition.\n\n\
             Example:\n\
             let label = when {\n\
               x > 10 => \"big\",\n\
               x > 5 => \"medium\",\n\
               _ => \"small\"\n\
             }"
                .into(),
        ),

        // Special
        "null" => Some(
            "null  — the null value\n\
             Represents absence of value or nothingness.\n\
             Equality: null == null is true.\n\
             Falsy in boolean context.\n\n\
             Example:\n\
             let x = null\n\
             if x == null { print \"nothing\" }"
                .into(),
        ),
        "true" => Some(
            "true  — boolean true\n\
             One of two boolean values (true/false).\n\
             Returned by comparisons and truthiness checks.\n\n\
             Example: 1 == 1  =>  true\n\
             Example: bool(1)  =>  true"
                .into(),
        ),
        "false" => Some(
            "false  — boolean false\n\
             One of two boolean values (true/false).\n\
             Falsy values: null, false, 0, 0.0, \"\".\n\n\
             Example: 1 == 2  =>  false"
                .into(),
        ),
        "is" => Some(
            "x is Type  — type checking\n\
             Returns true if x is an instance of the given class/type.\n\n\
             Example: 42 is int  =>  true\n\
             Example: \"hi\" is string  =>  true\n\
             Example: [1] is list  =>  true"
                .into(),
        ),
        "as" => Some(
            "as  — used in imports and typed catch\n\
             In imports: from mod import func as alias\n\
             In catch: catch TypeError as e { ... }\n\n\
             Example: from string import upper as up\n\
             Example: catch ValueError as e { print e }"
                .into(),
        ),

        // String methods
        "format" => Some(
            "s.format(args...)  — format string with placeholders\n\
             Replaces {} with positional args, {0} with indexed args, {name} with named args.\n\
             Named args come from dict arguments.\n\n\
             Example: \"hello {}\".format(\"world\")  =>  \"hello world\"\n\
             Example: \"{0} + {1}\".format(1, 2)  =>  \"1 + 2\"\n\
             Example: \"{a} is {b}\".format({a: \"Zen\", b: \"cool\"})  =>  \"Zen is cool\""
                .into(),
        ),
        "find" => Some(
            "s.find(needle)  — find substring position\n\
             Returns the index of the first occurrence, or -1 if not found.\n\n\
             Example: \"hello world\".find(\"world\")  =>  6\n\
             Example: \"hello\".find(\"xyz\")  =>  -1"
                .into(),
        ),
        "includes" | "contains" => Some(
            "s.includes(needle)  — check if string contains substring\n\
             Returns true if the string contains the given substring.\n\n\
             Example: \"hello\".includes(\"ell\")  =>  true\n\
             Example: \"hello\".includes(\"xyz\")  =>  false"
                .into(),
        ),
        "startsWith" | "startswith" => Some(
            "s.startsWith(prefix)  — check if string starts with prefix\n\
             Returns true if the string starts with the given prefix.\n\n\
             Example: \"hello\".startsWith(\"hel\")  =>  true\n\
             Example: \"hello\".startsWith(\"xyz\")  =>  false"
                .into(),
        ),
        "endsWith" | "endswith" => Some(
            "s.endsWith(suffix)  — check if string ends with suffix\n\
             Returns true if the string ends with the given suffix.\n\n\
             Example: \"hello\".endsWith(\"llo\")  =>  true\n\
             Example: \"hello\".endsWith(\"xyz\")  =>  false"
                .into(),
        ),
        "charAt" | "char" => Some(
            "s.charAt(index)  — get character at index\n\
             Returns a single-character string at the given index.\n\n\
             Example: \"hello\".charAt(0)  =>  \"h\"\n\
             Example: \"hello\".charAt(4)  =>  \"o\""
                .into(),
        ),
        "ord" => Some(
            "s.ord()  — get Unicode codepoint of first character\n\
             Returns the integer codepoint of the first character.\n\n\
             Example: \"A\".ord()  =>  65\n\
             Example: \"Z\".ord()  =>  90"
                .into(),
        ),
        "split" => Some(
            "s.split(sep)  — split string by separator\n\
             Returns a list of substrings.\n\n\
             Example: \"a,b,c\".split(\",\")  =>  [a, b, c]\n\
             Example: \"hello world\".split(\" \")  =>  [hello, world]"
                .into(),
        ),
        "replace" => Some(
            "s.replace(from, to)  — replace substring\n\
             Returns a new string with all occurrences of 'from' replaced by 'to'.\n\n\
             Example: \"hello\".replace(\"l\", \"r\")  =>  \"herro\""
                .into(),
        ),
        "trim" | "strip" => Some(
            "s.trim()  — remove leading/trailing whitespace\n\
             Returns a new string with whitespace stripped from both ends.\n\n\
             Example: \"  hello  \".trim()  =>  \"hello\""
                .into(),
        ),
        "trimStart" | "trimLeft" | "trim_left" => Some(
            "s.trimStart()  — remove leading whitespace\n\
             Returns a new string with whitespace stripped from the left.\n\n\
             Example: \"  hello\".trimStart()  =>  \"hello\""
                .into(),
        ),
        "trimEnd" | "trimRight" | "trim_right" => Some(
            "s.trimEnd()  — remove trailing whitespace\n\
             Returns a new string with whitespace stripped from the right.\n\n\
             Example: \"hello  \".trimEnd()  =>  \"hello\""
                .into(),
        ),
        "lower" | "toLower" | "toLowerCase" => Some(
            "s.lower()  — convert to lowercase\n\
             Returns a new lowercase string.\n\n\
             Example: \"Hello\".lower()  =>  \"hello\""
                .into(),
        ),
        "upper" | "toUpper" | "toUpperCase" => Some(
            "s.upper()  — convert to uppercase\n\
             Returns a new uppercase string.\n\n\
             Example: \"hello\".upper()  =>  \"HELLO\""
                .into(),
        ),
        "reverse" => Some(
            "s.reverse()  — reverse string\n\
             Returns a new string with characters in reverse order.\n\n\
             Example: \"hello\".reverse()  =>  \"olleh\""
                .into(),
        ),
        "length" => Some(
            "s.length()  — number of characters\n\
             Returns the character count of the string.\n\n\
             Example: \"hello\".length()  =>  5"
                .into(),
        ),
        "repeat" => Some(
            "s.repeat(n)  — repeat string n times\n\
             Returns a new string with the original repeated n times.\n\n\
             Example: \"ha\".repeat(3)  =>  \"hahaha\""
                .into(),
        ),
        "concat" => Some(
            "s.concat(other...)  — concatenate strings\n\
             Appends one or more strings to the original.\n\n\
             Example: \"hello\".concat(\" \", \"world\")  =>  \"hello world\""
                .into(),
        ),
        "substring" | "substr" | "slice" => Some(
            "s.substring(start, end?)  — extract substring\n\
             Returns characters from index start to end (exclusive).\n\
             If end is omitted, goes to the end of the string.\n\n\
             Example: \"hello\".substring(1, 4)  =>  \"ell\"\n\
             Example: \"hello\".substring(2)  =>  \"llo\""
                .into(),
        ),
        "indexOf" => Some(
            "s.indexOf(needle)  — find first occurrence index\n\
             Returns the index of the first occurrence, or -1 if not found.\n\
             Alias for find().\n\n\
             Example: \"hello\".indexOf(\"ll\")  =>  2"
                .into(),
        ),
        "toList" => Some(
            "s.toList()  — convert to list of characters\n\
             Returns a list where each element is a single-character string.\n\n\
             Example: \"abc\".toList()  =>  [a, b, c]"
                .into(),
        ),

        // List methods
        "list.push" | "push" => Some(
            "list.push(item)  — append element to end of list\n\
             Returns the modified list.\n\n\
             Example: [1, 2].push(3)  =>  [1, 2, 3]"
                .into(),
        ),
        "list.pop" | "pop" => Some(
            "list.pop()  — remove and return last element\n\
             Returns the removed element, or null if empty.\n\n\
             Example: [1, 2, 3].pop()  =>  3"
                .into(),
        ),
        "list.join" | "join" => Some(
            "list.join(sep?)  — join elements into string\n\
             Concatenates all elements with an optional separator.\n\n\
             Example: [1, 2, 3].join(\",\")  =>  \"1,2,3\"\n\
             Example: [\"a\", \"b\"].join(\"-\")  =>  \"a-b\""
                .into(),
        ),
        "list.reverse" | "reverse" => Some(
            "list.reverse()  — reverse the list in place\n\
             Returns the reversed list.\n\n\
             Example: [1, 2, 3].reverse()  =>  [3, 2, 1]"
                .into(),
        ),
        "list.sort" | "sort" => Some(
            "list.sort()  — sort list in place\n\
             Returns the sorted list (alphabetical for strings, numeric for numbers).\n\n\
             Example: [3, 1, 2].sort()  =>  [1, 2, 3]\n\
             Example: [\"b\", \"a\"].sort()  =>  [a, b]"
                .into(),
        ),
        "list.first" | "first" => Some(
            "list.first()  — get first element\n\
             Returns the first element, or null if empty.\n\n\
             Example: [1, 2, 3].first()  =>  1"
                .into(),
        ),
        "list.last" | "last" => Some(
            "list.last()  — get last element\n\
             Returns the last element, or null if empty.\n\n\
             Example: [1, 2, 3].last()  =>  3"
                .into(),
        ),
        "list.contains" => Some(
            "list.contains(item)  — check if list contains item\n\
             Returns true if the element is in the list.\n\n\
             Example: [1, 2, 3].contains(2)  =>  true\n\
             Example: [1, 2, 3].contains(4)  =>  false"
                .into(),
        ),
        "list.includes" | "list.index_of" | "index_of" => Some(
            "list.includes(item)  — find item position in list\n\
             Returns the index of the first occurrence, or -1 if not found.\n\n\
             Example: [10, 20, 30].includes(20)  =>  1\n\
             Example: [10, 20, 30].includes(40)  =>  -1"
                .into(),
        ),
        "list.length" => Some(
            "list.length()  — number of elements\n\
             Returns the element count of the list.\n\n\
             Example: [1, 2, 3].length()  =>  3"
                .into(),
        ),
        "list.sum" => Some(
            "list.sum()  — sum of numeric elements\n\
             Returns the sum of all number elements.\n\n\
             Example: [1, 2, 3].sum()  =>  6"
                .into(),
        ),
        "list.flat" | "list.flatten" | "flatten" => Some(
            "list.flat()  — flatten nested lists one level\n\
             Returns a new list with one level of nesting removed.\n\n\
             Example: [[1, 2], [3, 4]].flat()  =>  [1, 2, 3, 4]"
                .into(),
        ),
        "list.compact" | "compact" => Some(
            "list.compact()  — remove null/false/empty values\n\
             Returns a new list with all falsy values removed.\n\n\
             Example: [1, null, 2, false, 3].compact()  =>  [1, 2, 3]"
                .into(),
        ),
        "list.uniq" | "list.unique" | "uniq" => Some(
            "list.uniq()  — remove duplicate elements\n\
             Returns a new list with duplicates removed.\n\n\
             Example: [1, 2, 2, 3, 3].uniq()  =>  [1, 2, 3]"
                .into(),
        ),
        "list.shuffle" | "shuffle" => Some(
            "list.shuffle()  — randomize element order\n\
             Returns a new list in random order.\n\n\
             Example: [1, 2, 3].shuffle()  =>  [2, 1, 3] (varies)"
                .into(),
        ),
        "list.sample" | "sample" => Some(
            "list.sample()  — pick random element\n\
             Returns a random element from the list.\n\n\
             Example: [1, 2, 3].sample()  =>  2 (varies)"
                .into(),
        ),
        "list.take" | "take" => Some(
            "list.take(n)  — take first n elements\n\
             Returns a new list with only the first n elements.\n\n\
             Example: [1, 2, 3, 4].take(2)  =>  [1, 2]"
                .into(),
        ),
        "list.drop" | "drop" => Some(
            "list.drop(n)  — skip first n elements\n\
             Returns a new list without the first n elements.\n\n\
             Example: [1, 2, 3, 4].drop(2)  =>  [3, 4]"
                .into(),
        ),
        "list.chunk" | "chunk" => Some(
            "list.chunk(size)  — split into chunks\n\
             Returns a list of sub-lists of the given size.\n\n\
             Example: [1, 2, 3, 4, 5].chunk(2)  =>  [[1, 2], [3, 4], [5]]"
                .into(),
        ),
        "list.copy" | "copy" => Some(
            "list.copy()  — shallow copy of list\n\
             Returns a new list with the same elements.\n\n\
             Example: [1, 2, 3].copy()  =>  [1, 2, 3]"
                .into(),
        ),
        "list.splice" | "splice" => Some(
            "list.splice(start, delete_count, items...)  — insert/remove elements\n\
             Removes elements and optionally inserts new ones.\n\n\
             Example: [1, 2, 3].splice(1, 1, 4, 5)  =>  [1, 4, 5, 3]"
                .into(),
        ),
        "list.map" | "map" => Some(
            "list.map(func)  — transform each element\n\
             Returns a new list by applying the function to each element.\n\n\
             Example: [1, 2, 3].map(lambda(x) { x * 2 })  =>  [2, 4, 6]"
                .into(),
        ),
        "list.filter" | "filter" => Some(
            "list.filter(func)  — keep elements where func returns true\n\
             Returns a new list with only matching elements.\n\n\
             Example: [1, 2, 3, 4].filter(lambda(x) { x > 2 })  =>  [3, 4]"
                .into(),
        ),
        "list.reduce" | "reduce" => Some(
            "list.reduce(func, init?)  — accumulate elements with a function\n\
             Reduces the list to a single value. If no init, uses the first element.\n\n\
             Example: [1, 2, 3].reduce(lambda(a, b) { a + b })  =>  6\n\
             Example: [1, 2, 3].reduce(lambda(a, b) { a + b }, 10)  =>  16"
                .into(),
        ),
        "list.slice" => Some(
            "list.slice(start, end?)  — extract a sub-list\n\
             Returns elements from start to end (exclusive).\n\n\
             Example: [1, 2, 3, 4].slice(1, 3)  =>  [2, 3]\n\
             Example: [1, 2, 3, 4].slice(2)  =>  [3, 4]"
                .into(),
        ),
        "list.skip" => Some(
            "list.skip(n)  — skip first n elements\n\
             Alias for drop(). Returns a new list without the first n elements.\n\n\
             Example: [1, 2, 3, 4].skip(2)  =>  [3, 4]"
                .into(),
        ),
        "list.concat" => Some(
            "list.concat(other...)  — merge lists\n\
             Returns a new list combining all arguments.\n\n\
             Example: [1, 2].concat([3, 4])  =>  [1, 2, 3, 4]"
                .into(),
        ),
        "list.zip" => Some(
            "list.zip(other)  — pair elements from two lists\n\
             Returns a list of two-element sub-lists pairing corresponding elements.\n\n\
             Example: [1, 2, 3].zip([a, b, c])  =>  [[1, a], [2, b], [3, c]]"
                .into(),
        ),

        // Dict methods
        "dict.keys" | "keys" => Some(
            "dict.keys()  — list of all keys\n\
             Returns a list of all keys in the dict.\n\n\
             Example: {a: 1, b: 2}.keys()  =>  [a, b]"
                .into(),
        ),
        "dict.values" | "values" => Some(
            "dict.values()  — list of all values\n\
             Returns a list of all values in the dict.\n\n\
             Example: {a: 1, b: 2}.values()  =>  [1, 2]"
                .into(),
        ),
        "dict.has" | "dict.has_key" | "dict.containsKey" | "dict.contains" | "has_key" | "has" => Some(
            "dict.has(key)  — check if key exists\n\
             Returns true if the dict has the given key.\n\n\
             Example: {a: 1}.has(\"a\")  =>  true\n\
             Example: {a: 1}.has(\"b\")  =>  false"
                .into(),
        ),
        "dict.get" | "get" => Some(
            "dict.get(key, default?)  — get value by key\n\
             Returns the value for the key, or default (null if omitted).\n\n\
             Example: {a: 1}.get(\"a\")  =>  1\n\
             Example: {a: 1}.get(\"b\", 0)  =>  0"
                .into(),
        ),
        "dict.set" | "set" => Some(
            "dict.set(key, value)  — set key-value pair\n\
             Returns the modified dict.\n\n\
             Example: {a: 1}.set(\"b\", 2)  =>  {a: 1, b: 2}"
                .into(),
        ),
        "dict.delete" | "dict.remove" | "delete" => Some(
            "dict.delete(key)  — remove a key\n\
             Returns the modified dict without the given key.\n\n\
             Example: {a: 1, b: 2}.delete(\"a\")  =>  {b: 2}"
                .into(),
        ),
        "dict.update" | "update" => Some(
            "dict.update(other)  — merge other dict into this one\n\
             Overwrites existing keys. Returns the modified dict.\n\n\
             Example: {a: 1}.update({a: 2, b: 3})  =>  {a: 2, b: 3}"
                .into(),
        ),
        "dict.merge" | "merge" => Some(
            "dict.merge(other)  — merge two dicts (returns new dict)\n\
             Returns a new dict combining both. Other's keys take precedence.\n\n\
             Example: {a: 1}.merge({b: 2})  =>  {a: 1, b: 2}"
                .into(),
        ),
        "dict.map_values" | "map_values" => Some(
            "dict.map_values(func)  — transform each value\n\
             Returns a new dict with the function applied to each value.\n\n\
             Example: {a: 1, b: 2}.map_values(lambda(v) { v * 10 })  =>  {a: 10, b: 20}"
                .into(),
        ),
        "dict.filter_values" | "filter_values" => Some(
            "dict.filter_values(func)  — keep entries where func returns true\n\
             Returns a new dict with only matching entries.\n\n\
             Example: {a: 1, b: 2, c: 3}.filter_values(lambda(v) { v > 1 })  =>  {b: 2, c: 3}"
                .into(),
        ),
        "dict.key_of" | "key_of" => Some(
            "dict.key_of(value)  — find key for a value\n\
             Returns the first key with the given value, or null if not found.\n\n\
             Example: {a: 1, b: 2}.key_of(2)  =>  \"b\""
                .into(),
        ),
        "dict.invert" | "invert" => Some(
            "dict.invert()  — swap keys and values\n\
             Returns a new dict with keys and values swapped.\n\n\
             Example: {a: 1, b: 2}.invert()  =>  {1: a, 2: b}"
                .into(),
        ),
        "dict.length" | "dict.count" | "count" => Some(
            "dict.length()  — number of entries\n\
             Returns the number of key-value pairs.\n\n\
             Example: {a: 1, b: 2}.length()  =>  2"
                .into(),
        ),

        // Number methods
        "number.floor" | "number.ceil" | "number.round" | "number.abs"
        | "number.toInt" | "number.toFixed" | "number.toString"
        | "number.sqrt" | "number.pow" | "number.isNaN"
        | "number.isFinite" | "number.isInfinite" | "number.isInteger" => Some(
            format!(
                "{name}()  — number method (call on a number)\n\
                 Use parentheses: (42).{name}()\n\n\
                 Available: floor, ceil, round, abs, toInt, toFixed(n),\n\
                 toString, sqrt, pow(n), isNaN, isFinite, isInfinite, isInteger"
            ).into(),
        ),

        // List methods (generic fallback)
        "list.map" | "list.filter" | "list.reduce" | "list.find" | "list.find_index"
        | "list.flat_map" | "list.some" | "list.every" | "list.sort_by"
        | "list.last_index_of" | "list.keys" | "list.values" | "list.entries"
        | "list.splice" | "list.flat" | "list.compact" | "list.uniq" | "list.union"
        | "list.intersection" | "list.difference" | "list.pluck" | "list.shuffle"
        | "list.sample" | "list.take" | "list.drop" | "list.chunk" | "list.zip"
        | "list.flatten" | "list.fill" => Some(
            format!(
                "{name}()  — list method (call on a list value)\n\
                 Call it on a list: mylist.{name}(args)\n\n\
                 Example: [1, 2, 3].{name}(args)"
            ).into(),
        ),
        // Dict methods (generic fallback)
        "dict.get" | "dict.set" | "dict.has_key" | "dict.delete" | "dict.update"
        | "dict.merge" | "dict.map_values" | "dict.filter_values"
        | "dict.key_of" | "dict.invert" => Some(
            format!(
                "{name}()  — dict method (call on a dict value)\n\
                 Call it on a dict: mydict.{name}(args)\n\n\
                 Example: {{a: 1}}.{name}(args)"
            ).into(),
        ),

        // Global convenience functions
        "ord" => Some(
            "ord(s)  — Unicode codepoint of first character\n\
             Takes a single-character string, returns its integer codepoint.\n\n\
             Example: ord(\"A\")  =>  65\n\
             Example: ord(\"\\n\")  =>  10\n\
             See also: char(code) to convert back."
                .into(),
        ),
        "char" | "chr" => Some(
            "char(code)  — character from Unicode codepoint\n\
             Takes an integer codepoint, returns a single-character string.\n\n\
             Example: char(65)  =>  \"A\"\n\
             Example: char(10)  =>  \"\\n\"\n\
             See also: ord(s) to convert to codepoint."
                .into(),
        ),
        "keys" | "values" | "items" => Some(
            format!(
                "{name}()  — dict utility (standalone function)\n\
                 Extracts data from a dict argument.\n\
                 Also available as dict methods: d.{name}()\n\n\
                 Example: {name}({{a: 1, b: 2}})"
            ).into(),
        ),
        "slice" => Some(
            "slice(collection, start, end?)  — extract a sub-list or sub-string\n\
             Works on lists and strings.\n\n\
             Example: slice([1, 2, 3, 4], 1, 3)  =>  [2, 3]\n\
             Example: slice(\"hello\", 1, 4)  =>  \"ell\""
                .into(),
        ),
        "enumerate" => Some(
            "enumerate(list)  — list of [index, value] pairs\n\
             Returns a list of two-element sub-lists.\n\n\
             Example: enumerate([a, b, c])  =>  [[0, a], [1, b], [2, c]]"
                .into(),
        ),
        "json" => Some(
            "json.parse(s)    — decode JSON string to value\n\
             json.stringify(v)  — encode value to JSON string\n\
             json.pretty(v)    — encode to pretty-printed JSON\n\n\
             Example: json.parse(\"{\\\"a\\\": 1}\")  =>  {a: 1}\n\
             Example: json.stringify({a: 1})  =>  \"{\\\"a\\\":1}\""
                .into(),
        ),
        "hash" | "hashlib" => Some(
            "hashlib.md5(data)     — MD5 hex digest\n\
             hashlib.sha1(data)    — SHA-1 hex digest\n\
             hashlib.sha256(data)  — SHA-256 hex digest\n\
             hashlib.sha512(data)  — SHA-512 hex digest\n\
             hashlib.create(algo)  — create hash by name\n\
             hashlib.algorithms_available()  — list available algorithms\n\n\
             Example: hashlib.sha256(\"hello\")  =>  \"2cf24dba...\""
                .into(),
        ),
        "errors" => Some(
            "errors — Python-style error classes with inheritance and typed catch\n\n\
             Built-in: Error, TypeError, ValueError, IndexError, KeyError,\n\
             FileNotFoundError, ZeroDivisionError, etc.\n\n\
             errors.define(\"MyError\", \"Error\", \"default message\")\n\
             throw MyError(\"details\")\n\n\
             try { ... } catch MyError as e { print e }"
                .into(),
        ),

        _ => None,
    }
}

/// Help for a specific builtin, with fuzzy-match suggestions on miss.
pub fn help_builtin_or_error(name: &str) -> String {
    help_builtin(name).unwrap_or_else(|| {
        let builtins = [
            "print", "input", "exit", "len", "str", "int", "float", "bool",
            "list", "dict", "typeof", "type", "abs", "min", "max", "round",
            "trunc", "hex", "range", "sleep", "wait", "throw", "raise",
            "try", "catch", "finally", "assert", "let", "const", "global",
            "func", "def", "fn", "lambda", "return", "if", "elif", "else",
            "while", "for", "in", "break", "continue", "class", "inherit",
            "this", "new", "import", "from", "match", "when", "null",
            "true", "false", "is", "as", "help", "var", "enumerate",
            // List methods
            "map", "filter", "reduce", "sort", "reverse", "fill", "copy",
            "first", "last", "slice", "splice", "flat", "compact", "uniq",
            "union", "intersection", "difference", "shuffle", "sample",
            "take", "drop", "chunk", "zip", "flatten", "sum",
            // Dict methods
            "get", "set", "has_key", "delete", "update", "merge",
            "map_values", "filter_values", "key_of", "invert",
        ];
        let close: Vec<&str> = builtins.iter()
            .filter(|b| levenshtein(name, b) <= 2)
            .cloned()
            .collect();
        if close.is_empty() {
            format!("Unknown name: {name}\n\nRun :help for help, :help functions for a full list.")
        } else {
            format!("Unknown name: {name}\n\nDid you mean: {}?", close.join(", "))
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
        "help",
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
                for (param, _) in params {
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
            // Walk up from the executable's directory looking for a `std/`
            // directory so imports work from any working directory.
            let mut dir = exe_dir.to_path_buf();
            loop {
                let path = dir.join("std").join(name);
                if path.exists() {
                    return Some(path.to_string_lossy().into_owned());
                }
                if !dir.pop() {
                    break;
                }
            }
        }
    }
    // Check std/ relative to the project root (where Cargo.toml lives)
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let project_std = std::path::Path::new(&manifest_dir).join("std").join(name);
        if project_std.exists() {
            return Some(project_std.to_string_lossy().into_owned());
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
        Value::UdpSocket(_) => "\"<udp_socket>\"".into(),
        Value::Listener(_) => "\"<listener>\"".into(),
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
        Value::UdpSocket(_) => "\"<udp_socket>\"".into(),
        Value::Listener(_) => "\"<listener>\"".into(),
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
            return Ok(Value::Dict(Arc::new(map)));
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
        Ok(Value::Dict(Arc::new(map)))
    }

    fn array(&mut self) -> Result<Value, String> {
        self.eat('[')?;
        self.skip_ws();
        let mut items = vec![];
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(Value::List(Arc::new(items)));
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
        Ok(Value::List(Arc::new(items)))
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
            Some(&Value::List(Arc::new(vec![
                Value::Number(1.0),
                Value::Number(2.0),
                Value::Number(3.0)
            ])))
        );
        assert_eq!(
            vm.vars.get("down"),
            Some(&Value::List(Arc::new(vec![
                Value::Number(2.0),
                Value::Number(1.0),
                Value::Number(0.0)
            ])))
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
        let source = "class Person { function init(name) { self.name = name } function greet() { return \"Hi, \" + self.name } }\nclass Friendly inherit Person { function salute() { return self.greet() + \"!\" } }\nlet user = new Friendly(\"Zen\")\nlet message = user.salute()";
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
