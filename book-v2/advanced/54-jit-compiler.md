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

## 19.12 The Cranelift JIT — strategy, gate, and design

Zen 3.0 includes an optional, feature-gated JIT built on **Cranelift 0.136**
(the same code generator as Wasmtime). It is **M11, the last milestone**,
off by default, and may not block a release. The interpreter is always correct;
the JIT is an optional performance accelerator.

### Why M11 and not earlier

Three correctness dependencies force the ordering:

| Dependency | Reason |
|------------|--------|
| Shapes + inline caches (vm §6, §7) | A JIT without stable shape ids deopts on every field access. |
| `DEOPT`, `JMP_BACK`, `SAFEPOINT`, `TYPE_GUARD` (bytecode §4) | The deopt mechanism *is* these opcodes. |
| The M9 profile (memory §10) | The JIT's value prop is "replace the dispatch loop"; M9's gate measures whether the loop is the bottleneck. |

So every benchmark up to M10 is an **interpreter benchmark**, and the JIT
must beat *that* interpreter, not an idealised one.

### What "optional" means operationally

| Commitment | How kept |
|------------|----------|
| JIT may not block a release | `jit` feature off by default; `cargo build/test/bench` work with it off. |
| JIT may be abandoned | Fallback is the interpreter, unchanged. No "JIT-compatible bytecode" tax. |
| JIT may not destabilise correctness | Every compiled path has a `deopt → interpreter` exit; a misprediction is a slowdown, never a wrong answer. |
| A wrong compilation must not be a security bug | Deopt re-checks every type guard; a stale path is bounded by "runs the interpreter". |

Cranelift's own README says **"This crate is extremely experimental."**
So the project uses only **four methods** from `cranelift-jit`:
`JITBuilder::new`/`symbol`/`memory`, `JITModule::new`/`define_function`/
`get_finalized_function`/`finalize_definitions`. Everything else
(frontend lowering, guards, deopt metadata, frame layout, safepoint protocol,
tiering policy) is Zen's own code — reimplementable against a future Cranelift
in an afternoon.

### Hot-swap problem — the ruling

Cranelift 0.136 removed `JITBuilder::hotswap` and
`prepare_for_function_redefine`, and rejects duplicate definitions. So **in-
place code patching is unavailable**. The ruling:

> **Every compilation produces a brand-new, immutable code region. Nothing is
> ever patched. A call site reads an `AtomicPtr` and calls through it. Old code
> is retained until VM teardown.**

```rust
pub struct CodeSlot {
    ptr:  AtomicPtr<u8>,   // entry point, or null (= tier 0 / interpreter)
    len:  AtomicUsize,     // lifetime accounting
    module: RwLock<Option<Arc<JitModule>>>,  // keeps the mapping alive
}
```

`publish` stores the `Arc<JitModule>` first (Release), then the pointer.
Reads are Acquire. Old versions coexist; both are semantically identical, so
running an old version is fine.

Compensation for lost hot-swap: the shape-transition IC entry is patched in
**data** (a `u32` in a table), not in code — no cache-coherency dance. One
version per `FunctionProto`, ever; recompile replaces the `CodeSlot`.

### Tiering: three tiers

```
tier 0: Interpreter          — always correct, ~2.4 ns/op [E]
   ↑ hit counter ≥ TIER_UP
tier 1: Interpreter + ICs    — M5, always on, IC hit = load+cmp+branch
   ↑ tier-1 hot count ≥ COMPILE
tier 2: Cranelift machine code — M11, optional, ~0.9 ns/op [E]
   ↺ deopt (one-way, permanent, back to tier 0)
```

| Tier | Exists in | Threshold | Deoptimizable? |
|------|-----------|-----------|----------------|
| 0 interpreter | M2+ | 0 | n/a |
| 1 IC-annotated interpreter | M5+ | 0 | n/a |
| 2 compiled | M11, `--features jit` | 1,000 invocations or 8 IC misses at one site | yes → tier 0 |

Deoptimization is **one-way and permanent**: a deopt'd function goes back to
tier 0 and its compiled code is discarded. It can be re-triggered and
recompiled; thrashing sets a "do not recompile" flag so the cost is one wasted
compile, not a loop. There is no tier 3 / re-optimisation — a deliberate scope
decision.

### Compilation happens off the dispatch path

Compilation runs on a **dedicated compiler thread**, never on the mutator or
inside the dispatch loop. The mutator only `push`es proto IDs to a queue; the
compiler thread pops and compiles. A compile failure is **not** a program error:
the proto is marked uncompilable and stays at tier 0 forever. This makes the
JIT safe to experiment with — a Cranelift regression degrades performance,
never correctness.

### Performance targets (per benchmark, falsifiable)

| Benchmark | Stresses | Target vs tier 1 |
|-----------|----------|------------------|
| `fib` (naive recursive, 27) | call overhead, ICs, int arithmetic | ≥ 2.5× |
| `loop_sum` (1e8 while) | dispatch removal on tight loop | ≥ 1.5× |
| `json_parse` | shape transitions, string ops, dict churn | ≥ 2.0× |
| `nbody` (float) | float arithmetic, while, sqrt | ≥ 2.0× |
| `list_ops` | CoW tests, rc traffic, bounds checks | ≥ 1.3× |
| `startup` (hello world) | nothing | ≥ 0.9× (may regress ≤10%) |

`startup` is in the list on purpose: Cranelift compile time (~0.3–2 ms per
small function) would dominate a 0.4 ms hello world. Regressing >10% stops the
feature from defaulting on.

## 19.13 Code emission — shape of a compiled function

### One Zen opcode → roughly one machine instruction

Generated code is straight-line with labels, not a switch. A compiled `fib`
shape (x86-64, illustrative):

```
zen_fn_0:                          ; fn fib(n) { if n < 2 { return n } ... }
    global_addr.  r2, [vm + n_off]
    global_load.  r1, r2           ; r1 = n (Value)
    ; mask off tag bits, sign-extend i48
    typecheck.    r1 == t_int      ; guard: tag == 0b011
    brif           deopt_0, r1, deopt_0
    icmp_imm.     r3, r1, 2        ; if n < 2 → return n
    brif           deopt_0, r3, ret_0
    call          fib_slot, (vm, n); indirect through CodeSlot
    ...
ret_0:
    return       r1
deopt_0:
    jmp          zen_deopt(vm, site_0, frame_map)
```

No dispatch load, no indirect jump per op. One `brif` per type assumption;
deopt is a tail-call to a shared stub (not duplicated per site).

### The i48 problem — overflow **promotes to `float`**

An `int` is a sign-extended i48 in a 64-bit register. `int` add is **not** a
plain `iadd`: it is `iadd`, an i48-range overflow test, and a `#[cold]`
promote-to-`float` block that changes the `Value`'s tag to `0b100`.

| Operation | Emitted | Note |
|-----------|---------|------|
| INT_ADD | iadd + overflow test + `brif` to promote block | fast path = iadd + jo/seto + brif |
| INT_MUL | same shape | `imul` must test i48 range, not 64-bit OF (`2^47 × 2^47` overflows 64 bits) |
| IDIV / IMOD | sdivrem on sign-extended i48s + explicit zero check | cannot trap; INT_MIN / -1 == +2^47, in range |
| INT_TO_FLOAT | fcvt_from_sint | |

Consequences a reviewer should check:
1. The promote path changes the tag, so downstream `TYPE_GUARD`s must be
   re-derived; a frame map recording a promoted register as `int` is a wrong
   answer after deopt.
2. `IMUL` needs an i48-range test (not 64-bit OF).
3. `INT_TO_FLOAT` after a promotion is a **type error**, not a no-op — the
   compiled code must re-check the tag before converting.

### Field-access lowering

`GET_FIELD` compiles to:
1. typecheck `obj` is an instance (tag == 0b111 && header.kind == INSTANCE),
2. load header shape, `cmp` against the cached shape id, `brif` to deopt on
   mismatch (this is the inlined IC — predicted-taken ~99.9% after warm-up),
3. `load [obj + 16 + 8*k]` — one `mov` with a displacement (what hot-swap
   would have patched; the compare is the accepted cost),
4. `retain` the loaded `Value` (eliding this is a use-after-free).

Why `cmp`+`brif` and not a trap: a trap crashes; §1.2 forbids a JIT that
crashes the process. A branch to a deopt stub is always correct.

## 19.14 Semantic parity checklist

| Concern | Interpreter | Compiled |
|---------|-------------|----------|
| int overflow | promotes to float | iadd + i48 test + promote block (tag 0b100) |
| int overflow in frame-mapped register | whatever bytecode put there | deopt frame map records type-agnostic, not int |
| idiv by zero | DivideByZero error, checked before op | same error construction, not a trap |
| INT_MIN / -1 | +2^47, in range | cannot trap (sign-extended i48, +2^47 fits i64) |
| imod sign | follows dividend (legacy fmod path) | same helper, not bare `srem` |
| INT_TO_FLOAT after promotion | type error | re-check tag before converting |
| float nan/inf | IEEE, no trap | identical; no finite-only fast path |
| GET_FIELD on non-instance / wrong shape | Zen error | deopt, then interpreter raises same error |
| set_field on shared object | cow_mut clones | `cmp rc, 1` inline; `#[cold]` cow_mut call otherwise |
| CHANNEL_RECV on closed channel | Zen error | builtin call → interpreter code runs |

Blocking operations (channel recv/send) are **calls, not inlined** — an
inlined spin loop would starve the scheduler and is a miscompile.

## 19.15 Exercises

1. Name the four `cranelift-jit` methods Zen uses and why only four.
2. Why can't the JIT patch code in place (hot-swap)?
3. What publishes a new compiled version, and with what memory ordering?
4. What moves a function from tier 0 to tier 2?
5. Why is deoptimization one-way and permanent?
6. What does a compiled `int` add emit, and what happens on overflow?
7. Why does compiled `GET_FIELD` end with a `retain`?
8. What is the startup benchmark target and why is it included?
9. Why must blocking ops (channel recv) be calls, not inlined sequences?
10. How does the JIT guarantee a wrong compilation is never a wrong answer?

