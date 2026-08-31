# Page Information

Everything you can learn about the currently loaded page, without touching the
DOM yourself.

## One-line summary

| Call | Returns |
|------|---------|
| `browser.title()` | Page title (`document.title`) |
| `browser.url()` | Current URL (`location.href`) |
| `browser.html()` | Full page HTML (`outerHTML` of `<html>`) |
| `browser.page_text()` | Visible text of the whole page (`document.body.innerText`) |

## Title

```zen
browser.go("https://en.wikipedia.org/wiki/Zen")
print browser.title()      # "Zen - Wikipedia"
```

`get_title` is an alias:

```zen
print browser.get_title()
```

## Current URL

```zen
print browser.url()        # "https://en.wikipedia.org/wiki/Zen"
```

Useful for detecting redirects after a form submit or an OAuth dance:

```zen
browser.click("button[type=submit]")
browser.wait_for_ms("body", 5_000)          # give the redirect a beat
if browser.url().contains("profile") {
    print "Redirected to the profile"
}
```

## Full page HTML

`browser.html()` returns the outer HTML of the `<html>` element — the whole
document, including the doctype-less markup inside `<html>`:

```zen
var page = browser.html()
print len(page)
```

You can pipe it straight into files:

```zen
fs.write("/tmp/snapshot.html", browser.html())
```

### Grabbing the raw HTML vs the rendered DOM

`browser.html()` reflects whatever the browser has rendered *right now*. For
pages that mutate heavily after load, wait first:

```zen
var html = browser.html()
```

If you need the pre-rendered source, fetch it instead:

```zen
var r = http.get(browser.url())
var source = r.text()
```

> **Gotcha:** `browser.html()` gives you the **current DOM**, which is not
> necessarily the original HTTP response.

## Visible text

`browser.page_text()` returns `document.body.innerText` (or `""` if there is
no body) — everything you can actually see, with line breaks preserved:

```zen
browser.go("https://example.com")
print browser.page_text()
```

`innerText` differs from `textContent` (which includes hidden text and no
line-break normalization). If you need `textContent` semantics, use JS:

```zen
var raw = browser.eval("document.body.textContent")
```

### "{[media]}" markers are gone

The old Python backend inserted `{[media]}` markers into page text for
embedded media. The CDP backend returns plain `innerText` — no markers.

## Page text vs per-element text

- Whole page: `browser.page_text()`
- One element, first match: `browser.text("h1")`
- Many elements: `browser.query("p")`

```zen
var first_paragraph = browser.text("article p")
var all_paragraphs  = browser.query("article p")
```

## Counting and inspecting elements

No dedicated "existence" / "count" helper. Use `query` (returns a list):

```zen
var links = browser.query("a")
print "There are", len(links), "links"

if len(browser.query(".empty-state")) > 0 {
    print "Showing the empty state"
}
```

or an evaluate for an existence boolean:

```zen
var has_login = browser.eval("!!document.querySelector('form.login')")
print has_login
```

## Reading structured data from a page

Combine the readers with `evaluate` to extract any JSON-ish structure:

```zen
browser.go("https://example.com/products")

var grid = browser.eval("({"
    + " count: document.querySelectorAll('.card').length,"
    + " first: document.querySelector('.card h2')?.textContent,"
    + " url: location.href"
    + "})")

print grid.count
print grid.first
print grid.url
```

## What the legacy API used to look like

Older Zen had magic globals (`page`, `_page_html`, `_page_links`,
`page_links()`, etc.). These do **not** exist in the CDP runtime. The mapping:

| Legacy (retired) | Current replacement |
|------------------|---------------------|
| `page.title` | `browser.title()` |
| `page.url` | `browser.url()` |
| `page.html` / `page.source` | `browser.html()` |
| `page.text` | `browser.page_text()` |
| `page.links` | `browser.query("a[href]")` (+ `browser.attr` for the href) |
| `page.images` | `browser.query("img")` (+ `browser.attr("img", "src")`) |
| `page.buttons` | `browser.query("button")` |
| `page.inputs` / `page.forms` | `browser.query("input, select, textarea")`, `browser.query("form")` |
| `page_links()` etc. | same as above |

## Reference

| Function | Aliases | Returns |
|----------|---------|---------|
| `get_title` | `title` | `string` |
| `get_url` | `url` | `string` |
| `html` | `page` | `string` (outer HTML) |
| `page_text` | – | `string` (visible text) |