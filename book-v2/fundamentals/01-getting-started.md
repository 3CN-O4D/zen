# Chapter 1: Getting Started

Welcome to *Zen for Dummies*. This chapter walks you through installing Zen,
writing your first program, running it, and using the REPL. By the end you'll
have a working Zen development environment and a solid grasp of the basic
workflow.

## 1.1 What is Zen?

Zen 3.0 is a fast, native scripting language with a Python-flavoured syntax,
a Cranelift JIT compiler, embedded-Python interoperability (`pymport`),
Rust-grade coded diagnostics (`zen explain`), and a batteries-included
standard library. It compiles to a single static binary.

Key capabilities:

- **JIT compilation** — Cranelift backend with monomorphic inline caches,
  OSR (on-stack replacement), and deoptimization.
- **`pymport`** — Import Python modules (stdlib, venv, project-local `.py`)
  with value marshalling and chained tracebacks.
- **Coded diagnostics** — Error codes `Z01xx`–`Z05xx`, `zen explain`, and
  Python tracebacks chained under Zen frames.
- **Stdlib** — HTTP, files, regex, JSON, threading, sockets, crypto, SQLite,
  and more.
- **REPL** — Interactive REPL with `zen` (no arguments).

## 1.2 Installation

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

Verify the install:

```bash
zen --version
# zen 3.0.0
```

## 1.3 Your first program

Create a file named `hello.z`:

```zen
print("Hello, Zen!")
```

Run it:

```bash
zen run hello.z
# Hello, Zen!
```

Zen scripts use `.z` by convention, but any extension works.

### Variables and functions

Zen uses Python-flavoured syntax. Variables are declared with `let` (immutable
by default), `var` (mutable), or `const` (compile-time constant). Functions
use `fn`.

```zen
# hello2.z
func greet(name) {
    return "Hello, " + name + "!"
}

let recipient = "Zen programmer"
print(greet(recipient))
# Hello, Zen programmer!
```

Run it:

```bash
zen run hello2.z
```

### Comments

Comments start with `#` and run to the end of the line:

```zen
# This is a comment
print("code")  # inline comment
```

## 1.4 The REPL

Start the interactive REPL by running `zen` with no arguments:

```bash
zen
# Zen 3.0 REPL
# >
```

In the REPL, type expressions and press Enter to evaluate them immediately:

```zen
> 1 + 2 * 3
7
> "Zen " + "3.0"
'Zen 3.0'
> let x = 42
> x * 2
84
> func double(n) { return n * 2 }
> double(21)
42
> :quit
```

REPL commands:

| Command | Effect |
|---------|--------|
| `:quit` / `:q` | Exit the REPL |
| `:help` | Show help |
| `:clear` | Clear the screen |

Multi-line statements are supported — the REPL detects when a block is
incomplete (e.g. after `fn`, `if`, `for`) and keeps prompting until the block
is closed.

## 1.5 `zen explain`

Every runtime error in Zen has a coded diagnostic (e.g. `Z0401`, `Z0325`).
Use `zen explain` to see the long-form explanation:

```bash
zen explain Z0401
zen explain Z0325
zen explain Z0501
```

Output includes the error title, a detailed description, and often a
`= help:` suggestion with a code example. This is your primary reference
when something goes wrong.

## 1.6 Type-checking without running

`zen check` compiles a script and reports errors without executing it:

```bash
zen check app.z
# OK: app.z
```

This catches undefined names, structural errors, and unresolved imports before
you run the program.

## 1.7 Linting

`zen lint` warns about suspicious patterns: unused variables, unused
parameters, and dead (unreachable) code:

```bash
zen lint app.z
zen lint --strict app.z   # warnings become errors (Werror)
```

## 1.8 Formatting

`zen fmt` reformats source files to canonical Zen style:

```bash
zen fmt app.z         # format in place
zen fmt --check app.z # check formatting without changing files
```

## 1.9 Building standalone executables

`zen build` bundles a program into a standalone executable that carries its
embedded bytecode payload:

```bash
zen build app.z -o app
./app
```

The resulting binary requires no Zen installation to run.

## 1.10 Running tests

`zen test` runs the project's test suite:

```bash
zen test
```

## 1.11 Project layout

A typical Zen project:

```
myproject/
  main.z          # entry point
  lib.z           # library module
  utils/
    helper.z
  requirements.txt # Python deps for pymport (optional)
  tests/
    test_main.z
```

Run from the project root:

```bash
zen run main.z
```

## 1.12 Next steps

- [Chapter 2: Data Types](./fundamentals/02-data-types.md) — primitives,
  collections, and the type system.
- [Chapter 3: Operators and Expressions](./fundamentals/03-operators.md) —
  arithmetic, comparison, logic, `?.` safe access, `??` coalescing.
- [Chapter 4: Control Flow](./fundamentals/04-control-flow.md) — conditionals,
  loops, `match`, `when`, `try`/`catch`.
