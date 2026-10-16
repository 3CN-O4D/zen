//! Diagnostics: severities, notes, and the terminal renderer.
//!
//! See `docs/05-lexer.md` §9.4. The output shape, the `= help:` / `= note:`
//! house style, and the caret rules are all preserved from the legacy runtime so
//! that existing documentation and user expectations carry over unchanged.

use std::fmt;

use crate::source::SourceMap;
use crate::span::LineIndex;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A secondary labelled range in a diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub span: crate::span::Span,
    pub text: String,
}

impl Label {
    pub fn new(span: crate::span::Span, text: impl Into<String>) -> Self {
        Self {
            span,
            text: text.into(),
        }
    }
}

/// The `= help:` / `= note:` lines under a diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteLabel {
    Help,
    Note,
}

impl NoteLabel {
    pub const fn as_str(self) -> &'static str {
        match self {
            NoteLabel::Help => "help",
            NoteLabel::Note => "note",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticNote {
    pub label: NoteLabel,
    pub text: String,
}

impl DiagnosticNote {
    pub fn help(text: impl Into<String>) -> Self {
        Self {
            label: NoteLabel::Help,
            text: text.into(),
        }
    }

    pub fn note(text: impl Into<String>) -> Self {
        Self {
            label: NoteLabel::Note,
            text: text.into(),
        }
    }
}

impl fmt::Display for DiagnosticNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "= {}: {}", self.label.as_str(), self.text)
    }
}

/// When to emit ANSI colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    Always,
    /// Colour only when stderr is a terminal and `NO_COLOR` is unset.
    Auto,
    Never,
}

impl ColorMode {
    fn resolve(self) -> bool {
        match self {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => std::env::var_os("NO_COLOR").is_none() && is_stderr_tty(),
        }
    }
}

fn is_stderr_tty() -> bool {
    // Avoid a libc dependency: `IsTerminal` is in std since 1.70.
    use std::io::IsTerminal;
    std::io::stderr().is_terminal()
}

/// A rendered, human-facing diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Optional stable code such as `E0101`; omitted from output when `None`.
    pub code: Option<String>,
    pub message: String,
    /// Where the caret goes. Usually the offending token.
    pub primary: crate::span::Span,
    pub labels: Vec<Label>,
    pub notes: Vec<DiagnosticNote>,
}

impl Diagnostic {
    pub fn new(severity: Severity, message: impl Into<String>, primary: crate::span::Span) -> Self {
        Self {
            severity,
            code: None,
            message: message.into(),
            primary,
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn error(message: impl Into<String>, primary: crate::span::Span) -> Self {
        Self::new(Severity::Error, message, primary)
    }

    pub fn warning(message: impl Into<String>, primary: crate::span::Span) -> Self {
        Self::new(Severity::Warning, message, primary)
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_note(mut self, note: DiagnosticNote) -> Self {
        self.notes.push(note);
        self
    }

    pub fn with_notes(mut self, notes: impl IntoIterator<Item = DiagnosticNote>) -> Self {
        self.notes.extend(notes);
        self
    }

    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }
}

/// Tab stops are expanded to the next multiple of this, so a caret under a
/// tab-indented line stays aligned.
const TAB_WIDTH: usize = 4;

/// A diagnostic is never longer than this; longer context is truncated.
const MAX_LINES: usize = 12;

fn expand_tabs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch == '\t' {
            let pad = TAB_WIDTH - (out.chars().count() % TAB_WIDTH);
            for _ in 0..pad {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Render `d` in the legacy `rustc`-shaped form.
pub fn render(d: &Diagnostic, files: &SourceMap, color: ColorMode) -> String {
    let paint = color.resolve();
    let bold = |s: &str| {
        if paint {
            format!("\x1b[1m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let red = |s: &str| {
        if paint {
            format!("\x1b[31m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let yellow = |s: &str| {
        if paint {
            format!("\x1b[33m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let blue = |s: &str| {
        if paint {
            format!("\x1b[34m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };
    let cyan = |s: &str| {
        if paint {
            format!("\x1b[36m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    };

    let mut out = String::new();

    // ── headline ──────────────────────────────────────────────────────────
    let sev = match d.severity {
        Severity::Error => red("error"),
        Severity::Warning => yellow("warning"),
    };
    match &d.code {
        Some(code) => out.push_str(&format!("{}: {}[{}]\n", sev, bold(&d.message), blue(code))),
        None => out.push_str(&format!("{}: {}\n", sev, bold(&d.message))),
    }

    let file = match files.get(d.primary.file) {
        Some(file) => file,
        None => {
            // A span into a file the map does not know: say so rather than pretend.
            out.push_str(&format!("  --> {}:{}\n", d.primary.file, d.primary.lo));
            for note in &d.notes {
                let marker = note_marker(note.label, &blue, &cyan);
                out.push_str(&format!("  = {}: {}\n", marker, note.text));
            }
            return out;
        }
    };

    let index = LineIndex::build(&file.text);
    let (line, col) = index.line_col(d.primary.lo);
    out.push_str(&format!("  --> {}:{}:{}\n", file.path.display(), line, col));

    let width = usize::try_from(line).unwrap_or(0).to_string().len().max(1);
    let gutter = " ".repeat(width);

    // ── the offending line and its caret run ──────────────────────────────
    if let Some(text) = index.line_text(line) {
        let shown = expand_tabs(text);
        out.push_str(&format!("{gutter} |\n"));
        out.push_str(&format!("{:>width$} | {}\n", line, shown, width = width));

        // Caret run: one `^` per character of the span, minimum one.
        let start_col = index.col_of(d.primary.lo);
        let end_col = index.col_of(d.primary.hi.max(d.primary.lo));
        let span_cols = end_col.saturating_sub(start_col).max(1);
        let caret_pad = expand_tabs(
            &text
                .chars()
                .take(col_prefix_chars(text, start_col))
                .collect::<String>(),
        );
        let lead = caret_pad.chars().count();
        let run: String =
            std::iter::repeat_n('^', usize::try_from(span_cols).unwrap_or(1)).collect();
        out.push_str(&format!(
            "{:>width$} | {}{}\n",
            "",
            " ".repeat(lead),
            red(&run),
            width = width
        ));
    }

    // ── labels and notes ──────────────────────────────────────────────────
    if !d.labels.is_empty() {
        out.push_str(&format!("{:>width$} |\n", "", width = width));
        for label in &d.labels {
            let (ll, _) = index.line_col(label.span.lo);
            out.push_str(&format!(
                "{:>width$} | {}: {}\n",
                "",
                ll,
                label.text,
                width = width
            ));
        }
    }

    if !d.notes.is_empty() {
        out.push_str(&format!("{:>width$} |\n", "", width = width));
        for note in &d.notes {
            let marker = note_marker(note.label, &blue, &cyan);
            out.push_str(&format!(
                "{:>width$} = {}: {}\n",
                "",
                marker,
                note.text,
                width = width
            ));
        }
    }

    // Every coded error points at its long-form explanation, rustc-style.
    // (Warnings and codeless diagnostics stay quiet.)
    if d.severity == Severity::Error {
        if let Some(code) = &d.code {
            out.push_str(&format!(
                "{:>width$} = note: run `zen explain {code}` for detail\n",
                "",
                width = width
            ));
        }
    }

    // ── truncation ────────────────────────────────────────────────────────
    let count = out.lines().count();
    if count > MAX_LINES {
        let kept: Vec<&str> = out.lines().take(MAX_LINES.saturating_sub(1)).collect();
        let dropped = count.saturating_sub(MAX_LINES.saturating_sub(1));
        let mut trimmed = kept.join("\n");
        trimmed.push('\n');
        trimmed.push_str(&format!("... and {dropped} more lines\n"));
        out = trimmed;
    }

    out
}

/// How many characters precede column `col` (1-based) on the line.
fn col_prefix_chars(text: &str, col: u32) -> usize {
    let want = usize::try_from(col.saturating_sub(1)).unwrap_or(0);
    text.chars().take(want).count()
}

fn note_marker<F, G>(label: NoteLabel, blue: &F, cyan: &G) -> String
where
    F: Fn(&str) -> String,
    G: Fn(&str) -> String,
{
    match label {
        NoteLabel::Help => blue("help"),
        NoteLabel::Note => cyan("note"),
    }
}

impl fmt::Display for Diagnostic {
    /// Display without a source map prints the bare message, which is what
    /// `--message-format=short` wants and what unit tests compare against.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.code {
            Some(code) => write!(f, "{}: {}[{}]", self.severity, self.message, code),
            None => write!(f, "{}: {}", self.severity, self.message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::{FileId, Span};

    fn sm() -> (SourceMap, FileId) {
        let mut sm = SourceMap::new();
        let id = sm.add("app.z", "let s = \"oops\nprint(s)\n".to_string());
        (sm, id)
    }

    #[test]
    fn renders_message_location_and_caret() {
        let (sm, id) = sm();
        let d = Diagnostic::error("unterminated string literal", Span::new(id, 8, 13));
        let out = render(&d, &sm, ColorMode::Never);
        assert!(out.contains("error: unterminated string literal"), "{out}");
        assert!(out.contains("app.z:1:9"), "{out}");
        assert!(out.contains("let s = \"oops"), "{out}");
        assert!(out.contains("^^^^^"), "{out}");
        assert!(!out.contains('\x1b'), "colour leaked in Never mode");
    }

    #[test]
    fn always_mode_emits_ansi() {
        let (sm, id) = sm();
        let d = Diagnostic::error("boom", Span::new(id, 0, 1));
        assert!(render(&d, &sm, ColorMode::Always).contains('\x1b'));
    }

    #[test]
    fn tab_indentation_keeps_carets_aligned() {
        let mut sm = SourceMap::new();
        let id = sm.add("t.z", "\t\tabc".to_string());
        let d = Diagnostic::error("bad", Span::new(id, 2, 5));
        let out = render(&d, &sm, ColorMode::Never);
        let caret_line = out.lines().find(|l| l.contains('^')).unwrap_or("");
        let source_line = out.lines().find(|l| l.contains("abc")).unwrap_or("");
        let caret_at = caret_line.find('^').unwrap_or(0);
        let source_at = source_line.find("abc").unwrap_or(0);
        assert_eq!(caret_at, source_at, "{out}");
    }

    #[test]
    fn output_is_capped() {
        let (sm, id) = sm();
        let mut d = Diagnostic::error("boom", Span::new(id, 0, 1));
        for n in 0..40 {
            d.notes.push(DiagnosticNote::note(format!("note {n}")));
        }
        let out = render(&d, &sm, ColorMode::Never);
        assert!(out.lines().count() <= MAX_LINES, "{}", out.lines().count());
        assert!(out.contains("more lines"), "{out}");
    }

    #[test]
    fn unknown_file_does_not_panic() {
        let sm = SourceMap::new();
        let d = Diagnostic::error("boom", Span::new(FileId(7), 0, 1));
        let out = render(&d, &sm, ColorMode::Never);
        assert!(out.contains("file#7"), "{out}");
    }
}
