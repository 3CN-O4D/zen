//! Zen CLI: zenn and zen binaries.
//! See `docs/15-toolchain.md`.

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
        // Rustc-style diagnostics carry their own header/span/notes — print
        // them verbatim. Plain messages get the `zen:` prefix so every
        // failure is clearly from the tool, never a bare fragment.
        if e.starts_with("\x1b[1;31merror") {
            eprintln!("{e}");
        } else {
            eprintln!("zen: {e}");
        }
        process::exit(1);
    }
}
