//! `zen repl` — interactive session with persistent top-level state.
//!
//! Design: the session keeps every accepted input in `history_src`. Each new
//! fragment is appended to a copy, staged in a temp file (so the linker's
//! path-based import resolution applies unchanged), linked, and run on a fresh
//! VM. On success the fragment joins the session; on error it is discarded and
//! the session is unaffected.
//!
//! Re-running the whole session re-executes `print`s, so only the output
//! *delta* (bytes beyond the previous run's output, when it is an exact
//! prefix) is shown. The fragment's value (module return) is shown when
//! non-nil.

use rustyline::Config;
use rustyline::EditMode;
use rustyline::Editor;
use rustyline::history::DefaultHistory;
use zen_syntax::{lex, FileId, TokenKind};
use zen_vm::{RefCountGc, Vm};

use crate::cli::{display_value, link_temp_source, run_entry};

/// Run the interactive REPL. Returns when the user quits or EOFs stdin.
pub fn repl() -> Result<(), String> {
    let config = Config::builder()
        .edit_mode(EditMode::Emacs)
        .build();
    let mut rl: Editor<(), DefaultHistory> =
        Editor::with_config(config).map_err(|e| format!("repl: {e}"))?;
    let history_path = history_file();
    if let Some(ref p) = history_path {
        let _ = rl.load_history(p);
    }

    println!("zen repl — :help for commands, :quit to exit");
    let mut session = Session::default();
    let mut pending = String::new();

    loop {
        let prompt = if pending.is_empty() { "zen> " } else { "... " };
        match rl.readline(prompt) {
            Err(rustyline::error::ReadlineError::Interrupted) => {
                // Ctrl-C: drop the pending fragment, keep the session.
                pending.clear();
                println!();
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => break,
            Err(e) => return Err(format!("repl: {e}")),
            Ok(line) => {
                let trimmed = line.trim();
                if pending.is_empty() && trimmed.starts_with(':') {
                    match handle_command(trimmed, &mut session) {
                        Command::Quit => break,
                        Command::Done => continue,
                    }
                }
                if !line.trim().is_empty() {
                    let _ = rl.add_history_entry(line.as_str());
                }
                pending.push_str(&line);
                pending.push('\n');
                if !fragment_complete(&pending) {
                    continue;
                }
                let frag = std::mem::take(&mut pending);
                session.eval_fragment(&frag);
            }
        }
    }
    if let Some(ref p) = history_path {
        let _ = rl.save_history(p);
    }
    Ok(())
}

enum Command {
    Quit,
    Done,
}

fn handle_command(line: &str, session: &mut Session) -> Command {
    // `:help(topic)` / `:help topic` / `:help` — Python-style introspection.
    if line == ":help" || line.starts_with(":help ") || line.starts_with(":help(") {
        print!("{}", help_text(line, session));
        return Command::Done;
    }
    match line {
        ":quit" | ":q" | ":exit" => Command::Quit,
        ":clear" | ":reset" => {
            session.reset();
            println!("session cleared");
            Command::Done
        }
        other => {
            println!("unknown command '{other}' — :help for commands");
            Command::Done
        }
    }
}

/// Resolve `:help`, `:help topic`, and `:help(topic)` (quotes stripped).
fn help_text(line: &str, session: &mut Session) -> String {
    let topic = {
        let rest = line.strip_prefix(":help").unwrap_or("").trim();
        let rest = rest.strip_prefix('(').unwrap_or(rest);
        let rest = rest.strip_suffix(')').unwrap_or(rest);
        rest.trim().trim_matches(|c| c == '"' || c == '\'').trim().to_string()
    };
    if topic.is_empty() {
        return general_help();
    }
    if let Some(text) = zen_cli::help::module_help(&topic) {
        return text;
    }
    if let Some(text) = zen_cli::help::keyword_help(&topic) {
        return text;
    }
    if let Some(text) = zen_cli::help::type_help(&topic) {
        return text;
    }
    // Session value: probe its type and value without changing the session.
    if is_probeable(&topic) {
        match session.probe(&topic) {
            Some((type_name, value)) => {
                return format!("{topic}: {type_name} = {value}\n");
            }
            None => {}
        }
    }
    let mut out = format!("no help found for '{topic}'\n");
    let suggestions = zen_cli::help::suggest(&topic);
    if !suggestions.is_empty() {
        out.push_str(&format!("did you mean: {}\n", suggestions.join(", ")));
    }
    out.push_str("try a module name, type (int, string, list, dict), keyword, or session variable\n");
    out
}

/// True when `topic` is a single expression worth probing (usually a name).
fn is_probeable(topic: &str) -> bool {
    is_bare_expression(topic)
}

fn general_help() -> String {
    let mut out = String::from(
        "commands:\n  :help [topic]   this help, or help on a module/type/keyword/variable\n  :help(topic)    same, Python-style — e.g. :help(requests), :help(list)\n  :quit           exit the repl\n  :clear          drop all session state\nfragments run when braces balance; Ctrl-C drops a fragment.\n",
    );
    out.push_str("\nmodules:\n  ");
    out.push_str(&zen_cli::help::module_names().join(" "));
    out.push_str("\n");
    out
}

/// True when braces balance and the fragment is non-blank: ready to evaluate.
/// Brace counting uses lexer tokens so `{`/`}` inside strings don't count.
fn fragment_complete(frag: &str) -> bool {
    if frag.trim().is_empty() {
        return false;
    }
    let lx = lex(frag, FileId(0));
    if !lx.errors.is_empty() {
        return false;
    }
    let mut depth: i32 = 0;
    for t in &lx.toks {
        match t.kind {
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return true; // let the compiler report the error
        }
    }
    depth == 0
}

#[derive(Default)]
struct Session {
    /// Accepted source, re-run on every evaluation.
    accepted: String,
    /// Full captured output of the last successful run (for delta display).
    last_output: String,
}

impl Session {
    fn reset(&mut self) {
        self.accepted.clear();
        self.last_output.clear();
    }

    /// Evaluate `expr` against the current session without changing it:
    /// returns `(type_name, display_value)` on success, `None` on error.
    /// Output is discarded — the probe must be side-effect free to observe.
    fn probe(&self, expr: &str) -> Option<(String, String)> {
        let eval_src = format!("{}return (\n{}\n)\n", self.accepted, expr);
        let staged = link_temp_source(&eval_src).ok()?;
        let mut vm = Vm::new(Box::new(RefCountGc));
        staged.register_with(&mut vm);
        let (_, value) = crate::cli::run_entry(&staged, &mut vm, Some("<repl>")).ok()?;
        Some((value.type_name().to_string(), display_value(&value)))
    }

    fn eval_fragment(&mut self, frag: &str) {
        let candidate = format!("{}{}", self.accepted, frag);
        // A fragment that is a single bare expression runs wrapped in
        // `return (...)` so its value can be displayed; the session still
        // stores the original text. One execution — no double side effects.
        let show_value = is_bare_expression(frag);
        let eval_src = if show_value {
            format!("{}return (\n{}\n)\n", self.accepted, frag)
        } else {
            candidate.clone()
        };
        let staged = match link_temp_source(&eval_src) {
            Ok(program) => program,
            Err(e) => {
                println!("error: {}", clean_temp_path(&e));
                return;
            }
        };
        let mut vm = Vm::new(Box::new(RefCountGc));
        staged.register_with(&mut vm);
        match run_entry(&staged, &mut vm, Some("<repl>")) {
            Ok((output, value)) => {
                self.accepted = candidate;
                // Show only output beyond the previous run's prefix.
                let delta = output
                    .strip_prefix(self.last_output.as_str())
                    .unwrap_or(output.as_str());
                if !delta.is_empty() {
                    print!("{delta}");
                }
                self.last_output = output;
                if show_value && !value.is_nil() {
                    println!("{}", display_value(&value));
                }
            }
            Err(failure) => {
                // Show output produced before the failure, but only the new
                // delta: the session replays accepted history on a fresh VM.
                // The fragment itself stays rejected, so `last_output` is
                // intentionally left unchanged.
                let delta = failure
                    .output
                    .strip_prefix(self.last_output.as_str())
                    .unwrap_or(failure.output.as_str());
                if !delta.is_empty() {
                    print!("{delta}");
                }
                println!("error: {}", clean_temp_path(&failure.diagnostic))
            }
        }
    }
}

/// Replace the staging temp-file path in diagnostics with `<repl>`.
fn clean_temp_path(msg: &str) -> String {
    let tmp = std::env::temp_dir()
        .join(format!("zen_eval_{}.z", std::process::id()))
        .to_string_lossy()
        .into_owned();
    msg.replace(tmp.as_str(), "<repl>")
}

/// True when the fragment parses as a module with exactly one statement and
/// that statement is a bare expression.
fn is_bare_expression(frag: &str) -> bool {
    use zen_syntax::ast::Stmt;
    use zen_syntax::parser::Parser;
    let lx = lex(frag, FileId(0));
    if !lx.errors.is_empty() {
        return false;
    }
    let mut parser = Parser::new(lx);
    match parser.parse_module() {
        Ok(m) => m.items.len() == 1 && matches!(m.items[0], Stmt::Expr(_)),
        Err(_) => false,
    }
}

/// `~/.zen_history` when a home directory resolves, else in-memory only.
fn history_file() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .map(|h| std::path::PathBuf::from(h).join(".zen_history"))
}
