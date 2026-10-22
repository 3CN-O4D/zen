# ADR-0003 — Concurrency semantics: explicit sharing, deep copy on send

**Status:** Accepted · **Date:** 2026-09-29

## Context

The blueprint requires that "sending mutable data (list, dict) over a channel transfers ownership,
automatically setting the sender's reference to `nil`". This is a linear type system.

## Problem

For the sender's binding to become `nil` at runtime, the compiler must know the value is a mutable
collection, enumerate every alias, invalidate all of them, and reject the program if an alias
escapes. Zen is dynamically typed, has `dict.get`, supports reflection, and lets native modules
retain references. The analysis is not implementable.

It is also *fundamentally* incompatible with a moving tracing collector: a copying GC relocates
objects, so "the pointer now belongs to another thread" is not a stable invariant, and the
collector would need global knowledge of thread ownership — precisely the coordination the design
claims to avoid.

## Decision

- `string`, `bytes`, `tuple` are `Arc`-shared across threads at zero copy. They are immutable.
- `list`, `dict`, `set` sent on a channel are **deep-copied**.
- Explicit sharing is opt-in and visible: `shared x = [1,2,3]` produces `Arc<Mutex<T>>`.
  A non-`shared` mutable value touched from a second thread raises a catchable Zen error naming the
  value, not a data race.

## Consequences

- Races are impossible by default. They are a class of bug the *type system* prevents, rather than
  a class of bug the runtime must detect at 3 a.m.
- `spawn` + `channel` is safe without reading the docs.
- Costs one copy per cross-thread mutable send. Measured in [`../18-benchmarks.md`](../18-benchmarks.md)
  (`bench_channel`); accepted.

## Alternatives rejected

- **Linear move on send** — unimplementable in a dynamic language, incompatible with a moving GC.
- **Always share via `Arc<Mutex>`** — correct, but every shared value pays a lock. Deep copy is
  faster than a lock in the overwhelmingly common case.
