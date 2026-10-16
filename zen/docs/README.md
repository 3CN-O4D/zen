# Zen Language Documentation

This directory contains the complete engineering specification for the Zen language v3.

## Document Index

| # | Document | Purpose |
|---|---|---|
| 00 | [00-blueprint-review.md](00-blueprint-review.md) | **Start here.** Engineering review of the original blueprint with mandatory amendments. |
| 01 | [01-principles.md](01-principles.md) | Design principles and stability doctrine. |
| 02 | [02-architecture.md](02-architecture.md) | High-level system architecture. |
| 03 | [03-syntax.md](03-syntax.md) | **Canonical language syntax contract** — the single source of truth for what is valid Zen code. |
| 04 | [04-value-and-types.md](04-value-and-types.md) | 16-tag NaN-boxed `Value` specification (corrected from blueprint). |
| 05 | [05-lexer.md](05-lexer.md) | Lexer implementation plan. |
| 06 | [06-parser.md](06-parser.md) | Parser and AST implementation plan. |
| 07 | [07-bytecode.md](07-bytecode.md) | Register bytecode format and compiler. |
| 08 | [08-vm.md](08-vm.md) | Register VM executor (the speed milestone). |
| 09 | [09-memory.md](09-memory.md) | Memory model: Arc/CoW now, tracing GC roadmap. |
| 10 | [10-jit.md](10-jit.md) | Background Cranelift JIT (Milestone 11, optional feature). |
| 11 | [11-ffi-handles.md](11-ffi-handles.md) | FFI, handles, and native module ABI. |
| 12 | [12-concurrency.md](12-concurrency.md) | Threads, channels, mutexes, async. |
| 13 | [13-networking.md](13-networking.md) | 7-layer networking stack. |
| 14 | [14-modules-and-packages.md](14-modules-and-packages.md) | Module system and package management. |
| 15 | [15-toolchain.md](15-toolchain.md) | `zenn` and `zen` CLI toolchain. |
| 16 | [16-stdlib.md](16-stdlib.md) | Standard library module catalog. |
| 17 | [17-testing-strategy.md](17-testing-strategy.md) | Testing strategy and differential harness. |
| 18 | [18-benchmarks.md](18-benchmarks.md) | Benchmark suite and performance gates. |
| 19 | [19-roadmap.md](19-roadmap.md) | Milestone plan with pass/fail criteria. |

## Decision Records (ADRs)

| ADR | Title | Status |
|---|---|---|
| [0001](decisions/ADR-0001-tag-budget.md) | 8 machine tags, 48-bit payload | Accepted |
| [0002](decisions/ADR-0002-memory-model.md) | Arc+CoW now, tracing GC gated | Accepted |
| [0003](decisions/ADR-0003-concurrency-semantics.md) | Explicit sharing, deep copy on send | Accepted |
| [0004](decisions/ADR-0004-honest-types.md) | `handle` is an honest type | Accepted |
| [0005](decisions/ADR-0005-module-paths-and-tiers.md) | Bare module paths, linker-owned tier selection | Accepted |

## How to Use These Docs

1. **Read in order** 00 → 04 for the core contract, then the subsystem plans (05–18) as needed.
2. Each doc has `> **Status:**` and `> **Milestone:**` headers indicating readiness.
3. Cross-references use relative links; run `mdbook serve` or open in a Markdown viewer for navigation.
4. Implementation code lives in `../crates/` and should match these docs. If code and docs diverge, **docs win** — update the code.

## Conformance

The test suite in `../tests/` is the executable contract:
- `tests/cases/*.z` — black-box conformance (M1+)
- `tests/diff/*.z` — VM vs interpreter differential (M3+)
- `benchmarks/` — performance gates (M4+)

Run the full suite with `cargo test --workspace` from the project root.