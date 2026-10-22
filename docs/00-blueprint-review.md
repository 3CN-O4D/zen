# 00 — Engineering Review of the Zen Blueprint

> **Status:** Accepted with mandatory amendments.
> **Date:** 2026-09-29
> **Scope:** Review of the "Zen Fast Stack" blueprint and the "Zen Language Engine: Comprehensive
> Engineering Specification" (16-tag NaN-box, arena GC, register VM, background Cranelift JIT,
> FFI + 7-layer net, module system, `zenn` toolchain).

The blueprint is a good sketch. Seven of its ideas are load-bearing and survive review. Six of its
specific mechanisms are wrong or unsafe as written, and building them literally would produce a
runtime that corrupts memory, deadlocks, or cannot compile. This document records what we keep, what
we change, and **why**, so the decisions are auditable rather than tribal.

**Project goal, restated as a testable claim:**

> Zen must be **faster than CPython** on the benchmark suite in [`18-benchmarks.md`](18-benchmarks.md),
> and must **never** abort the process on user input. Every failure mode is a catchable Zen error.

Stability is not a nice-to-have here. A JIT with a wrong pointer tag is not "fast with occasional
bugs", it is silent memory corruption. Every amendment below is chosen to *reduce* the number of
things that can go wrong while still hitting the speed target.

---

## 1. What we keep, unchanged

| # | Blueprint idea | Verdict | Why it survives |
|---|---|---|---|
| K1 | Pack all values into one 64-bit word | **Keep** | Removes a pointer chase and a discriminant byte from the hottest path in the machine. This is how LuaJIT, SpiderMonkey and V8 all work. |
| K2 | Keep integer and float as separate machine representations | **Keep** | `i44 + i44` is one `iadd`. `f64 + f64` is one `fadd`. If every number were `f64`, integer loops pay float semantics and cannot be JIT-specialised. |
| K3 | A 16-type language surface, no more | **Keep** | Small type sets mean the JIT can specialise aggressively and the error messages stay short. |
| K4 | A bytecode VM as the single execution engine | **Keep** | One engine, not "AST walker + bytecode VM". Zen 2.x currently has *two* engines (`exec_source_tree` vs `exec_source_cached`) and a differential test harness exists purely to prove they agree. That is a bug factory. |
| K5 | Tiered execution (interpreter first, compile hot functions) | **Keep** | Correct strategy. Cold code should not pay compile cost; hot code should not pay dispatch cost. |
| K6 | Compile on a background thread, never on the executing thread | **Keep** | Removes JIT stutter entirely. Confirmed feasible: `cranelift-jit` exposes `JITBuilder::hotswap` and `JITModule::prepare_for_function_redefine`. |
| K7 | FFI handles as opaque IDs rather than raw pointers in the language | **Keep** | Languages that expose raw pointers (C, and `ptr->field` in the original draft) cannot be made memory-safe. Opaque handles are the correct call. |

---

## 2. Amendments (mandatory)

### A1 — Tag budget: 8 machine tags, not 16. The language still has 16 types.

**Blueprint says:** 4 tag bits at `TAG_SHIFT = 44`, giving a 44-bit payload, 16 machine tags.

**Why this is wrong.** The payload must hold a **pointer**, and pointers on x86-64 are not reliably
44 bits. The Linux kernel's own 5-level paging documentation is explicit:

> "5-level paging enables 56-bit userspace virtual address space. Not all user space is ready to
> handle wide addresses. **It's known that at least some JIT compilers use higher bits in pointers
> to encode their information. It collides with valid pointers with 5-level paging and leads to
> crashes.**" — `Documentation/arch/x86/x86/5level-paging.rst`

The kernel mitigates this by not allocating above 47 bits *by default*, but explicitly permits
userspace to request higher addresses via an `mmap` hint. A tag scheme that assumes 44-bit pointers
therefore produces **silent memory corruption on a 57-bit address-space kernel** — the exact
"unstable" outcome we are trying to eliminate.

There is a second, independent bug in the original 3-bit/4-bit mismatch: the first draft declared
`TAG_MASK: u64 = 0x0007_0000_0000_0000` (3 bits) while every tag read used `& 0xF` (4 bits). Tags
4–15 would be misread.

**Amendment.** Use **3 tag bits at shift 48**, a **48-bit payload**, and reserve tag `0b111` for
*all* heap objects. The concrete type of a heap object is read from its header, not from the tag.

```
 63    62 ......... 52 51 50 49 48 47 ......................... 0
+---------+---------------+---+---+---+----------------------------+
| S       | Exponent (11) | 1 | T | A |   Tag (3 bits)   |  Payload   |
|         |  = all 1s     | q | a | g |                    |  (48 bits) |
+---------+---------------+---+---+---+----------------------------+
                                            QNAN base = 0x7FF8_0000_0000_0000
```

- `0b000` Nil, `0b001` False, `0b010` True, `0b011` Int (i48, sign-extended), `0b100` Float
  (raw f64 bit pattern), `0b101` String, `0b110` Bytes, `0b111` **Heap** (header discriminated).

The 16 *language* types are unchanged. Nine of them (dict, set, tuple, func, error, channel, mutex,
handle, instance) share machine tag `0b111` and are told apart by `ObjHeader.kind`, a `u8` at
offset 0 of every heap object. One extra load from a cache line we are already touching.

**Cost:** ~2 extra instructions on `is_dict`-style type tests versus 16 distinct tags.
**Benefit:** provably no pointer/tag collision on any 48-bit or 57-bit address space, and 8 spare
tag slots for growth without re-laying-out every `Value`.

**Recorded as:** [`decisions/ADR-0001-tag-budget.md`](decisions/ADR-0001-tag-budget.md)

---

### A2 — No bump arena, no mark-sweep. `Arc` + copy-on-write, and a design for a real GC later.

**Blueprint says:** per-thread bump arenas (`bump_ptr += size`) with mark-and-sweep collection, and
`ArenaChunk::reset()` for "ephemeral heaps".

**Why this is wrong, in three independent ways.**

1. **You cannot sweep a bump arena.** Mark-sweep must free *individual* unreachable objects. A bump
   allocator can only free the whole chunk, or free a trailing region. The draft's
   `objects_head: *mut GcHeader` linked list is the giveaway: a linked list of separately-freeable
   objects is a malloc heap wearing a bump allocator's clothes. The two are mutually exclusive.
2. **`reset()` is a use-after-free generator.** The draft describes it as a "fast zero-cost reset
   for ephemeral heaps". Resetting an arena that still holds live `Value`s makes every surviving
   `Value` a dangling pointer. The first `spawn(fn(){...})` returning a collection would corrupt
   the heap. This is the single most dangerous line in the blueprint.
3. **The root set is wrong.** `collect(&stack, &registers)` scans two slices. A real root set is:
   the VM register file, every live call frame's slots, the constant pool of every function on the
   call stack, the shadow stack for temporaries, all pinned handles, and every object referenced by
   a native frame that has not yet returned. Missing any one of these is a dangling read.

**Amendment.** Phase 1 uses **reference counting with copy-on-write containers**, which is what
Zen 2.x already does correctly (`Value::List(Arc<Vec<Value>>)` + `Arc::make_mut`). This is safe by
construction, leaks only genuine reference cycles, and is measurably fast enough to reach the speed
target. A **tracing GC is a Milestone 9 deliverable**, specified in
[`09-memory.md`](09-memory.md) as a correct two-space copying collector with a *complete* root map,
written behind the same `Gc` trait so the two are interchangeable. If profiling later shows
refcount traffic is the bottleneck, we swap it. We do not write a broken collector up front.

**What we keep from the blueprint:** the *goal* — no global GC lock, no stop-the-world across
threads — and the honest admission that per-thread heaps cost more in complexity than they return.
Zen's real parallelism win comes from having no GIL at all, which needs no custom GC.

**Recorded as:** [`decisions/ADR-0002-memory-model.md`](decisions/ADR-0002-memory-model.md)

---

### A3 — No linear move-semantics on channels. Explicit sharing, deep copy on send by default.

**Blueprint says:** "sending mutable data (list, dict) over a channel **transfers ownership**,
automatically setting the sender's reference to `nil`."

**Why this is wrong.** This is a linear type system. For the sender's variable to become `nil`, the
compiler must (a) know statically that the value is a mutable collection, (b) find every alias of
it, (c) invalidate all of them, and (d) refuse to compile the program if an alias escapes. Dynamic
mutability, `dict.get(k)`, reflection, native modules that retain references, and closures capturing
the value all break that analysis. Worse, it is **fundamentally incompatible with a moving tracing
GC**: a copying collector relocates objects, so "the pointer is now owned by another thread" is not
a stable invariant — the collector would have to know about thread ownership, which is exactly the
global coordination the design claims to avoid.

**Amendment.**

- Immutable-ish values (`string`, `bytes`, `tuple`) are `Arc`-shared across threads at zero cost.
- Mutable containers sent on a channel are **deep-copied** by default. Correct, and cheap enough
  that it is invisible in every realistic script.
- Explicit sharing is opt-in and visible in the source: `shared` marks a binding as
  thread-shared (`Arc<Mutex<T>>`), and a runtime error is raised if a non-`shared` value is used
  from two threads concurrently. See [`12-concurrency.md`](12-concurrency.md).

**Net effect:** races become impossible by default instead of becoming a class of bug the runtime
must detect. The blueprint trades a correctness property for a performance property that nobody
benchmarked.

**Recorded as:** [`decisions/ADR-0003-concurrency-semantics.md`](decisions/ADR-0003-concurrency-semantics.md)

---

### A4 — `handle` is a real type. `typeof()` lies, and that lie will reach the JIT.

**Blueprint says:** tag `0xF` is `handle`, but `typeof()` "reports as `int` for backward
compatibility", justified by "current tests expect `typeof(h) == "int"`".

**Why this is wrong.** A JIT specialises on `typeof`. If `typeof()` returns a lie, the JIT
specialises an integer path for a value that is actually a file descriptor, and the guard is written
against the wrong tag. The failure is memory corruption, not a wrong answer. Compatibility with a
test assertion is not a licence to make the type system lie to the optimiser.

**Amendment.** `handle` is a distinct language type with a distinct `typeof()` result. The legacy
test that asserted `typeof(h) == "int"` is **changed**, not the type system. Handles are still
plain integers underneath (an index into a global table), so they stay cheap; they are just honest
about what they are.

**Recorded as:** [`decisions/ADR-0004-honest-types.md`](decisions/ADR-0004-honest-types.md)

---

### A5 — 32 opcodes is too few, and "26 opcodes" is not a plan.

**Blueprint says:** "[Opcode: u8 | RegA: u8 | RegB: u8 | RegC: u8]", 26 opcodes, a claim of "~40% fewer
instructions than a stack VM".

**Why the encoding is right and the number is wrong.** A 32-bit register instruction is exactly
right — one aligned load, no operand-count dispatch, and the register indices come free. LuaJIT
and Lua 5.4 both use it. But 26 opcodes cannot express a real language: you still need every
comparison, every branch kind, iterator protocol, upvalue access, field access on four container
shapes, exception unwind, tail calls, and the type-guard/deopt opcodes. Zen 2.x's abandoned bytecode
had **78**. The real number for the full language is in the 90–110 range, and pretending otherwise
produces an opcode soup with `ADD_LIST_INT` style specialisations.

**Amendment.** 32-bit register encoding, and a complete opcode table sized from the actual
compiler output, not from an aspiration. Full table in [`07-bytecode.md`](07-bytecode.md).

**The "~40% fewer instructions" claim** is real but is a stack-vs-register claim, not a NaN-boxing
claim. NaN-boxing removes allocation and boxing, not instructions.

---

### A6 — Register VM + inline caches beat JIT for the benchmarks we care about. JIT is Milestone 8, not day one.

**Blueprint says:** five-week build order starting with the JIT in week 2, and "fib(40) = pure
native i64 adds".

**The honest engineering view.** Fib(40) is 1.65 × 10⁸ calls. Any competent register VM does that in
a few seconds; the JIT makes it faster, but fib is a pathological benchmark that measures call
overhead and nothing else. The benchmarks that actually decide "faster than Python" are realistic
loops over lists, dict lookups, string building, and function calls with several arguments — and
those are won by:

- a register VM (fewer instructions),
- unboxed `Value` (no allocation per integer),
- **inline caches** on field access and call sites (the single biggest win in every real VM),
- cheap closure allocation,
- avoiding megamorphic dispatch.

Inline caching is roughly a day's work and buys more on real code than a JIT buys on synthetic code.
The JIT is still worth building — but building it *first* means spending weeks on a machine-code
backend before discovering that your dispatch design was wrong.

**Amendment — revised build order.**

| Milestone | Deliverable | Why first |
|---|---|---|
| M1 | Lexer + AST + parser | Nothing exists without this |
| M2 | `Value` + NaN-box | The representation everything else assumes |
| M3 | Tree-walking interpreter, full language | A correct reference implementation to diff against |
| M4 | Register compiler + VM | **The speed milestone.** Must beat CPython here. |
| M5 | Inline caches + hidden classes | Second biggest speed win, still no codegen |
| M6 | Builtins + core stdlib | Real programs |
| M7 | FFI, handles, `sys`/`os`/`fs`/`time` | Systems access |
| M8 | `zenn` toolchain: run/build/check/lint/test/fmt/repl | Release + distribution |
| M9 | Correct tracing GC behind a `Gc` trait | Only if M4–M5 profiles demand it |
| M10 | Concurrency + 7-layer net | Parallelism and IO |
| M11 | Background Cranelift tier-1 JIT | The last big win, on a known-good VM |

M3 is not throwaway: it is the differential-testing oracle for M4 onward, exactly the way Zen 2.x's
`tests/diff/` harness is used today, except designed in from the start rather than retrofitted.

---

## 3. Things the blueprint got right that are worth calling out

These are non-obvious and often argued against; they stay.

- **Separating `string` and `bytes`.** Most "fast scripting languages" merge these and then pay
  forever. Keeping bytes as a first-class type makes binary protocols (the stated use case: packet
  analysis) natural.
- **`hex` as a *literal syntax*, not a type.** `0xFF` is an integer written in base 16. It costs
  nothing, matches the actual use case, and avoids a whole tag.
- **No `char`, no `uint`, no `i32`.** One integer type, one float type. Correct.
- **The 7-layer networking decomposition.** Layer-by-layer is the right *ordering* of work, and each
  layer maps to a mature Rust crate. What is wrong is implying Zen must *reimplement* the layers; it
  must *wrap* them, which the blueprint does say.
- **`error` as a first-class value, not an exception.** Tag 12 in the spec. Exceptions in a VM cost
  a stack walk and a traceback object. A value-returning error with an explicit `try ... catch`
  sugar costs nothing on the happy path.

---

## 4. Performance: what the numbers actually have to be

"Faster than Python" is not a design goal, it is a measurement. The target suite is in
[`18-benchmarks.md`](18-benchmarks.md); the gate is:

> **Zen must be at least 1.5× faster than CPython 3.13 on the aggregate of `bench_core`, and never
> slower than CPython on any individual benchmark in that group.** A regression on any single
> benchmark is a release blocker.

The 1.5× aggregate with a "never slower individually" clause matters. "40× on fib, 0.2× on string
concatenation" would pass a naive aggregate and still be a worse language than Python. The clause
is the real requirement.

Known areas where a naive port will be **slower** than CPython, which we accept and mitigate:

| Benchmark | Why Zen 2.x-style would lose | Mitigation |
|---|---|---|
| `bench_json_parse` | CPython's `json` is a C extension with 20 years of tuning | Write ours in Rust; do not port the pure-Zen `std/json.z` parser |
| `bench_startup` | CPython starts in ~15 ms; a Rust binary with a large tokio + reqwest dependency graph takes longer to reach `main` | Lazy-init native modules; keep the binary's dynamic-import graph small |
| `bench_closure_alloc` | Arc + CoW containers refcount on every store | Inline caches and, later, escape analysis in the compiler |

---

## 5. Definition of "stable" for this project

The blueprint never defines stability, so we define it. Zen is stable when:

1. **No input can abort the process.** No panics, no `unwrap()` on user data, no arithmetic
   overflow, no index-out-of-bounds, no `unsafe` precondition reachable from a script. The binary
   sets a panic hook that converts any escaped panic into a Zen error with a non-zero exit code.
2. **Every runtime failure is a `ZenError`** with a source span: `TypeError`, `IndexError`,
   `KeyError`, `NameError`, `DivideByZero`, `StackOverflow`, `IOMismatch`, `NativeError`.
3. **Stack overflow is a catchable error**, detected by a depth counter, never by a guard page
   segfault. *(Catchable, not fatal — and never a crash.)*
4. **No use-after-free in release or debug.** The `unsafe` surface is a small, auditable list,
   enforced by `#![deny(unsafe_op_in_unsafe_fn)]` and a crate-level lint that forbids `unsafe`
   outside `zen-vm::value` and `zen-vm::gc`.
5. **Determinism.** The same program produces the same output, and the interpreter, the VM, and the
   JIT produce byte-identical output. This is enforced by the differential harness from M3 onward.
6. **Memory does not grow without bound.** A 10⁷-iteration loop that allocates and drops has flat
   RSS after warmup. Measured by the leak test in [`17-testing-strategy.md`](17-testing-strategy.md).

---

## 6. Ambiguities we resolved, and how

The blueprint and the existing Zen disagree in places. Where they conflict, this table is the
ruling. Everything in the "Ruling" column is also in
[`03-syntax.md`](03-syntax.md) with a test that enforces it.

| Conflict | Blueprint | Existing Zen | **Ruling** |
|---|---|---|---|
| Dict literal | `#{}` | `{a: 1}` | **Both.** `#{a:1}` is canonical; `{a:1}` parses as a dict only when a `key:` pair is found — and emits a deprecation warning. Preserves 32 existing test files. |
| Set / tuple | `%{}` / `#()` | absent | **Both added.** No existing syntax to break. |
| Comments | `#` only | `//`, `#`, `/* */` | **Keep all three.** `#` is used by `#{`/`#(`; `# ` (hash-space) is still a comment. Lexer resolves by peeking the next byte. |
| `->` | "match arm separator" | inclusive range `5 -> 1` | **Keep the existing range meaning.** Match arms accept both `->` and `=>`. Ruling in the grammar. |
| `let` vs `var` | unspecified | both, plus `const` | **Keep all three.** `let` = immutable, `const` = immutable + enforced, `var` = mutable. |
| `int` range | i44 | all numbers are `f64` | **i48 integer plus f64 float**, with Lua-style promotion (`int / int` → float, so `10/4 == 2.5` still holds and existing tests pass). |
| `nil` vs `null` | `nil` | `null` | **Both keywords, same value.** `nil` is canonical. |
| Hex literals | `0xFF` | rejected by lexer | **Add `0x`, `0b`, `0o`, `_` separators, `1e10`.** Purely additive; `13_bitwise.z` in `tests/diff/` already assumes hex works. |

---

## 7. Consequences

- The new project lives in `zen/` and shares nothing with the Zen 2.2.0 tree. The old tree is left
  intact as a working fallback and as the source of the 32-file test corpus. **It is not deleted.**
- `editors/`, `tests/cases/`, `tests/diff/`, and `std/` are copied into the new project, not
  re-invented. The differential harness in `tests/diff/` is preserved because it encodes years of
  regression knowledge, and it becomes the M3↔M4 conformance gate.
- Six defects in the existing tree are inherited and must not be carried forward: the duplicate
  root `build.sh` / `scripts/build.sh` pair, the third divergent copy of the VS Code grammar inlined
  in `setup-editors.sh`, `editors/tree-sitter/grammar.js` (which is a `.scm` query file with a `.js`
  extension and fails `node --check`), and the 16 broken MkDocs nav entries that make
  `mkdocs build --strict` fail.
