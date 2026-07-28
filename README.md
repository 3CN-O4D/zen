# Zen

A lightweight browser automation language with clean, expressive syntax.
Built on DrissionPage for fast Chromium-based automation.

```
zen shell              Interactive shell with REPL
zen script.z           Run a Zen script (auto-detect)
zen open <url>         Open URL, show title
zen shot <url>         Screenshot a page
zen scrape <url> -s <css>  Scrape text from elements
zen script.z --http    HTTP-only mode (no browser)
zen script.z --connect Attach to your running browser
```

## Quick Example

```zen
go "https://example.com"
fill("#search", "zen language")
click(".search-btn")
wait_for(".results")
print page.title
find_all(".result").each(function(el) {
    print el.text
})
```

## Install

```bash
# Standalone — just download zen.pyz (6.8MB)
python3 zen.pyz script.z

# Or from source:
git clone https://github.com/ecnord/zen.git
cd zen
pip install -e .
```

## Usage Modes

| Flag | What it does |
|------|-------------|
| *(none)* | Fresh headless Chromium |
| `--headful` | Show browser window |
| `--browser-path /usr/bin/brave` | Use specific browser |
| `--connect` | Attach to your running browser (port 9222) |
| `--connect=9999` | Attach on custom port |
| `--http` | HTTP-only (SessionPage), no browser |

## Documentation

📖 **Full Documentation**: [https://ecnord.github.io/zen](https://ecnord.github.io/zen)

### Quick Links

- [Getting Started](https://ecnord.github.io/zen/getting-started/installation/)
- [Language Reference](https://ecnord.github.io/zen/language/overview/)
- [Browser Automation](https://ecnord.github.io/zen/browser/overview/)
- [Modules](https://ecnord.github.io/zen/modules/overview/)
- [CLI Reference](https://ecnord.github.io/zen/cli/)
- [Examples](https://ecnord.github.io/zen/examples/overview/)

## Editor Support

Zen includes syntax highlighting for major editors:

| Editor | Installation |
|--------|--------------|
| **VSCode** | Copy `editors/vscode` to `~/.vscode/extensions/` |
| **Vim/Neovim** | Copy `editors/vim/syntax/zen.vim` to `~/.vim/syntax/` |
| **Helix** | Copy `editors/helix/languages.toml` to `~/.config/helix/` |
| **Sublime Text** | Copy `editors/sublime-text/zen.sublime-syntax` to Packages |

See [Editor Setup](https://ecnord.github.io/zen/editors/vscode/) for detailed instructions.

## Language Features

- **Clean Syntax**: Python-like readability with JavaScript-inspired features
- **Browser Automation**: CSS selectors, text finding, screenshots, JavaScript execution
- **Rich Standard Library**: File system, HTTP, crypto, threading, WhatsApp
- **Modern Features**: Arrow functions, template literals, list comprehensions, destructuring
- **Performance**: Automatic bytecode compilation for 100-250× speedup on hot paths

## Author

ecnord
