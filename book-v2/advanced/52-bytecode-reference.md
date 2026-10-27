# Chapter 52: Zen Bytecode Reference

*A reference for the bytecode instructions Zen 3.0 source is lowered to, and
how the interpreter and JIT execute them.*

This chapter is reference material. Most Zen programmers never write bytecode
by hand, but understanding it helps with debugging, performance tuning, and
understanding what `zen check` / `zen build` produce.

## 17.1 What bytecode is

After the lexer and parser turn source into an AST, the compiler lowers the
AST to a linear sequence of bytecode instructions. Each instruction has an
opcode and zero or more operands (register indices, constant pool indexes,
jump targets, etc.).

The interpreter (`zen-interp`) executes these instructions directly with a
value stack. The JIT (`zen-jit`) compiles frequently executed bytecode
sequences to native machine code via Cranelift.

## 17.2 Instruction format (conceptual)

```
<offset> <opcode> <operand1> <operand2> ...
```

Operands may reference:
- the constant pool (strings, numbers)
- local variables (by index)
- the value stack (implicit)
- jump targets (by offset)

Example (illustrative opcode names; exact mnemonics are build-defined):

```
000 LOAD_CONST     5        # push constant 42
002 STORE_LOCAL    0         # pop into local x
004 LOAD_LOCAL     0         # push x
006 LOAD_CONST     6         # push constant 1
008 ADD                    # pop two, push sum
010 RETURN                   # return top of stack
```

## 17.3 Opcode categories

### Load instructions

| Purpose | Example (conceptual) |
|---------|----------------------|
| load a constant | `LOAD_CONST <pool_index>` |
| load a local variable | `LOAD_LOCAL <index>` |
| load a global/module variable | `LOAD_GLOBAL <name>` |
| load a module member | `LOAD_MEMBER <module> <name>` |
| load `nil` / `true` / `false` | `LOAD_NIL`, `LOAD_TRUE`, `LOAD_FALSE` |

### Store instructions

| Purpose | Example |
|---------|---------|
| store to local | `STORE_LOCAL <index>` |
| store to global | `STORE_GLOBAL <name>` |
| store object field | `STORE_FIELD <name>` |
| store list element | `STORE_INDEX` |
| store dict entry | `STORE_DICT` |

### Call and return

| Purpose | Example |
|---------|---------|
| call a function | `CALL <arg_count>` |
| call a method | `CALL_METHOD <name> <arg_count>` |
| call a builtin | `CALL_BUILTIN <builtin_id>` |
| return a value | `RETURN` |
| return nil | `RETURN_NIL` |

### Branching

| Purpose | Example |
|---------|---------|
| unconditional jump | `JUMP <target>` |
| jump if truthy | `JUMP_IF_TRUE <target>` |
| jump if falsy | `JUMP_IF_FALSE <target>` |
| jump if nil | `JUMP_IF_NIL <target>` |

### Comparison

| Purpose | Example |
|---------|---------|
| equal | `EQ` (pops 2, pushes bool) |
| not equal | `NE` |
| strict equal | `SEQ` |
| less / greater / le / ge | `LT`, `GT`, `LE`, `GE` |
| identity | `IS` |
| membership | `IN` |

### Arithmetic

| Purpose | Example |
|---------|---------|
| add | `ADD` |
| subtract | `SUB` |
| multiply | `MUL` |
| divide | `DIV` |
| floor divide | `FLOORDIV` |
| modulo | `MOD` |
| power | `POW` |
| negate | `NEG` |
| bitwise and/or/xor/not | `BAND`, `BOR`, `BXOR`, `BNOT` |
| shift left/right | `LSHIFT`, `RSHIFT` |

### Container operations

| Purpose | Example |
|---------|---------|
| build list | `BUILD_LIST <count>` |
| build dict | `BUILD_DICT <count>` |
| build set | `BUILD_SET <count>` |
| build tuple | `BUILD_TUPLE <count>` |
| list append | `LIST_APPEND` |
| dict set | `DICT_SET` |
| subscript (index) | `BINARY_SUBSCR` |
| slice | `SLICE` |

### Exception handling

| Purpose | Example |
|---------|---------|
| begin try | `SETUP_FINALLY / SETUP_EXCEPT <handler>` |
| end finally | `END_FINALLY` |
| throw | `RAISE` |
| re-raise | `RERAISE` |

## 17.4 Stack effect

Most instructions follow a stack discipline: they pop operands off the value
stack and push results. For example:

- `ADD`: pops `b`, pops `a`, pushes `a + b`.
- `LOAD_CONST c`: pushes `c`.
- `STORE_LOCAL i`: pops value, stores in local `i`.
- `CALL n`: pops `n` arguments + the callable, pushes the return value.

The interpreter tracks stack depth to catch underflow/overflow bugs.

## 17.5 Constant pool

Strings, integers, floats, and other literals referenced by bytecode are stored
in a per-function constant pool. `LOAD_CONST <index>` fetches entry `<index>`.
This avoids emitting the full literal on every use.

## 17.6 How the interpreter executes

1. Fetch the instruction at the current program counter.
2. Decode opcode and operands.
3. Execute (manipulate the value stack, locals, globals).
4. Advance the PC (or jump on branch instructions).
5. Repeat until `RETURN` or an error.

Errors (Z03xx) are raised by individual ops when types are wrong, indexes are
out of range, etc.

## 17.7 How the JIT compiles

For hot functions, `zen-jit`:
1. Reads the bytecode.
2. Emits Cranelift IR for each instruction, mapping stack ops to virtual
   registers.
3. Inserts **inline cache guards** at call sites (check the cached type;
   if it matches, use the fast path; otherwise bail to a slow path / deopt).
4. Compiles to native machine code.
5. On type-guard failure, **deoptimizes**: reconstruct interpreter state and
   continue in the interpreter.

## 17.8 Inline caches (conceptual)

A call site `obj.method()` that's executed many times with `obj` of the same
type caches:
- the resolved method
- the type guard

On the next call, if the type still matches, it jumps directly to the cached
method. This avoids repeated method-lookup work.

## 17.9 Reading bytecode (if your build exposes it)

Some builds let you dump bytecode for inspection:

```bash
# conceptual; exact flag depends on build
zen check --dump-bytecode app.z
```

This is useful for verifying that a hot loop compiled as expected and that no
surprise type guards are present.

## 17.10 Exercises

1. Sketch the bytecode for `let x = 1 + 2; return x`.
2. What does `ADD` pop and push?
3. Why does the constant pool exist?
4. Describe how a branch instruction changes the PC.
5. Explain what an inline cache guards on.
6. What happens on deoptimization?
7. Contrast the interpreter loop with JIT-compiled native code.
8. Name three container bytecode ops.
9. Name three exception bytecode ops.
10. Why might `zen check --dump-bytecode` help optimize a hot loop?

'''
print('bytecode done')
