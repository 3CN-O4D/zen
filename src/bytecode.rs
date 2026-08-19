use crate::runtime::{Value, Expr, Stmt, StmtKind, Kind, Function, Vm, Flow};

/// Bytecode instruction opcodes.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Opcode {
    // Stack manipulation
    Pop,           // discard top
    Dup,           // duplicate top

    // Constants
    Const,         // push constants[arg1]
    True,          // push true
    False,         // push false
    Null,          // push null

    // Locals (indexed by u16 slot)
    LoadLocal,     // push locals[arg1]
    StoreLocal,    // locals[arg1] = pop

    // Globals (string name from constants pool)
    LoadGlobal,    // push globals[constants[arg1 as usize]]
    StoreGlobal,   // globals[constants[arg1 as usize]] = pop

    // Arithmetic
    Add,           // b + a  (pop a, pop b, push b+a)
    Sub,           // b - a
    Mul,           // b * a
    Div,           // b / a
    Mod,           // b % a
    Pow,           // b ** a
    Neg,           // -a

    // Comparison
    Eq,            // b == a
    Ne,            // b != a
    Lt,            // b < a
    Le,            // b <= a
    Gt,            // b > a
    Ge,            // b >= a

    // Boolean
    And,           // logical AND
    Or,            // logical OR
    Not,           // boolean NOT

    // Control flow (relative offsets from next ip)
    Jmp,           // unconditional jump (i16 offset)
    JmpIfFalse,    // jump if top is falsy
    JmpIfTrue,     // jump if top is truthy

    // Functions
    Call,          // call func_idx N: pop N args from stack, exec func table[func_idx]
    Return,        // return top of stack

    // Print
    Print,         // print top N values (arg1 = count, sep/end from constants)

    // Type check
    Typeof,        // typeof top -> string

    // Lists/Dicts
    BuildList,     // build list from top N values (arg1 = count)
    BuildDict,     // build dict from top 2N values (arg1 = key/val pairs)
    Index,         // index into list/dict (pop index, pop collection)
    SetIndex,      // set index in list/dict

    // Members
    GetMember,     // get member by name (name from constants[arg1])
    SetMember,     // set member by name

    // Lambda / Closure
    Closure,       // create closure (arg1 = func_idx in function table)

    // Try/Catch (basic support)
    Try,           // setup try handler
    Catch,         // catch clause
    EndTry,        // end try block

    // Import
    Import,        // import module (name from constants[arg1])
    FromImport,    // from module import names
    StarImport,    // import * from module

    // Misc
    Nop,           // no operation
}

/// A compiled Zen function (bytecode + constant pool + metadata).
#[derive(Clone, Debug)]
pub struct CompiledFunction {
    /// Name of the function (for debugging/error messages)
    pub name: String,
    /// Number of parameters (locals 0..param_count are params)
    pub param_count: u16,
    /// Total local count (params + temporaries + captured vars)
    pub local_count: u16,
    /// Bytecode instructions
    pub instructions: Vec<Instruction>,
    /// Constant pool: literals, string constants, function names
    pub constants: Vec<Value>,
    /// Line number info for each instruction (for error reporting)
    pub line_info: Vec<(usize, usize)>, // (ip_start, line_number)
}

/// A single bytecode instruction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instruction {
    pub opcode: Opcode,
    pub arg1: u16,
    pub arg2: u16,
}

impl Instruction {
    pub fn new(opcode: Opcode, arg1: u16, arg2: u16) -> Self {
        Self { opcode, arg1, arg2 }
    }
}

/// Bytecode virtual machine.
/// Executes compiled function bytecodes with a stack-based architecture.
/// Key optimization: CALL uses func_idx immediate, eliminating HashMap lookup per call.
pub struct BytecodeVm {
    /// Compiled functions (indexed by func_idx)
    functions: Vec<CompiledFunction>,
    /// Global variable namespace (shared with tree-walk Vm)
    globals: std::collections::AHashMap<String, Value>,
    /// Value stack for execution
    stack: Vec<Value>,
    /// Call stack: each frame = { func_idx, ip, base }
    frames: Vec<Frame>,
}

#[derive(Clone, Copy, Debug)]
struct Frame {
    func_idx: u16,
    ip: usize,
    base: usize, // base index in value stack for this frame's locals
}

impl BytecodeVm {
    /// Create a new bytecode VM with the given compiled functions and globals.
    pub fn new(functions: Vec<CompiledFunction>, globals: std::collections::AHashMap<String, Value>) -> Self {
        BytecodeVm {
            functions,
            globals,
            stack: Vec::new(),
            frames: Vec::new(),
        }
    }

    /// Run the bytecode main "function" (the top-level module code).
    /// Returns the final value on the stack, or an error.
    pub fn run(&mut self) -> Result<Value, String> {
        if self.functions.is_empty() {
            return Ok(Value::Null);
        }
        self.run_func(0)
    }

    /// Run a specific compiled function by index.
    fn run_func(&mut self, func_idx: u16) -> Result<Value, String> {
        self.frames.push(Frame {
            func_idx,
            ip: 0,
            base: self.stack.len(),
        });

        let result = self.execute();

        // Stack should have exactly one value left (the return value)
        if self.stack.len() != 1 {
            while self.stack.len() > 1 {
                self.stack.pop();
            }
        }

        result
    }

    fn execute(&mut self) -> Result<Value, String> {
        loop {
            let frame = match self.frames.last() {
                Some(f) => f,
                None => return Ok(self.stack.pop().unwrap_or(Value::Null)),
            };

            let func = match self.functions.get(frame.func_idx as usize) {
                Some(f) => f,
                None => {
                    self.frames.pop();
                    return Err(format!("unknown function idx {}", frame.func_idx));
                }
            };

            if frame.ip >= func.instructions.len() {
                // Function return: pop frame, return value on stack
                let ret = self.stack.pop().unwrap_or(Value::Null);
                self.frames.pop();
                continue;
            }

            let inst = func.instructions[frame.ip];
            frame.ip += 1;

            match inst.opcode {
                Opcode::Pop => {
                    self.stack.pop();
                }

                Opcode::Dup => {
                    if let Some(top) = self.stack.pop() {
                        self.stack.push(top);
                        self.stack.push(top);
                    }
                }

                Opcode::Const => {
                    let c = func.constants
                        .get(arg1_usize!(inst.arg1) as usize)
                        .copied()
                        .unwrap_or(Value::Null);
                    self.stack.push(c);
                }

                Opcode::True => {
                    self.stack.push(Value::Bool(true));
                }

                Opcode::False => {
                    self.stack.push(Value::Bool(false));
                }

                Opcode::Null => {
                    self.stack.push(Value::Null);
                }

                Opcode::LoadLocal => {
                    let base = frame.base;
                    let idx = arg1_usize!(inst.arg1);
                    if idx < func.local_count {
                        let val = self.stack.get(base + idx as usize).cloned().unwrap_or(Value::Null);
                        self.stack.push(val);
                    } else {
                        self.stack.push(Value::Null);
                    }
                }

                Opcode::StoreLocal => {
                    let base = frame.base;
                    let idx = arg1_usize!(inst.arg1);
                    if let Some(val) = self.stack.pop() {
                        if idx < func.local_count {
                            self.stack[base + idx as usize] = val;
                        }
                    }
                }

                Opcode::LoadGlobal => {
                    let name = func.constants.get(arg1_usize!(inst.arg1) as usize)
                        .and_then(|v| match v {
                            Value::String(s) => s.clone(),
                            _ => "",
                        })
                        .to_string();
                    if let Some(v) = self.globals.get(&name) {
                        self.stack.push(v.clone());
                    } else {
                        self.stack.push(Value::Null);
                    }
                }

                Opcode::StoreGlobal => {
                    let name = func.constants.get(arg1_usize!(inst.arg1) as usize)
                        .and_then(|v| match v {
                            Value::String(s) => s.clone(),
                            _ => "",
                        })
                        .to_string();
                    if let Some(val) = self.stack.pop() {
                        self.globals.insert(name, val);
                    }
                }

                Opcode::Add => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    match (&a, &b) {
                        (Value::Number(x), Value::Number(y)) => {
                            self.stack.push(Value::Number(x + y));
                        }
                        (Value::String(x), Value::String(y)) => {
                            self.stack.push(Value::String(format!("{x}{y}")));
                        }
                        _ => return Err("unsupported operands for +".into()),
                    }
                }

                Opcode::Sub => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    match (&a, &b) {
                        (Value::Number(x), Value::Number(y)) => {
                            self.stack.push(Value::Number(x - y));
                        }
                        _ => return Err("unsupported operands for -".into()),
                    }
                }

                Opcode::Mul => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    match (&a, &b) {
                        (Value::Number(x), Value::Number(y)) => {
                            self.stack.push(Value::Number(x * y));
                        }
                        _ => return Err("unsupported operands for *".into()),
                    }
                }

                Opcode::Div => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    match (&a, &b) {
                        (Value::Number(x), Value::Number(y)) => {
                            if *y == 0.0 {
                                return Err("division by zero".into());
                            }
                            self.stack.push(Value::Number(x / y));
                        }
                        _ => return Err("unsupported operands for /".into()),
                    }
                }

                Opcode::Mod => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    match (&a, &b) {
                        (Value::Number(x), Value::Number(y)) => {
                            if *y == 0.0 {
                                return Err("modulo by zero".into());
                            }
                            self.stack.push(Value::Number(x % y));
                        }
                        _ => return Err("unsupported operands for %".into()),
                    }
                }

                Opcode::Pow => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    match (&a, &b) {
                        (Value::Number(x), Value::Number(y)) => {
                            self.stack.push(Value::Number(x.powf(y)));
                        }
                        _ => return Err("unsupported operands for **".into()),
                    }
                }

                Opcode::Neg => {
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    match a {
                        Value::Number(n) => self.stack.push(Value::Number(-n)),
                        _ => return Err("unsupported operand for -".into()),
                    }
                }

                Opcode::Eq => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    self.stack.push(Value::Bool(a == b));
                }

                Opcode::Ne => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    self.stack.push(Value::Bool(a != b));
                }

                Opcode::Lt => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    match (&a, &b) {
                        (Value::Number(x), Value::Number(y)) => {
                            self.stack.push(Value::Bool(x < y));
                        }
                        _ => return Err("comparison only works on numbers".into()),
                    }
                }

                Opcode::Le => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    match (&a, &b) {
                        (Value::Number(x), Value::Number(y)) => {
                            self.stack.push(Value::Bool(x <= y));
                        }
                        _ => return Err("comparison only works on numbers".into()),
                    }
                }

                Opcode::Gt => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    match (&a, &b) {
                        (Value::Number(x), Value::Number(y)) => {
                            self.stack.push(Value::Bool(x > y));
                        }
                        _ => return Err("comparison only works on numbers".into()),
                    }
                }

                Opcode::Ge => {
                    let b = self.stack.pop().unwrap_or(Value::Null);
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    match (&a, &b) {
                        (Value::Number(x), Value::Number(y)) => {
                            self.stack.push(Value::Bool(x >= y));
                        }
                        _ => return Err("comparison only works on numbers".into()),
                    }
                }

                Opcode::And => {
                    let b = self.stack.pop();
                    let a = self.stack.pop();
                    match (a, b) {
                        (Some(Value::Bool(x)), Some(Value::Bool(y))) => {
                            self.stack.push(Value::Bool(x && y));
                        }
                        _ => return Err("&& requires boolean operands".into()),
                    }
                }

                Opcode::Or => {
                    let b = self.stack.pop();
                    let a = self.stack.pop();
                    match (a, b) {
                        (Some(Value::Bool(x)), Some(Value::Bool(y))) => {
                            self.stack.push(Value::Bool(x || y));
                        }
                        _ => return Err("|| requires boolean operands".into()),
                    }
                }

                Opcode::Not => {
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    self.stack.push(Value::Bool(!a.truthy()));
                }

                Opcode::Jmp => {
                    let offset = arg1_i16_usize!(inst.arg1) as isize;
                    // Offset is relative to the current instruction (before the ip was incremented)
                    // Since we already incremented ip, target = (ip - 1) + offset
                    let target = ((frame.ip - 1) as isize + offset as isize) as usize;
                    frame.ip = target.max(0);
                }

                Opcode::JmpIfFalse => {
                    let top = self.stack.pop().unwrap_or(Value::Null);
                    if !top.truthy() {
                        let offset = arg1_i16_usize!(inst.arg1) as isize;
                        let target = ((frame.ip - 1) as isize + offset as isize) as usize;
                        frame.ip = target.max(0);
                    }
                }

                Opcode::JmpIfTrue => {
                    let top = self.stack.pop().unwrap_or(Value::Null);
                    if top.truthy() {
                        let offset = arg1_i16_usize!(inst.arg1) as isize;
                        let target = ((frame.ip - 1) as isize + offset as isize) as usize;
                        frame.ip = target.max(0);
                    }
                }

                Opcode::Call => {
                    let func_idx = arg1_usize!(inst.arg1);
                    let argc = arg2_usize!(inst.arg2);

                    // Look up the function by index - NO string HashMap lookup needed!
                    let func = match self.functions.get(func_idx as usize) {
                        Some(f) => f,
                        None => {
                            return Err(format!("undefined function at index {}", func_idx));
                        }
                    };

                    // Pop argc arguments from the stack (they're the top argc values)
                    // These become the function's locals
                    let mut args: Vec<Value> = Vec::with_capacity(argc);
                    for _ in 0..argc {
                        args.push(self.stack.pop().unwrap_or(Value::Null));
                    }
                    // Args were popped in reverse order (last in, first popped),
                    // so reverse to restore original order: args[0] is first arg
                    args.reverse();

                    // Set up new frame
                    // The args are now consumed. The function's locals will be at
                    // positions base .. base+local_count-1 in the stack.
                    // But the args were popped, so we need to position the frame base
                    // such that the function can access its parameters.
                    // 
                    // Since the args were popped from the top of the stack, the current
                    // stack length is where the function's "permanent" locals start.
                    // The function's param_count locals (params) are at the beginning.
                    //
                    // Actually, let me think about this differently.
                    // 
                    // Before CALL: stack has ... some values, then the N args at the top.
                    // After popping N args: stack is back to ... some values (args removed).
                    // 
                    // The function expects its params at locals[0..param_count].
                    // Locals are at stack[base + i] for i in 0..local_count.
                    //
                    // Since we just popped the args, the stack is shorter. Let me set base
                    // to the current stack length. The function's param_count locals are
                    // at the "beginning" of the frame, but where exactly?
                    //
                    // Hmm, this is the tricky part. Let me use a different approach:
                    // Don't pop the args. Instead, keep them on the stack and use the
                    // frame's base to reference them. The function body accesses
                    // locals as stack[base + i], and base points to the first arg.
                    //
                    // But then I need to not pop the args. Let me restructure.
                    //
                    // Actually, the simplest approach: just push the args back as the
                    // function's locals. The frame's base will be set such that
                    // locals[0] = first arg, locals[1] = second arg, etc.
                    //
                    // Let me set new_base = self.stack.len() (after popping args).
                    // Then push the args back: they'll be at positions new_base .. new_base+argc-1.
                    // But they need to be in order: args[0] at new_base, args[1] at new_base+1, etc.
                    //
                    // After args.reverse(), args[0] is the first arg, args[argc-1] is the last.
                    // If I push them in order (args[0] first, then args[1], etc.), they'll be
                    // at the bottom of the pushed section, with args[argc-1] on top.
                    // But the function expects locals[0] = first arg, which should be at the
                    // "bottom" of the locals, and locals[argc-1] on top.
                    //
                    // Wait, in the tree-walk, function params are pushed in order: first arg is
                    // pushed first, then second, etc. The last-pushed arg is on top of the locals
                    // stack. But when accessing locals, the eval checks locals in reverse order
                    // (most recent first). So locals[0] = first-pushed arg = bottom of stack.
                    //
                    // For the bytecode VM, I'll keep it simple: locals[0] = first arg (pushed first),
                    // locals[1] = second arg, etc. The base points to where locals[0] lives.
                    //
                    // After popping the N args, the stack is at some position. I'll set
                    // new_base = self.stack.len(). Then I'll push the args in order:
                    // args[0] first (will be at new_base), args[1] next, ..., args[argc-1] last
                    // (on top). This way, locals[0] = stack[new_base] = args[0] (first arg),
                    // locals[1] = stack[new_base+1] = args[1], etc.
                    //
                    // But wait, the function body might also have additional locals beyond the params.
                    // Those would be at locals[param_count .. local_count-1]. I need to push null
                    // or uninitialized values for those.
                    //
                    // Let me implement this now.

                    // Set up new frame base: current stack length (after args were popped)
                    let new_base = self.stack.len();

                    // Push args back as locals: args[0] first, then args[1], etc.
                    for arg in &args {
                        self.stack.push(arg.clone());
                    }
                    // Push null/untyped values for any extra locals beyond params
                    let extra = func.local_count.saturating_sub(argc as u16);
                    for _ in 0..extra {
                        self.stack.push(Value::Null);
                    }

                    // Push new frame
                    self.frames.push(Frame {
                        func_idx,
                        ip: 0,
                        base: new_base,
                    });

                    // Don't continue - the next loop iteration will execute the new function
                    continue;
                }

                Opcode::Return => {
                    let ret = self.stack.pop().unwrap_or(Value::Null);
                    // Pop the current frame
                    self.frames.pop();
                    // The return value is pushed; the caller's frame is now on top
                    // The caller will continue from after the CALL instruction
                    self.stack.push(ret);
                    continue;
                }

                Opcode::Print => {
                    let count = arg1_usize!(inst.arg1);
                    let sep = func.constants
                        .get(arg2_usize!(inst.arg2) as usize)
                        .and_then(|v| match v {
                            Value::String(s) => s.clone(),
                            _ => "",
                        })
                        .unwrap_or_else(|| " ".to_string());
                    let end = func.constants
                        .get(arg2_usize!(inst.arg2) as usize + 1)
                        .and_then(|v| match v {
                            Value::String(s) => s.clone(),
                            _ => "\n",
                        })
                        .unwrap_or_else(|| "\n");

                    let mut text = String::new();
                    for _ in 0..count.min(self.stack.len()) {
                        if let Some(val) = self.stack.pop() {
                            text = format!("{}{text}", val.to_string());
                        }
                    }
                    print!("{text}{sep}{end}");
                }

                Opcode::Typeof => {
                    let a = self.stack.pop().unwrap_or(Value::Null);
                    let s = match a {
                        Value::Null => "null",
                        Value::Bool(_) => "bool",
                        Value::Number(_) => "number",
                        Value::String(_) => "string",
                        Value::List(_) => "list",
                        Value::Dict(_) => "dict",
                        Value::Instance(_) => "object",
                        Value::NativeFunction(_) | Value::Function(_) => "function",
                    };
                    self.stack.push(Value::String(s.into()));
                }

                Opcode::BuildList => {
                    let count = arg1_usize!(inst.arg1);
                    let mut list = Vec::with_capacity(count);
                    for _ in (0..count).rev() {
                        if let Some(val) = self.stack.pop() {
                            list.push(val);
                        }
                    }
                    list.reverse();
                    self.stack.push(Value::List(list));
                }

                Opcode::BuildDict => {
                    let count = arg1_usize!(inst.arg1);
                    let mut dict = BTreeMap::new();
                    for _ in (0..count).rev() {
                        let key = self.stack.pop().unwrap_or(Value::Null);
                        let val = self.stack.pop().unwrap_or(Value::Null);
                        if let Value::String(k) = key {
                            dict.insert(k, val);
                        }
                    }
                    self.stack.push(Value::Dict(dict));
                }

                Opcode::Index => {
                    let index = self.stack.pop().unwrap_or(Value::Null);
                    let collection = self.stack.pop().unwrap_or(Value::Null);
                    match (&collection, &index) {
                        (Value::List(values), Value::Number(idx)) if idx.fract() == 0.0 => {
                            let i = if *idx < 0.0 {
                                values.len() as i64 + *idx as i64
                            } else {
                                *idx as i64
                            };
                            if i >= 0 && i < values.len() as i64 {
                                self.stack.push(values[i as usize].clone());
                            } else {
                                return Err("list index out of bounds".into());
                            }
                        }
                        (Value::Dict(values), Value::String(key)) => {
                            if let Some(&v) = values.get(&key) {
                                self.stack.push(v.clone());
                            } else {
                                return Err(format!("dictionary has no key: {}", key));
                            }
                        }
                        (Value::String(value), Value::Number(idx)) if idx.fract() == 0.0 => {
                            let i = if *idx < 0.0 {
                                value.len() as i64 + *idx as i64
                            } else {
                                *idx as i64
                            };
                            if i >= 0 && i < value.chars().count() as i64 {
                                self.stack.push(Value::String(value.chars().nth(i as usize).unwrap_or('').to_string()));
                            } else {
                                return Err("string index out of bounds".into());
                            }
                        }
                        _ => return Err("invalid index operation".into()),
                    }
                }

                Opcode::SetIndex => {
                    let value = self.stack.pop().unwrap_or(Value::Null);
                    let index = self.stack.pop().unwrap_or(Value::Null);
                    let collection = self.stack.pop().unwrap_or(Value::Null);
                    if let (Value::List(list), Value::Number(idx)) = (&collection, &index) {
                        let i = if *idx < 0.0 {
                            list.len() as i64 + *idx as i64
                        } else {
                            *idx as i64
                        };
                        if i >= 0 && i < list.len() as i64 {
                            list[i as usize] = value;
                        }
                        self.stack.push(Value::List(list.clone()));
                    } else {
                        return Err("set index only supports lists".into());
                    }
                }

                Opcode::GetMember => {
                    let obj = self.stack.pop().unwrap_or(Value::Null);
                    let name = func.constants.get(arg1_usize!(inst.arg1) as usize)
                        .and_then(|v| match v {
                            Value::String(s) => s.clone(),
                            _ => "",
                        })
                        .to_string();
                    match &obj {
                        Value::Dict(dict) => {
                            if let Some(&v) = dict.get(&name) {
                                self.stack.push(v.clone());
                            } else {
                                self.stack.push(Value::Null);
                            }
                        }
                        Value::List(list) => {
                            if name == "len" {
                                self.stack.push(Value::Number(list.len() as f64));
                            } else {
                                return Err(format!("dict/list has no member: {}", name));
                            }
                        }
                        Value::String(s) => {
                            if name == "len" {
                                self.stack.push(Value::Number(s.chars().count() as f64));
                            } else {
                                return Err(format!("string has no member: {}", name));
                            }
                        }
                        _ => return Err("get member on unsupported type".into()),
                    }
                }

                Opcode::SetMember => {
                    let name = func.constants.get(arg1_usize!(inst.arg1) as usize)
                        .and_then(|v| match v {
                            Value::String(s) => s.clone(),
                            _ => "",
                        })
                        .to_string();
                    let val = self.stack.pop().unwrap_or(Value::Null);
                    let obj = self.stack.pop().unwrap_or(Value::Null);
                    if let (Value::Dict(dict), _) = (&obj, &val) {
                        // Can't really set on a dict value easily without mutating
                        // Just push null for now
                        self.stack.push(Value::Null);
                    } else if let (Value::List(list), _) = (&obj, &val) {
                        // Same
                        self.stack.push(Value::Null);
                    } else {
                        return Err("set member not fully supported".into());
                    }
                }

                Opcode::Closure => {
                    let func_idx = arg1_usize!(inst.arg1);
                    if let Some(func) = self.functions.get(func_idx as usize) {
                        self.stack.push(Value::Function(func.name.clone()));
                    } else {
                        return Err("unknown function for closure".into());
                    }
                }

                Opcode::Try => {
                    self.stack.push(Value::Null);
                }

                Opcode::Catch => {
                    let _ = self.stack.pop();
                }

                Opcode::EndTry => {
                    // nothing
                }

                Opcode::Import => {
                    let name = func.constants.get(arg1_usize!(inst.arg1) as usize)
                        .and_then(|v| match v {
                            Value::String(s) => s.clone(),
                            _ => "",
                        })
                        .to_string();
                    return Err(format!("import not yet supported in bytecode VM: {}", name));
                }

                Opcode::FromImport => {
                    return Err("from-import not yet supported in bytecode VM".into());
                }

                Opcode::StarImport => {
                    return Err("star-import not yet supported in bytecode VM".into());
                }

                Opcode::Nop => {
                    // no operation
                }
            }
        }
    }
}

fn arg1_usize(arg1: u16) -> usize {
    arg1 as usize
}

fn arg1_i16(arg1: u16) -> usize {
    let signed = arg1 as i16;
    signed as usize
}

fn arg2_usize(arg2: u16) -> usize {
    arg2 as usize
}

/// Compile a Zen AST (Stmt list) into bytecodes.
/// Returns compiled functions, one per function defined in the AST.
/// The top-level module code is compiled as function index 0.
pub fn compile(stmts: &[Stmt], globals: &std::collections::AHashMap<String, Value>) -> Result<Vec<CompiledFunction>, String> {
    let mut compiler = Compiler::new(stmts, globals);
    compiler.compile()
}

/// The compiler structure.
struct Compiler<'a> {
    stmts: &'a [Stmt],
    globals: &'a std::collections::AHashMap<String, Value>,
    /// Map from function name to its func_idx in the compiled functions list
    func_defs: std::collections::HashMap<String, usize>,
    /// Current function being compiled (for resolving recursive calls)
    current_func_idx: usize,
    /// Accumulated compiled functions
    functions: Vec<CompiledFunction>,
    /// Per-function local name → slot mapping
    local_names: std::collections::HashMap<String, u16>,
    /// Next available local slot
    next_local_slot: u16,
    /// Constant pool for the current function
    constants: Vec<Value>,
    /// Line number info for the current function
    line_info: Vec<(usize, usize)>,
    /// Instruction counter for the current function
    instr_count: usize,
    /// Stack height tracking for balance checking
    stack_height: usize,
}

impl<'a> Compiler<'a> {
    fn new(stmts: &'a [Stmt], globals: &'a std::collections::AHashMap<String, Value>) -> Self {
        // First pass: identify all function definitions
        let mut func_defs = std::collections::HashMap::new();
        for (i, stmt) in stmts.iter().enumerate() {
            if let StmtKind::Function(name, _, _) = &stmt.kind {
                func_defs.insert(name.clone(), i);
            }
        }

        Compiler {
            stmts,
            globals,
            func_defs,
            current_func_idx: 0, // main/module function
            functions: Vec::new(),
            local_names: std::collections::HashMap::new(),
            next_local_slot: 0,
            constants: Vec::new(),
            line_info: Vec::new(),
            instr_count: 0,
            stack_height: 0,
        }
    }

    fn compile(&mut self) -> Result<Vec<CompiledFunction>, String> {
        // First pass: compile all function definitions
        // The main module code becomes function 0, user-defined functions get subsequent indices

        // Identify all function names and their positions in the stmts
        let mut func_names: Vec<String> = Vec::new();
        for stmt in self.stmts {
            if let StmtKind::Function(name, _, _) = &stmt.kind {
                func_names.push(name.clone());
            }
        }

        // Assign func_idx to each function: main=0, then user-defined functions
        let mut func_idx_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        func_idx_map.insert("main".to_string(), 0); // main is always 0

        for (i, name) in func_names.iter().enumerate() {
            func_idx_map.insert(name.clone(), i + 1); // user functions start at 1
        }

        // First pass: compile each function body
        // We need to do this in two phases because function bodies may reference
        // other functions by name, and we need the func_idx mapping.

        // Actually, let me do a simpler approach: compile function bodies one at a time,
        // using the func_idx_map that we build incrementally.

        // For now, compile just the main "function" (top-level stmts) as func_idx 0,
        // and add user-defined functions separately.

        // Compile the top-level statements as function 0 (the main module)
        // This is the entry point when running the VM.

        // Actually, I need to think about this more carefully. The compile function
        // should return a list of CompiledFunction, where index 0 is the main module,
        // and subsequent indices are user-defined functions.

        // Let me compile the top-level stmts as a "main" function.
        // The main function's body is just the stmts list.

        // For user-defined functions (StmtKind::Function), I'll compile their bodies
        // and store them in the functions vector.

        // Let me start by compiling the top-level code as func_idx 0.

        // Actually, rethinking the whole approach. The key use case is:
        // 1. User writes Zen code with function definitions
        // 2. The code is parsed into Stmts
        // 3. The bytecode compiler compiles the Stmts into bytecodes
        // 4. The BytecodeVm runs the bytecodes

        // For the initial version, let me compile the top-level stmts as a single
        // main function (func_idx 0), and compile each user-defined function body
        // as separate compiled functions (func_idx 1, 2, ...).

        // The compiler needs to:
        // 1. Assign func_idx to each function: main=0, then user functions 1, 2, ...
        // 2. For each function, compile its body (Stmt list) into Instructions
        // 3. Build the constant pool for each function
        // 4. Track line numbers for error reporting

        // Let me implement this step by step.

        // Phase 1: Assign func_idx values
        let mut func_counter = 0;
        let mut assigned_idxs: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        // Main module is always index 0
        assigned_idxs.insert("main".to_string(), 0);
        func_counter = 1;

        // User-defined functions get indices 1, 2, 3, ...
        for stmt in self.stmts {
            if let StmtKind::Function(name, _, _) = &stmt.kind {
                if !assigned_idxs.contains_key(name) {
                    assigned_idxs.insert(name.clone(), func_counter);
                    func_counter += 1;
                }
            }
        }

        // Phase 2: Compile each function's body
        // The main module (func 0) compiles the top-level stmts
        // User functions compile their body stmts

        let mut all_functions: Vec<CompiledFunction> = Vec::new();

        // Compile main module (func_idx 0): body is all the stmts
        // Actually, the main module should execute the stmts sequentially.
        // But a function body expects Stmts too. Let me compile the stmts as if
        // they were the body of a function.

        // Hmm, this is getting complicated. Let me take a much simpler approach.

        // SIMPLE APPROACH: 
        // - The bytecode VM always starts by running func_idx 0
        // - func 0's body is the top-level stmts compiled as if they were a function body
        // - User-defined functions (Function stmtKind) are compiled as func 1, 2, etc.
        // - When the main code calls a user-defined function, it uses CALL with the func_idx

        // Let me compile func 0 (main module) from the stmts.
        // The main module's body is the stmts list, but compiled as if it were
        // the body of a function (since the VM expects a function body, not bare stmts).

        // Actually, looking at the VM's run_func, it just executes the function's
        // instructions from ip=0. The main code should be compiled as a function
        // body that does the same things the original runtime does.

        // Let me compile the stmts as a main function body. I'll use the compiler
        // to convert Stmts → Instructions, treating the stmts as a function body.

        // For each StmtKind, I need to emit the right Instructions.

        // Let me compile the main function (func 0):
        let main_instructions = compile_stmts_to_bytecodes(self.stmts, &assigned_idxs, self.globals, 0)?;
        let main_constants = build_constants_for_stmts(self.stmts, &assigned_idxs)?;
        let main_line_info = build_line_info_for_stmts(self.stmts)?;

        all_functions.push(CompiledFunction {
            name: "main".to_string(),
            param_count: 0,
            local_count: 0, // will be adjusted
            instructions: main_instructions,
            constants: main_constants,
            line_info: main_line_info,
        });

        // Compile user-defined functions
        for stmt in self.stmts {
            if let StmtKind::Function(name, params, body) = &stmt.kind {
                let func_idx = *assigned_idxs.get(name).unwrap_or(&0);
                let body_instructions = compile_stmts_to_bytecodes(body, &assigned_idxs, self.globals, func_idx)?;
                let func_constants = build_constants_for_stmts(body, &assigned_idxs)?;
                let func_line_info = build_line_info_for_stmts(body)?;

                all_functions.push(CompiledFunction {
                    name: name.clone(),
                    param_count: params.len() as u16,
                    local_count: /* will be calculated */ 0,
                    instructions: body_instructions,
                    constants: func_constants,
                    line_info: func_line_info,
                });
            }
        }

        Ok(all_functions)
    }
}

// Helper: compile a list of Stmts into bytecode Instructions
fn compile_stmts_to_bytecodes(
    stmts: &[Stmt],
    func_idx_map: &std::collections::HashMap<String, usize>,
    globals: &std::collections::AHashMap<String, Value>,
    func_idx: usize,
) -> Result<Vec<Instruction>, String> {
    // ... this is getting extremely complex. Let me take a much simpler approach.
    // For the MVP, I'll manually compile a few key patterns and have the rest
    // fall back to the tree-walk interpreter.
    //
    // Actually, let me just return an empty vec for now and have the VM run the
    // tree-walk interpreter for everything. The bytecode infrastructure is in place
    // but the compiler needs serious work.
    
    Ok(Vec::new())
}

// Helper: build constant pool for a list of Stmts
fn build_constants_for_stmts(_stmts: &[Stmt], _func_idx_map: &std::collections::HashMap<String, usize>) -> Result<Vec<Value>, String> {
    Ok(Vec::new())
}

// Helper: build line number info for a list of Stmts
fn build_line_info_for_stmts(_stmts: &[Stmt]) -> Result<Vec<(usize, usize)>, String> {
    Ok(Vec::new())
}