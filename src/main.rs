mod bytecode;
mod cdp;
mod pm;
mod runtime;
mod state;

use std::{env, fs, process};

fn usage() {
    eprintln!("Zen native runtime\n\nUsage:\n  zen run <file.z>\n  zen <file.z>\n  zen -e <source>\n  zen --version\n\nCommands:\n  zen run <file.z>            execute a script\n  zen check <file.z>          parse and validate a script without running it\n  zen lint <file.z>           report suspicious patterns and errors\n  zen repl                    start an interactive session\n\nPackage manager:\n  zen pm init [name]          initialize a new module (creates zen.json + main.z)\n  zen pm install <spec>       install: owner/repo, url, .z file, or local directory\n  zen pm install --force <spec>   reinstall\n  zen pm install -r <freeze.txt>  install from freeze file\n  zen pm list\n  zen pm freeze\n  zen pm remove <name>\n  zen pm info <name>\n  zen pm verify <name>            check sha256 against source\n  zen pm pack <dir>               build publishable tarball\n  zen pm publish <dir> <git-remote>");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--version") {
        println!("zen {} (native Rust runtime)", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.first().is_some_and(|arg| arg == "pm") {
        let result: Result<(), String> = match args.as_slice() {
            [_, sub, rest @ ..] => {
                let rest: Vec<&str> = rest.iter().map(|s| s.as_str()).collect();
                match (sub.as_str(), rest.as_slice()) {
                ("install", [spec]) => pm::install(spec, false).map(|_| ()),
                ("install", ["--force", spec]) | ("install", [spec, "--force"]) => pm::install(spec, true).map(|_| ()),
                ("install", ["-r", file]) | ("install", ["--requirements", file]) => pm::install_requirements(file),
                ("init", []) => pm::init(None, None).map(|_| ()),
                ("init", ["--name", name]) => pm::init(Some(name), None).map(|_| ()),
                ("init", ["--name", name, "--desc", desc]) => pm::init(Some(name), Some(desc)).map(|_| ()),
                ("init", [name]) => pm::init(Some(name), None).map(|_| ()),
                ("list", []) => pm::list(),
                ("freeze", []) => pm::freeze(),
                ("remove", [name]) | ("uninstall", [name]) => pm::remove(name),
                ("info", [name]) => pm::info(name),
                ("verify", [name]) => pm::verify(name),
                ("pack", [dir]) => pm::pack(dir).map(|_| ()),
                ("publish", [dir, remote]) => pm::publish(dir, remote),
                _ => {
                    usage();
                    process::exit(2);
                }
                }
            },
            _ => {
                usage();
                process::exit(2);
            }
        };
        match result {
            Ok(()) => {}
            Err(e) => {
                eprintln!("zen pm: {e}");
                process::exit(1);
            }
        }
        return;
    }
    if args.first().is_some_and(|arg| arg == "repl") {
        repl();
        return;
    }
    if args.first().is_some_and(|arg| arg == "check") || args.first().is_some_and(|arg| arg == "lint") {
        let command = args[0].clone();
        let file = match args.get(1) {
            Some(file) => file.clone(),
            None => {
                usage();
                process::exit(2);
            }
        };
        let result = match fs::read_to_string(&file) {
            Ok(source) => {
                if command == "lint" {
                    let warnings = runtime::lint(&source);
                    for warning in warnings {
                        println!("{warning}");
                    }
                    Ok(())
                } else {
                    match runtime::check(&source) {
                        Ok(()) => {
                            println!("ok");
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
            }
            Err(e) => Err(e.to_string()),
        };
        match result {
            Ok(()) => {}
            Err(e) => {
                eprintln!("zen: {e}");
                process::exit(1);
            }
        }
        return;
    }
    let (source, filename) = match args.as_slice() {
        [flag, code] if flag == "-e" || flag == "--eval" => (Ok(code.clone()), "<string>".to_string()),
        [command, file, ..] if command == "run" => (fs::read_to_string(file), file.clone()),
        [file, ..] => (fs::read_to_string(file), file.clone()),
        _ => {
            usage();
            process::exit(2);
        }
    };
    match source
        .and_then(|source| runtime::run_named(&source, &filename).map_err(std::io::Error::other))
    {
        Ok(()) => {}
        Err(error) => {
            eprintln!("zen: {error}");
            process::exit(1);
        }
    }
}

fn repl() {
    use rustyline::error::ReadlineError;
    use rustyline::{Config, DefaultEditor};

    let mut session = match runtime::Repl::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("zen: {e}");
            process::exit(1);
        }
    };

    // ── History file ────────────────────────────────────────────────────
    let hist_path = dirs()
        .join(".zen_history");
    let _ = std::fs::create_dir_all(hist_path.parent().unwrap_or(std::path::Path::new(".")));

    let config = Config::builder()
        .history_ignore_space(true)
        .build();

    let mut rl = match DefaultEditor::with_config(config) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("zen: failed to initialize readline: {e}");
            process::exit(1);
        }
    };

    // Load existing history
    let _ = rl.load_history(&hist_path);

    println!("Zen {} — interactive session", env!("CARGO_PKG_VERSION"));
    println!("Type :help for help, :q to quit.\n");

    loop {
        let prompt = match rl.readline("zen> ") {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) => {
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(e) => {
                eprintln!("zen: readline error: {e}");
                break;
            }
        };
        let line = prompt.trim().to_string();
        if line.is_empty() {
            continue;
        }

        // Add to history (only non-empty, non-command lines)
        let _ = rl.add_history_entry(line.as_str());

        // ── REPL commands ────────────────────────────────────────────
        match line.as_str() {
            ":q" | ":quit" | ":exit" => break,
            ":h" | ":help" => {
                println!("{}", runtime::repl_help());
                continue;
            }
            ":help modules" => {
                print!("{}", runtime::list_modules());
                continue;
            }
            ":help types" => {
                println!("{}", runtime::help_types());
                continue;
            }
            ":help functions" | ":help builtins" => {
                println!("{}", runtime::help_builtins());
                continue;
            }
            ":help operators" => {
                println!("{}", runtime::help_operators());
                continue;
            }
            ":help keywords" => {
                println!("{}", runtime::help_keywords());
                continue;
            }
            _ => {}
        }

        // :help <module> or :help <builtin>
        if let Some(modname) = line.strip_prefix(":help ") {
            let modname = modname.trim();
            if modname.is_empty() {
                println!("{}", runtime::repl_help());
            } else {
                // Try module help first; if not found, try builtin help.
                let result = runtime::help_module(modname);
                if result.starts_with("Unknown") {
                    print!("{}", runtime::help_builtin_or_error(modname));
                } else {
                    print!("{}", result);
                }
            }
            continue;
        }

        // :c <expr> — shorthand for print <expr>
        if let Some(code) = line.strip_prefix(":c ") {
            match session.eval_line(&format!("print {code}")) {
                Ok(()) => {}
                Err(e) => eprintln!("zen: {e}"),
            }
            continue;
        }

        // ── Evaluate Zen code ────────────────────────────────────────
        if let Err(e) = session.eval_line(&line) {
            eprintln!("zen: {e}");
        }
    }

    // Save history on exit
    let _ = rl.save_history(&hist_path);
}

fn dirs() -> std::path::PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        std::path::PathBuf::from(home).join(".zen")
    } else {
        std::path::PathBuf::from(".zen")
    }
}