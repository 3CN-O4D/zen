# 12 — Concurrency: threads, channels, mutexes, async

> **Status:** Draft for review. The rulings a reviewer should attack first are §2.4 (`spawn` is an
> OS thread, not a green thread), §3.4 (Rust's `Send` is useless for a NaN-boxed `Value`, so the
> affinity tag does all the work), §5.2 (deep copy on send, and what it costs), and §7.6 — the
> `await` ruling, which is the hardest design point in this document and the one most likely to
> need to move.
> **Milestone:** M10 (Concurrency + 7-layer net). §7.6's `await` is **M13**, not M10, and the
> reason is the JIT, not the parser.

---

## Contents

1. [Purpose and the ruling](#1-purpose-and-the-ruling)
2. [Threading model](#2-threading-model)
3. [The `Value` thread-safety question](#3-the-value-thread-safety-question)
4. [Channels](#4-channels)
5. [Deep copy on send](#5-deep-copy-on-send)
6. [`shared`, `mutex`, and atomics](#6-shared-mutex-and-atomics)
7. [Async](#7-async)
8. [Data parallelism](#8-data-parallelism)
9. [The 100 % parallel claim, audited](#9-the-100--parallel-claim-audited)
10. [Deadlock and livelock guidance](#10-deadlock-and-livelock-guidance)
11. [Test plan](#11-test-plan)
12. [Measurement](#12-measurement)
13. [Rulings a reviewer should double-check](#13-rulings-a-reviewer-should-double-check)

**Consumes:** the `!Send` `Vm` and per-thread-inline caches from
[`08-vm.md`](08-vm.md) §2.5 and §6.4; the `Arc`+CoW value representation from
[ADR-0002](decisions/ADR-0002-memory-model.md); the 16-type surface from
[`04-value-and-types.md`](04-value-and-types.md); the handle table from
[`11-ffi-handles.md`](11-ffi-handles.md) §4.
**Governs:** the `BlockingClass` field on `NativeEntry` (§9.4 there), the `--allow-raw-sockets`
interaction in [`13-networking.md`](13-networking.md) §3, and the loopback-only test rule in
[`13-networking.md`](13-networking.md) §11.

---

## 1. Purpose and the ruling

**The ruling, in one line: no GIL, explicit sharing, deep copy on send.** Recorded in
[ADR-0003](decisions/ADR-0003-concurrency-semantics.md) and justified in
[`00-blueprint-review.md`](00-blueprint-review.md) §A3.

This document turns that line into an API, a thread-safety argument, a test plan, and a set of
numbers a reviewer can check.

### 1.1 Why the blueprint's linear move-semantics was rejected

The blueprint says: *"sending mutable data (list, dict) over a channel **transfers ownership**,
automatically setting the sender's reference to `nil`."* That is a linear type system, and it
fails for four independent reasons, any one of which is sufficient.

1. **The analysis is not implementable in a dynamic language.** For the sender's binding to become
   `nil` at runtime, the compiler must statically know the value is a mutable collection, enumerate
   every alias of it, invalidate all of them, and refuse to compile if an alias escapes. Zen is
   dynamically typed; `dict.get(k)` reaches a container the compiler never saw; a `Value` can be
   produced by a native that hands back a reference; and a closure capturing the list is an alias
   the compiler would have to see through. There is no sound way to enumerate aliases, so the
   only choices are "miss one" (unsound) or "give up" (which is the deep-copy design).
2. **Reflection breaks it.** Zen has `type(x)`, `dir(x)`, and native modules that accept a
   container and store it. A native in another crate is a callback the compiler cannot analyse, so
   an alias can escape into code that was never type-checked against the linear rules.
3. **It is incompatible with a moving tracing GC.** A copying collector *relocates* objects, so
   "this pointer is now owned by another thread" is not a stable invariant — the collector would
   have to know about thread ownership to update a root set, which is exactly the global
   coordination the no-GIL design exists to avoid. ADR-0002 defers the GC to M9; if M9 ships a
   two-space copying collector, a move-semantics rule becomes unimplementable at the moment the GC
   lands, which is the worst possible time to discover it.
4. **It is not what users want.** The pattern people actually write is: build a batch, send it,
   keep sending more. With move semantics that pattern is a compile error, so the workarounds are
   "clone it anyway" (identical to deep copy, with worse error messages) or "give each worker its
   own copy and reassemble" (hand-rolled deep copy, with hand-rolled bugs).

The deep-copy alternative costs one `memcpy` of a collection per cross-thread send of a mutable
container. §5.3 measures that: at the median realistic script size it is under 2 µs, and at the
90th percentile under 40 µs. In exchange, a whole class of bug stops existing. That is the right
trade for a language whose stated priority is "never abort the process on user input"
([`00-blueprint-review.md`](00-blueprint-review.md) §5): races become impossible by default rather
than becoming something the runtime has to detect.

### 1.2 What "explicit sharing" means, precisely

Three mechanisms, all visible in the source, and no others:

| Mechanism | Syntax | Semantics | Cost |
|---|---|---|---|
| Arc-share (free) | `send(ch, "a string")`, `send(ch, bytes_buf)`, `send(ch, t)` | `Arc` clone; zero copy; the value is immutable so sharing is safe | one atomic increment |
| Deep copy (default for mutable) | `send(ch, my_list)` | A full recursive copy; sender and receiver have independent values | O(size), §5.3 |
| Explicit sharing | `shared x = [...]`; `x` is `Arc<Mutex<Value>>` | Both sides see the same value; every access takes a lock | one uncontended lock, ~15 ns |

There is no fourth mechanism. A script that wants a fast zero-copy mutable handoff uses `shared`
and pays a lock, which is a choice written in the source where a reader will see it.

---

## 2. Threading model

### 2.1 The three candidates

| | **Raw `std::thread` per task** | **rayon work-stealing pool** | **Green threads (a task scheduler)** |
|---|---|---|---|
| `spawn(f)` maps to | one OS thread per call | *nothing* — `spawn` is not rayon's model | one green thread per call |
| Memory per live task | 8 MiB virtual stack, 16–24 KiB resident | ~0 (tasks share the pool's threads) | 4–16 KiB |
| Latency to first instruction | **~25 µs** (mmap + clone + TLS setup + `Vm::new`) | ~200 ns (push onto a deque) | ~1 µs |
| Context switch | 2–5 µs (kernel) | 50–200 ns (userspace) | 100 ns–1 µs |
| How many can be alive | ~4000 comfortably; 10 000 pushes virtual RSS and thrash | bounded by pool size (default: core count) | ~100 000 |
| A Zen function that blocks on `sys.read` | **blocks its own thread, nothing else** | **starves the pool** — one blocked task stalls every other core | **deadlocks the whole scheduler** unless a reactor is woven through the VM |
| Works with a `Vm` per thread? | yes, naturally | no — one `Vm` cannot be `Send` ([`08-vm.md`](08-vm.md) §2.5) | no, and worse: a green thread's registers are VM state, so every yield is a VM suspension |
| Cost of the design | none | requires `Send + Sync` closures; see §8 | requires `await` or a full CPS transform of the language |

### 2.2 The green-thread column is the interesting one

A green thread is attractive precisely because it makes `spawn` cheap, and it is rejected because
of one question: **what happens when a Zen function inside a green thread calls `sys.read`?**

| Option | Result |
|---|---|
| The green thread blocks the OS thread | Then it is not a green thread, it is a very expensive OS thread with a scheduler bolted on |
| Every green thread gets an OS thread | Then it is an OS thread with a worse name |
| Every blocking native is made async under the hood | This is `await`, and it is a language change — see §7.6 |
| A blocking call in a green thread is an error | Correct, enforceable, and *only possible if the language can tell*, which it can: `NativeEntry::blocking` ([`11-ffi-handles.md`](11-ffi-handles.md) §2.1) |

The fourth option is the honest one and it is the M10 design: **`spawn` gives you an OS thread, so
a blocking call is always legal, and the concurrency model never has to be explained.** The cost is
25 µs per spawn, which is why `pmap` exists (§8): a script that wants 1000 units of work in
parallel wants a *data* parallel, not 1000 *control* flows.

### 2.3 The "1000 msg/sec" claim, audited

The blueprint claims the channel implementation targets 1000 messages per second. **[E] That
number is off by two to three orders of magnitude, in the *good* direction.** A `crossbeam-channel`
round trip between two pinned threads on the same NUMA node is 1–3 µs; with a buffer of 64 and a
batched producer it is well under 1 µs. The realistic figures:

| Operation | Latency | Throughput (2 threads, 1 channel) |
|---|---|---|
| `unbuffered` send + recv round trip | 1.2–3.0 µs | ~330 k msg/s |
| `channel(64)` send + recv | 0.6–1.4 µs | ~700 k msg/s |
| `channel(1024)` send + recv | 0.4–0.9 µs | ~1.1 M msg/s |
| Batched: send a 100-element `list` (one deep copy) | 4–20 µs | ~50–250 k batch/s |
| `spawn(fn(){})` + `join` | **~30 µs** | ~33 k spawns/s |
| `pmap` over 1000 items | ~200 µs total | ~5 M items/s |

**So: can raw threads hit 1000 msg/sec? Yes, by a factor of 330.** The claim is not a constraint
and should not appear in the documentation as a target. The number that *is* a constraint is
spawn latency: at 30 µs, a program that spawns a thread per unit of work is 97 % thread startup
overhead if the work is under 1 µs. The documentation rule that follows is:

> `spawn` is for **concurrency** (waiting on several things at once, long-lived workers).
> `pmap` is for **parallelism** (the same computation over N items).
> Spawning a thread for work shorter than ~50 µs is a mistake, and the docs say so with the number.

### 2.4 Ruling: `spawn` is one OS thread, one `Vm`, 8 MiB stack

```zen
spawn(fn() {
    // This runs on a real OS thread with its own VM, its own inline caches,
    // and its own string interner.
})
```

```rust
// zen-vm/src/scheduler.rs
pub fn spawn(&mut self, proto: ProtoId, args: &[Value]) -> VmResult<ThreadId> {
    if self.threads.len() as u32 >= self.limits.max_threads {
        return Err(native_error(format!(
            "too many threads: {} live, limit {} (raise with --max-threads)",
            self.threads.len(), self.limits.max_threads)));
    }
    let id = self.next_thread_id();
    // 8 MiB, matching the main thread. See the note below.
    let builder = std::thread::Builder::new()
        .name(format!("zen-{id}"))
        .stack_size(STACK_SIZE);
    let handle = builder.spawn(move || {
        let mut vm = Vm::new_with(self.clone_vm_config());   // one Vm per thread
        vm.run_function(proto, args)                        // returns VmResult<Value>
    })?;
    self.threads.insert(id, ThreadHandle::new(handle));
    Ok(id)
}
```

**Why 8 MiB and not 256 KiB.** [`08-vm.md`](08-vm.md) §4.5 sets `DEFAULT_MAX_DEPTH = 8192`
because 8192 Zen frames × ~900 B of worst-case native stack in a debug build is 7.4 MB, so the
counter fires before an 8 MiB thread stack is exhausted. A 256 KiB spawned stack would blow the
native stack at roughly **frame 290**, and a segfault is neither catchable nor deterministic. So
spawned threads get the same 8 MiB, and `max_depth` is uniform at 8192 across every thread.

The cost: 4000 threads × 8 MiB is 32 GB of *virtual* address space, of which resident is ~16–24 KiB
per thread — roughly 96 MB for 4000 threads. Linux overcommits, and `ulimit -v` is not usually set.
It is still the correct trade, and `zen --thread-stack=N` exists for the rare program that needs
more, in which case `max_depth` is **clamped down** and the script is told:

```
NativeError: --thread-stack=262144 is too small for max_depth=8192; max_depth clamped to 290
```

A uniform `max_depth` across threads is not a nicety: the differential test harness
([`00-blueprint-review.md`](00-blueprint-review.md) §5.5) requires the interpreter, the VM and the
JIT to produce byte-identical output, and a program that recurses 500 deep must not succeed on the
main thread and fail on a spawned one.

**Why one `Vm` per thread, and not a shared one.** [`08-vm.md`](08-vm.md) §2.5 makes `Vm`
explicitly `!Send` and §6.4 makes the inline caches per-thread. Giving each thread its own `Vm`
means no synchronisation on the hottest path in the VM, no lock in the dispatch loop, and no
shared mutable state to get wrong. The cost is that `spawn` cannot share a closure's captured
values by reference — which is *exactly the property ADR-0003 wants*. The two fit together.

**What `spawn` returns.** A `Thread` value: a `handle` (`ResKind::Thread`, M10) with
`thread.join()` and `thread.id()`. `join` returns the spawned function's return `Value`, or the
`VmErr` it raised, **as a Zen value** — not a Rust `Result` that the calling VM has to unpack.
Errors propagate as errors, not as a panic on the child thread.

```rust
/// A child's `Err` is delivered to the joiner as a `Value`, not as a
/// panic, so `join` is a normal Zen call.
pub enum ThreadResult { Ok(Value), Err(VmErr) }
```

### 2.5 What is NOT spawned

| Thing | Ruling |
|---|---|
| A native that calls `pthread_create` itself | A `NativeError`: *"natives must not create threads; use spawn"*. It is one line in `build.rs` (a grep for `pthread_create` in `zen-std-*`) and it closes a hole where a native's thread has no `Vm` and therefore no error path |
| A daemon thread that outlives the `Vm` | A `NativeError` at `Drop` time is too late, so the check is at registration: every `ResKind` that owns a thread is documented to join before drop, and `Vm::drop` joins (§2.4's `Drop` infallibility rule, [`11-ffi-handles.md`](11-ffi-handles.md) §9.3) |
| Nested `spawn` from a spawned thread | Allowed, and counted against the same `max_threads`. A script that spawns 4000 threads from each of 4000 threads gets the `NativeError` at depth 2, not a fork bomb |
| `spawn` of a closure that captures a `shared` value | Allowed and cheap — the capture is an `Arc<Mutex<Value>>` clone. This is the one concurrency pattern the type system makes easy |

---

## 3. The `Value` thread-safety question

### 3.1 The honest answer, which is uncomfortable

`Value` is a NaN-boxed `u64` ([ADR-0001](decisions/ADR-0001-tag-budget.md)). It is `Copy`, it is 8
bytes, and **it is unconditionally `Send + Sync + 'static` regardless of what it points at.** A
`Value` holding a pointer to a `list` is `Send` even if that list contains a raw pointer to a
thread-local.

```rust
// This compiles, and it is *correct as far as Rust is concerned*:
fn send_it(v: Value) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || { let _ = v; })
}
```

So the question "is this `Value` `Send`?" has no answer in Rust's type system, and any document
that pretends otherwise is describing a type-checker feature Zen does not have. **The real
mechanism is a runtime tag, and it is what [ADR-0003](decisions/ADR-0003-concurrency-semantics.md)
means by "a non-`shared` value touched from a second thread raises a catchable Zen error naming the
value, not a data race."**

### 3.2 The mechanism: the affinity tag

Every **mutable** Zen container carries an affinity tag in its object header, alongside the fields
[`08-vm.md`](08-vm.md) §8.2 already reserves:

```rust
#[repr(C)]
pub struct ObjHeader {
    pub kind: u8,
    pub shape: u32,
    pub rc: u32,
    pub size: u32,
    /// NEW in M10. Thread id that owns this container, or `ANY` when the
    /// value was created inside a `shared` binding. `u32` because a
    /// thread id is a `u32` and a tag of 0 means "main thread".
    pub affinity: u32,
}
```

`ObjHeader` is 16 bytes and this field fits in the existing padding-free tail only if `rc` and
`size` stay `u32` — they do. **No size change, and the field is in the same cache line as the
object data**, which is the same reasoning as [`08-vm.md`](08-vm.md) §7.5's shape field.

| Container | Mutable? | Tagging | Enforcement point |
|---|---|---|---|
| `list` | yes (`append`, `set`) | affinity = creating thread | capture, `send`, `shared` read/write |
| `dict` | yes (`set`, `del`) | affinity = creating thread | same |
| `set` | yes (`add`) | affinity = creating thread | same |
| `func` | its *cells* are | each captured cell carries its own affinity | capture into another thread |
| everything else | no | `ANY` | nothing to check |

The check is a `u32` compare on a field we are already loading. It is not free, and §12.3 says
what it costs.

### 3.3 The 16 language types, table

`Send` in the Rust sense is uniformly `yes` (§3.1) and is omitted. The columns that matter are
"affinity-tagged" and "the exact error if misused".

| # | Type | Tag | Immutable | `Send`-in-Rust | Sharing rule | Misuse error |
|---|---|---|---|---|---|---|
| 1 | `nil` | — | yes | yes | freely | — |
| 2 | `bool` | — | yes | yes | freely | — |
| 3 | `int` | — | yes | yes | freely | — |
| 4 | `float` | — | yes | yes | freely | — |
| 5 | `string` | `ANY` | yes | yes | `Arc`-shared, zero copy | — |
| 6 | `bytes` | `ANY` | yes | yes | `Arc`-shared, zero copy | — |
| 7 | `list` | thread | **no** | yes | **deep copy on send** | `NativeError: list bound on thread 0 was used from thread 3` |
| 8 | `dict` | thread | **no** | yes | **deep copy on send** | same |
| 9 | `set` | thread | **no** | yes | **deep copy on send** | same |
| 10 | `tuple` | `ANY` | yes (elements may be mutable) | yes | `Arc`-shared **but every element is re-checked** | the element's own error, naming the element's path |
| 11 | `func` | per-cell | depends | yes | captures re-checked; a `func` with a thread-affine cell **cannot be sent** | `NativeError: closure captures 'xs' (list, thread 0) and cannot be used from thread 3` |
| 12 | `error` | `ANY` | yes | yes | freely; the traceback is a `Vec<StackFrame>` of `Copy` protos | — |
| 13 | `channel` | `ANY` | the channel is immutable; the values are not | yes | movable, **not Sync** | `TypeError: channel is not Sync; 'ch' is shared but recv has a single caller` |
| 14 | `mutex` | `ANY` | the mutex is immutable; the value is not | yes | freely; a `mutex` *is* Sync | — |
| 15 | `handle` | `ANY` | the integer is; the resource is not | yes | movable; thread-safety is `ResKind`'s, §3.5 | `NativeError: sqlite handle 3#1 is not Sync (SQLITE_OPEN_FULLMUTEX not set)` |
| 16 | `instance` | thread | **no** | yes | **deep copy on send**, fields recursively | `NativeError: instance 'p' bound on thread 0 was used from thread 3` |

### 3.4 The two that are not `Send`, named explicitly

The question "which of the 16 are not `Send`" has a sharp answer once the framing is fixed.

**`func` capturing a non-`Send` cell.** A Zen closure's captures are cells. In a design where a
cell is `Cell<Value>` (a plain mutable slot), the closure is `Send` and *not* `Sync` — which is the
Rust-level statement, and it is the *less* important half. The important half is the affinity tag:
a cell holding a mutable container has that container's affinity, and `spawn` rejects a closure
whose cell is thread-affine **before the thread is created**:

```
NativeError: spawn: closure captures 'xs' (list, bound on thread 0) and cannot be
             used from thread 3
  at worker_pool.z:14
```

This is a compile-time-shaped error with a runtime spelling, and it is caught at the `spawn` call
rather than at the first mutation, which is the only place it can be caught cheaply.

**`mutex` holding a non-`Send` value.** In Rust this is `Mutex<Cell<T>>` (not `Sync`) or
`Mutex<*mut T>` (not `Send`). **In Zen it cannot arise**, and the reason is structural:

```rust
// The only way to build a Zen mutex.
pub fn mutex_new(vm: &mut Vm, args: &[Value]) -> VmResult<Value> {
    let v = arg_value(vm, args, 0, "mutex.new")?;
    // A `Value` is a u64, so there is no non-`Send` `T` to put in it. The
    // mutex stores `Value`, which is `Send + Sync` unconditionally.
    // The *value's own* affinity tag is what governs access — see below.
    Ok(Value::Obj(Obj::Mutex(Arc::new(Mutex::new(v)))))
}
```

So a Zen `mutex` is always `Send + Sync`, and a `NativeError: mutex holds a non-Send value` can
never be produced, which is why there is no such row in §3.3's error column. **The real hazard is
not that the mutex is non-`Send`; it is that the value inside it is thread-affine and someone put
it there on purpose.** `mutex.new(x)` where `x` is a thread-0 list is *legal* and is the documented
way to share a mutable value, because wrapping it in a `shared` binding is the act of saying "share
this". `mutex.new` on a value from a `shared` binding whose tag is a different thread is the one
that raises, and the message says so.

| Expression | Legal? | Why |
|---|---|---|
| `shared xs = [1,2,3]; mutex.new(xs)` | yes | `shared` is the opt-in; the tag becomes `ANY` at the `shared` |
| `xs = [1,2,3]; mutex.new(xs)` | yes, and a *silent* alias | Legal but pointless: the mutex is `Sync`, the value is not, so the only way to read it is through the mutex, and the only way to write it is through the mutex, so it is thread-safe by construction. Documented as a way to make a value thread-safe without `shared` |
| `shared xs = [1,2,3]; xs = [4,5]` (rebind) | **error** | `TypeError: 'xs' is a shared binding and cannot be rebound; use xs.replace(v)` — otherwise the mutex would be orphaned and the binding would silently become thread-affine again |
| `shared xs = [1,2,3]; del xs` | **error** | Same reason. A `shared` binding's lifetime is the binding's lifetime |

### 3.5 `handle` thread-safety, and the `ResKind` table that answers it

A handle is freely `Send` and freely `Sync`: per
[`11-ffi-handles.md`](11-ffi-handles.md) §4.1 it is a `0b111` `Value` — a tagged pointer to a
24-byte `HandleObj` — and a pointer plus a `u32` token is both. What is *not* freely `Send` is the
**resource behind it**, and each `ResKind` declares its own contract. This table is the FFI half of
[`11-ffi-handles.md`](11-ffi-handles.md) §4.3 and the two documents must agree; in particular the
`Memory` row is the one place Zen deliberately hands a script a data race, and it says so.

**Why a handle can be `Send` while its `ResKind` is not, and why that is not a hole.** The `Value`
is immutable by construction — there is no Zen operation that writes through a `Value`, only
operations that read one and allocate a new object — so moving a handle to another thread cannot
create a data race *through the handle itself*. What it can do is hand two threads the same
resource, and that is a property of the resource, not of the reference. A `Fd`'s `read(2)`s from
two threads interleave in the kernel; nothing about the handle's `Send`-ness changes that, and
nothing about it should. The rows below are the contract, and the natives are where it is enforced,
because a native that needs `Sync` asks for `ResKind::Socket` and gets a Rust compile error if the
type is not `Sync`.

| `ResKind` | `Send` | `Sync` | Contract |
|---|---|---|---|
| `Fd`, `File`, `Dir` | yes | **no** | Two threads reading the same fd interleave `read(2)` calls at the kernel; there is no way to know whose bytes you got. Use `sys.dup` per thread |
| `Socket` | yes | yes | `socket2::Socket` is `Send + Sync`; concurrent `send` is safe, and the *application* is responsible for the protocol |
| `TlsSocket` | yes | **no** | A TLS session is a state machine; two concurrent `write`s corrupt the record layer. `Sync` is not implemented on purpose |
| `Sqlite` | yes | no | `rusqlite::Connection` is `Send` but not `Sync` unless opened with `SQLITE_OPEN_FULLMUTEX`. Zen opens with `FULLMUTEX`, so it *is* `Sync`, and concurrent use is serialised inside SQLite |
| `Process` | yes | no | `std::process::Child` is `Send`, not `Sync`; `try_wait` from two threads is a data race on the child's internal state |
| `Library`, `FnPtr` | yes | no | `dlclose` from thread B while thread A is calling through a `FnPtr` is the worst bug in this table. Documented, gated behind `--allow-ffi`, and test 14 in [`11-ffi-handles.md`](11-ffi-handles.md) is the use-after-`dlclose` test |
| `Memory` | yes | **yes, deliberately** | An `mmap` region is process-wide shared memory by construction. Concurrent `poke` from two threads is a race **the script owns**, and the doc says so out loud rather than pretending it is prevented |
| `DnsResolver` | yes | yes | It is an `Arc<Resolver>`; the resolver's internal pool is `Sync` |
| `Timer` | yes | no | The slab's `entry` is per-thread; the slab itself is shared |

A native that needs `Sync` from a `ResKind` that is not `Sync` uses
`arg_handle(..., ResKind::Socket)` and gets a compile error in Rust. There is no Zen-level
`Sync` check for handles, and there does not need to be one: the natives are the boundary, and
they are type-checked.

### 3.6 The runtime check, exactly

Three check sites, and only three:

| Site | What it checks | Cost |
|---|---|---|
| `spawn(f)` | every cell `f` captures | O(captures), once |
| `send(ch, v)` | `v.affinity() == current_thread` or `v.affinity() == UNBOUND` — **you may only send from the thread that owns the value.** The copy then happens on that same thread, so the read is single-threaded by construction (§5.1) | one `u32` load + compare |
| `shared` read/write | the binding's tag is `ANY` (asserted at `shared` creation, not per access) | **zero** — the lock is the cost, the tag check is not needed |
| `recv` | none. `bind_affinity(v, current_thread())` *rebinds* rather than checks, which is legal only because the value is provably unaliased at that moment (§5.1) | one `u32` store per container in the value |

**Why the `send` check is not redundant with the deep copy.** The copy is made by the *sender*, on
the sender's thread, before the value is handed over. That is the only sound place to make it: a
copy performed on the receiver's thread would read a container from a thread the affinity tag says
may not read it, which is the race the whole design exists to remove. So `send` keeps the check —
it is what makes the sender's read legal — and the copy is what makes the *arrival* safe. The two
are complementary, and §5.1 is where the handoff sequence is pinned.

The tuple case (row 10 in §3.3) is the only recursive one: `send(ch, (a, mutable_list))` walks
the tuple, which is O(tuple length) and shallow. The walk is bounded by the nesting depth, which
the language's `len` limits already bound, and the error names the *path* —
`NativeError: t[1][2] (list, bound on thread 0) was used from thread 3` — because a bare "something
in the tuple is wrong" is not debuggable.

---

## 4. Channels

### 4.1 The Zen-visible API

`channel()` is a `channel` value (a heap object) backed by `crossbeam-channel` 0.5.17, already in
`zen/Cargo.toml:37`.

```zen
let ch = channel()            // unbuffered: a send blocks until a recv takes it
let ch = channel(64)          // bounded buffer of 64
let ch = channel(0)           // explicitly unbuffered

ch.send(v)                    // blocks if full. Returns nil.
ch.try_send(v)                // → true | false. Never blocks.
ch.recv()                     // blocks until a value or a close. → value | error{Closed}
ch.try_recv()                 // → value | nil (empty) | error{Closed} (drained and closed)
ch.close()                    // no further sends. recv drains, then returns error{Closed}.
ch.len()                      // buffered count
ch.capacity()                 // 0 for unbuffered
ch.is_closed()                // → bool
ch.is_empty()                 // → bool
```

### 4.2 Buffered vs unbuffered, and why unbounded is banned

| | unbuffered (`channel()`) | bounded (`channel(n)`) | **unbounded (`channel()` with no cap?)** |
|---|---|---|---|
| `send` blocks | until a receiver arrives | until the buffer is full | never |
| Backpressure | perfect, and the scheduler parks the sender (not spinning — crossbeam's `park`) | up to `n` items | **none** |
| Memory | O(1) per blocked sender | O(n) | O(total produced) |
| What it is good for | handoff, backpressure, work distribution | throughput, batching, decoupling | nothing |
| Verdict | default for `channel()` | `channel(n)` is the throughput form | **BANNED** |

An unbounded queue is a memory leak with a nicer name, and a producer that outruns a consumer
turns it into an OOM, which is the one fatal error in the language
([`00-blueprint-review.md`](00-blueprint-review.md) §5). A crossbeam `unbounded()` is available
in the crate and is not used: `channel()` with no argument is **unbuffered**, not unbounded, and
the documentation says so in the first line of the `channel` entry.

### 4.3 Closing semantics — the part that must not deadlock

The contract, in full:

1. **Dropping the last sender closes the channel.** `crossbeam` does this natively, and it is the
   right default: a producer that goes out of scope should not leave a consumer blocked forever.
2. **A closed channel with buffered values still delivers them.** `recv` after `close()` drains
   the buffer first. This is `crossbeam`'s behaviour and it is the behaviour users expect.
3. **A closed, drained channel makes `recv` *return* the sentinel, not block, and not raise.** This
   is the load-bearing rule. `recv()` returns the value `error{kind: "closed"}` as an ordinary
   `Value`; `try_recv()` returns `nil` for *empty but open* and `error{Closed}` for *drained and
   closed*, so a consumer loop can distinguish "nothing yet" from "nothing ever". **Returning
   rather than raising is the ruling, and it is what makes the no-deadlock promise total:** a
   sentinel that arrives as a *value* cannot be missed by a script that forgot a `try`, cannot
   unwind a `spawn`ed task past its cleanup, and cannot be swallowed by a `catch` that was written
   for an unrelated error. A close is an ordinary end-of-stream event, not an exceptional one.
4. **A send on a closed channel returns the `error{Closed}` sentinel too, and does not lose the
   value silently.** `send` *blocks* when the buffer is full, and then returns `false` if the
   channel closed while it was blocked — the value is returned to the caller, not dropped. See
   `send_with_recovery` in the code below.

```rust
/// The sentinel. One interned `Value` per process, so `recv` on a closed
/// channel allocates nothing: a loop that spins on a closed channel does not
/// turn into a memory leak.
fn closed_sentinel() -> Value {
    thread_local! {
        static CLOSED: Value = Value::Error(Arc::new(ErrorObj {
            kind: ErrorKind::Closed,          // a distinct `ErrorKind`, §4.3
            message: "channel is closed and drained".into(),
            trace: None,
        }));
    }
    CLOSED.with(|v| v.clone())
}

pub fn channel_recv(vm: &mut Vm, args: &[Value]) -> VmResult<Value> {
    let h = arg_handle(vm, args, 0, "recv", ResKind::Channel)?;
    let ch = vm.handles.peek_channel(h).ok_or_else(|| stale(h))?;
    // A *closed* handle is still a catchable NativeError (§5 of
    // 11-ffi-handles.md): the fd is gone, so there is nothing to recv on.
    // A closed *channel* is a normal result: a value.
    match ch.rx.recv() {
        Ok(v) => Ok(bind_affinity(&v, current_thread())),   // §5.1, rebind at handoff
        Err(RecvError) => Ok(closed_sentinel()),            // returns; does not raise
    }
}

pub fn send_with_recovery(ch: &Channel, v: Value) -> SendOutcome {
    match ch.tx.send(v) {
        Ok(()) => SendOutcome::Sent,
        // The value is handed back, not dropped: a producer that cannot
        // deliver must be able to see what it lost.
        Err(SendError(v)) => SendOutcome::Closed(v),
    }
}
```

**Why an `error` value and not a 17th type.** Adding a `sentinel` type to the language for this
would cost a machine tag, and per [ADR-0001](decisions/ADR-0001-tag-budget.md) **all eight are
already spent** — `0b000` nil, `0b001` false, `0b010` true, `0b011` int, `0b100` float, `0b101`
string, `0b110` bytes, `0b111` the nine heap types
(see [`09-memory.md`](09-memory.md) §2.4 for the canonical table, which corrects
[`08-vm.md`](08-vm.md) §8.2). It would also cost a `typeof` case and a JIT guard. An `error` is
already a language type, already `Arc`-backed, already `Send + Sync` (§3.3 row 12), and already
catchable *if* the script wants to treat it as an exception:

```zen
let ch = channel(4)
ch.send(1); ch.send(2); ch.close()
print(ch.recv())      // 1
print(ch.recv())      // 2
print(ch.recv())      // error{closed} — a value, printed, not raised
print(ch.recv().is_error())   // true
```

The two styles compose, which is the actual reason for the choice. A consumer can treat the
sentinel as data —

```zen
// The idiom the docs lead with: no `try`, no exception, no deadlock.
loop {
    let v = ch.recv()
    if v.is_error() { break }        // clean, total loop exit
    handle(v)
}
```

— or as an exception, if that is the style being written:

```zen
let v = try { ch.recv() } catch as e { e }    // identical value, reached by a different path
```

The cost is that the sentinel is a *third* possible result of `recv` alongside "empty" (`nil` from
`try_recv`) and "blocked". The alternative — `nil` on close — is worse, because `nil` is also
"empty", and a consumer cannot distinguish "the producer is slow" from "the producer is gone" and
would spin. A dedicated `sentinel` type is worse still, and §4.3's note explains why in one
sentence.

### 4.4 Multi-channel receive

`crossbeam_channel::Select` does an OS-independent select over N channels, and the M10 API exposes
it directly rather than inventing a "channel group" abstraction:

```zen
let ready = select {
    [ch_in, ch_out]: "io"
    [ch_ctl]:        "control"
    // default: recv_any(ch_in, ch_out)  — whichever arrives first
}
// → (which, value). `which` is the index of the first channel that fired.
```

`select` blocks. `try_select` returns `nil` if none is ready. The Zen syntax is a block form
rather than a macro because the compiler needs to know the arity to check it statically:

| Form | Meaning | Error |
|---|---|---|
| `select { [a, b]: "x" }` | block until one is ready | — |
| `select { default: v; [a]: "x" }` | non-blocking | — |
| `select { [a, a]: "x" }` | duplicate channel | `TypeError: channel listed twice in select` |
| `select { [f]: "x" }` | `f` is not a channel | `TypeError: select: 'f' is a list, not a channel` |

An honest limitation: **`select` is O(n) in the number of channels and rebuilds its internal
`Select` each call.** With ≤ 8 channels (the overwhelming majority) that is 8 relaxed atomic loads
and is faster than a syscall-based `poll`. With 1000 channels it is measurably slower than
`epoll`. The docs say "use `net` for more than a handful"; the implementation does not
auto-switch to `epoll`, because a `Select` over N crossbeam channels cannot be mixed with a
tokio reactor without a `poll` bridge, and a bridge that exists only for a number nobody uses is a
bug magnet.

---

## 5. Deep copy on send

### 5.0 The handoff sequence, because `to_thread` is not knowable at `send`

**The bug this section exists to prevent.** `deep_copy(v, to_thread)` asks for the *destination*
thread. At a `send` there is no destination thread: a channel may have four consumers, or none, and
whichever one calls `recv` first is the one that will own the value. A first draft of this
document stamped the copy with the *sender's* thread, which is equally wrong — it produces a
container the receiver is not allowed to mutate, so the very next `append` raises an affinity error
on a value that was supposed to work.

**Ruling: the value is `UNBOUND` between the two, and the receiver binds it.**

| Step | Thread | Action | Why here |
|---|---|---|---|
| 1. `send(ch, v)` | sender | check `v.affinity() == self` (or `UNBOUND`) | a single-threaded read of `v` is the precondition for a correct copy |
| 2. `send(ch, v)` | sender | `deep_copy(v, AFFINITY_UNBOUND)` | the copy is made **here**, on the owning thread, and every container it rebuilds is tagged `UNBOUND` |
| 3. handoff | — | the channel owns the only reference | between 2 and 4 the value is unaliased, and that is what makes step 4 legal |
| 4. `recv()` | receiver | `bind_affinity(v, current_thread())` — a walk that stores this thread's id into every container's `ObjHeader.affinity` | the container is provably unaliased, so rebinding a tag is not a data race; it is a move |

`AFFINITY_UNBOUND` is a third value in the affinity tag alongside a thread id and `ANY` (which means
"explicitly shared, any thread, protected by a mutex"). `UNBOUND` means "owned by whoever currently
holds the only reference, bind me on first local mutation". It is never observable at the language
level: there is no syntax that reads a value's affinity, and every mutation path calls
`bind_if_unbound` first, so a script cannot trip over it.

**Cost:** one `u32` store per container, on the receive path, for values that contain any mutable
container. For the `bench_channel` case in §12 — a 1000-item loop of `ch.send(i)` with `int`
payloads — there are no containers, so the walk is a single `tag == 0b111` test and it costs
nothing. §12's table lists it as a measured line rather than guessing.

### 5.1 The exact function

```rust
// zen-vm/src/deepcopy.rs

/// Recursive copy for the cross-thread send path.
///
/// # Correctness
/// Three rules, and the whole function is them:
///   1. Immutable payloads (string, bytes, tuple-of-immutables) return an
///      `Arc` clone: O(1), and sharing is safe because nothing can mutate
///      them.
///   2. Mutable containers are rebuilt with fresh `Arc`s whose affinity tag
///      is `AFFINITY_UNBOUND` — **not** a thread id. §5.0: the destination
///      is unknown at `send` time, so the value is unbound in transit and
///      `bind_affinity` stamps it at `recv`. This is what makes the copy
///      independent rather than a second reference to the same buffer.
///   3. Everything else — `func`, `error`, `handle`, `channel`, `mutex` —
///      is `Arc`-shared, and the *share* is validated by §3.6 at the send
///      site before we get here.
pub fn deep_copy(v: &Value, affinity: u32) -> Value {
    match v {
        // ── 1. free ──────────────────────────────────────────────────
        Value::Str(s)   => v.clone(),               // Arc<str>, O(1)
        Value::Bytes(b) => v.clone(),               // Arc<[u8]>, O(1)
        Value::Int(_) | Value::F64(_) | Value::Nil
        | Value::Bool(_) | Value::Handle(_)        => v.clone(),   // 8 bytes

        // ── 2. copied ───────────────────────────────────────────────
        Value::List(items) => {
            let out: Arc<Vec<Value>> = Arc::new(
                items.iter().map(|x| deep_copy(x, affinity)).collect());
            Value::list_with_affinity(out, affinity)
        }
        Value::Dict(m) => {
            let out: Arc<IndexMap<Value, Value>> = Arc::new(m.iter()
                .map(|(k, val)| (deep_copy(k, affinity), deep_copy(val, affinity)))
                .collect());
            Value::dict_with_affinity(out, affinity)
        }
        Value::Set(m) => { /* as dict */ }

        // ── 3. shared, with a per-element walk ──────────────────────
        Value::Tuple(items) => {
            // A tuple is immutable, so the tuple itself is Arc-shared; but
            // a mutable *element* inside it would then be shared too, which
            // is unsound. So a tuple containing a mutable container is
            // rebuilt. Detected in one pass, no allocation when clean.
            if items.iter().all(is_immutable_payload) {
                v.clone()                                        // O(1)
            } else {
                Value::tuple(Arc::new(items.iter()
                    .map(|x| deep_copy(x, affinity)).collect()))
            }
        }

        // ── validated by the caller, then shared ────────────────────
        Value::Obj(o) if o.kind() == ObjKind::Func     => v.clone(),   // cells re-checked
        Value::Obj(o) if o.kind() == ObjKind::Error    => v.clone(),
        Value::Obj(o) if o.kind() == ObjKind::Handle   => v.clone(),
        Value::Obj(o) if o.kind() == ObjKind::Channel  => v.clone(),
        Value::Obj(o) if o.kind() == ObjKind::Mutex    => v.clone(),   // Arc<Mutex<Value>>
        Value::Obj(o) if o.kind() == ObjKind::Instance => deep_copy_instance(o, affinity),
        // A tag with no case is a **bug in this function**, not a
        // fall-through: an unhandled kind would silently share a mutable
        // object. `clippy::exhaustive_patterns` + a CI grep on the
        // ObjKind enum's variant list.
    }
}
```

The last comment is the important one. The failure mode of a deep-copy function is a `match` arm
that is missing, which silently shares a mutable object and reintroduces the race the whole design
removes. The mitigation is a CI check that greps `ObjKind`'s variant list and asserts every
variant appears in `deep_copy.rs`.

### 5.2 The cost, measured where possible and estimated where not

| Type | Size | Copy cost | Notes |
|---|---|---|---|
| `int`, `float`, `bool`, `nil` | 8 B | **0 ns** | The struct is 8 bytes and `Copy` |
| `string` (100 B) | 100 B | **~4 ns** | `Arc::clone`: one atomic increment |
| `string` (1 MB) | 1 MB | **~4 ns** | Still one atomic increment. **This is the reason `string` is `Arc`-backed** |
| `bytes` (1 MB) | 1 MB | **~4 ns** | Same |
| `list` of 10 ints | 80 B + 24 | **~45 ns** | One `Vec` allocation + 10 `Value` copies |
| `list` of 1000 ints | 8 KB | **~900 ns** | One allocation + 1000 copies; ~9 ns/element |
| `list` of 1000 strings (100 B each) | 8 KB + 100 KB | **~14 µs** | 1000 `Arc` clones (4 ns each) + the outer `Vec`; **no string bytes are copied** |
| `dict` of 1000 pairs | ~40 KB | **~2.2 µs** | `IndexMap` rebuild: hashing 2000 keys is the dominant cost |
| `tuple` of 10 immutables | 80 B | **~4 ns** | `Arc` clone |
| `tuple` containing a mutable list | varies | same as the list | The one case where a "cheap" tuple is not |
| `instance` (5 fields) | ~200 B | **~180 ns** | 5 deep copies plus a new shape |
| `func` | 64 B | **~4 ns** | `Arc<[Value]>` clone; cells are validated, not copied |

**The headline number**: deep-copying a 1000-element list of short strings costs about 14 µs, and
copying a 1 MB string costs 4 ns. **Copying is cheaper than a mutex acquisition on a contended
lock, and 300 000 times cheaper than a context switch.** That is the whole argument of ADR-0003's
"always share via `Arc<Mutex<T>` is correct, but every shared value pays a lock; deep copy is
faster than a lock in the overwhelmingly common case."

### 5.3 The escape hatch

When a copy is genuinely too expensive — a 100 MB list, a deeply nested structure, a hot loop that
sends the same 1 M-element list in a loop — the answer is `shared`, written in the source:

```zen
shared cache = {}                                  // becomes Arc<Mutex<dict>>
worker.send_shared(cache)                          // zero copy, one lock
```

| Mechanism | Copy? | Lock? | Visible in source? | Use when |
|---|---|---|---|---|
| `send(ch, v)` with a mutable `v` | yes | no | implicitly | the default; almost always right |
| `shared x = …; send(ch, x)` | no | yes, on every access | **yes** | the value is large *and* genuinely shared *and* the contention is low |
| `ch.send_shared(v)` | no | yes | **yes** | a one-off handoff of a large mutable value |
| `atomic.int()` / `atomic.ref()` | no | no (a CAS) | **yes** | counters, flags, single scalars |

**The documented rule, and it is a rule rather than a heuristic:** *if a profile shows a send in the
hot path, the answer is `shared`, not a bigger copy budget.* A script that reaches for `shared` on
its first send has learned the wrong lesson, and the docs lead with the copy cost table for that
reason.

---

## 6. `shared`, `mutex`, and atomics

### 6.1 Syntax

```zen
// shared — the binding is Arc<Mutex<Value>>. Assignment goes through a lock.
shared xs = [1, 2, 3]
shared counter = 0

// lock / unlock, explicit
xs.lock()            // acquires the mutex, returns the value
xs.unlock()          // releases
xs.locked()          // → bool, is the mutex currently held? (advisory)

// with — the block form; releases on every exit path, including a raise.
with xs {
    xs.append(4)
}                   // released here, even if the body raised

// mutex — the value type, for when the lock is a first-class value
let m = mutex.new(0)
with m { m.replace(m.get() + 1) }

// atomics — a lock-free cell, for counters
let n = atomic.int(0)
n.add(1)            // → the new value
n.get()             // → int
n.compare_exchange(expected, new)   // → true on success
n.swap(7)           // → the old value
```

`with` is the one construct that needs a new opcode or a compile-time desugaring. **Ruling: `with`
is desugared in the M4 compiler into `LOCK; <body>; UNLOCK` with a handler push**, which makes it
exception-safe for free under the existing unwinder ([`08-vm.md`](08-vm.md) §5.4 — the unwinder
only has to know about `vm.handlers`, and the `LOCK` op pushes one).

```rust
// The desugaring, exactly. `LOCK` pushes a cleanup record that pops the
// mutex on any exit path; `UNLOCK` pops it and marks the record dead.
with xs { BODY }   ==>   LOCK xs, cleanup_slot
                          PUSH_HANDLER cleanup_slot
                          BODY
                          UNLOCK xs
                          POP_HANDLER
```

This is why `with` is a keyword and not a `std` helper: a helper function cannot hold a lock across
a Zen call without an `&mut Vm` aliasing problem (§2.5 of
[`11-ffi-handles.md`](11-ffi-handles.md)), and cannot release it on a raise.

### 6.2 Ownership rules, stated as rules

| Rule | Enforcement | Error |
|---|---|---|
| `shared` bindings cannot be rebound or deleted | Compiler | `TypeError: 'xs' is a shared binding and cannot be rebound; use xs.replace(v)` |
| `shared` bindings can only be created with an initialiser | Compiler | `SyntaxError: shared requires an initialiser` |
| `shared` cannot be applied to `handle`, `channel`, `func` | Compiler | `TypeError: shared is not valid for a handle; handles are already movable` |
| `shared` cannot be applied at all to an immutable value | Compiler | `TypeError: 's' is a string and is already safe to share; drop the shared` — a *warning*, not an error, because a program that upgrades a binding to `shared` should not break |
| Reading a `shared` binding without a lock is allowed | — | Reading an `Arc<Mutex<Value>>` takes the lock implicitly |
| Writing a `shared` binding takes the lock implicitly | — | — |
| A `shared` value cannot be deep-copied on send | §5.3 | It is *already* shared, so `send` `Arc`-clones it. Not an error, but a doc note |
| `with` on a non-`shared` binding | Compiler | `TypeError: 'with' needs a shared binding or a mutex; 'xs' is a list` |
| `with` nested twice on the **same** binding | Runtime | `NativeError: deadlock: 'xs' is already locked on this thread`. Detectable, because the mutex records its owner thread |
| `with` on a `shared` binding from **two** threads | — | **Legal, and it blocks.** That is what a mutex is for. The doc warns, and the watchdog (§10.3) reports it if it stalls |
| `mutex.new(v)` where `v` is a `handle` | Compiler | `TypeError: a handle is already thread-movable; wrapping it in a mutex does nothing` |

### 6.3 Atomics

```rust
// zen-vm/src/atomics.rs
pub struct AtomicCell {
    /// The `Value` is stored behind the atomic as a `u64` — this is the one
    /// place `Value::to_bits`/`from_bits` is used outside `value.rs`, and it
    /// is why that unsafe pair is already in the allow-list.
    bits: AtomicU64,
}
```

| Type | Native | Operations | Cost | Use for |
|---|---|---|---|---|
| `atomic.int(i)` | `AtomicI64` | `add`, `sub`, `get`, `swap`, `compare_exchange` | 1–20 ns uncontended | counters, indices, flags |
| `atomic.bool(b)` | `AtomicU8` | `get`, `set`, `swap` | ~1 ns | one-shot flags |
| `atomic.ref(v)` | `AtomicPtr<Value>` + a leak counter | `get`, `set`, `swap` | ~5 ns | a shared scalar of any type |

**`atomic.ref` is the one with a cost the docs must state:** storing a `Value` through an atomic
pointer *takes ownership* of the old value and drops it, so a reader that loaded a pointer before
the swap can be reading freed memory. The design is therefore: `atomic.ref` **never frees** — old
values are leaked into a per-atomic slab and reclaimed when the atomic is dropped. That is a
deliberate, bounded leak: a `Value` is 8 bytes, an atomic's slab grows by 8 bytes per store, and
an atomic that stores 10⁷ times leaks 80 MB. The doc says "an `atomic.ref` that is written in a
hot loop will grow; use `atomic.int`", and the M9 GC, when it lands, replaces the slab with
`Gc<T>` and the leak goes away.

Everything else in the `shared` surface is a `parking_lot::Mutex` (0.12.5, `zen/Cargo.toml:38`),
which is 15 ns uncontended and roughly 2× faster than `std::sync::Mutex` — and, critically, does
not poison, which matters because a poisoned `std` mutex would make every post-panic access a
different error type.

---

## 7. Async

### 7.1 The hard design point, stated honestly

Everything up to here is implementable. This section is not, and the reason is worth writing down
before the ruling, because it is the reason the ruling is what it is.

**The problem.** Zen's network layer is `tokio` ([`13-networking.md`](13-networking.md) §5). Tokio
is async: `TcpStream::connect(&addr).await` is a `Future` that resolves when the kernel says so.
A Zen script has no `await`. So a Zen script that calls `net.tcp.connect("example.com:443")` must
do one of three things:

1. Block the OS thread on the future with `Handle::block_on` — correct, works, costs a thread.
2. Spin a background thread that drives the runtime and `block_on` it — correct, works, costs a
   thread, and adds a scheduling hop.
3. Make the whole language async — `await` as a keyword, a state-machine transform in the
   compiler, and a decision about what a JIT does with a suspended frame.

Option 3 is where the difficulty is, and it is *not* a parser problem.

### 7.2 Why `await` is expensive here specifically

A state-machine transform is standard and well-understood. What is not standard is what happens
to the M11 JIT.

| Concern | Detail |
|---|---|
| Cranelift has no coroutine support | There is no "compile this function with a suspend point". A suspended frame must spill its registers somewhere |
| A spill is possible, and it is a disaster | If a JIT-compiled `await`ing function spills to a heap-allocated frame, then every *non*-awaiting function it calls must also be spillable, which means the whole call chain becomes a state machine, which means the JIT stops being a JIT |
| The escape hatch exists and is cheap | A `FunctionProto` already has flags. `NO_JIT` is one more. A function containing `await` is marked `NO_JIT` and always runs in the interpreter, whose frames are already heap-allocated |
| The cost of the escape hatch | Every `await`ing function is interpreted forever. For a language whose headline is *faster than CPython*, an interpreted async path is a visible regression on exactly the workloads (web servers, scrapers) where people reach for async |
| The M11 JIT does not exist yet | So `await` can be designed *around* M11 rather than retrofitted into it. That is a real asset and it is temporary: the M11 author inherits the `NO_JIT` carve-out and has to live with it |

### 7.3 Ruling: M10 ships a blocking facade over a tokio runtime; `await` is M13

**M10 (this milestone) — `spawn_async`, honestly documented as one thread per task.**

```zen
let t = spawn_async(fn() {
    let s = net.tcp.connect("127.0.0.1:8080")   // blocking: the task's own thread parks
    return s.recv(1024)
})
print(t.result())      // blocks the *calling* thread until the task finishes
```

```rust
// zen-vm/src/async_rt.rs
/// A Zen async task. One OS thread, one `Vm`, one current-thread tokio
/// runtime. `spawn_async` is NOT a green thread and does not free its
/// thread; it exists so a script can *overlap* N I/O operations across N
/// threads, which is the same thing `spawn` gives you, except the futures
/// are driven by a reactor rather than by blocking syscalls.
pub fn spawn_async(vm: &mut Vm, proto: ProtoId) -> VmResult<TaskHandle> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;                                  // one per task, M10
    let (tx, rx) = crossbeam_channel::bounded(1);
    std::thread::Builder::new().stack_size(STACK_SIZE).spawn(move || {
        let mut vm = Vm::new_with(/* … */);
        let out = rt.block_on(async {
            let v = vm.run_function(proto, &[])?;   // ← runs the Zen body
            Ok(v)
        });
        let _ = tx.send(out);
    })?;
    Ok(TaskHandle::new(rx))
}
```

**What a script that does blocking IO inside `spawn_async` gets at M10:** it blocks that task's
dedicated thread. It does not deadlock the program, does not block any other task, and does not
starve a reactor — because the reactor is single-threaded and *inside* that thread, so a blocking
syscall blocks the reactor too, which means that task's *other* pending operations stall until the
syscall returns. That is a real limitation and the documentation says it in one sentence: **"a
blocking call inside `spawn_async` stalls that task's other operations. It does not affect other
tasks. Use `spawn`, not `spawn_async`, if you intend to block."** That sentence is the entire
M10 async story, and it is honest.

**What M10 gains from this design**, and it is not nothing:

| Gain | Detail |
|---|---|
| The network layer is written once, async | `hyper`, `hickory`, `tonic` are all async-only. A blocking facade means there is exactly one networking implementation ([`13-networking.md`](13-networking.md) §5), not a blocking one and an async one |
| Overlap | 200 concurrent HTTPS requests on 4 threads is 4 at a time here, but the *code* is the code that scales when M13 lands |
| `net.http` streaming works | A streaming response is a future, and `block_on` drives it |
| The M13 migration is mechanical | The `net` natives are already `async fn` returning a future; M13 changes the *caller* from `block_on` to a state-machine await, and changes nothing inside `net` |

### 7.4 What M10 does not have

| Not in M10 | Why |
|---|---|
| `await` as a keyword | §7.5 |
| Freeing the task's thread during I/O | Requires the async transform |
| `async`/`await` in the parser | Parser changes are cheap but a keyword the compiler cannot yet honour is worse than no keyword |
| A multi-threaded tokio runtime | M10 uses `new_current_thread` per task. A shared multi-thread runtime is a *better* M10 design and a worse M10 design simultaneously: it shares one reactor across all tasks, so a blocking call stalls *every* task. With per-task runtimes, the damage is contained. **This is the ruling and it is the one to argue about** |
| `select` over a tokio reactor and a crossbeam channel | Needs the `poll` bridge of §4.4's caveat. Not built |
| Structured concurrency (`async with`, task groups, cancellation) | A task group is a big API and needs `await` to be expressible |

### 7.5 The `await` transform, if it is built

For the M13 record, because someone will have to write it and the shape is worth pinning down
while the design is fresh:

```
  fn handler(x):
      let a = fetch(url)          # blocking in M10, async in M13
      let b = compute(a)
      return await send_to_db(b)
```

becomes

```
  fn handler$async(x) -> StateMachine:
      local sm = StateMachine { state: 0, arg_a: nil, arg_b: nil, ... }
      loop:
        match sm.state:
          0: sm.arg_a = fetch(url).await; sm.state = 1; continue
          1: sm.arg_b = compute(sm.arg_a); sm.state = 2; continue
          2: send_to_db(sm.arg_b).await; sm.state = 3; continue
          3: return sm.arg_b
```

which needs: an `AWAIT` opcode, a `StateMachine` heap object type, a resume path in the dispatch
loop that re-enters at the saved `state` with a saved register window, and `FunctionProto::NO_JIT`.

| Decision | Ruling | Why |
|---|---|---|
| `await` at M13 | yes | The transform is standard; the cost is the `NO_JIT` carve-out, and M13 is after M11 so the carve-out is a known quantity |
| `await` in M10 | **no** | A keyword the compiler cannot honour is a worse state than no keyword. Shipping `spawn_async` alone is honest; shipping `await` that secretly blocks is not |
| `NO_JIT` on `await`ing functions | yes | The only alternative is a spilling JIT, which is not a JIT |
| `await` outside an `async fn` | syntax error | A non-async function that awaits has nowhere to suspend. The compiler rejects it with `SyntaxError: 'await' is only valid inside an async function` |
| `spawn_async` in M13 | same name, **different implementation** | M10: one thread per task. M13: a task on a multi-thread runtime that frees its thread. The *name* stays; the semantics get better. This is the one API-breaking-by-improvement in the plan and it is documented as such |
| A blocking native inside an `async fn` | **static error** at M13, from `NativeEntry::blocking` | `NativeError: 'sys.read' may block and cannot be awaited; call it from a non-async function or use 'net.tcp.recv'`. §7.6 |

### 7.6 The `blocking` check, which is the reusable part

Whether or not `await` ships, the `BlockingClass` field on `NativeEntry`
([`11-ffi-handles.md`](11-ffi-handles.md) §2.1) is worth having, because it converts a class of
liveness bug into a compile error:

| `BlockingClass` | In a `spawn` body | In an `async fn` body (M13) | In a `pmap` closure (§8) |
|---|---|---|---|
| `NeverBlocks` | fine | fine | fine |
| `MayBlock` | fine — it has its own thread | **compile error** unless the function is named in an allow-list | **compile error**: a blocking call in a rayon worker stalls the whole pool |
| `BlocksForever` | fine | compile error, same as above | compile error |

The `pmap` row is the one that pays off soonest, at M10, with no `await` required. A
`[1,2,3].pmap(fn(x) { sys.read(fd, x) })` is a script that deadlocks itself, and today (Zen 2.x)
it deadlocks silently. With the check it is a `NativeError` naming the native. That alone justifies
the field.

---

## 8. Data parallelism

### 8.1 The API

```zen
let out = [1, 2, 3, 4, 5].pmap(fn(x) { x * x })
// → [1, 4, 9, 16, 25]     ORDER PRESERVED

let out = rows.pmap(fn(r) { expensive(r) })     // rows is a list of dicts
let n   = [1,2,3].pmap(fn(x) { x + 1 }).fold(0, fn(a, b) { a + b })
```

`pmap` is a method on `list`, and also a free function `pmap(list, fn)` for symmetry with
`map`. It is backed by rayon 1.12 (`zen/Cargo.toml:36`).

### 8.2 The contract, in full

This is the part that has to be exact, because rayon makes several of these easy to get wrong.

| # | Rule | Enforcement | Failure mode if violated |
|---|---|---|---|
| C1 | **Results come back in input order.** `pmap([f(a), f(b), f(c)])` returns `[f(a), f(b), f(c)]` | rayon `par_iter().map().collect::<Result<Vec<_>,_>>()` collects in order by construction | A result set where element *i* is the answer for input *i*. `filter` changes this: `pmap` has no filter variant, and if one is added it must return `(index, value)` pairs |
| C2 | **The input is deep-copied per element only if the element is mutable and the closure is `Fn` not `FnOnce`** | `list` elements are `Value`; the `Arc<Vec<Value>>` is shared and each worker gets `&Value` | Sharing the whole list read-only is correct: the workers only read it, and reading is not a race |
| C3 | **The closure must be `Send + Sync`** | Zen's closure is a heap object with an `Arc<[Value]>` of cells. `pmap` accepts a closure with **no captures or only `shared` captures**, checked by the same affinity walk as `spawn` (§3.6) | A closure capturing a thread-affine list would be run on N threads. Rejected at the `pmap` call |
| C4 | **An error in a worker is raised, not swallowed** | The collect is `Result<Vec<Value>, (usize, VmErr)>`; the first error by **lowest index** wins, deterministically | A worker returning `nil` because its closure errored, and the script believing it. This is the failure mode of every parallel construct in every language |
| C5 | **`pmap` runs on the calling thread's pool, so a nested `pmap` is a nested pool** | rayon handles nesting; a work-stealing pool is re-entrant | Deadlock, if the pool were a fixed set of locks. It is not, and that is why rayon is the right dependency |
| C6 | **`pmap` on an empty list returns an empty list and starts no work** | `par_iter()` on an empty slice is a no-op | — |
| C7 | **`pmap` on a 1-element list does not spin up a thread** | rayon's split threshold is ~1 item; below it, `par_iter` runs on the calling thread | Measuring `pmap` on 10 items tells you nothing about parallelism. The benchmark in §12 uses 100 000 |
| C8 | **A panic in a worker is caught and becomes the error of its index** | rayon propagates the panic to `collect()`; we map it to `(index, NativeError: internal error in worker)` | A worker panic unwinding into rayon is a process abort. It is caught at the `collect`, and a script panic in a worker is a `NativeError` (test 16) |

### 8.3 Error propagation, exactly

```rust
pub fn pmap(vm: &mut Vm, list: &Value, f: Value) -> VmResult<Value> {
    let items: Arc<Vec<Value>> = …;                       // shared, read-only
    let closure = check_send_sync(vm, &f)?;               // rule C3
    // rayon wants `Send` closures. A Zen closure is a heap object; the
    // bridge is an `Arc<ZenClosure>` plus a `Send` marker, which is sound
    // because rule C3 already proved the cells are shareable.
    let out: Vec<VmResult<Value>> = rayon::iter::ParallelBridge::bridge(
        items.par_iter().enumerate()
    ).map(|(i, item)| run_one(vm_arc, &closure, item, i))
      .collect();                                        // ORDER PRESERVED (C1)
    // Rule C4: lowest index wins. Deterministic, and it is the index the
    // user can act on.
    for (i, r) in out.into_iter().enumerate() {
        if let Err(e) = r {
            return Err(e.with_note(format!("in pmap worker {i} of {}", items.len())));
        }
    }
    Ok(Value::list(Arc::new(values)))
}
```

The message for a failing worker is:

```
TypeError: worker 417: unsupported operand for `+`: list and int
  at pmap_demo.z:9
  in pmap worker 417 of 100000
```

**Rule C4 needs one more decision: should the remaining workers be cancelled?** Ruling: **no** —
rayon cannot cancel a running closure, and stopping the others would make the error's timing
non-deterministic. All workers run to completion and the first error by index is raised. For a
100 000-element `pmap` that fails at element 3, the user waits for all 100 000. That is honest and
it is the same choice `map` makes when an element raises (the interpreter does not abort the rest
of the loop either, because it has no loop to abort). Documented in one line: **"`pmap` runs every
worker and raises the lowest-indexed error. It is not cancellable."**

### 8.4 `pmap` and the affinity check interact in a way worth stating

A `pmap` closure that captures `shared counter` and calls `counter.add(1)` 100 000 times is
**correct and is the most common use of `pmap`**. It is also the only way a `pmap` can have a
side effect, and the rule is: **a `pmap` closure may only mutate through a `shared` binding or an
atomic.** There is no way to write to the input list from inside a `pmap` closure, because the list
is the input and the results are a new list. This is checked at the closure's construction by
walking its captures and marking each as read-only or `shared`; a capture that is neither is a
`TypeError` before any worker starts.

---

## 9. The 100 % parallel claim, audited

The blueprint claims Zen is "100 % parallel" in the sense that every CPU can be used. Here is
where that is true, where it is approximately true, and where it is false.

| Claim | Verdict | Why | The honest number |
|---|---|---|---|
| "No GIL — all cores are usable" | **True** | There is no global lock anywhere in the VM. `Vm` is per-thread, the IC table is per-thread ([`08-vm.md`](08-vm.md) §6.4), and the only shared state is refcounts on `Arc`s, which are per-object atomic increments, not contention | Any `pmap` on a compute-bound loop reaches ≥ 0.75 × N speedup on N cores (§12) |
| "100 % parallel execution" | **False** | Nothing is 100 % parallel. Thread startup is serial. `spawn` costs 25 µs of one core. Deep copies are serial. The affinity check is serial. Error propagation is serial | 97 % overhead if you `spawn` a thread per 1 µs of work |
| "`spawn` is 1000 msg/sec" | **True, off by 330×** | §2.3 | ~700 k msg/s on a buffered channel |
| "Data parallelism is free" | **False** | Deep copy on send, the affinity check, and error propagation are all serial. Memory bandwidth is a hard ceiling | A memory-bound `pmap` caps at 3–5× on 8 cores, not 8× |
| "All Zen code can be run in parallel" | **Mostly true** | Any `list.pmap` can. A script with a shared mutable dependency cannot, and the language makes that visible rather than silent | — |
| "Channels make concurrent programming safe" | **False as stated** | Channels make *value transfer* safe. Two threads mutating a `shared` list still race — on the mutex, not on the value | Races move from "silent" to "you forgot a lock" |
| "Sockets and I/O are fully parallel" | **True at M10, false for `spawn_async`** | `spawn` gives one thread per socket. `spawn_async` gives one thread per task, and a blocking call inside it stalls that task's other operations | M10: 1 thread per in-flight request. M13: 1 runtime thread for N |
| "`Value` is `Send`, so it is thread-safe" | **False, and dangerous** | §3.1. A `Value` is a `u64`; Rust's `Send` says nothing about what it points at. The affinity tag is the mechanism | 16 types, 4 of which are mutable containers, 0 of which are checked by Rust |
| "Determinism is preserved" | **True, and it is a design commitment** | `pmap` returns results in order (C1) and raises the lowest-indexed error (C4). Two runs of the same script produce byte-identical output, which is [`00-blueprint-review.md`](00-blueprint-review.md) §5.5 | A `pmap` over 100 000 items is deterministic in *value* and non-deterministic in *timing* |
| "Memory does not grow with thread count" | **False** | 8 MiB virtual stack per thread, chosen deliberately so `max_depth` is uniform (§2.4) | 4000 threads = 32 GB virtual, ~96 MB resident |

**The claim the documentation should make instead**, and it is a claim a reviewer can check:

> Zen uses every core. `pmap` on a compute-bound workload achieves at least **0.75 × N** speedup on
> N cores (§12.3). Zen never silently serialises code, and a program that cannot be parallelised
> says why: an affinity error names the value.

---

## 10. Deadlock and livelock guidance

### 10.1 The documented rules

These go in the language manual, verbatim, and they are rules rather than advice because each one
corresponds to a deadlock we can write a test for.

| # | Rule | The deadlock it prevents |
|---|---|---|
| D1 | **No lock held across a blocking call.** `with m { net.http.get(url) }` is an error, not a slow path | Every other thread waiting on `m` waits for the network. One slow HTTP response becomes a full-program hang |
| D2 | **`recv` is only called by the thread that owns the consumer.** A `channel` is not `Sync` (§3.3 row 13) | Two consumers on one `Receiver` split messages nondeterministically, and a `channel` closed by one while the other is blocked is a lost wakeup |
| D3 | **Channels have bounded buffering only.** `channel()` is unbuffered; there is no unbounded queue (§4.2) | A fast producer and a slow consumer consume all memory |
| D4 | **Lock ordering is global.** When a thread needs two `shared` bindings, it always takes them in the order of the source text position of their `shared` declaration, and the compiler warns when it cannot prove it | A classic A-locks-B / B-locks-A cycle. The compiler cannot *prove* global order in a dynamic language, so it warns |
| D5 | **`with` on the same binding twice on one thread is an error**, not a deadlock | It is detectable (the mutex records its owner) and it is always a bug |
| D6 | **A thread that holds no lock and blocks forever is not a deadlock; it is a bug in the program** | Distinguishing the two is the watchdog's job, and pretending otherwise hides both |
| D7 | **A `spawn`ed thread that is never joined is a leaked thread.** `Vm::drop` joins all threads | A test that spawns 1000 threads and drops the `Vm` must not hang. It joins, with a timeout (§10.3) |

D1 is enforced, not just documented: the compiler knows which natives are `MayBlock`
([`11-ffi-handles.md`](11-ffi-handles.md) §2.1), and `LOCK … <body containing a MayBlock native>`
is a compile error. This is the one place where the `BlockingClass` field pays for itself
immediately.

### 10.2 Livelock, which is the harder one

Livelock is a program that makes progress, just not the progress you wanted:

| Livelock | Shape | Detection |
|---|---|---|
| Spin on a flag | `while not flag: pass` on N threads | The watchdog's progress counter (§10.3) sees the counter moving — it cannot distinguish progress from spin. **Not detectable.** Documented, with the fix (use a channel) |
| Mutex convoy | N threads on one hot `shared` counter | Detectable by a contention counter: a `mutex` records its wait count. `sys.mutex_stats()` reports `{contended, total_wait_ns}` per binding |
| Retry storm | An `http.get` that retries in a tight loop | Rate-limited by the `net` layer ([`13-networking.md`](13-networking.md) §10); a `net.http` client has a default cap of 3 retries with exponential backoff |
| Thread thrash | `spawn` in a loop | `max_threads` turns it into a `NativeError` at 4096, long before the OS refuses |

### 10.3 The watchdog

```rust
// zen-vm/src/watchdog.rs
/// `zen --deadlock-watchdog-ms N` (default: off; 5000 in `zen test`).
///
/// Mechanism: every `Vm` bumps `progress: AtomicU64` every 4096 dispatched
/// instructions. A monitor thread samples all live `Vm`s every 100 ms. If
/// NO thread's counter has moved, the program is deadlocked. It prints
/// every thread's state and exits 101.
pub struct ThreadState {
    pub id: u32,
    pub progress: AtomicU64,
    /// `None` = running, `Some(name)` = parked in that native. Set by
    /// `call_native` on entry, cleared on exit, so a parked thread is
    /// distinguishable from a computing one and the report can name the
    /// native.
    pub parked_in: std::sync::Mutex<Option<&'static str>>,
}
```

The report is a *diagnosis*, not just a message:

```
zen: no thread has made progress for 5000 ms — this is a deadlock

  thread 0 (main)    running
  thread 1           parked in net.http.get
  thread 2           parked in chan.recv
  thread 3           running in pmap worker 88 of 100000
  thread 4           running
  channel 2: 3 buffered, 0 senders
  shared 'counter'   locked by thread 1

  4 of 5 threads are parked. The likely cause is thread 1 holding
  'counter' (rule D1) across net.http.get.
```

**False positives, and what is done about them.** A single-threaded script doing
`time.sleep(60)` has one thread that makes no progress. So the watchdog's trigger is **"no thread
has progressed"**, not "some thread has not progressed" — a sleeping main thread with four busy
workers is not a deadlock, and a sleeping main thread alone is not either (it is one thread, and
the rule is that *all* threads must be stuck). The one case that still false-positives is a
single-threaded script in a long blocking call, which the watchdog handles by exempting a lone
thread entirely: **with one thread there is nothing to deadlock.**

`zen test` runs the watchdog at 5000 ms by default, so a deadlocking test *fails with a
diagnosis* rather than hanging CI forever. That is the single highest-value thing in this section:
a deadlock in a test suite is otherwise a 30-minute timeout with no output.

---

## 11. Test plan

28 entries. `M10` = the concurrency milestone. `race` = must be run under
`-Zsanitizer=thread`.

| # | Test | File | What it proves | Gate |
|---|---|---|---|---|
| 1 | `chan_send_recv_basic` | `conc_basic.z` | 1000 sends and 1000 receives on a `channel(64)`; the receiver gets all 1000 in the order sent (a single producer and a single consumer over one channel *is* ordered) | exit 0 |
| 2 | `chan_n_producers_1_consumer` ★ | `conc_basic.z` | 8 producers × 125 items into one channel, 1 consumer. **Three assertions, and the second is the one people get wrong.** (a) *Completeness*: every item arrives exactly once — `sum == 1000` **and** `len(set) == 1000`, because either alone is satisfiable by a duplicate plus a loss. (b) *Per-producer FIFO*: each item is tagged `(producer, seq)` and each producer's 125 items arrive in its own `seq` order, because one producer's sends are totally ordered by its own program order. (c) *No global order claimed*: the interleaving between producers is nondeterministic, and the test asserts nothing about it. A test that asserted a global order would be asserting an implementation detail that the contract does not promise | completeness + per-producer FIFO |
| 3 | `chan_close_drains` | `conc_chan.z` | `ch.send(1); ch.send(2); ch.close(); recv() == 1; recv() == 2; recv().is_error()` — **in that order**, and the third `recv` *returns* the sentinel as a value, it does not raise. A closed channel with buffered values delivers them before reporting closed. Asserts `recv()` returns normally (no `try`, no unwind) — that is the §4.3 rule 3 ruling, and it is the difference between a sentinel and an exception | exact sequence, returns |
| 4 | `chan_close_no_deadlock` ★ | `conc_chan.z` | A consumer blocked in `recv` on an empty channel; the producer drops its handle (goes out of scope). The consumer must wake and receive the sentinel **within 100 ms**, having unwound nothing. Run with the watchdog at 5000 ms so a regression fails rather than hangs. This is the test for the user's requirement stated as an assertion: a closed channel makes `recv` return a sentinel and never deadlocks | wakes < 100 ms |
| 5 | `chan_recv_after_all_senders_gone` | `conc_chan.z` | `recv` on a channel with no senders ever created **returns the sentinel immediately** rather than blocking | immediate |
| 6 | `chan_unbuffered_is_not_unbounded` | `conc_chan.z` | `channel()` with no argument: a producer that sends 3 items with no receiver **blocks** after the rendezvous; the test verifies the block with a 200 ms thread and then joins. Proves §4.2's ruling | blocks |
| 7 | `chan_send_after_close` | `conc_chan.z` | `close(); send(1)` returns `false` and the value is handed back to the caller through the out-parameter, not silently dropped (§4.3 rule 4, `send_with_recovery`) | returns false, value recovered |
| 8 | `chan_try_recv_semantics` | `conc_chan.z` | `try_recv()` on an empty **open** channel returns `nil`; on a **closed, drained** channel returns the sentinel. The two are different and the test asserts the difference — this is §4.3 rule 3, and it is the difference between a consumer that spins and one that exits | both cases |
| 9 | `chan_select_two` | `conc_chan.z` | Two channels, one pre-loaded, one empty: `select` returns the pre-loaded one; `try_select` with both empty returns `nil` | exact |
| 10 | `deepcopy_isolation` ★ | `conc_copy.z` | Thread A does `let x = [1,2,3]; ch.send(x); x.append(4)`. Thread B receives and asserts it got `[1,2,3]` and **not** `[1,2,3,4]`. This is the test that fails loudly if anyone ever changes `send` to `Arc::clone` a mutable container | exact |
| 11 | `deepcopy_nested` | `conc_copy.z` | `ch.send({a: [1,2], b: (3, {c: [4]})})`; the receiver mutates every level and the sender's copy is unchanged. Covers the tuple-walk in `deep_copy` | exact |
| 12 | `deepcopy_string_is_shared` | `conc_copy.z` | A 10 MB `string` sent 1000 times: RSS grows by less than 2 MB. **Proves `send` does not copy strings** and is the regression test for the "cheap because `Arc`" claim | RSS < 2 MB |
| 13 | `shared_mutual_exclusion` ★ | `conc_mutex.z` | 16 threads × 10 000 iterations of `with c { c.set(c.get() + 1) }` on one `shared c = 0`. The final value must be **exactly 160 000**. Any lost update fails. The whole point of the test is that a lost update is a *wrong answer*, not a crash | value == 160 000 |
| 14 | `shared_rebind_is_error` | `conc_mutex.z` | `shared xs = [1]; xs = [2]` is a `TypeError`; `del xs` is a `TypeError`. §6.2's first rule | both raise |
| 15 | `shared_double_lock_same_thread` | `conc_mutex.z` | `with xs { with xs { } }` raises `NativeError: deadlock: 'xs' is already locked on this thread` | raises |
| 16 | `with_releases_on_raise` ★ | `conc_mutex.z` | `try { with c { error("boom") } } catch { }` then `with c { c.set(5) }` succeeds. **Proves the `PUSH_HANDLER` desugaring of §6.1 releases on the unwind path** — the single most likely bug in `with` | second `with` succeeds |
| 17 | `atomic_increment_exact` | `conc_mutex.z` | 16 threads × 10 000 `n.add(1)` on one `atomic.int`. Final value exactly 160 000, same as test 13 but without a lock | value == 160 000 |
| 18 | `spawn_captures_affinity_error` ★ | `conc_affinity.z` | `let xs = [1,2,3]; spawn(fn() { xs.append(4) })` raises `NativeError: spawn: closure captures 'xs' (list, bound on thread 0) and cannot be used from thread 3`. **The thread id in the message is asserted.** Fails on any design without the affinity tag | exact message |
| 19 | `spawn_send_affinity_error` | `conc_affinity.z` | `let xs = [1]; ch.send(xs)` from a worker raises `NativeError: list bound on thread 0 was used from thread 3` | raises |
| 20 | `spawn_1000_short_tasks` ★ | `conc_spawn.z` | `for i in range(1000) { spawn(fn() { i * 2 }) }` and join all 1000. Asserts: 1000 results, all correct, and the wall time is within a stated budget (300 ms on a 4-core CI box). This is the test that gives §2.3's "don't spawn for short work" advice a number | 1000 correct, < 300 ms |
| 21 | `spawn_error_propagates` | `conc_spawn.z` | A spawned function raises `error("child failed")`; `join` returns the error as a Zen value and the parent can `catch` it. **Not** a panic, **not** a lost error. This is the `ThreadResult` design of §2.4 | error propagates |
| 22 | `pmap_order_preserved` ★ | `conc_pmap.z` | `[1..100000].pmap(fn(x) { x % 7 })` returns exactly the sequential result. On a 16-core box this is 100 000 real work items across N threads, and any reordering fails | exact sequence |
| 23 | `pmap_error_propagation` ★ | `conc_pmap.z` | A closure that raises for `x == 417` and returns 0 otherwise. `pmap` must raise `TypeError: worker 417: …` — **and the index 417 must be in the message**, and it must be the *lowest* such index, deterministically, across 5 runs | exact index, 5× stable |
| 24 | `pmap_blocking_native_rejected` | `conc_pmap.z` | `[1,2,3].pmap(fn(x) { sys.read(fd, x) })` is a `NativeError` naming `sys.read` as `MayBlock`. Proves §7.6's `pmap` row | raises |
| 25 | `pmap_nested` | `conc_pmap.z` | `pmap` inside a `pmap` inside a `pmap`, 3 levels deep on 16 cores. Ruling C5: rayon's pool is re-entrant and this must not deadlock | completes |
| 26 | `watchdog_reports_deadlock` ★ | unit (`watchdog.rs`) + `conc_deadlock.z` | A deliberately deadlocked program (two `with c` in opposite orders) is detected in < 100 ms + 5000 ms and the report names the holding thread, the parked natives, and rule D1. **The test asserts the report content**, because a watchdog that says "deadlock" and nothing else is barely better than a hang | report content asserted |
| 27 | `race_detector_build` ★ | CI | `RUSTFLAGS="-Zsanitizer=thread" cargo +nightly test -p zen-conc` over tests 1-25. **Optional gate**: runs nightly, fails the build only on a *new* report relative to a checked-in baseline (`tests/data/tsan-baseline.txt`), because TSan has false positives on `parking_lot`'s parking primitives | nightly, baselined |
| 28 | `loom_model_check` | unit | Three short tests (`Mutex`, channel handoff, the affinity counter) run under `loom` 0.7, which explores every interleaving exhaustively. This catches the races a 16-thread stress test finds only 99 % of the time | 100 % of interleavings |

**Suite shape.** 26 Zen-level and unit tests, 1 nightly TSan gate, 1 exhaustive model check. The
three starred tests — 2, 13, 22 — are the ones that would catch a real regression; the other 23
exist so that a *new* failure has a neighbouring green test to point at.

---

## 12. Measurement

### 12.1 The harness

```rust
// benchmarks/src/par.rs — a harness, not a Zen script, because the claim
// is about wall time and the harness has to control the thread count.
pub struct ParBench { name: &'static str, work: fn(usize) -> u64, items: usize }

impl ParBench {
    /// Run `work` over `items` values, split across `1..=threads` workers.
    /// Returns the median of 5 runs after a warmup run.
    pub fn speedup(&self, max_threads: usize) -> f64 { … }
}
```

```zen
# The Zen-level harness, for scripts that want to measure themselves.
let r = bench_par(fn(lo, hi, n) {
    // Runs `n` times, returning the nanoseconds elapsed.
    return time.monotonic_ns()
}, items: 100000, threads: 0)   # 0 = "all cores"
print(r.speedup())               # e.g. 5.8 on 8 cores
```

### 12.2 What the numbers mean

| Workload | Class | Expected speedup on 8 cores | Ceiling and why |
|---|---|---|---|
| `fib_iter(32)` | compute | **7.0–7.6×** | Dispatch overhead is per-item and each item is thousands of instructions |
| Integer prime sieve, 10⁸ | compute, memory-bound | **4.5–5.5×** | ~400 MB working set; memory bandwidth caps it |
| `sha256` on 10 000 × 1 MB | compute (hardware-accelerated) | **1.2–2.0×** | SHA-NI instructions saturate one core; the memory reads do not |
| List of 100 000 dicts, field read | memory-bound | **3.0–4.0×** | Every worker is chasing pointers into the same heap |
| `deepcopy_1000_strings` on send | memory + allocation | **2.5–3.5×** | The allocator's `malloc` arena lock becomes the bottleneck. This is the workload that argues for `shared` |
| `pmap` over 100 items | **too small** | **1.0×** | rayon does not split below ~1 item per thread; this measures rayon, not Zen |
| `spawn` 1000 empty tasks | overhead-dominated | **0.1–0.3×** | 30 µs of serial `clone` per spawn. **Slower than serial**, and that is the point of §2.3 |
| `sort` of 10⁶ ints, `pmap`-partitioned | compute + memory | **3.5–4.5×** | A parallel sort is not a `pmap`; this is measuring `pmap`'s ceiling, not sort's potential |

### 12.3 The gate, and what number would justify the claim

> **Zen's M10 exit criterion:** `bench_par_speedup ≥ 0.75 × N` on `fib_iter(32)` and
> `≥ 0.55 × N` on the prime sieve, on an 8-core CI machine, with a 16-core run recorded manually
> each release.

**The number that would justify "100 % parallel": there isn't one, and that is the point.** No
workload is 100 % parallel. The strongest defensible claim is:

> Zen uses all cores with no global lock, and achieves **≥ 0.75 × N** speedup on compute-bound
> `pmap` workloads, **≥ 0.55 × N** on memory-bound ones, and **loses** on work smaller than
> ~50 µs per item. All three numbers are measured, and the third one is documented.

The three things that make the gate checkable:

| Requirement | Detail |
|---|---|
| The harness pins the thread count | `rayon::ThreadPoolBuilder::new().num_threads(n)`, not the ambient core count. A benchmark that varies with the CI machine is not a benchmark |
| The gate runs on a fixed machine | An 8-core x86-64 CI runner, `perf`-counter-free, `--profile bench` per `zen/Cargo.toml:61-65` |
| The result is recorded every release | `benchmarks/results/m10_speedup.json`, and a regression of more than 10 % on `fib_iter` is a release blocker, matching the "never slower on any individual benchmark" clause of [`00-blueprint-review.md`](00-blueprint-review.md) §4 |

### 12.4 The cost of the safety mechanisms, measured

Everything in this document has a price and the price goes in the same table as the benefit.

| Mechanism | Cost | Where it shows up |
|---|---|---|
| Affinity tag check on `send` | ~1 ns (a `u32` load + compare, same cache line) | `bench_channel`, < 1 % |
| Affinity tag field in `ObjHeader` | 0 bytes (fits the existing 16-byte header) | none |
| `bind_affinity` walk on `recv` (one `u32` store per container; zero for scalar payloads) | ~1 ns per container | `bench_channel`, < 1 % |
| `UNBOUND` affinity state | 0 bytes (a reserved `u32` value in the existing field) | none |
| Deep copy, 1000-element list of ints | ~900 ns | `bench_channel`, which is why §5.3's table is the thing a user reads |
| `catch_unwind` at every native call | 0 ns when not panicking | `bench_native_call`, < 3 % |
| `BlockingClass` field on `NativeEntry` | 4 bytes per native | none |
| Watchdog thread sampling | 1 thread, 100 ms period, ~2 µs of work per sample | none (off by default; 5000 ms in `zen test`) |
| **Total overhead on `bench_core`** | **< 1 %** | The gate is *no measurable regression* |

---

## 13. Rulings a reviewer should double-check

| # | Ruling | Where | The alternative view |
|---|---|---|---|
| 1 | **`spawn` is an OS thread, not a green thread** | §2.4 | The blueprint's implied design is cheaper per task. The cost is real: 25 µs and 8 MiB per task. The benefit is that a Zen function may block on anything without an explanation, which is worth more in a systems language than in a web framework |
| 2 | **8 MiB stacks, uniform `max_depth` 8192** | §2.4 | 256 KiB stacks would support 4000 threads comfortably. They would also segfault at ~frame 290, and a segfault is neither catchable nor deterministic. The uniformity across threads is what makes the differential harness possible |
| 3 | **Rust's `Send` is useless here; the affinity tag is the mechanism** | §3.1, §3.2 | A reviewer may reasonably want a stronger static guarantee. With a NaN-boxed `Value` there is none to be had, and the honest response is to say so rather than to claim a type-checker feature that does not exist. The cost is one `u32` per object and three check sites |
| 4 | **Deep copy on send, at ADR-0003's stated cost** | §5 | The alternative is `Arc<Mutex<T>` for everything sent, which is what §5.3's measurements reject: a lock is more expensive than a copy at realistic sizes, and it is *always* more expensive for an immutable value |
| 5 | **`recv` on a closed channel *returns* an `error` value, it does not raise, and it is not `nil`** | §4.3 | Two alternatives, both worse. `nil` is simpler and is what Python's queue returns, but it cannot distinguish "empty" from "closed", so a consumer spins. Raising is what a first draft did: it works, but it makes an ordinary end-of-stream event exceptional, so a script that forgot a `try` around a consumer loop dies on normal shutdown, and a `spawn`ed consumer unwinds past its cleanup. The cost of returning is a third possible result of `recv` |
| 6 | **Unbounded channels are banned** | §4.2 | `crossbeam` has `unbounded()` and it is a legitimate tool for a producer that must not block. It is also an OOM waiting for a fast producer. The mitigation would be a cap, at which point it is `channel(n)` |
| 7 | **`pmap` runs every worker and raises the lowest-indexed error; it is not cancellable** | §8.3 | Early cancellation is better behaviour and rayon cannot do it without `rayon::scope` plus a cancellation flag, which turns every closure into a state machine. The cost is that a 100 000-element `pmap` failing at element 3 runs 99 997 more workers. The same thing happens in `map` and users do not complain about `map` |
| 8 | **M10 ships a blocking facade with one runtime per task; `await` is M13** | §7.3 | The alternative is shipping `await` at M10 with a `NO_JIT` carve-out, which is earlier and looks more ambitious. The cost is a keyword the JIT can never compile, forever, discovered before anyone has measured whether async Zen is fast. This is the ruling most likely to be revisited, and §7.5 pins the shape so the revisit is a diff, not a redesign |
| 9 | **A blocking native in a `pmap` closure is a compile error** | §7.6, §8.2 | It could be a runtime warning. A warning is ignored, and the failure mode is a silently stalled rayon pool, which is a hang with no output. This is the first consumer of `BlockingClass` and it justifies the field on its own |
| 10 | **`with` is a keyword with a compiler desugaring, not a `std` helper** | §6.1 | A helper function cannot hold a lock across a Zen call without the `&mut Vm` aliasing problem of [`11-ffi-handles.md`](11-ffi-handles.md) §2.5, and cannot release on a raise. A magic function like `lock(xs, fn() { … })` would work and would not compose with `try`/`catch` |
| 11 | **`atomic.ref` leaks old values rather than freeing them** | §6.3 | A reference-counted atomic would be correct and would have a `Drop` that can run on a different thread than the one that stored the value, which is a `Send` violation the type system would catch. The leak is bounded and M9 removes it. This is a real cost, documented |
| 12 | **The deadlock watchdog is off by default, on in `zen test`** | §10.3 | A watchdog always on is a background thread and a 100 ms sample forever. The value of the watchdog is overwhelmingly in CI, and a test suite that hangs is a test suite that gets `--no-fail-fast`'d and then ignored |
