# Zen

A native scripting language with a Rust runtime. Zero Python dependency.

## Quick Start

```bash
# Clone
git clone https://github.com/3CN-O4D/zen
cd zen

# Build and install
./scripts/install.sh

# Verify
zen --version
zen -e 'print "Hello, Zen World!"'
```

## Manual Build

```bash
cargo build --release
./target/release/zen --help
```

## Cross-Compile

```bash
./scripts/build.sh --target aarch64-unknown-linux-gnu
```

## Editor Setup

Auto-detects installed editors and configures Zen syntax:

```bash
./scripts/setup-editors.sh
```

Supported: Vim, Neovim, VS Code, Helix, Sublime Text, Emacs, Nano, Micro, Kate, Gedit, plus Bash/Zsh completions.

## Documentation

| Section | Description |
|---------|-------------|
| [Tutorial](zen/docs/language/tutorial.md) | Learn Zen from zero |
| [Language Reference](zen/docs/language/reference.md) | Syntax, types, control flow |
| [CLI Reference](zen/docs/cli.md) | Commands and options |
| [Modules](docs/modules/overview.md) | Built-in module API |
| [Installation](zen/docs/install.md) | All install methods |

## Project Structure

```
zen/
  Cargo.toml          Rust crate configuration
  src/                Runtime source (Rust)
  std/                Standard library (.z files)
  editors/            Syntax files for all editors
  scripts/            Build, install, and setup scripts
  docs/               Documentation (MkDocs)
  examples/           Example scripts
```

## License

MIT
