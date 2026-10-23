# Zen — a fast, native scripting language

[![release](https://img.shields.io/github/v/release/3CN-O4D/zen)](https://github.com/3CN-O4D/zen/releases)
[![build](https://img.shields.io/github/actions/workflow/status/3CN-O4D/zen/release.yml)](https://github.com/3CN-O4D/zen/actions)
[![license](https://img.shields.io/github/license/3CN-O4D/zen)](https://github.com/3CN-O4D/zen/blob/main/LICENSE)

<img src="assets/logo-terminal.png" width="200" alt="Zen logo" align="right"/>

Zen 3.0 is a fast, native scripting language with a Python-flavoured syntax,
Cranelift JIT, embedded-Python interop (`pymport`), and Rust-grade coded
diagnostics (`zen explain`). It ships as a single static binary with a
batteries-included stdlib.

```bash
# Build from source
cargo build --release --package zen-cli --bin zen --all-features
./target/release/zen run app.z
./target/release/zen explain Z0416        # every error code documented
```

## Highlights

| Capability | Details |
|------------|---------|
| JIT | Cranelift backend, monomorphic inline caches, OSR/deopt, call-site caches |
| `pymport` | `import` Python modules (stdlib, venv, project-local `.py`) with value marshalling and chained tracebacks |
| Errors | Coded diagnostics (`Z01xx–Z05xx`), `zen explain`, Python tracebacks chained under Zen frames |
| Diagnostics | Rust-style error codes, help messages, source spans |
| Stdlib | HTTP, files, regex, JSON, threading, sockets, crypto, SQLite, and more |
| REPL | Interactive REPL with `zen` (no args) |
| Concurrency | Rayon, crossbeam channels, threads, atomic operations |

## Quick start

```bash
# Run a script
zen run hello.z

# Start REPL
zen

# Explain an error code
zen explain Z0401

# Type-check a file
zen check app.z
```

## Installation

### From release binaries

Download the latest release from the [Releases page](https://github.com/3CN-O4D/zen/releases).

**Linux (Debian/Ubuntu):**
```bash
sudo dpkg -i zen_3.0.0_amd64.deb
```

**Linux/macOS (tarball):**
```bash
tar xzf zen-linux-x86_64.tar.gz
cd zen-linux-x86_64
sudo ./install.sh
```

**Windows:**
Extract the zip and add `zen.exe` to your PATH.

### From source

```bash
git clone https://github.com/3CN-O4D/zen.git
cd zen
cargo build --release --package zen-cli --bin zen --all-features
# Binary at: target/release/zen
```

## Language overview

```zen
# Variables and functions
let x = 42
func greet(name) {
    print("Hello, " + name)
}
greet("world")

# Containers
let nums = [1, 2, 3, 4, 5]
let total = sum(nums)
print(total)  # 15

# File I/O
let f = open("data.txt", "r")
print(f.read(100))
f.close()

# With statement (auto-scoping)
with open("data.txt", "r") as f {
    print(f.read(100))
}

# Python interop
pymport json
let data = json.loads('{"a": 1}')
print(data.a)  # 1

# Error handling
try {
    let x = 1 / 0
} catch (e) {
    print("Error: " + e)
}
```

## Project structure

```
crates/
  zen-syntax/     # Lexer, parser, AST
  zen-vm/         # Bytecode VM, value system, native dispatch
  zen-interp/     # Tree-walking reference interpreter
  zen-compiler/   # AST → bytecode compiler
  zen-jit/        # Cranelift JIT backend
  zen-cli/        # CLI, REPL, diagnostics
docs/             # Architecture and design docs
book-v2/          # "Zen for Dummies" book + runnable test suite
assets/           # Brand assets (logo, icons)
std/              # Standard library (.z files)
tests/            # Integration test cases
benchmarks/       # Performance benchmarks
```

## Documentation

Engine docs live in [`docs/`](docs/) (blueprint, principles,
architecture, syntax, value system, lexer, parser, bytecode, VM,
memory, JIT, FFI handles, concurrency, networking, roadmap, pymport).

The book *Zen for Dummies* is in [`book-v2/`](book-v2/), and every chapter's
code is runnable. Start with the self-contained test suite:

```bash
cd book-v2/tests
./setup.sh
./run-all.sh                 # hermetic pymport tests
./run-all.sh --network       # + live web-automation examples
```

See [`book-v2/tests/README.md`](book-v2/tests/README.md) for requirements and
the full test matrix, and [`book-v2/interop/42-pymport-python-interoperability.md`](book-v2/interop/42-pymport-python-interoperability.md)
for the pymport chapter.

## License

MIT