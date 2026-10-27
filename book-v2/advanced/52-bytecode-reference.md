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

## 17.12 Instruction encoding

Zen uses a **register machine** with fixed-width 32-bit instructions:

```
 Bit  31      24 23       16 15        8 7         0
      +----------+-----------+-----------+-----------+
      |    c     |     b     |     a     |    op     |
      byte 3      byte 2      byte 1      byte 0
```

- `op` (u8): opcode. 104 core opcodes + 6 gated superinstructions = 110.
- `a` (u8): destination register in nearly every opcode. 256 registers per
  function max (~250 usable).
- `b` (u8): first source — register index, constant-pool index, name id, or
  jump target, depending on `op`.
- `c` (u8): second source — same overload.

Jump targets are pc-relative, ±127 instructions; wider targets use `JMP_FAR`.
Constant indices are 8-bit (256 per function); wider use `LOAD_CONST_W`.

## 17.13 Full opcode table

### Category A — Constants and immediates

| ID | Name | a | b | c | Description |
|----|------|---|---|---|-------------|
| 0x00 | NOP | — | — | — | No-op; alignment padding / jump target. Never removed. |
| 0x01 | LOAD_CONST | dst | const idx | — | R[a] = K[b] (constant pool). |
| 0x02 | LOAD_NIL | dst | — | — | R[a] = nil. |
| 0x03 | LOAD_TRUE | dst | — | — | R[a] = true. |
| 0x04 | LOAD_FALSE | dst | — | — | R[a] = false. |
| 0x05 | LOAD_SMALL_INT | dst | signed imm (i8) | — | R[a] = i48(sext(b)); for 0, 1, -1, small n. |
| 0x06 | LOAD_DEFAULT | dst | param idx | — | R[a] = pre-evaluated default for param b. |

### Category B — Moves and conversions

| ID | Name | a | b | c | Description |
|----|------|---|---|---|-------------|
| 0x10 | MOVE | dst | src | — | R[a] = R[b]; consumes R[b] (source dead after). |
| 0x11 | DUP | dst | src | — | R[a] = R[b]; R[b] stays live. |
| 0x12 | CONV | dst | src | type id | R[a] = coerce(R[b], TypeId(c)); int/float/str/bytes/bool. |
| 0x13 | TO_BOOL | dst | src | — | R[a] = truthy(R[b]); canonical truthiness. |

### Category C — Arithmetic

| ID | Name | a | b | c | Description |
|----|------|---|---|---|-------------|
| 0x18 | ADD | dst | lhs | rhs | R[a] = R[b] + R[c]; int+int→int (overflow→float), str+str, list+list. |
| 0x19 | SUB | dst | lhs | rhs | R[a] = R[b] − R[c]. |
| 0x1A | MUL | dst | lhs | rhs | int×int, str×int (repeat), list×int. |
| 0x1B | DIV | dst | lhs | rhs | int/int→float (10/4==2.5); ÷0 raises ZeroDivisionError. |
| 0x1C | MOD | dst | lhs | rhs | R[b] % R[c], sign of dividend; ÷0 raises. |
| 0x1D | POW | dst | lhs | rhs | R[b] ** R[c]; overflow→OverflowError. |
| 0x1E | NEG | dst | src | — | R[a] = −R[b]. |
| 0x1F | BIT_AND | dst | lhs | rhs | int & int. |
| 0x20 | BIT_OR | dst | lhs | rhs | int \| int. |
| 0x21 | BIT_XOR | dst | lhs | rhs | int ^ int. |
| 0x22 | BIT_NOT | dst | src | — | ~R[b]. |
| 0x23 | SHL | dst | lhs | rhs | R[b] << R[c]; count ≥48 or negative → ValueError. |
| 0x24 | SHR | dst | lhs | rhs | arithmetic (sign-extending) right shift. |
| 0x25 | CONCAT | dst | lhs | rhs | string/bytes/list concat (compile-time-proven str side). |
| 0x26 | BUILD_STRING | dst | template idx | count | string interpolation from template K[b] + registers R[c..]. |

### Category D — Comparison

| ID | Name | a | b | c | Description |
|----|------|---|---|---|-------------|
| 0x28 | EQ | dst | lhs | rhs | R[b] == R[c]; total, never raises. |
| 0x29 | NE | dst | lhs | rhs | R[b] != R[c]; total, never raises. |
| 0x2A | LT | dst | lhs | rhs | R[b] < R[c]; ordered pairs only, else TypeError. |
| 0x2B | LE | dst | lhs | rhs | <= |
| 0x2C | GT | dst | lhs | rhs | > |
| 0x2D | GE | dst | lhs | rhs | >= |
| 0x2E | IS | dst | lhs | rhs | pointer compare for heap; value compare for scalars. |
| 0x2F | IDENTICAL | dst | lhs | rhs | deep_equal recursive over containers. |
| 0x30 | NOT | dst | src | — | R[a] = !truthy(R[b]). |
| 0x31 | IN | dst | needle | haystack | R[b] in R[c] (string/bytes/list/tuple/set/dict-key). |

### Category E — Jumps

| ID | Name | a | b | c | Description |
|----|------|---|---|---|-------------|
| 0x38 | JMP | — | rel (i8) | — | Unconditional jump, relative to next instruction. |
| 0x39 | JMP_BACK | — | rel (i8, negative) | — | Backward jump; helps branch predictors. |
| 0x3A | JMP_IF_FALSE | tested | rel | — | if !truthy(R[a]) { pc += b }. |
| 0x3B | JMP_IF_NIL | tested | rel | — | if R[a] is nil { pc += b }. (x?.y, x ?? y primitive) |
| 0x3C | JMP_IF_NOT_NIL | tested | rel | — | if R[a] is not nil { pc += b }. |
| 0x3D | JUMP_TABLE | scratch | const idx | default | idx=R[a]; if in table range jump K[b][idx] else default. |

### Category F — Fused compare-and-branch

| ID | Name | a | b | c | Description |
|----|------|---|---|---|-------------|
| 0x40 | JMP_IF_LT | lhs | rel | rhs | if R[a] < R[c] { pc += b }. |
| 0x41 | JMP_IF_LE | lhs | rel | rhs | <= |
| 0x42 | JMP_IF_GT | lhs | rel | rhs | > |
| 0x43 | JMP_IF_GE | lhs | rel | rhs | >= |

`b` sits between the two operand registers deliberately (hot regs low, cold
target separate).

### Category G — Calls and returns

| ID | Name | a | b | c | Description |
|----|------|---|---|---|-------------|
| 0x48 | CALL | dst | proto idx | argc | R[a] = call(proto[b], R[a−1−c .. a)); direct call. |
| 0x49 | CALL_KW | dst | proto idx | argc | + kwargs dict at R[a−2−c]. |
| 0x4A | CALL_INDIRECT | dst | — | argc | R[a] = call_value(R[a−1−c], R[a−c .. a)). |
| 0x4B | CALL_METHOD | dst | name id | argc | s.upper(); goes through M5 inline cache. |
| 0x4C | CALL_NATIVE | dst | native idx | argc | natives[b].call(vm, &R[a−c .. a]); no copy. |
| 0x4D | CALL_SUPER | dst | name id | argc | super_method(class_of(self), name, args); self = R[0]. |
| 0x4E | TAIL_CALL | dst | proto idx | argc | replaces current frame; return f(x) is O(1) stack. |
| 0x4F | TAIL_CALL_METHOD | dst | name id | argc | tail-call form of CALL_METHOD. |
| 0x50 | RETURN | value | — | — | Return R[a]. |
| 0x51 | RETURN_UNIT | — | — | — | Return nil (implicit returns are common). |
| 0x52 | MAKE_KWARGS | dst | count | first | kwargs object at a call site. |

### Category H — Containers and iteration

| ID | Name | a | b | c | Description |
|----|------|---|---|---|-------------|
| 0x58 | MAKE_LIST | dst | count | first | R[a] = [R[c..c+b−1]]. Preallocates capacity b. |
| 0x59 | MAKE_TUPLE | dst | count | first | Immutable, Arc-shared. |
| 0x5A | MAKE_SET | dst | count | first | Deduplicated on insert. |
| 0x5B | MAKE_DICT | dst | pair count | first | {R[c]:R[c+1], ...}; interns a shape if keys are string constants. |
| 0x5C | MAKE_RANGE | dst | argc (1..3) | first | Lazy range object, never a materialised list. |
| 0x5D | LIST_APPEND | list | value | — | Append to fresh list literal R[a]. |
| 0x5E | LIST_INSERT | list | index | value | Insert at index. |
| 0x5F | DICT_SET | dict | key | value | Insert or overwrite; records shape transition on IC site. |
| 0x60 | SET_ADD | set | value | — | Insert into set. |
| 0x61 | LEN | dst | src | — | R[a] = len(R[b]). |
| 0x62 | SLICE | dst | src | argc (0..3) | R[a] = R[b][R[a−c .. a]]. |
| 0x63 | UNPACK | dst | count | first | Destructuring; UnpackError on arity mismatch. |
| 0x64 | ITER_INIT | iter | seq | — | Hidden 2-slot iterator state at R[a], R[a+1]. |
| 0x65 | ITER_NEXT | iter | dst | — | Advance; R[b] = next item or nil when exhausted. |
| 0x66 | ITER_DONE | iter | dst | — | R[b] = iterator_exhausted(R[a]); not fused into ITER_NEXT. |

### Category I — Field and index access

| ID | Name | a | b | c | Description |
|----|------|---|---|---|-------------|
| 0x6C | GET_INDEX | dst | container | key | R[a] = R[b][R[c]]; prime inline-cache site. |
| 0x6D | SET_INDEX | container | key | value | R[a][R[b]] = R[c]; order: container, key, value. |
| 0x6E | GET_FIELD | dst | obj | name id | R[a] = R[b].K_names[c]; dot form; IC site. |
| 0x6F | SET_FIELD | obj | name id | value | R[a].K_names[b] = R[c]. |
| 0x70 | GET_INDEX_DEFAULT | dst | container | key | if present R[a] = R[b][c], else R[a] unchanged (pre-loaded default). |
| 0x71 | DELETE_INDEX | container | key | — | del R[a][R[b]]; KeyError/IndexError if absent. |
| 0x72 | HAS_FIELD | dst | obj | name id | R[a] = K_names[c] in R[b]. |
| 0x73 | TYPEOF | dst | src | — | R[a] = typeof_id(R[b]) as interned string id. |

### Category J — Closures and upvalues

| ID | Name | a | b | c | Description |
|----|------|---|---|---|-------------|
| 0x78 | MAKE_CLOSURE | dst | proto idx | upvalue count | R[a] = Closure{proto, upvalues from R[a+1..a+1+c]}. |
| 0x79 | GET_UPVAL | dst | upvalue idx | — | R[a] = upvalue[b].load() (open or closed cell). |
| 0x7A | SET_UPVAL | upvalue idx | value | — | upvalue[a].store(R[b]). |
| 0x7B | OPEN_UPVAL | local reg | upvalue slot | — | Pin a local register as an open upvalue. |
| 0x7C | CLOSE_UPVAL | — | first local | count | Box R[b..b+c−1] and repoint open upvalues. |

### Category K — Errors and exceptions

| ID | Name | a | b | c | Description |
|----|------|---|---|---|-------------|
| 0x90 | TRY_PUSH | — | handler pc | finally pc | Push handler entry covering [pc, try_end); c=0 → no finally. |
| 0x91 | TRY_POP | — | — | — | Pop innermost handler. |
| 0x92 | THROW | value | — | — | Raise R[a]; never returns. |
| 0x93 | RERAISE | — | — | — | Re-raise in-flight error after finally. |
| 0x94 | CATCH_MATCH | dst (bool) | class name id | — | R[a] = thrown matches class K_names[b]. |
| 0x95 | ERROR_NEW | dst | class name id | msg | R[a] = new_error(K_names[b], R[c]). |
| 0x96 | ERROR_TYPE | dst | error | — | R[a] = class_name_of(R[b]). |
| 0x97 | ERROR_MSG | dst | error | — | R[a] = message_of(R[b]). |

### Category L — JIT / patch-point interface

| ID | Name | a | b | c | Description |
|----|------|---|---|---|-------------|
| 0xA0 | TYPE_GUARD | src | type id | deopt id | JIT: if typeof(R[a]) != TypeId(b), deopt to landing pad c. |
| 0xA1 | DEOPT | deopt id | — | — | Interpreter: unreachable; JIT: landing-pad marker. |
| 0xA2 | SAFEPOINT | — | reason name id | — | Debugger/signal/hotswap pause point. |
| 0xA3 | IC_GET_FIELD | dst | IC site idx | name id | GET_FIELD with inline-cache handle at P.ic_sites[b]. |
| 0xA4 | IC_SET_FIELD | obj | IC site idx | value | SET_FIELD with IC site. |
| 0xA5 | IC_CALL | dst | IC site idx | argc | CALL_INDIRECT with IC on callee identity. |
| 0xA6 | PATCH_POINT | — | IC site idx | kind | No-op; marks a site the M5/M11 patcher may rewrite. |
| 0xA7 | IC_BUMP | — | IC site idx | — | Site exceeded monomorphic budget (patcher-emitted). |

### Category M — Modules and globals

| ID | Name | a | b | c | Description |
|----|------|---|---|---|-------------|
| 0xB0 | IMPORT | dst | module name id | flags | R[a] = import(K_names[b]); c bit0 = `from m import *`. |
| 0xB1 | IMPORT_FROM | dst | module name id | symbol name id | R[a] = import(K_names[b]).K_names[c]. |
| 0xB2 | DEFINE_GLOBAL | value | name id | flags | globals()[K_names[b]] = R[a]; c bit0 = const. |
| 0xB3 | BIND_NAME | value | name id | flags | Bind in current scope; c bits0–1 = let/var/const, bit2 = export. |

### Reserved IDs (never used in M4)

| ID | Reserved for | Milestone |
|----|--------------|-----------|
| 0x07 | LOAD_CONST_W (16-bit const index) | M9 |
| 0x08 | LOAD_LOCAL_FRAME (per-frame spill array) | M9 |
| 0x09 | LOAD_UPVALUE_FRAME | M9 |
| 0x0A | YIELD_FROM (coroutine) | M10 |
| 0x0B | SPAWN (scheduler intercept) | M10 |

## 17.14 Stack effect and register window

Zen's calling convention is a **register window with no operand stack**:

| Call form | Meaning |
|-----------|---------|
| `CALL a, proto, argc` | callee = R[a−1−c]; args = R[a−c .. a); result → R[a]. |
| `CALL_METHOD a, name, argc` | receiver = R[a−1−c]; name = K_names[b]; args = R[a−c .. a); result → R[a]. |
| `TAIL_CALL a, proto, argc` | same window; current frame replaced (O(1) stack depth). |

Arguments are already in the right place: a call is a bounds check, a memcpy
of `c × 8` bytes into the callee's register window, and `base += window_size`.
No marshalling loop, no shuffling.

## 17.15 Constant pool, local-slot table, upvalue table

Each function carries:
- a flat `Vec<Instruction>` of fixed-width 32-bit words,
- a per-function **constant pool** (deduplicated immutable values),
- a **local-slot table** (binding name → register index),
- an **upvalue table** (what a closure captures and how),
- a **line table** (diagnostics, REPL, differential harness),
- an **exception table** (try/catch/finally),
- a table of **inline-cache sites** (so the M5 patcher and M11 translator
  find them without re-analysis).

One bytecode, two consumers: the M4 VM interprets it directly; the M11
Cranelift translator walks the same `FunctionProto`. Any disagreement is a
translator bug, not a divergent lowering.

## 17.16 Exercises

1. Sketch the 32-bit instruction layout and name each field.
2. What is the max number of registers per function, and what happens when it
   is exceeded?
3. How does `JMP_IF_NIL` implement `x?.y` and `x ?? y`?
4. What is the difference between `MOVE` and `DUP`?
5. Why is `CONCAT` a separate opcode from `ADD`?
6. What does `MAKE_RANGE` produce — a list or a lazy object?
7. How does `GET_INDEX_DEFAULT` implement `d.get(k, 0)` in two instructions?
8. What does `TRY_PUSH` / `TRY_POP` / `THROW` implement?
9. What is the `CodeSlot` / `TYPE_GUARD` / `DEOPT` mechanism for?
10. Why must compiled code `retain` a value loaded from an object?

