# Zen - Native Rust Runtime

A ground-up Rust rewrite of the Zen scripting language, providing a native runtime with **zero Python dependency**.

## Quick Start

Install the native Zen binary:

# Quick auto-installer (recommended)
curl -fsSL https://raw.githubusercontent.com/3CN-O4D/zen/main/install.sh | bash

# Or manually download from the Releases page

Verify the installation:
```
zen --version
```

Run your first script:
```
zen -e 'print "Hello, Zen World!"'
```

## Documentation Structure

- [Tutorial - Learn Zen from Zero](zen/docs/language/tutorial.md) - Wordy, example-packed guide to how Zen works
- [Language Reference](zen/docs/language/reference.md) - Syntax, variables, types, control flow, functions, classes
- [CLI Reference](zen/docs/cli.md) - Commands (`run`, `check`, `lint`, `repl`, `pm`, `-h`/`--help`)
- [Modules](zen/docs/) - File system (`fs`), Data (JSON, CSV, regex, random, math), System (OS, time, datetime, crypto, HTTP, browser)
- [Installation Guide](zen/docs/install.md) - Quick installer, pre-compiled binaries, from source

## Build From Source

```bash
# Clone the repository
git clone https://github.com/3CN-O4D/zen
cd zen/zen

# Build an optimized release executable
cargo build --release

# Run verification
./target/release/zen --help
```
