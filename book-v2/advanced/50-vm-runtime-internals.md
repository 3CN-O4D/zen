# Chapter 50: Zen VM and Runtime Internals

*How Zen 3.0 actually executes your code: the interpreter, the Cranelift JIT,
garbage collection, memory layout, and the FFI boundary.*

This chapter is for readers who want to look under the hood. It documents the
runtime architecture, value representation, execution models, and the
interaction between interpreted and JIT-compiled code.

## 15.1 Runtime architecture overview

Zen 3.0 has a multi-tier execution pipeline:

```
source (.z) ──► lexer ──► parser ──► AST ──► bytecode ──┬─► interpreter (zen-interp)
                                                        └─► Cranelift JIT (zen-jit)
                                                          ┬─► OSR / deopt
                                                          └─► native machine code
```

- **zen-syntax** — lexer + parser; produces the AST.
- **zen-vm** — the virtual machine, bytecode interpreter, ops, builtins, FFI.
- **zen-interp** — the tree-walking / bytecode interpreter used by `zen run`.
- **zen-jit** — the Cranelift-based JIT compiler with inline caches, OSR, and
  deoptimization.
- **zen-cli** — the `zen` command-line front end.

A source file is lexed, parsed into an AST, lowered to bytecode, and then either
interpreted directly or JIT-compiled to native code (Cranelift) for hot
functions.

## 15.2 Value representation

Every Zen value at runtime is a tagged `Value`. The tag distinguishes the type
and the payload holds the data (or a pointer to heap data).

### Primitive (immediate) values

| Tag | Kind | Storage |
|-----|------|---------|
| nil | `nil`/`null` | none |
| bool | boolean | 1 bit |
| int | signed integer | i64 (i48 guaranteed range for literals) |
| float | IEEE-754 double | f64 |
| string | pointer to heap string | `String` handle |

Immediate values (nil, bool, int, float) are copied by value. Strings and all
containers are heap-allocated handles (reference-counted / GC-managed).

### Heap (container) values

| Kind | Zen type | Heap structure |
|------|----------|----------------|
| list | `list` | growable array of `Value` |
| dict | `dict` | hash map string → `Value` |
| set | `set` | hash set of `Value` |
| tuple | `tuple` | fixed array of `Value` |
| bytes | `bytes` | immutable byte buffer |
| instance | class instance | field map + class pointer |
| func | function | closure + environment |
| module | module | member table |
| foreign | FFI handle | opaque pointer + finalizer |

### `shared` values

`shared x = expr` wraps the value in an `Arc<Mutex<_>>`-like container so it can
be safely read and written from multiple threads. Every access acquires the
internal lock.

## 15.3 Memory model

Zen is garbage collected (the exact collector is build-configured). Heap
objects are freed when unreachable.

### Reference cycles

Cycles (e.g., object A references B and B references A) can leak unless the
collector handles cycles or you break them explicitly.

```zen
class Node {
    var peer = nil
}
let a = new Node()
let b = new Node()
a.peer = b
b.peer = a   # cycle: a <-> b
```

To break: `a.peer = nil` when tearing down.

### `let` vs `var` vs `const`

- `let` — the binding cannot be reassigned, but the referenced heap object can
  still be mutated (e.g., `lst.push` on a `let` list).
- `var` — the binding can be reassigned.
- `const` — compile-time/module-level constant.

```zen
let lst = [1, 2]
lst.push(3)     # OK: mutating the object
# lst = [4]     # error: cannot rebind a let
```

## 15.4 Execution models

### Interpreter (tree-walk / bytecode)

`zen-interp` evaluates the AST or bytecode directly. Each operation dispatches
on the operator and operand types at runtime. This is the default for `zen run`
in many builds and is easiest to debug.

### Cranelift JIT

`zen-jit` compiles hot functions to native machine code using the Cranelift
code generator (the same backend used by Wasmtime).

Key JIT features:
- **Monomorphic inline caches** — cache the type of a call site so repeated
  calls with the same types skip type dispatch.
- **OSR (on-stack replacement)** — replace a running interpreted frame with a
  JIT-compiled frame mid-execution (and back again on deopt).
- **Deoptimization** — if a guarded assumption (e.g., a type) is violated,
  fall back to the interpreter safely.

### When is JIT used?

Hot loops and frequently called functions are JIT-compiled after a threshold of
invocations. Cold code stays interpreted. You typically don't control this
directly; `ZEN_DEBUG=1` may show JIT decisions in some builds.

## 15.5 Bytecode (conceptual)

Zen source is lowered to a stack/register-based bytecode. Each instruction has
an opcode and operands. Common opcode categories:

- **Load**: load constant, local, global, module member
- **Store**: store to local/global
- **Call**: invoke a function (native, closure, builtin, method)
- **Return**: return a value
- **Branch**: jump, conditional jump
- **Compare**: `==`, `<`, `>`, `is`, `in`, etc.
- **Math**: `+`, `-`, `*`, `/`, `%`, `**`, bitwise
- **Container**: list/dict/set/tuple construction, index, slice
- **Exception**: throw, catch, finally setup

Example (illustrative, not exact opcode names):
```
000 LOAD_CONST 42
002 STORE_LOCAL x
004 LOAD_LOCAL x
006 LOAD_CONST 1
008 ADD
010 RETURN
```

The bytecode is what the interpreter executes and what the JIT compiles.

## 15.6 The call stack and frames

Each function call creates a **frame** containing:
- the instruction pointer / program counter
- local variables
- the value stack (operand stack)
- a reference to the enclosing scope (for closures)
- the function being executed

Recursion and deep call chains grow the native stack; very deep recursion can
overflow it (Z0310 `MaxRecursionDepth`, Z0317 `StackOverflow`).

## 15.7 Closures and upvalues

A closure captures variables from its enclosing scope. Captured variables become
**upvalues** — heap cells shared between the closure and the scope that created
it. Mutating an upvalue from inside the closure is visible outside, and vice
versa.

```zen
func make_counter() {
    var n = 0     # n becomes an upvalue captured by the closure
    return fn() { n += 1; return n }
}
```

## 15.8 Method dispatch

Method calls (`obj.method(args)`) resolve the method through the object's class
chain:
1. Look for the method on the instance's class.
2. If not found, look on parent classes (inheritance).
3. Bind `self` to the instance and invoke.

If the instance has a field holding a callable with the same name, the field
may shadow the method depending on build.

## 15.9 FFI boundary (Zen ↔ C)

Zen can call C functions in shared libraries and pass data across the boundary.

### Marshalling (typical)

| Zen type | C side | Direction |
|----------|--------|-----------|
| int | `int32_t`/`int64_t` | by value |
| float | `double` | by value |
| string | `const char*` | NUL-terminated; Zen owns lifetime |
| bytes | `unsigned char*` | raw buffer |
| nil | `NULL` | |
| handle | `void*` | opaque pointer |

### Safety rules

1. Know who owns each allocation (Zen or C) and who frees it.
2. Never dereference a NULL handle.
3. Match calling conventions (cdecl/stdcall per platform).
4. Don't let a Zen exception unwind through C frames — catch at the Zen edge.
5. Guard cross-thread FFI calls with a Mutex if the C library isn't thread-safe.

### Debugging FFI

- `nm -D libfoo.so` / `objdump -T libfoo.so` list exported symbols.
- Start with a trivial function (add two ints) before structs/pointers.
- Log every FFI call and return.

## 15.10 pymport boundary (Zen ↔ Python)

`pymport` lets Zen import Python modules and call Python functions. Values are
marshalled both directions:

| Zen | Python | Zen | Python |
|-----|--------|-----|--------|
| int | int | list | list |
| float | float | dict | dict |
| string | str | nil | None |
| bool | bool | bytes | bytes |

Python exceptions are caught and re-surfaced in Zen as **chained tracebacks**
(showing both Zen and Python frames), error code Z0508 `PythonError`.

## 15.11 Garbage collection tuning (conceptual)

If your build exposes GC controls, common knobs are:
- heap growth factor
- collection threshold (bytes allocated since last collection)
- cycle collector enable/disable
- write barrier on/off

Most users never touch these; the defaults are chosen for general workloads.

## 15.12 Debugging the runtime

```bash
ZEN_DEBUG=1 zen run app.z          # verbose runtime logging (if supported)
zen explain Z0310                  # explain MaxRecursionDepth
zen explain Z0317                  # explain StackOverflow
```

## 15.13 Exercises

1. Draw the execution pipeline from `.z` source to native code.
2. Explain why `lst.push(3)` works on a `let` binding but `lst = [4]` does not.
3. Sketch a reference cycle and show how to break it.
4. Describe what an inline cache stores and why it speeds up repeated calls.
5. Explain OSR: when does the interpreter switch to JIT code, and back?
6. What information does a stack frame hold?
7. How does a closure capture a variable (upvalue)?
8. List the FFI marshalling rules for string, int, float, handle.
9. What happens when Python raises inside a pymport call?
10. Why might deep recursion overflow the stack, and how do you avoid it?

'''
print('vm internals done')

## 15.12 The 16-tag NaN-boxed `Value`

Every Zen runtime value is a single 64-bit word. The representation is a
NaN-box: IEEE-754 doubles reserve the exponent field `0x7FF` (all 1s) for
infinities and NaNs, and a Quiet NaN leaves 51 payload bits unused by hardware.
Zen takes those 51 bits and splits them into:

- **3 tag bits** (positions 48–50), selecting one of 8 machine tags.
- **48-bit payload** (positions 0–47).

```
 63   62 ............ 52 51  50 49 48 47 .................................... 0
+-----+---------------+---+---+---+-----------------------------------------+
| S   | Exponent =111 | 1 | T | A | tag (3 bits) | payload (48 bits)          |
+-----+---------------+---+---+---+-----------------------------------------+
                                              QNAN base = 0x7FF8_0000_0000_0000
```

Why a 48-bit payload rather than 44? Because x86-64 5-level paging enables
57-bit userspace addresses, and the Linux kernel warns that JITs encoding
information in high pointer bits collide with valid 57-bit allocations and
crash. A 48-bit payload is safe on all current and future address spaces.

### Machine-tag map

| Tag | Machine type | Language type(s) | Notes |
|-----|--------------|------------------|-------|
| `0b000` | Nil | `nil` / `null` | no payload |
| `0b001` | False | `false` | no payload |
| `0b010` | True | `true` | no payload |
| `0b011` | Int | `int` | i48, sign-extended; overflow promotes to `float` |
| `0b100` | Float | `float` | raw f64 bit pattern; NaNs normalised |
| `0b101` | String | `string` | thin pointer, no `ObjHeader` |
| `0b110` | Bytes | `bytes` | thin pointer, no `ObjHeader` |
| `0b111` | Heap | list, dict, set, tuple, func, instance, error, channel, mutex, handle | pointer to `ObjHeader`, kind discriminates |

`nil`/`false`/`true`/int/float are **immediates**: they fit inside the `Value`
and are copied by value. Strings and bytes are thin pointers (no header).
Everything else is a heap object with a 16-byte `ObjHeader` at offset 0.

### Integer semantics (i48)

- Range: `[-2^47, 2^47)` ≈ `±140,737,488,355,327` (~140 trillion).
- Literals beyond i48 are a lexer error (Z0106); build big values at runtime.
- Arithmetic overflow **promotes to float** (Lua 5.4 style); it never wraps.
- `2^47 + 1` → `140737488355328.0`.
- `int / int` → float (`10 / 4 == 2.5`). Use `//` for floor division.

### Float semantics

- Raw f64 when the exponent is not all-1s.
- All NaN bit patterns normalised to the canonical QNAN (tag `Float`, payload 0).
- `-0.0` is preserved (distinct bit pattern from `+0.0`).
- `typeof(0.0) == "float"`, never `"nil"`.

### Heap object layout (`ObjHeader`)

Every heap object (tag `0b111`) begins with a 16-byte, 8-aligned header:

| Offset | Size | Field | Meaning |
|--------|------|-------|---------|
| +0 | 1 | `kind` | `ObjKind` discriminant (List=0 … Bytes=11) |
| +1 | 1 | `marks` | M9 GC mark bits; 0 in M2–M8 |
| +2 | 2 | `flags` | object flags (closed, frozen, has-traceback, …) |
| +4 | 4 | `shape` | shape id for shaped objects; 0 = unshaped |
| +8 | 4 | `rc` | manual strong reference count |
| +12 | 4 | `size` | total allocation bytes, header included |
| +16 | … | payload | kind-specific data |

`string` and `bytes` are the exception: they have **no** `ObjHeader`. Their
length and refcount live behind the thin pointer (at `ptr − 8` for length,
`ptr − 24` for refcount in the `Arc` layout). This makes `is_string()` a pure
tag compare with zero loads, while `is_dict()` costs one header load.

Heap-kind discriminants (`ObjHeader.kind`):

| Kind | Value | Zen type |
|------|-------|----------|
| List | 0 | list |
| Dict | 1 | dict |
| Set | 2 | set |
| Tuple | 3 | tuple |
| Func | 4 | func/closure |
| Instance | 5 | instance |
| Error | 6 | error |
| Channel | 7 | channel |
| Mutex | 8 | mutex |
| Handle | 9 | handle |
| String | 10 | string |
| Bytes | 11 | bytes |

### `shared` values

`shared x = expr` wraps the value so it can cross threads safely. Internally it
is an `Arc<Mutex<_>>`-like handle: every read/write acquires the internal lock,
and the wrapper is freed when the last shared reference drops.

### Why `Value` is `Copy` — and what that costs

`Value` is 8 bytes and must be `Copy` (the register file is `Value[]`; inline
caches read `ic[site]`). `Copy` means **no destructor**, so writing a new heap
pointer into a register does **not** release the old value. The VM must go
through an explicit store that `release`s the old heap reference:

```rust
// conceptual
pub fn store(&mut self, a: u16, v: Value) {
    let old = self.registers[a as usize];
    if old is heap/string/bytes { release(old) }
    self.registers[a as usize] = v;
}
```

Getting this wrong is a leak (if you forget to release) or a double-free/use
after free (if you release twice). Every container `Drop` and every
`SET_FIELD`/`SET_INDEX` arm goes through this store.

### `let` vs `var` vs `const` at runtime

- `let` — the binding cannot be reassigned, but a referenced heap object can
  still be mutated (`lst.push(3)` on a `let` list is fine; `lst = [4]` errors).
- `var` — the binding can be reassigned.
- `const` — compile-time/module-level constant.

### Reference cycles

Heap objects are reference counted. A cycle (A → B → A) never reaches rc 0 and
leaks. Zen accepts bounded cycle leaks (rare in scripts) rather than paying for
cycle collection before M9. Break cycles manually by nilling a back-reference.

## 15.13 Execution models — interpreter vs JIT

### Interpreter (`zen-interp`)

Evaluates the register bytecode directly. Each instruction dispatches on its
opcode, reads/writes registers, and manipulates the value stack (the register
file is the value stack). One dispatch load + one indirect jump per op.

### Cranelift JIT (`zen-jit`, M11, feature-gated)

Compiles whole `FunctionProto`s to native machine code via Cranelift. Key
design points:

- **One version per `FunctionProto`, ever.** Code is immutable; new versions
  replace an `AtomicPtr` slot. No in-place patching (Cranelift 0.136 removed
  `hotswap`).
- **Tiering:** interpreter → IC-annotated interpreter → compiled. Compile after
  `TIER_UP = 1000` invocations or when one IC site reaches 8 misses.
- **Deoptimization is one-way and permanent:** a compiled function that fails
  a guard falls back to the interpreter and its compiled code is discarded.
- **Compilation runs on a dedicated thread**, never on the mutator/dispatch
  thread, so a compile never pauses a running program.

The interpreter is always correct; the JIT is an optional performance
accelerator guarded by `deopt → interpreter` exits.

### Call stack and frames

Each call creates a frame holding: program counter, local registers, the
register window for arguments/results, a reference to the enclosing scope (for
closures), and the function proto. Deep recursion can overflow the native
stack (Z0310/Z0317); convert deep recursion to iteration.

## 15.14 Closures and upvalues

A closure captures variables from its enclosing scope. Captured variables become
**upvalues** — heap cells shared between the closure and its creating scope.
Mutating an upvalue from inside the closure is visible outside, and vice versa.

```zen
func make_counter(start = 0) {
    var n = start
    return fn() { n += 1; return n }
}
```

`OPEN_UPVALUE` pins a local register so a later `MAKE_CLOSURE` can reference it
in place; `CLOSE_UPVALUE` boxes the values at scope exit.

## 15.15 Method dispatch

`obj.method(args)` resolves through the instance's class chain:
1. Look for the method on the instance's class.
2. If absent, walk parent classes (single inheritance).
3. Bind `self` to the instance and invoke.

`GET_FIELD`/`SET_FIELD` go through inline caches keyed by shape id: a load, a
compare against the cached shape id, and a `#[cold]` miss path that re-looks-up
and patches the IC entry (data, not code).

## 15.16 FFI boundary (Zen ↔ C)

Zen calls C functions in shared libraries (`.so`, `.dll`, `.dylib`).

Typical Zen→C type mapping:

| Zen type | C side | Notes |
|----------|--------|-------|
| int | `int32_t` / `int64_t` | by value |
| float | `double` | by value |
| string | `const char*` | NUL-terminated; Zen owns lifetime |
| bytes | `unsigned char*` | raw buffer |
| nil | `NULL` | |
| handle | `u32` index | process-global handle table, no header |

Safety rules:
1. Know who owns each allocation (Zen or C) and who frees it.
2. Never dereference a NULL handle.
3. Match calling conventions (cdecl/stdcall per platform).
4. Never let a Zen exception unwind through C frames — catch at the Zen edge.
5. Guard cross-thread FFI calls with a Mutex if the C library isn't thread-safe.

Debug: `nm -D libfoo.so` / `objdump -T libfoo.so` list exported symbols.

## 15.17 pymport boundary (Zen ↔ Python)

`pymport` imports Python modules (stdlib, venv, project-local `.py`) and calls
Python functions, marshalling values both directions:

| Zen | Python | Zen | Python |
|-----|--------|-----|--------|
| int | int | list | list |
| float | float | dict | dict |
| string | str | nil | None |
| bool | bool | bytes | bytes |

Python exceptions surface in Zen as **chained tracebacks** (both Zen and
Python frames), error code Z0508 `PythonError`. Don't hold the Python GIL
across long Zen operations.

## 15.18 Exercises

1. Draw the NaN-box `Value` layout and mark the tag/payload bit positions.
2. Why is `ObjHeader.marks` a byte in the object and not a bit in the `Value`?
3. What happens when an `int` add overflows i48?
4. Why does `d2 = d` (a 100,000-entry dict) cost only one 8-byte move?
5. Sketch a reference cycle and show how to break it manually.
6. What does `store()` do that a bare `registers[a] = v` would not?
7. Describe the three JIT tiers and what moves a function between them.
8. Why is JIT deoptimization one-way and permanent?
9. List the Zen→C type mappings for string, int, float, handle.
10. How does `pymport` surface a Python exception in Zen?

