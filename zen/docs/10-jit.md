# 10 — The JIT: strategy, code emission, and the M11 gate

> **Status:** Draft for review. The load-bearing ruling is §3: **Cranelift 0.136 has no hot-swap
> and no redefine API**, so the design is *immutable per-batch code behind an `AtomicPtr`* and
> never in-place patching. §4.1 is the API evidence, verified against docs.rs 0.136.1.
> **Milestone:** **M11, the last milestone.** The JIT is optional, feature-gated, and may not block
> a release. [ADR-0002](decisions/ADR-0002-memory-model.md)'s "the JIT work in M11 is worth more
> than cycle collection" is the priority statement; this document is what that priority buys.
> **Recorded as:** no ADR yet. This document is the input to `ADR-0005-jit-strategy.md`, which
> should be written when M9 opens, not before — see §12.4.
> **Depends on:** [`09-memory.md`](09-memory.md) §5.5 (the `StringObj` representation change is
> gated by the same feature flag, for the same reason, and they are one piece of work).

### How to read the numbers in this document

| Marker | Meaning |
|---|---|
| **[M]** | **Measured** by a command named in the same sentence. Reproducible today. |
| **[T]** | **Target.** A CI gate. Failing it blocks the M11 release, not the language release. |
| **[E]** | **Estimate**, from first principles or from a comparable project (LuaJIT, V8 TurboFan, Wasmtime's Cranelift use, CPython 3.13's specializing interpreter). |

**The only [M] numbers in this document are the API facts in §4.1**, which are read off
docs.rs 0.136.1. There are no [M] performance numbers, because the new tree has no compiler
emitter to profile. Everything about speed is a [T] or an [E], and the [T]s are stated so a
reviewer can reject them individually.

---

## Contents

1. [Purpose, and the M11 priority statement](#1-purpose-and-the-m11-priority-statement)
2. [Strategy: what to compile, and what not to](#2-strategy-what-to-compile-and-what-not-to)
3. [The hot-swap problem, and the ruling that resolves it](#3-the-hot-swap-problem-and-the-ruling-that-resolves-it)
4. [Cranelift API survey: what 0.136.1 actually offers](#4-cranelift-api-survey-what-01361-actually-offers)
5. [Tiering: when does code get compiled](#5-tiering-when-does-code-get-compiled)
6. [Code emission: the shape of a compiled Zen function](#6-code-emission-the-shape-of-a-compiled-zen-function)
7. [Deoptimization, safepoints, and the frame table](#7-deoptimization-safepoints-and-the-frame-table)
8. [Registers, ABI, and the entry point](#8-registers-abi-and-the-entry-point)
9. [Memory management for generated code](#9-memory-management-for-generated-code)
10. [The interpreter fallback and feature detection](#10-the-interpreter-fallback-and-feature-detection)
11. [Test plan](#11-test-plan)
12. [Metrics, the M11 gate, and reviewer rulings](#12-metrics-the-m11-gate-and-reviewer-rulings)

---

## 1. Purpose, and the M11 priority statement

### 1.1 Why M11 and not M4

The ordering is not a style preference. It is forced by three dependencies, each of which is a
*correctness* dependency rather than a performance one.

| Dependency | Why the JIT cannot come earlier |
|---|---|
| **Shapes and inline caches** ([`08-vm.md`](08-vm.md) §6, §7) | A JIT without a stable shape id is a JIT that deoptimizes on every field access. The shape mechanism is what lets compiled code load a slot with a *load, a test, and a branch* instead of a dictionary lookup. Building the JIT first means building the ICs twice. |
| **`DEOPT`, `JMP_BACK`, `SAFEPOINT`, `TYPE_GUARD`** ([`07-bytecode.md`](07-bytecode.md) §4) | The deopt mechanism *is* these opcodes. A JIT that is not preceded by them either deoptimizes into a broken interpreter or never deoptimizes, which is worse. |
| **The M9 profile** ([`09-memory.md`](09-memory.md) §10) | The JIT's value proposition is "replace the dispatch loop". If the dispatch loop is not the bottleneck, the JIT is 400 lines of risk for no gain, and M9's gate is the measurement that tells us. |

So M11 is last, and the consequence is stated plainly in §2.4: **every benchmark in the project up
to M10 is an interpreter benchmark, and the JIT must beat the interpreter it was written against,
not an idealised one.** That is a constraint on the [T]s in §12.1 and it is why the target is
2–3× on one benchmark rather than 10× on a synthetic one.

### 1.2 What "optional" means, operationally

| Commitment | How it is kept |
|---|---|
| The JIT may not block a release | The `jit` feature is off by default; `cargo build`, `cargo test`, and `cargo bench` all work with it off; §11's language tests run in both configurations, exactly as [`09-memory.md`](09-memory.md) §6.4 describes for `gc` |
| The JIT may be abandoned | §10's fallback is *the interpreter*, unchanged. There is no intermediate representation the interpreter needs, no "JIT-compatible bytecode" the interpreter pays for, and the bytecode is identical with and without the feature |
| The JIT may not destabilise correctness | Every compiled path has a `deopt → interpreter` exit, and the interpreter is the *same code* the language ships. A misprediction is a slowdown, not a wrong answer — and §11 test 4 asserts exactly that over 10⁹ random operations |
| A wrong compilation must not be a security bug | The deopt path re-checks every type guard (§7.3), so a stale compiled path is bounded by "runs the interpreter", not "reads a wrong slot" |

**The one thing a JIT is *not* allowed to be here is a source of undefined behaviour.** Cranelift
emits real machine code into an executable mapping; a bug in it is a segfault or worse, not a
panic. That asymmetry is why §11's test plan is heavier than §11 of any other document, and why
§9 spends as much space on "never free executing code" as on the register allocator.

### 1.3 The "extremely experimental" problem, stated up front

The `cranelift-jit` crate's own README says, verbatim: **"This crate is extremely experimental."**
That is not a hedge; it is the crate's own description of its stability, and it is the single
most important input to this document. It means:

- **A Cranelift upgrade can be a breaking change with no deprecation period.** The `JITBuilder`
  and `JITModule` APIs have changed shape across releases, and `hotswap` and
  `prepare_for_function_redefine` — which existed in earlier versions and were the obvious way to
  build a hot-swapping JIT — are **gone** in 0.136.1 (§4.1). Any design that depends on a
  specific 0.136 internal is one minor version away from a rewrite.
- **Therefore the project must not depend on `cranelift-jit` as a load-bearing crate**, and
  §4.3's ruling is that the *only* `cranelift-jit` API used is `JITModule::define_function` plus
  `JITMemoryProvider`. Everything else is `cranelift-codegen` (`cranelift-native`, which is
  stable and a different maturity level) plus our own linker.
- **The dependency is `0.136` in `Cargo.toml` with an exact-looking pin.** The existing
  declaration in the workspace `Cargo.toml` is `version = "0.136"`, which in Cargo means
  `^0.136` — i.e. 0.136.x only, since 0.x minor bumps are breaking. That is the correct degree
  of pin for an experimental dependency, and §12.3 makes it a CI check rather than a convention.

Wasmtime reached the same conclusion independently: **PR #10345
(<https://github.com/bytecodealliance/wasmtime/pull/10345>) removed the hot-swap machinery from
Wasmtime's JIT** rather than maintain it, because the complexity was not paying for itself. The
design in §3 is a consequence of that removal, and the citation is here so a reviewer can see this
is a known industry pattern rather than an idiosyncratic choice.

---

## 2. Strategy: what to compile, and what not to

### 2.1 The unit of compilation: one Zen function, per `FunctionProto`

[`08-vm.md`](08-vm.md) §3.4 gives the loop, the frame window, and the `RETURN`/`JMP_BACK`
structure. Two decisions follow.

> **Ruling 1: the unit of compilation is a whole `FunctionProto` — one Zen function body, from its
> entry to every `RETURN`.** Not a hot basic block, not a loop body, not a trace.

> **Ruling 2: the compilation granularity never affects semantics.** A compiled function must be
> able to call any other function — compiled or not — through the same `CALL` path, so a
> mixed-mode run is a normal state, not a special case.

**Whole-function is chosen over hot-block (LuaJIT-style trace compilation) for four reasons**, and
the fourth is decisive:

1. **Hot-block requires a trace compiler**: a state-machine assembler, a side-exit table, and a
   trace-quality heuristic. [E] 3 000–5 000 lines, and every bug in it is a miscompile.
2. **Hot-block's failure mode is visible and bad.** A trace compiled from a warm-up loop is
   specialised to that loop's shapes. A Zen program that hits a different shape mid-loop takes a
   side exit; if the side exit is wrong, the program's arithmetic is wrong. Whole-function
   compilation fails *safe* instead: it deoptimizes to the interpreter (§7), which is the shipped,
   tested semantics.
3. **Zen's opcodes are already register-based and shape-dispatched** ([`07-bytecode.md`](07-bytecode.md)
   §3), so a whole function is roughly 3–10 machine instructions per opcode, and the
   remaining win is the *removal of the dispatch* — which whole-function compilation gets. **The
   headroom is smaller than a general-purpose JIT's, and that is fine**, because §2.4's target
   accounts for it.
4. **Hot-block buys the loop-invariant hoisting that whole-function compilation does not.** A
   whole-function compiler *can* still do a peephole pass and a simple loop-invariant hoisting
   pass, and [E] that recovers most of it. It does not need a trace.

**The cost of Ruling 1, stated honestly:** Zen will not match LuaJIT on a tight numeric loop.
LuaJIT compiles one *iteration* of a hot loop, so register allocation and invariant hoisting are
near-optimal. Zen compiles the whole loop, so a loop-carried dependency chain is served from
registers but nothing is hoisted out. **[E] on a tight `while` integer loop, whole-function
compilation gets maybe 3–5× over a well-implemented dispatch interpreter; LuaJIT gets 10–20×.**
§2.4 sets the target at 3× for exactly this reason, and §12.1 makes "if we get 3× on `fib` we
are done" the success criterion, not "10× on a synthetic loop".

### 2.2 What the compiler assumes, and what it re-checks

A compiled Zen function may assume, and the deopt path re-checks:

| Assumption | Guard it installs | Deopt action |
|---|---|---|
| Every `Value` in a register is tag-X, with a specific type | `TYPE_GUARD` on entry to a block that is only reached if the tag matched | reconstruct the interpreter's register state and `JMP` to the deopt label |
| Every `instance` access is on a shape with slot `k` at offset `+16 + 8k` | the shape id, loaded and compared against a constant | see §7.2 |
| Every `dict` access with a constant key is at a stable slot | the IC entry, checked at compile time; the *runtime* check is that the IC has not been invalidated | invalidate the IC, re-look-up, patch the IC, and — for a shape *change* — deopt |
| The host has a `&mut Vm` for the whole function | none; it is a parameter | — |
| Arithmetic cannot trap | `INT_ADD` overflow **promotes to `float`**, it does not trap and does not wrap ([`08-vm.md`](08-vm.md) §4.3); division by zero is a *semantic* check, not a trap ([`08-vm.md`](08-vm.md) §4.4) | the compiled code emits the same promote-and-recheck (§6.2) |

**Every one of those guards is a real machine-code sequence, and each is a place a miscompile
becomes a wrong answer.** §11's test plan is built around that: test 4 runs 10⁹ randomised
operations through both paths and compares. That test is the JIT's correctness argument, and a
reviewer should be suspicious of any design where the argument is "we emit the right thing".

### 2.3 Tiering: the three-tier ladder

```
                 ┌──────────────────────────────────────────┐
   tier 0        │  Interpreter (08-vm.md §3)               │  always correct
                 │  1 dispatch load + 1 indirect jump / op  │  ~2.4 ns/op [E]
                 └───────────────┬──────────────────────────┘
                                 │ hit counter ≥ TIER_UP
                 ┌───────────────▼──────────────────────────┐
   tier 1        │  Interpreter + inline caches             │  M5, always on.
                 │  IC hit: 1 load + 1 cmp + 1 branch       │  the field-access
                 │  IC miss: #[cold] fallback + IC patch    │  fast path lands
                 └───────────────┬──────────────────────────┘
                                 │ tier-1 hot count ≥ COMPILE
                 ┌───────────────▼──────────────────────────┐
   tier 2        │  Cranelift-compiled machine code          │  M11, optional.
                 │  no dispatch; guards inline; deopt exit   │  ~0.9 ns/op [E]
                 └───────────────┬──────────────────────────┘
                                 │ deopt (IC invalidated, STW GC, unhandled shape)
                 └───────────────▶ back to tier 0, forever
```

| Tier | Exists in | Hot count threshold | Deoptimizable? |
|---|---|---|---|
| 0 interpreter | M2+ | 0 | n/a |
| 1 IC-annotated interpreter | M5+ | 0 (ICs are always installed) | n/a |
| 2 compiled | M11, `--features jit` | `TIER_UP = 1_000` loop iterations or `COMPILE = 8` distinct IC misses at one site | **yes, to tier 0** |

**Deoptimization is one-way and permanent.** A function that deoptimizes goes back to tier 0 and
its compiled code is discarded. It can be re-entered and re-triggered, and if it thrashes
(§7.6) the `Compiled` state records a "do not recompile" flag so the cost is one wasted compile,
not a compile loop. **There is no re-optimisation and no tier 3.** A reviewer should ask about
this, because "deopt then recompile better" is what V8 does and it is the most complex part of a
real JIT; excluding it is a scope decision, and §12.2 records it.

**The `#[cold]` in tier 1 is why tier 2 is even worth building.** By M5, `d.a` on a shaped object
is a load, a compare, and a predicted-taken branch. The JIT's win over *that* is removing the
1-load dispatch and the indirect jump, which on a modern OoO core is worth maybe 1.5–2 cycles per
opcode when predicted — hence §2.4's modest 2–3× and §12.1's per-benchmark targets rather than a
blanket one.

### 2.4 The performance target, and the benchmark list

The target is stated per benchmark, because a single "3× faster" number is unfalsifiable.

| Benchmark | What it stresses | [T] target vs tier 1 | Why that number |
|---|---|---|---|
| `zen-bench fib` (naive recursive, 27) | call overhead, ICs, `int` arithmetic | ≥ 2.5× | the call sequence is the bulk of the work; whole-function compilation makes a leaf function ~15 instructions with no dispatch |
| `zen-bench loop_sum` (1e8 `while`) | dispatch removal on a tight loop | ≥ 1.5× | a tight loop is *already* well-predicted by the indirect branch predictor, so the win is smallest here. A JIT that claims 5× on this is measuring a broken tier-0 |
| `zen-bench json_parse` | shape transitions, string ops, dict churn | ≥ 2.0× | the string-object change ([`09-memory.md`](09-memory.md) §5.5) is a real part of this number |
| `zen-bench nbody` (float) | float arithmetic, `while`, sqrt | ≥ 2.0× | same reasoning as `loop_sum` |
| `zen-bench list_ops` (append/index/iterate) | CoW tests, `rc` traffic, bounds checks | ≥ 1.3× | the `cow_mut` `rc == 1` compare stays in the compiled code, so the win is only the dispatch |
| `zen-bench startup` (parse + run a hello world) | **nothing** | **≥ 0.9× (may regress ≤ 10%)** | see below |

**`startup` is in the list on purpose.** Cranelift compile time is [E] 0.3–2 ms per small function;
a hello world that runs in 0.4 ms would be dominated by compiling it. §5.3's rules (compile only
above a hot-count threshold, and only in a background thread) exist to keep the regression under
10%, and **`startup` regressing by more than 10% is a [T] failure that stops the feature from
defaulting to on.** That is the honest way to handle a real cost.

---

## 3. The hot-swap problem, and the ruling that resolves it

### 3.1 The two desirable properties, and why they are incompatible

A JIT for a language with inline caches wants two things:

1. **In-place patching.** When `d.b = 2` discovers that `{a}`'s shape must extend to `{a, b}`,
   every compiled access to `d.a` at every call site is now slower by one hop. The cheapest fix
   is to rewrite those instructions in place: a 4-byte displacement, or a 5-byte `jmp`. This is
   what "hot-swap" means, and it is why `hotswap` was a `JITBuilder` option in earlier Cranelift.
2. **Immutable code.** x86 and AArch64 caches are not coherent with self-modifying stores in
   the general case. Modern x86 requires the `WB`/`INVD`-equivalent dance (Intel SDM §8.1.3:
   serialising instruction, cross-modifying-code detection, invalidation), and AArch64 requires
   `dc cvau` + `ic ivau` + a barrier per modified instruction. Getting this wrong does not produce
   a slow program; it produces **an intermittently wrong program**, which is the worst failure
   class in this project.

**These conflict, and the conflict is a hardware fact, not a design taste.** The resolution
available in Cranelift 0.136.1 is that (2) is enforced: the API no longer offers (1) at all
(§4.1). So the design must obtain whatever benefit it can from (1) *without* patching code.

### 3.2 The ruling: immutable per-function code, published by an atomic swap

> **Ruling: every compilation produces a brand-new, immutable code region. Nothing is ever patched.
> A call site that wants a newer version of a callee reads an `AtomicPtr` and calls through it. Old
> code is retained until VM teardown.**

```rust
// zen-vm/src/jit.rs  — M11, #[cfg(feature = "jit")]

/// The published address of a compiled `FunctionProto`.
///
/// Reads on the hot path are `Acquire`; writes happen once per compilation.
/// `None` means "tier 0" and the caller goes through the interpreter's
/// `CALL` arm, which is the *unchanged* interpreter path — so a `None`
/// here is not an error state, it is the default state.
pub struct CodeSlot {
    ptr: AtomicPtr<u8>,        // entry point, or null
    len: AtomicUsize,          // for §9's lifetime accounting
    /// The module that owns `ptr`'s mapping. **Never freed while the VM
    /// lives** (§9). Held so that teardown can unmap in the right order.
    module: RwLock<Option<Arc<JitModule>>>,
}

impl CodeSlot {
    /// # Safety
    /// The returned pointer must remain valid for the lifetime of the
    /// `&self`, which the `module` field guarantees: the module is only
    /// dropped in `CodeSlot::drop`, and `drop` runs when no `&self` exists.
    #[inline(always)]
    fn entry(&self) -> Option<NonNull<u8>> {
        // Relaxed would be *almost* correct — the code mapping is immutable
        // and the code pointer is a data word — but the pairing with the
        // IC (below) needs Acquire: the IC that leads here is published
        // with Release, and this read must not be reordered before it.
        NonNull::new(self.ptr.load(Ordering::Acquire))
    }

    #[inline]
    fn publish(&self, module: Arc<JitModule>, entry: *mut u8, len: usize) {
        let mut m = self.module.write().expect("poison is impossible: §07 of 09-memory");
        *m = Some(module);                    // the mapping outlives the code
        self.len.store(len, Ordering::Relaxed);
        // Release: the code bytes were written by Cranelift *before* this
        // store, and any thread that Acquire-loads this pointer must see
        // them. A Release store on a pointer into a mapping that is already
        // executable is the standard publication idiom.
        self.ptr.store(entry as *mut u8, Ordering::Release);
    }
}
```

**Why this is correct, in the three cases that matter:**

1. **A thread calls through a pointer it read 10 minutes ago.** The code region is immutable and
   retained for the VM's lifetime, so the pointer is valid. The only "staleness" is that the
   thread is running an old version of the callee, which is *semantically identical* by §3.3.
2. **A thread reads a pointer published by another thread.** `Release`/`Acquire` on `CodeSlot::ptr`
   orders the code bytes before the pointer. The `module` field is published *before* the pointer
   store, so a thread that sees the pointer also sees the `Arc<JitModule>` that keeps the mapping
   alive — a non-trivial ordering requirement, and the reason `publish` writes the module first.
3. **A compilation replaces an existing version.** The old module is dropped from the `RwLock`
   only at teardown, so both versions coexist. `Arc<JitModule>`'s `Drop` unmaps; it runs once, at
   the end, never while a thread might be executing in that mapping.

### 3.3 What we lose, and the compensating design

The lost property is in-place patching, and the loss is real. Compensation:

| Lost | Compensation | Residual cost |
|---|---|---|
| Patch a 4-byte displacement in a hot field access | **The shape-transition IC entry is a separate 8-byte word in the interpreter's IC table**, patched in *data*, not in code. A compiled `GET_FIELD` re-checks the shape id (a `cmp` against an immediate) and takes a `#[cold]` exit that re-reads the IC. So the "patch" is an ordinary store to a `u32` in a table, which needs no cache-coherency dance. | one predictable compare + branch per field access, where hot-swap would have been zero |
| Hot-swap a whole function body after an IC update | the `CodeSlot` swap (§3.2). The next call goes to the new version. | one atomic load per call site (a `mov` on x86, hit in L1) |
| Avoid a second compilation of an improved function | none. **One version per `FunctionProto`, ever.** A recompile replaces the `CodeSlot`, so the old version lingers. | bounded by the number of recompiles; see §7.6's recompile cap |

**"One version per `FunctionProto`, ever" is a deliberate simplification.** It means the compiler
does not need to react to IC updates at all: it compiles from the *current* IC state and that
state never changes under it. If the IC is invalidated (a new shape appears), the compiled code
deoptimizes (§7) and the interpreter rebuilds the IC; a *later* recompilation of the same proto
picks up the better IC and publishes a new version. So the loop is: compile once → deopt on a
surprise → interpreter improves its ICs → recompile → deopt on the *next* surprise. The
`recompile` cap in §7.6 bounds it.

### 3.4 Why not just use the older Cranelift with `hotswap`?

Because it is gone, and pinning to an old release to get it would mean:

| Option | Verdict |
|---|---|
| Pin `cranelift-jit` to a version that has `JITBuilder::hotswap` | **Rejected.** It is an experimental crate on a 0.x cadence; pinning to a *stale* release means missing security fixes in `cranelift-codegen`, and the maintenance tail grows every upgrade. The dependency is already declared at `0.136` in the workspace `Cargo.toml`, and the whole point of §1.3's `^0.136` pin is that it is the *current* 0.136.x. |
| Implement self-modifying-code patching ourselves on top of 0.136 | **Rejected.** Intel SDM §8.1.3 and AArch64's `dc cvau`/`ic ivau` sequence must be correct on every microarchitecture, and a mistake is an intermittent wrong answer. §1.2's rule — a JIT must not be a source of UB — forbids trading a hard problem for a scarier one. |
| Use `mmap` + `mprotect` and write the patch ourselves | Same as above. |
| **Immutable code + `AtomicPtr` publication (§3.2)** | **Adopted.** ~30 lines, no hardware-specific cache maintenance, no undefined behaviour, and the only cost is one predictable compare per field access and one atomic load per call site. |

---

## 4. Cranelift API survey: what 0.136.1 actually offers

### 4.1 The API facts, verified

These are the **[M]** measurements of this document — read off docs.rs for `cranelift-jit` 0.136.1:

| Fact | Value | Consequence |
|---|---|---|
| Latest published version | `0.136.1` | the workspace's `version = "0.136"` resolves to `^0.136` → 0.136.x only. Correct for an experimental dep |
| `JITBuilder::hotswap` | **does not exist in 0.136.1** | §3.1's in-place patching is unavailable; §3.2's ruling is forced, not chosen |
| `JITModule::prepare_for_function_redefine` | **does not exist in 0.136.1** | redefining a function in place is unavailable; a new module per compilation is required |
| Redefining an already-defined function | **rejected** — the definition machinery keeps a symbol table and errors on a duplicate | "recompile into the same module" is not an option; each compilation needs a fresh `JITModule` |
| `JITMemoryProvider` | **present** — user-supplied allocation/finalization hooks | §9's custom code allocator; the only reason to use `cranelift-jit` rather than a raw `mmap` |
| README stability | **"This crate is extremely experimental."** | §1.3: depend on a four-method surface only |
| Independent precedent | Wasmtime removed its hot-swap machinery (PR #10345) rather than maintain it | the industry has already made this call |
| `cranelift-codegen` / `cranelift-native` | **present and separately versioned** | the *ISA description* is in `cranelift-native`, which is the stable half; §4.3 keeps the dependency on the experimental half to four methods |

**The single most important row is the third and fourth together**: no `hotswap`, no
`prepare_for_function_redefine`, and duplicate definitions are rejected. That is a complete
argument for "one fresh `JITModule` per compilation", and it is why §9's module lifetime is a
`Vec<JitModule>` rather than a single one.

### 4.2 What we use, and what we do not

```rust
// The entire `cranelift-jit` surface this project uses. Four items.
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_jit::memory::{JITMemoryProvider, JITMemory};
```

| `cranelift-jit` item | Used? | For what |
|---|---|---|
| `JITBuilder::new()` | **yes** | one per compilation, configured with our `JITMemoryProvider` (§9) and the target ISA from `cranelift_native::builder()` |
| `JITBuilder::symbol()` | **yes** | register `zen_builtin_*` so compiled code can call builtins by name |
| `JITBuilder::memory(&mut provider)` | **yes** | the code allocator of §9 |
| `JITModule::new(&mut builder)` | **yes** | one per compilation (§3.2) |
| `JITModule::define_function(&mut self, id, &mut ctx, &mut ctrl_plan)` | **yes** | the only compilation call |
| `JITModule::get_finalized_function(id)` | **yes** | yields the entry pointer, which §3.2 publishes |
| `JITModule::finalize_definitions()` | **yes** | flips the memory RX before any pointer is published |
| `JITBuilder::hotswap` | **unavailable** | §3.1 |
| `prepare_for_function_redefine` | **unavailable** | §4.1 |
| `JITModule::from_buffer` | no | we compile from IR every time; no IR caching in M11 |
| Any `cranelift-codegen` context/IR **optimisation** pass | **no** | see §4.4 |

### 4.3 The `cranelift-jit` API surface is four methods. Everything else is ours.

The dependency argument, in one line: **four methods is an API that can be reimplemented against a
future Cranelift in an afternoon.** This is the design constraint that makes §1.3's
"extremely experimental" tolerable.

| We use | We implement ourselves | Why we implement it |
|---|---|---|
| `JITBuilder::new` / `symbol` / `memory`, `JITModule::new` / `define_function` / `get_finalized_function` / `finalize_definitions` | the **frontend**: Zen bytecode → `cranelift_codegen::ir` (`Function` with blocks, `Inst`s, types) | `cranelift-frontend` is a *Cranelift-IR* builder, not a Zen-bytecode compiler. Somebody has to write the translation, and the IR types are the stable part of the dependency |
| " | the **lowering**: the register allocator's inputs, the guard emission, the deopt metadata in `func.user_srclocs` / a side table | the guard/deopt scheme is Zen's, not Cranelift's |
| " | the **cache model**: `flags.enable_pic`, `is_pic`, the relocation-free direct calls | §4.4 |
| " | the **frame layout** and the **safepoint protocol** (§7, §8) | the interaction with `08-vm.md`'s register file is Zen's |
| " | the **profiling / tiering policy** (§5) | no JIT does this for you |
| " | the **deopt reconstruction** from guards back to interpreter register state (§7.3) | entirely Zen-specific |

`cranelift-frontend` is still useful — for building the `Function`'s entry block and for
`Variable`s — but it is `cranelift-codegen`'s sibling with a *stable* API, and §4.3's table shows
it is not on the critical path for the experimental-crate risk.

### 4.4 Position independence, the symbol table, and `set_colocated`

| Setting | Value | Why |
|---|---|---|
| `JITBuilder::symbol` | register `zen_builtin_*` (each builtin), `zen_safepoint`, `zen_deopt` | compiled code calls builtins by name; the symbol table is how `define_function`'s relocations resolve |
| `JITBuilder::new()` with default settings | `libcall_names` for `memcpy`, `memset`, `memmove` | `list.append`'s `Vec` grow lowers to `memcpy`; Cranelift emits a call to a libcall, and the symbol must resolve |
| PIC / `is_pic` | **`true`**, i.e. all access indirect through a GOT | the default is on and the correct choice: Zen is a shared library (`libzen.so` in the plugin design), so code *can* be loaded at a non-fixed address |
| `set_colocated` | **`true`** | enables the direct-call encoding, so `f()` inside a compiled `f` is a `call rel32` rather than an indirect call. This matters for `fib` and §2.4's 2.5× target |
| Register allocation | the default Cranelift RA | reimplementing a register allocator is a multi-milestone project and buys nothing for a single-shot compiler |
| Optimisation passes | **none beyond Cranelift's default `egraph`/`alias`** | the default pipeline is already good; custom passes are a project, not a feature. Revisit only if §2.4's targets are missed *for a reason a profile attributes to the emitted code* |

**The relocation-free-calls decision.** With `set_colocated(true)` and PIC on, a call to a Zen
function inside the *same compiled module* is a direct `call rel32`. A call to a *different*
`FunctionProto*` is an indirect call through the callee's `CodeSlot` (§3.2), which is `mov` +
`call reg`. **Both are re-targetable without patching code** — the `CodeSlot` is the indirection
point — and that is the property that makes §3.2 a complete substitute for hot-swap on the call
path. Only the *field-access displacement* would have needed real patching, and §3.3's compare-
against-immediate is the accepted cost.

---

## 5. Tiering: when does code get compiled

### 5.1 The counters, and where they live

Two counters, both in data structures that already exist, so tiering needs **no new hot-path
state**:

| Counter | Where | Incremented by | Reset by |
|---|---|---|---|
| `invocations` | `FunctionProto.jit_count: u32` (M11, `#[cfg(feature = "jit")]`, 4 bytes per proto) | the `CALL` arm, once per call | never (monotone, `Relaxed`) |
| `ic_misses` | already exists: the IC entry's own miss counter ([`08-vm.md`](08-vm.md) §6.1) | the `#[cold]` miss arm | never |

A proto is compiled when **either**:

- `invocations ≥ 1_000` (**TIER_UP**), *or*
- any single IC site in it reaches `8` misses (**COMPILE_MISS**).

**Why `TIER_UP = 1_000` is not 10.** [E] a proto is compiled in 0.3–2 ms of Cranelift time; a
`1_000`-invocation threshold amortises that to 0.3–2 µs per call, i.e. below a tier-0 call. At
`TIER_UP = 10` the compile cost would dominate short-running functions by 10–100×. At
`1_000` it is invisible; at `1_000_000` a program that calls a hot function a million times pays
1 ms once and still wins. **The threshold is deliberately high, because a JIT that compiles
eagerly turns a 5 ms script into a 200 ms script** — and §2.4's `startup` gate is the backstop
for the case where a low threshold slips through.

### 5.2 Compilation is off the dispatch path, on its own thread

> **Ruling: compilation happens on a dedicated compiler thread, never on the mutator thread and
> never inside the dispatch loop.**

```rust
// zen-vm/src/jit.rs

/// A `FunctionProto` queued for tier 2. `Mutex<VecDeque>` is correct and
/// sufficient: the mutator only ever `push`es, and only the compiler thread
/// pops. A lock-free queue (Vyukov) is the [E] upgrade if `push` ever shows
/// up in a profile, which for a once-per-proto push it will not.
struct CompileQueue(Mutex<VecDeque<u32>>);

/// The compiler thread.
///
/// # Safety-critical
/// It holds a `&Jit` (not a `&mut Vm`). The `Vm` it needs — the `protos`
/// table, the ICs, the `CodeSlot`s — is behind a `RwLock` split out of the
/// mutator's `&mut Vm` at M11, and that split is the single largest
/// structural change M11 makes to the VM. §7.2 spells it out.
pub fn compiler_thread(vm: Arc<JitContext>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        let Some(proto) = vm.queue.pop_blocking() else { return };
        if vm.is_compiled(proto) { continue }               // a race lost; fine
        if stop.load(Ordering::Relaxed) { break }

        match vm.compile_proto(proto) {
            Ok(slot) => vm.slots[proto as usize].publish(slot),
            Err(e) => {
                // A compile failure is NOT a program error. Mark the proto
                // as not-compilable and fall back to tier 0 forever.
                // §11 test 6 asserts the program still produces the right
                // answer with the JIT "enabled but failing".
                vm.mark_uncompilable(proto, e);
            }
        }
    }
}
```

**Three properties of that loop are load-bearing, and all three are about failure:**

1. **`Err` is not fatal and does not deoptimize.** A proto that fails to compile is recorded and
   stays at tier 0. This is what makes the JIT *safe to experiment with*: a Cranelift regression
   in a future upgrade degrades performance, never correctness.
2. **The compiler thread never holds the mutator's lock across a compile.** `compile_proto` takes
   a snapshot of the proto's IR-relevant state (`&FunctionProto` is immutable after M4 — this is
   why §5.1 says the counters are separate from the proto) and copies the IC array. The mutator
   runs uninterrupted; the only synchronisation is the `Release` store in `publish`.
3. **A single thread, not a pool.** Cranelift is CPU-bound and the workload is bursty;
   [E] a second thread would halve M11's queue latency for a 2× compile-time cost, and queue
   latency does not matter (§5.1 made it invisible). Revisit if a profile disagrees.

### 5.3 The compile pipeline, step by step

```rust
// zen-vm/src/jit/lower.rs

/// Zen bytecode → Cranelift IR → machine code.
///
/// # Errors
/// Returns `Err` for any construct the lowering does not support. This is
/// the *only* legitimate way a proto fails to compile, and it is a
/// **per-proto opt-out, not an error path**: `mark_uncompilable` records it
/// and the proto stays at tier 0 (§5.2's property 1). Support grows
/// monotonically — a proto that is not supported today is a perf detail,
/// not a bug report.
pub fn compile(proto: &FunctionProto, ic: &IcArray) -> Result<Compiled, Unsupported> {
    let mut b = jitbuilder(ic);
    let mut ctx = cranelift_codegen::Context::new();
    let mut func = frontend::FunctionBuilder::new(&mut ctx.func, &mut b);

    let mut frame = Lowering::new(proto, ic);   // §8's frame map
    for block in proto.body.iter().split(JMP_BACK) {
        emit_block(&mut func, &mut frame, block)?;   // §6
    }
    func.finalize();
    ctx.func.signature = frame.cranelift_signature();   // §8.1

    // `max_reg` is the Cranelift-side allocation. The Zen register file is
    // spilled to the Rust stack (§8.2), so the *only* live registers across
    // an opcode are the ones §6 hoisted. Over-allocating here costs a push
    // per register; under-allocating spills correctly but slowly. 16 is
    // [E] enough for Zen's 3-address opcodes plus a few pinned.
    ctx.set_max_reg(if cfg!(target_os = "macos") { 20 } else { 16 });
    ctx.compile(&*b.engine(), &mut b.module_data())?;
    Ok(Compiled { ctx, module: b.module_data })
}
```

**The `ZEN_BYTECODE` ↔ `CRANELIFT_IR` correspondence**, so a reviewer can check completeness:

| Zen opcode ([`07-bytecode.md`](07-bytecode.md) §4) | Cranelift IR shape | Notes |
|---|---|---|
| `CONST_INT k, a` | `iconst.i64 k; ireduce.i48; global_store a` | the i48 truncation is a real instruction, not a no-op — §6.2 |
| `CONST_FLOAT` | `f64const k; global_store` | |
| `LOAD_GLOBAL g, a` | `global_addr g; global_load a` | the global array is a `&mut [Value]` behind the `&mut Vm` param |
| `SET_LOCAL` / `GET_LOCAL` | `global_addr`/`global_load`/`global_store` on the spill slot index | §8.2's mapping |
| `GET_FIELD k, a` | `global_load a; typecheck; load shape; cmp shape_id, imm; brif deopt; load [obj+16+8k]` | §6.3 |
| `SET_FIELD k, a` | `cmp [obj+8], 1; brif slow_cow;` then the store, then the `release` call | the `rc == 1` test is *in the compiled code* — §2.3's "CoW test stays" |
| `BIN_OP` (int, promoting) | `iadd`/`isub`/`imul` + an i48-range overflow test + a `#[cold]` promote-to-`float` block | **`INT_ADD` overflow promotes to `float`**, per [ADR-0001](decisions/ADR-0001-tag-budget.md) and [`08-vm.md`](08-vm.md) §4.3. It does **not** wrap and no overflow is silently dropped — §6.2 |
| `BIN_OP` (float) | `fadd`/`fsub`/`fmul`/`fdiv` | |
| `IDIV` | `sdivrem` on sign-extended i48s + an explicit zero test → a Zen error `Value` | division by zero is a `DivideByZero` error, checked *before* the division ([`08-vm.md`](08-vm.md) §4.4); it cannot trap, and neither can the division itself (§6.2's last row) |
| `IMOD` | the same `fmod`-derived path the interpreter uses + the same zero check | **Rule as [`08-vm.md`](08-vm.md) §4.4 states it: "`MOD` follows the sign of the dividend, matching the legacy `fmod` behaviour in `src/runtime.rs` rather than Rust's `%`."** Note the example that document gives (`-7 % 3` is `-1`) is *also* Rust's answer for integers, so the divergence it is warning about is in the `fmod` path, not the `srem` path. The lowering must call the same helper the interpreter calls rather than emitting `srem` itself, and §11 test 3 covers the sign cases. **Flagged for a reviewer** (§12.4): the canonical text names a difference the example does not show, and this document does not guess which is meant |
| `TYPE_GUARD` | `typecheck` + `brif deopt` | §7.3 |
| `CALL f, argc, ret` | `call sig0, f(void(vm, args))` — direct, or an indirect through the `CodeSlot` (§4.4) | |
| `RETURN` | `return` | |
| `JMP`, `JMP_BACK` | `br` | `JMP_BACK` is a loop back-edge, lowered as a branch; **back-edge counter** for the OSR case is §7.5 |
| `DEOPT site` | not lowered; the *guard* that would deopt is the branch, and the target label is a deopt stub | |
| `SAFEPOINT` | `call zen_safepoint(vm)` | §7.1 |
| `TYPE_GUARD` on a `handle` | `typecheck` + `brif deopt` | handles are indices; the compiled code loads the table entry |
| `ITER_INIT` / `ITER_DONE` / `ITER_NEXT` | a load of the iterator `Value` + the `ITER_DONE` flag test | the flag is `ObjHeader.flags & CLOSED`-style, so it is one `test` against a `u16` in the header |
| `CHANNEL_RECV` / `SEND` | **not lowered** — a `call` to a builtin | blocking must not be inlined; §7.1 |
| `SET_UPVAL` / `GET_UPVAL` | the upvalue cell load/store; an open upvalue is an index into the register file | |
| everything else | `Err(Unsupported)` → `mark_uncompilable` | tier 0, forever |

**The rule behind that table: a blocking operation is a call, not an inlined sequence.** A
`CHANNEL_RECV` that the JIT "optimised" into a spin loop would be both a miscompile (it would
starve the scheduler) and a way to hold the GIL-analogue forever. §7.1's safepoint protocol is
only sound if compiled code *stops running* at safepoints, and a spin loop does not stop.

---

## 6. Code emission: the shape of a compiled Zen function

### 6.1 One Zen opcode, roughly one instruction

The generated code is not a switch. It is straight-line code with labels, and the register
allocator does the work. The shape of a compiled `fib`:

```c
// Disassembly shape (x86-64, illustrative — the actual sequence is
// Cranelift's, and the point of the listing is the *absence* of a dispatch
// load and an indirect jump).

zen_fn_0:                          ; "fn fib(n) { if n < 2 { return n } ... }"
    ; r1 = n ; spilled to the Rust frame at this point
    global_addr.  r2, [vm + n_off]
    global_load.  r1, r2
    ; r1 = int, sign-extended from the 48-bit payload (a no-op width-wise on
    ; x86, but the `Value`'s tag bits must be masked off first)
    ; guard: tag == 0b011 (int)
    typecheck.    r1 == t_int
    brif           deopt_0, r1, deopt_0
    ; if n < 2 → return n
    icmp_imm.     r3, r1, 2
    brif           deopt_0, r3, ret_0
    ; else recurse twice
    call          fib_slot, (vm, n)        ; indirect through CodeSlot §3.2
    ...
ret_0:
    return       r1
deopt_0:                                ; the interpreter re-entry, §7.3
    jmp          zen_deopt(vm, site_0, frame_map)   ; never returns
```

**What the listing shows that a §2.1 hot-block JIT would not:** no `TYPE_GUARD` for the
*recursive* call's result (the callee's own guard covers it), one `brif` per type assumption, and
the deopt as a tail-call to a shared stub so the deopt code is not duplicated per site.

### 6.2 The i48 problem: overflow **promotes to `float`**, it does not wrap

`Value` packs an `int` into 48 bits ([ADR-0001](decisions/ADR-0001-tag-budget.md), whose
Consequences state the rule: *"`Int` is i48, range ±140,737,488,355,327. Overflow promotes to `float`
rather than wrapping"*), so an `int` is a sign-extended `i48` held in a 64-bit register. The
semantics live in exactly one place — [`08-vm.md`](08-vm.md) §4.3's table: *"Never Rust arithmetic.
`wrapping_add` on the masked payload, then an explicit overflow test that promotes to `float`."*

**This is the single most important semantic the JIT has to reproduce**, and it is the opposite of
what most JITs emit: an `int` add is *not* a `iadd` that leaves an `int`. It is a `iadd`, an
overflow test, and a **type change** on the overflow path. Cranelift has no `iadd`-with-overflow-
to-different-type, so the sequence must be built from pieces:

| Operation | Emitted | Note |
|---|---|---|
| `LOAD_GLOBAL` of an `int` | `global_load.i64` then `ireduce` (i64→i48) | the `Value`'s low 48 bits already hold the magnitude and bit 47 the sign, so this is a mask-and-sign-extend |
| `INT_ADD` | `iadd.i64` on the two sign-extended i48s, then an **`overflowing` test**, then `brif` to a `#[cold]` promote-to-`float` block | the fast path is `iadd` + `jo`/`seto` + `brif`; the taken branch converts both operands to `f64`, adds, and produces a **`Value` with tag `0b100`** |
| `INT_SUB`, `INT_MUL` | same shape | `imul` is the one to be careful with: `2⁴⁷ × 2⁴⁷` overflows 64 bits, so the test must be on the **i48 range**, not on the 64-bit `OF` flag, or a legitimate `int` result above 2⁴⁷ is wrongly promoted |
| `CONST_INT` | `iconst` + `ireduce` | in range by loader validation, so no promote path |
| `INT_TO_FLOAT` | `fcvt_from_sint` | |
| `IDIV`, `IMOD` | `sdivrem` on the sign-extended i48s, plus the explicit zero check (§6.4) | **cannot trap**: 08-vm.md §4.4 records that `INT_MIN / -1` is `+2⁴⁷`, in range, so the 64-bit overflow-UB case cannot arise from sign-extended i48 inputs. If a future lowering ever widens an `int` to a full 64 bits, this row becomes a trap and the zero check is no longer the only guard needed. |

**Three consequences that a reviewer should check, each a "works on all my tests" bug:**

1. **The promote path changes the `Value`'s tag**, so any *downstream* `TYPE_GUARD` that assumed
   `int` must be re-derived. In a compiled block the promotion is a `brif` to a block that
   rejoins with a `Value` whose tag is now `0b100`, and the frame map at any later deopt site
   must record that register as "possibly a `float`" — not "an `int`". A frame map that says
   `int` is a wrong answer waiting for a deopt.
2. **`IMUL` needs an i48-range test, not a 64-bit overflow test** (the table's second row). This is
   the same class of bug as §6.4's `INT_MIN / -1` row, and it is the one the fuzz test reaches
   first, because `2⁴⁷ × 2⁴⁷` is exactly the kind of input a grammar-generated test produces.
3. **A `float` result silently changes downstream code**, so a compiled function that promotes
   mid-block and then does `INT_TO_FLOAT` on the result is a **type error** in Zen, not a no-op.
   The compiled code must re-check the tag before the conversion, which means a `typecheck` the
   interpreter also performs. Emitting the conversion unconditionally is a miscompile that
   produces a wrong *type*, not a wrong number — harder to spot, and §11's test 4 (which compares
   results *and* types) is the check.

§11's test 3 is exactly this: a table of `{INT_MAX, INT_MIN, 0, 1, -1, 2⁴⁷−1, −2⁴⁷, 2⁴⁶ × 2}` crossed
with every arithmetic opcode, in both configurations, comparing the result's **bits**, not just its
value — because `int 1` and `float 1.0` print the same in most languages and are different `Value`s
in Zen.

### 6.3 The field-access lowering, and why the shape check is a `cmp`

```rust
// Lowering of GET_FIELD, the single most important instruction sequence
// in the compiler. `obj` is a `&Value` in a register/stack slot; `k` is
// the constant slot index; `shape_id` is the shape this site was
// specialised for.
fn emit_get_field(func: &mut FunctionBuilder, obj: Reg, k: u32, shape_id: u32, site: u32) {
    // 1. Is `obj` an instance at all?  tag == 0b111 && header.kind == INSTANCE.
    //    Two compares, both against immediates. On a misprediction this is
    //    the deopt, and deopt is correct-but-slow, so mispredicting is
    //    free in the correctness sense.
    emit_typecheck(func, obj, Type::Instance, site);

    // 2. Is its shape the one we compiled for? This is the IC, inlined as
    //    a compare against a constant. The `brif` is the whole hot path:
    //    predicted-taken ~99.9% of the time after warm-up.
    let shape = load_header_shape(func, obj);
    let ok = func.ins().icmp_imm(cond, shape, shape_id as i64);
    func.ins().brif(ok, block_cont, &[], deopt_block(site), &[]);

    // 3. The load. `+16` is the payload offset (§2.1 of 09-memory.md) and
    //    `8*k` is the slot index. Both are constants, so this is one
    //    `mov` with a displacement — the thing hot-swap would have
    //    patched, and §3.3 accepts the compare instead.
    let v = func.ins().load(I64, MemFlags::trusted(), obj, 16 + 8 * k as i32);
    // 4. `retain` the value we just loaded, because the `Value` leaves the
    //    object and lives in a register. `retain` is a call to a Rust
    //    builtin (§8.3). NOT elided.
    call_retain(func, v);
    store_value(func, dst, v);
}
```

**Step 4 is the line that would be a use-after-free if it were elided**, and it is in this
document because [`09-memory.md`](09-memory.md) §3.4's rule ("a `Value` leaving a store is
`retain`ed") applies to compiled code exactly as it does to the interpreter, and the compiled
code is written by a different person in a different language. §11 test 4 (`10^9` randomised ops)
and Miri (§11 test 9) are what check it, because the failure is a use-after-free that may not
manifest for hours.

**Why the shape check is `cmp` + `brif` and not a trap**: a trap (SIGSEGV on an unmapped slot, or
`ud2`) is a crash, and §1.2 forbids a JIT that crashes the process. A branch to a deopt stub is a
slow path that is always correct. This is the single most important design constraint on the
emitter, and it is why every guard in §2.2 is a `brif` and none is a trap.

### 6.4 Semantic parity checklist

The compiled path must be observationally identical to the interpreter. Each row is a place where
that is easy to get wrong, and each has a test in §11.

| Concern | Interpreter behaviour | Compiled behaviour | Test |
|---|---|---|---|
| `int` overflow | **promotes to `float`**, per [ADR-0001](decisions/ADR-0001-tag-budget.md) and [`08-vm.md`](08-vm.md) §4.3 | `iadd` + an i48-range overflow test + a `#[cold]` block that converts both operands to `f64` and returns a **tag-`0b100`** `Value`; downstream `TYPE_GUARD`s must be re-derived (§6.2) | 3, 4 |
| `int` overflow in a *frame-mapped* register | the interpreter's register holds whatever the bytecode put there | the deopt frame map must record a promoted register as **type-agnostic**, not as `int` (§6.2's consequence 1) | 3 |
| `idiv` by zero | a `DivideByZero` error, checked before the division ([`08-vm.md`](08-vm.md) §4.4) | the same error construction, not a trap | 5 |
| `idiv` on `INT_MIN / -1` | `+2⁴⁷`, in range, computed by the masked path ([`08-vm.md`](08-vm.md) §4.4) | **cannot trap**, because both operands are sign-extended i48 and `+2⁴⁷` fits in an i64. `sdiv` on a full 64-bit `INT64_MIN` *would* trap, so the lowering must never widen an `int` to 64 bits before dividing | 3 |
| `imod` sign | follows the dividend, via the legacy `fmod` path (§5.3's `IMOD` row) | the same helper the interpreter calls, not a bare `srem` | 3 |
| `INT_TO_FLOAT` after a promotion | a **type error**, not a no-op | the compiled code must re-check the tag before converting (§6.2's consequence 3) | 4 |
| float `nan`, `inf` | IEEE, no trap | identical; no `fcmp` fast path may assume finite | 4 |
| `GET_FIELD` on a non-instance | a Zen `error` (a `TYPE_GUARD` failure) | a **deopt**, then the interpreter raises the same error | 4 |
| `GET_FIELD` on a wrong shape | reads the slot by shape (a *polymorphic* site) or deopts | deopt to the interpreter's IC path, which then either hits the new shape or raises the error | 4 |
| `set_field` on a shared object | `cow_mut` clones | `cmp rc, 1` inline; `brif` to a `#[cold]` `cow_mut` call otherwise (§6.3's step 1) | 4 |
| `RETURN` value | moved to the caller's window | a `Value` written through the `&mut Vm` | 4 |
| `CHANNEL_RECV` on a closed channel | a Zen `error` | a builtin call, so the interpreter's code runs (§5.3) | 7 |
| a `SAFEPOINT` in a loop | the interpreter checks at the top of dispatch | a `call zen_safepoint` at the back edge | 8 |

**The three `int`-semantics rows are the ones that have broken real compilers**, and they are in
the checklist because they share one root cause: the compiled code is the *only* place where an
`int` operation can change the `Value`'s **type**, and every machine-level assumption about
arithmetic ("`iadd` leaves an `int`") is false here. §11's test 3 compares **bits rather than
printed values** for exactly that reason, because `int 1` and `float 1.0` are indistinguishable in
almost any output format and are different `Value`s in Zen.

---

## 7. Deoptimization, safepoints, and the frame table

### 7.1 Safepoints: where they come from and why they are not optional

Three sources, and the JIT must handle all of them:

| Source | Why | Mechanism |
|---|---|---|
| **M9's tracing collector** ([`09-memory.md`](09-memory.md) §7.6) | a collection must stop *every* thread, and a compiled function is running with the dispatch loop's safepoint check nowhere in it | the compiler emits a `call zen_safepoint(vm)` at **every loop back edge** (every `JMP_BACK`) and at every `call` site. Not at every instruction — at those two. |
| **M10's scheduler** | a thread that has run its timeslice must yield, and a compiled loop is a long straight-line block | same: the back-edge call is where the check is, and a back edge is [E] once per loop iteration |
| **Destructors / `defer`** | a `defer` at a scope exit needs a consistent stack | a `defer` exit is a call to a builtin, and builtins are calls, so it is already a safepoint point |

**The rule: the safepoint call goes at every back edge and every call site, and nowhere else.**
Every back edge, not every Nth back edge, because a `while` loop with a GC in the language needs
a bounded safepoint latency and "every N" makes the bound a function of N. §2.4's `loop_sum`
target (≥ 1.5×) is *after* paying for one call per iteration, so the honest target is what is
written.

A back edge in a compiled Zen function looks like:

```c
    ; loop body …
    jmp    loop_top

loop_top:
    call   zen_safepoint, (vm)      ; the *only* safepoint; never elided
    ; ... re-check the loop condition and either fall through or deopt
```

### 7.2 Deoptimization: the state that must be reconstructed

Deoptimization means: "the compiled code has discovered something it assumed is false, and the
interpreter must resume from the right point with the right register values." So the question is
**what state does the interpreter need, and does the compiled code have it?**

| The interpreter needs | Available in the compiled frame? | Mechanism |
|---|---|---|
| The `Value` in every Zen register that is live at this program point | **not necessarily** — a register is in a machine register or a spill slot, and the *live set* at this point is what the register allocator computed | the **frame map**, §7.3 |
| The `pc` (bytecode index) to resume at | no | the deopt site number, an immediate in the `call zen_deopt(vm, site, …)` |
| The IC state | yes, it is the interpreter's | nothing to do |
| The frame's local slots | they are spill slots | the frame map |
| The `&mut Vm` | it is the function's parameter | the `vm` argument |

**The frame map is the whole design of deoptimization**, and it is the one place where the JIT
and [`09-memory.md`](09-memory.md) §7.2 row 18 must agree on a data structure.

```rust
// zen-vm/src/jit/frame.rs

/// Per-deopt-site metadata. One entry per `DEOPT` site in a proto, so
/// `proto.deopt_sites[i]` corresponds to `site_i` in the emitted code.
pub struct DeoptSite {
    /// The bytecode index the interpreter resumes at. §6.3's `site`
    /// immediate indexes this.
    pub resume_pc: u32,
    /// The frame map: for each Zen register the interpreter needs at
    /// `resume_pc`, where is it *right now*?
    ///
    /// `Spill(i)`  — the Zen register is in the Rust frame's spill slot i
    /// `Reg(r)`     — the Zen register is in Cranelift register r
    /// `Dead`      — the interpreter may consider it clobbered; it will be
    ///               re-read from a `LOAD_GLOBAL` in the bytecode
    ///
    /// `Dead` is what makes this cheap: the register allocator is allowed
    /// to drop a value that the bytecode no longer needs, and the frame
    /// map says so rather than forcing a spill it does not need.
    pub map: Vec<(ZenReg, Location)>,
    /// The shape the site was specialised for, for the diagnostic in
    /// `zen jit_stats` (a deopt reason).
    pub expect_shape: Option<u32>,
}
```

**Where the frame map is built: from the bytecode's live-variable analysis**, not from the
register allocator. The lowering knows, at each point in the bytecode, which registers the
interpreter considers live (that is M4's liveness analysis, needed anyway for the
escape-analysis idea in [`07-bytecode.md`](07-bytecode.md) §11). The lowering then *tells* the
register allocator, at each Cranelift safepoint, which values must be preserved:

```rust
// The Cranelift API for this is a `safepoint` instruction plus a
// `user_stack_map`-style side table. The project uses the `safepoints`
// Cranelift function attribute, which is the mechanism designed for
// exactly this (a GC's stack map) and is therefore reusable by §7's
// collector.
//
// A `safepoints` entry per Zen call site and back edge:
//   { pc, spill_slot, refs: [cranelift reg -> "a Value in this slot"] }
```

**Using Cranelift's `safepoints` attribute rather than a bespoke mechanism is a real win**, and
it is the reason the M9-collector/JIT coupling in [`09-memory.md`](09-memory.md) §7.1 is
comfortable: the same table is the JIT's deopt frame map and the collector's stack root map. One
structure, two consumers, and a bug in it shows up in both.

### 7.3 The deopt stub, and the guarantee that makes deopt safe

```rust
// zen-vm/src/jit/deopt.rs

/// The shared deopt stub. Never returns to the compiled code: it
/// re-enters the interpreter and stays there (§2.3's one-way deopt).
///
/// # Safety
/// The compiled frame must match `site`'s frame map exactly. That is the
/// compiler's responsibility; the `debug_assert!`s below are the check
/// that it did, in a debug build, on every deopt.
#[inline(never)]
pub fn zen_deopt(vm: *mut Vm, site: u32, reg: *const u64) -> ! {
    let proto = (*vm).current_proto();
    let d = &proto.deopt_sites[site as usize];

    // 1. Rebuild the interpreter's register file from the frame map. The
    //    compiled frame is on the Rust stack below us; `reg` is a pointer
    //    to its first spill slot, passed by the `call` in §6.1.
    for (zreg, loc) in &d.map {
        let bits = match loc {
            Location::Spill(i) => *reg.add(*i),
            Location::Reg(r)  => debug_assert!(false, "a Reg-mapped value must have been spilled "
                                 "for the deopt; the lowering spills every frame-mapped value, so "
                                 "Reg never appears here — see §7.2's note"),
            Location::Dead    => continue,
        };
        // SAFETY: `vm` is the compiled function's `&mut Vm` parameter,
        // still live because the compiled frame holds it; the index is
        // `zreg` from the bytecode's own numbering, < 256 by 08-vm.md §2.1.
        unsafe { (*vm).registers[zreg.0 as usize] = Value::from_bits(bits) };
    }

    // 2. **Re-verify every type guard at the deopt site.** This is the
    //    guarantee that makes deopt *safe* rather than merely *slow*:
    //    the compiled code's assumption is not trusted, it is re-checked
    //    by the interpreter's own dispatch, which is the shipped
    //    semantics. A mispredicted guard therefore costs a re-check, and
    //    a *wrong* guard is caught by the interpreter, not by whatever
    //    the compiled code did with the bad value.
    //
    //    §11 test 4 is the test of this claim: a deliberately miscompiled
    //    `GET_FIELD` must produce the interpreter's answer, not garbage.
    (*vm).pc = d.resume_pc;

    // 3. Re-enter the interpreter. Note the `pc` is set *before* the call,
    //    so the first dispatch does `CONST_INT` at `resume_pc` and the
    //    partially-executed opcode is re-executed from its start. This
    //    requires that **every deopt site be at an opcode boundary**, which
    //    is why deopt sites are emitted at guard failures (which are always
    //    at the start of an opcode's semantics) and never mid-opcode.
    (*vm).tier = Tier::Interpreter;    // one-way, §2.3
    (*vm).run_from_current_frame();    // never returns to the compiled frame
    unreachable!()
}
```

**Step 2 is the answer to "what if the JIT is wrong?"** A miscompiled field access that reads the
wrong slot produces a wrong `Value` *before* the deopt, so the frame map re-check catches a wrong
*type* but not a wrong *slot*. That is the honest limit: **a wrong slot is a wrong answer, and no
deopt mechanism can fix it.** What §1.2's rule and §11's tests *do* is bound how wrong it can be:
every slot read is a *slot of an object whose shape was checked*, so a wrong slot is a slot of the
wrong field of the right kind of object — a `Value` with a valid tag, so the interpreter's next
`TYPE_GUARD` will catch a type error, and a value error is a Zen `error` rather than a crash. That
is the strongest available guarantee for a machine-code JIT, and §11 test 4 is what demonstrates
it (it asserts a Zen `error`, not a segfault, for a deliberately miscompiled site).

### 7.4 On-stack replacement: the feature M11 does *not* have

A loop that is entered cold and iterated ten million times will have triggered compilation (§5.1)
*after* its first iteration, so the interpreter runs the first ~1 000 iterations and the compiled
code runs the rest — but only after a `CALL` re-enters the function. **A function currently
executing in the interpreter is never migrated to compiled code mid-loop.** OSR is therefore
absent, and the consequence is a measurable one:

- A `while` loop that dominates a function but whose function is called once pays 1 000
  interpreter iterations before compiling. At [E] 2.4 ns/op that is 2.4 µs — invisible.
- The `startup` benchmark (§2.4) is *exactly* this case, which is why it is a [T] gate.

**Ruling: no OSR in M11.** OSR needs a stack walk, a state-reconstruction-at-arbitrary-point
mechanism, and a live range fixup across a mid-stack-entry — the hard part of any JIT. Adding it
would roughly double M11's risk for a [E] 2.4 µs win. It is on §12.2's "if M11 is to be continued"
list, and it is the first thing to add.

### 7.5 The recompile cap, and deopt thrashing

A function whose ICs keep getting invalidated (a dictionary that alternates between two shapes, or
a polymorphic call site) will deoptimize, recompile, deoptimize again. **The cap:**

```rust
/// M11's anti-thrash rule, and it is a [T] not a nicety: §11 test 10
/// runs a deliberately thrashing program and asserts it completes.
const MAX_RECOMPILES: u32 = 3;

/// After `MAX_RECOMPILES`, the proto is marked uncompilable and stays at
/// tier 0 forever. A function that deoptimizes 3 times is a function
/// whose assumptions are wrong in a way more compilation will not fix —
/// it is a *polymorphic* function, and it belongs in the interpreter, where
/// a dispatch per opcode is 1.5 ns and a bad assumption is 2.4 ns.
```

`MAX_RECOMPILES = 3` rather than 1 because the first deopt is often *legitimate* — the IC was
compiled before a second shape was seen, which is exactly what §3.3's loop expects — and the
second is usually the last. Three is where the loop is provably spinning.

### 7.6 A deopt makes the old code's cost visible

Every compilation retains its code region for the VM's lifetime (§9.2), so **a thrashing program
leaks code**. Bounding it: `MAX_RECOMPILES = 3` bounds compilations per proto, and a program with
10 000 protos that each recompile 3 times retains 30 000 code regions, [E] 1–3 MB total. That is
acceptable, and `zen.jit_stats()` reports it (§12.2's `jit.code_bytes`).

---

## 8. Registers, ABI, and the entry point

### 8.1 The compiled function's signature

```rust
// The Cranelift signature of every compiled Zen function.
//
//   fn(vm: &mut Vm, argv: *mut Value) -> Value
//
// Exactly ONE parameter pointer, and it is the *callee's* frame base
// address, not a Zen register index. This is the same convention as
// 08-vm.md §3.4's `CALL` (which passes a `&mut [Value]` window) and it
// is what makes the deopt frame map (§7.2) a set of offsets into one
// argument rather than a reconstruction of a frame.
//
// `&mut Vm` is `vm: i64` in the Cranelift signature: a `&mut Vm` is a
// pointer, and treating it as an opaque i64 keeps `mutability` out of
// the IR. The *deopt stub* is the only code that casts it back.
fn cranelift_signature(&self) -> Signature {
    let mut sig = Signature::new(CallConv::SystemV);   // §8.2's ruling
    sig.params.push(AbiParam::new(ptr_ty(self.vm_ty)));  // &mut Vm
    sig.params.push(AbiParam::new(ptr_ty(self.vm_ty)));  // argv: *mut Value
    sig.returns.push(AbiParam::new(self.val_ty));       // Value, by value
    sig
}
```

**`Value` is returned by value in a register, and `Value` is 8 bytes.** That is the one ABI
decision with a portability caveat: the x86-64 and AArch64 ABIs both return a 64-bit integer in
`rax`/`x0`, so an 8-byte struct return is free on both. A 16-byte struct would not be, and
`StringObj` ([`09-memory.md`](09-memory.md) §5.5) is 16+ bytes — **which is why the `StringObj`
change is a representation change that must not leak into the JIT's ABI**, and why the JIT reads a
string's length with one `mov` at a fixed offset (§5.5 of that document's table) instead of
returning it in a register pair. The two documents' §5.5 rulings are coupled for this reason and
that is the concrete reason they ship as one change.

### 8.2 The call convention ruling

> **Ruling: `SystemV` (x86-64) and `AAPCS64` (AArch64) — the platform's native C convention — for
> the compiled function's own signature. Not a custom convention.**

| Option | Verdict |
|---|---|
| Platform C ABI | **Adopted.** Compiled code can be called from Rust, from a `cc`-compiled C extension, and from a debugger, with no shim. `zen`'s `ffi` design ([`08-vm.md`](08-vm.md) §8.1) needs `extern "C"` compatibility, and a custom convention would make a compiled function uncallable from an FFI shim. |
| A custom "fast" convention (more registers, no callee-saved) | **Rejected.** Saves [E] 2–3 `push`/`pop` per call, and costs the entire `extern "C"` interop story. For `fib` (§2.4) the call is not the bottleneck after whole-function compilation removes the dispatch, so the [E] 3% win is not worth losing FFI. |
| Tail calls between compiled functions | Cranelift emits them for a `RETURN` from a tail position. **Allowed** — and safe, because the frame is destroyed and the deopt stub is never entered from a tail frame (§7.3's deopt is a `!` function that does not return, so it cannot be reached *after* the frame is gone). |

### 8.3 The Zen register file, spilled

[`08-vm.md`](08-vm.md) §2.1 gives the VM 256 `Value` registers as an array in the `Vm` struct. The
interpreter indexes it with `get_unchecked`; the JIT cannot, because `vm` is opaque `i64` and the
offset is not a link-time constant. So:

| Register kind | Compiled representation | Cost |
|---|---|---|
| The interpreter's `registers[0..256]` | **the same array in the same `Vm`**; the JIT addresses it as `[vm + off]` with `off` an immediate, where `off = offset_of!(Vm, registers)` | one add per access, or folded into the addressing mode by the RA for free |
| Locals beyond the first few | spill slots in the Rust frame (`[Value; N]`, addressed as `[rsp + k]`) | one `mov` each way |
| The **accumulator** (`acc` in 08-vm.md §3) | a real Cranelift register across an entire basic block | free — this is where the win is |
| Operand registers | real Cranelift registers while live, spilled when the RA decides | the RA's problem |

**`retain` and `release` are calls, not instructions.** §6.3's step 4 emits a `call` to a Rust
builtin for `retain`. This is a real cost — an indirect or direct call per field access — and it is
worth being explicit that **the JIT does not eliminate refcount traffic**; it *relocates* it into
a builtin call that is a `#[inline]`-able function in the same binary. In practice the call is
tail-call-optimised into a `lock incq [mem]` in the release build, and §12.1's `list_ops` target
(≥ 1.3×, the lowest of the six) accounts for it. **A JIT that claims to remove `retain` entirely
is either wrong or has reimplemented the ownership model**, which is an M10 ADR, not an M11
optimization.

### 8.4 The entry point contract

```rust
// The call from the interpreter's CALL arm (08-vm.md §3.4) into compiled
// code. This is the *only* interpreter→JIT transition, and it is a normal
// function call — no setjmp, no stack switching, no context object.
//
// Preconditions the caller must establish (and §11 test 3 verifies):
//   * `argv` points at `argc` valid `Value`s in the callee's window
//   * the callee's window is sized for the frame map's max spill index
//   * `vm.current_proto` is set, so a deopt (§7.3) knows which frame map
#[inline(always)]
pub unsafe fn call_compiled(vm: &mut Vm, slot: &CodeSlot, argc: u16) -> Value {
    let Some(entry) = slot.entry() else { unreachable!("caller checked the slot") };
    let f: extern "C" fn(*mut Vm, *mut Value) -> u64 = std::mem::transmute(entry.as_ptr());
    f(vm as *mut Vm, vm.frames.last_mut().window.as_mut_ptr())
}
```

**The `transmute` is the one genuinely frightening line in the document**, and it is confined to
one function with three properties that make it acceptable:

1. The signature is fixed by §8.1 and the platform C ABI (§8.2), so it matches the emitted code
   by construction.
2. `slot.entry()` returns a pointer only ever produced by
   `JITModule::get_finalized_function` (§4.1) and only after `finalize_definitions` (§9.1), so
   it points at Cranelift-emitted, finalized machine code.
3. It is a `Value` in, a `Value` out, both 8 bytes on two ABIs where that is free — so there is
   **no hidden struct-return convention to get wrong**, which is the usual way a `transmute` of a
   function pointer goes wrong.

If §12.1's targets are missed on a new platform and the `SystemV` choice is the reason, the
fallback is a small `extern "C"` shim generated per platform — not a different ABI. §12.2 notes
it.

---

## 9. Memory management for generated code

### 9.1 RW, then RX, and the exact sequence

`JITMemoryProvider` (§4.1) is the one experimental-crate API this project *depends on* rather
than merely uses, because it is what makes the permissions sequence ours to control:

```rust
// zen-vm/src/jit/mem.rs  — implements cranelift_jit::memory::JITMemoryProvider

pub struct CodeMemory {
    /// Executable mappings only. `PROT_READ | PROT_EXEC` in its final
    /// state; `PROT_READ | PROT_WRITE` while Cranelift is emitting into it.
    map: Mutex<HashMap<usize, Mapping>>,
}

struct Mapping { base: *mut u8, size: usize, executable: bool }

impl JITMemoryProvider for CodeMemory {
    /// Cranelift asks for `size` writable bytes. Return a
    /// `JITMemory { ptr, size, executable: false }`.
    fn allocate_memory(&mut self, size: usize) -> Result<JITMemory> {
        // 16-byte alignment: the code region must satisfy whatever
        // instruction alignment the ISA needs, and a page boundary
        // inside an instruction stream is a segfault that looks like a
        // compiler bug. `page_size` is the natural granularity for mmap;
        // the *instruction* alignment is 16, satisfied by construction.
        let base = mmap_writable(size)?;
        self.map.lock().insert(base as usize, Mapping { base, size, executable: false });
        Ok(JITMemory { ptr: base, size, executable: false })
    }

    /// Cranelift has finished writing. Flip to `PROT_READ | PROT_EXEC`.
    ///
    /// # Safety-critical, and this is the order that matters:
    ///   1. The bytes must be **fully written** before the `mprotect`.
    ///      `finalize_definitions` is that barrier (Cranelift calls this
    ///      after every `define_function`).
    ///   2. The `mprotect` must be `PROT_READ | PROT_EXEC`, **never**
    ///      `PROT_WRITE | PROT_EXEC`. W+X is a security hole (any
    ///      injected pointer becomes arbitrary code) and it is also
    ///      forbidden by `SELinux`'s `execmod` and by hardened kernels'
    ///      `vm.mmap_rnd_bits`. The code is emitted into a *separate*
    ///      mapping from any Zen data, so a Zen bug cannot turn a data
    ///      write into a code write.
    ///   3. Never `PROT_NONE` a region that any thread might be
    ///      executing in. §9.2 makes the unmap happen only at teardown.
    fn finalize_memory(&mut self, ptr: *mut u8, size: usize) -> Result<(JITMemory, bool)> {
        mprotect_rx(ptr, size)?;                    // step 1 then 2
        if let Some(m) = self.map.lock().get_mut(&(ptr as usize)) { m.executable = true; }
        Ok((JITMemory { ptr, size, executable: true }, /* writable_before: */ false))
    }

    /// Free. **Only called for a region that was never made executable**,
    /// because an executable region is owned by a `JitModule` and is
    /// released in `CodeMemory::drop` (§9.2), not here.
    fn free_memory(&mut self, ptr: *mut u8, size: usize) {
        debug_assert!(!self.map.lock()[&(ptr as usize)].executable,
                      "cranelift-jit asked to free executable code");
        munmap(ptr, size);
    }
}
```

**The permissions state machine**, which is the whole of "memory management for generated code":

```text
   mmap(PROT_READ|PROT_WRITE)          finalize_definitions()          module drop
        │                                      │                             │
        ▼                                      ▼                             ▼
   ┌────────────┐  bytes written  ┌──────────────────┐  never (immutable)  ┌────────┐
   │  RW        │ ─────────────▶  │  RX (published)  │ ─────────────────▶ │ unmapped│
   │  private   │                │  execute-only-ish │                   │        │
   └────────────┘                └──────────────────┘                   └────────┘
        │                                                                  
        └── free_memory() only reachable here (a failed compile)            
```

**There is no RW→RX→RW cycle, ever.** That is §3.1's ruling expressed as an API shape: once a
region is executable, no code path makes it writable again. §4.1's absence of `hotswap` and
`prepare_for_function_redefine` is what makes this diagram the only one, and it is the reason
`free_memory`'s `debug_assert` can be an assertion rather than a hope.

### 9.2 Lifetime: why nothing is ever unmapped during execution

> **Ruling: executable code is unmapped only at VM teardown, after every thread has joined.**

The reason is a plain race with no safe version:

| Attempt | Why it fails |
|---|---|
| Unmap the old module when a `CodeSlot` is republished | a thread may hold `entry` from §3.2 in a register and be mid-call. Unmapping is `munmap`, which removes the pages; the instruction fetch faults. **The window is unbounded** — the thread can be descheduled between the load and the call. |
| Reference-count the code region | works *only* if every path that could execute from it holds a reference, and the `entry` pointer is held in an arbitrary number of compiled frames' registers and spill slots, with no way to enumerate them. A `CodeSlot`-level refcount does not count *executing* uses. |
| `mprotect(PROT_NONE)` the old region | identical fault, one instruction later. |
| **Never unmap until teardown** | **adopted.** At teardown, `Vm::drop` runs after `vm.shutdown_threads()` ([`08-vm.md`](08-vm.md) §3.7) has joined every thread and every JIT frame has returned. Nothing can be executing. |

**The cost is bounded and it is a [T]:** code regions total [E] 1–3 MB for a program that compiles
100 protos (a compiled function is 1–4 KiB), `MAX_RECOMPILES = 3` (§7.5) triples the worst case,
and `jit.code_bytes` (§12.2) reports it. For a script that compiles 100 protos and exits, the
[E] 1–3 MB is *never* touched again, so it is virtual address space and page-table entries, not
resident pages — and the pages that were executed are resident anyway. **The 64-bit address space
is not the constraint; 48 bits of canonical address space with a 128 TiB user-space limit gives
~40 000 000 protos' worth of 4 KiB regions before exhaustion, and no program has that many
functions.**

### 9.3 Non-JIT allocations: constants, jump tables, and deopt side tables

Code is not the only thing the compiler emits:

| Allocation | Where it lives | Lifetime | Deopt relevance |
|---|---|---|---|
| Machine code | the `JITMemory` mapping, RX | VM | the entry pointer |
| The `DeoptSite` array (§7.2) | a `Vec<DeoptSite>` in the `FunctionProto` | VM | indexed by the site immediate |
| The Cranelift `safepoints` table (§7.2) | the `Compiled` struct, kept alive for the VM | VM | **shared with M9's collector** |
| Cranelift constants (float pools, `bigint` immediates) | `JITModule`'s own data, resolved via its symbol table | VM | — |
| The IC array | the interpreter's, `&mut [IcEntry]` | VM | read by the lowering (§5.3's `ic: &IcArray`) |

**Every one of these is a Rust `Vec` or a `cranelift` data section, and none of them is
`mmap`ed by us.** They are ordinary allocations, so [`09-memory.md`](09-memory.md) §3.4's rule
applies: `DeoptSite` contains no `Value`s (it is `u32`s, `Location`s, and an `Option<u32>`), so it
has no `retain`/`release` obligation, and it is therefore a leaf in §7.2's root table — **a fact
worth stating, because it is the only reason a JIT can coexist with a tracing collector without a
new rooting mechanism.**

---

## 10. The interpreter fallback and feature detection

### 10.1 The feature flag, and the build matrix

```toml
# zen-vm/Cargo.toml
[features]
default = []
# M11. Off by default. §1.2's commitments: the interpreter is unchanged,
# the bytecode is unchanged, and every language test passes with it off.
# Sharing the flag with zen-value's `StringObj` (09-memory.md §5.5) is
# deliberate — they are one change, for the same reason: the JIT needs a
# stable, cheaply-readable string layout, and `Arc<str>`'s length is
# behind a thin pointer.
jit      = ["dep:cranelift-codegen", "dep:cranelift-native", "dep:cranelift-jit", "zen-value/jit"]
jit-stats = ["jit"]        # §12.2's counters, off even when jit is on
```

| Build | Runs | Expected |
|---|---|---|
| `cargo test` (default) | the interpreter, the full language suite | all green; identical results to the JIT build |
| `cargo test --features jit` | the same suite; a jit-build may use tier 2 | all green, **byte-identical stdout** (§11 test 4's assertion) |
| `cargo test --features jit-stats` | as above plus `jit_stats()` assertions | green |
| `cargo build --target aarch64-unknown-linux-gnu` (with `jit`) | the same suite under qemu | green; §8.2's AAPCS64 path |

**The last row is not optional.** §8.2 chose the platform C ABI "for both", and a "for both" claim
that is never run on the second architecture is a guess. M11's definition of done includes the
AArch64 CI job, and the same `[M]` API facts in §4.1 mean the Cranelift side is already portable;
only the register map and the `ret` instruction differ, and Cranelift emits those.

### 10.2 Runtime feature detection

Some CPUs lack instructions the target ISA assumes, so:

| Check | How | Fallback |
|---|---|---|
| AVX2 / AVX-512 | `is_x86_feature_detected!` | `Cranelift`'s `builder()` returns an ISA with only SSE2; the *compiler thread* picks the ISA per process, and `zen.jit_info()` reports it |
| The host pointer width | `cfg!(target_pointer_width)` | 32-bit: the JIT is **disabled at runtime** with a `jit_info` message, because `Value`'s 48-bit payload ([ADR-0001](decisions/ADR-0001-tag-budget.md)) and `AtomicPtr<u8>`'s [E] 8-byte assumption in §3.2 are both 64-bit-specific |
| W^X enforcement | detected by a test allocation, not a feature check | if `mprotect` to RX fails, compilation is disabled at runtime and the interpreter is used. **Tested at startup, once, in `zen.jit_init()`** |
| Cranelift's own support for the host triple | a `JITBuilder::new()` at init | if it fails, the JIT is off and `jit_info` says why |

**Runtime disabling must be total, not partial.** If any check fails, `jit_state()` is `Disabled`
and every `CodeSlot::entry()` returns `None` forever, so the interpreter path is taken
unconditionally. There is no "some protos compiled" state, because that state is where the
§7.3 frame-map bugs live.

---

## 11. Test plan

Fourteen tests. The first four are the correctness argument; the rest are the engineering ones.

| # | Test | Fails if | How |
|---|---|---|---|
| 1 | `jit_parity_strings` | compiled and interpreted output differ on any program | the whole `zen-tests` corpus, run twice, stdout compared byte-for-byte with `--features jit` |
| 2 | `jit_guard_catch` | a guard is missing: a deliberately type-punned field access reaches the compiled path | a hand-written Zen program that stores an `int` where an `instance` is expected via `zen.ffi`; asserts the interpreter's `error`, and asserts it is a Zen `error` and not a segfault |
| 3 | `edge_int48` | the promote-to-`float` path is missing, mis-ranged (a 64-bit overflow test where an i48-range one is needed), or absent from the deopt frame map; or `idiv` traps on `INT_MIN / -1` (§6.4) | a table of `{INT_MAX, INT_MIN, 0, 1, -1, 2⁴⁷−1, −2⁴⁷, 2⁴⁶, 2⁴⁶×2, 2⁴⁷}` crossed with every arithmetic opcode and both sign conventions for `IMOD`, in both configurations, comparing **`to_bits()` and the tag**, not the printed value |
| 4 | `parity_fuzz` | **any** miscompile | 10⁹ randomised well-typed Zen programs (a small grammar generating `if`/`while`/field access/arith/calls) executed in both configurations, results compared; run nightly, ~10 min on the reference machine |
| 5 | `idiv_zero_parity` | compiled `idiv` by zero traps or returns a different error | `zen.idiv(1, 0)` and `zen.idiv(0, 0)` in both paths |
| 6 | `compile_failure_is_not_fatal` | a `Err` from `compile_proto` propagates as a program error | a build with a deliberately broken lowering (`#[cfg(test)]`), asserting the program's answer is unchanged and `jit_stats` records the failure |
| 7 | `channel_not_inlined` | a blocking op was inlined, so a safepoint is missed | a program that receives on a channel while a GC is running; the test hangs (with a timeout) if the safepoint is missing |
| 8 | `safepoint_at_back_edge` | the safepoint call was elided by a later pass | a `while` loop with a `defer` in the body, under the gc build; the deferred code must run every iteration |
| 9 | `miri_interpreter_core` | any UB in the interpreter's `unsafe` reached via a deopt | `cargo +nightly miri test -p zen-vm` with the jit feature off — this is [`09-memory.md`](09-memory.md) §9 test 18 and it is *not* replaced by any JIT test, because Miri cannot run generated code |
| 10 | `deopt_thrash_bounded` | the recompile cap does not bound work | a program alternating two shapes on one field; asserts it completes and `jit.compiles == 3 * protos` |
| 11 | `wx_permissions` | code is ever W+X, or a mapping is unmapped while executable | a build asserting `mprotect` to `PROT_WRITE|PROT_EXEC` **fails** (an `mmap` guard page plus an `mprotect` that must be rejected), plus an assertion that no region is `PROT_WRITE` when published |
| 12 | `aarch64_parity` | the AAPCS64 path diverges | the full corpus on `aarch64-unknown-linux-gnu` under qemu, compared against the x86-64 results |
| 13 | `startup_regression` | the JIT makes startup worse than 10% | `zen-bench startup` with and without `jit`; the [T] gate of §2.4 |
| 14 | `no_ic_dependency` | compiled code reads an IC that has been invalidated | an IC invalidation between two calls to a compiled function; the second call must re-derive, not reuse |

**Test 4 is the one that matters, and it is worth being honest about its limits.** 10⁹ random
programs of a small grammar will cover the *lowered* opcodes well, because the grammar is chosen
to hit them. It will not cover:

- **long chains of deopt-then-recompile** (test 10 covers that separately);
- **any interaction with `ffi`, channels, or the scheduler** (tests 7, 8, 11 cover pieces);
- **register-allocator pressure**: a program with 50 live values across a call, where the RA
  spills. Test 4's generator should therefore include functions with 20–50 locals, which is a
  specific grammar extension and the [E] most likely place a frame-map bug hides.

So test 4 is necessary and not sufficient, and a reviewer should reject any claim that the JIT is
"verified" on its basis. The honest claim is "verified on the lowered opcode set, with the
deopt, safepoint, and FFI interactions covered by dedicated tests".

---

## 12. Metrics, the M11 gate, and reviewer rulings

### 12.1 The performance table, restated as CI gates

| Benchmark | [T] target | Gate | Notes |
|---|---|---|---|
| `fib` | ≥ 2.5× vs tier 1 | `zen-bench fib --check` | the clearest win; the call sequence is the work |
| `loop_sum` | ≥ 1.5× | `--check` | after paying a safepoint call per iteration (§7.1) |
| `json_parse` | ≥ 2.0× | `--check` | includes the `StringObj` change ([`09-memory.md`](09-memory.md) §5.5) |
| `nbody` | ≥ 2.0× | `--check` | |
| `list_ops` | ≥ 1.3× | `--check` | the lowest, because `retain`/`release` remain calls (§8.3) |
| `startup` | ≤ 1.1× (≥ 0.9×) | `--check` | **the "may not regress" gate** |
| geometric mean | ≥ 1.9× | `--check` | the headline number for the ADR |

A missed target is **not a release blocker for the language**. It is a signal to either add a
lowering, revisit the target, or (per §1.2) drop the feature. Only `parity_fuzz`, `wx_permissions`,
and `aarch64_parity` are correctness gates, and all three are pass/fail with no target.

### 12.2 The metrics a JIT needs that an interpreter does not

| Metric | Kind | Why |
|---|---|---|
| `jit.compiles` | counter | compile volume; bounded by `MAX_RECOMPILES` |
| `jit.code_bytes` | [E] gauge | the §9.2 leak, bounded at [E] 1–3 MB typical |
| `jit.compile_us` | histogram | the §5.1 TIER_UP justification, in data |
| `jit.deopts` | counter, by site | the only way to know whether the guards are right |
| `jit.deopt_sites` | histogram | *which* site deoptimizes; a hot site is a missing lowering |
| `jit.tier_moves` | counter | should be 0 in steady state |
| `jit.failed_compiles` | counter | should be 0 in a release build; non-zero means §5.3's `Unsupported` set is larger than expected, which is a *performance* finding, not a bug |
| `jit.ic_hits_after_compile` | ratio | does compilation make the *interpreter* faster too? (It should not, since compiled functions stop running the IC — a surprising non-zero value means compiled and interpreted copies of the same code are both running, which is normal, and the ratio tells us if the IC path is still hot elsewhere.) |
| `jit.safepoint_us` | histogram | the [E] 50 ns call per back edge; if it is 500 ns, §7.1's design is wrong and the collector will stall |

**The metric that predicts everything else is `jit.deopt_sites`.** A JIT where one site
deoptimizes 90% of the time is a JIT that has found a missing lowering, and the fix is a lowering
not a heuristic. That is why §12.1's targets are worth chasing only *after* `deopt_sites` is flat.

### 12.3 The dependency policy, as a CI check

| Check | Command | Fails on |
|---|---|---|
| Version pin | `grep -A2 'cranelift' Cargo.toml` | `cranelift-jit` outside `^0.136` |
| No new API surface | `grep -rn 'cranelift_jit::' zen-vm/src/` | any use beyond the four items in §4.2 — this is the guard that makes §1.3's "extremely experimental" a *policy* rather than a worry |
| No `hotswap` | `grep -rn 'hotswap\|prepare_for_function_redefine'` | a resurrection of the idea, which would need §3.1 re-litigated against a version that has it |
| Upgrade smoke test | `cargo update -p cranelift-jit --dry-run` in a weekly job | a newer 0.136.x that fails to build — a *signal to read the changelog*, not a failure |
| Corpus parity on upgrade | the full corpus on the newest 0.136.x, weekly | a behaviour change in Cranelift; the response is a pin, and the pin is written down in the changelog entry |

The last row is the one that justifies §4.1's citation of Wasmtime's removal: an experimental crate
can change behaviour between patch releases, and the *only* defence is a weekly corpus run.

### 12.4 Reviewer rulings

| # | Ruling | Where | The question | Cost if wrong |
|---|---|---|---|---|
| 1 | **Whole-function compilation, not hot-block/trace** | §2.1 | Is a 3–5× target acceptable, given LuaJIT's 10–20×? | High if the user wanted a real JIT: a re-design of the unit of compilation is a new milestone, not a patch |
| 2 | **No in-place patching; `AtomicPtr` publication** | §3.2, §3.3 | Is a `cmp` per field access an acceptable substitute for hot-swap? | Low: it is 1 predictable branch, and it is forced by 0.136's API (§4.1) |
| 3 | **Nothing executable is ever unmapped before teardown** | §9.2 | Is [E] 1–3 MB of retained code acceptable for a scripting language? | Low: bounded, reported, and unmapped at exit |
| 4 | **One-way deopt, no re-optimisation, no tier 3** | §2.3, §7.5 | Is a 3-recompile cap and permanent fallback to tier 0 enough? | Medium: a pathological program is permanently slow, but bounded |
| 5 | **No OSR** | §7.4 | Is paying 1 000 interpreter iterations per cold loop acceptable? | Low: [E] 2.4 µs; but it is the reason `startup` regresses, and that is a [T] gate |
| 6 | **Platform C ABI, not a custom convention** | §8.2 | Is the [E] 3% loss worth `extern "C"` interop? | Low, and the answer is yes given the `ffi` design |
| 7 | **A safepoint call at every back edge and call site** | §7.1 | Is one call per loop iteration too expensive for `loop_sum`'s 1.5×? | Medium: it is the difference between the 1.5× target and 3×; if it matters, the answer is a **coalescing safepoint** (a flag check every N edges), which weakens the M9 collector's latency bound and should be rejected |
| 8 | **`retain`/`release` remain calls in compiled code** | §8.3 | Is a JIT that does not eliminate refcount traffic worth building? | Medium-high if the user wanted "no atomics"; that is an M10 ownership-model ADR, and saying so now is the right time |
| 9 | **The `StringObj` representation change ships with the JIT** | [`09-memory.md`](09-memory.md) §5.5 | Is coupling the two changes right, or should they be separate? | Low: they are both M11-gated and share the reason; separating them means two ABI-incompatible representation changes |
| 10 | **A compile failure is a permanent tier-0 fallback, not an error** | §5.2 | Is silently degrading acceptable, or should a failed compile be a warning? | Low: a warning on every call would be worse; `jit.failed_compiles` (§12.2) is the right channel |
| 11 | **Test 4's 10⁹ fuzz is necessary but not sufficient** | §11 | Is accepting that limit honest, or is a bigger verification effort needed before M11 ships? | Medium: a miscompile in an interaction test 4 cannot reach is a wrong answer for a user |
| 12 | **The JIT depends on a four-method surface of an "extremely experimental" crate** | §1.3, §4.3 | Is depending on `cranelift-jit` at all acceptable, given its own README? | High if no: the alternative is writing an x86-64 backend (a multi-milestone project) or shipping only tier 1. **The escape hatch is real and cheap: delete the `jit` feature, and the language is unaffected.** That asymmetry is the strongest argument for building it at M11 |
| 13 | **M11 comes after M9's gate, so the JIT's value proposition is unmeasured** | §1.1 | Should the JIT be attempted before the collector, on the theory that it helps more? | Medium: the [T]s in §12.1 assume the interpreter they replace is the one M10 ships. If M9 changes the allocation path's cost profile, these targets move. **Recording the targets now, and re-deriving them at M11, is the plan** — that is why §12.1 is per-benchmark |
| 14 | **`int` overflow promotes to `float`, and the compiled code must implement that** | §6.2, §6.4 | ADR-0001 and 08-vm.md §4.3 are unambiguous here, and it is the opposite of every stock JIT's `iadd`. Is a promote-to-`float` block on the cold path of every integer op acceptable, or should `int` be redefined to wrap? | **High if the ruling changes**: wrapping is one `ireduce`, promotion is a test plus a block plus a frame-map subtlety. But promotion is canonical and it is *observable user-visible semantics* (`typeof(1 + MAX) == "float"`), so it is **not the JIT's to change.** Recorded here only because the JIT is where the cost of the language's choice becomes visible |

**Action items this document creates for other documents:**

1. **`docs/01-principles.md` and `docs/04-value-and-types.md` do not exist.** This document links
   to them relatively; whoever writes them should link back here for §8.3's register-file
   representation and for [`09-memory.md`](09-memory.md) §2.4's tag table.
2. **`07-bytecode.md` §4's `DEOPT` / `SAFEPOINT` / `TYPE_GUARD` opcodes gain a fourth consumer**
   (the JIT's guard emission) beyond the interpreter's. No change to their semantics is requested;
   if their encoding lacks a *deopt site number* field, that is the one change to request.
3. **`09-memory.md` §7.2 row 18 depends on §7.2's `safepoints` table here.** The two must land
   together, or the collector and the JIT each assume the other's frame map exists.
4. **An ADR-0005 for the JIT strategy should be written when M9's gate opens**, not before,
   because its central question — "is the dispatch loop the bottleneck?" — is not answerable
   before then (§1.1's table).

---

*Related: [`00-blueprint-review.md`](00-blueprint-review.md) §A6 (JIT last, optional) ·
[`08-vm.md`](08-vm.md) §3 (the dispatch loop this replaces), §5 (semantics that must be preserved
exactly, §6.4 of this document's checklist), §6 (inline caches, §6.3's shape `cmp`), §7 (shapes),
§8.1 (the `ffi` ABI §8.2 preserves) · [`07-bytecode.md`](07-bytecode.md) §3 (register bytecode),
§4 (`DEOPT` / `SAFEPOINT` / `TYPE_GUARD` / `JMP_BACK`), §11 (escape analysis) ·
[`09-memory.md`](09-memory.md) §5.5 (the `StringObj` change this gates), §7.1–§7.2 (the collector
this must not break, and the shared frame map) ·
[`decisions/ADR-0001-tag-budget.md`](decisions/ADR-0001-tag-budget.md) (§8.1's 8-byte `Value` ABI
assumption) · [`decisions/ADR-0002-memory-model.md`](decisions/ADR-0002-memory-model.md) ("the JIT
work in M11 is worth more than cycle collection")*
