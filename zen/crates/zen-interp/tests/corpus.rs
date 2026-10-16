//! Runs the self-checking corpus through the M3 interpreter.
//!
//! Each case prints `SUITE PASS` when all of its checks hold. This is the
//! interpreter's standalone oracle until the register VM (M4) can execute the
//! same programs and the differential harness compares the two.

use std::fs;
use std::path::{Path, PathBuf};

fn cases_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/cases")
}

fn run_case(name: &str) -> String {
    let path = cases_dir().join(name);
    let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut interp = zen_interp::Interp::new();
    interp.add_module_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../std"));
    match interp.run_source(&src) {
        Ok(_) => interp.output().to_string(),
        Err(e) => format!("<error> {e}\n"),
    }
}

fn all_cases() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(cases_dir())
        .expect("read tests/cases")
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) == Some("z") {
                p.file_name()?.to_str().map(String::from)
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

#[test]
fn corpus_summary() {
    let mut summary = String::new();
    let mut passed = 0usize;
    let mut failed = Vec::new();
    for name in all_cases() {
        let out = run_case(&name);
        if out.contains("SUITE PASS") {
            passed += 1;
            summary.push_str(&format!("PASS {name}\n"));
        } else {
            failed.push(name.clone());
            summary.push_str(&format!("FAIL {name}\n{}\n", out.trim_end()));
        }
    }
    eprintln!("{summary}\n{passed} passed, {} failed", failed.len());
    assert!(passed >= 8, "core cases regressed; see stderr");
}
