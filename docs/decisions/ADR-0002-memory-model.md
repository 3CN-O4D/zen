# ADR-0002 — Memory model: `Arc` + CoW now, tracing GC later behind a trait

**Status:** Accepted · **Date:** 2026-09-29

## Context

The blueprint specifies per-thread bump arenas plus mark-and-sweep, with `ArenaChunk::reset()` for
"ephemeral heaps", and channels that move mutable collections between threads.

Review found three independent defects (see [`../00-blueprint-review.md`](../00-blueprint-review.md)
§A2): a bump arena cannot support per-object sweep; `reset()` frees live objects; and the root set
is incomplete. Correcting all three is a large project that must land *before* anything runs, and
its failure mode is memory corruption rather than a wrong answer.

## Decision

**Phase 1 (M2–M8):** Heap objects are allocated as `Box` + `Box::into_raw` with a **manual refcount** (`rc: u32` in `ObjHeader`). Containers (`List`, `Dict`, `Set`, `Tuple`, `String`, `Bytes`) are wrapped in `Arc` for copy-on-write semantics: mutation goes through `Arc::make_mut`, which clones only when the refcount is greater than one. This is exactly Zen 2.x's proven approach, and it is safe by construction.

**Phase 2 (M9):** a two-space copying collector behind a `Gc` trait, specified in
[`../09-memory.md`](../09-memory.md), is implemented **only if** the M4/M5 profile shows refcount
traffic on the critical path. A complete root map is a precondition, not a follow-up.

## Consequences

- Reference cycles leak. Accepted: cycles are rare in scripts, and the JIT work in M11 is worth more
  than cycle collection. Documented in the language manual.
- Channel sends deep-copy mutable containers. Correct, and the cost is invisible at realistic
  script sizes.
- No global GC lock, no stop-the-world — the blueprint's actual goal — is satisfied from day one
  because each thread's refcounts are independent and there is no shared collector.
- `unsafe` stays confined to `Box::into_raw` / manual drop in `zen-vm::value` and `zen-vm::gc`,
  plus the `Gc` implementation later.

## Alternatives rejected

- **Bump arena + mark-sweep** — unsound, see above.
- **Boehm GC / `gc` crate** — extra C dependency, non-portable, and cannot be used from a JIT
  without a stack-map handshake anyway.
- **Immix / `mmap` sweep** — interesting, but only worth it after the GC gate is opened.

---

## Amendment — M2 Implementation (2026-10-01)

The Phase 1 text above stated "containers are wrapped in `Arc` for copy-on-write". The
detailed memory-model spec in [`../09-memory.md`](../09-memory.md) §4.6 subsequently
ruled **Option B**: a **manual `rc: u32` in `ObjHeader`**, raw `*const ObjHeader` for
object identity, and a reimplemented `cow_mut` that clones only when `rc > 1`. Containers
are **not** `Arc`‑wrapped. This matches the Zen 2.x reference semantics without paying
for `Arc` on every allocation. The `Gc` trait (`docs/09-memory.md` §6) provides the
stable interface for the future M9 collector.
