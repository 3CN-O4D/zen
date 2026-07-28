# VSCode Setup

## Installation

### Option 1: Manual Installation

1. Copy the `editors/vscode` folder to your VSCode extensions directory:
   - Linux: `~/.vscode/extensions/zen-language-0.1.0`
   - macOS: `~/.vscode/extensions/zen-language-0.1.0`
   - Windows: `%USERPROFILE%\.vscode\extensions\zen-language-0.1.0`

2. Restart VSCode

### Option 2: Package for Distribution

```bash
cd editors/vscode
vsce package
```

This creates a `.vsix` file you can install:

```bash
code --install-extension zen-language-0.1.0.vsix
```

## Features

- Syntax highlighting for `.z` files
- Bracket matching
- Auto-closing pairs
- Comment toggling
- Code folding
- Indentation rules

## File Association

VSCode will automatically recognize `.z` files as Zen language.

To manually associate files:

1. Open Command Palette (`Ctrl+Shift+P`)
2. "Preferences: Configure File Association for Extension"
3. Enter `.z`
4. Select "Zen"
