//! Tier selection: who wins an import, and how a pure module reaches its
//! Rust tier. See `docs/decisions/ADR-0005-module-paths-and-tiers.md`.

use std::fs;
use std::path::{Path, PathBuf};

use zen_vm::{RefCountGc, Vm};

/// A scratch directory holding `std/<name>.z` and an entry file that imports
/// it, so `search_dirs` finds the module the way a real program does.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("zen-tier-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("std")).expect("scratch std/");
        Self(dir)
    }

    fn module(&self, name: &str, source: &str) -> &Self {
        fs::write(self.0.join("std").join(format!("{name}.z")), source).expect("write module");
        self
    }

    fn entry(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, source).expect("write entry");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Run `entry` through the linker and a fresh VM, returning its stdout.
fn run(path: &Path) -> Result<String, String> {
    let linked = zen_cli::linker::link(path)?;
    let mut vm = Vm::new(Box::new(RefCountGc));
    linked.register_with(&mut vm);
    vm.run_module(&linked.entry.main, &linked.entry.children)
        .map_err(|e| format!("Runtime error: {e}"))?;
    Ok(vm.output().to_string())
}

/// `json` ships both tiers, so the `.z` one wins and still exposes the other.
#[test]
fn a_pure_module_wins_and_keeps_its_native_tier() {
    let s = Scratch::new("pure");
    s.module(
        "json",
        "ZEN_PURE = true\n\
         \n\
         let tag = \"pure\"\n\
         \n\
         let echo = fn(s) {\n\
         \x20   return s\n\
         }\n",
    );
    let entry = s.entry(
        "main.z",
        "import json\n\
         print(json.tag)\n\
         print(json.echo(\"kept\"))\n\
         print(json[\"native\"] != null)\n",
    );

    let out = run(&entry).expect("run");
    assert_eq!(out, "pure\nkept\ntrue\n", "stdout was {out:?}");
}

/// Module bodies carry no trailing `Return` — the frame runs off the end — so
/// appended instructions must not land in the middle of the body and clobber a
/// live temporary. This module ends without one.
#[test]
fn attaching_a_tier_does_not_disturb_a_body_with_no_return() {
    let s = Scratch::new("noreturn");
    s.module(
        "json",
        "ZEN_PURE = true\n\
         \n\
         let first = 1\n\
         \n\
         let second = 2\n",
    );
    let entry = s.entry(
        "main.z",
        "import json\n\
         print(json.first)\n\
         print(json.second)\n\
         print(json[\"native\"] != null)\n",
    );

    let out = run(&entry).expect("run");
    assert_eq!(out, "1\n2\ntrue\n", "stdout was {out:?}");
}

/// An explicit `return` does end the frame, so the write has to precede it.
#[test]
fn attaching_a_tier_precedes_an_explicit_return() {
    let s = Scratch::new("return");
    s.module(
        "json",
        "ZEN_PURE = true\n\
         \n\
         func init() {\n\
         \x20   let n = 5\n\
         \x20   if n > 1 {\n\
         \x20\x20   return\n\
         \x20   }\n\
         }\n\
         \n\
         let n = 5\n",
    );
    let entry = s.entry("main.z", "import json\nprint(json.n)\n");

    let out = run(&entry).expect("run");
    assert_eq!(out, "5\n", "stdout was {out:?}");
}

/// A module with no `ZEN_PURE` gets no `native` member — the declaration is
/// what opts in, not the mere existence of a Rust tier.
#[test]
fn a_plain_module_gets_no_native_member() {
    let s = Scratch::new("plain");
    s.module("json", "let tag = \"pure\"\n");
    let entry = s.entry("main.z", "import json\nprint(json.tag)\n");

    let linked = zen_cli::linker::link(&entry).expect("link");
    let json = linked
        .modules
        .iter()
        .find(|(name, _)| name == "json")
        .expect("json module");
    assert!(!json.1.exports.iter().any(|(n, _)| n == "native"));
    assert!(!linked.modules.iter().any(|(n, _)| n == "json.native"));
}

/// Host modules have no `.z` file, so they are imported by bare name.
#[test]
fn a_host_module_is_imported_by_bare_name() {
    let s = Scratch::new("host");
    let entry = s.entry("main.z", "import fs\nprint(fs != null)\n");

    let out = run(&entry).expect("run");
    assert_eq!(out, "true\n", "stdout was {out:?}");
}

/// `std.<name>` is not a spelling that resolves: `std` is a directory in the
/// search path, not a prefix in the module name.
#[test]
fn the_std_prefix_does_not_resolve() {
    let s = Scratch::new("prefix");
    let entry = s.entry("main.z", "import std.fs\nprint(1)\n");

    let err = run(&entry).expect_err("should not link");
    assert!(err.contains("not found"), "error was {err:?}");
}

/// The value published under `native` is a live dictionary, not a copy: the
/// `native` export must reference the native tier's own global.
#[test]
fn the_native_member_is_a_real_dictionary() {
    let s = Scratch::new("dict");
    s.module("json", "ZEN_PURE = true\nlet tag = \"pure\"\n");
    let entry = s.entry(
        "main.z",
        "import json\n\
         let t = json[\"native\"]\n\
         print(type(t))\n\
         print(t.parse(\"true\"))\n",
    );

    let out = run(&entry).expect("run");
    assert_eq!(out, "dict\ntrue\n", "stdout was {out:?}");
}
