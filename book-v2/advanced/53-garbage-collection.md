# Chapter 53: Garbage Collection and Memory Management

*How Zen 3.0 reclaims memory, the collector design, reference cycles, and how
to write memory-friendly code.*

## 18.1 Overview

Zen is a garbage-collected language. You do not `free` memory manually. The
runtime automatically reclaims heap objects (lists, dicts, strings, instances,
etc.) when they become unreachable.

The exact collector implementation is build-configured, but the semantics are
consistent: unreachable memory is reclaimed; reachable memory is kept.

## 18.2 What gets collected

Heap-allocated objects:
- strings, lists, dicts, sets, tuples, bytes
- class instances
- closures / function objects
- modules
- FFI foreign handles (with finalizers)

Immediate values (nil, bool, int, float) are not heap-allocated and need no
collection.

## 18.3 Reachability

An object is **reachable** if it can be reached from a root:
- global/module variables
- local variables on the stack
- currently executing frames
- registers holding values
- other reachable objects (transitively)

An object is **unreachable** (garbage) when no root can reach it, directly or
indirectly. The collector reclaims unreachable objects.

## 18.4 Reference counting vs tracing

Zen's collector may use reference counting, tracing (mark-and-sweep), or a
hybrid, depending on build:

| Approach | How it works | Pros | Cons |
|----------|--------------|------|------|
| Reference counting | count references; free at zero | prompt reclamation | cycles leak; per-ref overhead |
| Tracing (mark-sweep) | mark reachable from roots, sweep rest | handles cycles | pause-time; throughput cost |
| Hybrid | refcount + periodic cycle collection | prompt + cycle-safe | more complex |

## 18.5 Reference cycles (the classic leak)

Two or more objects referencing each other form a cycle. With pure reference
counting, their counts never reach zero even if nothing outside references them.

```zen
class Node {
    var peer = nil
}
let a = new Node()
let b = new Node()
a.peer = b
b.peer = a   # cycle
# drop external refs:
a = nil
b = nil
# a and b still reference each other → leak (pure refcount)
```

With a tracing or hybrid collector, the cycle is detected and collected.
With pure refcounting, break it manually:

```zen
a.peer = nil
b.peer = nil
```

## 18.6 `shared` and thread-shared memory

`shared x = expr` wraps the value in an `Arc<Mutex<_>>`-like container. The
wrapper itself is reference counted; the inner `Mutex` protects the value for
concurrent access. The wrapped value is freed when the last shared reference
drops.

## 18.7 Writing memory-friendly code

1. **Drop large references when done:** `big = nil` after processing a huge
   list helps the collector reclaim it sooner.
2. **Avoid unbounded growth:** don't append to a global list forever; cap it or
   evict old entries.
3. **Break cycles in graphs:** null out back-references when tearing down
   object graphs.
4. **Close resources promptly:** files, sockets, FFI handles — use `with` /
   `finally` rather than waiting for GC.
5. **Prefer immutable values:** fewer surprise aliases means easier reasoning
   about lifetime.
6. **Don't micro-manage:** trust the collector; optimize only after measuring.

## 18.8 Memory leaks to watch for

| Leak | Cause | Fix |
|------|-------|-----|
| reference cycle | A↔B mutual refs | break one side |
| growing global cache | no eviction | size-limit / LRU |
| unremoved event listeners | callbacks hold refs | unsubscribe |
| open files/sockets | not closed | `with` / `finally` |
| FFI handle not freed | missing finalizer | call release in `finally` |
| closure capturing large object | closure outlives need | nil the captured ref |

## 18.9 GC and performance

Collection pauses can affect latency-sensitive code. If your build exposes GC
tuning:
- increase the allocation threshold to collect less often (higher throughput,
  larger pauses)
- decrease it for more frequent, smaller pauses (better latency)

Most applications need no tuning.

## 18.10 Exercises

1. Draw a reference cycle between two objects and mark which refs to break.
2. Write a tiny object graph, drop external refs, and explain what the GC sees.
3. Show how `big = nil` helps reclaim a large list.
4. Explain why a global cache that never evicts leaks memory.
5. Write `with_resource` using `try/finally` for a file.
6. Describe the difference between reachable and unreachable.
7. Name three heap types and three immediate types.
8. Why does `shared` need both refcounting and a mutex?
9. Implement an LRU cache with eviction (conceptually).
10. What can you do if GC pauses hurt your application's latency?

'''
print('gc done')

## 18.12 Memory model — phases

Zen ships **Phase 1 (M2–M8): reference counting with copy-on-write (CoW)
containers**. **Phase 2 (M9): a two-space tracing copying collector** behind a
`Gc` trait, implemented only if the M4/M5 profile shows refcount traffic on
the critical path. A complete root map is a precondition, not a follow-up.

The collector is reached through a free function `gc()`, not a `Vm` field, so
the dispatch loop never changes shape.

## 18.13 Heap object layout — exact offsets

Every heap allocation begins with a 16-byte, 8-aligned `ObjHeader`:

```rust
#[repr(C, align(8))]
pub struct ObjHeader {
    pub kind:  u8,   // +0   ObjKind discriminant
    pub marks: u8,   // +1   M9 GC mark bits; 0 in M2–M8
    pub flags: u16,  // +2..4 object flags
    pub shape: u32,  // +4..8 shape id; 0 = Unshaped
    pub rc:    u32,  // +8..12 manual strong reference count
    pub size:  u32,  // +12..16 total allocation bytes, header included
    // +16 … payload
}
```

`assert!(size_of::<ObjHeader>() == 16)` and `align_of == 8` are compile-time
asserted; every field offset is compile-time asserted so the JIT's constant-
folded field offsets and the M9 forwarding-pointer rule cannot silently break.

### The flags byte

| Bit | Name | Meaning |
|-----|------|---------|
| 0 | CLOSED | channel or mutex is closed; read by `ITER_DONE`/`CHANNEL_RECV`. |
| 1 | HAS_TRACEBACK | error's traceback captured; set once at the `THROW` site. |
| 2 | HAS_SENDERS | channel has ≥1 blocked sender; M10 scheduler hint. |
| 3 | FROZEN | instance is frozen; rejects `SET_FIELD`. |
| 4..15 | reserved | must be 0. |

## 18.14 Every heap type's Rust representation

| Zen type | Value payload | Header? | Allocation | Clone cost | Drop path |
|----------|---------------|---------|------------|------------|-----------|
| string | `*const u8` (Arc<str>) | no | `Arc` | 1 atomic inc | `Arc::from_raw` |
| bytes | `*const u8` (Arc<[u8]>) | no | `Arc` | 1 atomic inc | `Arc::from_raw` |
| list | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | release each element, then free |
| dict | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | release keys AND values, then free |
| set | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | release each element, then free |
| tuple | `*const ObjHeader` | prefix | 1 | 1 atomic inc | release each element, then free |
| func | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | drop upvalue `Arc`s |
| instance | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | release each slot |
| error | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | release `val`; msg/tb self-drop |
| channel | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | queue's Values, then mutex |
| mutex | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | release inner Value |
| handle | u32 table index | no header | none | 1 table inc | 1 table dec |

`handle` is an index into a process-global handle table (ADR-0004 honest
types): distinct `typeof()`, no refcount, no header, no `unsafe`.

`string`/`bytes` have their own tags so `is_string()` is a pure tag compare
with zero loads; `is_dict()` costs one header load.

## 18.15 Copy-on-write (CoW)

An assignment between two Zen bindings is **O(1) and allocates nothing**,
regardless of container size: `d2 = d` for a 100,000-entry dict is one 8-byte
move + one atomic increment.

Mutation goes through `cow_mut` (a manual `rc == 1` substitute for
`Arc::make_mut`):
- if `rc == 1`: mutate in place (fast path).
- if `rc > 1`: detach — allocate a fresh object, copy the payload, release the
  old reference, install the new.

Cost table (`n` = element count, `b` = string bytes):

| Operation | Cost | Allocates? | Atomic RMW? |
|-----------|------|------------|-------------|
| `d2 = d` | 1 increment | no | 1 |
| pass/return container | 1 increment in CALL/RETURN | no | 1 |
| `d.a = v`, rc==1, key exists | slot store + release + retain | no | 2 |
| `d.a = v`, rc>1 | **full clone of n** + release + retain | yes, n | 3 |
| `list.append(x)`, rc==1 | possible Vec grow, amortised O(1) | maybe | 2 |
| `s + t` | alloc b1+b2 + 2 memcpy | yes, O(b) | 3 |

The row that decides whether a real program is fast: `d2 = d; d2.a = 1` —
cloning a whole container for a single field write. Mitigation: a compiler
escape-analysis `unique` hint / `COW_SET_FIELD` with a `#[cold]` clone path
(M5).

## 18.16 `Value` is `Copy` — and therefore leaks by default

`Value` is 8 bytes and must be `Copy` (register file, inline caches read
`ic[site]`). `Copy` means **no destructor**, so `registers[a] = v` **leaks**
whatever was there. The VM must go through an explicit `store` that `release`s
the old heap reference:

```rust
pub fn store(&mut self, a: u16, v: Value) {
    let old = self.registers[a as usize];
    if old is heap/string/bytes { release(old) }
    self.registers[a as usize] = v;
}
```

Rules: never write `registers[a] = v` directly; every `push(v)` into a
`Vec<Value>` is preceded by `retain(v)`; in-place slot writes retain-then-
release, never just assign.

## 18.17 The unsafe audit list (complete, M2–M8)

All `unsafe` lives in `value.rs` (nine items):

1. `Value` ↔ `u64` bit punning (NaN-boxing).
2. `from_header` / `header_of` — rebuilding a `Value` from a raw pointer.
3. `rc_ptr` — `u32` as `AtomicU32`.
4. `str_len` / `bytes_len` — reading length behind a thin pointer (tested
   empirically in CI on every platform).
5. `release` — the whole drop path.
6. `new_tuple` and `Layout::extend` payloads — writing through a raw pointer
   into fresh memory.
7. `alloc`'s six initialisation stores.
8. `cow_mut`'s detach path — guarded on `rc == 1`.
9. the eight hand-written `Drop` impls — each calls `release`.

`gc.rs` is `unsafe`-free in M2–M8; M9's mark/forwarding writes are the only
`gc.rs` `unsafe`.

## 18.18 Reference cycles leak (by design)

A reference-counted collector cannot collect a cycle (A → B → A). Zen accepts
**bounded cycle leaks**: RSS grows by about one cycle's worth then stops. An
unbounded leak is a bug; a bounded one is a language property. Break cycles
manually by nilling a back-reference. M9's tracing collector (if built) handles
cycles.

## 18.19 String concatenation — the quadratic build loop (known issue)

```zen
s = ""
i = 0
while i < 200_000 {
    s = s + "x"
    i = i + 1
}
```

With M2–M8's `Arc<str>` where `s + t` always allocates and copies both, this
is **O(n²)**: ~20 GB allocated/copied for 200,000 iterations — roughly
7.5–10 s, 25–50× slower than CPython. String building is what a script *is*,
so this is called out deliberately.

Cheap mitigations inside M2–M8 (no representation change):
1. Constant folding of literal `+`.
2. `BUILD_STRING n` for n>2 operands (one alloc, one copy).
3. `join(list)` / `split(s, sep)` — one result allocation.
4. A `StringBuilder` in `zen-std` (~40 lines of safe Rust).

The M11 amendment (§5.5 of 09-memory.md) changes `string` to
`StringObj { len, cap, bytes }` so the JIT can append in place — gated behind
the same feature as the JIT, because they are one piece of work, and requires
a `.zeno` format version bump plus a new ADR.

## 18.20 Exercises

1. Draw the 16-byte `ObjHeader` with every field offset and size.
2. Why does `is_string()` cost zero loads but `is_dict()` cost one?
3. What does `cow_mut` do when `rc == 1` vs `rc > 1`?
4. Why is `registers[a] = v` a leak, and what must replace it?
5. Name the nine `unsafe` items and where they live.
6. Why does a reference cycle leak, and what does Zen do about it?
7. Show that `d2 = d; d2.a = 1` clones the whole container (rc>1 path).
8. Why is `s = s + "x"` in a 200,000-iteration loop O(n²)?
9. What is the M9 tracing collector's precondition (complete root map)?
10. What is the M11 string representation change, and what gates it?

