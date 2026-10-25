//! End-to-end `pymport` through `run`-equivalent wiring: link (all transitive
//! sources), scan every linked file, export import paths, attach the real
//! bridge, execute. Standard library plus scratch-local `.py` files only —
//! no venv, no network, hermetic apart from libpython itself.
#![cfg(feature = "python")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use zen_vm::pyembed::PythonBridge;
use zen_vm::{RefCountGc, Vm};

/// A scratch project: Zen entry + helper beside it, plus a Python package
/// exercising chained `.py` imports (`pychain_top` → `pychain_mid` → `json`).
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("zen-pye2e-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }

    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, source).expect("write scratch file");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Link, scan all linked sources, export paths, attach the bridge, run.
fn run_e2e(entry: &Path) -> Result<String, String> {
    let linked = zen_cli::linker::link(entry, false).map_err(|e| format!("link: {e}"))?;
    let mut specs = Vec::new();
    for (path, source) in linked.sources.values() {
        match zen_cli::pydeps::scan_pymports(source) {
            Ok(mut s) => specs.append(&mut s),
            Err(e) => return Err(format!("scan {path}: {e}")),
        }
    }
    if specs.is_empty() {
        return Err("no pymport found in linked sources".to_string());
    }
    zen_cli::pydeps::export_python_paths(entry);
    let mut vm = Vm::new(Box::new(RefCountGc));
    vm.py_bridge = Some(Arc::new(PythonBridge::new()));
    linked.register_with(&mut vm);
    match vm.run_module(&linked.entry.main, &linked.entry.children) {
        Ok(_) => Ok(vm.take_output()),
        Err(e) => Err(e.to_string()),
    }
}

#[test]
fn transitive_zen_helper_pymport_is_found() {
    // main.zen itself has no pymport; helper.z does. Only whole-program
    // scanning (not entry-only) sees it.
    let s = Scratch::new("transitive");
    s.write("helper.z", "pymport json\nlet enc = fn(v) {\n    return json.dumps(v)\n}\n");
    let entry = s.write("main.zen", "import helper\nprint helper.enc({\"a\": 1})\n");
    let linked = zen_cli::linker::link(&entry, false).expect("link");
    let mut mods = Vec::new();
    for source in linked.sources.values().map(|(_, src)| src) {
        for spec in zen_cli::pydeps::scan_pymports(source).expect("scan") {
            mods.push(spec.dotted());
        }
    }
    assert_eq!(mods, vec!["json".to_string()]);
    let out = run_e2e(&entry).expect("run");
    assert_eq!(out, "{\"a\": 1}\n");
}

#[test]
fn chained_python_imports_resolve() {
    // Zen -> pychain_top.py -> pychain_mid.py -> json: CPython handles every
    // level below the top import once the project dir is on sys.path.
    let s = Scratch::new("chain");
    s.write(
        "pychain_mid.py",
        "import json as _json\n\nSUFFIX = \"-chained\"\n\n\ndef build(name):\n    return {\"wrapped\": name + SUFFIX}\n",
    );
    s.write(
        "pychain_top.py",
        "from pychain_mid import build, SUFFIX\n\n\ndef process(name):\n    chained = build(name)\n    return [chained[\"wrapped\"], SUFFIX]\n",
    );
    let entry = s.write(
        "main.zen",
        "pymport pychain_top\nlet r = pychain_top.process(\"gadget\")\nprint r[0]\nprint r[1]\n",
    );
    let out = run_e2e(&entry).expect("run");
    assert_eq!(out, "gadget-chained\n-chained\n");
}
