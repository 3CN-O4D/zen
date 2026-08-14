# Zen for Rust

This directory contains the independent, native Zen runtime. It does not embed
or invoke Python.

## Current status

The first milestone implements a standalone command-line interpreter and the
language core: comments, variables, numbers, strings, booleans, lists,
dictionaries, inclusive ranges, arithmetic, comparisons, logical operators,
`if`, `while`, `for`, functions, recursion, `return`, `print`, `break`, and
`continue`. It also supports native class declarations, `new Class()`, and
instance method calls. The expression core includes `typeof`, `??`, strict
equality, and list/dictionary/string membership.

Classes support constructors, mutable fields, and single inheritance. Class
methods remain the only allowed declarations in a class body for now.

Browser commands and Python-backed modules are intentionally not present yet.
They will be added through native Rust libraries and a Chrome DevTools Protocol
client, not by calling the existing Python runtime.

## Build and run

```bash
cargo run -- run examples/basics.z
cargo run -- -e 'let total = 2 + 3 * 4\nprint total'
cargo test
```

To create an optimized executable:

```bash
cargo build --release
./target/release/zen run examples/basics.z
```

## Compatibility policy

The Python implementation remains the reference while this runtime reaches
feature parity. The native runtime must reject unsupported syntax explicitly;
it must never silently run a Python fallback.

See [ROADMAP.md](ROADMAP.md) for the feature-by-feature implementation plan.
