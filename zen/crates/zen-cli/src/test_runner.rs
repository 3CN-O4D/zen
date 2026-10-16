//! `zen test` — parallel corpus test runner.
//!
//! Runs `tests/cases/*.z` as child `zen run` processes, in parallel:
//! - Tests 21–26 spawn `../../target/debug/zen` themselves, so they must run
//!   with CWD set to `tests/cases/`.
//! - Test 09 needs the mock FTP server (`tests/mock_ftp.py`) on 127.0.0.1:2121;
//!   the runner starts it automatically when the port is closed.
//! - Pass = exit 0 with `SUITE PASS` in output and no `FAIL` lines.
//!
//! Gate: all 32 cases in < 5s on 8 cores.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Tests that must run with CWD = `tests/cases/` (they spawn the zen binary
/// via a relative `../../target/debug/zen` path).
const SPAWN_CWD_TESTS: &[&str] = &[
    "21_pure_zen.z",
    "22_binascii.z",
    "23_collections.z",
    "24_string.z",
    "25_tempfile.z",
    "26_random_itertools.z",
];

const MOCK_FTP_PORT: u16 = 2121;

pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub duration: Duration,
    pub output: String,
}

/// Run the corpus. `filter` matches substrings of file names (empty = all).
/// Returns the number of failures.
pub fn run_tests(filter: &[String], jobs: usize) -> Result<usize, String> {
    let cases_dir = find_cases_dir()?;
    let repo_root = cases_dir.parent().and_then(|p| p.parent()).map(|p| p.to_path_buf());
    let zen_bin = std::env::current_exe().map_err(|e| format!("test: {e}"))?;

    // Ensure mock FTP is up for test 09.
    let mut ftp = ensure_mock_ftp(&cases_dir)?;

    let mut files: Vec<PathBuf> = std::fs::read_dir(&cases_dir)
        .map_err(|e| format!("test: cannot list {}: {e}", cases_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "z").unwrap_or(false))
        .collect();
    files.sort();
    if !filter.is_empty() {
        files.retain(|p| {
            let n = p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            filter.iter().any(|f| n.contains(f.as_str()))
        });
    }
    if files.is_empty() {
        return Err("test: no matching test files".into());
    }

    let jobs = jobs.max(1);
    let results: Arc<Mutex<Vec<TestResult>>> = Arc::new(Mutex::new(Vec::new()));
    let queue: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(files));
    let total = queue.lock().unwrap().len();

    let started = Instant::now();
    let mut handles = Vec::new();
    for _ in 0..jobs.min(total) {
        let queue = Arc::clone(&queue);
        let results = Arc::clone(&results);
        let cases_dir = cases_dir.clone();
        let repo_root = repo_root.clone();
        let zen_bin = zen_bin.clone();
        handles.push(std::thread::spawn(move || loop {
            let file = { queue.lock().unwrap().pop() };
            let Some(file) = file else { break };
            let r = run_one(&file, &cases_dir, repo_root.as_deref(), &zen_bin);
            results.lock().unwrap().push(r);
        }));
    }
    for h in handles {
        h.join().map_err(|_| "test: worker panicked".to_string())?;
    }
    let elapsed = started.elapsed();

    let mut results = results.lock().unwrap();
    results.sort_by(|a, b| a.name.cmp(&b.name));
    let mut failures = 0;
    for r in results.iter() {
        if r.passed {
            println!("ok   {} ({:.1}s)", r.name, r.duration.as_secs_f32());
        } else {
            failures += 1;
            println!("FAIL {} ({:.1}s)", r.name, r.duration.as_secs_f32());
            for line in r.output.lines().filter(|l| {
                let t = l.trim();
                t.starts_with("FAIL") || t.contains("rror") || t.contains("SUITE FAIL")
            }).take(10) {
                println!("     | {line}");
            }
        }
    }
    println!(
        "test result: {} passed; {} failed; {:.1}s total",
        results.len() - failures,
        failures,
        elapsed.as_secs_f32()
    );
    if let Some(mut c) = ftp.take() {
        let _ = c.kill();
        let _ = c.wait();
    }
    Ok(failures)
}

fn run_one(
    file: &Path,
    cases_dir: &Path,
    repo_root: Option<&Path>,
    zen_bin: &Path,
) -> TestResult {
    let name = file
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let cwd = if SPAWN_CWD_TESTS.iter().any(|t| *t == name) {
        cases_dir.to_path_buf()
    } else {
        repo_root.map(|p| p.to_path_buf()).unwrap_or_else(|| cases_dir.to_path_buf())
    };
    let start = Instant::now();
    let out = Command::new(zen_bin)
        .arg("run")
        .arg(file)
        .current_dir(&cwd)
        .output();
    let duration = start.elapsed();
    match out {
        Ok(o) => {
            let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
            let err = String::from_utf8_lossy(&o.stderr);
            if !err.is_empty() {
                text.push_str(&err);
            }
            let passed = o.status.success() && !has_failure(&text);
            TestResult { name, passed, duration, output: text }
        }
        Err(e) => TestResult {
            name,
            passed: false,
            duration,
            output: format!("failed to spawn: {e}"),
        },
    }
}

/// True when test output signals failure: explicit FAIL lines, suite failure
/// summaries with a nonzero count, or runtime errors. Tests 29–32 never print
/// `SUITE PASS` — they only print `FAIL: ...` lines — so absence of failure
/// markers plus exit 0 is the pass signal.
fn has_failure(text: &str) -> bool {
    for line in text.lines() {
        let t = line.trim();
        // "FAIL: ...", "FAIL ...", "SUITE FAILURES: N" (N > 0), panics, errors.
        if t.starts_with("FAIL") && !t.starts_with("FAILURES: 0") {
            // Guard: "SUITE FAILURES: 0" would be a pass, but no test prints it.
            if t == "SUITE FAILURES: 0" {
                continue;
            }
            return true;
        }
        if t.contains("SUITE FAIL") {
            return true;
        }
        if t.starts_with("Runtime error") || t.starts_with("Traceback") {
            return true;
        }
    }
    // "SUITE FAILURES: N" buried mid-line.
    if let Some(idx) = text.find("SUITE FAILURES:") {
        let rest = text[idx..].chars().filter(|c| c.is_ascii_digit()).collect::<String>();
        if rest.parse::<u64>().unwrap_or(1) > 0 {
            return true;
        }
    }
    false
}
fn find_cases_dir() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("tests/cases"));
        candidates.push(cwd.join("cases"));
        candidates.push(cwd.clone());
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // <root>/target/debug/zen -> <root>/tests/cases
            if let Some(t) = dir.parent() {
                if let Some(root) = t.parent() {
                    candidates.push(root.join("tests/cases"));
                }
            }
            candidates.push(dir.join("tests/cases"));
        }
    }
    for c in candidates {
        if c.is_dir() && c.join("01_literals_ops.z").exists() {
            return Ok(c);
        }
    }
    Err("test: cannot locate tests/cases/ (run from the repo root)".into())
}

/// Start `tests/mock_ftp.py` when port 2121 is closed. Returns the child to
/// keep alive for the run (killed on drop via explicit kill).
fn ensure_mock_ftp(cases_dir: &Path) -> Result<Option<Child>, String> {
    if TcpStream::connect(("127.0.0.1", MOCK_FTP_PORT)).is_ok() {
        return Ok(None);
    }
    let mock = cases_dir
        .parent()
        .map(|t| t.join("mock_ftp.py"))
        .filter(|p| p.exists());
    let Some(mock) = mock else { return Ok(None) };
    let mut child = Command::new("python3")
        .arg(&mock)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("test: cannot start mock_ftp.py: {e}"))?;
    // Wait briefly for the listener.
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(50));
        if TcpStream::connect(("127.0.0.1", MOCK_FTP_PORT)).is_ok() {
            return Ok(Some(child));
        }
    }
    // Best effort: leave it; test 09 will report the failure itself.
    let _ = child.kill();
    Ok(None)
}

/// Drain helper kept for API symmetry (currently unused).
#[allow(dead_code)]
fn _drain<R: Read>(mut r: R) -> String {
    let mut s = String::new();
    let _ = r.read_to_string(&mut s);
    s
}

/// Write helper kept for API symmetry (currently unused).
#[allow(dead_code)]
fn _write_all(mut w: impl Write, s: &str) {
    let _ = w.write_all(s.as_bytes());
}
