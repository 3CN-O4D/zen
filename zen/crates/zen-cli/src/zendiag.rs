//! Rust-style error rendering for the CLI: exact spans, stable `Z`-codes,
//! and `= help:` notes with a cause, a fix, and an example where one fits.
//!
//! The [`zen_syntax::diag`] renderer already produces the `rustc` shape
//! (`error: msg`, `--> path:line:col`, source snippet, carets, labels,
//! notes). This module converts each pipeline error — lex, parse, compile,
//! runtime — into a [`Diagnostic`] with per-kind help, so every failure the
//! user sees points at the exact offending text and suggests the repair.

use zen_compiler::CompileError;
use zen_syntax::diag::{render, ColorMode, Diagnostic, DiagnosticNote, Severity};
use zen_syntax::source::SourceMap;
use zen_syntax::span::{FileId, LineIndex, Span};
use zen_syntax::parser::ParseError;
use zen_syntax::{LexError, LexErrorKind};
use zen_vm::VmError;

/// Build the one-file source map every renderer needs. The frontend always
/// lexes with `FileId(0)`, so a fresh map per render is correct.
fn source_map(path: &str, src: &str) -> SourceMap {
    let mut sm = SourceMap::new();
    sm.add(path, src.to_string());
    let _ = FileId(0);
    sm
}

fn render_diag(path: &str, src: &str, d: &Diagnostic) -> String {
    render(d, &source_map(path, src), ColorMode::Auto)
}

// ── lex ─────────────────────────────────────────────────────────────────

/// Render the first hard lex error. Returns `None` when only warnings remain.
pub fn render_lex(path: &str, src: &str, errors: &[LexError]) -> Option<String> {
    let err = errors.iter().find(|e| e.severity == Severity::Error)?;
    let (code, message) = lex_message(&err.kind);
    let mut d = Diagnostic::new(err.severity, message, err.at);
    d.code = Some(code.into());
    for note in &err.notes {
        d.notes.push(note.clone());
    }
    if let Some(help) = lex_help(&err.kind) {
        d.notes.push(DiagnosticNote::help(help));
    }
    Some(render_diag(path, src, &d))
}

fn lex_message(kind: &LexErrorKind) -> (&'static str, String) {
    match kind {
        LexErrorKind::UnterminatedString { .. } => {
            ("Z0101", "unterminated string literal".into())
        }
        LexErrorKind::UnterminatedTripleString { .. } => {
            ("Z0102", "unterminated triple-quoted string".into())
        }
        LexErrorKind::UnterminatedBlockComment => {
            ("Z0103", "unterminated block comment".into())
        }
        LexErrorKind::UnterminatedInterpolation { expr_line, expr_col } => (
            "Z0104",
            format!("unterminated interpolation (expression started at {expr_line}:{expr_col})"),
        ),
        LexErrorKind::RawNewlineInString { .. } => {
            ("Z0105", "raw newline inside string literal".into())
        }
        LexErrorKind::UnfinishedEscape => ("Z0106", "unfinished escape sequence".into()),
        LexErrorKind::UnknownEscape { .. } => ("Z0107", "unknown escape sequence".into()),
        LexErrorKind::IncompleteHexEscape | LexErrorKind::InvalidHexEscape => {
            ("Z0108", "invalid hex escape".into())
        }
        LexErrorKind::InvalidUnicodeEscape { .. }
        | LexErrorKind::UnterminatedUnicodeEscape { .. }
        | LexErrorKind::EmptyUnicodeEscape { .. } => {
            ("Z0109", "invalid unicode escape".into())
        }
        _ => ("Z0199", "invalid token".into()),
    }
}

fn lex_help(kind: &LexErrorKind) -> Option<String> {
    match kind {
        LexErrorKind::UnterminatedString { .. } => Some(
            "close the string with a matching quote, e.g. `var s = \"hello\"`".into(),
        ),
        LexErrorKind::UnterminatedTripleString { .. } => {
            Some("close the string with `\"\"\"` on its own terms".into())
        }
        LexErrorKind::UnterminatedBlockComment => {
            Some("close the comment with `*/`".into())
        }
        LexErrorKind::UnterminatedInterpolation { .. } => {
            Some("close the interpolation with `}`, e.g. `\"x = ${x}\"`".into())
        }
        LexErrorKind::RawNewlineInString { .. } => Some(
            "use `\\n` for a newline inside a string, or a triple-quoted string for literals spanning lines".into(),
        ),
        LexErrorKind::UnfinishedEscape => {
            Some("finish the escape, e.g. `\\n`, `\\t`, `\\\\`, `\\\"`".into())
        }
        LexErrorKind::UnknownEscape { .. } => {
            Some("drop the backslash or use a known escape: `\\n \\t \\\\ \\\" \\u{...}`".into())
        }
        LexErrorKind::IncompleteHexEscape | LexErrorKind::InvalidHexEscape => {
            Some("a hex escape needs exactly two hex digits, e.g. `\\x41` for `A`".into())
        }
        LexErrorKind::InvalidUnicodeEscape { .. }
        | LexErrorKind::UnterminatedUnicodeEscape { .. }
        | LexErrorKind::EmptyUnicodeEscape { .. } => Some(
            "use `\\u{...}` with 1-6 hex digits, e.g. `\\u{1F600}`".into(),
        ),
        _ => None,
    }
}

// ── parse ───────────────────────────────────────────────────────────────

/// Render a parse error with expected-vs-found context and a repair hint.
pub fn render_parse(path: &str, src: &str, err: &ParseError) -> String {
    let (code, message, help) = match err {
        ParseError::ExpectedToken { expected, found, .. } => (
            "Z0201",
            format!("expected {expected}, found {found}"),
            expected_token_help(expected, found),
        ),
        ParseError::UnexpectedEof { .. } => (
            "Z0202",
            "unexpected end of input".to_string(),
            Some("the file ends mid-construct; check for an unclosed `{`, `(`, `[`, or string".to_string()),
        ),
        ParseError::InvalidExpr { .. } => (
            "Z0203",
            "invalid expression".to_string(),
            Some(
                "an expression was expected here, e.g. a literal, variable, or call like `add(1, 2)`"
                    .to_string(),
            ),
        ),
        ParseError::Custom { msg, .. } => ("Z0204", msg.clone(), None),
    };
    let mut d = Diagnostic::error(message, err.span());
    d.code = Some(code.into());
    if let Some(h) = help {
        d.notes.push(DiagnosticNote::help(h));
    }
    render_diag(path, src, &d)
}

/// Targeted hints for the most common token mismatches.
fn expected_token_help(expected: &str, found: &str) -> Option<String> {
    if expected.contains('}') || expected.contains("`}`") {
        Some("check that every `{` is closed; the caret marks where the block should have ended".into())
    } else if expected.contains(')') {
        Some(format!(
            "a `)` is missing — an argument list or condition near here is unbalanced (found {found})"
        ))
    } else if expected.contains(']') {
        Some("a `]` is missing — check list/index brackets near the caret".into())
    } else if expected.contains("expression") {
        Some(format!(
            "a value was expected but {found} was found; e.g. `var x = 1` not `var x =`"
        ))
    } else if expected.contains("identifier") || expected.contains("name") {
        Some("a name was expected here, e.g. `var total = 0`".into())
    } else {
        None
    }
}

// ── compile ─────────────────────────────────────────────────────────────

/// Render a compile (resolution/lowering) error with a fix suggestion.
pub fn render_compile(path: &str, src: &str, err: &CompileError) -> String {
    let (code, message, help): (&str, String, Option<String>) = match err {
        CompileError::UndefinedVariable { name, .. } => (
            "Z0301",
            format!("undefined variable `{name}`"),
            Some(format!(
                "declare it before use with `var {name} = ...`, check the spelling, or import the module that defines it"
            )),
        ),
        CompileError::RedeclaredVariable { name, .. } => (
            "Z0302",
            format!("variable `{name}` is already declared in this scope"),
            Some(format!(
                "assign without `var` to update it (`{name} = ...`), or pick a different name"
            )),
        ),
        CompileError::AssignToConst { name, .. } => (
            "Z0303",
            format!("cannot assign to constant `{name}`"),
            Some(format!(
                "declare `{name}` with `var` instead of `const` if it must change"
            )),
        ),
        CompileError::BreakOutsideLoop { .. } => (
            "Z0304",
            "break outside of loop".into(),
            Some("`break` only works inside `for`/`while`; to leave a function early, use `return`".into()),
        ),
        CompileError::ContinueOutsideLoop { .. } => (
            "Z0305",
            "continue outside of loop".into(),
            Some("`continue` only works inside `for`/`while`".into()),
        ),
        CompileError::ReturnOutsideFunction { .. } => (
            "Z0306",
            "return outside of function".into(),
            Some("top-level `return` is only valid in `zen -e` and the REPL; in a file, wrap the code in `func main()`".into()),
        ),
        CompileError::TooManyLocals { .. } => (
            "Z0307",
            "too many local variables in one function".into(),
            Some("split the function into smaller helpers".into()),
        ),
        CompileError::TooManyRegisters { .. } => (
            "Z0308",
            "expression too complex for one function frame".into(),
            Some("break the expression into temporaries, e.g. `var t = a + b` then use `t`".into()),
        ),
        CompileError::Unsupported { what, .. } => (
            "Z0309",
            format!("{what} is not supported by this compiler yet"),
            None,
        ),
        CompileError::TooManyConstants { .. } => (
            "Z0310",
            "too many constants in one module".into(),
            Some("move large literal tables into a data file and load them at runtime".into()),
        ),
        CompileError::TooManyUpvalues { .. } => (
            "Z0311",
            "closure captures too many variables".into(),
            Some("capture a single dict holding the values instead".into()),
        ),
        CompileError::JumpOffsetOverflow { .. } => (
            "Z0312",
            "function body too large to compile".into(),
            Some("split the function into smaller helpers".into()),
        ),
        CompileError::InvalidMatchPattern { .. } => (
            "Z0313",
            "invalid match arm pattern".into(),
            Some("patterns are literals, names, `_`, or destructuring shapes — e.g. `1 | 2 => ...`".into()),
        ),
        CompileError::OrPatternBindingMismatch { .. } => (
            "Z0314",
            "or-pattern arms bind different names".into(),
            Some("every alternative must bind the same names, e.g. `(a, _) | (_, a)`".into()),
        ),
        CompileError::RestPatternNotLast { .. } => (
            "Z0315",
            "rest pattern must come last".into(),
            Some("move `..rest` to the end, e.g. `[first, ..rest]`".into()),
        ),
        CompileError::MultipleRestPatterns { .. } => (
            "Z0316",
            "only one rest pattern is allowed".into(),
            Some("merge the captures into a single `..rest`".into()),
        ),
        CompileError::DuplicateDictKey { .. } => (
            "Z0317",
            "duplicate dict key".into(),
            Some("keep one entry per key; later entries would shadow earlier ones".into()),
        ),
        CompileError::InvalidComprehension { .. } => (
            "Z0318",
            "invalid comprehension clause".into(),
            Some("use `for x in xs` with an optional `if` guard, e.g. `[x * 2 for x in xs if x > 0]`".into()),
        ),
        CompileError::SuperOutsideClass { .. } => (
            "Z0319",
            "super call outside of class".into(),
            Some("`super(...)` only works inside a class method".into()),
        ),
        CompileError::SuperMethodOutsideClass { .. } => (
            "Z0320",
            "super method call outside of class".into(),
            Some("`super.method(...)` only works inside a class method".into()),
        ),
        CompileError::UnsupportedDestructuring { .. } => (
            "Z0321",
            "destructuring is not supported here".into(),
            Some("bind the whole value, then index into it, e.g. `var p = pt; var x = p[0]`".into()),
        ),
        CompileError::Custom { msg, .. } => ("Z0322", msg.clone(), None),
    };
    let mut d = Diagnostic::error(message, err.span());
    d.code = Some(code.into());
    if let Some(h) = help {
        d.notes.push(DiagnosticNote::help(h));
    }
    render_diag(path, src, &d)
}

// ── runtime ─────────────────────────────────────────────────────────────

/// Render a runtime error at `line` (1-based) of `src` with the offending
/// function named. The caret targets the exact token when the message names
/// one (e.g. the missing key/variable); otherwise it covers the line.
pub fn render_runtime(
    path: &str,
    src: &str,
    func: &str,
    line: u32,
    err: &VmError,
) -> String {
    let (code, message, help): (&str, String, Option<String>) = match err {
        VmError::TypeError(_) => (
            "Z0401",
            err.to_string(),
            Some(
                "check operand types with `typeof(x)` and convert explicitly, e.g. `var n = \"42\".to_int()`"
                    .into(),
            ),
        ),
        VmError::IndexError { index, len } => (
            "Z0402",
            format!("index {index} out of bounds for length {len}"),
            Some(format!(
                "valid indices are 0..{}; check `.len()` first, e.g. `if i < xs.len()`",
                len.saturating_sub(1)
            )),
        ),
        VmError::KeyError(_) => (
            "Z0403",
            err.to_string(),
            Some(
                "test membership first (`if k in d`), or use a default: `d.get(k, fallback)`"
                    .into(),
            ),
        ),
        VmError::NameError(_) => (
            "Z0404",
            err.to_string(),
            Some(
                "declare the name before use (`var name = ...`), check the spelling, and make sure its module is imported"
                    .into(),
            ),
        ),
        VmError::ValueError(_) => (
            "Z0405",
            err.to_string(),
            Some(
                "the value has the right type but is not acceptable here; check ranges, formats, and empty inputs"
                    .into(),
            ),
        ),
        VmError::DivideByZero(_) => (
            "Z0406",
            err.to_string(),
            Some("guard the denominator, e.g. `if d != 0 { x / d } else { 0 }`".into()),
        ),
        VmError::StackOverflow(_) => (
            "Z0407",
            err.to_string(),
            Some("check the recursion base case — the function never stopped calling itself".into()),
        ),
        VmError::ArgumentCount { expected, got } => (
            "Z0408",
            format!("expected {expected} argument(s), got {got}"),
            Some("match the call against the function's parameter list".into()),
        ),
        VmError::HandleError(_) => (
            "Z0409",
            err.to_string(),
            Some("the handle may be closed or from a failed open; check the resource lifecycle".into()),
        ),
        VmError::IOMismatch(_) => (
            "Z0410",
            err.to_string(),
            Some("the read/written shape did not match; check sizes and modes".into()),
        ),
        VmError::NativeError(_) => (
            "Z0411",
            err.to_string(),
            Some(
                "raised by a native module; check its arguments against `:help(module)`".into(),
            ),
        ),
        VmError::UserError(_) => ("Z0412", err.to_string(), None),
        VmError::Panic(_) => (
            "Z0413",
            err.to_string(),
            Some("a native routine panicked — report it with a minimal reproducer".into()),
        ),
        VmError::PyImport { message, .. } => (
            "Z0414",
            format!("pymport failed: {message}"),
            Some(
                "check the module name, add third-party packages to requirements.txt and run `zen py-sync`, and make project files visible (the entry dir is on sys.path automatically)".into(),
            ),
        ),
        VmError::ForeignAttribute { message, .. } => (
            "Z0415",
            format!("foreign attribute failed: {message}"),
            Some(
                "check the member name against the Python object (versions drift); data attributes read as values, only callables can be called".into(),
            ),
        ),
        VmError::ForeignCall { message, .. } => (
            "Z0416",
            format!("foreign call failed: {message}"),
            Some(
                "match the Python signature (counts and types); calls are positional-only".into(),
            ),
        ),
        VmError::Marshal { message } => (
            "Z0417",
            format!("cannot cross the Python boundary: {message}"),
            Some(
                "only nil/bool/int/float/string/bytes and flat lists/dicts of those cross by value; everything else must travel as an opaque handle".into(),
            ),
        ),
    };
    let span = runtime_span(src, line, &err.to_string());
    // Chained Python tracebacks render as their own note block so the Zen
    // frame stays primary and the foreign frames stay readable. Read before
    // `message` moves into the diagnostic below.
    let python_traceback: Option<String> = match err {
        VmError::PyImport { traceback, .. }
        | VmError::ForeignAttribute { traceback, .. }
        | VmError::ForeignCall { traceback, .. } => traceback.clone(),
        _ => None,
    };
    let mut d = Diagnostic::error(message, span);
    d.code = Some(code.into());
    if func != "<module>" {
        d.notes.push(DiagnosticNote::note(format!("in function `{func}`")));
    }
    if let Some(tb) = python_traceback {
        d.notes.push(DiagnosticNote::note(tb));
    }
    if let Some(h) = help {
        d.notes.push(DiagnosticNote::help(h));
    }
    render_diag(path, src, &d)
}

/// Best-effort exact span for a runtime failure: the quoted name from the
/// message when it occurs on the failing line, else the trimmed line.
fn runtime_span(src: &str, line: u32, message: &str) -> Span {
    let index = LineIndex::build(src);
    let file = FileId(0);
    let (start, _) = match index.line_range(line) {
        Some(r) => r,
        None => return Span::new(file, 0, 0),
    };
    let text = index.line_text(line).unwrap_or("");
    let trim_start = text.len() - text.trim_start().len();
    let trim_end = text.trim_end().len();
    // Exact caret: a `'name'` or `` `name` `` from the message that appears
    // verbatim on this line (the missing key, variable, or field).
    if let Some(name) = quoted_name(message) {
        if let Some(rel) = text.find(name.as_str()) {
            let lo = start + rel as u32;
            return Span::new(file, lo, lo + name.len() as u32);
        }
    }
    Span::new(file, start + trim_start as u32, start + trim_end as u32)
}

/// First `'...'` or `` `...` `` payload in a message, if any.
fn quoted_name(message: &str) -> Option<String> {
    for (open, close) in [('\'', '\''), ('`', '`')] {
        let mut rest = message;
        while let Some(i) = rest.find(open) {
            let after = &rest[i + open.len_utf8()..];
            if let Some(j) = after.find(close) {
                let name = &after[..j];
                if !name.is_empty() && name.len() < 64 {
                    return Some(name.to_string());
                }
                rest = &after[j + 1..];
            } else {
                break;
            }
        }
    }
    None
}
