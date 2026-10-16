//! The AST-to-bytecode compiler.

use zen_syntax::ast::{
    AssignOp, BinaryOp, Block, BlockOrExpr, CatchClause, ClassMember, ClassMemberKind, CompClause,
    CompKind, Expr, ImportItems, ImportName, ImportSpec, Literal, MatchArm, Param, Pattern, Stmt,
    StringPart, UnaryOp,
};
use zen_syntax::span::{FileId, LineIndex, Span};
use zen_vm::bytecode::{FunctionProto, Instruction, UpvalueDesc};
use zen_vm::{Opcode, Value};

use crate::chunk::{Chunk, ChunkError};
use crate::error::{CompileError, CompileResult};
use crate::resolver::Resolver;

static BUILTINS: phf::Map<&'static str, u16> = phf::phf_map! {
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
    "import" => 11,
    "list" => 12,
    "tuple" => 13,
    "set" => 14,
    "dict" => 15,
    "range" => 16,
    "type" => 17,
    "enumerate" => 18,
    "cli_args" => 19,
    "zen_version" => 20,
    "os_platform" => 21,
    "chr" => 22,
    "sys_fork" => 246,
    "sys_exec" => 247,
    "sys_waitpid" => 248,
    "sys_wexitstatus" => 249,
    "sys_getpid" => 250,
};

/// The result of compiling a whole module: a top-level function plus a flat
/// table of nested prototypes referenced by `MAKE_CLOSURE`'s `b` operand.
#[derive(Debug, Clone)]
pub struct CompiledModule {
    pub main: FunctionProto,
    pub children: Vec<FunctionProto>,
    /// The module's exported bindings as `(name, global_slot)` in declaration
    /// order. Private bindings (leading `_`) are omitted. The importer zips
    /// these names with the module's runtime `globals` vector to build the
    /// export dictionary.
    pub exports: Vec<(String, u16)>,
}

impl CompiledModule {
    pub fn child(&self, index: u8) -> Option<&FunctionProto> {
        self.children.get(usize::from(index))
    }
}

impl From<CompiledModule> for zen_vm::bytecode::ModuleUnit {
    fn from(m: CompiledModule) -> Self {
        zen_vm::bytecode::ModuleUnit {
            main: m.main,
            children: m.children,
            exports: m.exports,
            const_globals: Vec::new(),
        }
    }
}

/// The oracle's wording for binding more loop variables than the yielded item
/// has elements. It travels as a constant so the VM never hardcodes a message
/// about `for` loops.
const UNPACK_FOR_ERROR: &str = "not enough values to unpack in for";

/// Compile a module produced by the parser.
pub fn compile_module(
    module: &zen_syntax::ast::Module,
    source: &str,
) -> CompileResult<CompiledModule> {
    let mut c = Compiler::new(module.file_id, source);
    c.compile_items(&module.items)?;
    let frame = c.funcs.pop().expect("module frame");
    let num_upvalues = frame.resolver.pending_upvalues.len() as u16;
    let descriptors = frame.resolver.pending_upvalues.clone();
    let mut main = frame.chunk.into_proto(
        "<module>".to_string(),
        Vec::new(),
        frame.resolver.num_locals,
        frame.resolver.max_reg,
        0,
        num_upvalues,
        descriptors,
        false,
    );
    main.num_globals = c.module_globals.len() as u16;
    // Read-only capture optimization (whole-module: sibling ordering cannot
    // hide a writer once every function is compiled).
    crate::capture::optimize_captures(&mut main, &mut c.children);
    // Every real top-level binding is reachable as `<module>.<name>`, including
    // single-underscore helpers (e.g. `json._load`); only the synthetic
    // star-import slots have no user-facing name and are dropped.
    let exports = c
        .module_globals
        .iter()
        .enumerate()
        .filter(|(_, name)| !name.starts_with('\u{0}') && name.as_str() != "ZEN_PURE")
        .map(|(slot, name)| (name.clone(), slot as u16))
        .collect();
    Ok(CompiledModule {
        main,
        children: c.children,
        exports,
    })
}

/// Lex and parse `source`, then compile it.
pub fn compile_source(source: &str) -> CompileResult<CompiledModule> {
    let lx = zen_syntax::lex::lex(source, FileId(0));
    let module = zen_syntax::parser::Parser::new(lx)
        .parse_module()
        .map_err(|e| CompileError::Custom {
            span: e.span(),
            msg: e.to_string(),
        })?;
    compile_module(&module, source)
}

struct Frame {
    resolver: Resolver,
    chunk: Chunk,
    params: Vec<String>,
}

impl Frame {
    fn new() -> Self {
        Self {
            resolver: Resolver::new(),
            chunk: Chunk::new(),
            params: Vec::new(),
        }
    }
}

struct Compiler<'a> {
    file: FileId,
    lines: LineIndex<'a>,
    funcs: Vec<Frame>,
    children: Vec<FunctionProto>,
    /// Module-scope global names by slot (in declaration order). Slot `i` maps
    /// to `module_globals[i]` at runtime; only the module frame owns these.
    module_globals: Vec<String>,
    /// Lookup for [`Compiler::module_globals`], carrying const-ness so
    /// assignment to a module-scope `const` is rejected.
    module_global_map: std::collections::HashMap<String, (u16, bool)>,
    /// `try` blocks entered by the function currently being compiled. A `break`
    /// or `continue` that leaves them must retire their handlers, otherwise a
    /// dead handler would outrank a live one further out.
    try_depth: usize,
    /// `try_depth` where each enclosing loop was entered, innermost last. A jump
    /// out of a loop only retires the handlers established inside it.
    loop_try_bases: Vec<usize>,
    /// Module-global slots holding the dicts of `from <mod> import *` statements,
    /// newest last. An unresolved bare name at module scope falls back to a field
    /// read on the newest such dict.
    star_globals: Vec<u16>,
}

impl<'a> Compiler<'a> {
    fn new(file: FileId, source: &'a str) -> Self {
        Self {
            file,
            lines: LineIndex::build(source),
            funcs: vec![Frame::new()],
            children: Vec::new(),
            module_globals: Vec::new(),
            module_global_map: std::collections::HashMap::new(),
            try_depth: 0,
            loop_try_bases: Vec::new(),
            star_globals: Vec::new(),
        }
    }

    fn is_builtin(&self, name: &str) -> Option<u16> {
        BUILTINS.get(name).copied()
    }

    /// A name is *bound* if any enclosing frame declares it (as a local, an
    /// upvalue, or a module global). Builtins are only consulted for unbound
    /// names, so a program may shadow `list`/`set`/`type` with its own binding.
    fn name_is_bound(&self, name: &str) -> bool {
        for f in self.funcs.iter().rev() {
            if f.resolver.resolve_local(name).is_some() {
                return true;
            }
            if f.resolver.resolve_upvalue(name).is_some() {
                return true;
            }
        }
        self.module_global_map.contains_key(name)
    }

    // ── frame plumbing ───────────────────────────────────────────────────
    fn cur(&self) -> &Frame {
        self.funcs.last().expect("compiler frame")
    }

    fn cur_mut(&mut self) -> &mut Frame {
        self.funcs.last_mut().expect("compiler frame")
    }

    fn line_of(&self, span: Span) -> u32 {
        self.lines.line_of(span.lo)
    }

    fn empty_span(&self) -> Span {
        Span::empty_at(self.file, 0)
    }

    fn emit(&mut self, op: Opcode, a: u8, b: u8, c: u8, span: Span) {
        let line = self.line_of(span);
        self.cur_mut().chunk.emit(op, a, b, c, line);
    }

    fn here(&self) -> usize {
        self.cur().chunk.here()
    }

    fn emit_jump(&mut self, op: Opcode, a: u8, span: Span) -> usize {
        let line = self.line_of(span);
        self.cur_mut().chunk.emit_jump(op, a, line)
    }

    fn patch(&mut self, pc: usize, target: usize) -> CompileResult<()> {
        let span = self.empty_span();
        self.cur_mut()
            .chunk
            .patch_jump(pc, target)
            .map_err(|e| self.chunk_err(e, span))
    }

    fn chunk_err(&self, e: ChunkError, span: Span) -> CompileError {
        match e {
            ChunkError::JumpOffsetOverflow => CompileError::JumpOffsetOverflow { span },
            ChunkError::ConstantPoolOverflow => CompileError::TooManyConstants { span },
            ChunkError::TooManyRegisters => CompileError::TooManyRegisters { span },
        }
    }

    // ── register allocation ──────────────────────────────────────────────
    fn free_all_temps(&mut self) {
        let base = self.cur().resolver.num_locals;
        self.cur_mut().resolver.free_to(base);
    }

    /// Release every temporary above `reg`, keeping `reg` live. `free_since`
    /// frees `reg` itself, so shift by one — callers pass the register holding
    /// the result they are about to return.
    fn free_above(&mut self, reg: u8) {
        self.cur_mut().resolver.free_since(reg.saturating_add(1));
    }

    fn alloc_temp(&mut self, span: Span) -> CompileResult<u8> {
        self.cur_mut()
            .resolver
            .alloc_temp()
            .ok_or(CompileError::TooManyLocals { span })
    }

    fn alloc_temps(&mut self, n: u16, span: Span) -> CompileResult<u8> {
        self.cur_mut()
            .resolver
            .alloc_temps(n)
            .ok_or(CompileError::TooManyLocals { span })
    }

    /// True while compiling a binding at the module's root lexical scope. Such
    /// bindings become module globals (heap-backed, shared across every
    /// function) rather than fixed local registers.
    fn at_module_root(&self) -> bool {
        self.funcs.len() == 1 && self.cur().resolver.depth() == 1
    }

    /// Allocate a module global slot. Returns the slot index (not a register).
    fn declare_global(&mut self, name: &str, is_const: bool, span: Span) -> CompileResult<u16> {
        // Redeclaring a module-scope name rebinds it rather than failing, which
        // is what the oracle does: the name keeps its global slot and the new
        // binding's const-ness is what later assignments are checked against.
        if let Some((slot, _)) = self.module_global_map.get(name).copied() {
            self.module_global_map
                .insert(name.to_string(), (slot, is_const));
            return Ok(slot);
        }
        let slot = self.module_globals.len();
        if slot > u8::MAX as usize {
            return Err(CompileError::TooManyLocals { span });
        }
        self.module_globals.push(name.to_string());
        self.module_global_map
            .insert(name.to_string(), (slot as u16, is_const));
        Ok(slot as u16)
    }

    /// Declare `name` then store `src` into it, choosing a module global or a
    /// fixed local register based on the current scope.
    fn define_binding(
        &mut self,
        name: &str,
        is_const: bool,
        src: u8,
        span: Span,
    ) -> CompileResult<()> {
        if self.at_module_root() {
            let slot = self.declare_global(name, is_const, span)?;
            self.emit(Opcode::SetGlobal, slot as u8, src, 0, span);
            Ok(())
        } else {
            let slot = self.declare_named(name, is_const, span)?;
            self.move_reg(slot as u8, src, span);
            Ok(())
        }
    }

    /// Bind an `import` target. Unlike a `let` in a nested block, an import
    /// binds at the enclosing *function* scope: at module level it becomes a
    /// module global even when written inside a loop or `if`, matching Python.
    fn define_import_binding(
        &mut self,
        name: &str,
        is_const: bool,
        src: u8,
        span: Span,
    ) -> CompileResult<()> {
        if self.funcs.len() == 1 {
            let slot = self.declare_global(name, is_const, span)?;
            self.emit(Opcode::SetGlobal, slot as u8, src, 0, span);
        } else {
            let slot = self.declare_named(name, is_const, span)?;
            self.move_reg(slot as u8, src, span);
        }
        Ok(())
    }

    fn declare_named(&mut self, name: &str, is_const: bool, span: Span) -> CompileResult<u16> {
        if self.at_module_root() {
            return self.declare_global(name, is_const, span);
        }
        // A redeclaration shadows the earlier binding: the new slot is found
        // first by the innermost-scope lookup, so the oracle's rebinding
        // behaviour falls out of the ordinary scoping rules.
        self.cur_mut()
            .resolver
            .declare_local(name.to_string(), is_const)
            .ok_or(CompileError::TooManyLocals { span })
    }

    fn declare_hidden(&mut self, span: Span) -> CompileResult<u16> {
        self.cur_mut()
            .resolver
            .declare_hidden()
            .ok_or(CompileError::TooManyLocals { span })
    }

    /// Take a slot from the reserved pool (see `Resolver::reserve_hidden`).
    fn reserve_slot(&mut self, span: Span) -> CompileResult<u8> {
        self.cur_mut()
            .resolver
            .reserve_hidden()
            .map(|slot| slot as u8)
            .ok_or(CompileError::TooManyLocals { span })
    }

    /// Give a reserved slot back once the construct that held it is compiled.
    fn release_slot(&mut self) {
        self.cur_mut().resolver.release_hidden();
    }

    fn move_reg(&mut self, dst: u8, src: u8, span: Span) {
        if dst != src {
            self.emit(Opcode::Move, dst, src, 0, span);
        }
    }

    fn add_const(&mut self, value: Value, span: Span) -> CompileResult<u8> {
        self.cur_mut()
            .chunk
            .add_constant(value)
            .map_err(|e| self.chunk_err(e, span))
    }

    // ── statements ───────────────────────────────────────────────────────
    fn compile_stmt(&mut self, stmt: &Stmt) -> CompileResult<()> {
        let span = self.stmt_span(stmt);
        match stmt {
            Stmt::Let {
                name, value, span, ..
            } => self.compile_binding(name, value, false, *span),
            Stmt::Const { name, value, span } => self.compile_binding(name, value, true, *span),
            Stmt::Shared { name, value, span } => self.compile_binding(name, value, false, *span),
            Stmt::Assign {
                target,
                op,
                value,
                span,
            } => {
                self.compile_assign(target, op, value, *span)?;
                self.free_all_temps();
                Ok(())
            }
            Stmt::Expr(e) => {
                self.expr(e)?;
                self.free_all_temps();
                Ok(())
            }
            Stmt::Print {
                args,
                sep,
                end,
                span,
            } => {
                if sep.is_some() || end.is_some() {
                    return Err(self.unsupported(*span, "print with sep/end"));
                }
                self.compile_native_call(args, 0, *span)?;
                self.free_all_temps();
                Ok(())
            }
            Stmt::If {
                cond,
                then_branch,
                elifs,
                else_branch,
                ..
            } => self.compile_if(cond, then_branch, elifs, else_branch.as_ref(), span),
            Stmt::While { cond, body, span } => self.compile_while(cond, body, *span),
            Stmt::For {
                vars,
                iter,
                body,
                span,
            } => self.compile_for(vars, iter, body, *span),
            Stmt::Function {
                name,
                params,
                body,
                span,
            } => self.compile_function_stmt(name, params, body, *span),
            Stmt::Return { value, span } => {
                let reg = match value {
                    Some(e) => self.expr(e)?,
                    None => self.load_const(Value::nil(), *span)?,
                };
                self.emit(Opcode::Return, reg, 0, 0, *span);
                self.free_all_temps();
                Ok(())
            }
            Stmt::Break { span } => self.compile_break(*span),
            Stmt::Continue { span } => self.compile_continue(*span),
            Stmt::Match {
                subject,
                arms,
                span,
            } => self.compile_match(Some(subject), arms, *span),
            Stmt::When { arms, span } => self.compile_match(None, arms, *span),
            Stmt::Throw { value, span } => {
                let r = self.expr(value)?;
                self.emit(Opcode::Throw, r, 0, 0, *span);
                self.free_all_temps();
                Ok(())
            }
            Stmt::Try {
                body,
                catches,
                finally,
                span,
            } => self.compile_try(body, catches, finally.as_ref(), *span),
            Stmt::Import { specs, span } => self.compile_import(specs, *span),
            Stmt::Pymport { path, alias, span } => {
                self.compile_pymport(path, alias.as_deref(), *span)
            }
            Stmt::FromImport { path, items, span } => self.compile_from_import(path, items, *span),
            Stmt::Include { span, .. } => Err(self.unsupported(*span, "include")),
            Stmt::Class {
                name,
                parent,
                members,
                span,
            } => self.compile_class(name, parent.as_deref(), members, *span),
        }
    }

    fn stmt_span(&self, s: &Stmt) -> Span {
        match s {
            Stmt::Let { span, .. }
            | Stmt::Const { span, .. }
            | Stmt::Assign { span, .. }
            | Stmt::Print { span, .. }
            | Stmt::If { span, .. }
            | Stmt::While { span, .. }
            | Stmt::For { span, .. }
            | Stmt::Function { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Break { span }
            | Stmt::Continue { span }
            | Stmt::Try { span, .. }
            | Stmt::Throw { span, .. }
            | Stmt::Import { span, .. }
            | Stmt::Pymport { span, .. }
            | Stmt::FromImport { span, .. }
            | Stmt::Include { span, .. }
            | Stmt::Class { span, .. }
            | Stmt::Match { span, .. }
            | Stmt::When { span, .. }
            | Stmt::Shared { span, .. } => *span,
            Stmt::Expr(_) => self.empty_span(),
        }
    }

    /// A binding whose initializer is a function is *hoisted*: the name is declared
    /// before the body is compiled, so `let walk = fn() { walk(...) }` resolves the
    /// recursive reference. Non-function initializers keep the ordinary
    /// value-then-name order, which preserves `let x = x` semantics.
    fn compile_binding(
        &mut self,
        name: &str,
        value: &Expr,
        is_const: bool,
        span: Span,
    ) -> CompileResult<()> {
        if matches!(
            value,
            Expr::Lambda { .. } | Expr::FnExpr { .. } | Expr::ArrowFn { .. }
        ) {
            let slot = self.declare_named(name, is_const, span)?;
            let r = self.expr(value)?;
            if self.at_module_root() {
                self.emit(Opcode::SetGlobal, slot as u8, r, 0, span);
            } else {
                self.emit(Opcode::Move, slot as u8, r, 0, span);
            }
            self.free_all_temps();
            return Ok(());
        }
        let r = self.expr(value)?;
        self.define_binding(name, is_const, r, span)?;
        self.free_all_temps();
        Ok(())
    }

    // ── module imports ──────────────────────────────────────────────────────
    fn compile_import(&mut self, specs: &[ImportSpec], span: Span) -> CompileResult<()> {
        for spec in specs {
            match spec.alias.as_deref() {
                // `import a.b as x` binds the leaf module directly (no
                // namespace chain — the alias names the leaf itself).
                Some(alias) => {
                    let module_path = spec.path.join(".");
                    let k = self.add_const(Value::string(module_path), span)?;
                    let reg = self.alloc_temp(span)?;
                    self.emit(Opcode::Import, reg, k, 0, span);
                    self.define_import_binding(alias, true, reg, span)?;
                }
                // `import a` binds the module itself.
                None if spec.path.len() == 1 => {
                    let current = self.emit_module_load(Opcode::Import, &spec.path, span)?;
                    self.define_import_binding(&spec.path[0], true, current, span)?;
                }
                // `import a.b.c` binds the *root* package name, with each
                // segment reachable as an attribute (`a.b.c.member`), matching
                // Python. The linker registers the leaf; the chain is built
                // here from nested one-pair dicts.
                None => {
                    let current = self.emit_module_load(Opcode::Import, &spec.path, span)?;
                    self.define_import_binding(&spec.path[0], true, current, span)?;
                }
            }
            self.free_all_temps();
        }
        Ok(())
    }

    /// Emit a module load (`Import` or `PyImport`) for a dotted path and build
    /// the root namespace chain for multi-segment paths, returning the
    /// register holding the value to bind. Shared so native and foreign
    /// imports bind identically.
    fn emit_module_load(
        &mut self,
        op: Opcode,
        path: &[String],
        span: Span,
    ) -> CompileResult<u8> {
        let module_path = path.join(".");
        let k = self.add_const(Value::string(module_path), span)?;
        let mut current = self.alloc_temp(span)?;
        self.emit(op, current, k, 0, span);
        let mut i = path.len();
        while i > 1 {
            i -= 1;
            let dst = self.alloc_temp(span)?;
            let kv = self.alloc_temps(2, span)?;
            let key_reg = self.load_const(Value::string(path[i].clone()), span)?;
            self.move_reg(kv, key_reg, span);
            self.move_reg(kv + 1, current, span);
            self.emit(Opcode::MakeDict, dst, 1, kv, span);
            self.free_above(dst);
            current = dst;
        }
        Ok(current)
    }

    /// `pymport pkg.sub [as alias]`: like `import` but through `PyImport`,
    /// so the module always comes from the CPython bridge and never from Zen
    /// bytecode. Binding rules mirror `import`.
    fn compile_pymport(
        &mut self,
        path: &[String],
        alias: Option<&str>,
        span: Span,
    ) -> CompileResult<()> {
        if path.is_empty() {
            return Err(CompileError::Custom {
                span,
                msg: "pymport needs a module name".to_string(),
            });
        }
        match alias {
            // `pymport a.b as x` binds the leaf module directly.
            Some(name) => {
                let module_path = path.join(".");
                let k = self.add_const(Value::string(module_path), span)?;
                let reg = self.alloc_temp(span)?;
                self.emit(Opcode::PyImport, reg, k, 0, span);
                self.define_import_binding(name, true, reg, span)?;
            }
            None if path.len() == 1 => {
                let current = self.emit_module_load(Opcode::PyImport, path, span)?;
                self.define_import_binding(&path[0], true, current, span)?;
            }
            None => {
                let current = self.emit_module_load(Opcode::PyImport, path, span)?;
                self.define_import_binding(&path[0], true, current, span)?;
            }
        }
        self.free_all_temps();
        Ok(())
    }

    fn compile_from_import(
        &mut self,
        path: &[String],
        items: &ImportItems,
        span: Span,
    ) -> CompileResult<()> {
        let module_path = path.join(".");
        let dict_k = self.add_const(Value::string(module_path), span)?;
        match items {
            ImportItems::All => {
                // A star import at module scope publishes the whole imported
                // dict; unresolved bare names then read from it (see
                // `resolve_ident`). In a function there is no persistent
                // namespace to hold it, so that form stays unsupported.
                if !self.at_module_root() {
                    return Err(self.unsupported(span, "wildcard import (*) inside a function"));
                }
                let module_reg = self.alloc_temp(span)?;
                self.emit(Opcode::Import, module_reg, dict_k, 0, span);
                let synthetic = format!("\u{0}star{}", self.star_globals.len());
                let slot = self.declare_global(&synthetic, false, span)?;
                self.emit(Opcode::SetGlobal, slot as u8, module_reg, 0, span);
                self.star_globals.push(slot);
                self.free_all_temps();
            }
            ImportItems::Names(names) => {
                let module_reg = self.alloc_temp(span)?;
                self.emit(Opcode::Import, module_reg, dict_k, 0, span);
                for ImportName {
                    name,
                    alias,
                    span: _,
                } in names
                {
                    let field_reg = self.alloc_temp(span)?;
                    let name_const = self.name_const(name, span)?;
                    self.emit(Opcode::GetField, field_reg, module_reg, name_const, span);
                    let bind_name = alias.as_ref().unwrap_or(name);
                    self.define_import_binding(bind_name, true, field_reg, span)?;
                }
            }
        }
        self.free_all_temps();
        Ok(())
    }

    // ── closures and functions ───────────────────────────────────────────
    fn compile_function_stmt(
        &mut self,
        name: &str,
        params: &[Param],
        body: &Block,
        span: Span,
    ) -> CompileResult<()> {
        let slot = self.declare_named(name, false, span)?;
        let (proto_idx, descriptors) =
            self.compile_function(name, params, FnBody::Block(body), span, None)?;
        if self.at_module_root() {
            let tmp = self.alloc_temp(span)?;
            self.emit_closure(tmp, proto_idx, &descriptors, span)?;
            self.emit(Opcode::SetGlobal, slot as u8, tmp, 0, span);
            self.free_all_temps();
        } else {
            self.emit_closure(slot as u8, proto_idx, &descriptors, span)?;
        }
        Ok(())
    }

    fn compile_closure(
        &mut self,
        name: &str,
        params: &[Param],
        body: FnBody<'_>,
        span: Span,
    ) -> CompileResult<u8> {
        let (proto_idx, descriptors) = self.compile_function(name, params, body, span, None)?;
        let dst = self.alloc_temp(span)?;
        self.emit_closure(dst, proto_idx, &descriptors, span)?;
        Ok(dst)
    }

    /// Emit `MAKE_CLOSURE` leaving the closure in `dst`, staging captured
    /// sources in the registers immediately above `dst`.
    fn emit_closure(
        &mut self,
        dst: u8,
        proto_idx: u8,
        descriptors: &[UpvalueDesc],
        span: Span,
    ) -> CompileResult<()> {
        let count = descriptors.len();
        if count > u8::MAX as usize {
            return Err(CompileError::TooManyUpvalues { span });
        }
        // Ensure the descriptor window sits directly above `dst`.
        self.free_above(dst);
        let our_top = u16::from(dst) + 1;
        if our_top + count as u16 > crate::resolver::MAX_REGISTERS {
            return Err(CompileError::TooManyRegisters { span });
        }
        for (i, desc) in descriptors.iter().enumerate() {
            let slot = dst + 1 + i as u8;
            if desc.is_local {
                self.move_reg(slot, desc.index as u8, span);
            } else {
                self.emit(Opcode::GetUpval, slot, desc.index as u8, 0, span);
            }
        }
        self.emit(Opcode::MakeClosure, dst, proto_idx, count as u8, span);
        self.free_above(dst);
        Ok(())
    }

    fn compile_function(
        &mut self,
        name: &str,
        params: &[Param],
        body: FnBody<'_>,
        span: Span,
        self_name: Option<&str>,
    ) -> CompileResult<(u8, Vec<UpvalueDesc>)> {
        if self.children.len() > u8::MAX as usize {
            return Err(CompileError::TooManyConstants { span });
        }
        let idx = self.children.len() as u8;
        self.children.push(placeholder_proto());

        let mut frame = Frame::new();

        // A method's implicit receiver occupies register 0 but is *not* an
        // entry in `params`, so call arity checks stay in terms of the
        // declared parameters.
        if let Some(sn) = self_name {
            if frame.resolver.declare_param(sn.to_string()).is_none() {
                return Err(CompileError::TooManyLocals { span });
            }
        }

        for p in params {
            if frame.resolver.declare_param(p.name.clone()).is_none() {
                return Err(CompileError::TooManyLocals { span: p.span });
            }
            frame.params.push(p.name.clone());
        }
        let saved_try_depth = std::mem::replace(&mut self.try_depth, 0);
        let saved_loop_try_bases = std::mem::take(&mut self.loop_try_bases);
        self.funcs.push(frame);
        // Default parameter values. An omitted argument arrives as `nil`, so
        // the prologue fills it in; this matches the reference interpreter,
        // which also substitutes a default when the argument is absent.
        for p in params {
            let Some(default) = &p.default else { continue };
            let slot = self.local_slot(&p.name, p.span)?;
            let val = self.alloc_temp(p.span)?;
            self.emit(Opcode::Move, val, slot, 0, p.span);
            let nil_reg = self.alloc_temp(p.span)?;
            self.emit(Opcode::LoadNil, nil_reg, 0, 0, p.span);
            let is_nil = self.alloc_temp(p.span)?;
            self.emit(Opcode::Eq, is_nil, val, nil_reg, p.span);
            let skip = self.emit_jump(Opcode::JumpIfFalse, is_nil, p.span);
            self.free_all_temps();
            let d = self.expr(default)?;
            self.emit(Opcode::Move, slot, d, 0, p.span);
            self.free_all_temps();
            let next = self.here();
            self.patch(skip, next)?;
        }
        match body {
            FnBody::Block(b) => {
                // A trailing expression statement yields its value; the oracle's
                // `exec_block` skips it and `last_expr_value` then evaluates it,
                // so the value is produced once — here by simply not compiling it
                // in statement position.
                let last = b.stmts.len().saturating_sub(1);
                let mut returned = false;
                for (i, s) in b.stmts.iter().enumerate() {
                    if i == last {
                        if let Stmt::Expr(e) = s {
                            let r = self.expr(e)?;
                            self.emit(Opcode::Return, r, 0, 0, span);
                            returned = true;
                            break;
                        }
                    }
                    self.compile_stmt(s)?;
                }
                if !returned {
                    let n = self.alloc_temp(span)?;
                    self.emit(Opcode::LoadNil, n, 0, 0, span);
                    self.emit(Opcode::Return, n, 0, 0, span);
                }
            }
            FnBody::Expr(e) => {
                let r = self.expr(e)?;
                self.emit(Opcode::Return, r, 0, 0, span);
            }
        }
        let frame = self.funcs.pop().expect("function frame");
        self.try_depth = saved_try_depth;
        self.loop_try_bases = saved_loop_try_bases;
        let descriptors = frame.resolver.pending_upvalues.clone();
        let num_upvalues = descriptors.len() as u16;
        // Defaults are trailing, so the required count is the index of the
        // first parameter that has a default value.
        let num_required = params
            .iter()
            .position(|p| p.default.is_some())
            .unwrap_or(params.len()) as u16;
        let proto = frame.chunk.into_proto(
            name.to_string(),
            frame.params,
            frame.resolver.num_locals,
            frame.resolver.max_reg,
            num_required,
            num_upvalues,
            descriptors.clone(),
            self_name.is_some(),
        );
        self.children[idx as usize] = proto;
        Ok((idx, descriptors))
    }

    // ── classes ──────────────────────────────────────────────────────────
    /// Lower `class Name [extends Parent] { … }` to a `MakeClass` plus one
    /// `MakeClosure`/`AddMethod` pair per method. The class value is bound to
    /// the class name before its methods are compiled, so a method may refer
    /// to the class through the captured name.
    fn compile_class(
        &mut self,
        name: &str,
        parent: Option<&str>,
        members: &[ClassMember],
        span: Span,
    ) -> CompileResult<()> {
        let class_slot = self.declare_named(name, false, span)?;
        let name_k = self.name_const(name, span)?;

        let parent_reg: u8 = match parent {
            Some(p) => self.expr(&Expr::Ident { name: p.to_string(), span })?,
            None => 0xFF,
        };
        // At module scope the class name is a global, so build the class in a
        // permanent scratch register and publish it once all methods are added.
        let module_root = self.at_module_root();
        let class_reg: u8 = if module_root {
            self.declare_hidden(span)? as u8
        } else {
            class_slot as u8
        };
        self.emit(Opcode::MakeClass, class_reg, name_k, parent_reg, span);

        for member in members {
            let method_name = match member.kind {
                ClassMemberKind::Init => "init",
                ClassMemberKind::Method => member.name.as_str(),
                // Class-level declared fields (with defaults) are not yet
                // lowered; instance fields are created by assignment.
                ClassMemberKind::Field => continue,
            };
            let (proto_idx, descriptors) = self.compile_function(
                method_name,
                &member.params,
                FnBody::Block(&member.body),
                member.span,
                Some("self"),
            )?;
            let mreg = self.alloc_temp(member.span)?;
            self.emit_closure(mreg, proto_idx, &descriptors, member.span)?;
            let mk = self.name_const(method_name, member.span)?;
            self.emit(Opcode::AddMethod, class_reg, mk, mreg, member.span);
            self.free_all_temps();
        }
        if module_root {
            self.emit(Opcode::SetGlobal, class_slot as u8, class_reg, 0, span);
            self.free_all_temps();
        }
        Ok(())
    }

    // ── name resolution ──────────────────────────────────────────────────
    fn resolve_ident(&mut self, name: &str, span: Span) -> CompileResult<IdentLoc> {
        if let Some(b) = self.cur().resolver.resolve_local(name) {
            return Ok(IdentLoc::Local(b.slot as u8));
        }
        if let Some(idx) = self.cur().resolver.resolve_upvalue(name) {
            return Ok(IdentLoc::Upvalue(idx));
        }
        if self.funcs.len() == 1 {
            if let Some((slot, _)) = self.module_global_map.get(name) {
                return Ok(IdentLoc::Global(*slot));
            }
            // A `from <mod> import *` at module scope makes every member of the
            // imported dict reachable as a bare name: fall back to a field read
            // on the newest star dict. Missing names still fault at runtime.
            if let Some(&slot) = self.star_globals.last() {
                return Ok(IdentLoc::StarGlobal(slot));
            }
            return Err(CompileError::UndefinedVariable {
                span,
                name: name.to_string(),
            });
        }
        let frame = self.funcs.len() - 1;
        match self.resolve_in_parent(frame, name, span)? {
            ParentRes::Desc(desc) => {
                let idx = self.cur_mut().resolver.add_upvalue(
                    name.to_string(),
                    desc.is_local,
                    desc.index,
                );
                match idx {
                    Some(i) => Ok(IdentLoc::Upvalue(i)),
                    None => Err(CompileError::TooManyUpvalues { span }),
                }
            }
            // A binding owned by the module frame is a *global*: it is not
            // captured by value but read/written in the shared module register
            // file, so mutations propagate across every function.
            ParentRes::Global(slot) => Ok(IdentLoc::Global(slot)),
            ParentRes::NotFound => Err(CompileError::UndefinedVariable {
                span,
                name: name.to_string(),
            }),
        }
    }

    fn resolve_in_parent(
        &mut self,
        frame: usize,
        name: &str,
        span: Span,
    ) -> CompileResult<ParentRes> {
        if frame == 0 {
            return Ok(ParentRes::NotFound);
        }
        let parent = frame - 1;
        if let Some(b) = self.funcs[parent].resolver.resolve_local(name) {
            // Module-frame *block* locals (not root globals) are transient and
            // captured by value like any other local.
            return Ok(ParentRes::Desc(UpvalueDesc {
                is_local: true,
                index: b.slot,
            }));
        }
        if let Some(idx) = self.funcs[parent].resolver.resolve_upvalue(name) {
            return Ok(ParentRes::Desc(UpvalueDesc {
                is_local: false,
                index: idx,
            }));
        }
        // Module globals owned by frame 0 bypass the closure system entirely.
        if parent == 0 {
            if let Some((slot, _)) = self.module_global_map.get(name) {
                return Ok(ParentRes::Global(*slot));
            }
        }
        match self.resolve_in_parent(parent, name, span)? {
            ParentRes::Desc(desc) => {
                let idx = self.funcs[parent].resolver.add_upvalue(
                    name.to_string(),
                    desc.is_local,
                    desc.index,
                );
                match idx {
                    Some(i) => Ok(ParentRes::Desc(UpvalueDesc {
                        is_local: false,
                        index: i,
                    })),
                    None => Err(CompileError::TooManyUpvalues { span }),
                }
            }
            ParentRes::Global(slot) => Ok(ParentRes::Global(slot)),
            ParentRes::NotFound => Ok(ParentRes::NotFound),
        }
    }

    // ── pattern matching ─────────────────────────────────────────────────
    //
    // `match`/`when` are expressions (`Expr::Match`/`Expr::When`).
    //
    // The subject and the result are ordinary *temporaries* taken in stack
    // order — subject, then result — and the floor mark is captured above both.
    // Every per-arm temporary (a literal, an equality result, a guard) is then
    // released back to that floor, so a long arm list costs a constant number
    // of registers instead of two per `match`. `num_locals` is untouched, which
    // matters because a module root is a single frame capped at
    // `MAX_REGISTERS`: a hidden slot per `match` exhausted it on ordinary
    // scripts.
    //
    // Freeing to the floor is best-effort by construction: an arm that binds a
    // pattern variable declares a local, which moves the temp base above the
    // floor, and `free_since` then declines to free (it only frees registers at
    // or above the base). Those temporaries stay live, which costs registers
    // but can never hand out a register that is still needed.
    //
    // The oracle (`zen-interp` `exec_match_arms`) tries arms in order, gives
    // each its own child scope, takes the first match, and yields **nil** when
    // nothing matches — there is no exhaustiveness error, so `LoadNil` seeds the
    // result register before any arm is tested.
    fn compile_match_expr(
        &mut self,
        subject: Option<&Expr>,
        arms: &[MatchArm],
        span: Span,
    ) -> CompileResult<u8> {
        let dst = self.reserve_slot(span)?;
        let subj = self.reserve_slot(span)?;
        self.emit(Opcode::LoadNil, dst, 0, 0, span);
        match subject {
            Some(e) => {
                let reg = self.expr(e)?;
                self.move_reg(subj, reg, span);
            }
            // `when` tests conditions, not a subject; the oracle still passes
            // nil, so a bare binding pattern would see nil too.
            None => self.emit(Opcode::LoadNil, subj, 0, 0, span),
        }
        // Above the subject temp: arm-local temporaries are released back to
        // this mark, never below it. `free_all_temps` would also release the
        // *enclosing* expression's temporaries — in `a + match x { … }` that is
        // `a`, and the next arm's literal would overwrite it.
        let floor = self.cur().resolver.temp_top();
        self.compile_arms(subj, floor, arms, span, ArmValue::Register(dst))?;
        // Hand the result to the enclosing expression as one of *its*
        // temporaries before returning both slots to the pool: a sibling match
        // compiled next would otherwise be handed `dst` and overwrite it.
        let out = self.alloc_temp(span)?;
        self.move_reg(out, dst, span);
        self.release_slot();
        self.release_slot();
        Ok(out)
    }

    /// `Stmt::Match` / `Stmt::When`: each arm `return`s from the enclosing
    /// function, as the statement forms have always done.
    fn compile_match(
        &mut self,
        subject: Option<&Expr>,
        arms: &[MatchArm],
        span: Span,
    ) -> CompileResult<()> {
        let subj = self.reserve_slot(span)?;
        match subject {
            Some(e) => {
                let reg = self.expr(e)?;
                self.move_reg(subj, reg, span);
            }
            None => self.emit(Opcode::LoadNil, subj, 0, 0, span),
        }
        let floor = self.cur().resolver.temp_top();
        self.compile_arms(subj, floor, arms, span, ArmValue::Return)?;
        self.release_slot();
        Ok(())
    }

    /// Test arms in order, running the first body that matches. `fail` collects
    /// the jumps each arm's own test emitted; they all land on the next arm,
    /// which is the "no match here, keep looking" edge.
    fn compile_arms(
        &mut self,
        subject: u8,
        floor: u16,
        arms: &[MatchArm],
        span: Span,
        value: ArmValue,
    ) -> CompileResult<()> {
        let mut end_jumps = Vec::new();
        for arm in arms {
            // Keep the register cost independent of the number of arms without
            // touching anything the enclosing expression is holding.
            self.cur_mut().resolver.free_to(floor);
            // One child scope per arm, mirroring the oracle: a binding from a
            // failed arm is invisible to the arms after it.
            self.cur_mut().resolver.enter_scope();
            let mut fail = Vec::new();
            self.test_pattern(&arm.pattern, subject, floor, &mut fail)?;
            match value {
                ArmValue::Return => self.compile_match_body(&arm.body)?,
                ArmValue::Register(dst) => self.compile_match_body_into(&arm.body, dst, floor)?,
            }
            end_jumps.push(self.emit_jump(Opcode::Jump, 0, span));
            let next = self.here();
            for f in fail {
                self.patch(f, next)?;
            }
            self.cur_mut().resolver.exit_scope();
        }
        let end = self.here();
        for j in end_jumps {
            self.patch(j, end)?;
        }
        Ok(())
    }

    fn test_pattern(
        &mut self,
        pattern: &Pattern,
        subject: u8,
        floor: u16,
        fail: &mut Vec<usize>,
    ) -> CompileResult<()> {
        match pattern {
            Pattern::Wildcard(_) => Ok(()),
            Pattern::Binding { name, span } => {
                self.promote_live_temps();
                let slot = self.declare_named(name, false, *span)?;
                self.move_reg(slot as u8, subject, *span);
                Ok(())
            }
            Pattern::Literal { value, span } => {
                let lit_reg = self.literal_reg(value, *span)?;
                let eq_reg = self.alloc_temp(*span)?;
                self.emit(Opcode::Eq, eq_reg, subject, lit_reg, *span);
                fail.push(self.emit_jump(Opcode::JumpIfFalse, eq_reg, *span));
                Ok(())
            }
            Pattern::Constructor { path, fields, span } => {
                // The oracle only implements the bare-capitalised case: a name
                // with no fields is tested by equality against the value bound
                // to that name. `Some(1)` and other field patterns raise
                // `unsupported` there too, so a compile error is the faithful
                // outcome rather than a runtime one.
                if fields.is_empty() && path.len() == 1 {
                    let bound = self.expr(&Expr::Ident { name: path[0].clone(), span: *span })?;
                    let eq = self.alloc_temp(*span)?;
                    self.emit(Opcode::Eq, eq, subject, bound, *span);
                    fail.push(self.emit_jump(Opcode::JumpIfFalse, eq, *span));
                    return Ok(());
                }
                Err(self.unsupported(*span, "constructor pattern"))
            }
            Pattern::List { elems, rest, span } => {
                // Without a rest the length must be exact; with one it is a
                // lower bound and the tail is collected into a fresh list,
                // which is what the oracle binds `...rest` to.
                self.test_arity(subject, elems.len(), rest.is_some(), *span, fail)?;
                for (i, p) in elems.iter().enumerate() {
                    let item = self.alloc_temp(*span)?;
                    self.emit(Opcode::IterIndex, item, subject, i as u8, *span);
                    self.test_pattern(p, item, floor, fail)?;
                    self.cur_mut().resolver.free_to(floor);
                }
                if let Some(r) = rest {
                    self.bind_rest(subject, elems.len(), r, floor, *span)?;
                }
                Ok(())
            }
            Pattern::Dict { span, .. } => Err(self.unsupported(*span, "dict pattern")),
            Pattern::Range { span, .. } => Err(self.unsupported(*span, "range pattern")),
            Pattern::Guard { pat, guards, span } => {
                // The inner pattern binds first and its guards run in the same
                // arm scope, so `n if n > 100` sees `n`.
                self.test_pattern(pat, subject, floor, fail)?;
                for guard in guards {
                    let g = self.expr(guard)?;
                    // The jump reads `g` as the very next instruction, so the
                    // temp can be recycled right away.
                    fail.push(self.emit_jump(Opcode::JumpIfFalse, g, *span));
                    self.cur_mut().resolver.free_to(floor);
                }
                Ok(())
            }
            Pattern::Or { alts, span } => {
                // An alternative that fails tries the next one; only when every
                // alternative has failed does the arm fall through. Bindings
                // made by an alternative that then failed stay visible to the
                // next, matching the oracle's single shared arm env.
                let mut matched = Vec::new();
                let mut last_fail = Vec::new();
                for (i, a) in alts.iter().enumerate() {
                    let mut local = Vec::new();
                    self.test_pattern(a, subject, floor, &mut local)?;
                    self.cur_mut().resolver.free_to(floor);
                    // If this alternative matches, jump to the end of the OR.
                    matched.push(self.emit_jump(Opcode::Jump, 0, *span));
                    if i + 1 == alts.len() {
                        last_fail = local;
                    } else {
                        // Failure jumps land at the start of the next alternative.
                        let next_alt = self.here();
                        for f in local {
                            self.patch(f, next_alt)?;
                        }
                    }
                }
                let end = self.here();
                for m in matched {
                    self.patch(m, end)?;
                }
                fail.extend(last_fail);
                Ok(())
            }
            Pattern::Bind { pat, name, span } => {
                // `pat as name`: the name is bound to the subject only after
                // `pat` matched, and it is not in scope inside `pat`. Because
                // the inner pattern's failure jumps all point at the next arm,
                // the binding instructions are simply placed after them.
                self.test_pattern(pat, subject, floor, fail)?;
                self.promote_live_temps();
                let slot = self.declare_named(name, false, *span)?;
                self.move_reg(slot as u8, subject, *span);
                Ok(())
            }
            Pattern::Tuple { elems, span } => {
                // `#(a, b)` destructures any iterable of exactly that length —
                // the oracle measures with `iter_len`, so a list or a string of
                // the same length matches too.
                self.test_arity(subject, elems.len(), false, *span, fail)?;
                for (i, p) in elems.iter().enumerate() {
                    let item = self.alloc_temp(*span)?;
                    self.emit(Opcode::IterIndex, item, subject, i as u8, *span);
                    self.test_pattern(p, item, floor, fail)?;
                    self.cur_mut().resolver.free_to(floor);
                }
                Ok(())
            }
            Pattern::Condition { cond, span } => {
                // `when` arm heads are conditions. The bare `_` fallback reaches
                // the oracle as an identifier expression, which is not a name
                // anything can resolve, so it is matched here instead.
                if matches!(cond.as_ref(), Expr::Ident { name, .. } if name == "_") {
                    return Ok(());
                }
                let c = self.expr(cond)?;
                fail.push(self.emit_jump(Opcode::JumpIfFalse, c, *span));
                self.cur_mut().resolver.free_to(floor);
                Ok(())
            }
        }
    }

    /// Promote every live temporary to a local slot.
    ///
    /// [`Resolver::declare_hidden`] hands out `num_locals`, which is the *lowest*
    /// live temporary, so a binding declared while temporaries are in flight
    /// would take over one of them. A destructuring sub-pattern is exactly that
    /// case: `Pattern::List`/`Pattern::Tuple` pass an element extracted into a
    /// temporary, and the enclosing pattern still indexes it for the remaining
    /// elements. Promoting the temporaries first leaves `num_locals` free and
    /// keeps every value where it is.
    fn promote_live_temps(&mut self) {
        self.cur_mut().resolver.spill_temps();
    }

    /// The length precondition of a destructuring pattern. `IterLen` yields 0
    /// for a value that cannot be iterated, so a non-iterable subject fails the
    /// check (or fails the `>=` for a rest pattern) rather than raising.
    fn test_arity(
        &mut self,
        subject: u8,
        n: usize,
        at_least: bool,
        span: Span,
        fail: &mut Vec<usize>,
    ) -> CompileResult<()> {
        // This runs before any of the pattern's own element registers exist, so
        // the lowest live temporary belongs to the *enclosing* destructuring:
        // `item` in `Pattern::List`/`Pattern::Tuple`, which later sub-patterns
        // still index into. Leaving `len` alive would put it under a binding's
        // new local slot (see `Resolver::declare_hidden`, which takes
        // `num_locals`), so a pattern like `#([a], [b, c])` would overwrite the
        // element register its sibling reads.
        let mark = self.cur_mut().resolver.temp_top();
        let len = self.alloc_temp(span)?;
        self.emit(Opcode::IterLen, len, subject, 0, span);
        let want = self.literal_reg(&Literal::Int(n as i64), span)?;
        let ok = self.alloc_temp(span)?;
        self.emit(
            if at_least { Opcode::Ge } else { Opcode::Eq },
            ok,
            len,
            want,
            span,
        );
        fail.push(self.emit_jump(Opcode::JumpIfFalse, ok, span));
        // The failure jump reads `ok` as the very next instruction, so all three
        // registers are dead here. Free back to `mark` rather than the arm's
        // floor: the enclosing `item` register sits below `mark` and is live.
        self.cur_mut().resolver.free_to(mark);
        Ok(())
    }

    /// `...rest` in a list pattern: copy `subject[n..]` into a new list and
    /// match the rest pattern against it.
    fn bind_rest(
        &mut self,
        subject: u8,
        from: usize,
        rest: &Pattern,
        floor: u16,
        span: Span,
    ) -> CompileResult<()> {
        let len = self.alloc_temp(span)?;
        self.emit(Opcode::IterLen, len, subject, 0, span);

        let acc = self.alloc_temp(span)?;
        self.emit(Opcode::MakeList, acc, 0, 0, span);

        // The counter and its successor have to outlive one iteration, so they
        // are temps above `floor` that the loop's own jumps do not free.
        let counter = self.alloc_temp(span)?;
        let start = self.literal_reg(&Literal::Int(from as i64), span)?;
        self.move_reg(counter, start, span);
        let one = self.literal_reg(&Literal::Int(1), span)?;

        let head = self.here();
        let more = self.alloc_temp(span)?;
        self.emit(Opcode::Lt, more, counter, len, span);
        let done = self.emit_jump(Opcode::JumpIfFalse, more, span);
        let item = self.alloc_temp(span)?;
        self.emit(Opcode::IterIndex, item, subject, counter, span);
        self.emit(Opcode::ListAppend, acc, item, 0, span);
        let next = self.alloc_temp(span)?;
        self.emit(Opcode::Add, next, counter, one, span);
        self.move_reg(counter, next, span);
        let back = self.emit_jump(Opcode::Jump, 0, span);
        self.patch(back, head)?;
        self.patch(done, self.here())?;

        self.test_pattern(rest, acc, floor, &mut Vec::new())?;
        Ok(())
    }

    /// An arm body that yields a value: an expression moves into `dst`, and a
    /// block body yields nil (the oracle's `arm_body` runs the statements and
    /// discards their values).
    fn compile_match_body_into(
        &mut self,
        body: &BlockOrExpr,
        dst: u8,
        floor: u16,
    ) -> CompileResult<()> {
        let span = Span::empty_at(self.file, 0);
        match body {
            BlockOrExpr::Expr(e) => {
                let r = self.expr(e)?;
                self.move_reg(dst, r, span);
            }
            BlockOrExpr::Block(b) => {
                self.compile_block(b)?;
                self.emit(Opcode::LoadNil, dst, 0, 0, span);
            }
        }
        self.cur_mut().resolver.free_to(floor);
        Ok(())
    }

    fn compile_match_body(&mut self, body: &BlockOrExpr) -> CompileResult<()> {
        match body {
            BlockOrExpr::Expr(e) => {
                let r = self.expr(e)?;
                self.emit(Opcode::Return, r, 0, 0, self.expr_span(e));
            }
            BlockOrExpr::Block(b) => {
                self.compile_block(b)?;
                let n = self.alloc_temp(Span::empty_at(self.file, 0))?;
                self.emit(Opcode::LoadNil, n, 0, 0, Span::empty_at(self.file, 0));
                self.emit(Opcode::Return, n, 0, 0, Span::empty_at(self.file, 0));
            }
        }
        Ok(())
    }

    // ── expressions ──────────────────────────────────────────────────────
    fn expr(&mut self, e: &Expr) -> CompileResult<u8> {
        let span = self.expr_span(e);
        match e {
            Expr::Literal(lit) => self.literal_reg(lit, span),
            Expr::Ident { name, span: ispan } => match self.resolve_ident(name, *ispan)? {
                IdentLoc::Local(slot) => Ok(slot),
                IdentLoc::Upvalue(idx) => {
                    let dst = self.alloc_temp(span)?;
                    self.emit(Opcode::GetUpval, dst, idx as u8, 0, span);
                    Ok(dst)
                }
                IdentLoc::Global(slot) => {
                    let dst = self.alloc_temp(span)?;
                    self.emit(Opcode::GetGlobal, dst, slot as u8, 0, span);
                    Ok(dst)
                }
                IdentLoc::StarGlobal(slot) => {
                    let dict = self.alloc_temp(span)?;
                    self.emit(Opcode::GetGlobal, dict, slot as u8, 0, span);
                    let key = self.name_const(name, span)?;
                    let dst = self.alloc_temp(span)?;
                    self.emit(Opcode::GetField, dst, dict, key, span);
                    Ok(dst)
                }
            },
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => {
                if *op == BinaryOp::And || *op == BinaryOp::Or {
                    return self.compile_and_or(left, right, *op == BinaryOp::And, *span);
                }
                let l = self.expr(left)?;
                let r = self.expr(right)?;
                let dst = self.alloc_temp(*span)?;
                self.emit_binop(*op, dst, l, r, *span)?;
                Ok(dst)
            }
            Expr::Unary { op, operand, span } => {
                if *op == UnaryOp::TypeOf {
                    // `typeof x` and `typeof(x)` both parse to this prefix
                    // operator: the `(` is consumed by the operand's own primary,
                    // so the parenthesised form is never a call. The type name
                    // comes from native 17 — the same one `type(x)` reaches.
                    return self.compile_native_call(std::slice::from_ref(&**operand), 17, *span);
                }
                let r = self.expr(operand)?;
                let dst = self.alloc_temp(*span)?;
                let oc = match op {
                    UnaryOp::Neg => Opcode::Neg,
                    UnaryOp::Not => Opcode::Not,
                    UnaryOp::BitNot => Opcode::BitNot,
                    UnaryOp::TypeOf => unreachable!("handled above"),
                };
                self.emit(oc, dst, r, 0, *span);
                Ok(dst)
            }
            Expr::Call { callee, args, span } => self.compile_call(callee, args, *span),
            Expr::MethodCall {
                receiver,
                method,
                args,
                span,
            } => self.compile_method_call(receiver, method, args, *span),
            Expr::Index {
                object,
                index,
                span,
            } => {
                let o = self.expr(object)?;
                let k = self.expr(index)?;
                let dst = self.alloc_temp(*span)?;
                self.emit(Opcode::GetIndex, dst, o, k, *span);
                Ok(dst)
            }
            Expr::Field {
                object,
                field,
                span,
            } => {
                let o = self.expr(object)?;
                let dst = self.alloc_temp(*span)?;
                let k = self.name_const(field, *span)?;
                self.emit(Opcode::GetField, dst, o, k, *span);
                Ok(dst)
            }
            Expr::OptionalField {
                object,
                field,
                span,
            } => self.compile_optional_field(object, field, *span),
            Expr::List(items) => self.compile_seq(items, Opcode::MakeList, span),
            Expr::Set(items) => self.compile_seq(items, Opcode::MakeSet, span),
            Expr::Tuple(items) => self.compile_seq(items, Opcode::MakeTuple, span),
            Expr::Dict(pairs) => self.compile_dict(pairs, span),
            Expr::SetLiteral(items) => self.compile_seq(items, Opcode::MakeSet, span),
            Expr::TupleLiteral(items) => self.compile_seq(items, Opcode::MakeTuple, span),
            Expr::DictLiteral(map) => {
                let pairs: Vec<(Expr, Expr)> = map
                    .iter()
                    .map(|(k, v)| (Expr::Literal(Literal::String(k.clone())), v.clone()))
                    .collect();
                self.compile_dict(&pairs, span)
            }
            Expr::Lambda { params, body, span } => {
                self.compile_closure("<lambda>", params, FnBody::Expr(body), *span)
            }
            Expr::FnExpr { params, body, span } => {
                self.compile_closure("<fn>", params, FnBody::Block(body), *span)
            }
            Expr::ArrowFn { params, body, span } => {
                let b = match body {
                    BlockOrExpr::Expr(e) => FnBody::Expr(e),
                    BlockOrExpr::Block(blk) => FnBody::Block(blk),
                };
                self.compile_closure("<arrow>", params, b, *span)
            }
            Expr::Ternary {
                cond,
                then_branch,
                else_branch,
                span,
            } => self.compile_ternary(cond, then_branch, else_branch, *span),
            Expr::NullCoalesce { left, right, span } => {
                self.compile_null_coalesce(left, right, *span)
            }
            Expr::Chain {
                ops,
                operands,
                span,
            } => self.compile_chain(ops, operands, *span),
            Expr::InterpolatedString(parts) => self.compile_interp(parts, span),
            Expr::IfExpr {
                arms,
                otherwise,
                span,
            } => self.compile_if_expr(arms, otherwise.as_ref(), *span),
            Expr::Slice {
                object,
                start,
                end,
                span,
            } => {
                // The subject is evaluated *first* so its register sits below
                // `dst` and the bounds. `eval_into` frees everything above the
                // slot it wrote, so a subject allocated after the bounds would
                // be handed back by the allocator and overwritten by a bound
                // expression — `a[1:-4]` read `-4` as the subject that way.
                let subject = self.expr(object)?;
                // `dst` below the bounds, and the two bounds reserved as one
                // consecutive pair so a single operand addresses both — the
                // `MakeList` first/count idiom, and the same shape
                // `compile_seq` already relies on. An omitted bound becomes
                // `nil` so the runtime can tell it from an explicit `0`, which
                // means "to the start" rather than "to the end".
                let dst = self.alloc_temp(*span)?;
                let bounds = self.alloc_temps(2, *span)?;
                match start {
                    Some(e) => self.eval_into(e, bounds, *span)?,
                    None => self.emit(Opcode::LoadNil, bounds, 0, 0, *span),
                }
                match end {
                    Some(e) => self.eval_into(e, bounds + 1, *span)?,
                    None => self.emit(Opcode::LoadNil, bounds + 1, 0, 0, *span),
                }
                self.emit(Opcode::Slice, dst, subject, bounds, *span);
                self.free_above(dst);
                Ok(dst)
            }
            Expr::Comprehension {
                kind,
                key,
                body,
                clauses,
                span,
            } => self.compile_comprehension(*kind, key.as_deref(), body, clauses, *span),
            Expr::Range { span, .. } => Err(self.unsupported(*span, "range")),
            Expr::Match { subject, arms, .. } => {
                // The hidden result slot outlives the caller's temps.
                let dst = self.compile_match_expr(Some(subject), arms, span)?;
                self.free_all_temps();
                Ok(dst)
            }
            Expr::When { arms, .. } => {
                let dst = self.compile_match_expr(None, arms, span)?;
                self.free_all_temps();
                Ok(dst)
            }
            Expr::New { class, args, span } => {
                let callee = Expr::Ident { name: class.clone(), span: *span };
                self.compile_call(&callee, args, *span)
            }
            Expr::SuperCall { span, .. } => Err(self.unsupported(*span, "super()")),
            Expr::SuperMethod { span, .. } => Err(self.unsupported(*span, "super.method()")),
        }
    }

    fn expr_span(&self, e: &Expr) -> Span {
        match e {
            Expr::Literal(_) => self.empty_span(),
            Expr::Ident { span, .. } => *span,
            Expr::Binary { span, .. } => *span,
            Expr::Unary { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::MethodCall { span, .. } => *span,
            Expr::Index { span, .. } => *span,
            Expr::Slice { span, .. } => *span,
            Expr::Field { span, .. } => *span,
            Expr::OptionalField { span, .. } => *span,
            Expr::List(_)
            | Expr::Set(_)
            | Expr::Tuple(_)
            | Expr::Dict(_)
            | Expr::SetLiteral(_)
            | Expr::TupleLiteral(_)
            | Expr::DictLiteral(_) => self.empty_span(),
            Expr::Lambda { span, .. } => *span,
            Expr::FnExpr { span, .. } => *span,
            Expr::ArrowFn { span, .. } => *span,
            Expr::Ternary { span, .. } => *span,
            Expr::NullCoalesce { span, .. } => *span,
            Expr::Chain { span, .. } => *span,
            Expr::InterpolatedString(_) => self.empty_span(),
            Expr::IfExpr { span, .. } => *span,
            Expr::Comprehension { span, .. } => *span,
            Expr::Range { span, .. } => *span,
            Expr::Match { span, .. } => *span,
            Expr::When { span, .. } => *span,
            Expr::New { span, .. } => *span,
            Expr::SuperCall { span, .. } => *span,
            Expr::SuperMethod { span, .. } => *span,
        }
    }

    fn literal_reg(&mut self, lit: &Literal, span: Span) -> CompileResult<u8> {
        let dst = self.alloc_temp(span)?;
        match lit {
            Literal::Nil => self.emit(Opcode::LoadNil, dst, 0, 0, span),
            Literal::Bool(true) => self.emit(Opcode::LoadTrue, dst, 0, 0, span),
            Literal::Bool(false) => self.emit(Opcode::LoadFalse, dst, 0, 0, span),
            Literal::Int(i) => {
                let k = self.add_const(Value::int_promote(*i), span)?;
                self.emit(Opcode::LoadConst, dst, k, 0, span);
            }
            Literal::Float(f) => {
                let k = self.add_const(Value::float(*f), span)?;
                self.emit(Opcode::LoadConst, dst, k, 0, span);
            }
            Literal::String(s) => {
                let k = self.add_const(Value::string(s.clone()), span)?;
                self.emit(Opcode::LoadConst, dst, k, 0, span);
            }
            Literal::Bytes(b) => {
                let k = self.add_const(Value::bytes(b.clone()), span)?;
                self.emit(Opcode::LoadConst, dst, k, 0, span);
            }
        }
        Ok(dst)
    }

    fn emit_binop(&mut self, op: BinaryOp, dst: u8, l: u8, r: u8, span: Span) -> CompileResult<()> {
        let oc = match op {
            BinaryOp::Add => Opcode::Add,
            BinaryOp::Sub => Opcode::Sub,
            BinaryOp::Mul => Opcode::Mul,
            BinaryOp::Div => Opcode::Div,
            BinaryOp::Mod => Opcode::Mod,
            BinaryOp::Pow => Opcode::Pow,
            BinaryOp::Eq | BinaryOp::StrictEq => Opcode::Eq,
            BinaryOp::Ne | BinaryOp::StrictNe => Opcode::Ne,
            BinaryOp::Lt => Opcode::Lt,
            BinaryOp::Le => Opcode::Le,
            BinaryOp::Gt => Opcode::Gt,
            BinaryOp::Ge => Opcode::Ge,
            BinaryOp::BitAnd => Opcode::BitAnd,
            BinaryOp::BitOr => Opcode::BitOr,
            BinaryOp::BitXor => Opcode::BitXor,
            BinaryOp::Shl => Opcode::Shl,
            BinaryOp::Shr => Opcode::Shr,
            BinaryOp::And => Opcode::And,
            BinaryOp::Or => Opcode::Or,
            BinaryOp::Nullish => {
                return Err(self.unsupported(span, "?? operator should not reach here"))
            }
            BinaryOp::In => return Err(self.unsupported(span, "in")),
            BinaryOp::NotIn => return Err(self.unsupported(span, "not in")),
            BinaryOp::Is => return Err(self.unsupported(span, "is")),
            BinaryOp::IsNot => return Err(self.unsupported(span, "is not")),
            BinaryOp::Range => Opcode::MakeRange,
        };
        self.emit(oc, dst, l, r, span);
        Ok(())
    }

    fn compile_and_or(
        &mut self,
        left: &Expr,
        right: &Expr,
        is_and: bool,
        span: Span,
    ) -> CompileResult<u8> {
        let dst = self.alloc_temp(span)?;
        let l = self.expr(left)?;
        self.move_reg(dst, l, span);
        self.free_above(dst);
        let jump_op = if is_and {
            Opcode::JumpIfFalse
        } else {
            Opcode::JumpIfTrue
        };
        let j = self.emit_jump(jump_op, dst, span);
        let r = self.expr(right)?;
        self.move_reg(dst, r, span);
        self.free_above(dst);
        let end = self.here();
        self.patch(j, end)?;
        Ok(dst)
    }

    fn compile_call(&mut self, callee: &Expr, args: &[Expr], span: Span) -> CompileResult<u8> {
        if args.len() > u8::MAX as usize {
            return Err(self.unsupported(span, "calls with more than 255 arguments"));
        }
        // Check for builtin functions (an unbound name only: a program binding
        // with the same name takes precedence).
        if let Expr::Ident { name, .. } = callee {
            if !self.name_is_bound(name) {
                if let Some(native_id) = self.is_builtin(name) {
                    return self.compile_native_call(args, native_id, span);
                }
            }
        }
        let argc = args.len() as u8;
        let base = self.alloc_temps(u16::from(argc) + 2, span)?;
        self.eval_into(callee, base, span)?;
        for (i, a) in args.iter().enumerate() {
            self.eval_into(a, base + 1 + i as u8, span)?;
        }
        let result = base + 1 + argc;
        self.emit(Opcode::Call, result, argc, base + 1, span);
        self.move_reg(base, result, span);
        self.free_above(base);
        Ok(base)
    }

    fn compile_native_call(&mut self, args: &[Expr], native: u16, span: Span) -> CompileResult<u8> {
        let argc = args.len();
        if argc > u8::MAX as usize {
            return Err(self.unsupported(span, "calls with more than 255 arguments"));
        }
        let base = self.alloc_temps(argc as u16 + 1, span)?;
        for (i, a) in args.iter().enumerate() {
            self.eval_into(a, base + i as u8, span)?;
        }
        let result = base + argc as u8;
        if native <= u8::MAX as u16 {
            self.emit(Opcode::CallNative, result, native as u8, argc as u8, span);
        } else {
            self.emit(Opcode::CallNativeW, result, argc as u8, 0, span);
            let line = self.line_of(span);
            self.cur_mut()
                .chunk
                .emit_raw(Instruction::native_wide_payload(native), line);
        }
        self.move_reg(base, result, span);
        self.free_above(base);
        Ok(base)
    }

    fn compile_method_call(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[Expr],
        span: Span,
    ) -> CompileResult<u8> {
        if args.len() > u8::MAX as usize {
            return Err(self.unsupported(span, "calls with more than 255 arguments"));
        }
        let argc = args.len() as u8;
        let base = self.alloc_temps(u16::from(argc) + 2, span)?;
        self.eval_into(receiver, base, span)?;
        for (i, a) in args.iter().enumerate() {
            self.eval_into(a, base + 1 + i as u8, span)?;
        }
        let name = self.name_const(method, span)?;
        let result = base + 1 + argc;
        self.emit(Opcode::CallMethod, result, name, argc, span);
        self.move_reg(base, result, span);
        self.free_above(base);
        Ok(base)
    }

    fn compile_optional_field(
        &mut self,
        object: &Expr,
        field: &str,
        span: Span,
    ) -> CompileResult<u8> {
        let dst = self.alloc_temp(span)?;
        let o = self.expr(object)?;
        self.move_reg(dst, o, span);
        self.free_above(dst);
        let nil = self.alloc_temp(span)?;
        self.emit(Opcode::LoadNil, nil, 0, 0, span);
        let is_nil = self.alloc_temp(span)?;
        self.emit(Opcode::Eq, is_nil, dst, nil, span);
        let jf = self.emit_jump(Opcode::JumpIfFalse, is_nil, span);
        self.free_above(dst);
        let jend = self.emit_jump(Opcode::Jump, 0, span);
        let present = self.here();
        self.patch(jf, present)?;
        let k = self.name_const(field, span)?;
        self.emit(Opcode::GetField, dst, dst, k, span);
        let end = self.here();
        self.patch(jend, end)?;
        Ok(dst)
    }

    fn compile_ternary(
        &mut self,
        cond: &Expr,
        then_branch: &Expr,
        else_branch: &Expr,
        span: Span,
    ) -> CompileResult<u8> {
        let dst = self.alloc_temp(span)?;
        let c = self.expr(cond)?;
        let jf = self.emit_jump(Opcode::JumpIfFalse, c, span);
        self.free_above(dst);
        let t = self.expr(then_branch)?;
        self.move_reg(dst, t, span);
        self.free_above(dst);
        let jend = self.emit_jump(Opcode::Jump, 0, span);
        let els = self.here();
        self.patch(jf, els)?;
        let e = self.expr(else_branch)?;
        self.move_reg(dst, e, span);
        self.free_above(dst);
        let end = self.here();
        self.patch(jend, end)?;
        Ok(dst)
    }

    /// `if c { … } elif c2 { … } else { … }` in expression position. Purely a
    /// desugaring: every arm writes its value into one shared register, so the
    /// VM needs no join opcode.
    fn compile_if_expr(
        &mut self,
        arms: &[(Expr, Block)],
        otherwise: Option<&Block>,
        span: Span,
    ) -> CompileResult<u8> {
        let dst = self.alloc_temp(span)?;
        let mut ends: Vec<usize> = Vec::new();
        for (cond, body) in arms {
            let c = self.expr(cond)?;
            let jf = self.emit_jump(Opcode::JumpIfFalse, c, span);
            self.free_above(dst);
            self.compile_block_value(body, dst, span)?;
            ends.push(self.emit_jump(Opcode::Jump, 0, span));
            let next = self.here();
            self.patch(jf, next)?;
            self.free_above(dst);
        }
        match otherwise {
            Some(b) => self.compile_block_value(b, dst, span)?,
            // No `else` and nothing matched yields nil, like the oracle.
            None => self.emit(Opcode::LoadNil, dst, 0, 0, span),
        }
        let end = self.here();
        for j in ends.drain(..) {
            self.patch(j, end)?;
        }
        self.free_above(dst);
        Ok(dst)
    }

    /// Compile `block` for its value, leaving the result in `dst`. A block's
    /// value is its last statement's value when that statement is an
    /// expression, and nil otherwise. The arm body deliberately does **not**
    /// open a scope: the oracle evaluates arm blocks in the enclosing
    /// environment, so a binding inside one stays visible afterwards.
    fn compile_block_value(&mut self, block: &Block, dst: u8, span: Span) -> CompileResult<()> {
        let last = block.stmts.len().saturating_sub(1);
        for (i, s) in block.stmts.iter().enumerate() {
            if i == last {
                if let Stmt::Expr(e) = s {
                    let r = self.expr(e)?;
                    self.move_reg(dst, r, span);
                    self.free_above(dst);
                    return Ok(());
                }
            }
            self.compile_stmt(s)?;
            self.free_above(dst);
        }
        self.emit(Opcode::LoadNil, dst, 0, 0, span);
        Ok(())
    }

    fn compile_null_coalesce(
        &mut self,
        left: &Expr,
        right: &Expr,
        span: Span,
    ) -> CompileResult<u8> {
        let dst = self.alloc_temp(span)?;
        let l = self.expr(left)?;
        self.move_reg(dst, l, span);
        self.free_above(dst);
        let nil = self.alloc_temp(span)?;
        self.emit(Opcode::LoadNil, nil, 0, 0, span);
        let is_nil = self.alloc_temp(span)?;
        self.emit(Opcode::Eq, is_nil, dst, nil, span);
        let jf = self.emit_jump(Opcode::JumpIfFalse, is_nil, span);
        self.free_above(dst);
        let r = self.expr(right)?;
        self.move_reg(dst, r, span);
        self.free_above(dst);
        let end = self.here();
        self.patch(jf, end)?;
        Ok(dst)
    }

    fn compile_chain(
        &mut self,
        ops: &[BinaryOp],
        operands: &[Expr],
        span: Span,
    ) -> CompileResult<u8> {
        let dst = self.alloc_temp(span)?;
        self.emit(Opcode::LoadTrue, dst, 0, 0, span);
        let base = self.alloc_temps(operands.len() as u16, span)?;
        for (i, operand) in operands.iter().enumerate() {
            let r = self.expr(operand)?;
            self.move_reg(base + i as u8, r, span);
            self.free_above(base + i as u8);
        }
        for (i, op) in ops.iter().enumerate() {
            let t = self.alloc_temp(span)?;
            self.emit_binop(*op, t, base + i as u8, base + i as u8 + 1, span)?;
            self.emit(Opcode::And, dst, dst, t, span);
            self.free_above(dst);
        }
        Ok(dst)
    }

    fn compile_seq(&mut self, items: &[Expr], op: Opcode, span: Span) -> CompileResult<u8> {
        if items.len() > u8::MAX as usize {
            return Err(self.unsupported(span, "containers with more than 255 elements"));
        }
        let dst = self.alloc_temp(span)?;
        let first = self.alloc_temps(items.len() as u16, span)?;
        for (i, item) in items.iter().enumerate() {
            let slot = first + i as u8;
            self.eval_into(item, slot, span)?;
        }
        self.emit(op, dst, items.len() as u8, first, span);
        self.free_above(dst);
        Ok(dst)
    }

    fn compile_dict(&mut self, pairs: &[(Expr, Expr)], span: Span) -> CompileResult<u8> {
        if pairs.len() > u8::MAX as usize {
            return Err(self.unsupported(span, "dicts with more than 255 pairs"));
        }
        let dst = self.alloc_temp(span)?;
        let first = self.alloc_temps(pairs.len() as u16 * 2, span)?;
        for (i, (k, v)) in pairs.iter().enumerate() {
            let kb = first + (i as u8) * 2;
            self.dict_key_into(k, kb, span)?;
            self.eval_into(v, kb + 1, span)?;
        }
        self.emit(Opcode::MakeDict, dst, pairs.len() as u8, first, span);
        self.free_above(dst);
        Ok(dst)
    }

    /// A dict key. A bare identifier (`#{name: …}`) is a string key, matching
    /// `docs/03-syntax.md`; anything else is evaluated as an expression.
    fn dict_key_into(&mut self, key: &Expr, slot: u8, span: Span) -> CompileResult<()> {
        match key {
            Expr::Ident { name, .. } => {
                let r = self.load_const(Value::string(name.clone()), span)?;
                self.move_reg(slot, r, span);
                self.free_above(slot);
                Ok(())
            }
            other => self.eval_into(other, slot, span),
        }
    }

    fn compile_interp(&mut self, parts: &[StringPart], span: Span) -> CompileResult<u8> {
        let mut acc: Option<u8> = None;
        for part in parts {
            let reg = match part {
                StringPart::Text(t) => self.load_const(Value::string(t.clone()), span)?,
                StringPart::Interpolation(e) => self.expr(e)?,
            };
            acc = Some(match acc {
                None => reg,
                Some(a) => {
                    self.emit(Opcode::Add, a, a, reg, span);
                    self.free_above(a);
                    a
                }
            });
        }
        match acc {
            Some(r) => Ok(r),
            None => self.load_const(Value::string(String::new()), span),
        }
    }

    // ── closures and functions ───────────────────────────────────────────
    // (compile_function_stmt, compile_closure, emit_closure, compile_function already defined above)

    // ── control flow ─────────────────────────────────────────────────────
    fn compile_if(
        &mut self,
        cond: &Expr,
        then_branch: &Block,
        elifs: &[(Expr, Block)],
        else_branch: Option<&Block>,
        span: Span,
    ) -> CompileResult<()> {
        let mut end_jumps = Vec::new();
        let mut branches: Vec<(&Expr, &Block)> = Vec::with_capacity(1 + elifs.len());
        branches.push((cond, then_branch));
        for (c, b) in elifs {
            branches.push((c, b));
        }
        for (i, (c, b)) in branches.iter().enumerate() {
            let r = self.expr(c)?;
            let jf = self.emit_jump(Opcode::JumpIfFalse, r, span);
            self.free_all_temps();
            self.compile_block(b)?;
            let more = i + 1 < branches.len() || else_branch.is_some();
            if more {
                end_jumps.push(self.emit_jump(Opcode::Jump, 0, span));
            }
            let next = self.here();
            self.patch(jf, next)?;
        }
        if let Some(else_b) = else_branch {
            self.compile_block(else_b)?;
        }
        let end = self.here();
        for j in end_jumps {
            self.patch(j, end)?;
        }
        Ok(())
    }

    fn compile_while(&mut self, cond: &Expr, body: &Block, span: Span) -> CompileResult<()> {
        let head = self.here();
        self.cur_mut().resolver.enter_loop(head);
        self.loop_try_bases.push(self.try_depth);
        let r = self.expr(cond)?;
        let jf = self.emit_jump(Opcode::JumpIfFalse, r, span);
        self.free_all_temps();
        self.compile_block(body)?;
        let back = self.emit_jump(Opcode::Jump, 0, span);
        self.patch(back, head)?;
        let end = self.here();
        self.patch(jf, end)?;
        let loop_ctx = self.cur_mut().resolver.exit_loop().expect("loop");
        self.loop_try_bases.pop();
        for br in loop_ctx.breaks {
            self.patch(br, end)?;
        }
        self.free_all_temps();
        Ok(())
    }

    fn compile_for(
        &mut self,
        vars: &[String],
        iter: &Expr,
        body: &Block,
        span: Span,
    ) -> CompileResult<()> {
        if vars.len() > u8::MAX as usize {
            return Err(CompileError::TooManyLocals { span });
        }
        let seq = self.expr(iter)?;
        let seq_slot = self.declare_hidden(span)?;
        self.move_reg(seq_slot as u8, seq, span);
        self.free_all_temps();

        self.cur_mut().resolver.enter_scope();
        let iter_slot = self.declare_hidden(span)?;
        let _cursor_slot = self.declare_hidden(span)?;
        self.emit(Opcode::IterInit, iter_slot as u8, seq_slot as u8, 0, span);

        let head = self.here();
        self.cur_mut().resolver.enter_loop(head);
        self.loop_try_bases.push(self.try_depth);
        let done = self.declare_hidden(span)?;
        self.emit(Opcode::IterDone, iter_slot as u8, done as u8, 0, span);
        let jf = self.emit_jump(Opcode::JumpIfTrue, done as u8, span);

        if vars.len() == 1 {
            // Nothing to unpack: the iterator can write straight into the name.
            let var_slot = self.declare_named(&vars[0], false, span)?;
            self.emit(Opcode::IterNext, iter_slot as u8, var_slot as u8, 0, span);
        } else {
            // Each variable binds one element of the item, so the item lands in
            // its own slot first: its arity is checked before anything is bound
            // (a short item raises `ValueError`, exactly like the oracle) and
            // only then are the elements moved into the names.
            let item = self.declare_hidden(span)?;
            self.emit(Opcode::IterNext, iter_slot as u8, item as u8, 0, span);
            let msg = self.add_const(Value::string(UNPACK_FOR_ERROR.to_string()), span)?;
            self.emit(Opcode::UnpackCheck, item as u8, vars.len() as u8, msg, span);
            for (i, name) in vars.iter().enumerate() {
                let slot = self.declare_named(name, false, span)?;
                self.emit(Opcode::IterIndex, slot as u8, item as u8, i as u8, span);
            }
        }
        self.compile_block(body)?;
        let back = self.emit_jump(Opcode::Jump, 0, span);
        self.patch(back, head)?;

        let end = self.here();
        self.patch(jf, end)?;
        let loop_ctx = self.cur_mut().resolver.exit_loop().expect("loop");
        self.loop_try_bases.pop();
        for br in loop_ctx.breaks {
            self.patch(br, end)?;
        }
        self.cur_mut().resolver.exit_scope();
        self.free_all_temps();
        Ok(())
    }

    /// `[body for var in iter (if guard)? (for var2 in iter2 …)?]`
    ///
    /// The oracle (`eval_comprehension` → `run_clauses`) gives every clause its
    /// own child scope, evaluates each clause's iterable once *per outer
    /// iteration*, and appends the body value for every iteration whose guard is
    /// truthy. So the lowering is the `IterInit`/`IterDone`/`IterNext` skeleton
    /// `compile_for` already uses, with the accumulator in an enclosing scope
    /// (so clause scopes nest inside it and an inner clause can read the outer
    /// clause's variable), `ListAppend` where `compile_for` compiles statements,
    /// and the guard's `JumpIfFalse` jumping back to the loop head.
    ///
    /// The accumulator is a reserved slot above the enclosing expression's live
    /// temporaries (`f(1) + [x for x in xs]` still needs `f(1)`'s register), and
    /// so is the clause state, which lets the clause loop release *all*
    /// temporaries per iteration — register cost independent of clause count.
    /// The result is copied into a fresh temp while the scope is still open,
    /// before `exit_scope` drops the slots.
    ///
    /// No loop context is entered. The oracle rejects `break`/`continue` in a
    /// comprehension body ("break/continue outside loop") because `run_clauses`
    /// is a plain `for idx in 0..len`, so the compiler must reject it too.
    fn compile_comprehension(
        &mut self,
        kind: CompKind,
        key: Option<&Expr>,
        body: &Expr,
        clauses: &[CompClause],
        span: Span,
    ) -> CompileResult<u8> {
        match kind {
            CompKind::List => {}
            CompKind::Set => return Err(self.unsupported(span, "set comprehension")),
            CompKind::Dict => return Err(self.unsupported(span, "dict comprehension")),
        }
        if key.is_some() {
            return Err(self.unsupported(span, "dict comprehension key"));
        }
        if clauses.is_empty() {
            return Err(self.unsupported(span, "comprehension without a `for` clause"));
        }

        self.cur_mut().resolver.enter_scope();
        let acc = self.reserve_slot(span)?;
        // Matches the oracle's `list_out` existing before `run_clauses` walks
        // the first clause.
        self.emit(Opcode::MakeList, acc, 0, 0, span);
        let floor = self.cur().resolver.temp_top();
        self.compile_comp_clauses(clauses, body, 0, acc, floor, span)?;
        // Copy the accumulator out before the clause scope closes, then release
        // it with everything else the comprehension allocated.
        let dst = self.alloc_temp(span)?;
        self.move_reg(dst, acc, span);
        self.cur_mut().resolver.exit_scope();
        self.release_slot();
        Ok(dst)
    }

    /// One clause of a comprehension: iterate `clause.iter`, bind `clause.var`,
    /// apply the guard, then either recurse into the next clause or evaluate
    /// the body and append it.
    fn compile_comp_clauses(
        &mut self,
        clauses: &[CompClause],
        body: &Expr,
        i: usize,
        acc: u8,
        floor: u16,
        span: Span,
    ) -> CompileResult<()> {
        let clause = &clauses[i];
        let seq = self.expr(&clause.iter)?;
        let seq_slot = self.declare_hidden(span)? as u8;
        self.move_reg(seq_slot, seq, span);
        self.cur_mut().resolver.free_to(floor);

        self.cur_mut().resolver.enter_scope();
        let iter_slot = self.declare_hidden(span)? as u8;
        let _cursor_slot = self.declare_hidden(span)? as u8;
        self.emit(Opcode::IterInit, iter_slot, seq_slot, 0, span);

        let head = self.here();
        let done = self.declare_hidden(span)? as u8;
        self.emit(Opcode::IterDone, iter_slot, done, 0, span);
        let exit = self.emit_jump(Opcode::JumpIfTrue, done, span);
        // A comprehension binds one plain name per clause; `CompClause.var` is
        // a `String`, so there is no destructuring form to reuse here.
        let var_slot = self.declare_named(&clause.var, false, span)?;
        self.emit(Opcode::IterNext, iter_slot, var_slot as u8, 0, span);

        if let Some(guard) = &clause.guard {
            let cond = self.expr(guard)?;
            let skip = self.emit_jump(Opcode::JumpIfFalse, cond, span);
            self.patch(skip, head)?;
            self.cur_mut().resolver.free_to(floor);
        }

        if i + 1 == clauses.len() {
            let elem = self.expr(body)?;
            self.emit(Opcode::ListAppend, acc, elem, 0, span);
            self.cur_mut().resolver.free_to(floor);
        } else {
            self.compile_comp_clauses(clauses, body, i + 1, acc, floor, span)?;
        }

        let back = self.emit_jump(Opcode::Jump, 0, span);
        self.patch(back, head)?;
        let end = self.here();
        self.patch(exit, end)?;
        self.cur_mut().resolver.exit_scope();
        self.cur_mut().resolver.free_to(floor);
        Ok(())
    }

    /// `try { body } catch [e] { handler } [finally { … }]`.
    ///
    /// The VM keeps a per-frame handler stack, so the body only needs a
    /// `TryEnter` carrying the error slot and the (jumped-to) catch block, and
    /// a `TryExit` on the normal path. An error anywhere in the body unwinds to
    /// the innermost open handler, which resumes at `catch_pc` with the error
    /// value already stored in the handler's slot.
    ///
    /// An untyped clause matches every runtime error, exactly like the oracle.
    /// Typed clauses need `VmError::kind` matching at the handler, which this
    /// encoding does not carry, so they stay an explicit compile error.
    fn compile_try(
        &mut self,
        body: &Block,
        catches: &[CatchClause],
        finally: Option<&Block>,
        span: Span,
    ) -> CompileResult<()> {
        if catches.is_empty() && finally.is_none() {
            return Err(self.unsupported(span, "try without catch or finally"));
        }

        let enter = self.emit_jump(Opcode::TryEnter, 0, span);
        self.free_all_temps();
        self.try_depth += 1;
        let body_result = self.compile_block(body);
        self.try_depth -= 1;
        body_result?;
        self.emit(Opcode::TryExit, 0, 0, 0, span);
        let over = self.emit_jump(Opcode::Jump, 0, span);
        self.free_all_temps();

        // One handler entry dispatches through the clauses in order. A clause
        // whose type does not match falls through to the next clause instead of
        // re-raising, so clause order behaves like the oracle's clause search;
        // an error no clause wants reaches `unhandled`.
        let err_slot = self.declare_hidden(span)? as u8;
        self.patch_try_enter(enter, err_slot, span)?;
        let dispatch = self.here();
        self.patch(enter, dispatch)?;

        let mut pending_mismatch: Vec<usize> = Vec::new();
        let mut body_exit: Vec<usize> = Vec::new();
        for c in catches {
            if let Some(typ) = &c.typ {
                // `catch TypeError { }` handles only that kind of error.
                let kconst = self.add_const(Value::string(typ.clone()), c.span)?;
                let eq = self.alloc_temp(c.span)?;
                let kreg = self.alloc_temp(c.span)?;
                self.emit(Opcode::LoadConst, kreg, kconst, 0, c.span);
                let got = self.alloc_temp(c.span)?;
                self.emit(Opcode::ErrorKind, got, 0, 0, c.span);
                self.emit(Opcode::Eq, eq, got, kreg, c.span);
                self.free_above(eq);
                pending_mismatch.push(self.emit_jump(Opcode::JumpIfFalse, eq, c.span));
                let matched = self.emit_jump(Opcode::Jump, 0, c.span);
                let body_pc = self.here();
                self.patch(matched, body_pc)?;
            }
            self.cur_mut().resolver.enter_scope();
            if let Some(name) = &c.binding {
                let slot = self.declare_named(name, false, c.span)?;
                self.move_reg(slot as u8, err_slot, c.span);
            }
            self.compile_block(&c.body)?;
            self.cur_mut().resolver.exit_scope();
            body_exit.push(self.emit_jump(Opcode::Jump, 0, c.span));
            self.free_all_temps();

            let next_clause = self.here();
            for j in pending_mismatch.drain(..) {
                self.patch(j, next_clause)?;
            }
        }

        // Nothing wanted this error: run `finally` if there is one, then let it
        // keep propagating to the enclosing `try`.
        let unhandled = self.here();
        for j in pending_mismatch.drain(..) {
            self.patch(j, unhandled)?;
        }
        if let Some(f) = finally {
            self.compile_block(f)?;
        }
        self.emit(Opcode::Rethrow, 0, 0, 0, span);

        let end = self.here();
        self.patch(over, end)?;
        for j in body_exit.drain(..) {
            self.patch(j, end)?;
        }
        self.free_all_temps();
        if let Some(f) = finally {
            // `finally` also runs when the body or a clause completed normally,
            // so the block is emitted once per exit path.
            self.compile_block(f)?;
            self.free_all_temps();
        }
        Ok(())
    }

    /// Re-emit a pending `TryEnter` with its error slot in `a`, keeping the
    /// still-unpatched jump offset in `(b, c)` intact.
    fn patch_try_enter(&mut self, pc: usize, err_slot: u8, span: Span) -> CompileResult<()> {
        let res = self.cur_mut().chunk.set_operand_a(pc, err_slot);
        match res {
            Ok(()) => Ok(()),
            Err(e) => Err(self.chunk_err(e, span)),
        }
    }

    fn compile_break(&mut self, span: Span) -> CompileResult<()> {
        if !self.cur().resolver.in_loop() {
            return Err(CompileError::BreakOutsideLoop { span });
        }
        self.emit_try_exits(span);
        let pc = self.emit_jump(Opcode::Jump, 0, span);
        if let Some(lp) = self.cur_mut().resolver.loops.last_mut() {
            lp.breaks.push(pc);
        }
        Ok(())
    }

    fn compile_continue(&mut self, span: Span) -> CompileResult<()> {
        let head = match self.cur().resolver.loops.last() {
            Some(lp) => lp.head,
            None => return Err(CompileError::ContinueOutsideLoop { span }),
        };
        self.emit_try_exits(span);
        let pc = self.emit_jump(Opcode::Jump, 0, span);
        self.patch(pc, head)?;
        Ok(())
    }

    /// Retire the handlers of every `try` the jump is leaving. `return` needs
    /// nothing here: popping the frame discards its handler stack.
    fn emit_try_exits(&mut self, span: Span) {
        let base = self.loop_try_bases.last().copied().unwrap_or(0);
        for _ in base..self.try_depth {
            self.emit(Opcode::TryExit, 0, 0, 0, span);
        }
    }

    /// Evaluate `e` and leave its value in `slot`, then release its temporaries.
    fn eval_into(&mut self, e: &Expr, slot: u8, span: Span) -> CompileResult<()> {
        let r = self.expr(e)?;
        self.move_reg(slot, r, span);
        self.free_above(slot);
        Ok(())
    }

    fn compile_block(&mut self, block: &Block) -> CompileResult<()> {
        self.cur_mut().resolver.enter_scope();
        for s in &block.stmts {
            self.compile_stmt(s)?;
            self.free_all_temps();
        }
        self.cur_mut().resolver.exit_scope();
        Ok(())
    }

    // ── assignments ──────────────────────────────────────────────────────
    fn compile_assign(
        &mut self,
        target: &Expr,
        op: &AssignOp,
        value: &Expr,
        span: Span,
    ) -> CompileResult<()> {
        let binop = match op {
            AssignOp::Assign => None,
            AssignOp::AddAssign => Some(BinaryOp::Add),
            AssignOp::SubAssign => Some(BinaryOp::Sub),
            AssignOp::MulAssign => Some(BinaryOp::Mul),
            AssignOp::DivAssign => Some(BinaryOp::Div),
            AssignOp::ModAssign => Some(BinaryOp::Mod),
            AssignOp::BitAndAssign => Some(BinaryOp::BitAnd),
            AssignOp::BitOrAssign => Some(BinaryOp::BitOr),
            AssignOp::BitXorAssign => Some(BinaryOp::BitXor),
            AssignOp::ShlAssign => Some(BinaryOp::Shl),
            AssignOp::ShrAssign => Some(BinaryOp::Shr),
            AssignOp::Inc | AssignOp::Dec => None,
            AssignOp::NullCoalesceAssign => None,
        };
        if matches!(op, AssignOp::NullCoalesceAssign) {
            return self.assign_null_coalesce(target, value, span);
        }
        if matches!(op, AssignOp::Inc | AssignOp::Dec) {
            let is_inc = matches!(op, AssignOp::Inc);
            return self.assign_incdec(target, is_inc, span);
        }
        match binop {
            None => self.assign_plain(target, value, span),
            Some(b) => self.assign_compound(target, b, value, span),
        }
    }

    /// `x++` / `x--`: load, add/sub one, store.
    fn assign_incdec(&mut self, target: &Expr, is_inc: bool, span: Span) -> CompileResult<()> {
        let op = if is_inc { BinaryOp::Add } else { BinaryOp::Sub };
        match target {
            Expr::Ident { name, .. } => {
                let current = self.load_ident(name, span)?;
                let one = self.load_const(Value::int(1), span)?;
                let dst = self.alloc_temp(span)?;
                self.emit_binop(op, dst, current, one, span)?;
                self.free_above(dst);
                self.store_ident(name, dst, span)
            }
            other => Err(self.unsupported(self.expr_span(other), "++/-- target")),
        }
    }

    fn assign_plain(&mut self, target: &Expr, value: &Expr, span: Span) -> CompileResult<()> {
        match target {
            Expr::Ident { name, .. } => {
                let r = self.expr(value)?;
                self.store_ident(name, r, span)
            }
            Expr::Index { object, index, .. } => {
                let o = self.expr(object)?;
                let k = self.expr(index)?;
                let v = self.expr(value)?;
                self.emit(Opcode::SetIndex, o, k, v, span);
                Ok(())
            }
            Expr::Field { object, field, .. } => {
                let o = self.expr(object)?;
                let v = self.expr(value)?;
                let k = self.name_const(field, span)?;
                self.emit(Opcode::SetField, o, k, v, span);
                Ok(())
            }
            other => Err(self.unsupported(self.expr_span(other), "assignment target")),
        }
    }

    fn assign_compound(
        &mut self,
        target: &Expr,
        op: BinaryOp,
        value: &Expr,
        span: Span,
    ) -> CompileResult<()> {
        match target {
            Expr::Ident { name, .. } => {
                let current = self.load_ident(name, span)?;
                let r = self.expr(value)?;
                let dst = self.alloc_temp(span)?;
                self.emit_binop(op, dst, current, r, span)?;
                self.free_above(dst);
                self.store_ident(name, dst, span)
            }
            Expr::Index { object, index, .. } => {
                let o = self.expr(object)?;
                let k = self.expr(index)?;
                let current = self.alloc_temp(span)?;
                self.emit(Opcode::GetIndex, current, o, k, span);
                let r = self.expr(value)?;
                let dst = self.alloc_temp(span)?;
                self.emit_binop(op, dst, current, r, span)?;
                self.emit(Opcode::SetIndex, o, k, dst, span);
                Ok(())
            }
            Expr::Field { object, field, .. } => {
                let o = self.expr(object)?;
                let k = self.name_const(field, span)?;
                let current = self.alloc_temp(span)?;
                self.emit(Opcode::GetField, current, o, k, span);
                let r = self.expr(value)?;
                let dst = self.alloc_temp(span)?;
                self.emit_binop(op, dst, current, r, span)?;
                self.emit(Opcode::SetField, o, k, dst, span);
                Ok(())
            }
            other => Err(self.unsupported(self.expr_span(other), "compound assignment target")),
        }
    }

    fn assign_null_coalesce(
        &mut self,
        target: &Expr,
        value: &Expr,
        span: Span,
    ) -> CompileResult<()> {
        let Expr::Ident { name, .. } = target else {
            return Err(self.unsupported(self.expr_span(target), "??= on a non-name target"));
        };
        let current = self.load_ident(name, span)?;
        let nil = self.alloc_temp(span)?;
        self.emit(Opcode::LoadNil, nil, 0, 0, span);
        let is_nil = self.alloc_temp(span)?;
        self.emit(Opcode::Eq, is_nil, current, nil, span);
        let jf = self.emit_jump(Opcode::JumpIfFalse, is_nil, span);
        self.free_all_temps();
        let r = self.expr(value)?;
        self.store_ident(name, r, span)?;
        self.free_all_temps();
        let end = self.here();
        self.patch(jf, end)
    }

    /// Load `name` for a read-modify-write. Unlike [`Compiler::resolve_ident`],
    /// this rejects assigning to a `const` (local or module global).
    fn local_slot(&mut self, name: &str, span: Span) -> CompileResult<u8> {
        if let Some(b) = self.cur().resolver.resolve_local(name) {
            if b.is_const {
                return Err(CompileError::AssignToConst {
                    span,
                    name: name.to_string(),
                });
            }
            return Ok(b.slot as u8);
        }
        if self.funcs.len() == 1 {
            if let Some((slot, is_const)) = self.module_global_map.get(name).copied() {
                if is_const {
                    return Err(CompileError::AssignToConst {
                        span,
                        name: name.to_string(),
                    });
                }
                let dst = self.alloc_temp(span)?;
                self.emit(Opcode::GetGlobal, dst, slot as u8, 0, span);
                return Ok(dst);
            }
        }
        if let Some(idx) = self.cur().resolver.resolve_upvalue(name) {
            let dst = self.alloc_temp(span)?;
            self.emit(Opcode::GetUpval, dst, idx as u8, 0, span);
            return Ok(dst);
        }
        // Walk up parent frames to find captured variables.
        match self.resolve_ident(name, span)? {
            IdentLoc::Upvalue(idx) => {
                let dst = self.alloc_temp(span)?;
                self.emit(Opcode::GetUpval, dst, idx as u8, 0, span);
                Ok(dst)
            }
            IdentLoc::Local(slot) => Ok(slot),
            IdentLoc::Global(slot) => {
                let dst = self.alloc_temp(span)?;
                self.emit(Opcode::GetGlobal, dst, slot as u8, 0, span);
                Ok(dst)
            }
            IdentLoc::StarGlobal(slot) => {
                let dict = self.alloc_temp(span)?;
                self.emit(Opcode::GetGlobal, dict, slot as u8, 0, span);
                let key = self.name_const(name, span)?;
                let dst = self.alloc_temp(span)?;
                self.emit(Opcode::GetField, dst, dict, key, span);
                Ok(dst)
            }
        }
    }

    fn load_ident(&mut self, name: &str, span: Span) -> CompileResult<u8> {
        self.local_slot(name, span)
    }

    fn store_ident(&mut self, name: &str, reg: u8, span: Span) -> CompileResult<()> {
        if let Some(b) = self.cur().resolver.resolve_local(name) {
            if b.is_const {
                return Err(CompileError::AssignToConst {
                    span,
                    name: name.to_string(),
                });
            }
            self.move_reg(b.slot as u8, reg, span);
            return Ok(());
        }
        if self.funcs.len() == 1 {
            let slot = match self.module_global_map.get(name).copied() {
                Some((_, true)) => {
                    return Err(CompileError::AssignToConst {
                        span,
                        name: name.to_string(),
                    });
                }
                Some((slot, false)) => slot,
                // A top-level assignment to an undeclared name implicitly
                // declares a module global (mirrors `ZEN_PURE = true` in
                // `std/*.z`).
                None => self.declare_global(name, false, span)?,
            };
            self.emit(Opcode::SetGlobal, slot as u8, reg, 0, span);
            return Ok(());
        }
        if let Some(idx) = self.cur().resolver.resolve_upvalue(name) {
            self.emit(Opcode::SetUpval, idx as u8, reg, 0, span);
            return Ok(());
        }
        match self.resolve_ident(name, span)? {
            IdentLoc::Upvalue(idx) => {
                self.emit(Opcode::SetUpval, idx as u8, reg, 0, span);
                Ok(())
            }
            IdentLoc::Local(slot) => {
                self.move_reg(slot, reg, span);
                Ok(())
            }
            IdentLoc::Global(slot) => {
                self.emit(Opcode::SetGlobal, slot as u8, reg, 0, span);
                Ok(())
            }
            IdentLoc::StarGlobal(_) => {
                Err(self.unsupported(span, "assignment to a name from `import *`"))
            }
        }
    }

    fn name_const(&mut self, name: &str, span: Span) -> CompileResult<u8> {
        let k = self.add_const(Value::string(name.to_string()), span)?;
        Ok(k)
    }

    fn load_const(&mut self, v: Value, span: Span) -> CompileResult<u8> {
        let dst = self.alloc_temp(span)?;
        let k = self.add_const(v, span)?;
        self.emit(Opcode::LoadConst, dst, k, 0, span);
        Ok(dst)
    }

    fn unsupported(&self, span: Span, feature: &str) -> CompileError {
        CompileError::Unsupported {
            span,
            what: feature.to_string(),
        }
    }

    fn compile_items(&mut self, items: &[Stmt]) -> CompileResult<()> {
        // Hoist module-scope function bindings before compiling any body so a
        // function can forward-reference a `let f = fn() { … }` that is declared
        // later in the module — needed for mutual recursion between top-level
        // helper definitions (e.g. `std/json.z`'s `_object` ↔ `_value`). Only
        // function-valued bindings are hoisted, matching `compile_binding`'s
        // existing self-recursion rule; value bindings keep their ordinary
        // value-then-name order (`let x = x` still reads the outer `x`).
        self.predeclare_module_bindings(items);
        let last = items.len().saturating_sub(1);
        for (i, item) in items.iter().enumerate() {
            // A trailing bare expression yields the module's value (shown by
            // `zen run` / `-e` when non-nil), mirroring a function body's
            // implicit return of its last expression.
            if i == last {
                if let Stmt::Expr(e) = item {
                    let r = self.expr(e)?;
                    let span = self.expr_span(e);
                    self.emit(Opcode::Return, r, 0, 0, span);
                    break;
                }
            }
            self.compile_stmt(item)?;
        }
        Ok(())
    }

    /// Reserve a module-global slot for every top-level function binding, in
    /// declaration order, without emitting anything. See `compile_items`.
    fn predeclare_module_bindings(&mut self, items: &[Stmt]) {
        let is_fn_value = |e: &Expr| {
            matches!(
                e,
                Expr::Lambda { .. } | Expr::FnExpr { .. } | Expr::ArrowFn { .. }
            )
        };
        for item in items {
            let (name, span) = match item {
                Stmt::Let {
                    name, value, span, ..
                }
                | Stmt::Shared {
                    name, value, span, ..
                } if is_fn_value(value) => (name.as_str(), *span),
                Stmt::Const { name, value, span } if is_fn_value(value) => (name.as_str(), *span),
                Stmt::Function { name, span, .. } => (name.as_str(), *span),
                _ => continue,
            };
            let _ = self.declare_global(name, false, span);
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────────

// ── helpers ──────────────────────────────────────────────────────────────
fn placeholder_proto() -> FunctionProto {
    FunctionProto {
        name: String::new(),
        params: Vec::new(),
        module_id: 0,
        num_globals: 0,
        num_locals: 0,
        max_reg: 0,
        num_required: 0,
        num_upvalues: 0,
        code: Vec::new(),
        constants: Vec::new(),
        line_table: Vec::new(),
        upvalue_descriptors: Vec::new(),
        ic_slots: std::cell::RefCell::new(Vec::new()),
        jit: std::sync::OnceLock::new(),
        is_method: false,
    }
}

/// Where a match arm body leaves its value: `Stmt::Match` returns from the
/// function, `Expr::Match` stores into the result register.
#[derive(Clone, Copy)]
enum ArmValue {
    Return,
    Register(u8),
}

#[derive(Debug, Clone, Copy)]
enum IdentLoc {
    Local(u8),
    Upvalue(u16),
    /// A module-scope binding, addressed in the module frame's register file
    /// (base 0) rather than captured by value.
    Global(u16),
    /// An unresolved bare name reached through a module-scope
    /// `from <mod> import *`: read as a field of the star dict in this global
    /// slot (see `Compiler::star_globals`).
    StarGlobal(u16),
}

/// Where a name was found while walking enclosing frames.
enum ParentRes {
    /// Found in a non-module ancestor; capture it as an upvalue.
    Desc(UpvalueDesc),
    /// Found in the module frame; access it as a shared global.
    Global(u16),
    NotFound,
}

enum FnBody<'a> {
    /// A block body. Falling off the end yields a trailing expression
    /// statement's value, so `func` declarations, methods, `fn` expressions and
    /// arrow blocks all behave alike. The oracle's `Body::Block` carries an
    /// `auto_return` flag; every block form sets it.
    Block(&'a Block),
    Expr(&'a Expr),
}
