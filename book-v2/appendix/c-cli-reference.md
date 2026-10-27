# Appendix C: Zen CLI Reference

Exhaustive reference for the `zen` command-line interface (Zen 3.0).

## C.1 Commands

| Command | Purpose | Example |
|---------|---------|---------|
| `zen` / `zen repl` | Start interactive REPL | `zen` |
| `zen run <file>` | Execute a Zen source file | `zen run app.z` |
| `zen -m <module>` | Run a module as `__main__` | `zen -m http.server` |
| `zen -e <code>` | Execute inline code | `zen -e 'print(1+1)'` |
| `zen check <file>` | Type-check/compile without running | `zen check lib.z` |
| `zen lint <file>` | Warn on suspicious patterns | `zen lint app.z` |
| `zen explain <code>` | Explain an error code | `zen explain Z0308` |
| `zen build <file>` | Bundle into a standalone executable | `zen build app.z -o app` |
| `zen test` | Run tests in the project | `zen test` |
| `zen fmt` | Format source files | `zen fmt .` |
| `zen help` | Show help | `zen help` |
| `zen --version` | Print version | `zen --version` |

### REPL

Start with no arguments or `zen repl`. Features:
- Enter expressions and statements; results are printed.
- Multi-line blocks are supported (the REPL detects open braces).
- Type `:help`, `:quit`, `:exit` to leave (build-dependent).
- Use `print()` for output; the last expression's value is also shown.

### `zen run`

Executes a `.z` file as a script. `__name__` is set to `"__main__"`.
Exit code is 0 on success, non-zero on error.

```bash
zen run hello.z
zen run ./scripts/build.z
```

### `zen -m <module>`

Runs a module as the main program (calls its `main(argv)` if defined).
Module resolution follows the normal import rules.

```bash
zen -m http.server
zen -m myproject.cli
```

### `zen -e <code>`

Executes inline Zen code. Useful for quick calculations.

```bash
zen -e 'print(2 ** 10)'
zen -e 'import math; print(math.pi)'
```

### `zen check <file>`

Compiles/type-checks without executing. Reports syntax and type errors.

```bash
zen check library.z
```

### `zen lint <file>`

Static analysis: unused variables, dead code, suspicious patterns.

```bash
zen lint app.z
zen lint .
```

### `zen explain <code>`

Prints the meaning, common causes, and fix for an error code.

```bash
zen explain Z0301
zen explain Z0408
zen explain Z0506
```

### `zen build <file>`

Bundles a program and its dependencies into a single standalone executable.

```bash
zen build app.z -o myapp
./myapp
```

### `zen test`

Discovers and runs tests in the current project. Test files typically match
`test_*.z` or `*_test.z`, or functions named `test_*`.

```bash
zen test
zen test tests/
```

### `zen fmt`

Formats Zen source files to a canonical style.

```bash
zen fmt file.z
zen fmt src/
```

## C.2 Exit codes

| Exit code | Meaning |
|-----------|---------|
| 0 | success |
| 1 | general error |
| 2 | usage / command-line error |
| 127 | command not found |
| 130 | interrupted by signal (Ctrl-C) |
| N | program's `exit(N)` value |

## C.3 Environment variables

| Variable | Purpose | Example |
|----------|---------|---------|
| `ZEN_PATH` | Module search paths (colon-separated) | `ZEN_PATH="$HOME/zen/lib"` |
| `ZEN_DEBUG` | Enable debug/verbose logging | `ZEN_DEBUG=1` |
| `PYTHONPATH` | Python module resolution for pymport | `PYTHONPATH=/opt/pylibs` |
| `VIRTUAL_ENV` | Python venv used by pymport | `VIRTUAL_ENV=.venv` |
| `HOME` | user home (used for config/cache) | inherited |
| `PATH` | executable search | inherited |

### ZEN_PATH examples

```bash
# add a local modules directory
export ZEN_PATH="$PWD/lib:$HOME/zen/modules"
zen run app.z

# multiple directories
export ZEN_PATH="/opt/zen/stdlib:/usr/local/zen/lib"
```

### ZEN_DEBUG

When set, the VM prints additional diagnostic info (bytecode, JIT decisions,
GC events, depending on build).

```bash
ZEN_DEBUG=1 zen run app.z
```

## C.4 Configuration files

Zen does not require a config file, but projects may use:
- `zen.toml` / `zen.json` — project settings (build-dependent)
- `.zenrc` — REPL preferences
- `.gitignore` — exclude `target/`, `__pycache__/`, `.zen/`

## C.5 Common workflows

```bash
# 1. Create and run a script
echo 'print("hello")' > hello.z
zen run hello.z

# 2. Type-check before running
zen check hello.z && zen run hello.z

# 3. Quick calculation
zen -e 'print(100 * 1.05)'

# 4. Explain an error
zen explain Z0304

# 5. Format and lint
zen fmt src/
zen lint src/

# 6. Run tests
zen test

# 7. Build a binary
zen build app.z -o app
./app

# 8. Run with debug logging
ZEN_DEBUG=1 zen run app.z

# 9. Use a custom module path
ZEN_PATH="$PWD/lib" zen run app.z
```

## C.6 REPL commands (if supported)

| Command | Purpose |
|---------|---------|
| `:help` | show REPL help |
| `:quit` / `:exit` | leave the REPL |
| `:load <file>` | load a file into the REPL |
| `:vars` | list current variables |
| `:clear` | clear the screen |

## C.7 Exercises

1. Print the Zen version: `zen --version`.
2. Start the REPL, evaluate `2 + 2`, then exit.
3. Write `hello.z` and run it with `zen run`.
4. Type-check a file with `zen check`.
5. Explain an error code with `zen explain Z0304`.
6. Run an inline calculation with `zen -e`.
7. Format a file with `zen fmt`.
8. Lint a file with `zen lint`.
9. Set `ZEN_PATH` and import a module from another directory.
10. Build a script into a standalone executable with `zen build`.

'''
print('appendix c done')