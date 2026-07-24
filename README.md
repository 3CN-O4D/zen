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
pip install -r requirements.txt && pip install -e .
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

Full language reference: [`ZEN_101.md`](ZEN_101.md)

## Author

ecnord
