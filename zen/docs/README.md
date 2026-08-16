# Zen Documentation Index

Welcome to the native Zen language documentation. This project is a complete, ground-up Rust rewrite of the Zen scripting language, providing a native runtime with zero Python dependency.

## Quick Start

1. Install the native Zen binary:
   ```bash
   # Quick auto-installer (recommended)
   curl -fsSL https://raw.githubusercontent.com/3CN-O4D/zen/main/install.sh | bash
   
   # Or manually download from the Releases page
   ```

2. Verify the installation:
   ```bash
   zen --version
   ```

3. Run your first script:
   ```bash
   zen -e 'print "Hello, Zen World!"'
   ```

## Documentation Structure

All documentation lives under the `docs/` directory. The root `README.md` serves as the main entry point.

### Language Reference

- `docs/language/tutorial.md` — A friendly, example-packed, from-zero introduction to Zen: variables, values, operators, control flow, functions, containers, classes, modules, and a complete runnable sample program. Start here if you are new to Zen.
- `docs/language/reference.md` — The complete, compact syntax reference covering variables, operators, control flow, functions, classes, modules, destructuring, and built-in globals. Use it as a checklist after the tutorial.

### Command-Line Interface

- `docs/cli.md` — Usage documentation for `zen run`, `zen check`, `zen lint`, `zen repl`, and the `zen pm` package manager.

### Installation Guide

- `docs/install.md` — How to install the native Zen binary on Desktop Linux, Termux (Android), or by compiling from source.

### Categorized Module Documentation

Zen ships with an extensive standard library of native modules. Each is documented in its own page under `docs/modules/`:

| Module | Documentation |
|--------|---------------|
| `fs` | `docs/modules-fs.md` — File and directory operations |
| `json` | Covered in `docs/modules-data.md` |
| `csv` | Covered in `docs/modules-data.md` |
| `re` (regex) | Covered in `docs/modules-data.md` |
| `random` | Covered in `docs/modules-data.md` |
| `base64`, `base32` | Covered in `docs/modules-data.md` |
| `statistics` | Covered in `docs/modules-data.md` |
| `decimal` | Covered in `docs/modules-data.md` |
| `color` | Covered in `docs/modules-data.md` |
| `uuid` | Covered in `docs/modules-data.md` |
| `threading` | Covered in `docs/modules-data.md` |
| `os` | `docs/modules-system.md` |
| `time` | `docs/modules-system.md` |
| `datetime` | `docs/modules-system.md` |
| `math` | `docs/modules-system.md` |
| `crypto` | `docs/modules-crypto.md` |
| `cryptography` (fernet) | `docs/modules-crypto.md` |
| `http` | `docs/modules-http.md` |
| `net` / `socket` | `docs/modules-http.md` |
| `browser` | `docs/browser.md` |

## Philosophy

- The Rust runtime never starts Python as a fallback.
- Unsupported constructs fail with a location-aware error; no silent Python fallback.
- The language semantics are tested independently of browser support.
- Cross-compilation produces binaries for Desktop Linux (x86_64, aarch64, armv7, i686) and Termux Android architectures.

## Releases

Pre-compiled binaries and source artifacts are available on the [Releases Page](./release.md). Each release includes optimized builds for multiple architectures.