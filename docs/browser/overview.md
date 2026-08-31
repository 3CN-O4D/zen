# Browser Automation

> **Zen's browser automation is a first-class module, not part of the core
> language.** All browser functions live in the `browser` module and talk to a
> real browser over the **Chrome DevTools Protocol (CDP)**. The language core
> stays 100 % pure — there are no browser statements, "magic" variables like
> `page`, or element objects.

## What this module does

`browser` launches a real Chromium/Chrome instance, connects to it over CDP,
and lets you script the browser as if you were driving it by hand:

- navigate to URLs and wait for the page to finish loading
- read the page (HTML, visible text, title, URL)
- find elements with CSS selectors
- click, type into inputs, read attributes
- take screenshots of the whole page
- run arbitrary JavaScript and get the result back as a Zen value

```zen
browser.launch()                       # headless Chromium on port 9222
browser.go("https://example.com")      # navigate and wait for load
print browser.title()                  # "Example Domain"
print browser.page_text()              # visible page text
browser.quit()                         # shut the browser down
```

## Requirements

- **Chromium or Google Chrome** must be installed and on `PATH`. Zen discovers
  the browser executable automatically the first time it is needed.
- No external Python packages — the old Python ("DrissionPage") backend is
  gone. Everything is implemented in Rust talking CDP directly.
- The module works on Linux, macOS, and Windows (wherever Chromium runs).

## The module is always available

`browser` is pre-registered in every Zen VM, so you can use it without an
`import`:

```zen
print typeof browser     # dict
```

You can also `import browser` explicitly — it is the same module.

```zen
import browser
print browser.url()
```

## Quick Start

A complete, runnable first script:

```zen
# 1. Start a headless browser
browser.launch()

# 2. Navigate — this blocks until the page is fully loaded
browser.go("https://news.ycombinator.com")

# 3. Read information back
print "Page:   " + browser.title()
print "URL:    " + browser.url()
print "Links:  " + len(browser.query("a"))      # number of link texts

# 4. Interact
browser.click("button.morelink")                # click "More" on HN

# 5. Save a screenshot
browser.shot("/tmp/hn.png")

# 6. Tidy up
browser.close()
```

## Launch and connect

| Function | Behaviour |
|----------|-----------|
| `browser.launch()` | Start a **headless** Chromium on port `9222`. Returns `true`. |
| `browser.launch(false)` | Start a **visible** (headful) Chromium on port `9222`. |
| `browser.launch(false, 9333)` | Headful browser on a custom port. |
| `browser.connect()` | Start a **headful** browser and connect (specialised helper). |

Details:

```zen
browser.launch()                  # headless   (default)
browser.launch(false)             # headful — you can watch what it does
browser.launch(true, 9123)        # headless on a non-default port

if browser.launch(false) {
    print "Browser is up — watch the window!"
}
```

> **Why port matters.** The port is only the *local debugging* port CDP listens
> on. It is picked automatically from `9222` unless you say otherwise, and each
> `launch()` uses a fresh temporary user-data directory, so runs are isolated
> and clean up after themselves.

> **Auto-launch.** You don't actually have to call `launch()` first. The first
> navigation function (`go`, `eval`, etc.) starts a headless browser for you.
> `launch()` exists so you can pick headful mode or a custom port.

## Navigating

| Function | Description |
|----------|-------------|
| `browser.navigate(url)` / `browser.go(url)` | Load `url`, then block until `document.readyState` is `complete` (up to ~10 s). Returns `true`. |

```zen
browser.go("https://example.com")

var url = "https://example.com/pricing"
if browser.go(url) {
    print "Loaded " + browser.title()
}
```

## Reading page information

| Function | Description |
|----------|-------------|
| `browser.title()` / `browser.get_title()` | `document.title` as a string. |
| `browser.url()` / `browser.get_url()` | Current URL, `location.href`, as a string. |
| `browser.html()` / `browser.page()` | Full page HTML (`document.documentElement.outerHTML`). |
| `browser.page_text()` | Visible text of the whole page (`document.body.innerText`). |

```zen
browser.go("https://en.wikipedia.org/wiki/Zen")

print browser.title()          # "Zen - Wikipedia"
print browser.url()            # ".../wiki/Zen"
print len(browser.html())      # e.g. 117583
print browser.page_text().slice(0, 120)
```

## Finding elements

All selector functions use standard **CSS selectors** (`document.querySelector`
/ `querySelectorAll`), so anything that works in CSS works here: `#id`, `.class`,
`a[href^="https"]`, `ul li:first-child`, and so on.

| Function | Description |
|----------|-------------|
| `browser.text(selector)` / `browser.get_text(selector)` | Visible text of the *first* match, or `null`. |
| `browser.query(selector)` | Visible text of *all* matches, as a list of strings. |
| `browser.attr(selector, name)` | Value of an attribute on the first match, or `null`. |

```zen
print browser.text("h1")           # heading text
print browser.text("title")        # nothing — use browser.title()
print browser.query("article h2")  # every article header

# Pull links out of a page without any element objects:
var urls = browser.query("a")
for u in urls {
    print u
}

print browser.attr("img.logo", "src")   # logo image path
```

> **Gotcha:** `browser.text()` returns the *first* matching element's text.
> To get a list of texts for every match, use `browser.query()`.

## Interacting with the page

| Function | Description |
|----------|-------------|
| `browser.click(selector)` | Click the first match. Returns `true` if the element existed, else `false`. |
| `browser.fill(selector, value)` | Set an input's value and fire `input`/`change` events. Returns `true`/`false`. |
| `browser.wait_for(selector, maxMs?)` | Poll (every 100 ms) until the element exists. Default timeout **20 s**, hard cap 60 s. Returns `true`/`false`. |
| `browser.wait_for_ms(selector, maxMs)` | Same check but implemented with a real JS promise; `maxMs` is required. Returns `true`/`false`. |

```zen
browser.go("https://example.com/login")

browser.fill("#username", "admin")
browser.fill("#password", "s3cret")
browser.click("button[type=submit]")

# Wait for the dashboard to appear (up to 20 seconds by default):
if browser.wait_for(".dashboard") {
    print "Logged in: " + browser.title()
} else {
    print "Dashboard never appeared 💥"
}
```

## Running JavaScript

| Function | Description |
|----------|-------------|
| `browser.evaluate(code)` / `browser.eval(code)` | Run JS in the page and return the result as a Zen value. |

The code is evaluated with smart wrapping:

- **Plain expressions** are evaluated directly:
  ```zen
  var h = browser.evaluate("document.body.scrollHeight")   # int
  var title = browser.eval("document.title")               # string
  ```

- **Statements** (any code starting with `var`, `let`, `const`, `if`, `for`,
  `while`, `function`, `switch`, `try`, `throw`) — or anything starting with
  `return` — are wrapped in an anonymous function and invoked it immediately:
  ```zen
  browser.eval("var x = 5; var y = 6; return x * y")       # 30
  browser.eval("for (let i=0;i<3;i++) console.log(i)")     # logs 1,2,3
  ```

- **`return` is only legal when wrapped**, so this also works:
  ```zen
  var n = browser.evaluate("return 2 + 2")   # 4
  ```

Return values convert automatically: JS `number` → Zen `int`, `string` →
`string`, `boolean` → `bool`, `null`/`undefined` → `null`, arrays → Zen lists,
objects → Zen dicts.

```zen
var meta = browser.eval("({ title: document.title, height: innerHeight })")
print meta.title
print meta.height
```

> **Gotcha:** statements are matched by *prefix*, so a plain expression such as
> `variant || "default"` is fine, but code beginning with a statement keyword
> is always wrapped. Multi-statement scripts must use `return` to produce a
> value.

## Screenshots

| Function | Description |
|----------|-------------|
| `browser.screenshot(path)` / `browser.shot(path)` | Capture the current viewport as a PNG and write it to `path`. Returns `true`. |

```zen
browser.go("https://example.com")
browser.shot("/tmp/example.png")
```

See [Screenshots](screenshots.md) for more.

## Element-object API vs selector API

Older Zen shipped an "element object" API (`find("h1").click()`,
`page.inputs`, magic `_page_links` variables, etc). All of that has been
removed. The modern, supported API is **selector-based and function-based**,
as documented on this page:

```zen
# Before (retired):  find(".btn").click();  page.title
# After  (current):  browser.click(".btn"); browser.title()
```

The greppable rules to remember:

1. Every call starts with `browser.`.
2. Anywhere you used an element object, use a **selector string** + the
   relevant function instead.
3. Page-wide reads use `browser.title()`, `browser.url()`, `browser.html()`,
   `browser.page_text()`.
4. Everything returns plain Zen values (strings, bools, lists, dicts) — there
   are no element objects, no method chaining, no magic globals.

## Common pitfalls

| Mistake | Why it fails | Correct |
|---------|--------------|---------|
| `go "https://..."` (no `browser.`) | `go` is not a language statement | `browser.go("https://...")` |
| `title()` / `page.title` | There is no global `title`/`page` | `browser.title()` |
| `find("h1").click()` | No element objects exist | `browser.click("h1")` |
| `wait 2000` | No `wait` loop statement | `browser.wait_for(".x", 2000)` |
| `browser.eval("var x = 1; x")` | Statement wrap needs a `return` to produce a value | `browser.eval("var x = 1; return x")` |
| Calling navigation before any browser exists | Auto-launch kicks in (headless) | Call `browser.launch(false)` first to go headful |

## Reference

| Function | Aliases | Arguments | Returns |
|----------|---------|-----------|---------|
| `launch` | – | `headless?=true`, `port?=9222` | `bool` |
| `connect` | – | – | `bool` |
| `navigate` | `go` | `url` | `bool` |
| `evaluate` | `eval` | `jsCode` | value/null |
| `screenshot` | `shot` | `path` | `bool` |
| `html` | `page` | – | `string` |
| `get_title` | `title` | – | `string` |
| `get_url` | `url` | – | `string` |
| `get_text` | `text` | `selector` | `string`/null |
| `page_text` | – | – | `string` |
| `click` | – | `selector` | `bool` |
| `fill` | – | `selector, value` | `bool` |
| `query` | – | `selector` | `list[string]` |
| `attr` | – | `selector, name` | `string`/null |
| `wait_for` | – | `selector, maxMs?=20_000` | `bool` |
| `wait_for_ms` | – | `selector, maxMs` | `bool` |
| `close` | `quit` | – | `bool` |