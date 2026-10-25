//! Module linker: the single place in the workspace that touches the
//! filesystem on behalf of the compiler.
//!
//! The compiler stays pure (AST → bytecode) and the VM stays pure (bytecode →
//! values), so resolving `import "a.b"` to a source file has to live in a
//! host-side component. [`link`] produces data only — the entry program plus
//! every transitively imported [`ModuleUnit`] — so any embedder (the `zen`
//! binary, the differential harness, a future build cache) can build its own VM.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use zen_compiler::{compile_module, CompiledModule};
use zen_syntax::{
    ast::{AssignOp, Expr, Literal, Stmt},
    lex,
    parser::Parser,
    FileId,
};
use zen_vm::bytecode::ModuleUnit;
use zen_vm::native::native_modules;
use zen_vm::{FunctionProto, Instruction, Opcode};

/// A fully linked program: the entry module plus its dependencies in link
/// order. Dependencies are keyed by the dotted name used in `import`.
#[derive(Debug)]
pub struct LinkedProgram {
    pub entry: CompiledModule,
    pub modules: Vec<(String, ModuleUnit)>,
    /// Module name → (file path, source text) for every `.z` module linked,
    /// so runtime diagnostics can point at the failing file. The entry module
    /// is stored under the empty key; native modules have no source and are
    /// absent (their errors surface at the Zen call site instead).
    pub sources: HashMap<String, (String, String)>,
    /// When true, dangerous native modules (ffi, sys, process, socket,
    /// requests, http) are not registered.
    pub safe_mode: bool,
}

impl LinkedProgram {
    /// Register every linked dependency with `vm` under its dotted name.
    pub fn register_with(&self, vm: &mut zen_vm::Vm) {
        for (name, unit) in &self.modules {
            vm.register_module(name.clone(), unit.clone());
        }
    }

    /// Names of all modules the program imports, transitively, in link order.
    pub fn module_names(&self) -> Vec<&str> {
        self.modules.iter().map(|(n, _)| n.as_str()).collect()
    }
}

/// Lex, parse and compile one source file. All diagnostics carry the path so a
/// linker failure points at the file that actually failed.
pub fn compile_source(path: &str, source: &str) -> Result<CompiledModule, String> {
    let file_id = FileId(0);
    let lx = lex(source, file_id);
    // The parser never inspects `Lexed.errors`, so a hard lex failure must
    // surface here — otherwise an unterminated string silently mis-parses.
    if let Some(rendered) = crate::zendiag::render_lex(path, source, &lx.errors) {
        return Err(rendered);
    }
    let mut parser = Parser::new(lx);
    let ast = parser
        .parse_module()
        .map_err(|e| crate::zendiag::render_parse(path, source, &e))?;
    compile_module(&ast, source)
        .map_err(|e| crate::zendiag::render_compile(path, source, &e))
}

/// Link `entry_path` and every module it transitively imports.
pub fn link(entry_path: impl AsRef<Path>, safe_mode: bool) -> Result<LinkedProgram, String> {
    let entry_path = entry_path.as_ref();
    let display = entry_path.to_string_lossy().to_string();
    let source =
        fs::read_to_string(entry_path)
            .map_err(|e| format!("Z0325: cannot read {}: {}", display, e))?;
    link_program(display, source, Some(entry_path), &HashMap::new(), safe_mode)
}

/// Link a program whose entry source is supplied directly, with an optional
/// in-memory overlay of module-name → source. Sources in `overlay` take
/// precedence over the filesystem, which is what makes a `zen build` binary
/// self-contained: the overlay holds every `.z` module it imported.
pub fn link_program(
    display: String,
    source: String,
    entry_path: Option<&Path>,
    overlay: &HashMap<String, String>,
    safe_mode: bool,
) -> Result<LinkedProgram, String> {
    let entry = compile_source(&display, &source)?;

    let base_dir = entry_path
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let search = entry_path
        .map(search_dirs)
        .unwrap_or_else(default_search_dirs);

    let mut modules: Vec<(String, ModuleUnit)> = Vec::new();
    let mut sources: HashMap<String, (String, String)> = HashMap::new();
    sources.insert(String::new(), (display.clone(), source.clone()));
    let natives = native_modules();
    let mut linked: HashSet<String> = HashSet::new();
    let mut pending: Vec<String> = module_deps(&entry);
    
    // Modules that are unsafe in safe mode.
    let dangerous: &[&str] = &["ffi", "sys", "process", "socket", "requests", "http", "net"];
    
    while let Some(name) = pending.pop() {
        if !linked.insert(name.clone()) {
            continue;
        }
        // In safe mode, skip dangerous native modules entirely.
        if safe_mode && dangerous.contains(&name.as_str()) {
            continue;
        }
        // `ZEN_NATIVE_ONLY=<mod>` bypasses a `.z` tier for one module, so the
        // Rust implementation can be compared against its pure counterpart.
        if native_only(&name) {
            if let Some(unit) = natives.get(&name) {
                modules.push((name, unit.clone()));
                continue;
            }
        }
        // A `.z` tier normally loses to a same-named native: host modules like
        // `fs` have no source file at all, and a Rust one is authoritative.
        // `ZEN_PURE = true` reverses that for the modules that ship both.
        let (path, src) = match resolve_module(&name, &base_dir, &search, overlay) {
            Some(found) => found,
            None => {
                if let Some(unit) = natives.get(&name) {
                    modules.push((name, unit.clone()));
                    continue;
                }
                return Err(format!("Z0324: module '{name}' not found"));
            }
        };
        let module = compile_source(&path, &src)?;
        pending.extend(module_deps(&module));
        let mut unit: ModuleUnit = module.into();
        // The native tier is registered under `<name>.native` rather than
        // `<name>`, so the pure module's own `Import` of `<name>` cannot resolve
        // to the dictionary it is still building.
        if declares_zen_pure(&src) {
            if let Some(native) = natives.get(&name) {
                let key = native_tier_key(&name);
                attach_native_tier(&mut unit, &key);
                modules.push((key, native.clone()));
            }
        }
        modules.push((name.clone(), unit));
        sources.insert(name, (path, src));
    }
    Ok(LinkedProgram { entry, modules, sources, safe_mode })
}

/// True when `ZEN_NATIVE_ONLY` names this module.
fn native_only(name: &str) -> bool {
    std::env::var("ZEN_NATIVE_ONLY")
        .map(|v| v.split(',').any(|m| m.trim() == name))
        .unwrap_or(false)
}

/// The registry key a pure module's native tier is linked under.
fn native_tier_key(name: &str) -> String {
    format!("{name}.native")
}

/// Whether a `.z` source declares `ZEN_PURE = true` at its top level.
///
/// Read from the AST, not the source text: every pure stdlib header names the
/// symbol in its own comment, so a textual scan would match all of them. The
/// declaration is a bare assignment (`ZEN_PURE = true`), not a `let`, and only a
/// plain `=` counts — `ZEN_PURE += true` would not be opting in.
fn declares_zen_pure(source: &str) -> bool {
    let lx = lex(source, FileId(0));
    let mut parser = Parser::new(lx);
    let module = match parser.parse_module() {
        Ok(m) => m,
        Err(_) => return false,
    };
    let is_true = |e: &Expr| matches!(e, Expr::Literal(Literal::Bool(true)));
    module.items.iter().any(|item| match item {
        Stmt::Let { name, value, .. } | Stmt::Const { name, value, .. } => {
            name == "ZEN_PURE" && is_true(value)
        }
        Stmt::Assign {
            target, op, value, ..
        } => {
            matches!(&**target, Expr::Ident { name, .. } if name == "ZEN_PURE")
                && matches!(op, AssignOp::Assign)
                && is_true(value)
        }
        _ => false,
    })
}

/// Give a pure-Zen module the Rust tier of the same name as its `native`
/// member, so `<mod>["native"]` reaches it.
///
/// The two instructions are placed after every one of the module's own
/// bindings, into a register above `num_locals` so nothing the module already
/// wrote is disturbed.
fn attach_native_tier(unit: &mut ModuleUnit, native_key: &str) {
    let slot = unit.main.num_globals;
    unit.main.num_globals += 1;

    // Module bodies compile to a bare register window with no trailing `Return`
    // — the frame simply runs off the end — so `num_locals` is the first
    // register free of the module's own temporaries.
    let reg = unit.main.num_locals;
    unit.main.num_locals += 1;
    if unit.main.max_reg < unit.main.num_locals {
        unit.main.max_reg = unit.main.num_locals;
    }

    let constant = unit.main.constants.len();
    unit.main
        .constants
        .push(zen_vm::Value::string(native_key.to_string()));

    let import = Instruction::new(Opcode::Import, reg as u8, constant as u8, 0);
    let store = Instruction::new(Opcode::SetGlobal, slot as u8, reg as u8, 0);

    // An explicit `return` anywhere in the body ends the frame, and the export
    // dictionary is published there, so anything after it would be discarded.
    // Insert before the last one; with no `return` at all, append.
    match unit
        .main
        .code
        .iter()
        .rposition(|i| i.op() == Opcode::Return)
    {
        Some(at) => {
            unit.main.code.insert(at, import);
            unit.main.code.insert(at + 1, store);
        }
        None => {
            unit.main.code.push(import);
            unit.main.code.push(store);
        }
    }
    unit.exports.push(("native".to_string(), slot));
}

/// Dotted module names referenced by `Import` opcodes anywhere in `module`,
/// including nested function prototypes.
pub fn module_deps(module: &CompiledModule) -> Vec<String> {
    fn scan(proto: &FunctionProto, out: &mut Vec<String>) {
        for ins in &proto.code {
            if ins.op() != Opcode::Import {
                continue;
            }
            if let Some(name) = proto
                .constants
                .get(ins.b() as usize)
                .and_then(|v| v.as_str())
            {
                out.push(name.to_string());
            }
        }
    }
    let mut out = Vec::new();
    scan(&module.main, &mut out);
    for child in &module.children {
        scan(child, &mut out);
    }
    out
}

/// Directories searched for module sources: the entry's own directory, then
/// every ancestor that contains a `std/` directory (so `tests/cases/x.z` finds
/// the workspace `std/`), then the process working directory.
fn search_dirs(entry_path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(parent) = entry_path.parent() {
        dirs.push(parent.to_path_buf());
        for ancestor in parent.ancestors() {
            let std = ancestor.join("std");
            if std.is_dir() {
                dirs.push(ancestor.to_path_buf());
                break;
            }
        }
    }
    for dir in default_search_dirs() {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs
}

/// Directories searched when there is no entry path (an embedded `zen build`
/// binary): the working directory and the installed binary's `std/`.
fn default_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd);
    }
    // Inline programs (`zen -e`) and probes run from arbitrary directories;
    // anchor the search to the installed binary so `std/` still resolves.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            for ancestor in parent.ancestors() {
                if ancestor.join("std").is_dir() {
                    let dir = ancestor.to_path_buf();
                    if !dirs.contains(&dir) {
                        dirs.push(dir);
                    }
                    break;
                }
            }
        }
    }
    dirs
}

/// Resolve a dotted module name to a source file. Within each search directory
/// try `a/b.z`, the package form `a/b/__init__.z`, then the same two under that
/// directory's `std/` subdirectory.
///
/// Source resolution only: whether a `.z` tier or a native one wins is
/// [`link`]'s decision, so this never consults `native_modules()`.
fn resolve_module(
    name: &str,
    base_dir: &Path,
    search: &[PathBuf],
    overlay: &HashMap<String, String>,
) -> Option<(String, String)> {
    // An embedded program's own modules win over anything on disk.
    if let Some(src) = overlay.get(name) {
        return Some((format!("<embedded:{name}>"), src.clone()));
    }
    let rel = name.replace('.', "/");
    let mut roots: Vec<&Path> = vec![base_dir];
    for dir in search {
        if !roots.contains(&dir.as_path()) {
            roots.push(dir.as_path());
        }
    }
    for root in roots {
        for dir in [root.to_path_buf(), root.join("std")] {
            for candidate in [
                dir.join(format!("{rel}.z")),
                dir.join(&rel).join("__init__.z"),
            ] {
                if let Ok(src) = fs::read_to_string(&candidate) {
                    return Some((candidate.to_string_lossy().into_owned(), src));
                }
            }
        }
    }
    None
}
