//! Differential harness: run the same source through the M3 tree-walking
//! interpreter and the M4 register VM.
//!
//! The interpreter is the oracle for the VM (docs/06-parser.md). Two layers:
//!
//! * `tests/diff/*.z` — **enforced parity**. Both engines support these
//!   constructs and must produce byte-identical output; any divergence fails.
//! * `tests/cases/*.z` — a non-fatal **progress dashboard**. These exercise
//!   features M4 is still growing into (try/catch, the module system,
//!   comprehensions, upvalue write-through, …); we report the gap rather than
//!   fail so progress is visible without a permanently red gate.

use std::fs;
use std::path::{Path, PathBuf};

use zen_vm::{RefCountGc, Vm};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn interp_out(src: &str) -> Result<String, String> {
    let mut interp = zen_interp::Interp::new();
    interp.add_module_dir(repo_root().join("std"));
    match interp.run_source(src) {
        Ok(_) => Ok(interp.take_output()),
        Err(e) => Err(e),
    }
}

/// Run a `.z` file through the M4 VM exactly as `zen run` does: link the entry
/// and every transitive import from disk, register the dependencies, execute.
/// Linking is shared with the CLI so the harness can never drift from it.
fn vm_out_file(path: &Path) -> Result<String, String> {
    let linked = match zen_cli::linker::link(path, false) {
        Ok(l) => l,
        Err(e) => return Err(format!("link: {e}")),
    };
    let mut vm = Vm::new(Box::new(RefCountGc));
    linked.register_with(&mut vm);
    match vm.run_module(&linked.entry.main, &linked.entry.children) {
        Ok(_) => Ok(vm.take_output()),
        // `Display`, not `Debug`: the interpreter side yields the error's
        // message as a plain string, so a probe that ends in an uncaught error
        // could never compare equal while this side printed `Variant("…")`.
        // Both sides then render the same `Kind: message` text, and a genuine
        // difference in kind or wording still shows up.
        Err(e) => Err(e.to_string()),
    }
}

fn files_in(dir: &str) -> Vec<(String, PathBuf, String)> {
    let path = repo_root().join(dir);
    let mut entries: Vec<PathBuf> = match fs::read_dir(&path) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(_) => return Vec::new(),
    };
    entries.sort();
    entries
        .into_iter()
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("z"))
        .map(|p| {
            let name = format!("{dir}/{}", p.file_name().unwrap().to_string_lossy());
            let src = fs::read_to_string(&p).unwrap();
            (name, p, src)
        })
        .collect()
}

/// `tests/diff`: both engines must agree exactly.
#[test]
fn diff_probes_match() {
    let mut mismatches = Vec::new();
    let mut unreached = Vec::new();
    let mut count = 0;
    for (name, path, src) in files_in("tests/diff") {
        count += 1;
        match (interp_out(&src), vm_out_file(&path)) {
            (Ok(i), Ok(v)) if i == v => {}
            // Both engines rejecting a program the same way *is* parity — the
            // oracle is the spec. But a whole probe file that ends in an
            // uncaught error is a different thing: its output was thrown away
            // on both sides, so it compared nothing. Those are collected
            // separately and fail below, because a silent no-op probe is worse
            // than a missing one.
            (Err(i), Err(v)) if i == v => unreached.push(format!("{name}: {i}")),
            (i, v) => mismatches.push(format!(
                "{name}\n--- interp ---\n{}\n--- vm ---\n{}",
                i.unwrap_or_else(|e| format!("<err> {e}")),
                v.unwrap_or_else(|e| format!("<err> {e}")),
            )),
        }
    }
    assert!(count > 0, "no differential probes found");
    assert!(
        unreached.is_empty(),
        "{} probe file(s) ended in the same error on both engines, so their \
         output was never compared — catch the error instead:\n\n{}",
        unreached.len(),
        unreached.join("\n")
    );
    assert!(
        mismatches.is_empty(),
        "{} differential mismatch(es):\n\n{}",
        mismatches.len(),
        mismatches.join("\n\n")
    );
}

/// `tests/cases`: report VM-vs-oracle agreement without failing; the VM is
/// still implementing features these cases exercise.
#[test]
fn corpus_progress_dashboard() {
    let mut agree = 0usize;
    let mut both_unsupported = 0usize;
    let mut gaps = Vec::new();
    for (name, path, src) in files_in("tests/cases") {
        match (interp_out(&src), vm_out_file(&path)) {
            (Ok(i), Ok(v)) if i == v => agree += 1,
            (Err(_), Err(_)) => both_unsupported += 1,
            (Ok(i), Ok(_)) if i.contains("SUITE PASS") => {
                gaps.push(format!("{name}: VM output differs (oracle passes)"))
            }
            (Ok(_), Err(e)) => gaps.push(format!("{name}: VM unsupported: {e}")),
            _ => gaps.push(format!("{name}: oracle and VM both fail differently")),
        }
    }
    eprintln!(
        "corpus dashboard: {agree} agree, {both_unsupported} both-unsupported, {} gaps",
        gaps.len()
    );
    for g in &gaps {
        eprintln!("  gap: {g}");
    }
}
