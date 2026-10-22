//! AST round-trip gate (`docs/19-roadmap.md` M1): parse → serialize → parse →
//! identical AST. Every corpus file is parsed, serialized to JSON, deserialized,
//! and compared for structural equality.

use std::fs;
use std::path::{Path, PathBuf};

use zen_syntax::ast::Module;
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

fn parse(src: &str) -> Module {
    let lx = lex(src, FileId(0));
    Parser::new(lx).parse_module().expect("corpus parses")
}

fn roundtrip(m: &Module) -> Module {
    let json = serde_json::to_string(m).expect("AST serializes");
    serde_json::from_str(&json).expect("AST deserializes")
}

#[test]
fn m1_ast_round_trips_over_corpus() {
    let files = all_files();
    for f in &files {
        let src = std::fs::read_to_string(f).unwrap();
        let lx = lex(&src, FileId(0));
        let mut p = Parser::new(lx);
        if let Err(e) = p.parse_module() {
            panic!("FAILED: {:?} -> {:?}", f, e);
        }
    }
    assert!(!files.is_empty(), "corpus is present");
    let mut failures = Vec::new();
    for f in &files {
        let src = fs::read_to_string(f).expect("utf-8 corpus file");
        let ast = parse(&src);
        let json = match serde_json::to_string(&ast) {
            Ok(j) => j,
            Err(e) => {
                failures.push(format!("{}: serialize: {e}", f.display()));
                continue;
            }
        };
        match serde_json::from_str::<Module>(&json) {
            Ok(back) if back == ast => {}
            Ok(_) => failures.push(format!("{}: AST differs after round-trip", f.display())),
            Err(e) => failures.push(format!("{}: deserialize: {e}", f.display())),
        }
    }
    assert!(
        failures.is_empty(),
        "{} file(s) failed to round-trip:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn m1_ast_round_trips_handwritten_constructs() {
    let sources = [
        "var x = 1\nconst y = 2",
        "func add(a, b = 1) { return a + b }",
        "(x) => x * 2",
        "[x ** 2 for x in 1 -> 5 if x > 1]",
        "s[start:end]\nt[0, 1]",
        "match x { 1: { y = 2 }, \"bob\": { name: \"Bob\", id: 7 } }",
        "when { a => 1, b => 2 }",
        "try { risky() } catch { recover() } finally { done() }",
        "shared counter = 0",
        "new Counter(10)",
        "import a, b.c as d\nfrom x import y\ninclude \"./z.z\"",
    ];
    for src in sources {
        let ast = parse(src);
        assert_eq!(ast, roundtrip(&ast), "round-trip differs for: {src}");
    }
}
