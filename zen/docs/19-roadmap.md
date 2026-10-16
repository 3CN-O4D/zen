# 19 — Milestone Roadmap & Gates

> **Status:** Active planning  
> **Milestone:** M0 (this doc is the project plan)

---

## Gate Philosophy

A milestone is **DONE** only when **ALL** gates pass:
1. **Unit tests** — crate-level `cargo test -p <crate>`
2. **Integration tests** — `cargo test --workspace` (includes differential harness)
3. **Benchmarks** — `cargo bench --bench <name>` meets gate for that milestone
4. **Lint** — `cargo clippy --all-targets --all-features -- -D warnings`
5. **Fuzzing** — `cargo fuzz run <target>` 10⁶ iterations, zero crashes
6. **Docs updated** — This directory matches implementation

**No partial credit.** A milestone with 5/6 gates is not released.

---

## M0 — Project Setup (THIS WEEK)

**Deliverable:** Compiling workspace with all 4 crates, `cargo test --workspace` passes (even if tests are stubs).

| Task | Status |
|---|---|
| Workspace `Cargo.toml` with 4 crates | ✅ |
| `zen-syntax` crate (lexer/parser/ast stubs) | ✅ |
| `zen-vm` crate (value/bytecode/vm/gc stubs) | ✅ |
| `zen-jit` crate (optional feature) | ✅ |
| `zen-cli` crate (`zen` + `zenn` binaries) | ✅ |
| `benchmarks` crate | ✅ |
| `cargo build --workspace` passes | ✅ |
| `cargo test --workspace` passes | ✅ |

**Gate:** Clean build, zero warnings (except known dead-code stubs).

---

## M1 — Lexer + AST + Parser (Week 1–2)

**Deliverable:** `zen-syntax` crate produces a valid AST for every file in `tests/cases/` and `tests/diff/`.

| Component | Spec | Tests |
|---|---|---|
| Lexer | [`05-lexer.md`](05-lexer.md) | 30+ unit tests (tokens, numbers, strings, comments, `#` disambiguation) |
| AST | [`06-parser.md`](06-parser.md) §2 | Round-trip: parse → serialize → parse → identical AST |
| Parser | [`06-parser.md`](06-parser.md) | 66 named parse tests (legacy + new syntax, deprecation warnings) |
| Diagnostics | [`05-lexer.md`](05-lexer.md) §9, [`06-parser.md`](06-parser.md) §10 | Error spans point to exact byte; no panics on invalid input |

**Gate:**
- `cargo test -p zen-syntax` passes
- All 32 `tests/cases/*.z` parse without error (legacy dict `{a:1}` emits warning)
- All 19 `tests/diff/*.z` parse without error
- `cargo clippy -p zen-syntax -- -D warnings` passes

---

## M2 — NaN-Boxed Value + GC Trait (Week 2–3)

**Deliverable:** `zen-vm::value` and `zen-vm::gc` implement the full 16-type system with Arc+CoW.

| Component | Spec | Tests |
|---|---|---|
| `Value` | [`04-value-and-types.md`](04-value-and-types.md) | 25+ unit tests: constructors, accessors, tag logic, integer boundaries, float normalization, pointer packing |
| `ObjKind` + header | [`04-value-and-types.md`](04-value-and-types.md) §8 | Exhaustive match in `Debug`; `ObjHeader::new` |
| `ArcGc` trait + impl | [`09-memory.md`](09-memory.md) §6–7 | `allocate_*` round-trip; CoW: `Arc::make_mut` only on shared mutation |
| `Gc` trait | [`09-memory.md`](09-memory.md) §6 | Both `ArcGc` and future `CopyingGc` satisfy it |

**Gate:**
- `cargo test -p zen-vm` passes
- No `unsafe` outside `value.rs` + `gc.rs` (enforced by `forbid(unsafe_op_in_unsafe_fn)`)
- `cargo fuzz run value_fuzz` 10⁶ iterations, zero crashes

---

## M3 — Tree-Walking Interpreter (Week 3–4)

**Deliverable:** A correct, slow reference interpreter that executes the AST directly. Used as the differential oracle for M4.

| Component | Spec | Tests |
|---|---|---|
| Interpreter | Executes `Module` AST | All 32 `tests/cases/*.z` produce correct output |
| Builtins | `print`, `len`, arithmetic, comparisons, list/dict ops, `typeof`, `bool` | Unit tests per builtin |
| Error handling | `VmError` with spans | Every error path tested |

**Gate:**
- `cargo test --workspace` passes (includes differential harness `tests/diff/`)
- **Byte-identical output** between interpreter and future VM on all 19 `tests/diff/*.z` probes
- Zero panics on fuzzed input (`cargo fuzz run interp_fuzz`)

**Note:** This interpreter is **throwaway** — deleted after M4 validates against it. Do not optimize it.

---

## M4 — Register Compiler + VM (THE SPEED MILESTONE) (Week 5–7)

**Deliverable:** A register-based bytecode VM that **beats CPython 3.13 on `bench_core`**.

| Component | Spec | Tests |
|---|---|---|
| Bytecode format | [`07-bytecode.md`](07-bytecode.md) | Round-trip: compile → serialize `.zeno` → deserialize → identical `FunctionProto` |
| Compiler | [`07-bytecode.md`](07-bytecode.md) | 20+ compile tests: AST → instruction sequence |
| VM executor | [`08-vm.md`](08-vm.md) | All 32 `tests/cases/*.z` + 19 `tests/diff/*.z` |
| Inline caches | [`08-vm.md`](08-vm.md) §6 | Monomorphic cache hit rate > 95% on `bench_call` |
| Hidden classes | [`08-vm.md`](08-vm.md) §7 | Dict shape interning; field access via cached offset |

**Performance Gate (MANDATORY):**
| Benchmark | Target |
|---|---|
| `bench_core` aggregate | **≥ 1.5× CPython 3.13** |
| **Every individual `bench_core` test** | **Never slower than CPython** |
| `bench_fib(35)` | < 1.0s (CPython ~1.8s) |
| `bench_dict_lookup` | ≥ 2× CPython |
| `bench_list_iter` | ≥ 2× CPython |
| `bench_string_concat` | ≥ 1.5× CPython (known weak spot — rope in M11) |

**Correctness Gate:**
- `cargo test --workspace` passes (VM vs interpreter differential on `tests/diff/`)
- `cargo fuzz run vm_fuzz` 10⁶ iterations, zero crashes
- `cargo clippy --all-targets --all-features -- -D warnings` passes

**If M4 gate fails:** Do not proceed to M5. Fix the VM.

---

## M5 — Inline Caches + Hidden Classes (Week 7–8)

**Deliverable:** Field access and call sites use monomorphic inline caches with shape guards.

| Component | Spec | Tests |
|---|---|---|
| IC structure | [`08-vm.md`](08-vm.md) §6 | Cache hit/miss counters exposed for benchmarks |
| Shape interning | [`08-vm.md`](08-vm.md) §7 | `ShapeId` for dict literals; transition on new key |
| Megamorphic fallback | [`08-vm.md`](08-vm.md) §6 | After 4 shapes, switch to hashmap lookup |

**Gate:**
- `bench_dict_lookup` improves ≥ 2× over M4 baseline
- `bench_method_call` improves ≥ 2× over M4 baseline
- No regression on any M4 benchmark

---

## M6 — Builtins + Core Stdlib (Week 8–9)

**Deliverable:** All builtin functions and the minimal stdlib modules work.

| Module | Functions | Source |
|---|---|---|
| `math` | 47 (sin, cos, sqrt, pow, log, gcd, etc.) | Pure Rust (no `std/math.z`) |
| `json` | parse, stringify, pretty | Pure Rust |
| `string` | 25 + 9 constants | Pure Rust |
| `list` | map, filter, reduce, sort, etc. | Pure Rust |
| `dict` | keys, values, get, has, etc. | Pure Rust |
| `os` | env, pid, cpu_count, exit | Pure Rust |
| `fs` | read, write, exists, mkdir, etc. | FFI |
| `time` | unix, now, sleep, format | FFI |
| `random` | randint, choice, shuffle, seed | FFI |
| `crypto`/`hashlib` | md5, sha256, pbkdf2, etc. | FFI |

**Gate:**
- All 32 `tests/cases/*.z` pass (these exercise every module above)
- `bench_json_parse` within 20% of CPython's C-extension `json` (pure Rust is fast enough)
- Zero panics on malformed input

---

## M7 — FFI + System Modules (Week 9–10)

**Deliverable:** `sys`, `net`, `sqlite`, `ffi` modules functional.

| Module | Capability | Spec |
|---|---|---|
| `sys` | pid, uid, uname, times, pipe, open, mmap, syscall | [`11-ffi-handles.md`](11-ffi-handles.md) |
| `net.tcp` | connect, listen, accept, send, recv | [`13-networking.md`](13-networking.md) |
| `net.udp` | bind, sendto, recvfrom | |
| `net.tls` | wrap, verify | |
| `net.dns` | resolve | |
| `sqlite` | open, exec, query, bind | |
| `ffi` | dlopen, symbol, call, malloc (gated by `--allow-ffi`) | |

**Gate:**
- `tests/cases/19_sys_ffi.z` passes (all 137 lines)
- `tests/cases/20_sockets.z` passes (TCP + UDP loopback)
- `tests/cases/15_sqlite.z` passes
- Handle table: stale-handle use raises catchable error, not UB

---

## M8 — `zenn` Toolchain (Week 10–11)

**Deliverable:** Complete CLI for development and distribution.

| Command | Spec | Tests |
|---|---|---|
| `zenn run file.z` | Interpret + JIT | All test files |
| `zenn build file.z -o out` | AOT compile → standalone binary | Binary runs without source |
| `zenn check file.z` | Parse + type-check (no exec) | Error on `1/0`, unknown name |
| `zenn lint file.z` | Suspicious pattern warnings | Unused var, dead code |
| `zenn test` | Parallel test runner | `tests/cases/` in < 5s |
| `zenn fmt` | Opinionated formatter | `zenn fmt --check` on CI |
| `zenn repl` | Interactive session | `:help`, history, tab-completion |

**Gate:**
- `zenn build` produces a binary that runs `bench_fib(35)` in < 1.0s
- `zenn test` runs all 32 cases in < 5s on 8 cores
- `zenn fmt` is idempotent and stable

---

## M9 — Tracing GC (GATED — ONLY IF PROFILING DEMANDS)

**Trigger:** M4/M5 profile shows `Arc::make_mut` / refcount traffic > 15% of CPU time on `bench_core`.

**Deliverable:** Two-space copying collector behind `Gc` trait.

| Component | Spec |
|---|---|
| Semispace allocator | [`09-memory.md`](09-memory.md) §7 |
| Complete root map | Registers, all frame slots, constant pools, shadow stack, pinned handles, native frames |
| Cheney scan | Breadth-first, forwarding pointers |
| Write barrier | Card marking or snapshot-at-beginning |
| Finalizers | For `Handle` (close fd), `Channel` (close) |

**Gate:**
- `cargo test --workspace` passes
- No regression on M8 benchmarks
- `cargo fuzz run gc_fuzz` 10⁶ iterations, zero crashes
- Leak test: 10⁷ alloc/drop loop → flat RSS

**Deferred here from the M1–M5 freeze audit** (see [`08-vm.md`](08-vm.md) §11.4 and
[`09-memory.md`](09-memory.md) §4.5):
- **Aggregate allocation bound** (`--mem-limit`, `08-vm.md` §11.5 test 33): a program can currently
  allocate without limit until the process dies. M9 owns the clean-message/non-zero-exit behavior.
- **Reference cycles**: the interpreter scope↔closure cycle and the VM closure/module cycle leak by
  design (ADR-0002). M9/the tracing collector is where they are expected to be reclaimed.

---

## M10 — Concurrency + 7-Layer Networking (Week 12–14)

**Deliverable:** True multi-core execution + production networking.

| Feature | Spec |
|---|---|
| `spawn(fn(){})` | OS thread + new `Vm` |
| `channel()` / `send` / `recv` | Deep copy mutable, `Arc` share immutable |
| `shared` / `mutex` | Explicit sharing |
| `spawn_async` / `await` (blocking) | Tokio runtime |
| `pmap` | Rayon work-stealing, ordered results |
| `net.http` / `ws` / `grpc` | Hyper, Tungstenite, Tonic |

**Gate:**
- `bench_channel_throughput` > 1M msgs/sec on 8 cores
- `bench_http_client` within 20% of raw `hyper`
- `spawn` 1000 tasks → all complete, no leaks
- Race detector (`-Zsanitizer=thread`) clean on nightly

---

## M11 — Background Cranelift JIT (Week 15–18, OPTIONAL)

**Trigger:** M10 benchmarks show dispatch overhead > 15% of `bench_core` time AND inline caches saturated.

**Deliverable:** Tier-1 Cranelift compilation on background pool.

| Component | Spec |
|---|---|
| Hotness counters | Calls > 100, loop back-edges > 1000 |
| Background queue | Lock-free `crossbeam-channel` → `rayon` pool |
| IR translation | [`10-jit.md`](10-jit.md) §5 |
| Atomic hotswap | `JITBuilder::hotswap` + `AtomicPtr` |
| Deopt | Function entry + loop headers only |
| Feature gate | `cargo build --no-default-features` works |

**Gate:**
- `bench_fib(35)` < 0.4s (JIT) vs 0.8s (VM)
- `bench_core` aggregate ≥ 2× M10 baseline
- **No output difference** between VM and JIT on any test (torture test: 10⁴ alternating runs)
- `cargo build --no-default-features` passes

---

## Summary Timeline

| Milestone | Weeks | Primary Gate |
|---|---|---|
| M0 | 0 | Workspace builds |
| M1 | 1–2 | Lexer/Parser tests pass |
| M2 | 2–3 | Value/GC tests pass |
| M3 | 3–4 | Interpreter passes all tests |
| **M4** | **5–7** | **≥ 1.5× CPython, never slower individually** |
| M5 | 7–8 | IC hit rate > 95% |
| M6 | 8–9 | All stdlib tests pass |
| M7 | 9–10 | sys/net/sqlite tests pass |
| M8 | 10–11 | `zenn` CLI complete |
| M9 | (gated) | Only if profiling demands |
| M10 | 12–14 | Concurrency + net benchmarks |
| M11 | 15–18 | JIT optional, feature-gated |

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Cranelift API breaks | High | M11 blocked | Feature-gated; VM is shippable without JIT |
| `Arc` refcount bottleneck | Medium | M4–M8 slower | M9 GC gated on profiling |
| `String` concat O(n²) | High | `bench_string_concat` fails | Rope in M11; accept M8 regression if small |
| Differential harness flakes | Low | CI noise | Fixed seeds, hermetic tests |
| Fuzzing finds UB | Medium | Delay | `forbid(unsafe_op_in_unsafe_fn)` + `cargo fuzz` |
| `dashmap` RC issues | Low | Handle table | Use `dashmap 6` stable |

---

## Definition of "v3.0.0 Release"

**Minimum:** M0–M8 complete, all gates green.  
**Target:** M0–M10 complete, all gates green.  
**Stretch:** M11 JIT working and on by default.

**Release checklist:**
- [ ] `cargo test --workspace` passes
- [ ] `cargo bench` meets M8 gates
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo fuzz` 10⁶ iterations on all targets, zero crashes
- [ ] All docs in `docs/` match implementation
- [ ] `zenn build` produces working standalone binary
- [ ] `README.md` at project root updated
- [ ] Git tag `v3.0.0` pushed