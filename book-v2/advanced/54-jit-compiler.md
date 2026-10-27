# Chapter 54: The Cranelift JIT Compiler

*How Zen 3.0 compiles hot functions to native machine code, inline caches,
on-stack replacement, and deoptimization.*

## 19.1 Overview

Zen 3.0 includes a just-in-time (JIT) compiler built on **Cranelift**, the same
code generator used by Wasmtime and other projects. The JIT compiles
frequently executed Zen functions to native machine code at runtime, while
cold code stays interpreted.

Pipeline:
```
bytecode ──► Cranelift IR ──► native machine code ──► execute
```

## 19.2 Interpreter vs JIT

| Aspect | Interpreter (`zen-interp`) | JIT (`zen-jit`) |
|--------|---------------------------|-----------------|
| Startup | fast (no compile) | slower (compile cost) |
| Steady-state speed | slower (per-op dispatch) | faster (native code) |
| Best for | cold / rarely-run code | hot loops / frequent calls |
| Debugging | easier (direct mapping) | harder (native frames) |

Zen automatically decides: run in the interpreter first; after a function
crosses an invocation threshold, JIT-compile it.

## 19.3 Compilation process

1. **Bytecode → Cranelift IR**: each bytecode instruction maps to one or more
   Cranelift intermediate-representation operations. Stack-based bytecode ops
   are converted to virtual-register IR.
2. **Type guards**: at call sites and type-sensitive ops, insert guards that
   check the operand types match expectations.
3. **Compile**: Cranelift lowers IR to target machine code (x86-64, AArch64,
   etc.).
4. **Install**: patch the call site to jump to the native code.

## 19.4 Inline caches

At a polymorphic call site (e.g., `obj.method()`), the types of `obj` may vary
across calls. The JIT installs an **inline cache**:
- On the first call, resolve the method normally (slow path) and cache the
  result + the observed type.
- On subsequent calls, check the type guard. If the type matches the cache,
  jump directly to the cached method (fast path).
- If the type differs, fall back to the slow path and possibly expand the
  cache (polymorphic inline cache) or deopt.

This makes repeated monomorphic calls nearly as fast as statically-typed code.

## 19.5 On-stack replacement (OSR)

OSR lets the VM switch between interpreted and JIT-compiled code **mid-execution**.

- **Interpreter → JIT**: a long-running loop that's interpreted gets compiled
  on the fly; the running frame is replaced with a JIT frame at a loop back
  edge, continuing from the same point.
- **JIT → interpreter (deopt)**: if a JIT assumption fails (see below), the VM
  reconstructs interpreter state from the native frame and resumes
  interpretation.

OSR means long loops benefit from JIT without restarting the function.

## 19.6 Deoptimization

JIT code is compiled under **assumptions** (guards), e.g.:
- a variable always holds an int at this point
- a call site always sees the same type

If a guard fails at runtime (the assumption is violated), the VM **deopts**:
1. Capture the current native machine state (registers, stack).
2. Reconstruct the equivalent interpreter frame (values, locals, PC).
3. Transfer control to the interpreter at the corresponding bytecode point.
4. Continue interpreting (possibly re-JIT later if it gets hot again).

Deopt guarantees correctness even when speculation was wrong.

## 19.7 When does JIT help?

JIT wins when:
- a function is called many times
- a loop iterates many times
- types at call sites are stable (monomorphic)

JIT does **not** help (and may hurt) when:
- code runs once (compile cost outweighs benefit)
- types change constantly (guards always fail → deopt thrashing)

## 19.8 Observing JIT behavior

Some builds support debug logging:

```bash
ZEN_DEBUG=1 zen run app.z   # may print JIT/compile/deopt events (build-dependent)
```

There's typically no user-facing flag to force or disable JIT; it's automatic.
`zen explain` covers runtime errors (Z03xx), not JIT internals.

## 19.9 JIT and FFI / pymport

JIT-compiled code that calls out to C (FFI) or Python (pymport) uses the same
marshalling as interpreted code. The JIT emits the call sequence; the runtime
handles crossing the boundary. A deopt during an FFI/pymport call must
reconstruct state at the boundary safely.

## 19.10 Exercises

1. Contrast interpreter startup vs steady-state speed with JIT.
2. What does an inline cache store at a call site?
3. When does OSR trigger (interpreter → JIT)?
4. What triggers deoptimization (JIT → interpreter)?
5. Name two situations where JIT helps and two where it doesn't.
6. Sketch the bytecode→IR→native pipeline.
7. Why must deopt reconstruct interpreter state?
8. How do type guards keep JIT code correct?
9. What does `ZEN_DEBUG=1` potentially show?
10. Does JIT change the semantics of your program? (No — only performance.)

'''
print('jit done')
