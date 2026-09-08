use crate::runtime::{DictEntry, Expr, Kind, LetTarget, Stmt, StmtKind, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// Bytecode instruction opcodes.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Opcode {
    Pop,
    Dup,

    // Constants
    Const,
    True,
    False,
    Null,

    // Locals (indexed by u16 slot)
    LoadLocal,
    StoreLocal,

    // Globals (string name from constants pool)
    LoadGlobal,
    StoreGlobal,

    // const/locked tracking
    CheckLockedAssign,
    CheckLockedRedefine,
    LockGlobal,
    UnlockGlobal,

    // Arithmetic (pop b, pop a, push binary(a, op, b))
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Neg,

    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Boolean
    Not,

    // Compound assignment to a global (arg1 = name const; pops RHS, reads
    // current value, applies op, writes back). Unlike the generic arithmetic
    // opcodes these leave no result on the stack.
    AddGlobal,
    SubGlobal,
    MulGlobal,
    DivGlobal,
    ModGlobal,

    // Compound assignment to a local slot (arg1 = slot index; pops RHS, reads
    // the slot, applies op, writes back). No locked check or name resolution.
    AddLocal,
    SubLocal,
    MulLocal,
    DivLocal,
    ModLocal,

    // Control flow (absolute instruction index target in arg1)
    Jmp,
    JmpIfFalse,
    // Superinstructions for counting loops (locals only):
    JmpIfTrue,
    // locals[a] <  locals[b] ? fall through : jump arg1
    JmpLtLocal,
    // locals[a] <= locals[b] ? fall through : jump arg1
    JmpLeLocal,
    // locals[a] <  constants[b] ? fall through : jump arg1
    JmpLtLocalConst,
    // locals[a] <= constants[b] ? fall through : jump arg1
    JmpLeLocalConst,
    // locals[a] >  constants[b] ? fall through : jump arg1
    JmpGtLocalConst,

    // Global (top-level) siblings of the fused compare-jumps: arg1 = patched
    // jump target, arg2 = name-constant index of the global, arg3 = numeric
    // constant index. Taken when the condition HOLDS.
    JmpLtGlobalConst,
    JmpLeGlobalConst,
    JmpGtGlobalConst,
    JmpGeGlobalConst,

    // Global += / -= numeric immediate: arg1 = name-constant index,
    // arg2 = numeric constant index.
    AddGlobalImm,
    SubGlobalImm,
    // locals[a] >= constants[b] ? fall through : jump arg1
    JmpGeLocalConst,
    // locals[arg1] += constants[arg2] (Number fast path, generic fallback)
    AddLocalImm,
    SubLocalImm,
    JmpIfNotNull,

    // Functions
    Call,
    // Method call on a stack target (arg1 = method-name const, arg2 = argc;
    // pops target then args, pushes result). Enables obj.method(...) in
    // compiled code.
    CallMethod,
    // Statement-position list mutation on a variable slot so the caller sees
    // the change: arg1 = local slot. PushSlot pops the element to append;
    // PopSlot pushes the removed element back onto the stack.
    PushSlot,
    PopSlot,
    // Same, for globals (arg1 = name const).
    PushGlobal,
    PopGlobal,
    Return,

    // Print (arg1=count, arg2=sep const idx|u16::MAX, arg3=end const idx|u16::MAX)
    Print,

    // Lists/Dicts
    BuildList,
    BuildDict,
    Index,
    GetMember,

    // Type check / length
    Typeof,
    Len,

    // Function definition (module-level) / closure
    DefineFunction,
    Closure,

    // Imports (packed name lists in the constants pool, '\u{1f}'-separated;
    // items are "name" or "name=alias").
    Import,
    ImportFrom,
    ImportStar,

    // Pops end, start (Numbers); pushes the materialized range list.
    // arg1 != 0 => exclusive (..) instead of inclusive (..=).
    MakeRange,

    // General call: stack holds [callee, arg1..argN]; arg1 = N. The callee
    // value must be a Function or NativeFunction name reference.
    CallValue,
    // Statement-position list mutation through a variable-rooted member/index
    // chain (`d.l.push(x)`, `d["l"].push(x)`, `d.a.b.push(x)`). arg1 = const
    // index of the chain descriptor ("method\u{1f}root\u{1f}step..." with
    // "k:key" for member steps and "i" for index steps); arg2 = count of
    // mutator argument values already on the stack (top-of-stack). Any index
    // step values were pushed below the mutator args, in step order.
    NestedMutate,
}

/// A single bytecode instruction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instruction {
    pub opcode: Opcode,
    pub arg1: u16,
    pub arg2: u16,
    pub arg3: u16,
}

impl Instruction {
    pub fn new(opcode: Opcode, arg1: u16, arg2: u16, arg3: u16) -> Self {
        Self { opcode, arg1, arg2, arg3 }
    }
}

/// A compiled Zen function body (bytecode + constant pool + metadata).
#[derive(Clone, Debug)]
pub struct CompiledFunction {
    pub name: String,
    pub params: Vec<String>,
    pub param_count: u16,
    /// Captured variable names, in slot order (slots param_count..param_count+captured_count)
    pub captured_names: Vec<String>,
    /// Total local slot count (params + captured + temporaries)
    pub local_count: u16,
    pub instructions: Vec<Instruction>,
    pub constants: Vec<Value>,
}

struct LoopCtx {
    continue_target: Option<usize>,
    continue_fixups: Vec<usize>,
    break_fixups: Vec<usize>,
}

/// Compiler for a single function body (or the module "main" body).
struct FunctionCompiler {
    instructions: Vec<Instruction>,
    constants: Vec<Value>,
    str_consts: HashMap<String, u16>,
    num_consts: HashMap<u64, u16>,
    slots: HashMap<String, u16>,
    next_slot: u16,
    /// Number of leading args bound to slots 0..param_count (params, or
    /// params + 1 for methods where `self` occupies slot 0).
    param_count: u16,
    /// Whether nested function definitions (StmtKind::Function) and lambdas are
    /// allowed here (true only for module-level compilation).
    allow_defs: bool,
    loops: Vec<LoopCtx>,
    /// Compiled nested functions (module-level defs / closures). Index 0 in the
    /// final table is the main function, so nested entries are at idx + 1.
    nested: Vec<CompiledFunction>,
}

/// Compile a single function body. Returns an Err for any construct the
/// bytecode VM cannot handle, in which case the caller should fall back to
/// tree-walk interpretation for the whole function.
pub fn compile_function(
    name: &str,
    params: &[String],
    captured_names: &[String],
    body: &[Stmt],
) -> Result<Arc<CompiledFunction>, String> {
    let mut c = FunctionCompiler::new(params, captured_names, false);
    for stmt in body {
        c.compile_stmt(stmt)?;
    }
    Ok(Arc::new(c.finish(name, params, captured_names)))
}

/// Compile a class method body. `self` occupies local slot 0 and the
/// parameters follow at slots 1..=params.len().
#[allow(dead_code)]
pub fn compile_method(
    name: &str,
    params: &[String],
    body: &[Stmt],
) -> Result<Arc<CompiledFunction>, String> {
    let mut c = FunctionCompiler::new(&[], &[], false);
    c.slots.insert("self".to_string(), 0);
    for (i, p) in params.iter().enumerate() {
        c.slots.insert(p.clone(), (i + 1) as u16);
    }
    c.next_slot = (params.len() + 1) as u16;
    c.param_count = (params.len() + 1) as u16;
    for stmt in body {
        c.compile_stmt(stmt)?;
    }
    Ok(Arc::new(c.finish(name, params, &[])))
}

/// Compile a whole program (module). Index 0 of the returned table is the
/// "main" function; user-defined functions and closures follow. On Err, fall
/// back to tree-walk for the whole module.
pub fn compile_program(stmts: &[Stmt]) -> Result<Vec<Arc<CompiledFunction>>, String> {
    let mut c = FunctionCompiler::new(&[], &[], true);
    for stmt in stmts {
        c.compile_stmt(stmt)?;
    }
    let main = c.finish("main", &[], &[]);
    let mut out = vec![Arc::new(main)];
    for f in c.nested {
        out.push(Arc::new(f));
    }
    Ok(out)
}

impl FunctionCompiler {
    fn new(params: &[String], captured_names: &[String], allow_defs: bool) -> Self {
        let mut slots = HashMap::new();
        for (i, p) in params.iter().enumerate() {
            slots.insert(p.clone(), i as u16);
        }        let base = params.len() as u16;
        for (i, c) in captured_names.iter().enumerate() {
            slots.insert(c.clone(), base + i as u16);
        }
        FunctionCompiler {
            instructions: Vec::new(),
            constants: Vec::new(),
            str_consts: HashMap::new(),
            num_consts: HashMap::new(),
            slots,
            next_slot: base + captured_names.len() as u16,
            param_count: params.len() as u16,
            allow_defs,
            loops: Vec::new(),
            nested: Vec::new(),
        }
    }

    fn finish(&mut self, name: &str, params: &[String], captured_names: &[String]) -> CompiledFunction {
        CompiledFunction {
            name: name.to_string(),
            params: params.to_vec(),
            param_count: self.param_count,
            captured_names: captured_names.to_vec(),
            local_count: self.next_slot,
            instructions: std::mem::take(&mut self.instructions),
            constants: std::mem::take(&mut self.constants),
        }
    }

    fn emit(&mut self, opcode: Opcode, arg1: u16, arg2: u16, arg3: u16) -> usize {
        if self.instructions.len() >= u16::MAX as usize {
            return usize::MAX;
        }
        let idx = self.instructions.len();
        self.instructions.push(Instruction::new(opcode, arg1, arg2, arg3));
        idx
    }

    /// Fuse `var <op> <literal>` / `<literal> <op> var` / `var <op> var`
    /// conditions into single compare-jump opcodes. Locals use slot indices;
    /// anything not in the local slot map is treated as a global by name.
    /// Returns (opcode, operand_a, operand_b); jump target rides in arg1 via
    /// patch(), taken when the condition FAILS (same polarity as JmpIfFalse).
    fn try_fuse_cond(&mut self, c: &Expr) -> Option<(Opcode, u16, u16)> {
        let Expr::Binary(l, k @ (Kind::Lt | Kind::Le | Kind::Gt | Kind::Ge), r) = c else {
            return None;
        };
        let lit_r = match r.as_ref() {
            Expr::Value(Value::Number(m)) => Some(self.const_num(*m)),
            _ => None,
        };
        let lit_l = match l.as_ref() {
            Expr::Value(Value::Number(m)) => Some(self.const_num(*m)),
            _ => None,
        };
        let local_op = |k: &Kind| match k {
            Kind::Lt => Opcode::JmpLtLocalConst,
            Kind::Le => Opcode::JmpLeLocalConst,
            Kind::Gt => Opcode::JmpGtLocalConst,
            _ => Opcode::JmpGeLocalConst,
        };
        let global_op = |k: &Kind| match k {
            Kind::Lt => Opcode::JmpLtGlobalConst,
            Kind::Le => Opcode::JmpLeGlobalConst,
            Kind::Gt => Opcode::JmpGtGlobalConst,
            _ => Opcode::JmpGeGlobalConst,
        };
        // var vs literal
        if let (Expr::Var(an), Some(cr)) = (l.as_ref(), lit_r) {
            if let Some(&sa) = self.slots.get(an) {
                return Some((local_op(k), sa, cr));
            }
            let ni = self.const_str(an);
            return Some((global_op(k), ni, cr));
        }
        // literal vs var (rewrite literal-left to var-first comparison)
        if let (Some(cl), Expr::Var(bn)) = (lit_l, r.as_ref()) {
            if let Some(&sb) = self.slots.get(bn) {
                let fop = match *k {
                    Kind::Lt => Opcode::JmpGtLocalConst,
                    Kind::Le => Opcode::JmpGeLocalConst,
                    Kind::Gt => Opcode::JmpLtLocalConst,
                    _ => Opcode::JmpLeLocalConst,
                };
                return Some((fop, sb, cl));
            }
            let ni = self.const_str(bn);
            let fop = match *k {
                Kind::Lt => Opcode::JmpGtGlobalConst,
                Kind::Le => Opcode::JmpGeGlobalConst,
                Kind::Gt => Opcode::JmpLtGlobalConst,
                _ => Opcode::JmpLeGlobalConst,
            };
            return Some((fop, ni, cl));
        }
        None
    }

    fn patch(&mut self, idx: usize, target: usize) {
        if let Some(inst) = self.instructions.get_mut(idx) {
            inst.arg1 = target as u16;
        }
    }

    fn const_str(&mut self, s: &str) -> u16 {
        if let Some(&i) = self.str_consts.get(s) {
            return i;
        }
        let i = self.constants.len() as u16;
        self.constants.push(Value::String(s.to_string()));
        self.str_consts.insert(s.to_string(), i);
        i
    }

    fn const_num(&mut self, n: f64) -> u16 {
        let key = n.to_bits();
        if let Some(&i) = self.num_consts.get(&key) {
            return i;
        }
        let i = self.constants.len() as u16;
        self.constants.push(Value::Number(n));
        self.num_consts.insert(key, i);
        i
    }

    fn alloc_temp(&mut self) -> u16 {
        let s = self.next_slot;
        self.next_slot += 1;
        s
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        match &stmt.kind {
            StmtKind::Let(target, e, is_const) => match target {
                LetTarget::Var(n) => {
                    self.compile_expr(e)?;
                    // Inside function bodies (allow_defs == false), `var`
                    // declares a function-local: allocate a slot so this and
                    // later references stay isolated from the caller's globals.
                    if !self.allow_defs && !self.slots.contains_key(n) {
                        let slot = self.next_slot;
                        self.next_slot += 1;
                        self.slots.insert(n.clone(), slot);
                        self.emit(Opcode::StoreLocal, slot, 0, 0);
                        return Ok(());
                    }
                    if *is_const {
                        let ni = self.const_str(n);
                        self.emit(Opcode::CheckLockedRedefine, ni, 0, 0);
                    }
                    let ni = self.const_str(n);
                    self.emit(Opcode::StoreGlobal, ni, 0, 0);
                    if *is_const {
                        self.emit(Opcode::LockGlobal, ni, 0, 0);
                    } else {
                        self.emit(Opcode::UnlockGlobal, ni, 0, 0);
                    }
                    Ok(())
                }
                _ => Err("destructuring in let is not supported by the bytecode compiler".into()),
            },
            StmtKind::Assign(n, op, e) => {
                if let Some(&slot) = self.slots.get(n) {
                    match op {
                        Kind::Assign => {
                            // Fusion: i = i + <num> / i = i - <num>
                            if let Expr::Binary(l, k2, r) = e {
                                let imm = match (k2, l.as_ref(), r.as_ref()) {
                                    (Kind::Plus, Expr::Var(ln), Expr::Value(Value::Number(m))) if ln == n => Some(*m ),
                                    // Store the positive magnitude; the
                                    // SubLocalImm handler negates it once.
                                    (Kind::Minus, Expr::Var(ln), Expr::Value(Value::Number(m))) if ln == n => Some(*m ),
                                    _ => None,
                                };
                                if let Some(m) = imm {
                                    let ci = self.const_num(m);
                                    self.emit(
                                        if matches!(k2, Kind::Plus) { Opcode::AddLocalImm } else { Opcode::SubLocalImm },
                                        slot, ci, 0,
                                    );
                                    return Ok(());
                                }
                            }
                            self.compile_expr(e)?;
                            self.emit(Opcode::StoreLocal, slot, 0, 0);
                        }
                        Kind::PlusAssign => {
                            // Fusion: i += <num>
                            if let Expr::Value(Value::Number(m)) = e {
                                let ci = self.const_num(*m);
                                self.emit(Opcode::AddLocalImm, slot, ci, 0);
                                return Ok(());
                            }
                            self.compile_expr(e)?;
                            self.emit(self.local_opcode(op)?, slot, 0, 0);
                        }
                        Kind::MinusAssign => {
                            // Fusion: i -= <num>
                            if let Expr::Value(Value::Number(m)) = e {
                                let ci = self.const_num(*m);
                                self.emit(Opcode::SubLocalImm, slot, ci, 0);
                                return Ok(());
                            }
                            self.compile_expr(e)?;
                            self.emit(self.local_opcode(op)?, slot, 0, 0);
                        }
                        Kind::StarAssign
                        | Kind::SlashAssign
                        | Kind::PercentAssign => {
                            self.compile_expr(e)?;
                            self.emit(self.local_opcode(op)?, slot, 0, 0);
                        }
                        Kind::NullishAssign => {
                            self.compile_expr(e)?;
                            self.emit(Opcode::LoadLocal, slot, 0, 0);
                            self.emit(Opcode::Dup, 0, 0, 0);
                            let jn = self.emit(Opcode::JmpIfNotNull, 0, 0, 0);
                            self.emit(Opcode::Pop, 0, 0, 0);
                            self.patch(jn, self.instructions.len());
                            self.emit(Opcode::StoreLocal, slot, 0, 0);
                        }
                        _ => return Err("unsupported assignment operator in bytecode".into()),
                    }
                    return Ok(());
                }
                let ni = self.const_str(n);
                match op {
                    Kind::Assign => {
                        // Fusion: i = i + <num> / i = i - <num> for globals
                        if let Expr::Binary(l, k2, r) = e {
                            let imm = match (k2, l.as_ref(), r.as_ref()) {
                                (Kind::Plus, Expr::Var(ln), Expr::Value(Value::Number(m))) if ln == n => Some(*m),
                                (Kind::Minus, Expr::Var(ln), Expr::Value(Value::Number(m))) if ln == n => Some(*m),
                                _ => None,
                            };
                            if let Some(m) = imm {
                                let ci = self.const_num(m);
                                self.emit(
                                    if matches!(k2, Kind::Plus) { Opcode::AddGlobalImm } else { Opcode::SubGlobalImm },
                                    ni, ci, 0,
                                );
                                return Ok(());
                            }
                        }
                        self.compile_expr(e)?;
                        self.emit(Opcode::CheckLockedAssign, ni, 0, 0);
                        self.emit(Opcode::StoreGlobal, ni, 0, 0);
                        Ok(())
                    }
                    Kind::PlusAssign => {
                        if let Expr::Value(Value::Number(m)) = e {
                            let ci = self.const_num(*m);
                            self.emit(Opcode::AddGlobalImm, ni, ci, 0);
                            return Ok(());
                        }
                        self.compile_expr(e)?;
                        self.emit(self.binary_opcode(op)?, ni, 0, 0);
                        Ok(())
                    }
                    Kind::MinusAssign => {
                        if let Expr::Value(Value::Number(m)) = e {
                            let ci = self.const_num(*m);
                            self.emit(Opcode::SubGlobalImm, ni, ci, 0);
                            return Ok(());
                        }
                        self.compile_expr(e)?;
                        self.emit(self.binary_opcode(op)?, ni, 0, 0);
                        Ok(())
                    }
                    Kind::StarAssign
                    | Kind::SlashAssign
                    | Kind::PercentAssign => {
                        self.compile_expr(e)?;
                        self.emit(self.binary_opcode(op)?, ni, 0, 0);
                        Ok(())
                    }
                    Kind::NullishAssign => {
                        self.compile_expr(e)?;
                        let temp = self.alloc_temp();
                        self.emit(Opcode::StoreLocal, temp, 0, 0);
                        self.emit(Opcode::LoadGlobal, ni, 0, 0);
                        self.emit(Opcode::Dup, 0, 0, 0);
                        let jn = self.emit(Opcode::JmpIfNotNull, 0, 0, 0);
                        self.emit(Opcode::Pop, 0, 0, 0);
                        self.emit(Opcode::LoadLocal, temp, 0, 0);
                        self.patch(jn, self.instructions.len());
                        self.emit(Opcode::StoreGlobal, ni, 0, 0);
                        Ok(())
                    }
                    _ => Err("unsupported assignment operator in bytecode".into()),
                }
            }
            StmtKind::Print(values, sep, end) => {
                for v in values {
                    self.compile_expr(v)?;
                }
                let sep_idx = sep
                    .as_deref()
                    .map(|s| self.const_str(s))
                    .unwrap_or(u16::MAX);
                let end_idx = end
                    .as_deref()
                    .map(|s| self.const_str(s))
                    .unwrap_or(u16::MAX);
                self.emit(Opcode::Print, values.len() as u16, sep_idx, end_idx);
                Ok(())
            }
            StmtKind::If(c, yes, no) => {
                let jf = if let Some((fop, sa, sb)) = self.try_fuse_cond(c) {
                    self.emit(fop, 0, sa, sb)
                } else {
                    self.compile_expr(c)?;
                    self.emit(Opcode::JmpIfFalse, 0, 0, 0)
                };
                for s in yes {
                    self.compile_stmt(s)?;
                }
                let je = self.emit(Opcode::Jmp, 0, 0, 0);
                self.patch(jf, self.instructions.len());
                for s in no {
                    self.compile_stmt(s)?;
                }
                self.patch(je, self.instructions.len());
                Ok(())
            }
            StmtKind::While(c, body) => {
                // Superinstruction: fuse `while a < b` / `while a <= b` on two
                // locals into a single compare-and-branch opcode.
                // `a > b` / `a >= b` are normalized to the swapped
                // Lt/Le forms so all four comparisons reuse the two
                // fused compare-jump opcodes.
                let cond = self.instructions.len();
                let jf = if let Some((fop, sa, sb)) = self.try_fuse_cond(c) {
                    self.emit(fop, 0, sa, sb)
                } else {
                    self.compile_expr(c)?;
                    self.emit(Opcode::JmpIfFalse, 0, 0, 0)
                };
                self.loops.push(LoopCtx {
                    continue_target: Some(cond),
                    continue_fixups: Vec::new(),
                    break_fixups: Vec::new(),
                });
                for s in body {
                    self.compile_stmt(s)?;
                }
                self.emit(Opcode::Jmp, cond as u16, 0, 0);
                let ctx = self.loops.pop().expect("loop ctx");
                let end = self.instructions.len();
                self.patch(jf, end);
                for b in ctx.break_fixups {
                    self.patch(b, end);
                }
                Ok(())
            }
            StmtKind::For(names, e, body) => {
                if names.len() != 1 {
                    return Err("multi-variable for is not supported by the bytecode compiler"
                        .into());
                }
                let n = &names[0];
                self.compile_expr(e)?;
                let list_t = self.alloc_temp();
                self.emit(Opcode::StoreLocal, list_t, 0, 0);
                let idx_t = self.alloc_temp();
                let zero = self.const_num(0.0);
                self.emit(Opcode::Const, zero, 0, 0);
                self.emit(Opcode::StoreLocal, idx_t, 0, 0);
                let start = self.instructions.len();
                self.emit(Opcode::LoadLocal, idx_t, 0, 0);
                self.emit(Opcode::LoadLocal, list_t, 0, 0);
                self.emit(Opcode::Len, 0, 0, 0);
                self.emit(Opcode::Lt, 0, 0, 0);
                let jf = self.emit(Opcode::JmpIfFalse, 0, 0, 0);
                self.emit(Opcode::LoadLocal, list_t, 0, 0);
                self.emit(Opcode::LoadLocal, idx_t, 0, 0);
                self.emit(Opcode::Index, 0, 0, 0);
                let name_idx = self.const_str(n);
                self.emit(Opcode::StoreGlobal, name_idx, 0, 0);
                self.loops.push(LoopCtx {
                    continue_target: None,
                    continue_fixups: Vec::new(),
                    break_fixups: Vec::new(),
                });
                for s in body {
                    self.compile_stmt(s)?;
                }
                let ctx = self.loops.pop().expect("loop ctx");
                let inc = self.instructions.len();
                for c in ctx.continue_fixups {
                    self.patch(c, inc);
                }
                let one = self.const_num(1.0);
                self.emit(Opcode::AddLocalImm, idx_t, one, 0);
                self.emit(Opcode::Jmp, start as u16, 0, 0);
                let end = self.instructions.len();
                self.patch(jf, end);
                for b in ctx.break_fixups {
                    self.patch(b, end);
                }
                Ok(())
            }
            StmtKind::Break => {
                let i = self.emit(Opcode::Jmp, 0, 0, 0);
                self.loops
                    .last_mut()
                    .ok_or("break used outside a loop")?
                    .break_fixups
                    .push(i);
                Ok(())
            }
            StmtKind::Continue => {
                if let Some(continue_target) = self.loops.last().and_then(|l| l.continue_target) {
                    self.emit(Opcode::Jmp, continue_target as u16, 0, 0);
                } else {
                    let i = self.emit(Opcode::Jmp, 0, 0, 0);
                    self.loops
                        .last_mut()
                        .ok_or("continue used outside a loop")?
                        .continue_fixups
                        .push(i);
                }
                Ok(())
            }
            StmtKind::Return(Some(e)) => {
                self.compile_expr(e)?;
                self.emit(Opcode::Return, 0, 0, 0);
                Ok(())
            }
            StmtKind::Return(None) => {
                self.emit(Opcode::Null, 0, 0, 0);
                self.emit(Opcode::Return, 0, 0, 0);
                Ok(())
            }
            StmtKind::Expr(e) => {
                // Statement-position list mutation on a variable-rooted chain
                // must write back through the chain. Prefer the general
                // member/index chain path (d.l.push(x), d["l"].push(x),
                // d.a.b.push(x)), then fall back to the plain-variable fast
                // path (l.push(x)) which uses dedicated slot opcodes.
                if let Expr::Call(callee, args) = e {
                    if let Expr::Member(obj, method) = callee.as_ref() {
                        let m = method.as_str();
                        if matches!(m, "push" | "append" | "pop" | "splice" | "insert" | "shift" | "unshift") {
                            match obj.as_ref() {
                                Expr::Var(name) => {
                                    if matches!(m, "push" | "append" | "pop") {
                                        if let Some(&slot) = self.slots.get(name) {
                                            match m {
                                                "push" | "append" => {
                                                    if args.len() != 1 {
                                                        return Err("push expects exactly one argument".into());
                                                    }
                                                    self.compile_expr(&args[0])?;
                                                    self.emit(Opcode::PushSlot, slot, 0, 0);
                                                }
                                                _ => {
                                                    if !args.is_empty() {
                                                        return Err("pop expects no arguments".into());
                                                    }
                                                    self.emit(Opcode::PopSlot, slot, 0, 0);
                                                    self.emit(Opcode::Pop, 0, 0, 0);
                                                }
                                            }
                                            return Ok(());
                                        } else {
                                            let ci = self.const_str(name);
                                            match m {
                                                "push" | "append" => {
                                                    if args.len() != 1 {
                                                        return Err("push expects exactly one argument".into());
                                                    }
                                                    self.compile_expr(&args[0])?;
                                                    self.emit(Opcode::PushGlobal, ci, 0, 0);
                                                }
                                                _ => {
                                                    if !args.is_empty() {
                                                        return Err("pop expects no arguments".into());
                                                    }
                                                    self.emit(Opcode::PopGlobal, ci, 0, 0);
                                                    self.emit(Opcode::Pop, 0, 0, 0);
                                                }
                                            }
                                            return Ok(());
                                        }
                                    }
                                }
                                _ => {
                                    if self.try_emit_nested_mutate(obj, m, args)? {
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                }
                self.compile_expr(e)?;
                self.emit(Opcode::Pop, 0, 0, 0);
                Ok(())
            }
            StmtKind::Function(name, params, body) => {
                if !self.allow_defs {
                    return Err("nested function definitions are not supported in bytecode".into());
                }
                // Default parameters are handled by the tree-walk runtime, so a
                // module containing them falls back to interpretation.
                if params.iter().any(|(_, d)| d.is_some()) {
                    return Err("default parameters are not supported in bytecode".into());
                }
                let names: Vec<String> = params.iter().map(|(n, _)| n.clone()).collect();
                // Module-level functions reference module globals by name at call
                // time (LoadGlobal), so no captured slots are needed here.
                let cf = compile_function(name, &names, &[], body)?;
                let idx = self.nested.len() + 1; // main is index 0 in the final table
                self.nested.push((*cf).clone());
                let ci = self.const_str(name);
                self.emit(Opcode::DefineFunction, ci, idx as u16, 0);
                Ok(())
            }
            StmtKind::Import(imports) => {
                let packed = imports
                    .iter()
                    .map(|(m, a)| match a {
                        Some(a) => format!("{m}={a}"),
                        None => m.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join("\u{1f}");
                let ci = self.const_str(&packed);
                self.emit(Opcode::Import, ci, 0, 0);
                Ok(())
            }
            StmtKind::FromImport(module, items) => {
                let cm = self.const_str(module);
                let packed = items
                    .iter()
                    .map(|(n, a)| match a {
                        Some(a) => format!("{n}={a}"),
                        None => n.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join("\u{1f}");
                let ci = self.const_str(&packed);
                self.emit(Opcode::ImportFrom, cm, ci, 0);
                Ok(())
            }
            StmtKind::StarImport(module) => {
                let cm = self.const_str(module);
                self.emit(Opcode::ImportStar, cm, 0, 0);
                Ok(())
            }
            _ => Err(
                "statement not supported by the bytecode compiler".into(),
            ),
        }
    }

fn local_opcode(&self, op: &Kind) -> Result<Opcode, String> {
        Ok(match op {
            Kind::PlusAssign => Opcode::AddLocal,
            Kind::MinusAssign => Opcode::SubLocal,
            Kind::StarAssign => Opcode::MulLocal,
            Kind::SlashAssign => Opcode::DivLocal,
            Kind::PercentAssign => Opcode::ModLocal,
            _ => return Err("unsupported assignment operator in bytecode".into()),
        })
    }

    fn binary_opcode(&self, op: &Kind) -> Result<Opcode, String> {
        Ok(match op {
            Kind::PlusAssign => Opcode::AddGlobal,
            Kind::MinusAssign => Opcode::SubGlobal,
            Kind::StarAssign => Opcode::MulGlobal,
            Kind::SlashAssign => Opcode::DivGlobal,
            Kind::PercentAssign => Opcode::ModGlobal,
            _ => return Err("unsupported assignment operator in bytecode".into()),
        })
    }

    /// Statement-position list mutation through a variable-rooted member/index
    /// chain (`d.l.push(x)`, `d["l"].push(x)`, `d.a.b.push(x)`). Emits the
    /// NestedMutate opcode with a packed chain descriptor and the mutator
    /// arguments on the stack, so the runtime can write the mutated list back
    /// through the chain. Returns Ok(false) when `obj` isn't a supported path.
    fn try_emit_nested_mutate(&mut self, obj: &Expr, method: &str, args: &[Expr]) -> Result<bool, String> {
        enum Step {
            Key(String),
            Index,
        }
        // Walk the chain terminal-to-root, then reverse so the steps and index
        // expressions are in root-to-terminal (evaluation) order.
        let mut steps: Vec<Step> = Vec::new();
        let mut index_exprs: Vec<&Expr> = Vec::new();
        let mut cur = obj;
        let root = loop {
            match cur {
                Expr::Member(inner, field) => {
                    steps.push(Step::Key(field.clone()));
                    cur = inner;
                }
                Expr::Index(inner, idx) => {
                    steps.push(Step::Index);
                    index_exprs.push(idx);
                    cur = inner;
                }
                Expr::Var(name) => break name,
                _ => return Ok(false),
            }
        };
        steps.reverse();
        index_exprs.reverse();
        if steps.is_empty() {
            return Ok(false);
        }
        // Descriptor: "g:name\u{1f}method\u{1f}step..." for globals, or
        // "s:slot_idx\u{1f}method\u{1f}step..." for function-locals.
        let root_info = if let Some(&slot) = self.slots.get(root) {
            format!("s:{slot}")
        } else {
            format!("g:{root}")
        };
        let mut desc = String::new();
        desc.push_str(&root_info);
        desc.push('\u{1f}');
        desc.push_str(method);
        let mut index_count: u16 = 0;
        for s in &steps {
            desc.push('\u{1f}');
            match s {
                Step::Key(k) => {
                    desc.push_str("k:");
                    desc.push_str(k);
                }
                Step::Index => {
                    desc.push('i');
                    index_count += 1;
                }
            }
        }
        let ci = self.const_str(&desc);
        // Compute index step values (in step order) below the mutator args.
        for idx in &index_exprs {
            self.compile_expr(idx)?;
        }
        for a in args {
            self.compile_expr(a)?;
        }
        self.emit(Opcode::NestedMutate, ci, args.len() as u16, index_count);
        Ok(true)
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::Value(Value::Bool(b)) => {
                self.emit(if *b { Opcode::True } else { Opcode::False }, 0, 0, 0);
                Ok(())
            }
            Expr::Value(Value::Null) => {
                self.emit(Opcode::Null, 0, 0, 0);
                Ok(())
            }
            Expr::Value(Value::Number(n)) => {
                let ci = self.const_num(*n);
                self.emit(Opcode::Const, ci, 0, 0);
                Ok(())
            }
            Expr::Value(Value::String(s)) => {
                let ci = self.const_str(s);
                self.emit(Opcode::Const, ci, 0, 0);
                Ok(())
            }
            Expr::Value(_) => Err("unsupported constant in bytecode".into()),
            Expr::Var(n) => {
                if let Some(&slot) = self.slots.get(n) {
                    self.emit(Opcode::LoadLocal, slot, 0, 0);
                } else {
                    let ci = self.const_str(n);
                    self.emit(Opcode::LoadGlobal, ci, 0, 0);
                }
                Ok(())
            }
            Expr::List(items) => {
                for item in items {
                    if matches!(item, Expr::Spread(_)) {
                        return Err("spread in list not supported in bytecode".into());
                    }
                    self.compile_expr(item)?;
                }
                self.emit(Opcode::BuildList, items.len() as u16, 0, 0);
                Ok(())
            }
            Expr::Dict(entries) => {
                for entry in entries {
                    match entry {
                        DictEntry::Pair(k, v) => {
                            let ci = self.const_str(k);
                            self.emit(Opcode::Const, ci, 0, 0);
                            self.compile_expr(v)?;
                        }
                        DictEntry::Spread(_) | DictEntry::Computed(_, _) => {
                            return Err("spread/computed keys in dict not supported in bytecode".into());
                        }
                    }
                }
                self.emit(Opcode::BuildDict, entries.len() as u16, 0, 0);
                Ok(())
            }
            Expr::Unary(op, e) => match op {
                Kind::Minus => {
                    self.compile_expr(e)?;
                    self.emit(Opcode::Neg, 0, 0, 0);
                    Ok(())
                }
                Kind::Bang | Kind::Not => {
                    self.compile_expr(e)?;
                    self.emit(Opcode::Not, 0, 0, 0);
                    Ok(())
                }
                Kind::Typeof => {
                    self.compile_expr(e)?;
                    self.emit(Opcode::Typeof, 0, 0, 0);
                    Ok(())
                }
                _ => Err("unsupported unary operator in bytecode".into()),
            },
            Expr::Binary(l, op, r) => match op {
                Kind::And => {
                    self.compile_expr(l)?;
                    self.emit(Opcode::Dup, 0, 0, 0);
                    let jf = self.emit(Opcode::JmpIfFalse, 0, 0, 0);
                    self.emit(Opcode::Pop, 0, 0, 0);
                    self.compile_expr(r)?;
                    self.patch(jf, self.instructions.len());
                    Ok(())
                }
                Kind::Or => {
                    self.compile_expr(l)?;
                    self.emit(Opcode::Dup, 0, 0, 0);
                    let jt = self.emit(Opcode::JmpIfTrue, 0, 0, 0);
                    self.emit(Opcode::Pop, 0, 0, 0);
                    self.compile_expr(r)?;
                    self.patch(jt, self.instructions.len());
                    Ok(())
                }
                Kind::Nullish => {
                    self.compile_expr(l)?;
                    self.emit(Opcode::Dup, 0, 0, 0);
                    let jn = self.emit(Opcode::JmpIfNotNull, 0, 0, 0);
                    self.emit(Opcode::Pop, 0, 0, 0);
                    self.compile_expr(r)?;
                    self.patch(jn, self.instructions.len());
                    Ok(())
                }
                _ => {
                    self.compile_expr(l)?;
                    self.compile_expr(r)?;
                    self.emit(self.binary_expr_opcode(op)?, 0, 0, 0);
                    Ok(())
                }
            },
            Expr::Ternary(c, y, n) => {
                self.compile_expr(c)?;
                let jf = self.emit(Opcode::JmpIfFalse, 0, 0, 0);
                self.compile_expr(y)?;
                let je = self.emit(Opcode::Jmp, 0, 0, 0);
                self.patch(jf, self.instructions.len());
                self.compile_expr(n)?;
                self.patch(je, self.instructions.len());
                Ok(())
            }
            Expr::Index(o, i) => {
                self.compile_expr(o)?;
                self.compile_expr(i)?;
                self.emit(Opcode::Index, 0, 0, 0);
                Ok(())
            }
            Expr::Member(o, name) => {
                self.compile_expr(o)?;
                let ci = self.const_str(name);
                self.emit(Opcode::GetMember, ci, 0, 0);
                Ok(())
            }
            Expr::Call(callee, args) => {
                // Method call: compile target object, then args, then invoke.
                if let Expr::Member(obj, method) = callee.as_ref() {
                    // Stack order: target object first, then args.
                    for arg in args {
                        if matches!(arg, Expr::Named(_, _)) {
                            return Err("named arguments not supported in bytecode".into());
                        }
                    }
                    self.compile_expr(obj)?;
                    for arg in args {
                        self.compile_expr(arg)?;
                    }
                    let ci = self.const_str(method);
                    self.emit(Opcode::CallMethod, ci, args.len() as u16, 0);
                    return Ok(());
                }
                // General callee expression (e.g. a value held in a list,
                // dict, or returned from another call): evaluate the callable
                // first, then the args, then invoke.
                if !matches!(callee.as_ref(), Expr::Var(_)) {
                    for arg in args {
                        if matches!(arg, Expr::Named(_, _)) {
                            return Err("named arguments not supported in bytecode".into());
                        }
                    }
                    self.compile_expr(callee)?;
                    for arg in args {
                        self.compile_expr(arg)?;
                    }
                    self.emit(Opcode::CallValue, args.len() as u16, 0, 0);
                    return Ok(());
                }
                // Named call fast path ONLY when the name cannot be a local
                // slot (params/captured). Slot-held callables (function
                // values passed as arguments) go through CallValue.
                let Expr::Var(name) = callee.as_ref() else {
                    return Err("unsupported call target in bytecode".into());
                };
                if let Some(&slot) = self.slots.get(name) {
                    // Callee value lives in a local slot: load it first, then
                    // args, then invoke generically.
                    for arg in args {
                        if matches!(arg, Expr::Named(_, _)) {
                            return Err("named arguments not supported in bytecode".into());
                        }
                    }
                    self.emit(Opcode::LoadLocal, slot, 0, 0);
                    for arg in args {
                        self.compile_expr(arg)?;
                    }
                    self.emit(Opcode::CallValue, args.len() as u16, 0, 0);
                    return Ok(());
                }
                for arg in args {
                    if matches!(arg, Expr::Named(_, _)) {
                        return Err("named arguments not supported in bytecode".into());
                    }
                    self.compile_expr(arg)?;
                }
                let ci = self.const_str(name);
                self.emit(Opcode::Call, ci, args.len() as u16, 0);
                Ok(())
            }
            Expr::Increment(target, amount) => {
                let Expr::Var(name) = target.as_ref() else {
                    return Err("increment requires a variable in bytecode".into());
                };
                if let Some(&slot) = self.slots.get(name) {
                    self.emit(Opcode::LoadLocal, slot, 0, 0);
                    let ci = self.const_num(*amount as f64);
                    self.emit(Opcode::Const, ci, 0, 0);
                    self.emit(Opcode::Add, 0, 0, 0);
                    self.emit(Opcode::Dup, 0, 0, 0);
                    self.emit(Opcode::StoreLocal, slot, 0, 0);
                    return Ok(());
                }
                let ni = self.const_str(name);
                self.emit(Opcode::LoadGlobal, ni, 0, 0);
                let ci = self.const_num(*amount as f64);
                self.emit(Opcode::Const, ci, 0, 0);
                self.emit(Opcode::Add, 0, 0, 0);
                self.emit(Opcode::Dup, 0, 0, 0);
                self.emit(Opcode::StoreGlobal, ni, 0, 0);
                Ok(())
            }
            Expr::Lambda(params, body) => {
                if !self.allow_defs {
                    return Err("lambdas are not supported in bytecode".into());
                }
                if params.iter().any(|(_, d)| d.is_some()) {
                    return Err("default parameters are not supported in bytecode".into());
                }
                let fname = format!("__lambda_{}", self.nested.len());
                let names: Vec<String> = params.iter().map(|(n, _)| n.clone()).collect();
                // Module-level lambdas reference globals by name, no capture slots.
                let cf = compile_function(&fname, &names, &[], body)?;
                let idx = self.nested.len() + 1;
                self.nested.push((*cf).clone());
                self.emit(Opcode::Closure, idx as u16, 0, 0);
                Ok(())
            }
            Expr::Range(rs, re, exclusive) => {
                self.compile_expr(rs)?;
                self.compile_expr(re)?;
                self.emit(Opcode::MakeRange, *exclusive as u16, 0, 0);
                Ok(())
            }
            _ => Err("expression not supported by the bytecode compiler".into()),
        }
    }

    fn binary_expr_opcode(&self, op: &Kind) -> Result<Opcode, String> {
        Ok(match op {
            Kind::Plus => Opcode::Add,
            Kind::Minus => Opcode::Sub,
            Kind::Star => Opcode::Mul,
            Kind::Slash => Opcode::Div,
            Kind::Percent => Opcode::Mod,
            Kind::Pow => Opcode::Pow,
            Kind::Eq => Opcode::Eq,
            Kind::Ne => Opcode::Ne,
            Kind::Lt => Opcode::Lt,
            Kind::Le => Opcode::Le,
            Kind::Gt => Opcode::Gt,
            Kind::Ge => Opcode::Ge,
            _ => return Err("unsupported binary operator in bytecode".into()),
        })
    }
}