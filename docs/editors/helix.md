# Helix Setup

## Installation

### Option 1: Manual Installation

1. Add the language configuration to your Helix config:

```bash
# Copy languages.toml to your Helix config
cp editors/helix/languages.toml ~/.config/helix/
```

2. Clone and build the Treesitter grammar:

```bash
cd ~/tree-sitter-zen
npx tree-sitter generate
```

3. Add the grammar to your runtime path:

```bash
# Add to ~/.config/helix/runtime/queries/zen/
mkdir -p ~/.config/helix/runtime/queries/zen
```

### Option 2: Build from Source

```bash
cd editors/helix
helix-fetch-grammars
helix-build-grammars
```

## Features

- Syntax highlighting via Treesitter
- Auto-closing pairs
- Comment tokens
- Code objects (functions, classes, etc.)

## Configuration

The `languages.toml` file configures:

- File extension: `.z`
- Comment tokens: `//`, `#`
- Block comment tokens: `/*`, `*/`
- Auto-closing pairs for brackets, quotes, and backticks
