# Zen

**A lightweight browser automation language with a clean, expressive syntax.**

---

Zen is a domain-specific language for browser automation that combines:

- A **clean, Python-like syntax** that's easy to read and write
- **DrissionPage** under the hood for reliable browser control
- An **interactive shell** with history, autocomplete, and live feedback
- **Script execution** for repeatable automation
- **Zero external dependencies** beyond DrissionPage itself

## Quick Example

```
go "https://example.com"
print title()
print page.text
```

That's it. Three lines to navigate to a page and extract its content.

## Features

### Language

- **Variables & Scope**: `let`, `const`, closures, destructuring
- **Functions**: Named, anonymous, lambdas, arrow functions
- **Classes**: OOP with inheritance, `self`, `__init__`
- **Control Flow**: `if`/`elif`/`else`, `for`, `while`, `switch`, `try`/`catch`
- **Collections**: Lists, dicts, comprehensions, spread operator
- **Operators**: Arithmetic, comparison, logical, bitwise, nullish coalescing
- **String Interpolation**: Template literals with `${expression}`

### Browser Automation

- **Element Finding**: CSS selectors, text, regex, URL matching
- **Interaction**: Click, fill, check, hover, select
- **Navigation**: Go back/forward, wait for elements, scroll
- **Screenshots**: Full page or element captures
- **JavaScript Execution**: Run JS in the page context

### Modules

- **fs**: File system operations
- **http**: HTTP requests
- **re**: Regular expressions
- **json**: JSON parsing and serialization
- **crypto**: Hashing and encryption
- **threading**: Concurrent execution
- **whatsapp**: Full WhatsApp client via Baileys

## Installation

```bash
git clone https://github.com/3CN-O4D/zen.git
cd zen
pip install -e .
```

## Quick Start

### Interactive Shell

```bash
zen shell
```

### Run a Script

```bash
zen run script.z
```

### Take a Screenshot

```bash
zen shot https://example.com
```

## Documentation

<div class="grid cards" markdown>

- [:material-language-python: **Language Reference**](language/overview.md)
- [:material-web: **Browser Automation**](browser/overview.md)
- [:material-package-variant: **Modules**](modules/overview.md)
- [:material-test-tube: **Standard Library**](stdlib/overview.md)
- [:material-console: **CLI Reference**](cli.md)
- [:material-pencil: **Editor Setup**](editors/vscode.md)

</div>

## Example

```zen
// Scrape quotes from a website
go "https://quotes.toscrape.com/"
let data = []

for quote in find_all(".quote") {
    data.append({
        "text": quote.find(".text").text,
        "author": quote.find(".author").text
    })
}

write_file("quotes.json", json_encode(data))
print "Saved " + data.len + " quotes"
```

## License

MIT
