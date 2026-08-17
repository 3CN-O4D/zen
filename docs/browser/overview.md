# Browser Overview

Zen uses DrissionPage for reliable browser control. All browser operations work in both headless and headful modes.

## Quick Start

```
go "https://example.com"
print title()
print page.text
```

## Key Concepts

### Element Finding

```
find("h1")                    // CSS selector
find(text="Click Here")       // by text
find_by_text("Click Here")    // by text
find_by_url("example.com")    // by URL
find_all("a")                 // all matches
```

### Interaction

```
click("button")
fill("#input", "value")
check("#checkbox")
hover(".menu-item")
```

### Navigation

```
go "https://example.com"
back
forward
refresh
```

### Waiting

```
wait 2000                    // 2 seconds
wait_for(".loaded")          // wait for element
wait_for_network()           // wait for network idle
```

## Headless vs Headful

By default, Zen runs the browser headlessly (invisible). Use `--headful` to watch what's happening:

```bash
zen shell --headful
zen run script.z --headful
```

## Auto-Detection

When you pass a bare string to `find()` or `click()`, Zen auto-detects:

- Strings with CSS characters (`.`, `#`, `:`, `>`, etc.) → treated as CSS
- Strings with spaces but no CSS characters → treated as text
- Single words → treated as CSS tag name

```
find("div.item")              // CSS (has '.')
find("Log In")                // text (has space, no CSS chars)
click("button")               // CSS tag name
```
