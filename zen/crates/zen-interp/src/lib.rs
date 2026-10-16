//! M3 tree-walking reference interpreter.
//!
//! This is the differential oracle for the register VM (M4). It evaluates the
//! same AST the compiler lowers, and it deliberately shares every piece of
//! value semantics with the VM through [`zen_vm::ops`] — arithmetic,
//! comparison, indexing, stringification and native dispatch. The only thing
//! this crate adds is *evaluation order / control flow*: lexical scopes,
//! closures and statement execution.
//!
//! Data values are the VM's [`Value`]; function values cannot live in a
//! [`Value`] (the VM represents functions as bytecode prototypes), so the
//! interpreter keeps them in `Ev::Func` and, when a function must be stored in
//! a container, round-trips it through a `handle` value backed by a side table.
#![deny(unsafe_code)]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};

use zen_syntax::ast::*;
use zen_syntax::span::Span;
use zen_vm::errors::{VmError, VmResult};
use zen_vm::gc::{new_dict, new_handle, new_list, new_set, new_tuple};
use zen_vm::object::ObjKind;
use zen_vm::ops;
use zen_vm::value::{DictKey, Value};

#[allow(dead_code)]
struct DummyVm<'a> {
    output: &'a mut String,
}
impl<'a> DummyVm<'a> {
    #[allow(dead_code)]
    fn append_output(&mut self, s: &str) {
        self.output.push_str(s);
    }
}

/// Maximum nested calls before a `StackOverflow`, matching the VM.
pub const MAX_DEPTH: usize = 1024;

// ── values ───────────────────────────────────────────────────────────────────

/// A native (non-AST) callable, e.g. a member of a native module.
pub type NativeFn = Rc<dyn Fn(&[Value]) -> Result<Value, VmError>>;

/// A native module namespace (`import string as stringmod`).
#[derive(Default)]
pub struct NativeModule {
    pub members: HashMap<String, Ev>,
}

/// A class blueprint, shared by all of its instances.
pub struct ClassDef {
    pub name: String,
    pub parent: Option<Rc<ClassDef>>,
    pub methods: HashMap<String, Rc<Closure>>,
    /// Field defaults (`var field = default`), materialised per instance.
    pub fields: IndexMap<String, Ev>,
    pub init: Option<Rc<Closure>>,
    /// Environment the class body was evaluated in (for field defaults).
    pub def_env: Env,
}

/// A live instance: a pointer to its class plus its own field storage.
pub struct Instance {
    pub class: Rc<ClassDef>,
    pub fields: IndexMap<String, Ev>,
}

impl Instance {
    /// Resolve a method through the inheritance chain.
    fn find_method(&self, name: &str) -> Option<Rc<Closure>> {
        class_find_method(&self.class, name)
    }

    /// The class in the chain that defines `cl`, so `super` can be resolved.
    fn owner_of(&self, cl: &Rc<Closure>) -> Option<Rc<ClassDef>> {
        let mut cur = Some(self.class.clone());
        while let Some(c) = cur {
            let owns = c.init.as_ref().is_some_and(|i| Rc::ptr_eq(i, cl))
                || c.methods.values().any(|m| Rc::ptr_eq(m, cl));
            if owns {
                return Some(c);
            }
            cur = c.parent.clone();
        }
        None
    }
}

/// Look up a method starting at `class` and walking up its parents.
fn class_find_method(class: &Rc<ClassDef>, name: &str) -> Option<Rc<Closure>> {
    let mut cur = Some(class.clone());
    while let Some(c) = cur {
        if let Some(m) = c.methods.get(name) {
            return Some(m.clone());
        }
        cur = c.parent.clone();
    }
    None
}

/// An interpreter value: VM data, an interpreter-only closure, a class or
/// instance, a bound method, or a native function/module supplied by the host.
#[derive(Clone)]
pub enum Ev {
    Data(Value),
    Func(Rc<Closure>),
    /// `obj.method` where `obj` is an instance: `self` is already bound.
    BoundMethod {
        receiver: Rc<RefCell<Instance>>,
        method: Rc<Closure>,
    },
    Class(Rc<ClassDef>),
    Instance(Rc<RefCell<Instance>>),
    Native {
        name: String,
        f: NativeFn,
    },
    Module(Rc<NativeModule>),
    /// A foreign (CPython) handle from `pymport`: an opaque id into the
    /// linked Python bridge's registry. Attribute access and calls route
    /// back through the bridge; Zen never inspects the referent.
    Foreign(u32),
}

impl Ev {
    fn data(&self) -> Value {
        match self {
            Ev::Data(v) => *v,
            _ => Value::nil(),
        }
    }
}

/// A captured function: params, body, and the lexical environment it closed over.
pub struct Closure {
    pub name: Option<String>,
    pub params: Vec<Param>,
    pub body: Body,
    pub env: Env,
}

pub enum Body {
    /// A block body; `auto_return` makes its last expression statement the
    /// result when there is no explicit `return`. Every block form (`func`
    /// declarations, methods, `fn` expressions, arrow blocks) sets it.
    Block { block: Block, auto_return: bool },
    /// An expression body (`lambda x: ...`, `(x) => expr`).
    Expr(Box<Expr>),
}

// ── scopes ───────────────────────────────────────────────────────────────────

pub type Env = Rc<RefCell<Scope>>;

pub struct Scope {
    vars: HashMap<String, Ev>,
    parent: Option<Env>,
}

impl Scope {
    fn root() -> Env {
        Rc::new(RefCell::new(Scope {
            vars: HashMap::new(),
            parent: None,
        }))
    }

    fn child(parent: &Env) -> Env {
        Rc::new(RefCell::new(Scope {
            vars: HashMap::new(),
            parent: Some(parent.clone()),
        }))
    }
}

// ── control flow ─────────────────────────────────────────────────────────────

/// Non-local exits. `Error` is catchable by `try`; the rest unwind to their
/// enclosing function or loop.
enum Abort {
    Error(VmError),
    Return(Ev),
    Break,
    Continue,
}

type X<T> = Result<T, Abort>;

// ── interpreter ──────────────────────────────────────────────────────────────

pub struct Interp {
    output: String,
    globals: Env,
    /// Side table backing `handle` values that stand in for closures stored in
    /// containers.
    funcs: RefCell<HashMap<u32, Rc<Closure>>>,
    next_handle: Cell<u32>,
    depth: usize,
    /// Directories searched for `import name` (a `name.z` file).
    module_dirs: Vec<PathBuf>,
    /// Loaded modules, cached by resolved stem so a module runs once.
    modules: RefCell<HashMap<String, Rc<NativeModule>>>,
    /// Optional evaluation budget for `eval`. `u64::MAX` (the default) is
    /// effectively unbounded; embedding hosts and fuzzers lower it so a
    /// non-terminating program returns an error instead of hanging.
    budget: Cell<u64>,
    pub exit_code: i32,
    /// Foreign (CPython) bridge for `pymport`. `None` (the default) means no
    /// embedding is linked: `pymport` and foreign dispatch fail with an
    /// actionable message. Mirrors `Vm::py_bridge`.
    pub py_bridge: Option<std::sync::Arc<dyn zen_vm::pybridge::PyBridge>>,
}

impl Default for Interp {
    fn default() -> Self {
        Self::new()
    }
}

impl Interp {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            globals: Scope::root(),
            funcs: RefCell::new(HashMap::new()),
            next_handle: Cell::new(1),
            depth: 0,
            module_dirs: default_module_dirs(),
            modules: RefCell::new(HashMap::new()),
            budget: Cell::new(u64::MAX),
            exit_code: 0,
            py_bridge: None,
        }
    }

    /// Bound evaluation to at most `budget` `eval` steps. A run that exhausts
    /// its budget returns an error instead of looping forever.
    pub fn set_step_budget(&mut self, budget: u64) {
        self.budget.set(budget);
    }

    /// Consume one unit of the optional evaluation budget.
    #[inline]
    fn tick(&self) -> X<()> {
        let b = self.budget.get();
        if b == 0 {
            return Err(Abort::Error(VmError::NativeError(
                "step budget exhausted".into(),
            )));
        }
        self.budget.set(b - 1);
        Ok(())
    }

    /// Add a directory to search for `import name` targets (`name.z`).
    pub fn add_module_dir(&mut self, dir: impl Into<PathBuf>) {
        self.module_dirs.push(dir.into());
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    pub fn take_output(&mut self) -> String {
        std::mem::take(&mut self.output)
    }

    /// Execute a parsed module. Returns the module's final value.
    pub fn run_module(&mut self, module: &Module) -> VmResult<Ev> {
        match self.exec_block(&module.items, &self.globals.clone()) {
            Ok(()) => Ok(Ev::Data(Value::nil())),
            Err(Abort::Error(e)) => Err(e),
            Err(Abort::Return(v)) => Ok(v),
            Err(Abort::Break) | Err(Abort::Continue) => {
                Err(VmError::NativeError("break/continue outside loop".into()))
            }
        }
    }

    /// Lex, parse and execute a source string.
    pub fn run_source(&mut self, source: &str) -> Result<Ev, String> {
        let lx = zen_syntax::lex(source, zen_syntax::FileId(0));
        let mut parser = zen_syntax::parser::Parser::new(lx);
        let module = parser.parse_module().map_err(|e| e.to_string())?;
        self.run_module(&module).map_err(|e| e.to_string())
    }

    /// Load and execute a `std/<name>.z` module, returning its namespace.
    ///
    /// `Ok(None)` means no file matched the search path (the caller decides
    /// whether a native fallback exists); `Err` means a file was found but
    /// failed to parse or run. Modules are cached and executed once.
    fn try_load_module(&mut self, path: &[String]) -> X<Option<Ev>> {
        let name = path.join("/");
        if let Some(m) = self.modules.borrow().get(&name) {
            return Ok(Some(Ev::Module(m.clone())));
        }
        let file = self.module_dirs.iter().find_map(|dir| {
            let p = dir.join(format!("{name}.z"));
            if p.is_file() {
                Some(p)
            } else {
                None
            }
        });
        let Some(file) = file else {
            return Ok(None);
        };
        let src = std::fs::read_to_string(&file).map_err(|e| {
            Abort::Error(VmError::NativeError(format!(
                "read {}: {e}",
                file.display()
            )))
        })?;
        // Insert an empty namespace first so an import cycle terminates rather
        // than recursing forever; it is replaced once execution completes.
        let placeholder = Rc::new(NativeModule::default());
        self.modules.borrow_mut().insert(name.clone(), placeholder);
        let lx = zen_syntax::lex(&src, zen_syntax::FileId(0));
        let mut parser = zen_syntax::parser::Parser::new(lx);
        let module = parser.parse_module().map_err(|e| {
            Abort::Error(VmError::NativeError(format!(
                "parse {}: {e}",
                file.display()
            )))
        })?;
        let scope = Scope::child(&self.globals.clone());
        self.exec_block(&module.items, &scope)?;
        let members = scope.borrow().vars.clone();
        let m = Rc::new(NativeModule { members });
        self.modules.borrow_mut().insert(name, m.clone());
        Ok(Some(Ev::Module(m)))
    }

    // ── scope helpers ────────────────────────────────────────────────────

    fn define(&self, env: &Env, name: &str, ev: Ev) {
        env.borrow_mut().vars.insert(name.to_string(), ev);
    }

    fn lookup(&self, env: &Env, name: &str) -> Option<Ev> {
        let mut cur = Some(env.clone());
        while let Some(e) = cur {
            if let Some(v) = e.borrow().vars.get(name) {
                return Some(v.clone());
            }
            cur = e.borrow().parent.clone();
        }
        None
    }

    /// Assign to an existing binding in the scope chain, or create a global.
    fn assign_name(&self, env: &Env, name: &str, ev: Ev) {
        let mut cur = Some(env.clone());
        while let Some(e) = cur {
            if e.borrow().vars.contains_key(name) {
                e.borrow_mut().vars.insert(name.to_string(), ev);
                return;
            }
            cur = e.borrow().parent.clone();
        }
        self.globals.borrow_mut().vars.insert(name.to_string(), ev);
    }

    // ── function handles ─────────────────────────────────────────────────

    fn register_func(&self, f: Rc<Closure>) -> Value {
        let id = self.next_handle.get();
        self.next_handle.set(id + 1);
        self.funcs.borrow_mut().insert(id, f);
        new_handle(id)
    }

    fn to_value(&self, ev: &Ev) -> Value {
        match ev {
            Ev::Data(v) => *v,
            Ev::Foreign(h) => zen_vm::gc::new_foreign(*h),
            Ev::Func(f) => self.register_func(f.clone()),
            Ev::BoundMethod { .. }
            | Ev::Class(_)
            | Ev::Instance(_)
            | Ev::Native { .. }
            | Ev::Module(_) => Value::nil(),
        }
    }

    /// The linked Python bridge. Unlinked builds fail here with the
    /// foreign-import error (the actionable message belongs with the
    /// PyImport essay: rebuild linked).
    fn pybridge(&self) -> Result<std::sync::Arc<dyn zen_vm::pybridge::PyBridge>, Abort> {
        self.py_bridge.clone().ok_or_else(|| {
            Abort::Error(VmError::PyImport {
                message: "pymport needs a linked Python bridge (this build has none). \
                     Rebuild with `--features python` (requires libpython)."
                    .into(),
                traceback: None,
            })
        })
    }

    /// Marshal evaluated arguments across the foreign boundary.
    fn marshal_args(&self, argv: Vec<Ev>) -> Result<Vec<zen_vm::pybridge::PyValue>, Abort> {
        argv.into_iter()
            .map(|a| {
                zen_vm::pybridge::value_to_pyvalue(self.to_value(&a))
                    .map_err(|e| Self::bridge_err(e, zen_vm::pybridge::BridgeOp::Marshal))
            })
            .collect()
    }

    /// Wrap a bridge result back into interpreter values.
    fn wrap_pyvalue(&self, v: zen_vm::pybridge::PyValue) -> Ev {
        self.from_value(zen_vm::pybridge::pyvalue_to_value(v))
    }

    /// Map a bridge failure to its operation's error variant.
    fn bridge_err(
        e: zen_vm::pybridge::PyBridgeError,
        op: zen_vm::pybridge::BridgeOp,
    ) -> Abort {
        Abort::Error(zen_vm::pybridge::into_vm_error(e, op))
    }

    #[allow(clippy::wrong_self_convention)]
    fn from_value(&self, v: Value) -> Ev {
        // Foreign handles normalize here so `Ev::Data` never carries one:
        // every consumer below only needs the `Ev::Foreign` arm.
        if let Some(h) = v.as_foreign() {
            return Ev::Foreign(h);
        }
        if v.kind() == Some(ObjKind::Handle) {
            if let Some(id) = v.as_handle_id() {
                if let Some(f) = self.funcs.borrow().get(&id) {
                    return Ev::Func(f.clone());
                }
            }
        }
        Ev::Data(v)
    }

    fn truthy(&self, ev: &Ev) -> bool {
        match ev {
            Ev::Data(v) => ops::truthy(*v),
            _ => true,
        }
    }

    /// Evaluate `e` and convert it to a VM [`Value`] (round-tripping closures
    /// through the handle table). A helper so callers never hold an immutable
    /// borrow of `self` across a mutable evaluation.
    fn eval_to_value(&mut self, e: &Expr, env: &Env) -> X<Value> {
        let v = self.eval(e, env)?;
        Ok(self.to_value(&v))
    }

    fn eval_truthy(&mut self, e: &Expr, env: &Env) -> X<bool> {
        let v = self.eval(e, env)?;
        Ok(self.truthy(&v))
    }

    /// Materialise an iterable value into a `Vec<Value>` using the shared
    /// iteration protocol.
    fn iter_values(&mut self, ev: &Ev) -> X<Vec<Value>> {
        let value = self.to_value(ev);
        let len = ops::iter_len(value) as i64;
        let mut out = Vec::with_capacity(len as usize);
        for i in 0..len {
            out.push(ops::iter_get(value, i).map_err(Abort::Error)?);
        }
        Ok(out)
    }

    // ── statements ───────────────────────────────────────────────────────

    fn exec_block(&mut self, stmts: &[Stmt], env: &Env) -> X<()> {
        for s in stmts {
            self.exec_stmt(s, env)?;
        }
        Ok(())
    }

    fn exec_stmt(&mut self, stmt: &Stmt, env: &Env) -> X<()> {
        match stmt {
            Stmt::Let { name, value, .. } | Stmt::Shared { name, value, .. } => {
                let v = self.eval(value, env)?;
                self.define(env, name, v);
                Ok(())
            }
            Stmt::Const { name, value, .. } => {
                let v = self.eval(value, env)?;
                self.define(env, name, v);
                Ok(())
            }
            Stmt::Assign {
                target,
                op,
                value,
                span,
            } => {
                self.exec_assign(target, op, value, env, *span)?;
                Ok(())
            }
            Stmt::Expr(e) => {
                self.eval(e, env)?;
                Ok(())
            }
            Stmt::Print { args, sep, end, .. } => {
                let sep = match sep {
                    Some(e) => self.eval_str(e, env)?,
                    None => " ".to_string(),
                };
                let end = match end {
                    Some(e) => self.eval_str(e, env)?,
                    None => "\n".to_string(),
                };
                let mut parts = Vec::with_capacity(args.len());
                for a in args {
                    let v = self.eval(a, env)?;
                    parts.push(ops::stringify(self.to_value(&v)));
                }
                self.output.push_str(&parts.join(&sep));
                self.output.push_str(&end);
                Ok(())
            }
            Stmt::If {
                cond,
                then_branch,
                elifs,
                else_branch,
                ..
            } => {
                if self.eval_truthy(cond, env)? {
                    self.exec_block(&then_branch.stmts, env)
                } else {
                    for (c, b) in elifs {
                        if self.eval_truthy(c, env)? {
                            return self.exec_block(&b.stmts, env);
                        }
                    }
                    match else_branch {
                        Some(b) => self.exec_block(&b.stmts, env),
                        None => Ok(()),
                    }
                }
            }
            Stmt::While { cond, body, .. } => {
                while self.eval_truthy(cond, env)? {
                    match self.exec_block(&body.stmts, env) {
                        Ok(()) => {}
                        Err(Abort::Break) => break,
                        Err(Abort::Continue) => continue,
                        Err(e) => return Err(e),
                    }
                }
                Ok(())
            }
            Stmt::For {
                vars, iter, body, ..
            } => {
                let seq = self.eval(iter, env)?;
                let value = self.to_value(&seq);
                let len = ops::iter_len(value) as i64;
                for i in 0..len {
                    let item = ops::iter_get(value, i).map_err(Abort::Error)?;
                    let item = self.from_value(item);
                    let child = Scope::child(env);
                    self.bind_loop_vars(&child, vars, item)?;
                    match self.exec_block(&body.stmts, &child) {
                        Ok(()) => {}
                        Err(Abort::Break) => break,
                        Err(Abort::Continue) => continue,
                        Err(e) => return Err(e),
                    }
                }
                Ok(())
            }
            Stmt::Function {
                name, params, body, ..
            } => {
                let f = Rc::new(Closure {
                    name: Some(name.clone()),
                    params: params.clone(),
                    body: Body::Block {
                        block: body.clone(),
                        auto_return: true,
                    },
                    env: env.clone(),
                });
                self.define(env, name, Ev::Func(f));
                Ok(())
            }
            Stmt::Return { value, .. } => {
                let v = match value {
                    Some(e) => self.eval(e, env)?,
                    None => Ev::Data(Value::nil()),
                };
                Err(Abort::Return(v))
            }
            Stmt::Break { .. } => Err(Abort::Break),
            Stmt::Continue { .. } => Err(Abort::Continue),
            Stmt::Throw { value, .. } => {
                let v = self.eval(value, env)?;
                Err(Abort::Error(VmError::UserError(ops::stringify(
                    self.to_value(&v),
                ))))
            }
            Stmt::Try {
                body,
                catches,
                finally,
                ..
            } => {
                let result = self.exec_block(&body.stmts, env);
                let result = match result {
                    Err(Abort::Error(e)) => {
                        let err_value = zen_vm::gc::new_error(
                            std::sync::Arc::from(e.to_string()),
                            0,
                            Value::nil(),
                        );
                        let mut handled: Option<X<()>> = None;
                        for c in catches {
                            if catch_matches(c, &e) {
                                let child = Scope::child(env);
                                if let Some(b) = &c.binding {
                                    self.define(&child, b, Ev::Data(err_value));
                                }
                                handled = Some(self.exec_block(&c.body.stmts, &child));
                                break;
                            }
                        }
                        match handled {
                            Some(r) => r,
                            None => Err(Abort::Error(e)),
                        }
                    }
                    other => other,
                };
                if let Some(f) = finally {
                    self.exec_block(&f.stmts, env)?;
                }
                result
            }
            Stmt::Class {
                name,
                parent,
                members,
                ..
            } => self.exec_class(name, parent, members, env),
            Stmt::Import { specs, .. } => {
                for spec in specs {
                    // `string` has a native fast path that mirrors `std/string.z`;
                    // prefer it so the oracle does not depend on the std file.
                    let module = if spec.path.last().map(String::as_str) == Some("string") {
                        Ev::Module(Rc::new(native_string_module()))
                    } else {
                        match self.try_load_module(&spec.path)? {
                            Some(m) => m,
                            None => {
                                return Err(Abort::Error(unsupported(
                                    spec.span,
                                    &format!("import {}", spec.path.join(".")),
                                )))
                            }
                        }
                    };
                    let binding = spec
                        .alias
                        .clone()
                        .or_else(|| spec.path.last().cloned())
                        .unwrap_or_default();
                    self.define(env, &binding, module);
                }
                Ok(())
            }
            Stmt::Pymport { path, alias, span } => {
                let name = path.join(".");
                if name.is_empty() {
                    return Err(Abort::Error(unsupported(*span, "pymport needs a module name")));
                }
                let bridge = self.pybridge()?;
                let handle = bridge.import_module(&name).map_err(|e| Self::bridge_err(e, zen_vm::pybridge::BridgeOp::Import))?;
                // Bind the alias, the single name, or (Python-style) the root
                // package for dotted paths.
                let binding = alias.clone().unwrap_or_else(|| {
                    path.first().cloned().unwrap_or_else(|| name.clone())
                });
                self.define(env, &binding, Ev::Foreign(handle));
                Ok(())
            }
            Stmt::FromImport { path, items, span } => {
                let module = self.try_load_module(path)?.ok_or_else(|| {
                    Abort::Error(unsupported(*span, &format!("from {}", path.join("."))))
                })?;
                let Ev::Module(m) = &module else {
                    return Err(Abort::Error(VmError::NativeError(
                        "from-import target is not a module".into(),
                    )));
                };
                match items {
                    ImportItems::All => {
                        let binding = path.last().cloned().unwrap_or_default();
                        self.define(env, &binding, module.clone());
                    }
                    ImportItems::Names(names) => {
                        for n in names {
                            let member = m.members.get(&n.name).cloned().ok_or_else(|| {
                                Abort::Error(VmError::NativeError(format!(
                                    "module {} has no member {}",
                                    path.join("."),
                                    n.name
                                )))
                            })?;
                            let binding = n.alias.clone().unwrap_or_else(|| n.name.clone());
                            self.define(env, &binding, member);
                        }
                    }
                }
                Ok(())
            }
            Stmt::Include { span, .. } => Err(Abort::Error(unsupported(*span, "include"))),
            Stmt::Match { subject, arms, .. } => {
                let subj = self.eval(subject, env)?;
                self.exec_match_arms(&subj, arms, env)?;
                Ok(())
            }
            Stmt::When { arms, .. } => {
                self.eval_when(arms, env)?;
                Ok(())
            }
        }
    }

    fn bind_loop_vars(&mut self, env: &Env, vars: &[String], item: Ev) -> X<()> {
        if vars.len() == 1 {
            self.define(env, &vars[0], item);
            return Ok(());
        }
        let value = self.to_value(&item);
        let len = ops::iter_len(value) as i64;
        if len < vars.len() as i64 {
            return Err(Abort::Error(VmError::ValueError(
                "not enough values to unpack in for".into(),
            )));
        }
        for (i, name) in vars.iter().enumerate() {
            let v = ops::iter_get(value, i as i64).map_err(Abort::Error)?;
            let ev = self.from_value(v);
            self.define(env, name, ev);
        }
        Ok(())
    }

    fn exec_assign(
        &mut self,
        target: &Expr,
        op: &AssignOp,
        value: &Expr,
        env: &Env,
        span: Span,
    ) -> X<()> {
        let rhs = self.eval(value, env)?;
        match target {
            Expr::Ident { name, .. } => {
                let cur = self.lookup(env, name).unwrap_or(Ev::Data(Value::nil()));
                let new = self.assign_value(op, cur, rhs, span)?;
                self.assign_name(env, name, new);
                Ok(())
            }
            Expr::Index { object, index, .. } => {
                let obj = self.eval(object, env)?;
                let key = self.eval(index, env)?;
                let base = self.to_value(&obj);
                let key = self.to_value(&key);
                let new = if needs_current(op) {
                    let cur = self.from_value(ops::get_index(base, key).map_err(Abort::Error)?);
                    self.assign_value(op, cur, rhs, span)?
                } else {
                    rhs
                };
                let updated =
                    ops::set_index(base, key, self.to_value(&new)).map_err(Abort::Error)?;
                self.store_place(object, Ev::Data(updated), env)
            }
            Expr::Field { object, field, .. } => {
                let obj = self.eval(object, env)?;
                if let Ev::Instance(inst) = &obj {
                    let new = if needs_current(op) {
                        let cur = inst
                            .borrow()
                            .fields
                            .get(field)
                            .cloned()
                            .unwrap_or(Ev::Data(Value::nil()));
                        self.assign_value(op, cur, rhs, span)?
                    } else {
                        rhs
                    };
                    inst.borrow_mut().fields.insert(field.clone(), new);
                    return Ok(());
                }
                // Foreign attribute write through the bridge. Compound
                // assignment reads back through the bridge first, mirroring
                // the instance arm above.
                if let Ev::Foreign(h) = &obj {
                    let bridge = self.pybridge()?;
                    let new = if needs_current(op) {
                        let cur = bridge
                            .getattr(*h, field)
                            .map_err(|e| Self::bridge_err(e, zen_vm::pybridge::BridgeOp::GetAttr))?;
                        self.assign_value(op, self.wrap_pyvalue(cur), rhs, span)?
                    } else {
                        rhs
                    };
                    let pv = zen_vm::pybridge::value_to_pyvalue(self.to_value(&new))
                        .map_err(|e| Self::bridge_err(e, zen_vm::pybridge::BridgeOp::Marshal))?;
                    bridge
                        .setattr(*h, field, pv)
                        .map_err(|e| Self::bridge_err(e, zen_vm::pybridge::BridgeOp::SetAttr))?;
                    return Ok(());
                }
                let base = self.to_value(&obj);
                let name = Value::string(field.clone());
                let new = if needs_current(op) {
                    let cur = self.from_value(ops::get_field(base, name).map_err(Abort::Error)?);
                    self.assign_value(op, cur, rhs, span)?
                } else {
                    rhs
                };
                let updated =
                    ops::set_field(base, name, self.to_value(&new)).map_err(Abort::Error)?;
                self.store_place(object, Ev::Data(updated), env)
            }
            _ => Err(Abort::Error(unsupported(span, "assignment target"))),
        }
    }

    fn assign_value(&mut self, op: &AssignOp, cur: Ev, rhs: Ev, span: Span) -> X<Ev> {
        match op {
            AssignOp::Assign => Ok(rhs),
            AssignOp::Inc => self.binary(BinaryOp::Add, cur, Ev::Data(Value::int(1)), span),
            AssignOp::Dec => self.binary(BinaryOp::Sub, cur, Ev::Data(Value::int(1)), span),
            _ => {
                let bin = assign_binop(op).expect("compound assignment has a binary op");
                self.binary(bin, cur, rhs, span)
            }
        }
    }

    fn exec_class(
        &mut self,
        name: &str,
        parent: &Option<String>,
        members: &[ClassMember],
        env: &Env,
    ) -> X<()> {
        let parent_class = match parent {
            Some(p) => match self.lookup(env, p) {
                Some(Ev::Class(c)) => Some(c),
                Some(_) => {
                    return Err(Abort::Error(VmError::TypeError(format!(
                        "`{p}` is not a class"
                    ))))
                }
                None => return Err(Abort::Error(VmError::NameError(p.clone()))),
            },
            None => None,
        };
        let mut methods: HashMap<String, Rc<Closure>> = HashMap::new();
        let mut fields: IndexMap<String, Ev> = IndexMap::new();
        let mut init = None;
        for m in members {
            match m.kind {
                ClassMemberKind::Init => {
                    init = Some(Rc::new(Closure {
                        name: Some("init".to_string()),
                        params: m.params.clone(),
                        body: Body::Block {
                            block: m.body.clone(),
                            auto_return: true,
                        },
                        env: env.clone(),
                    }));
                }
                ClassMemberKind::Method => {
                    methods.insert(
                        m.name.clone(),
                        Rc::new(Closure {
                            name: Some(m.name.clone()),
                            params: m.params.clone(),
                            body: Body::Block {
                                block: m.body.clone(),
                                auto_return: true,
                            },
                            env: env.clone(),
                        }),
                    );
                }
                ClassMemberKind::Field => {
                    let default = match &m.default {
                        Some(e) => self.eval(e, env)?,
                        None => Ev::Data(Value::nil()),
                    };
                    fields.insert(m.name.clone(), default);
                }
            }
        }
        let class = Rc::new(ClassDef {
            name: name.to_string(),
            parent: parent_class,
            methods,
            fields,
            init,
            def_env: env.clone(),
        });
        self.define(env, name, Ev::Class(class));
        Ok(())
    }

    /// Allocate an instance, seed its fields from the class chain's defaults,
    /// then run `init` (if any) with `self` bound. `init`'s return is discarded.
    fn instantiate(&mut self, class: Rc<ClassDef>, args: Vec<Ev>) -> X<Ev> {
        let mut chain = Vec::new();
        let mut cur = Some(class.clone());
        while let Some(c) = cur {
            chain.push(c.clone());
            cur = c.parent.clone();
        }
        let mut fields: IndexMap<String, Ev> = IndexMap::new();
        for c in chain.iter().rev() {
            for (k, v) in &c.fields {
                fields.insert(k.clone(), v.clone());
            }
        }
        let inst = Rc::new(RefCell::new(Instance {
            class: class.clone(),
            fields,
        }));
        if let Some(init) = class.init.clone() {
            self.call_method(init, Ev::Instance(inst.clone()), args)?;
        }
        Ok(Ev::Instance(inst))
    }

    /// Write `ev` back to a place (identifier / index / field), if assignable.
    fn store_place(&mut self, target: &Expr, ev: Ev, env: &Env) -> X<()> {
        match target {
            Expr::Ident { name, .. } => {
                self.assign_name(env, name, ev);
                Ok(())
            }
            Expr::Index { object, index, .. } => {
                let obj = self.eval(object, env)?;
                let key = self.eval(index, env)?;
                let base = self.to_value(&obj);
                let key = self.to_value(&key);
                let v = self.to_value(&ev);
                let updated = ops::set_index(base, key, v).map_err(Abort::Error)?;
                self.store_place(object, Ev::Data(updated), env)
            }
            Expr::Field { object, field, .. } => {
                let obj = self.eval(object, env)?;
                if let Ev::Instance(inst) = &obj {
                    inst.borrow_mut().fields.insert(field.clone(), ev);
                    return Ok(());
                }
                let base = self.to_value(&obj);
                let name = Value::string(field.clone());
                let v = self.to_value(&ev);
                let updated = ops::set_field(base, name, v).map_err(Abort::Error)?;
                self.store_place(object, Ev::Data(updated), env)
            }
            _ => Ok(()),
        }
    }

    fn exec_match_arms(&mut self, subject: &Ev, arms: &[MatchArm], env: &Env) -> X<Ev> {
        for arm in arms {
            let child = Scope::child(env);
            if self.pattern_matches(&arm.pattern, subject, &child)? {
                return self.arm_body(&arm.body, &child);
            }
        }
        Ok(Ev::Data(Value::nil()))
    }

    fn arm_body(&mut self, body: &BlockOrExpr, env: &Env) -> X<Ev> {
        match body {
            BlockOrExpr::Expr(e) => self.eval(e, env),
            BlockOrExpr::Block(b) => {
                self.exec_block(&b.stmts, env)?;
                Ok(Ev::Data(Value::nil()))
            }
        }
    }

    // ── pattern matching ─────────────────────────────────────────────────

    fn pattern_matches(&mut self, pat: &Pattern, subject: &Ev, env: &Env) -> X<bool> {
        match pat {
            Pattern::Wildcard(_) => Ok(true),
            Pattern::Binding { name, .. } => {
                self.define(env, name, subject.clone());
                Ok(true)
            }
            Pattern::Literal { value, .. } => {
                let lit = self.literal(value);
                Ok(self.values_eq(&subject.data(), &lit))
            }
            Pattern::Tuple { elems, .. } => {
                let v = self.to_value(subject);
                let len = ops::iter_len(v);
                if len != elems.len() {
                    return Ok(false);
                }
                for (i, p) in elems.iter().enumerate() {
                    let item = ops::iter_get(v, i as i64).map_err(Abort::Error)?;
                    if !self.pattern_matches(p, &self.from_value(item), env)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Pattern::List { elems, rest, .. } => {
                let v = self.to_value(subject);
                let len = ops::iter_len(v);
                if rest.is_none() && len != elems.len() {
                    return Ok(false);
                }
                if rest.is_some() && len < elems.len() {
                    return Ok(false);
                }
                for (i, p) in elems.iter().enumerate() {
                    let item = ops::iter_get(v, i as i64).map_err(Abort::Error)?;
                    if !self.pattern_matches(p, &self.from_value(item), env)? {
                        return Ok(false);
                    }
                }
                if let Some(r) = rest {
                    let mut tail = Vec::new();
                    for i in elems.len()..len {
                        tail.push(ops::iter_get(v, i as i64).map_err(Abort::Error)?);
                    }
                    let ev = Ev::Data(new_list(tail));
                    if !self.pattern_matches(r, &ev, env)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Pattern::Guard { pat, guards, .. } => {
                if !self.pattern_matches(pat, subject, env)? {
                    return Ok(false);
                }
                for g in guards {
                    if !self.eval_truthy(g, env)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Pattern::Or { alts, .. } => {
                for a in alts {
                    if self.pattern_matches(a, subject, env)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Pattern::Bind { pat, name, .. } => {
                if self.pattern_matches(pat, subject, env)? {
                    self.define(env, name, subject.clone());
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Pattern::Constructor { path, fields, span } => {
                if fields.is_empty() && path.len() == 1 {
                    // A bare capitalised name is a value tested by equality
                    // against a bound constant of that name.
                    if let Some(bound) = self.lookup(env, &path[0]) {
                        return Ok(self.values_eq(&subject.data(), &bound.data()));
                    }
                }
                Err(Abort::Error(unsupported(*span, "constructor pattern")))
            }
            Pattern::Dict { .. } => Err(Abort::Error(VmError::NativeError(
                "dict patterns are not supported yet".into(),
            ))),
            Pattern::Range {
                lo,
                hi,
                lo_incl,
                hi_incl,
                ..
            } => {
                let v = self.to_value(subject);
                let Some(n) = v.as_int() else {
                    return Ok(false);
                };
                let lo = self.eval(lo, env)?;
                let hi = self.eval(hi, env)?;
                let (Some(lo), Some(hi)) = (lo.data().as_int(), hi.data().as_int()) else {
                    return Ok(false);
                };
                let lower = if *lo_incl { n >= lo } else { n > lo };
                let upper = if *hi_incl { n <= hi } else { n < hi };
                Ok(lower && upper)
            }
            Pattern::Condition { cond, .. } => {
                // `when { _: ... }` may arrive as a `Condition` holding the bare
                // `_` identifier; it is the always-matching fallback arm.
                if matches!(cond.as_ref(), Expr::Ident { name, .. } if name == "_") {
                    return Ok(true);
                }
                Ok(self.eval_truthy(cond, env)?)
            }
        }
    }

    // ── expressions ──────────────────────────────────────────────────────

    fn eval(&mut self, expr: &Expr, env: &Env) -> X<Ev> {
        self.tick()?;
        match expr {
            Expr::Literal(l) => Ok(Ev::Data(self.literal(l))),
            Expr::Ident { name, .. } => self.lookup(env, name).ok_or_else(|| {
                Abort::Error(VmError::NameError(format!("name '{name}' is not defined")))
            }),
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => {
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    let l = self.eval(left, env)?;
                    let lt = self.truthy(&l);
                    if (*op == BinaryOp::And && !lt) || (*op == BinaryOp::Or && lt) {
                        return Ok(Ev::Data(Value::bool(lt)));
                    }
                    let r = self.eval(right, env)?;
                    return Ok(Ev::Data(Value::bool(self.truthy(&r))));
                }
                let l = self.eval(left, env)?;
                let r = self.eval(right, env)?;
                self.binary(*op, l, r, *span)
            }
            Expr::Unary { op, operand, span } => {
                let v = self.eval(operand, env)?;
                self.unary(*op, v, *span)
            }
            Expr::Ternary {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                if self.eval_truthy(cond, env)? {
                    self.eval(then_branch, env)
                } else {
                    self.eval(else_branch, env)
                }
            }
            Expr::NullCoalesce { left, right, .. } => {
                let l = self.eval(left, env)?;
                if l.data().is_nil() {
                    self.eval(right, env)
                } else {
                    Ok(l)
                }
            }
            Expr::Chain {
                ops: chain_ops,
                operands,
                ..
            } => {
                let mut prev = self.eval(&operands[0], env)?;
                for (i, op) in chain_ops.iter().enumerate() {
                    let next = self.eval(&operands[i + 1], env)?;
                    if !self.compare(*op, &prev, &next)? {
                        return Ok(Ev::Data(Value::bool(false)));
                    }
                    prev = next;
                }
                Ok(Ev::Data(Value::bool(true)))
            }
            Expr::IfExpr {
                arms, otherwise, ..
            } => {
                for (c, b) in arms {
                    if self.eval_truthy(c, env)? {
                        return self.eval_block_value(b, env);
                    }
                }
                match otherwise {
                    Some(b) => self.eval_block_value(b, env),
                    None => Ok(Ev::Data(Value::nil())),
                }
            }
            Expr::Call { callee, args, .. } => self.eval_call(callee, args, env),
            Expr::MethodCall {
                receiver,
                method,
                args,
                span,
            } => self.eval_method(receiver, method, args, env, *span),
            Expr::Index { object, index, .. } => {
                let obj = self.eval(object, env)?;
                let key = self.eval(index, env)?;
                let v = ops::get_index(self.to_value(&obj), self.to_value(&key))
                    .map_err(Abort::Error)?;
                Ok(self.from_value(v))
            }
            Expr::Field { object, field, .. } | Expr::OptionalField { object, field, .. } => {
                let obj = self.eval(object, env)?;
                // Foreign attribute read (bridge owns its `(handle, attr)` cache).
                if let Ev::Foreign(h) = &obj {
                    let bridge = self.pybridge()?;
                    let v = bridge
                        .getattr(*h, field)
                        .map_err(|e| Self::bridge_err(e, zen_vm::pybridge::BridgeOp::GetAttr))?;
                    return Ok(self.wrap_pyvalue(v));
                }
                if let Ev::Module(m) = &obj {
                    if let Some(v) = m.members.get(field) {
                        return Ok(v.clone());
                    }
                    return Err(Abort::Error(VmError::NativeError(format!(
                        "module has no member {field}"
                    ))));
                }
                if let Ev::Instance(inst) = &obj {
                    let found = inst.borrow().fields.get(field).cloned();
                    if let Some(v) = found {
                        return Ok(v);
                    }
                    if let Some(m) = inst.borrow().find_method(field) {
                        return Ok(Ev::BoundMethod {
                            receiver: inst.clone(),
                            method: m,
                        });
                    }
                    if matches!(expr, Expr::OptionalField { .. }) {
                        return Ok(Ev::Data(Value::nil()));
                    }
                    return Err(Abort::Error(VmError::NativeError(format!(
                        "instance of {} has no field {field}",
                        inst.borrow().class.name
                    ))));
                }
                let name = Value::string(field.clone());
                match ops::get_field(self.to_value(&obj), name) {
                    Ok(v) => Ok(self.from_value(v)),
                    Err(e) => {
                        if matches!(expr, Expr::OptionalField { .. }) {
                            Ok(Ev::Data(Value::nil()))
                        } else {
                            Err(Abort::Error(e))
                        }
                    }
                }
            }
            Expr::Slice {
                object, start, end, ..
            } => {
                let obj = self.eval(object, env)?;
                let s = match start {
                    Some(e) => Some(self.eval(e, env)?.data().as_int().unwrap_or(0)),
                    None => None,
                };
                let e = match end {
                    Some(e) => Some(self.eval(e, env)?.data().as_int().unwrap_or(0)),
                    None => None,
                };
                let v = slice_value(self.to_value(&obj), s, e)?;
                Ok(self.from_value(v))
            }
            Expr::List(items) | Expr::Tuple(items) => {
                let vals = self.eval_values(items, env)?;
                let v = if matches!(expr, Expr::List(_)) {
                    new_list(vals)
                } else {
                    new_tuple(vals)
                };
                Ok(Ev::Data(v))
            }
            Expr::TupleLiteral(items) => {
                let vals = self.eval_values(items, env)?;
                Ok(Ev::Data(new_tuple(vals)))
            }
            Expr::Set(items) | Expr::SetLiteral(items) => {
                let vals = self.eval_values(items, env)?;
                let set: IndexSet<Value> = vals.into_iter().collect();
                Ok(Ev::Data(new_set(set)))
            }
            Expr::Dict(pairs) => {
                let mut map = IndexMap::new();
                for (k, v) in pairs {
                    // A bare identifier is a string key, matching the compiler.
                    let key = match k {
                        Expr::Ident { name, .. } => Value::string(name.clone()),
                        _ => self.eval_to_value(k, env)?,
                    };
                    let val = self.eval_to_value(v, env)?;
                    ops::dict_insert(&mut map, key, val);
                }
                Ok(Ev::Data(new_dict(map)))
            }
            Expr::DictLiteral(pairs) => {
                let mut map = IndexMap::new();
                for (k, v) in pairs {
                    let val = self.eval_to_value(v, env)?;
                    ops::dict_insert(&mut map, Value::string(k.clone()), val);
                }
                Ok(Ev::Data(new_dict(map)))
            }
            Expr::Lambda { params, body, .. } => {
                let f = Rc::new(Closure {
                    name: None,
                    params: params.clone(),
                    body: Body::Expr(body.clone()),
                    env: env.clone(),
                });
                Ok(Ev::Func(f))
            }
            Expr::FnExpr { params, body, .. } => {
                let f = Rc::new(Closure {
                    name: None,
                    params: params.clone(),
                    body: Body::Block {
                        block: body.clone(),
                        auto_return: true,
                    },
                    env: env.clone(),
                });
                Ok(Ev::Func(f))
            }
            Expr::ArrowFn { params, body, .. } => {
                let body = match body {
                    BlockOrExpr::Expr(e) => Body::Expr(e.clone()),
                    BlockOrExpr::Block(b) => Body::Block {
                        block: b.clone(),
                        auto_return: true,
                    },
                };
                let f = Rc::new(Closure {
                    name: None,
                    params: params.clone(),
                    body,
                    env: env.clone(),
                });
                Ok(Ev::Func(f))
            }
            Expr::InterpolatedString(parts) => {
                let mut out = String::new();
                for p in parts {
                    match p {
                        StringPart::Text(t) => out.push_str(t),
                        StringPart::Interpolation(e) => {
                            let v = self.eval(e, env)?;
                            out.push_str(&ops::stringify(self.to_value(&v)));
                        }
                    }
                }
                Ok(Ev::Data(Value::string(out)))
            }
            Expr::Range {
                start,
                end,
                inclusive,
                ..
            } => {
                let s = self.eval(start, env)?.data().as_int().unwrap_or(0);
                let e = self.eval(end, env)?.data().as_int().unwrap_or(0);
                let last = if *inclusive { e } else { e - 1 };
                let items: Vec<Value> = if last >= s {
                    (s..=last).map(Value::int_promote).collect()
                } else {
                    Vec::new()
                };
                Ok(Ev::Data(new_list(items)))
            }
            Expr::Comprehension {
                kind,
                key,
                body,
                clauses,
                ..
            } => self.eval_comprehension(*kind, key.as_deref(), body, clauses, env),
            Expr::Match { subject, arms, .. } => {
                let subj = self.eval(subject, env)?;
                self.exec_match_arms(&subj, arms, env)
            }
            Expr::When { arms, .. } => self.eval_when(arms, env),
            Expr::New { class, args, span } => {
                let class = match self.lookup(env, class) {
                    Some(Ev::Class(c)) => c,
                    Some(_) => {
                        return Err(Abort::Error(VmError::TypeError(format!(
                            "`{class}` is not a class"
                        ))))
                    }
                    None => return Err(Abort::Error(VmError::NameError(class.clone()))),
                };
                let _ = span;
                let argv = self.eval_args(args, env)?;
                self.instantiate(class, argv)
            }
            Expr::SuperCall { args, .. } => {
                let (sup, recv) = self.super_bindings(env)?;
                if let Some(init) = sup.init.clone() {
                    let argv = self.eval_args(args, env)?;
                    self.call_method(init, Ev::Instance(recv.clone()), argv)?;
                }
                Ok(Ev::Instance(recv))
            }
            Expr::SuperMethod { method, args, .. } => {
                let (sup, recv) = self.super_bindings(env)?;
                let m = class_find_method(&sup, method).ok_or_else(|| {
                    Abort::Error(VmError::NativeError(format!(
                        "super of {} has no method {method}",
                        recv.borrow().class.name
                    )))
                })?;
                let argv = self.eval_args(args, env)?;
                self.call_method(m, Ev::Instance(recv), argv)
            }
        }
    }

    /// The `super` class and `self` instance of the method currently executing.
    fn super_bindings(&self, env: &Env) -> X<(Rc<ClassDef>, Rc<RefCell<Instance>>)> {
        let sup = match self.lookup(env, "super") {
            Some(Ev::Class(c)) => c,
            _ => {
                return Err(Abort::Error(VmError::NativeError(
                    "`super` used outside a method".into(),
                )))
            }
        };
        let recv = match self.lookup(env, "self") {
            Some(Ev::Instance(i)) => i,
            _ => {
                return Err(Abort::Error(VmError::NativeError(
                    "`super` used outside a method".into(),
                )))
            }
        };
        Ok((sup, recv))
    }

    fn eval_block_value(&mut self, block: &Block, env: &Env) -> X<Ev> {
        let mut last = Ev::Data(Value::nil());
        for s in &block.stmts {
            if let Stmt::Expr(e) = s {
                last = self.eval(e, env)?;
            } else {
                self.exec_stmt(s, env)?;
                last = Ev::Data(Value::nil());
            }
        }
        Ok(last)
    }

    fn eval_values(&mut self, exprs: &[Expr], env: &Env) -> X<Vec<Value>> {
        let mut out = Vec::with_capacity(exprs.len());
        for e in exprs {
            let v = self.eval(e, env)?;
            out.push(self.to_value(&v));
        }
        Ok(out)
    }

    fn eval_str(&mut self, e: &Expr, env: &Env) -> X<String> {
        let v = self.eval(e, env)?;
        Ok(ops::stringify(self.to_value(&v)))
    }

    fn eval_when(&mut self, arms: &[MatchArm], env: &Env) -> X<Ev> {
        for arm in arms {
            let child = Scope::child(env);
            if self.pattern_matches(&arm.pattern, &Ev::Data(Value::nil()), &child)? {
                return self.arm_body(&arm.body, &child);
            }
        }
        Ok(Ev::Data(Value::nil()))
    }

    fn eval_comprehension(
        &mut self,
        kind: CompKind,
        key: Option<&Expr>,
        body: &Expr,
        clauses: &[CompClause],
        env: &Env,
    ) -> X<Ev> {
        let mut list_out: Vec<Value> = Vec::new();
        let mut set_out: IndexSet<Value> = IndexSet::new();
        let mut dict_out: IndexMap<DictKey, Value> = IndexMap::new();
        self.run_clauses(clauses, 0, env, &mut |e: &Env, this: &mut Self| {
            let body_ev = this.eval(body, e)?;
            let val = this.to_value(&body_ev);
            match kind {
                CompKind::List => list_out.push(val),
                CompKind::Set => {
                    set_out.insert(val);
                }
                CompKind::Dict => {
                    let k = match key {
                        Some(k) => {
                            let key_ev = this.eval(k, e)?;
                            this.to_value(&key_ev)
                        }
                        None => val,
                    };
                    dict_out.insert(DictKey(k), val);
                }
            }
            Ok(())
        })?;
        Ok(Ev::Data(match kind {
            CompKind::List => new_list(list_out),
            CompKind::Set => new_set(set_out),
            CompKind::Dict => new_dict(dict_out),
        }))
    }

    fn run_clauses(
        &mut self,
        clauses: &[CompClause],
        i: usize,
        env: &Env,
        emit: &mut dyn FnMut(&Env, &mut Self) -> X<()>,
    ) -> X<()> {
        if i == clauses.len() {
            return emit(env, self);
        }
        let clause = &clauses[i];
        let seq = self.eval(&clause.iter, env)?;
        let value = self.to_value(&seq);
        let len = ops::iter_len(value) as i64;
        for idx in 0..len {
            let item = self.from_value(ops::iter_get(value, idx).map_err(Abort::Error)?);
            let child = Scope::child(env);
            self.define(&child, &clause.var, item);
            if let Some(g) = &clause.guard {
                if !self.eval_truthy(g, &child)? {
                    continue;
                }
            }
            self.run_clauses(clauses, i + 1, &child, &mut *emit)?;
        }
        Ok(())
    }

    // ── calls ────────────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn eval_call(&mut self, callee: &Expr, args: &[Expr], env: &Env) -> X<Ev> {
        if let Expr::Ident { name, .. } = callee {
            if let Some(Ev::Func(f)) = self.lookup(env, name) {
                let argv = self.eval_args(args, env)?;
                return self.call_closure(f, argv);
            }
            if let Some(v) = self.call_builtin(name, args, env)? {
                return Ok(v);
            }
        }
        let mut f = self.eval(callee, env)?;
        if let Ev::Data(v) = f {
            f = self.from_value(v);
        }
        let argv = self.eval_args(args, env)?;
        match f {
            Ev::Func(f) => self.call_closure(f, argv),
            Ev::Foreign(h) => {
                let bridge = self.pybridge()?;
                let py_args = self.marshal_args(argv)?;
                let out = bridge
                    .call(h, py_args)
                    .map_err(|e| Self::bridge_err(e, zen_vm::pybridge::BridgeOp::Call))?;
                Ok(self.wrap_pyvalue(out))
            }
            Ev::BoundMethod { receiver, method } => {
                self.call_method(method, Ev::Instance(receiver), argv)
            }
            Ev::Class(class) => self.instantiate(class, argv),
            Ev::Native { f, .. } => {
                let vals: Vec<Value> = argv.iter().map(|a| self.to_value(a)).collect();
                f(&vals).map(Ev::Data).map_err(Abort::Error)
            }
            other => {
                let v = self.to_value(&other);
                Err(Abort::Error(VmError::type_mismatch("function", v)))
            }
        }
    }

    fn eval_args(&mut self, args: &[Expr], env: &Env) -> X<Vec<Ev>> {
        let mut out = Vec::with_capacity(args.len());
        for a in args {
            out.push(self.eval(a, env)?);
        }
        Ok(out)
    }

    fn call_closure(&mut self, cl: Rc<Closure>, args: Vec<Ev>) -> X<Ev> {
        self.invoke(cl, None, None, args)
    }

    /// Call a class method, binding `self` (and its `this` alias) and — when the
    /// owning class has a parent — a `super` binding, in the callee.
    fn call_method(&mut self, cl: Rc<Closure>, receiver: Ev, args: Vec<Ev>) -> X<Ev> {
        let super_class = match &receiver {
            Ev::Instance(inst) => inst.borrow().owner_of(&cl).and_then(|c| c.parent.clone()),
            _ => None,
        };
        self.invoke(cl, Some(receiver), super_class, args)
    }

    fn invoke(
        &mut self,
        cl: Rc<Closure>,
        receiver: Option<Ev>,
        super_class: Option<Rc<ClassDef>>,
        args: Vec<Ev>,
    ) -> X<Ev> {
        if self.depth >= MAX_DEPTH {
            return Err(Abort::Error(VmError::StackOverflow(
                "maximum recursion depth exceeded".into(),
            )));
        }
        self.depth += 1;
        let result = self.call_closure_inner(&cl, receiver, super_class, args);
        self.depth -= 1;
        result
    }

    fn call_closure_inner(
        &mut self,
        cl: &Rc<Closure>,
        receiver: Option<Ev>,
        super_class: Option<Rc<ClassDef>>,
        args: Vec<Ev>,
    ) -> X<Ev> {
        let env = Scope::child(&cl.env);
        if let Some(recv) = receiver {
            self.define(&env, "self", recv.clone());
            self.define(&env, "this", recv);
        }
        if let Some(sup) = super_class {
            self.define(&env, "super", Ev::Class(sup));
        }
        for (i, p) in cl.params.iter().enumerate() {
            let v = match args.get(i) {
                Some(v) => v.clone(),
                None => match &p.default {
                    Some(d) => self.eval(d, &env)?,
                    None => {
                        return Err(Abort::Error(VmError::ArgumentCount {
                            expected: cl.params.len(),
                            got: args.len(),
                        }))
                    }
                },
            };
            self.define(&env, &p.name, v);
        }
        match &cl.body {
            Body::Expr(e) => self.eval(e, &env),
            Body::Block { block, auto_return } => match self.exec_block(&block.stmts, &env) {
                Ok(()) => {
                    if *auto_return {
                        self.last_expr_value(block, &env)
                    } else {
                        Ok(Ev::Data(Value::nil()))
                    }
                }
                Err(Abort::Return(v)) => Ok(v),
                Err(e) => Err(e),
            },
        }
    }

    fn last_expr_value(&mut self, block: &Block, env: &Env) -> X<Ev> {
        match block.stmts.last() {
            Some(Stmt::Expr(e)) => self.eval(e, env),
            _ => Ok(Ev::Data(Value::nil())),
        }
    }

    fn call_builtin(&mut self, name: &str, args: &[Expr], env: &Env) -> X<Option<Ev>> {
        let id = match name {
            "print" => 0,
            "str" => 1,
            "int" => 2,
            "float" => 3,
            "bool" => 4,
            "len" => 5,
            "max" => 6,
            "min" => 7,
            "sum" => 8,
            "abs" => 9,
            "exit" => 10,
            _ => {
                let first = |this: &mut Self| -> X<Ev> {
                    let e = args.first().ok_or_else(|| {
                        Abort::Error(VmError::ArgumentCount {
                            expected: 1,
                            got: 0,
                        })
                    })?;
                    this.eval(e, env)
                };
                match name {
                    "type" => {
                        let v = first(self)?;
                        let ty = match &v {
                            Ev::Func(_) | Ev::Native { .. } | Ev::BoundMethod { .. } => "func",
                            Ev::Class(_) => "class",
                            Ev::Instance(_) => "instance",
                            Ev::Module(_) => "module",
                            Ev::Foreign(_) => "foreign",
                            Ev::Data(d) => d.type_name(),
                        };
                        return Ok(Some(Ev::Data(Value::string(ty))));
                    }
                    "list" => {
                        if args.is_empty() {
                            return Ok(Some(Ev::Data(new_list(Vec::new()))));
                        }
                        let v = first(self)?;
                        let items = self.iter_values(&v)?;
                        return Ok(Some(Ev::Data(new_list(items))));
                    }
                    "tuple" => {
                        let v = first(self)?;
                        let items = self.iter_values(&v)?;
                        return Ok(Some(Ev::Data(new_tuple(items))));
                    }
                    "set" => {
                        if args.is_empty() {
                            return Ok(Some(Ev::Data(new_set(IndexSet::new()))));
                        }
                        let v = first(self)?;
                        let items: IndexSet<Value> = self.iter_values(&v)?.into_iter().collect();
                        return Ok(Some(Ev::Data(new_set(items))));
                    }
                    "enumerate" => {
                        let v = first(self)?;
                        let items = self.iter_values(&v)?;
                        let out = items
                            .into_iter()
                            .enumerate()
                            .map(|(i, item)| new_list(vec![Value::int_promote(i as i64), item]))
                            .collect();
                        return Ok(Some(Ev::Data(new_list(out))));
                    }
                    "dict" => {
                        let mut map = IndexMap::new();
                        if let Some(e) = args.first() {
                            let v = self.eval(e, env)?;
                            for item in self.iter_values(&v)? {
                                if let Some(pair) = item.as_list() {
                                    if pair.len() == 2 {
                                        ops::dict_insert(&mut map, pair[0], pair[1]);
                                    }
                                }
                            }
                        }
                        return Ok(Some(Ev::Data(new_dict(map))));
                    }
                    "range" => {
                        let mut nums = Vec::new();
                        for e in args {
                            nums.push(self.eval_to_value(e, env)?.as_int().ok_or_else(|| {
                                Abort::Error(VmError::type_mismatch("int", Value::nil()))
                            })?);
                        }
                        let (start, stop, step) = match nums.as_slice() {
                            [stop] => (0, *stop, 1),
                            [start, stop] => (*start, *stop, 1),
                            [start, stop, step] => (*start, *stop, *step),
                            _ => {
                                return Err(Abort::Error(VmError::ArgumentCount {
                                    expected: 1,
                                    got: args.len(),
                                }))
                            }
                        };
                        let mut out = Vec::new();
                        let mut i = start;
                        if step > 0 {
                            while i < stop {
                                out.push(Value::int_promote(i));
                                i += step;
                            }
                        } else if step < 0 {
                            while i > stop {
                                out.push(Value::int_promote(i));
                                i += step;
                            }
                        }
                        return Ok(Some(Ev::Data(new_list(out))));
                    }
                    _ => return Ok(None),
                }
            }
        };
        let mut vals = Vec::with_capacity(args.len());
        for a in args {
            vals.push(self.eval_to_value(a, env)?);
        }
        let v = ops::native_call(id, &vals, &mut self.output).map_err(Abort::Error)?;
        if id == 10 {
            self.exit_code = vals.first().and_then(|v| v.as_int()).unwrap_or(0) as i32;
        }
        Ok(Some(Ev::Data(v)))
    }

    fn eval_method(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[Expr],
        env: &Env,
        span: Span,
    ) -> X<Ev> {
        let recv = self.eval(receiver, env)?;
        // Foreign dispatch: one fused bridge round-trip.
        if let Ev::Foreign(h) = &recv {
            let bridge = self.pybridge()?;
            let argv = self.eval_args(args, env)?;
            let py_args = self.marshal_args(argv)?;
            let out = bridge
                .call_attr(*h, method, py_args)
                .map_err(|e| Self::bridge_err(e, zen_vm::pybridge::BridgeOp::Call))?;
            return Ok(self.wrap_pyvalue(out));
        }
        // `stringmod.title(s)`: the receiver is a native module, so the method
        // is a namespace member rather than a method on a value.
        if let Ev::Module(m) = &recv {
            let member = m.members.get(method).cloned().ok_or_else(|| {
                Abort::Error(VmError::NativeError(format!(
                    "module has no member {method}"
                )))
            })?;
            let argv: Vec<Value> = self.eval_values(args, env)?;
            return match member {
                Ev::Native { f, .. } => f(&argv).map(Ev::Data).map_err(Abort::Error),
                Ev::Func(cl) => {
                    let ev_args: Vec<Ev> = argv.into_iter().map(Ev::Data).collect();
                    self.call_closure(cl, ev_args)
                }
                other => Err(Abort::Error(VmError::type_mismatch(
                    "function",
                    self.to_value(&other),
                ))),
            };
        }
        // A method call on an instance dispatches through the class chain with
        // `self` bound. A field holding a callable falls through to the generic
        // path below.
        if let Ev::Instance(inst) = &recv {
            // Bind the lookup to a local first: an `if let` scrutinee temporary
            // stays borrowed through the body, and `call_method` may mutate the
            // instance (e.g. `self.n = …`).
            let found = inst.borrow().find_method(method);
            if let Some(m) = found {
                let argv = self.eval_args(args, env)?;
                return self.call_method(m, Ev::Instance(inst.clone()), argv);
            }
        }
        // Higher-order methods need interpreter closures, so they are handled
        // here rather than in the shared `ops::method_call`.
        match method {
            "map" | "filter" | "reduce" | "each" | "for_each" => {
                return self.eval_higher_order(recv, method, args, env, span);
            }
            _ => {}
        }
        let mut value = self.to_value(&recv);
        let argvals = self.eval_values(args, env)?;
        let result = ops::method_call(&mut value, method, &argvals).map_err(Abort::Error)?;
        self.store_place(receiver, Ev::Data(value), env)?;
        Ok(self.from_value(result))
    }

    fn eval_higher_order(
        &mut self,
        recv: Ev,
        method: &str,
        args: &[Expr],
        env: &Env,
        span: Span,
    ) -> X<Ev> {
        let seq = self.to_value(&recv);
        let f = self.eval(
            args.first().ok_or_else(|| {
                Abort::Error(VmError::ArgumentCount {
                    expected: 1,
                    got: 0,
                })
            })?,
            env,
        )?;
        let f = match f {
            Ev::Func(f) => f,
            other => {
                return Err(Abort::Error(VmError::type_mismatch(
                    "function",
                    self.to_value(&other),
                )))
            }
        };
        let len = ops::iter_len(seq) as i64;
        match method {
            "map" => {
                let mut out = Vec::with_capacity(len as usize);
                for i in 0..len {
                    let item = self.from_value(ops::iter_get(seq, i).map_err(Abort::Error)?);
                    let mapped = self.call_closure(f.clone(), vec![item])?;
                    out.push(self.to_value(&mapped));
                }
                Ok(Ev::Data(new_list(out)))
            }
            "filter" => {
                let mut out = Vec::new();
                for i in 0..len {
                    let item = self.from_value(ops::iter_get(seq, i).map_err(Abort::Error)?);
                    let keep = self.call_closure(f.clone(), vec![item.clone()])?;
                    if self.truthy(&keep) {
                        out.push(self.to_value(&item));
                    }
                }
                Ok(Ev::Data(new_list(out)))
            }
            "reduce" => {
                let mut acc = match args.get(1) {
                    Some(init) => self.eval(init, env)?,
                    None => {
                        if len == 0 {
                            return Err(Abort::Error(VmError::ValueError(
                                "reduce of empty sequence with no initial value".into(),
                            )));
                        }
                        self.from_value(ops::iter_get(seq, 0).map_err(Abort::Error)?)
                    }
                };
                let start = if args.get(1).is_some() { 0 } else { 1 };
                for i in start..len {
                    let item = self.from_value(ops::iter_get(seq, i).map_err(Abort::Error)?);
                    acc = self.call_closure(f.clone(), vec![acc, item])?;
                }
                Ok(acc)
            }
            "each" | "for_each" => {
                for i in 0..len {
                    let item = self.from_value(ops::iter_get(seq, i).map_err(Abort::Error)?);
                    self.call_closure(f.clone(), vec![item])?;
                }
                Ok(Ev::Data(Value::nil()))
            }
            _ => Err(Abort::Error(unsupported(span, "method"))),
        }
    }

    // ── operators ────────────────────────────────────────────────────────

    fn binary(&mut self, op: BinaryOp, l: Ev, r: Ev, span: Span) -> X<Ev> {
        match op {
            BinaryOp::And | BinaryOp::Or => {
                let res = match op {
                    BinaryOp::And => self.truthy(&l) && self.truthy(&r),
                    _ => self.truthy(&l) || self.truthy(&r),
                };
                return Ok(Ev::Data(Value::bool(res)));
            }
            BinaryOp::Eq | BinaryOp::StrictEq => {
                return Ok(Ev::Data(Value::bool(self.ev_equal(&l, &r))));
            }
            BinaryOp::Ne | BinaryOp::StrictNe => {
                return Ok(Ev::Data(Value::bool(!self.ev_equal(&l, &r))));
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                return Ok(Ev::Data(Value::bool(self.compare(op, &l, &r)?)));
            }
            _ => {}
        }
        let (lv, rv) = (self.to_value(&l), self.to_value(&r));
        let v = match op {
            BinaryOp::Add => ops::arith(ops::Arith::Add, lv, rv),
            BinaryOp::Sub => ops::arith(ops::Arith::Sub, lv, rv),
            BinaryOp::Mul => ops::arith(ops::Arith::Mul, lv, rv),
            BinaryOp::Div => ops::arith(ops::Arith::Div, lv, rv),
            BinaryOp::Mod => ops::arith(ops::Arith::Mod, lv, rv),
            BinaryOp::Pow => ops::arith(ops::Arith::Pow, lv, rv),
            BinaryOp::BitAnd => ops::bitwise(ops::Bit::And, lv, rv),
            BinaryOp::BitOr => ops::bitwise(ops::Bit::Or, lv, rv),
            BinaryOp::BitXor => ops::bitwise(ops::Bit::Xor, lv, rv),
            BinaryOp::Shl => ops::bitwise(ops::Bit::Shl, lv, rv),
            BinaryOp::Shr => ops::bitwise(ops::Bit::Shr, lv, rv),
            BinaryOp::Nullish => {
                return Ok(if lv.is_nil() { r } else { l });
            }
            BinaryOp::In => {
                let found = contains_value(rv, lv).map_err(Abort::Error)?;
                return Ok(Ev::Data(Value::bool(found)));
            }
            BinaryOp::NotIn => {
                let found = contains_value(rv, lv).map_err(Abort::Error)?;
                return Ok(Ev::Data(Value::bool(!found)));
            }
            BinaryOp::Is | BinaryOp::IsNot => {
                return Err(Abort::Error(unsupported(span, "is")));
            }
            BinaryOp::Range => {
                let (Some(s), Some(e)) = (lv.as_int(), rv.as_int()) else {
                    return Err(Abort::Error(VmError::type_mismatch("int", lv)));
                };
                let items: Vec<Value> = if e >= s {
                    (s..=e).map(Value::int_promote).collect()
                } else {
                    Vec::new()
                };
                return Ok(Ev::Data(new_list(items)));
            }
            BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::Eq
            | BinaryOp::StrictEq
            | BinaryOp::Ne
            | BinaryOp::StrictNe
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge => unreachable!(),
        };
        Ok(Ev::Data(v.map_err(Abort::Error)?))
    }

    fn unary(&mut self, op: UnaryOp, v: Ev, _span: Span) -> X<Ev> {
        let val = self.to_value(&v);
        let out = match op {
            UnaryOp::Neg => ops::negate(val),
            UnaryOp::Not => Ok(Value::bool(!ops::truthy(val))),
            UnaryOp::BitNot => ops::bitnot(val),
            UnaryOp::TypeOf => Ok(Value::string(val.type_name())),
        };
        Ok(Ev::Data(out.map_err(Abort::Error)?))
    }

    fn compare(&self, op: BinaryOp, l: &Ev, r: &Ev) -> X<bool> {
        // Equality (including structural container equality) does not order.
        match op {
            BinaryOp::Eq | BinaryOp::StrictEq => return Ok(self.ev_equal(l, r)),
            BinaryOp::Ne | BinaryOp::StrictNe => return Ok(!self.ev_equal(l, r)),
            _ => {}
        }
        let ord = ops::compare_order(l.data(), r.data()).map_err(Abort::Error)?;
        Ok(match op {
            BinaryOp::Lt => ord == std::cmp::Ordering::Less,
            BinaryOp::Le => ord != std::cmp::Ordering::Greater,
            BinaryOp::Gt => ord == std::cmp::Ordering::Greater,
            BinaryOp::Ge => ord != std::cmp::Ordering::Less,
            _ => false,
        })
    }

    fn values_eq(&self, a: &Value, b: &Value) -> bool {
        ops::values_equal(*a, *b)
    }

    fn ev_equal(&self, l: &Ev, r: &Ev) -> bool {
        match (l, r) {
            (Ev::Func(a), Ev::Func(b)) => Rc::ptr_eq(a, b),
            (Ev::Class(a), Ev::Class(b)) => Rc::ptr_eq(a, b),
            (Ev::Instance(a), Ev::Instance(b)) => Rc::ptr_eq(a, b),
            (Ev::Data(a), Ev::Data(b)) => self.values_eq(a, b),
            _ => false,
        }
    }

    fn literal(&self, l: &Literal) -> Value {
        match l {
            Literal::Nil => Value::nil(),
            Literal::Bool(b) => Value::bool(*b),
            Literal::Int(i) => Value::int_promote(*i),
            Literal::Float(f) => Value::float(*f),
            Literal::String(s) => Value::string(s.clone()),
            Literal::Bytes(b) => Value::bytes(b.clone()),
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// Default `import` search path: `./std`, `../../std` (for crate-local tests),
/// plus `ZEN_MODULE_PATH` entries.
fn default_module_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("std"), PathBuf::from("../../std")];
    if let Ok(extra) = std::env::var("ZEN_MODULE_PATH") {
        dirs.extend(
            extra
                .split(':')
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
        );
    }
    dirs
}

/// A native module member: the first argument is the receiver, the rest are
/// forwarded to the shared string method dispatcher in `zen_vm::ops`.
fn string_module_fn(name: &'static str) -> Ev {
    let method = name.to_string();
    let f: NativeFn = Rc::new(move |args: &[Value]| {
        let mut recv = *args.first().ok_or(VmError::ArgumentCount {
            expected: 1,
            got: 0,
        })?;
        ops::method_call(&mut recv, &method, &args[1..])
    });
    Ev::Native {
        name: name.to_string(),
        f,
    }
}

/// The built-in `string` module for M3 (mirrors `std/string.z`).
fn native_string_module() -> NativeModule {
    let mut m = NativeModule::default();
    for name in [
        "title",
        "count",
        "zfill",
        "upper",
        "lower",
        "length",
        "len",
        "contains",
        "replace",
        "find",
        "index",
        "split",
        "strip",
        "trim",
        "lstrip",
        "rstrip",
        "startswith",
        "starts_with",
        "endswith",
        "ends_with",
        "substring",
        "slice",
        "to_num",
        "to_int",
        "to_float",
        "repeat",
    ] {
        m.members.insert(name.to_string(), string_module_fn(name));
    }
    m
}

fn unsupported(span: Span, what: &str) -> VmError {
    VmError::NativeError(format!(
        "unsupported construct '{what}' (M3 interpreter) at {:?}",
        span
    ))
}

fn catch_matches(c: &CatchClause, e: &VmError) -> bool {
    match &c.typ {
        None => true,
        Some(name) => name == e.kind(),
    }
}

/// Compound assignments (and `++`/`--`) read the target's current value first;
/// a plain `=` does not, so `d["new"] = v` does not raise a spurious `KeyError`.
fn needs_current(op: &AssignOp) -> bool {
    !matches!(op, AssignOp::Assign)
}

fn assign_binop(op: &AssignOp) -> Option<BinaryOp> {
    Some(match op {
        AssignOp::Assign => return None,
        AssignOp::AddAssign => BinaryOp::Add,
        AssignOp::SubAssign => BinaryOp::Sub,
        AssignOp::MulAssign => BinaryOp::Mul,
        AssignOp::DivAssign => BinaryOp::Div,
        AssignOp::ModAssign => BinaryOp::Mod,
        AssignOp::BitAndAssign => BinaryOp::BitAnd,
        AssignOp::BitOrAssign => BinaryOp::BitOr,
        AssignOp::BitXorAssign => BinaryOp::BitXor,
        AssignOp::ShlAssign => BinaryOp::Shl,
        AssignOp::ShrAssign => BinaryOp::Shr,
        AssignOp::NullCoalesceAssign => BinaryOp::Nullish,
        AssignOp::Inc | AssignOp::Dec => return None,
    })
}

fn contains_value(haystack: Value, needle: Value) -> VmResult<bool> {
    if let Some(list) = haystack.as_list() {
        return Ok(list.iter().any(|v| ops::values_equal(*v, needle)));
    }
    if let Some(tuple) = haystack.as_tuple() {
        return Ok(tuple.iter().any(|v| ops::values_equal(*v, needle)));
    }
    if let Some(set) = haystack.as_set() {
        return Ok(set.iter().any(|v| ops::values_equal(*v, needle)));
    }
    if let Some(dict) = haystack.as_dict() {
        return Ok(dict.keys().any(|k| ops::values_equal(k.0, needle)));
    }
    if let (Some(s), Some(c)) = (haystack.as_str(), needle.as_str()) {
        return Ok(s.contains(c));
    }
    Err(VmError::type_mismatch("container", haystack))
}

fn slice_value(obj: Value, start: Option<i64>, end: Option<i64>) -> X<Value> {
    if let Some(list) = obj.as_list() {
        let (a, b) = slice_bounds(start, end, list.len());
        return Ok(new_list(list[a..b].to_vec()));
    }
    if let Some(tuple) = obj.as_tuple() {
        let (a, b) = slice_bounds(start, end, tuple.len());
        return Ok(new_tuple(tuple[a..b].to_vec()));
    }
    if let Some(s) = obj.as_str() {
        let chars: Vec<char> = s.chars().collect();
        let (a, b) = slice_bounds(start, end, chars.len());
        return Ok(Value::string(chars[a..b].iter().collect::<String>()));
    }
    Err(Abort::Error(VmError::type_mismatch("sliceable", obj)))
}

fn slice_bounds(start: Option<i64>, end: Option<i64>, len: usize) -> (usize, usize) {
    let len_i = len as i64;
    let norm = |i: i64| -> i64 {
        let i = if i < 0 { i + len_i } else { i };
        i.clamp(0, len_i)
    };
    let a = norm(start.unwrap_or(0));
    let b = norm(end.unwrap_or(len_i));
    (a as usize, b.max(a) as usize)
}
