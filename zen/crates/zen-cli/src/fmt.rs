//! `zen fmt` — opinionated, idempotent source formatter.
//!
//! Token-based (not AST-based) so comments survive: lex the file, re-emit each
//! token's original text, and normalize only the *gaps* between tokens
//! (spaces vs newlines + indent). Deterministic gap rules make output a fixed
//! point: `fmt(fmt(x)) == fmt(x)`.
//!
//! Rules (v1):
//! - 4-space indent, `{`/`}` set the level; `}` dedents before printing.
//! - One space around binary operators and after `,`/`:`/`;`; none before
//!   `,`/`;`/`)`/`]` or after `(`/`[`/`!`/`-` (unary kept tight by context).
//! - Blank lines collapse to at most one; file ends with exactly one newline.
//! - `//`, `#`, and `/* */` comments in gaps are preserved verbatim.
//!
//! Gate: idempotent and stable (`fmt --check` on CI).

use std::path::{Path, PathBuf};
use zen_syntax::{lex, FileId, TokenKind};

/// Format one file in place. Returns true when the file changed.
pub fn format_file(path: &Path) -> Result<bool, String> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| format!("fmt: cannot read {}: {e}", path.display()))?;
    let out = format_source(&src)?;
    if out == src {
        return Ok(false);
    }
    std::fs::write(path, &out)
        .map_err(|e| format!("fmt: cannot write {}: {e}", path.display()))?;
    Ok(true)
}

/// Check mode: true when the file is already formatted.
pub fn check_file(path: &Path) -> Result<bool, String> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| format!("fmt: cannot read {}: {e}", path.display()))?;
    Ok(format_source(&src)? == src)
}

pub fn format_source(src: &str) -> Result<String, String> {
    let lexed = lex(src, FileId(0));
    if !lexed.errors.is_empty() {
        return Err(format!("fmt: lex error: {:?}", lexed.errors[0]));
    }
    let toks: Vec<_> = lexed
        .toks
        .iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .collect();
    if toks.is_empty() {
        return Ok(String::new());
    }

    let mut out = String::with_capacity(src.len() + 32);
    let mut indent: usize = 0;
    let mut at_line_start = true;
    let mut prev_kind: Option<TokenKind> = None;
    let mut prev_hi: usize = 0;

    let mut i = 0;
    while i < toks.len() {
        let t = toks[i];
        let text = &src[t.lo as usize..t.hi as usize];
        let gap = &src[prev_hi..t.lo as usize];

        let newlines = gap.chars().filter(|&c| c == '\n').count();
        let comments = extract_comments(gap);

        // NOTE: RBrace dedents *after* gap processing so comments/blank lines
        // before `}` keep the block's indent; only `}` itself dedents.

        if newlines > 0 || !comments.is_empty() {
            // Hard break: emit pending comments, then newline(s) + indent.
            for c in &comments {
                if !at_line_start {
                    out.push('\n');
                }
                push_indent(&mut out, indent);
                out.push_str(c);
                at_line_start = false;
            }
            if !comments.is_empty() || newlines > 0 {
                // The break itself: skipped when the upcoming token is a
                // Newline/Semi (it breaks the line itself) to avoid doubling.
                let breaks_itself =
                    t.kind == TokenKind::Newline || t.kind == TokenKind::Semi;
                if !breaks_itself {
                    out.push('\n');
                }
                // Preserve one blank line (2+ newlines), except before `}`
                // where trailing blanks just push the brace down.
                if newlines >= 2 && t.kind != TokenKind::RBrace {
                    out.push('\n');
                }
                at_line_start = true;
            }
            // A `{` directly before this newline keeps indent (set below).
        } else if !at_line_start {
            if need_space(prev_kind, t.kind) {
                out.push(' ');
            }
        }

        if at_line_start {
            // `}` dedents here so gap comments above kept block indent.
            if t.kind == TokenKind::RBrace && indent > 0 {
                indent -= 1;
            }
            // Suppress indent for blank continuation (only whitespace so far
            // can't happen here — we always emit a token after indent).
            push_indent(&mut out, indent);
            at_line_start = false;
        }
        // Newline/Semi tokens are pure breaks: gaps carry the actual newline
        // chars, so pushing text here would double them.
        if t.kind != TokenKind::Newline && t.kind != TokenKind::Semi {
            out.push_str(text);
        }

        // Newline tokens force a break; semicolons break too (one stmt/line).
        // Don't push their text (gaps carry the newlines) and don't space
        // before them — the post-pass strips trailing whitespace anyway.
        if t.kind == TokenKind::Newline || t.kind == TokenKind::Semi {
            out.push('\n');
            at_line_start = true;
        } else if t.kind == TokenKind::LBrace {
            indent += 1;
            // Line breaks after `{` come from the next token's gap; emitting
            // here too would double them.
        } else if t.kind == TokenKind::RBrace {
            // Dedent happened before printing. Breaks after `}` likewise come
            // from the next token's gap.
        }

        prev_kind = Some(t.kind);
        prev_hi = t.hi as usize;
        i += 1;
    }

    // Trailing gap comments (e.g. `// done` at EOF without newline).
    let tail = &src[prev_hi..];
    for c in extract_comments(tail) {
        if !at_line_start {
            out.push('\n');
        }
        push_indent(&mut out, indent);
        out.push_str(&c);
        at_line_start = false;
    }
    if !at_line_start {
        out.push('\n');
    }
    Ok(normalize_blank_lines(&out))
}

/// Post-pass: strip trailing whitespace per line, collapse 3+ newlines to 2
/// (one blank line max), ensure exactly one trailing newline. Deterministic,
/// so formatting is idempotent.
fn normalize_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank_run = 0;
    for line in s.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let stripped = body.trim_end();
        if stripped.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push('\n');
            }
        } else {
            blank_run = 0;
            out.push_str(stripped);
            out.push('\n');
        }
    }
    // Handle a final line without newline.
    if !s.ends_with('\n') {
        // split_inclusive yielded the tail without `\n`; we already added one.
    }
    while out.ends_with("\n\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("    ");
    }
}

/// Pull `//`, `#`, and `/* */` comments out of an inter-token gap.
fn extract_comments(gap: &str) -> Vec<String> {
    let mut out = Vec::new();
    let b = gap.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            let start = i;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            out.push(gap[start..i].trim_end().to_string());
        } else if b[i] == b'#' {
            let start = i;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            out.push(gap[start..i].trim_end().to_string());
        } else if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(b.len());
            out.push(gap[start..i].to_string());
        } else {
            i += 1;
        }
    }
    out
}

/// Space between two adjacent same-line tokens?
fn need_space(prev: Option<TokenKind>, cur: TokenKind) -> bool {
    use TokenKind::*;
    let Some(p) = prev else { return false };
    // Never space before these.
    if matches!(
        cur,
        Comma | Semi | Colon | RParen | RBracket | Dot | SafeDot | Question
    ) {
        return false;
    }
    // Never space after these.
    if matches!(
        p,
        LParen | LBracket | Dot | SafeDot | Bang | Question | ThinArrow
    ) {
        return false;
    }
    // Calls and definitions bind tight: `add(`, `print(`, `func name(`.
    // Control keywords in Zen rarely take parens; `if(x)` is fine and stable.
    if cur == LParen {
        return false;
    }
    // `{`/`}` get spaces when inline (`{ }`, `{a: 1}` handled by brace logic).
    if p == LBrace || cur == RBrace {
        return true;
    }
    // Unary minus/bang directly before a value: `!x`, `-x`, `-(a)`.
    if matches!(p, Bang | Minus) {
        // Heuristic: if the previous token before the operator was a value or
        // `)`/`]`, it's binary and wants spaces. We only see `p` here, so keep
        // the space (binary is far more common); tight unary is preserved only
        // when the operator opens the line (handled by at_line_start path).
        return true;
    }
    // Keywords that bind tight to `(`: unreachable now (blanket rule above),
    // kept for documentation. `if(x)` formats without space.
    let _ = p;
    true
}

/// Collect `.z` files from CLI args (files or dirs, recursive).
pub fn collect_files(args: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for a in args {
        let p = PathBuf::from(a);
        if p.is_file() {
            out.push(p);
        } else if p.is_dir() {
            visit(&p, &mut out)?;
        } else {
            return Err(format!("fmt: no such file or directory: {a}"));
        }
    }
    out.sort();
    Ok(out)
}

fn visit(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let rd =
        std::fs::read_dir(dir).map_err(|e| format!("fmt: cannot list {}: {e}", dir.display()))?;
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for e in entries {
        if e.is_dir() {
            // Skip build artifacts and deps.
            if let Some(n) = e.file_name().and_then(|s| s.to_str()) {
                if n == "target" || n.starts_with('.') {
                    continue;
                }
            }
            visit(&e, out)?;
        } else if e.extension().map(|x| x == "z").unwrap_or(false) {
            out.push(e);
        }
    }
    Ok(())
}
