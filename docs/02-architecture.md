# 02 — System Architecture

> **Status:** Canonical  
> **Milestone:** M0

---

## 1. Overview

Zen v3 is a **single-process, multi-threaded** language runtime with:

- A **register-based bytecode VM** (the sole execution engine)
- **NaN-boxed `Value(u64)`** for all runtime values
- **Arc + copy-on-write** memory management (Phase 1)
- **Inline caches** on field access and calls
- **Optional background Cranelift JIT** (feature-gated, Milestone 11)
- **Per-thread VM instances** for `spawn`, sharing immutable code
- **Lock-free channels** (`crossbeam-channel`) for concurrency
- **FFI handle table** (`DashMap<u32, Box<dyn Any + Send>>`) for native resources
- **Tokio-based async runtime** for networking

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Zen Process                                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │
│  │  Thread 1   │  │  Thread 2   │  │  Thread N   │  ... (spawn)    │
│  │  ┌───────┐  │  │  ┌───────┐  │  │  ┌───────┐  │                 │
│  │  │  VM   │  │  │  │  VM   │  │  │  │  VM   │  │                 │
│  │  │regfile│  │  │  │regfile│  │  │  │regfile│  │                 │
│  │  │stack  │  │  │  │stack  │  │  │  │stack  │  │                 │
│  │  │Arc GC │  │  │  │Arc GC │  │  │  │Arc GC │  │                 │
│  │  └───────┘  │  │  └───────┘  │  │  └───────┘  │                 │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                 │
│         │                │                │                        │
│         └────────────────┼────────────────┘                        │
│                          ▼                                         │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │ SHARED (Arc / lock-free)                                     │  │
│  │  • Bytecode (FunctionProto, Instruction[])                   │  │
│  │  • String/Bytes/Tuple data (Arc)                             │  │
│  │  • Native modules (sys, fs, net, crypto, ...)                │  │
│  │  • Handle table (DashMap<u32, Box<dyn Any + Send>>)          │  │
│  │  • JIT code cache (AtomicPtr per function, Milestone 11)     │  │
│  └─────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Crate Layout

```
zen/ (project root)
├── Cargo.toml                    # Workspace manifest
├── docs/                         # This directory
├── crates/
│   ├── zen-syntax/               # Lexer, AST, Parser, Span, Diagnostics
│   ├── zen-vm/                   # Value, Bytecode, VM, GC, Modules, FFI, Builtins
│   ├── zen-jit/                  # Cranelift JIT (optional, feature = "jit")
│   └── zen-cli/                  # `zen` and `zenn` binaries
├── std/                          # Pure-Zen stdlib (.z files)
├── tests/
│   ├── cases/                    # 32 black-box conformance tests
│   ├── diff/                     # VM vs interpreter differential harness
│   └── data/                     # Fixtures (signal_golden.json)
├── examples/                     # Example scripts
├── benchmarks/                   # Criterion benchmarks
├── tools/                        # Build/dev tooling
└── editors/                      # Syntax definitions (copied from v2)
```

---

## 3. Data Flow

```
Source (.z)
    │
    ▼
┌──────────────────┐
│ zen-syntax       │
│  Lexer  ──► AST  │   (M1)
│  Parser         │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ zen-vm           │
│  Compiler       │   (M4)
│  AST ──► Bytecode│
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ zen-vm           │
│  VM              │   (M4)
│  Bytecode ──►    │
│  Value           │
└────────┬─────────┘
         │
    ┌────┴────┐
    ▼         ▼
Interpreter   JIT (M11, optional)
    │         │
    └────┬────┘
         ▼
      Result
```

---

## 4. Threading Model

| Component | Strategy |
|---|---|
| `spawn(fn(){})` | New OS thread, new `Vm` instance, shares `Arc<FunctionProto>` and native modules |
| `channel()` | `crossbeam_channel::unbounded::<Value>` — deep copy mutable, `Arc` share immutable |
| `shared x = ...` | `Arc<Mutex<T>>` — explicit, visible in source |
| `mutex.new()` | `parking_lot::Mutex<Value>` |
| `spawn_async` | Submits to a shared `tokio` runtime (single `Runtime` per process) |
| Native modules | All `Send + Sync`; `tokio` runtime started lazily on first async use |

**No GIL.** Each thread's `Vm` has its own register file, stack, and `ArcGc` (refcounts are thread-local). Only the handle table and JIT code cache are globally shared (lock-free).

---

## 5. Memory Management

| Phase | Strategy | Rationale |
|---|---|---|
| M1–M8 | `Arc` + CoW (`Arc::make_mut`) | Safe, zero STW, proven in Zen 2.x. Cycles leak (rare in scripts). |
| M9+ | Two-space copying GC | Only if profiling shows refcount traffic on hot path. Behind `Gc` trait. |

**Object layout:** Every heap object is a `#[repr(C)]` struct with `ObjHeader` (kind, flags, rc) followed by payload. `Value` stores a raw pointer to the header. See [`09-memory.md`](09-memory.md).

---

## 6. JIT Integration (Milestone 11)

- **Tier 0:** Register VM with inline caches.
- **Tier 1:** Cranelift compiles hot functions on a background `rayon` pool.
- **Hotswap:** `JITBuilder::hotswap` + `JITModule::prepare_for_function_redefine` → `AtomicPtr` swap on function entry point.
- **Deopt:** Only at function entry and loop headers. No mid-basic-block deopt.
- **Feature-gated:** `cargo build --no-default-features` compiles and runs without JIT.

---

## 7. Native Module ABI

```rust
type NativeFn = fn(&mut Vm, &[Value]) -> VmResult<Value>;
```

- Arguments: slice of `Value` (already type-checked by caller).
- Strings/bytes: zero-copy — VM passes `&str` / `&[u8]` from the GC object.
- Return: `VmResult<Value>` — errors become catchable Zen errors.
- Registration: `zen_register_module("name", &[("func", native_fn)])`.
- Panic safety: every native call wrapped in `catch_unwind`; panic → `VmError::NativeError`.

### Import paths and tier selection

Imports are **bare names** — `import fs`, not `import std.fs`. `std` is a directory in the module
search path, not a prefix in the module name; a dotted path (`import a.b`) names a package. The
native registry holds bare keys only.

`native_modules()` is inert data; `link()` alone decides which tier an import gets: a `.z` file wins,
and one declaring `ZEN_PURE = true` gains its native tier as the `native` export member, reachable as
`mod["native"]` or `import mod.native`. `ZEN_NATIVE_ONLY=<mod>` inverts this for one module. See
[ADR-0005](decisions/ADR-0005-module-paths-and-tiers.md).

---

## 8. Build Profiles

| Profile | Use Case |
|---|---|
| `dev` | Development, differential testing |
| `release` | Production binary — LTO, thin, 1 codegen unit |
| `bench` | Identical to `release` — for Criterion |

**Critical flags:**
- `panic = "unwind"` — never abort on user code
- `overflow-checks = true` — integer promotion, not wrap
- `debug = 1` — symbols for JIT profiling and crash dumps

---

## 9. Cross-Cutting Concerns

| Concern | Solution |
|---|---|
| Diagnostics | Every `VmError` carries a `Span`; `SourceMap` in `Vm` maps to source lines |
| Fuzzing | `cargo fuzz` target generates random valid programs; runs in CI |
| Differential testing | `tests/diff/` harness runs every case under VM and interpreter; byte-identical output required |
| Benchmarks | `benchmarks/` crate with Criterion; gates in [`18-benchmarks.md`](18-benchmarks.md) |
| Versioning | Semantic version in workspace `Cargo.toml`; `.zeno` cache includes format + crate version |

---

## 10. Out of Scope (Explicitly)

- **No REPL in the VM crate** — REPL is a `zen-cli` feature.
- **No parser in the VM crate** — `zen-syntax` is a separate crate.
- **No JIT in the VM crate** — `zen-jit` is a separate crate, optional feature.
- **No `async`/`await` syntax in v3** — `spawn_async` + `block_on` is the model; `await` is a language addition for v4.
- **No `match` exhaustiveness checking** — dynamic language, runtime error on no match.
- **No `trait`/`interface` system** — structural typing via `dict`/`func` is sufficient.