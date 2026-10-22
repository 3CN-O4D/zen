# Changelog

## 3.0.0

- Initial release of Zen 3.0: fast native scripting language with Python-flavoured syntax
- Cranelift JIT backend
- Embedded-Python interop (`pymport`)
- Rust-grade coded diagnostics (`zen explain`, Z01xx–Z05xx)
- Bytecode VM + tree-walking reference interpreter
- Batteries-included stdlib (HTTP, files, regex, JSON, threading, sockets, crypto, SQLite)
- Interactive REPL (`zen` with no args)
- `open` builtin for file I/O with `.read`, `.write`, `.close` methods
- `with <expr> [as <name>] { body }` syntax