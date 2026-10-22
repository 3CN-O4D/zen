//! M1 corpus gate (`docs/19-roadmap.md`): every `tests/cases/*.z` and every
//! `std/*.z` file must parse without error. This is the parser's counterpart to
//! `lex_corpus.rs`'s totality gate.

use std::fs;
use std::path::{Path, PathBuf};

use zen_syntax::lex::lex;
use zen_syntax::parser::Parser;
use zen_syntax::span::FileId;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn z_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "z"))
        .collect();
    v.sort();
    v
}

fn all_files() -> Vec<PathBuf> {
    let mut v = z_files(&repo_root().join("tests/cases"));
    v.extend(z_files(&repo_root().join("tests/diff")));
    v.extend(z_files(&repo_root().join("std")));
    v
}

#[test]
fn m1_every_corpus_file_parses() {
    let files = all_files();
    assert!(!files.is_empty(), "corpus is present");
    let mut failures = Vec::new();
    for f in &files {
        let src = fs::read_to_string(f).expect("utf-8 corpus file");
        let lx = lex(&src, FileId(0));
        if let Err(e) = Parser::new(lx).parse_module() {
            failures.push(format!("{}: {e}", f.display()));
        }
    }
    assert!(
        failures.is_empty(),
        "{} file(s) failed to parse:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
