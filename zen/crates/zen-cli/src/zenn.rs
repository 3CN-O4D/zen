//! `zenn` binary: the 3.0 engine CLI. Same commands as `zen`.

mod build;
mod check;
mod cli;
mod explain;
mod fmt;
mod lint;
mod pydeps;
mod repl;
mod test_runner;

use std::process;

fn main() {
    if let Err(e) = cli::run() {
        eprintln!("{}", cli::format_error(&e));
        process::exit(1);
    }
}
