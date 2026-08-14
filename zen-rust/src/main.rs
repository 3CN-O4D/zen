mod cdp;
mod pm;
mod runtime;
mod state;

use std::{env, fs, process};

fn usage() {
    eprintln!("Zen native runtime\n\nUsage:\n  zen run <file.z>\n  zen <file.z>\n  zen -e <source>\n  zen --version\n\nCommands:\n  zen run <file.z>            execute a script\n  zen check <file.z>          parse and validate a script without running it\n  zen lint <file.z>           report suspicious patterns and errors\n  zen repl                    start an interactive session\n\nPackage manager:\n  zen pm install <owner/repo>[@tag] | <url> | <file>\n  zen pm install --force <spec>   reinstall\n  zen pm install -r <freeze.txt>  install from freeze file\n  zen pm list\n  zen pm freeze\n  zen pm remove <name>\n  zen pm info <name>\n  zen pm verify <name>            check sha256 against source\n  zen pm pack <dir>               build publishable tarball\n  zen pm publish <dir> <git-remote>");
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
    let source = match args.as_slice() {
        [flag, code] if flag == "-e" || flag == "--eval" => Ok(code.clone()),
        [command, file, ..] if command == "run" => fs::read_to_string(file),
        [file, ..] => fs::read_to_string(file),
        _ => {
            usage();
            process::exit(2);
        }
    };
    match source.and_then(|source| runtime::run(&source).map_err(std::io::Error::other)) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("zen: {error}");
            process::exit(1);
        }
    }
}

fn repl() {
    use std::io::{stdin, stdout, Write};
    let mut session = match runtime::Repl::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("zen: {e}");
            process::exit(1);
        }
    };
    loop {
        print!("zen> ");
        let _ = stdout().flush();
        let mut line = String::new();
        match stdin().read_line(&mut line) {
            Ok(0) => {
                println!();
                break;
            }
            Ok(_) => {}
            Err(_) => break,
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match line {
            ":q" | ":quit" | ":exit" => break,
            ":h" | ":help" => {
                println!("Commands: :q quit, :h help");
                continue;
            }
            _ => {}
        }
        if let Some(code) = line.strip_prefix(":c ") {
            match session.eval_line(&format!("print {code}")) {
                Ok(()) => {}
                Err(e) => eprintln!("zen: {e}"),
            }
        } else if let Err(e) = session.eval_line(line) {
            eprintln!("zen: {e}");
        }
    }
}