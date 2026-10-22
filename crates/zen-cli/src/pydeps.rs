//! Python dependency bootstrap for `pymport` (blueprint Step 2).
//!
//! `pymport` names a *foreign* package, so the CPython bridge needs it
//! importable. This module scans entry source for `pymport` statements with
//! the real parser (never regexes), resolves a project root, and plans the
//! bootstrap: an existing `.zen_venv` whose stamp matches `requirements.txt`
//! is reused untouched; otherwise the venv is created and `pip install -r`
//! runs.
//!
//! Network/process execution happens ONLY inside [`ensure_bootstrap`], which
//! runs when a program actually uses `pymport` — and never when
//! `ZEN_NO_PY_BOOTSTRAP` is set (tests, hermetic runs, benchmarks). A
//! missing `requirements.txt` cannot be fixed automatically (a module name
//! is not a pip name: `whatsapp` ships as something else), so that plan
//! reports instead of guessing.

use std::path::{Path, PathBuf};
use zen_syntax::ast::{BlockOrExpr, ClassMember, Stmt};
use zen_syntax::lex::lex;
use zen_syntax::parser::Parser;
use zen_syntax::span::FileId;

/// One `pymport` statement: `pymport dotted.path [as alias]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PymportSpec {
    /// Dotted module path segments (`["a", "b"]` for `pymport a.b`).
    pub path: Vec<String>,
    /// Optional `as` alias.
    pub alias: Option<String>,
}

impl PymportSpec {
    /// Dotted path as written (`a.b`).
    pub fn dotted(&self) -> String {
        self.path.join(".")
    }
}

/// Collect every `pymport` in `source`, including ones nested in functions,
/// classes, and control flow (bootstrap installs a superset — execution is
/// what decides). Parse errors surface as `Err` with the parser's message.
pub fn scan_pymports(source: &str) -> Result<Vec<PymportSpec>, String> {
    let lx = lex(source, FileId(0));
    let mut parser = Parser::new(lx);
    let module = parser
        .parse_module()
        .map_err(|e| format!("pymport scan: {e:?}"))?;
    let mut out = Vec::new();
    walk_block(&module.items, &mut out);
    Ok(out)
}

fn walk_block(stmts: &[Stmt], out: &mut Vec<PymportSpec>) {
    for s in stmts {
        walk_stmt(s, out);
    }
}

fn walk_stmt(s: &Stmt, out: &mut Vec<PymportSpec>) {
    match s {
        Stmt::Pymport { path, alias, .. } => out.push(PymportSpec {
            path: path.clone(),
            alias: alias.clone(),
        }),
        Stmt::If {
            then_branch,
            elifs,
            else_branch,
            ..
        } => {
            walk_block(&then_branch.stmts, out);
            for (_, b) in elifs {
                walk_block(&b.stmts, out);
            }
            if let Some(b) = else_branch {
                walk_block(&b.stmts, out);
            }
        }
        Stmt::While { body, .. } | Stmt::For { body, .. } => walk_block(&body.stmts, out),
        Stmt::Function { body, .. } => walk_block(&body.stmts, out),
        Stmt::Try {
            body,
            catches,
            finally,
            ..
        } => {
            walk_block(&body.stmts, out);
            for c in catches {
                walk_block(&c.body.stmts, out);
            }
            if let Some(b) = finally {
                walk_block(&b.stmts, out);
            }
        }
        Stmt::Class { members, .. } => {
            for m in members {
                walk_member(m, out);
            }
        }
        Stmt::Match { arms, .. } | Stmt::When { arms, .. } => {
            for a in arms {
                match &a.body {
                    BlockOrExpr::Block(b) => walk_block(&b.stmts, out),
                    BlockOrExpr::Expr(_) => {}
                }
            }
        }
        _ => {}
    }
}

fn walk_member(m: &ClassMember, out: &mut Vec<PymportSpec>) {
    walk_block(&m.body.stmts, out);
}

/// What the bootstrap needs to do (pure filesystem checks, no network).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapPlan {
    /// No `pymport` in the program, or the venv already satisfies the
    /// requirements stamp: run without touching anything.
    Clean { modules: Vec<String> },
    /// No usable venv and no `requirements.txt`: report what to do. Zen
    /// cannot guess pip names from module names, so this never installs.
    NoRequirements { root: PathBuf, modules: Vec<String> },
    /// `.zen_venv` missing (but requirements exist): create + install.
    MissingVenv { root: PathBuf, modules: Vec<String> },
    /// Venv present but the stamp disagrees with `requirements.txt`.
    MissingPackages { root: PathBuf, modules: Vec<String> },
}

impl BootstrapPlan {
    /// Modules that triggered the plan (dotted paths, deduplicated).
    pub fn modules(&self) -> Vec<String> {
        match self {
            BootstrapPlan::Clean { modules } => modules.clone(),
            BootstrapPlan::NoRequirements { modules, .. }
            | BootstrapPlan::MissingVenv { modules, .. }
            | BootstrapPlan::MissingPackages { modules, .. } => modules.clone(),
        }
    }

    /// True when nothing needs to run.
    pub fn is_clean(&self) -> bool {
        matches!(self, BootstrapPlan::Clean { .. })
    }
}

/// Resolve the project root for `entry_file`: the nearest ancestor holding
/// `requirements.txt` (or `zen.toml`, reserved for `[py-deps]`), else the
/// entry file's own directory.
pub fn project_root(entry_file: &Path) -> PathBuf {
    let mut dir = entry_file
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    loop {
        if dir.join("requirements.txt").is_file() || dir.join("zen.toml").is_file() {
            return dir;
        }
        match dir.parent() {
            Some(p) if p != dir => dir = p.to_path_buf(),
            _ => {
                return entry_file
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."))
            }
        }
    }
}

fn dedup_modules(specs: &[PymportSpec]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in specs {
        let d = s.dotted();
        if !out.contains(&d) {
            out.push(d);
        }
    }
    out
}

/// Plan the bootstrap for `specs` found in the program at `entry_file`.
/// Pure filesystem reads; safe to call on every run.
pub fn plan_bootstrap(entry_file: &Path, specs: &[PymportSpec]) -> BootstrapPlan {
    let modules = dedup_modules(specs);
    if modules.is_empty() {
        return BootstrapPlan::Clean { modules };
    }
    plan_for_root(project_root(entry_file), modules)
}

fn plan_for_root(root: PathBuf, modules: Vec<String>) -> BootstrapPlan {
    let req = root.join("requirements.txt");
    if !req.is_file() {
        return BootstrapPlan::NoRequirements { root, modules };
    }
    let venv = root.join(".zen_venv");
    if !venv.join("pyvenv.cfg").is_file() {
        return BootstrapPlan::MissingVenv { root, modules };
    }
    let stamp = venv.join("zen_py_stamp");
    let want = stamp_content(&req);
    let have = std::fs::read_to_string(&stamp).unwrap_or_default();
    if have.trim() == want {
        BootstrapPlan::Clean { modules }
    } else {
        BootstrapPlan::MissingPackages { root, modules }
    }
}

/// Sync the requirements in `root` (usually the current directory) without
/// scanning any source: reinstalls when the stamp is missing or stale.
/// Backs bare `zen py-sync`.
pub fn sync_directory(root: &Path) -> BootstrapPlan {
    plan_for_root(root.to_path_buf(), Vec::new())
}

/// The stamp couples the requirements byte length with an FNV-1a hash of the
/// content: std-only, no new dependencies, and accidental collision is
/// practically impossible.
fn stamp_content(req: &Path) -> String {
    let bytes = std::fs::read(req).unwrap_or_default();
    let mut h: u64 = 0xcbf29ce484222325;
    for b in &bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{}:{:016x}", bytes.len(), h)
}

/// Execute `plan`: create the venv and `pip install -r` as needed. Prints
/// what it does. Respects `ZEN_NO_PY_BOOTSTRAP` (any value disables), for
/// tests and hermetic runs. Foreground and deterministic — the program must
/// not start before its packages exist.
pub fn ensure_bootstrap(plan: &BootstrapPlan) -> Result<(), String> {
    if plan.is_clean() {
        return Ok(());
    }
    if std::env::var_os("ZEN_NO_PY_BOOTSTRAP").is_some() {
        eprintln!("zen: pymport needs {:?}, but ZEN_NO_PY_BOOTSTRAP is set — skipping Python bootstrap (import will fail at runtime)", plan.modules());
        return Ok(());
    }
    match plan {
        BootstrapPlan::Clean { .. } => Ok(()),
        BootstrapPlan::NoRequirements { root, modules } => {
            eprintln!(
                "zen: note: pymport uses {modules:?}, but {} has no requirements.txt — \
                 if any of these are third-party packages, create one mapping modules \
                 to pip packages (a module name is not always a pip name). \
                 Standard-library modules need nothing.",
                root.display()
            );
            Ok(())
        }
        BootstrapPlan::MissingVenv { root, modules } => {
            eprintln!("zen: creating Python venv for {modules:?} at {}", root.join(".zen_venv").display());
            create_venv(root)?;
            pip_install(root)?;
            write_stamp(root)?;
            Ok(())
        }
        BootstrapPlan::MissingPackages { root, modules } => {
            eprintln!("zen: requirements changed for {modules:?}; reinstalling into {}", root.join(".zen_venv").display());
            pip_install(root)?;
            write_stamp(root)?;
            Ok(())
        }
    }
}

/// Export the import paths the embedded interpreter needs: the venv
/// site-packages (if any), the project root, then the entry file's own
/// directory. Priority is venv > root > script dir (pinned deps beat stray
/// files; the project root covers monorepos). Idempotent (each step skips
/// when already listed).
pub fn export_python_paths(entry_file: &Path) {
    let root = project_root(entry_file);
    if let Some(entry_dir) = entry_file.parent() {
        prepend_pythonpath_unique(entry_dir);
    }
    prepend_pythonpath_unique(&root);
    if let Some(site) = venv_site_packages(&root) {
        prepend_pythonpath_unique(&site);
    }
}

/// Locate the venv's site-packages dir (`lib/python*/site-packages`, or
/// `Lib/site-packages` on Windows). `None` when there is no usable venv.
pub fn venv_site_packages(root: &Path) -> Option<PathBuf> {
    let venv = root.join(".zen_venv");
    if !venv.join("pyvenv.cfg").is_file() {
        return None;
    }
    #[cfg(windows)]
    {
        let p = venv.join("Lib").join("site-packages");
        if p.is_dir() {
            return Some(p);
        }
        return None;
    }
    #[cfg(not(windows))]
    {
        let lib = venv.join("lib");
        let entries = std::fs::read_dir(&lib).ok()?;
        for entry in entries.flatten() {
            let site = entry.path().join("site-packages");
            if site.is_dir() {
                return Some(site);
            }
        }
        None
    }
}

/// Prepend `dir` to `PYTHONPATH` (preserving any existing value) so the
/// embedded interpreter imports project packages first.
pub fn prepend_pythonpath(dir: &Path) {
    let dir = dir.to_string_lossy().into_owned();
    let sep = if cfg!(windows) { ";" } else { ":" };
    let next = match std::env::var_os("PYTHONPATH") {
        Some(cur) if !cur.is_empty() => {
            let mut s = std::ffi::OsString::from(dir);
            s.push(sep);
            s.push(cur);
            s
        }
        _ => std::ffi::OsString::from(dir),
    };
    std::env::set_var("PYTHONPATH", next);
}

/// Like [`prepend_pythonpath`], but a no-op when `dir` is already listed
/// (compared textually after normalization). Keeps repeated runs and the
/// venv/root/script-dir triple from stacking duplicates.
pub fn prepend_pythonpath_unique(dir: &Path) {
    let sep = if cfg!(windows) { ";" } else { ":" };
    let want = dir.to_string_lossy().into_owned();
    if let Some(cur) = std::env::var_os("PYTHONPATH") {
        let cur = cur.to_string_lossy();
        if cur.split(sep).any(|p| p == want) {
            return;
        }
    }
    prepend_pythonpath(dir);
}

fn python3() -> Result<String, String> {
    if let Ok(p) = std::env::var("ZEN_PYTHON") {
        return Ok(p);
    }
    Ok("python3".to_string())
}

fn create_venv(root: &Path) -> Result<(), String> {
    let py = python3()?;
    let status = std::process::Command::new(&py)
        .arg("-m")
        .arg("venv")
        .arg(root.join(".zen_venv"))
        .status()
        .map_err(|e| format!("cannot run {py} -m venv: {e} (set ZEN_PYTHON to a Python with venv)"))?;
    if !status.success() {
        return Err(format!("{py} -m venv failed for {}", root.display()));
    }
    Ok(())
}

fn pip_of(root: &Path) -> PathBuf {
    let venv = root.join(".zen_venv");
    #[cfg(windows)]
    let pip = venv.join("Scripts").join("pip.exe");
    #[cfg(not(windows))]
    let pip = venv.join("bin").join("pip");
    pip
}

fn pip_install(root: &Path) -> Result<(), String> {
    let pip = pip_of(root);
    let pip_cmd = if pip.is_file() {
        pip.to_string_lossy().into_owned()
    } else {
        // Fallback: system pip into the venv's python.
        return Err(format!(
            "no pip at {}; install pip in the venv or set ZEN_NO_PY_BOOTSTRAP=1",
            pip.display()
        ));
    };
    // Relative paths in requirements.txt (e.g. `./pkg`) resolve against the
    // project root, not whatever directory the user invoked from.
    let status = std::process::Command::new(&pip_cmd)
        .arg("install")
        .arg("-q")
        .arg("-r")
        .arg(root.join("requirements.txt"))
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot run {pip_cmd}: {e}"))?;
    if !status.success() {
        return Err(format!("{pip_cmd} install -r requirements.txt failed"));
    }
    Ok(())
}

fn write_stamp(root: &Path) -> Result<(), String> {
    let stamp = stamp_content(&root.join("requirements.txt"));
    std::fs::write(root.join(".zen_venv").join("zen_py_stamp"), stamp + "\n")
        .map_err(|e| format!("cannot write venv stamp: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    /// A fresh scratch dir per test (no shared state, no network).
    fn scratch() -> PathBuf {
        let id = N.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("zen_pydeps_{}_{}", std::process::id(), id));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn entry_in(dir: &Path) -> PathBuf {
        dir.join("main.zen")
    }

    #[test]
    fn scan_finds_plain_dotted_and_aliased() {
        let specs = scan_pymports("pymport whatsapp\npymport a.b as c\nimport x\n").unwrap();
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].path, vec!["whatsapp"]);
        assert!(specs[0].alias.is_none());
        assert_eq!(specs[1].path, vec!["a", "b"]);
        assert_eq!(specs[1].alias.as_deref(), Some("c"));
    }

    #[test]
    fn scan_finds_nested_and_dedups() {
        let src = "pymport a\nfunc f() {\n    pymport b as c\n    pymport a\n}\n";
        let specs = scan_pymports(src).unwrap();
        assert_eq!(specs.len(), 3);
        let dir = scratch();
        let plan = plan_bootstrap(&entry_in(&dir), &specs);
        // No requirements.txt anywhere: report, never install.
        assert!(matches!(plan, BootstrapPlan::NoRequirements { .. }));
        assert_eq!(plan.modules(), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn scan_rejects_garbage() {
        assert!(scan_pymports("pymport").is_err());
    }

    #[test]
    fn plan_clean_without_pymports() {
        let dir = scratch();
        let plan = plan_bootstrap(&entry_in(&dir), &[]);
        assert!(plan.is_clean());
    }

    #[test]
    fn plan_missing_venv_then_clean_after_stamp() {
        let dir = scratch();
        std::fs::write(dir.join("requirements.txt"), "requests==2.0\n").unwrap();
        let specs = scan_pymports("pymport requests\n").unwrap();
        let entry = entry_in(&dir);
        assert!(matches!(
            plan_bootstrap(&entry, &specs),
            BootstrapPlan::MissingVenv { .. }
        ));
        // Fake a satisfied venv: pyvenv.cfg + matching stamp, no network.
        std::fs::create_dir_all(dir.join(".zen_venv")).unwrap();
        std::fs::write(dir.join(".zen_venv").join("pyvenv.cfg"), "x").unwrap();
        assert!(matches!(
            plan_bootstrap(&entry, &specs),
            BootstrapPlan::MissingPackages { .. }
        ));
        std::fs::write(
            dir.join(".zen_venv").join("zen_py_stamp"),
            stamp_content(&dir.join("requirements.txt")) + "\n",
        )
        .unwrap();
        assert!(plan_bootstrap(&entry, &specs).is_clean());
        // Changing requirements invalidates the stamp.
        std::fs::write(dir.join("requirements.txt"), "requests==2.1\n").unwrap();
        assert!(matches!(
            plan_bootstrap(&entry, &specs),
            BootstrapPlan::MissingPackages { .. }
        ));
    }

    #[test]
    fn project_root_prefers_requirements_dir() {
        let dir = scratch();
        let sub = dir.join("pkg").join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.join("requirements.txt"), "x\n").unwrap();
        assert_eq!(project_root(&sub.join("m.zen")), dir);
        let bare = scratch();
        assert_eq!(
            project_root(&bare.join("m.zen")),
            bare,
            "no requirements anywhere: entry dir itself"
        );
    }

    #[test]
    fn ensure_never_installs_without_requirements() {
        // NoRequirements always succeeds without touching the network,
        // whatever the environment says.
        let dir = scratch();
        let plan = BootstrapPlan::NoRequirements {
            root: dir,
            modules: vec!["whatsapp".to_string()],
        };
        assert!(ensure_bootstrap(&plan).is_ok());
        assert!(!plan.is_clean());
    }
}

#[cfg(test)]
mod venv_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(1000);

    fn scratch_venv() -> PathBuf {
        let id = N.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("zen_pv_{}_{}", std::process::id(), id));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn site_packages_found_by_layout() {
        let dir = scratch_venv();
        assert!(venv_site_packages(&dir).is_none());
        #[cfg(not(windows))]
        {
            let site = dir.join(".zen_venv").join("lib").join("python3.13").join("site-packages");
            std::fs::create_dir_all(&site).unwrap();
            // pyvenv.cfg gates recognition (a bare dir is not a venv).
            assert!(venv_site_packages(&dir).is_none());
            std::fs::write(dir.join(".zen_venv").join("pyvenv.cfg"), "x").unwrap();
            assert_eq!(venv_site_packages(&dir), Some(site));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prepend_pythonpath_preserves_existing() {
        let prev = std::env::var_os("PYTHONPATH");
        std::env::set_var("PYTHONPATH", "/already/there");
        prepend_pythonpath(Path::new("/venv/site-packages"));
        let sep = if cfg!(windows) { ";" } else { ":" };
        let got = std::env::var_os("PYTHONPATH").unwrap();
        assert_eq!(
            got,
            std::ffi::OsString::from(format!("/venv/site-packages{sep}/already/there"))
        );
        match prev {
            Some(v) => std::env::set_var("PYTHONPATH", v),
            None => std::env::remove_var("PYTHONPATH"),
        }
    }
}
