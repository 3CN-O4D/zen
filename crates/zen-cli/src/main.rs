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
        // them verbatim. Z-coded errors print as `error[Z####]: message`.
        // Everything else gets the `zen:` prefix.
        eprintln!("{}", cli::format_error(&e));
        process::exit(1);
    }
}
