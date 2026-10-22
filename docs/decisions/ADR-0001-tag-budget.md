# ADR-0001 — Tag budget: 8 machine tags, 48-bit payload

**Status:** Accepted · **Date:** 2026-09-29 · **Supersedes:** the blueprint's 4-bit tag design

## Context

A NaN-boxed `Value` must store, in one `u64`, either a raw `f64` bit pattern, an immediate, or a
**pointer**. Pointers are the constraint. On x86-64 the kernel normally keeps userspace below bit
47, but explicitly allows userspace to request allocations above 47 bits, and 5-level paging makes
57-bit userspace real. The kernel's own documentation warns that pointer-tagging JITs "collide with
valid pointers with 5-level paging and lead to crashes".

A 4-bit tag at shift 44 leaves a 44-bit payload, which cannot represent every legal allocation.

## Decision

3 tag bits at shift 48; 48-bit payload. Tag `0b111` covers all heap objects, discriminated by an
`ObjHeader.kind: u8` at offset 0 of every heap allocation.

## Consequences

- No pointer/tag collision on any 48-bit or 57-bit address space. Correctness is not conditional on
  allocator behaviour.
- 8 tags for 16 language types means `is_dict()` costs one extra load. Measured as negligible
  (the object header is in the same cache line as the object data).
- 8 spare tags remain for v2 growth (generational marks, a `range` type) without changing layout.
- `Int` is i48, range ±140,737,488,355,327. Overflow promotes to `float` rather than wrapping.
- Implementation must include a debug-build assertion in `Value::from_ptr` that the address fits
  in 48 bits, so a future 5-level-paging regression is caught in tests, not in production.

## Alternatives rejected

- **4 bits @ 44** — pointer truncation. Rejected for the reason above.
- **`enum Value { ... }` (no boxing)** — Rust enums are 16 bytes for this shape and cost an extra
  load plus a bounds check on tag. Rejected *for the VM hot path only*; the AST in `zen-syntax`
  still uses a normal enum, because parser speed is irrelevant.
