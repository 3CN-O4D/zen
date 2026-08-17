# Sublime Text Setup

## Installation

### Option 1: Manual Installation

1. Open Sublime Text
2. Go to `Preferences > Browse Packages...`
3. Create a `Zen` folder
4. Copy `editors/sublime-text/zen.sublime-syntax` into that folder
5. Restart Sublime Text

### Option 2: Package Control

1. Open Command Palette (`Ctrl+Shift+P`)
2. "Package Control: Install Package"
3. Search for "Zen Language"
4. Install

## Features

- Syntax highlighting for all Zen constructs
- Comment toggling (`//`, `#`, `/* */`)
- String interpolation
- Template literals with `${expression}`
- Auto-closing pairs
- Bracket matching
- Code folding

## File Association

Sublime Text will automatically recognize `.z` files as Zen language.

To manually associate files:

1. Open a `.z` file
2. Click the syntax name in the bottom-right corner
3. Select "Zen" from the list
