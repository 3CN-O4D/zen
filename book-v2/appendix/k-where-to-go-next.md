# Appendix K: Where to Go Next

Congratulations — you've now seen the full breadth of Zen 3.0: fundamentals,
core data structures, the standard library, networking, concurrency, FFI,
system/interop, runtime internals, cookbook recipes, and 101 complete reference
implementations. Here's how to keep going.

## K.1 Contributing to Zen

Zen is an open-source language. Contributions are welcome in many forms:

- **Bug reports:** file issues with `zen explain Z####` output and a minimal
  reproducer.
- **Documentation:** improve this book, the stdlib docs, or inline comments.
- **Standard library:** add modules, fix bugs, optimize hot paths.
- **Compiler / VM / JIT:** work on `zen-syntax`, `zen-vm`, `zen-interp`,
  `zen-jit` (see `crates/`).
- **Tests:** add cases to `tests/cases/*.z` and `tests/`.

### Good first issues

Look for labels like `good-first-issue`, `help-wanted`, or `docs` in the
repository. The M4–M11 milestones (register compiler, VM, GC, shapes, inline
caches, JIT) are where the biggest opportunities are.

## K.2 Official resources

| Resource | Purpose |
|----------|---------|
| `docs/` | Canonical specs: syntax, lexer, parser, bytecode, VM, memory, JIT, FFI, concurrency, networking |
| `decisions/` | Architecture decision records (ADR-0001 tag budget, ADR-0002 memory model, ADR-0004 honest types) |
| `tests/cases/*.z` | 32 conformance test programs — the executable syntax contract |
| `std/` | Standard library source (math, json, crypto, socket, …) |
| `crates/` | Compiler, VM, JIT, CLI, interpreter source (Rust) |
| `book-v2/` | This book (source) |
| `book/` | Zen 2.x book (legacy) |

## K.3 Milestones to watch

| Milestone | Focus |
|-----------|-------|
| M4 | Register compiler + VM (07-bytecode, 08-vm) |
| M5 | Shapes + inline caches |
| M9 | Tracing GC option |
| M11 | Cranelift JIT (optional, feature-gated) |

Each milestone is a chance to contribute a meaningful, well-scoped piece.

## K.4 Your next project

Pick something from this book and extend it:

- Take the chat server (ch 61) and add rooms, private messages, and persistence.
- Take the key-value store (ch 62) and add TTL, replication, or an HTTP API.
- Take the static site generator (ch 60) and add themes, tags, and RSS.
- Take the URL shortener (ch 80) and add click tracking and custom aliases.
- Combine several: build a personal dashboard (ch 98) that monitors your own
  services and serves a static status page.

## K.5 Join the community

- Discuss design and ask questions in the repository discussions.
- Share what you build — Zen programs, libraries, and tutorials are all
  welcome.
- Run `zen test` on your contributions and keep the conformance suite green.

## K.6 Final thoughts

Zen 3.0 is a small, fast, Python-flavored language with a Cranelift JIT,
`pymport` Python interop, coded diagnostics (`Z01xx`–`Z05xx`, `zen explain`),
and a batteries-included standard library. It compiles to a single static
binary and is designed to be readable, debuggable, and extensible.

The best way to learn Zen is to write Zen. Open `hello.z`, write `zen run
hello.z`, and keep going.

```
$ zen
Zen 3.0 REPL. Type :help for help.
> print("Hello, Zen!")
Hello, Zen!
>
```

Happy coding!
