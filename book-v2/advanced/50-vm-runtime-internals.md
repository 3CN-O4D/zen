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
