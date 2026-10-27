# Appendix C: Zen CLI Reference

## Synopsis

```bash
zen [command] [args]
zen            # Start REPL
```

## Commands

| Command | Description |
|---------|-------------|
| `zen [script.z]` | Run a Zen script |
| `zen [--safe] <script.z>` | Run in safe mode (no FFI/socket/process/net/requests/http) |
| `zen -m <module>` | Run a module as `__main__` |
| `zen -e <code>` | Execute inline code |
| `zen check <file>` | Type-check / compile without running |
| `zen lint [--strict] <file>` | Warn on unused/dead code |
| `zen explain <code>` | Explain an error code |
| `zen explain --list` | List all error codes |
| `zen repl` | Start interactive REPL |
| `zen build <file> -o <out>` | Bundle into standalone executable |
| `zen test` | Run tests |
| `zen fmt [--check] <files>` | Format source |
| `zen py-sync` | Sync Python dependencies (`requirements.txt`) |
| `zen --version` / `-v` | Show version |
| `zen help` / `-h` | Show help |

## Global options / environment variables

| Option / Env var | Meaning |
|------------------|---------|
| `--safe` | Disable FFI, sockets, process, HTTP, requests, net modules |
| `ZEN_NO_PY_BOOTSTRAP=1` | Skip Python bootstrap for `pymport` |
| `ZEN_NATIVE_ONLY=<mod>` | Force native tier for a module |
| `ZEN_PURE=true` | Prefer pure-Zen tier in modules |
| `ZEN_MODULE_PATH=<dirs>` | Extra module search directories |
| `ZEN_FIPS=1` | Enable strict crypto policy |
| `ZEN_NO_JIT=1` | Force interpreter-only tier |
| `ZEN_NO_IC=1` | Disable inline caches |

## Examples

```bash
# Run a script
zen run app.z

# Run in safe mode
zen run --safe untrusted.z

# Run inline code
zen -e "print(1 + 2)"

# Run a module as main
zen -m mymodule arg1 arg2

# Type-check
zen check app.z

# Lint (warnings)
zen lint app.z
# Lint (warnings are errors)
zen lint --strict app.z

# Explain an error
zen explain Z0401
zen explain --list

# Format
zen fmt app.z
zen fmt --check app.z

# Build standalone
zen build app.z -o myapp

# Sync Python deps
zen py-sync

# Version
zen --version
```

## `py-sync` details

`zen py-sync` scans `.z` files for `pymport` statements, creates `.zen_venv/`, installs `requirements.txt`, and prepends the venv to `PYTHONPATH` for subsequent `zen run` calls.

```bash
cat > requirements.txt << 'EOF'
beautifulsoup4
requests
EOF
zen py-sync
zen run myscript.z
```

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | General error |
| `2` | Usage / command error |
| `N` | Script exit code (from `exit(N)`) |

## REPL commands

| Command | Effect |
|---------|--------|
| `:quit` / `:q` | Exit the REPL |
| `:help` | Show help |
| `:clear` | Clear the screen |

## Exercises

1. Write a script, run it with `zen run`, then type-check it with `zen check`.
2. Use `zen explain` on three different error codes.
3. Build a standalone executable with `zen build` and run it directly.
4. Use `zen lint --strict` on a file with unused variables and fix them.
5. Create a `requirements.txt` and run `zen py-sync` to install Python deps.
