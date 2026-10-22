# ADR-0004 — `handle` is an honest type; `typeof()` never lies

**Status:** Accepted · **Date:** 2026-09-29

## Context

The blueprint defines tag `0xF` as `handle` but specifies that `typeof(handle)` reports `"int"` "for
backward compatibility", because an existing test asserted `typeof(sqlite.open()) == "int"`.

## Problem

The JIT specialises on `typeof`. A `typeof` that disagrees with the representation means the JIT
emits an integer fast path — including the unboxed-i48 path, which for a handle tag is a pointer —
guarded by nothing. The result is not a wrong answer, it is out-of-bounds access on a descriptor
that is not a number. Compatibility with one test assertion is not worth a memory-safety hole in
the optimiser.

## Decision

`handle` is a distinct language type. `typeof(handle_value) == "handle"`. The Zen 2.x test
assertion is **updated**, not the type system. Handles remain small integers (an index into a
global resource table) so the representation stays cheap.

## Consequences

- Every `typeof` in the language is a function of the actual representation. A property the JIT and
  the interpreter can both rely on.
- The `zen::TypeOf` test in `tests/cases/15_sqlite.z` is rewritten from `== "int"` to `== "handle"`.
- `bool(handle)` is true for a live handle, false for a closed one — the ergonomic way to test.

## Alternatives rejected

- **Keep the lie** — unsound in combination with a JIT.
- **Make `handle` an opaque struct** — more type safety, but the underlying value is an integer
  index and pretending otherwise costs a pointer chase for no benefit.
