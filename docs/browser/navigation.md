# Navigation

This guide covers everything related to moving around the web with the
`browser` module: going to URLs, reloading, waiting for pages, scrolling, and
injecting JavaScript.

## Navigating to a URL

`browser.navigate(url)` and its alias `browser.go(url)` load a page and **wait
until it has fully loaded** — specifically until
`document.readyState === "complete"` (checked every 100 ms, up to ~10 seconds).

```zen
browser.go("https://example.com")
print browser.url()
```

You can pass a URL held in a variable, interpolate one, etc.:

```zen
var host = "example.com"
var path = "/docs/page-2"

if browser.go("https://" + host + path) {
    print "Now at " + browser.url()
}
```

### What "wait for load" means

The return value is `true` as soon as the page reports `readyState = complete`.
Pages that never finish loading (hanging analytics, infinite polling loops)
simply move on after roughly ten seconds. Use `wait_for` for elements that
appear *after* this load event (see below).

## Reloading

There is no dedicated `reload`/`refresh` helper — re-navigate to the current
URL:

```zen
browser.go(browser.url())
```

or use JavaScript:

```zen
browser.eval("location.reload()")
```

## Back and forward

There is no `back`/`forward` helper either. Drive history through JavaScript:

```zen
browser.go("https://example.com/a")
browser.go("https://example.com/b")

browser.eval("history.back()")       # back to /a
browser.eval("history.forward()")    # forward to /b again
```

## Waiting

Two wait helpers exist, both checking that a CSS selector matches an element.

| Function | Behaviour |
|----------|-----------|
| `browser.wait_for(selector, maxMs?)` | Polls **every 100 ms** from Zen. Default timeout 20 000 ms, hard cap 60 s. |
| `browser.wait_for_ms(selector, maxMs)` | Same check via a **JS promise** inside the page (async CDP response). `maxMs` is required and capped at 60 s. |

```zen
browser.go("https://example.com/checkout")

# Wait up to 20 s (default) for the form to render:
if browser.wait_for("form#payment") {
    print "Payment form is here"
}

# Explicit, longer/shorter timeout:
if browser.wait_for("div.toast.success", 5_000) {
    print "Toast appeared within 5s"
}
```

### Typical flow: single-page app

SPAs update the DOM long after `readyState` completes. The reliable pattern is
`go()` followed by `wait_for()` on the element you actually need:

```zen
browser.go("https://example.com/app")

# The SPA loads a shell first; the real content appears later:
if browser.wait_for("div[data-view='dashboard']", 30_000) {
    print browser.text("div[data-view='dashboard']")
}
```

### Timeouts

The defaults live in the module:

- `wait_for` default is **20 s**.
- `wait_for_ms` has **no default** — you must pass a number.

Both treat a timeout as `false` (not an error), so you can branch cleanly:

```zen
var got_it = browser.wait_for(".result", 3_000)
if not got_it {
    print "Still no results after 3s"
}
```

## Scrolling

No scroll helper exists; use `browser.evaluate`:

```zen
# Scroll to the bottom of the page:
browser.evaluate("window.scrollTo(0, document.body.scrollHeight)")

# Scroll to the top:
browser.evaluate("window.scrollTo(0, 0)")

# Scroll an element into view — triggers lazy-loading image blocks:
browser.evaluate("document.querySelector('#comments')?.scrollIntoView()")
```

### Infinite-scroll scrapes

A classic "load more while scrolling" loop:

```zen
browser.go("https://example.com/feed")

for i in 0..5 {
    browser.evaluate("window.scrollTo(0, document.body.scrollHeight)")
    browser.wait_for_ms("div.post:last-child", 3_000)
}

print len(browser.query("div.post"))    # how many posts we managed to load
```

## Executing JavaScript

`browser.evaluate(expr)` / `browser.eval(expr)` runs JavaScript in the page
context and returns the result converted to a Zen value.

```zen
var title    = browser.eval("document.title")                 # string
var width    = browser.eval("innerWidth")                     # int
var isDark   = browser.eval("matchMedia('(prefers-color-scheme: dark)').matches")
var cookies  = browser.eval("document.cookie")                # string
```

Return-value conversion table:

| JavaScript | Zen |
|-----------|-----|
| `number` | `int` |
| `string` | `string` |
| `boolean` | `bool` |
| `null` / `undefined` | `null` |
| array | list |
| object | dict |

### Multi-statement code and `return`

Code that *starts* with a statement keyword (`var`, `let`, `const`, `if`,
`for`, `while`, `function`, `switch`, `try`, `throw`) or with `return` is
automatically wrapped in an IIFE, so **`return` becomes legal**:

```zen
browser.eval("var a = 2, b = 3; return a + b")    # 5
browser.eval("return document.querySelector('h1').textContent")
```

Without a `return`, the wrapped IIFE evaluates to `undefined` → `null`:

```zen
print browser.eval("for (let i=0;i<3;i++) console.log(i)")   # null (logs to console)
```

### Getting computed values back as dicts

```zen
var metrics = browser.eval("({ height: innerHeight, width: innerWidth })")
print metrics.height, "x", metrics.width
```

### Useful one-liners

```zen
browser.eval("document.title")                                # page title
browser.eval("location.href")                                 # current URL
browser.eval("document.querySelector('h1')?.textContent")     # first h1
browser.eval("document.body.innerText.length")                # text size
browser.eval("window.scrollTo(0, 0)")                         # scroll up
browser.eval("document.querySelector('form')?.submit()")      # submit a form
```

## Headers, user-agent, downloads

The current CDP integration does **not** expose helpers for setting custom
user agents, extra request headers, or downloading files. If you need them,
either:

- fetch the resource with the `http` module and write it to disk with `fs`
  (no browser involved):

  ```zen
  var r = http.get("https://example.com/file.zip")
  fs.write("/tmp/file.zip", r.text())
  ```

- or, inside the page, drive the network through JavaScript
  (`fetch` + `readAsArrayBuffer`).

The retired `set_user_agent`, `set_headers`, `download ... to ...`,
`user_agent()`, and `headers()` APIs from the old Python backend are gone.

## Reference

| Function | Aliases | Arguments | Returns |
|----------|---------|-----------|---------|
| `navigate` | `go` | `url` | `bool` |
| `wait_for` | – | `selector, maxMs?` | `bool` |
| `wait_for_ms` | – | `selector, maxMs` | `bool` (JS-driven) |
| `evaluate` | `eval` | `jsCode` | Zen value / `null` |
| `url` | `get_url` | – | `string` |