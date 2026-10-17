//! CLI argument parsing and command dispatch.

use zen_vm::{RefCountGc, Value, Vm};

use zen_cli::linker::{self, LinkedProgram};

pub fn run() -> Result<(), String> {
    // A `zen build` artifact is the interpreter with an embedded payload; if
    // this executable carries one, run it instead of parsing the command line.
    if let Some(prog) = crate::build::embedded_program() {
        return crate::build::run_embedded(&prog);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("zen 3.0.0 (native Rust runtime)");
        eprintln!("Usage: zen <command> [args]");
        eprintln!("       zenn <command> [args]");
        return Ok(());
    }
    match args[0].as_str() {
        "run" => run_command(&args[1..]),
        "-m" => {
            if args.len() < 2 {
                return Err(format!("Z0501: usage: zen -m <module> [args...]"));
            }
            run_module_as_main(&args[1], &args[2..])
        }
        "-e" => {
            if args.len() < 2 {
                return Err(format!("Z0501: usage: zen -e <code>"));
            }
            run_source(&args[1])
        }
        "check" => check_command(&args[1..]),
        "lint" => lint_command(&args[1..]),
        "py-sync" => py_sync_command(&args[1..]),
        "explain" => explain_command(&args[1..]),
        "--explain" => explain_command(&args[1..]),
        "repl" => crate::repl::repl(),
        "build" => build_command(&args[1..]),
        "test" => test_command(&args[1..]),
        "fmt" => fmt_command(&args[1..]),
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        "-v" | "--version" => {
            println!("zen 3.0.0");
            Ok(())
        }
        _ => Err(format!("Z0502: unknown command: {}", args[0])),
    }
}

fn print_help() {
    eprintln!("zen 3.0.0 (native Rust runtime)");
    eprintln!("Usage: zen <command> [args]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  run <file>      Execute a Zen source file");
    eprintln!("  -m <module>     Run a module as __main__ (calls its main(argv))");
    eprintln!("  -e <code>       Execute inline code");
eprintln!("  check <file>    Type-check / compile a file without running");
  eprintln!("  lint <file>     Warn on suspicious patterns (unused, dead code)");
  eprintln!("  explain <code>  Explain an error code (e.g. `zen explain Z0308`)");
  eprintln!("  repl            Start interactive REPL");
    eprintln!("  build <file>    Bundle a program into a standalone executable");
    eprintln!("  test            Run tests");
    eprintln!("  fmt             Format source files");
    eprintln!("  help            Show this help");
    eprintln!("  --version       Show version");
}

/// Execute a module as `__main__` (`zen -m <module> [args]`): import it and
/// call its `main(argv)` with the remaining arguments as a list of strings.
fn run_module_as_main(module: &str, argv: &[String]) -> Result<(), String> {
    if !module
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        return Err(format!("-m: invalid module name '{module}'"));
    }
    let arg_lits: Vec<String> = argv.iter().map(|a| zen_string_literal(a)).collect();
    let source = format!(
        "import {module} as __zen_main\n__zen_main.main([{}])\n",
        arg_lits.join(", ")
    );
    let path = std::env::temp_dir().join(format!("zen_m_{}.z", std::process::id()));
    std::fs::write(&path, source).map_err(|e| format!("-m: {e}"))?;
    let result = run_command(&[path.to_string_lossy().into_owned()]);
    let _ = std::fs::remove_file(&path);
    result
}

/// Escape a host string into a double-quoted Zen string literal.
fn zen_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Format a top-level error for display. Rustc-style diagnostics (ANSI
/// escape prefix) print verbatim; Z-coded errors print as
/// `error[Z####]: message`; everything else gets the `zen:` prefix.
/// Callers pass raw error strings (possibly `Z####: message` prefixed);
/// this is applied exactly once in [`main`](crate::main::main).
pub fn format_error(e: &str) -> String {
    if e.starts_with("\x1b[1;31merror") {
        e.to_string()
    } else if let Some(rest) = e.strip_prefix('Z') {
        // Z-coded error: `Z0324: module 'foo' not found` →
        // `error[Z0324]: module 'foo' not found`
        let code_end = rest.find(':').map_or(rest.len(), |i| i);
        let code = &rest[..code_end];
        let message = rest[code_end..].trim_start_matches(':').trim_start();
        format!("error[Z{code}]: {message}")
    } else {
        format!("zen: {e}")
    }
}

fn run_command(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err(format!("Z0501: usage: zen run <file>"));
    }
    let linked = linker::link(&args[0])?;
    ensure_python_deps(&args[0], &linked)?;
    run_linked(linked)
}

/// Scan ALL linked sources (entry plus transitive `.z` imports) for
/// `pymport` statements and satisfy the Python bootstrap plan. A helper that
/// itself pymports is covered exactly like top-level code — scanning only
/// the entry file would miss it and skip installs it needs.
fn ensure_python_deps(entry: &str, linked: &linker::LinkedProgram) -> Result<(), String> {
    let mut specs = Vec::new();
    for (path, source) in linked.sources.values() {
        match crate::pydeps::scan_pymports(source) {
            Ok(mut s) => specs.append(&mut s),
            Err(e) => eprintln!(
                "zen: warning: pymport scan of {path} failed ({e}); bootstrap may miss its modules"
            ),
        }
    }
    if specs.is_empty() {
        return Ok(());
    }
    let entry_path = std::path::Path::new(entry);
    let plan = crate::pydeps::plan_bootstrap(entry_path, &specs);
    if let Err(e) = crate::pydeps::ensure_bootstrap(&plan) {
        eprintln!("zen: Python bootstrap failed: {e} (continuing; pymport will error at runtime)");
    }
    crate::pydeps::export_python_paths(entry_path);
    Ok(())
}

/// `zen explain <code>` (also `zen --explain <code>`): print the
/// long-form explanation for an error code like `Z0308`. Accepts `Z0308`,
/// `z0308`, and bare `0308`; Rust-style `E0308` redirects to the Zen twin.
fn explain_command(args: &[String]) -> Result<(), String> {
    use crate::explain::{band_members, lookup, normalize, rust_redirect, EXPLANATIONS};
    let arg = args.first().map(String::as_str).unwrap_or("");
    // No code at all: show the full registry instead of a usage error — the
    // most useful default for a discovery-oriented command.
    if arg.is_empty() {
        return explain_command(&["--list".to_string()]);
    }
    if arg == "--list" || arg == "list" {
        println!("Zen error codes (see `zen explain <code>` for detail):\n");
        let mut band = String::new();
        for e in EXPLANATIONS {
            let b = &e.code[..3];
            if b != band {
                band = b.to_string();
                let name = match b {
                    "Z01" => "lexer",
                    "Z02" => "parser",
                    "Z03" => "compile time",
                    "Z04" => "runtime",
                    _ => "other",
                };
                println!("{b}xx — {name}");
            }
            println!("  {code:<6} {title}", code = e.code, title = e.title);
        }
        return Ok(());
    }
    let Some(canonical) = normalize(arg) else {
        if let Some(twin) = rust_redirect(arg) {
            if lookup(&twin).is_some() {
                eprintln!("Zen codes start with Z, not E — did you mean `{twin}`?\n");
                return explain_command(&[twin]);
            }
        }
        return Err(format!(
            "usage: zen explain <code> (e.g. `zen explain Z0308`); `zen explain --list` shows all codes"
        ));
    };    match lookup(&canonical) {
        Some(e) => {
            println!("error[{code}]: {title}\n", code = e.code, title = e.title);
            println!("{}", e.body);
            Ok(())
        }
        None => {
            let band = &canonical[..3];
            let mut msg = format!("unknown error code `{canonical}`.");
            let similar = band_members(band);
            if !similar.is_empty() {
                let names: Vec<&str> = similar.iter().map(|e| e.code).collect();
                msg.push_str(&format!(
                    "\nDid you mean one of these? {}",
                    names.join(", ")
                ));
            }
            msg.push_str("\nSee `zen explain --list` for every code.");
            Err(msg)
        }
    }
}

/// `zen py-sync [file]`: plan (and, unless disabled, execute) the Python
/// bootstrap — for a file's `pymport`s, or the current directory's
/// `requirements.txt` with no file. Explicit and idempotent; safe in CI.
fn py_sync_command(args: &[String]) -> Result<(), String> {
    let plan = if args.is_empty() {
        crate::pydeps::sync_directory(std::path::Path::new("."))
    } else {
        let source = std::fs::read_to_string(&args[0])
            .map_err(|e| format!("cannot read {}: {e}", args[0]))?;
        let specs = crate::pydeps::scan_pymports(&source)?;
        crate::pydeps::plan_bootstrap(std::path::Path::new(&args[0]), &specs)
    };
    crate::pydeps::ensure_bootstrap(&plan)?;
    println!("py-sync: {plan:?}");
    Ok(())
}

/// Register and execute an already-linked program on a fresh VM.
pub fn run_linked(linked: linker::LinkedProgram) -> Result<(), String> {
    let mut vm = Vm::new(Box::new(RefCountGc));
    attach_jit(&mut vm);
    attach_python(&mut vm);
    linked.register_with(&mut vm);
    execute_module_fragment(&linked, &mut vm, None)
}

/// When built with the `jit` feature, attach the Cranelift tier-up pool so hot
/// functions compile to native code in the background. Without the feature this
/// is a no-op and the interpreter is the only tier.
#[cfg(feature = "jit")]
fn attach_jit(vm: &mut Vm) {
    // `ZEN_NO_JIT=1` forces the pure-interpreter tier, for A/B measurement.
    if std::env::var_os("ZEN_NO_JIT").is_some() {
        return;
    }
    let threads = std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1))
        .unwrap_or(1);
    vm.jit = zen_jit::JitHandle::start(threads, || Box::new(zen_jit::CraneliftCompiler::new()));
}

#[cfg(not(feature = "jit"))]
fn attach_jit(_vm: &mut Vm) {}

/// When built with the `python` feature, link the embedded CPython bridge so
/// `pymport` resolves (initialization itself stays lazy until first use).
/// Without the feature this is a no-op and `pymport` fails with the
/// actionable unlinked message.
#[cfg(feature = "python")]
fn attach_python(vm: &mut Vm) {
    vm.py_bridge = Some(std::sync::Arc::new(zen_vm::pyembed::PythonBridge::new()));
}

#[cfg(not(feature = "python"))]
fn attach_python(_vm: &mut Vm) {}

/// Execute an inline program (`zen -e '<code>'`) by staging it in a temporary
/// file, so the linker's path-based module search applies unchanged.
fn run_source(code: &str) -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("zen_eval_{}.z", std::process::id()));
    std::fs::write(&path, code)
        .map_err(|e| format!("Z0325: cannot write temp file: {e}"))?;
    let result = run_command(&[path.to_string_lossy().into_owned()]);
    let _ = std::fs::remove_file(&path);
    result
}

fn check_command(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err(format!("Z0501: usage: zen check <file>"));
    }
    let source = std::fs::read_to_string(&args[0]).map_err(|e| {
        format!("Z0325: cannot read {}: {e}", args[0])
    })?;
    // Static checks that need only the AST (division by a literal zero).
    crate::check::check_source(&args[0], &source)?;
    // Full compile: undefined names, structural errors, unresolved imports.
    let _ = linker::link(&args[0])?;
    println!("OK: {}", args[0]);
    Ok(())
}

/// `zen lint [--strict] <file>`: warn on unused vars/params and dead code.
fn lint_command(args: &[String]) -> Result<(), String> {
    let mut strict = false;
    let mut path = None;
    for a in args {
        match a.as_str() {
            "--strict" | "-Werror" => strict = true,
            "--help" | "-h" => {
                println!("usage: zen lint [--strict] <file>");
                println!("  Warns on unused variables/parameters and unreachable code.");
                return Ok(());
            }
            other => {
                if path.is_some() {
                    return Err(format!("Z0501: usage: zen lint [--strict] <file>"));
                }
                path = Some(other.to_string());
            }
        }
    }
    let Some(path) = path else {
        return Err(format!("Z0501: usage: zen lint [--strict] <file>"));
    };
    let source = std::fs::read_to_string(&path).map_err(|e| {
        format!("Z0325: cannot read {path}: {e}")
    })?;
    let warnings = crate::lint::lint_source(&path, &source)?;
    if warnings.is_empty() {
        println!("{}: no warnings", path);
        return Ok(());
    }
    for w in &warnings {
        eprint!("{}", crate::lint::render_warning(&path, &source, w));
    }
    if strict {
        return Err(format!(
            "lint: {} warning(s) found (--strict)",
            warnings.len()
        ));
    }
    Ok(())
}

/// `zen build <file> [-o out]`: bundle a program into a standalone executable.
fn build_command(args: &[String]) -> Result<(), String> {
    let mut source = None;
    let mut output = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                output = Some(
                    args.get(i)
                        .ok_or_else(|| {
                            format!("Z0501: usage: zen build <file> [-o out]")
                        })?
                        .clone(),
                );
            }
            "--help" | "-h" => {
                println!("usage: zen build <file> [-o out]");
                println!("  Bundle <file> and its imports into a standalone executable.");
                return Ok(());
            }
            other => {
                if source.is_some() {
                    return Err(format!("Z0501: usage: zen build <file> [-o out]"));
                }
                source = Some(other.to_string());
            }
        }
        i += 1;
    }
    let Some(source) = source else {
        return Err(format!("Z0501: usage: zen build <file> [-o out]"));
    };
    let output = output.unwrap_or_else(|| default_output_name(&source));
    crate::build::build(&source, &output)
}

/// Default output name: the source stem with no extension (`fib.z` → `fib`).
fn default_output_name(source: &str) -> String {
    let path = std::path::Path::new(source);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "a.out".to_string());
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            parent.join(stem).to_string_lossy().into_owned()
        }
        _ => stem,
    }
}

/// `zen test [filter...] [--jobs N]`: run the corpus in parallel.
fn test_command(args: &[String]) -> Result<(), String> {
    let mut filter = Vec::new();
    let mut jobs = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--jobs" | "-j" => {
                i += 1;
                jobs = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| {
                        format!("Z0501: usage: zen test [filter...] [--jobs N]")
                    })?;
            }
            "--help" | "-h" => {
                println!("usage: zen test [filter...] [--jobs N]");
                println!("  Run tests/cases/*.z in parallel; filter matches file names.");
                return Ok(());
            }
            other => filter.push(other.to_string()),
        }
        i += 1;
    }
    let failures = crate::test_runner::run_tests(&filter, jobs)?;
    if failures > 0 {
        return Err(format!("Z0501: {failures} test(s) failed"));
    }
    Ok(())
}

/// `zen fmt [--check] <files|dirs...>`: format in place, or report diffs.
fn fmt_command(args: &[String]) -> Result<(), String> {
    let mut check = false;
    let mut paths = Vec::new();
    for a in args {
        match a.as_str() {
            "--check" => check = true,
            "--help" | "-h" => {
                println!("usage: zen fmt [--check] <file|dir>...");
                return Ok(());
            }
            other => paths.push(other.to_string()),
        }
    }
    if paths.is_empty() {
        return Err(format!("Z0501: usage: zen fmt [--check] <file|dir>..."));
    }
    let files = crate::fmt::collect_files(&paths)?;
    if files.is_empty() {
        return Err(format!("Z0501: fmt: no .z files found"));
    }
    if check {
        let mut dirty = Vec::new();
        for f in &files {
            if !crate::fmt::check_file(f)? {
                dirty.push(f.clone());
            }
        }
        if dirty.is_empty() {
            println!("fmt: {} file(s) clean", files.len());
            Ok(())
        } else {
            for f in &dirty {
                println!("would reformat: {}", f.display());
            }
            Err(format!("fmt: {} file(s) need formatting", dirty.len()))
        }
    } else {
        let mut changed = 0;
        for f in &files {
            if crate::fmt::format_file(f)? {
                changed += 1;
            }
        }
        println!("fmt: {changed} of {} file(s) reformatted", files.len());
        Ok(())
    }
}

/// Link staged source kept in a temp file so the linker's path-based module
/// search applies unchanged. Shared by `-e` and the REPL.
pub fn link_temp_source(code: &str) -> Result<LinkedProgram, String> {
    let path = std::env::temp_dir().join(format!("zen_eval_{}.z", std::process::id()));
    std::fs::write(&path, code).map_err(|e| format!("repl: {e}"))?;
    linker::link(&path)
}

/// Run one linked entry on `vm`, returning captured `print` output and the
/// module's return value. Runtime failures render as rustc-style diagnostics
/// (exact line, caret, `= help:`) using the failing module's own source.
///
/// Captured output is preserved even when execution fails: callers decide
/// whether to display it (a normal `run` does; a speculative REPL probe does
/// not). This matters because stdout and stderr can be redirected separately;
/// discarding `print` output on the error path would silently lose successful
/// work that happened before the failure.
pub struct RunFailure {
    pub output: String,
    pub diagnostic: String,
}

pub fn run_entry(
    program: &LinkedProgram,
    vm: &mut Vm,
    display_path: Option<&str>,
) -> Result<(String, Value), RunFailure> {
    match vm.run_module(&program.entry.main, &program.entry.children) {
        Ok(result) => Ok((vm.output().to_string(), result)),
        Err(e) => Err(RunFailure {
            output: vm.output().to_string(),
            diagnostic: render_runtime_error(program, vm, display_path, &e),
        }),
    }
}

/// Build the diagnostic for a runtime failure: resolve the failing module's
/// source (entry under `""`, imports by registry name), fall back to the
/// entry source when the module has none (native frames).
fn render_runtime_error(
    program: &LinkedProgram,
    vm: &Vm,
    display_path: Option<&str>,
    err: &zen_vm::VmError,
) -> String {
    let loc = vm.error_location();
    let mod_name = loc
        .as_ref()
        .and_then(|l| vm.module_name_by_id(l.module_id))
        .unwrap_or("");
    let (path, src) = program
        .sources
        .get(mod_name)
        .or_else(|| program.sources.get(""))
        .cloned()
        .unwrap_or_else(|| ("<unknown>".to_string(), String::new()));
    let path = display_path.unwrap_or(path.as_str());
    let (func, line) = loc
        .map(|l| (l.func, l.line))
        .unwrap_or_else(|| ("<module>".to_string(), 0));
    if src.is_empty() {
        return format!("Runtime error: {err}");
    }
    zen_cli::zendiag::render_runtime(path, &src, &func, line, err)
}

/// Run one linked entry on `vm`, echoing captured `print` output and the
/// non-nil result value. Shared by `run` and the REPL.
pub fn execute_module_fragment(
    program: &LinkedProgram,
    vm: &mut Vm,
    display_path: Option<&str>,
) -> Result<(), String> {
    match run_entry(program, vm, display_path) {
        Ok((output, result)) => {
            if !output.is_empty() {
                print!("{output}");
            }
            if !result.is_nil() {
                println!("{}", stringify_value(result));
            }
            Ok(())
        }
        Err(failure) => {
            // Emit successful output before reporting the failure. The
            // process exits nonzero after this diagnostic reaches `main`,
            // which would otherwise discard Rust's buffered stdout.
            if !failure.output.is_empty() {
                print!("{}", failure.output);
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
            Err(failure.diagnostic)
        }
    }
}

/// Format a runtime value for REPL/`run` display.
pub fn display_value(v: &Value) -> String {
    stringify_value(v.clone())
}

fn stringify_value(v: Value) -> String {
    if v.is_nil() {
        "nil".to_string()
    } else if let Some(b) = v.as_bool() {
        b.to_string()
    } else if let Some(i) = v.as_int() {
        i.to_string()
    } else if let Some(f) = v.as_float() {
        format_float(f)
    } else if let Some(s) = v.as_str() {
        format!("\"{}\"", s)
    } else if let Some(b) = v.as_bytes() {
        format!("bytes({})", String::from_utf8_lossy(b))
    } else {
        format!("<{}>", v.type_name())
    }
}

fn format_float(f: f64) -> String {
    if f.fract() == 0.0 && f.is_finite() {
        format!("{f:.1}")
    } else {
        format!("{f}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_run_preserves_output_before_error() {
        let program = linker::link_program(
            "<output-before-error>".to_string(),
            "print 15\nthrow \"boom\"\n".to_string(),
            None,
            &std::collections::HashMap::new(),
        )
        .expect("link");
        let mut vm = Vm::new(Box::new(RefCountGc));
        program.register_with(&mut vm);

        let failure = run_entry(&program, &mut vm, None).expect_err("run fails");
        assert_eq!(failure.output, "15\n");
        assert!(
            failure.diagnostic.contains("boom"),
            "diagnostic was {}",
            failure.diagnostic
        );
    }
}
