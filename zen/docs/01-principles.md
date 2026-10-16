# 01 — Design Principles & Stability Doctrine

> **Status:** Canonical  
> **Milestone:** M0 (before any code)

---

## 1. Stability Is Not Optional

Every design decision is evaluated against: **Can a user script crash the process?**

- **No panics on user input.** The interpreter, VM, and JIT convert all failures into catchable `ZenError` values with source spans.
- **No `unwrap()` on user data.** The `expect`/`unwrap` pattern is forbidden on any value that originated from a script. Use `VmResult` everywhere.
- **No arithmetic overflow.** Integer math promotes to float on overflow (Lua-style) rather than wrapping or panicking.
- **No index-out-of-bounds.** All array/index access returns a `ZenError` or `nil` per the language spec.
- **No use-after-free.** The `unsafe` surface is a small, audited list (see [`09-memory.md`](09-memory.md) §9). `unsafe` is forbidden outside `zen-vm::value` and `zen-vm::gc`.
- **Stack overflow is a catchable error.** Detected by a frame-depth counter, never by a guard-page segfault.

**Why this matters:** A scripting language that crashes the host process on a typo is not a tool — it's a liability. Zen targets systems programming (networking, security tools, file processing) where the script is often untrusted or generated.

---

## 2. Performance Without Compromise

The speed target is **≥ 1.5× CPython 3.13 on aggregate `bench_core`, never slower on any individual benchmark**.

- **Register VM** reduces instruction count ~40% vs stack machines.
- **NaN-boxed `Value(u64)`** eliminates heap allocation for integers, floats, booleans, and nil.
- **Inline caches** on field access and call sites are the single biggest real-world win — they cost ~1 day to implement and beat a JIT on most script workloads.
- **JIT is Milestone 11**, not Milestone 2. The register VM + inline caches must already beat CPython before we spend weeks on Cranelift.
- **Zero-copy strings/bytes** in the FFI boundary. Native modules receive `&str` / `&[u8]` directly from the GC object.

**No "fast enough" excuses.** If a benchmark regresses, it blocks the release.

---

## 3. Simplicity Over Cleverness

- **No custom GC in Phase 1.** `Arc` + copy-on-write containers (proven in Zen 2.x) is safe, leak-only-on-cycles, and fast enough. A correct tracing GC is Milestone 9, gated on profiling evidence.
- **No linear move-semantics on channels.** Deep copy mutable containers on send; explicit `shared` for zero-copy sharing. The blueprint's "sender becomes nil" is unimplementable in a dynamic language and incompatible with a moving GC.
- **No `typeof` lies.** `handle` reports `"handle"`, not `"int"`. The JIT must trust `typeof`.
- **No dual execution engines.** Zen 2.x had a tree-walker AND a bytecode VM plus a differential harness to prove they agree. v3 has ONE bytecode VM. The tree-walker exists only as a Milestone-3 reference implementation for differential testing, then is deleted.
- **No hand-rolled HTTP/client stacks.** Use `hyper`, `tokio`, `rustls`. A hand-rolled HTTP client is a correctness liability.

---

## 4. Honest Engineering

- **Cranelift is "extremely experimental"** (its own description). The JIT is behind a Cargo feature `jit` so a Cranelift regression can never block a Zen release. `cargo build` must succeed with the JIT off.
- **Every benchmark is recorded both ways** (JIT on / JIT off).
- **The differential harness** (VM vs interpreter) runs on every commit. Byte-identical output is required.
- **Fuzzing is mandatory.** A structured random program generator runs in CI and must never crash the process.
- **Dependencies are audited.** `dashmap 6` (not RC), `cranelift 0.136` (experimental flag), `tokio 1` (stable). No `unsafe` in dependencies we don't control unless wrapped.

---

## 5. The 16 Types Are Final

The language surface has exactly 16 types. No `char`, no `uint`, no `i32`, no `Decimal`, no `Complex`. One integer (i48 with float promotion), one float (f64). This constraint is what makes the JIT feasible and the error messages short.

| Tag | Type | Syntax |
|---|---|---|
| 0 | `nil` | `nil` / `null` |
| 1 | `false` | `false` |
| 2 | `true` | `true` |
| 3 | `int` | `42`, `0x2A`, `0b101010` |
| 4 | `float` | `3.14`, `1e-10` |
| 5 | `string` | `"..."`, `"""..."""` |
| 6 | `bytes` | `bytes([1,2])` |
| 7 | `list` | `[1, 2, 3]` |
| 8 | `dict` | `#{a: 1}` |
| 9 | `set` | `%{1, 2, 3}` |
| 10 | `tuple` | `#(1, 2)` |
| 11 | `func` | `fn() {}` |
| 12 | `error` | `error("msg")` |
| 13 | `channel` | `channel()` |
| 14 | `mutex` | `mutex.new()` |
| 15 | `handle` | `tcp.connect()` |

---

## 6. Syntax Changes Are Additive Only

The existing 32 test files in `tests/cases/` must continue to pass unchanged. New syntax (`#{}`, `%{}`, `#()`) is added alongside, never replacing. Legacy `{a:1}` dicts parse with a deprecation warning. This is not a breaking release — it's a reimplementation with extensions.

---

## 7. Tooling Is Part of the Language

`zenn run`, `zenn build`, `zenn test`, `zenn fmt`, `zenn check`, `zenn lint`, `zenn repl` are not afterthoughts. They are specified in [`15-toolchain.md`](15-toolchain.md) and implemented in Milestone 8. A language without a formatter, test runner, and standalone-binary builder is a hobby, not a tool.

---

## 8. Networking Is Layered, Not Monolithic

The 7-layer stack in [`13-networking.md`](13-networking.md) maps each OSI layer to a mature Rust crate. Zen's job is the clean API and zero-copy handoff. Reimplementing TLS or HTTP is explicitly out of scope.

---

## 9. Cross-Cutting Rules

| Rule | Enforcement |
|---|---|
| No `panic = "abort"` | Workspace `panic = "unwind"` |
| Overflow checks on | Workspace `overflow-checks = true` |
| Debug symbols in release | Workspace `debug = 1` |
| Clippy `pedantic` + `nursery` | CI gate |
| `unsafe` only in `zen-vm::value` + `zen-vm::gc` | `cargo audit` + custom lint |
| Fuzzing in CI | `cargo fuzz` nightly |

---

## 10. What "Done" Looks Like

A milestone is **not done** until:
- All unit tests pass
- `cargo test --workspace` passes (including differential harness)
- `cargo bench` meets the gate for that milestone
- `cargo clippy --all-targets --all-features -- -D warnings` passes
- `cargo fuzz run` completes 10⁶ iterations without crash
- Documentation in this directory is updated to match the implementation

No milestone is "80% done". It is either done or not in the release.