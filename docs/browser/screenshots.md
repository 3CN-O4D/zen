# Screenshots

The `browser` module can capture the current browser viewport as a PNG file.

## One-line summary

| Function | Aliases | Arguments | Returns |
|----------|---------|-----------|---------|
| `browser.screenshot(path)` | `browser.shot(path)` | destination file path | `bool` |

## Basic capture

```zen
browser.go("https://example.com")
browser.shot("/tmp/example.png")
```

The file is written immediately. `true` is returned on success; an error is
raised if the browser session can't capture (e.g. no page loaded).

```zen
if browser.screenshot("/tmp/mypage.png") {
    print "Saved screenshot"
} else {
    print "Capture failed"
}
```

## Headful vs headless

Screenshots work in both modes — CDP grabs the *viewport buffer*, not the
window:

```zen
browser.launch(false)       # visible window (probably want this to debug)
browser.go("https://example.com")
browser.shot("/tmp/headful.png")
browser.quit()

browser.launch(true)        # invisible but identical pixel output
browser.go("https://example.com")
browser.shot("/tmp/headless.png")
browser.quit()
```

> **Tip:** headless output is byte-identical to headful for the same page —
> headless is nicer for CI and bulk archives.

## Waiting for content before shooting

A screenshot captures whatever is painted. For dynamic pages, wait for the
content first:

```zen
browser.go("https://example.com/dashboard")
browser.wait_for("div.chart", 15_000)
browser.wait_for_ms("canvas.chart", 5_000)     # let charts finish drawing
browser.shot("/tmp/dashboard.png")
```

## Full-page captures

The module captures the **viewport only** by default. There is no built-in
full-page helper. Two workarounds:

### 1. Resize the window then capture

Scroll-based stitching is awkward; the simplest reliable approach for tall
pages is to expand the viewport and capture in one shot via `evaluate`:

```zen
# Resize the browser to the full content height first
browser.eval("document.documentElement.style.height = 'auto'")
browser.eval("window.resizeTo?.(screen.width," + 
             " document.body.scrollHeight + 200)")
browser.shot("/tmp/fullpage.png")
```

### 2. Capture a section by scrolling

For "long-page" archives, capture a horizontal band, scroll, repeat:

```zen
for p in 0..4 {
    browser.shot("/tmp/feed_" + str(p) + ".png")
    browser.evaluate("window.scrollTo(0, (p + 1) * innerHeight)")
    sleep 0.5
}
```

(You can also drive full-page capture entirely from JS with CDP, but the
eight-argument `captureScreenshot` API is not wrapped by the module — see
`browser.evaluate` if you need to roll your own.)

## Capturing a specific element

No element screenshot helper exists. If you need a tight crop of one element,
script it with CSS + `evaluate` (capture the element's bounding box region) or
simply take a normal screenshot. An easy text-based alternative is to export
the element's outer HTML:

```zen
var card = browser.eval("document.querySelector('.card')?.outerHTML")
fs.write("/tmp/card.html", card ?? "")
```

## Batch captures (e.g. crawling)

```zen
var fs_root = "/tmp/captures/"
var pages = ["https://example.com/a", "https://example.com/b", "https://example.com/c"]

for i in 0..len(pages) {
    browser.go(pages[i])
    browser.wait_for_ms("body", 5_000)
    browser.shot(fs_root + "page_" + str(i) + ".png")
}

browser.quit()
```

## Common pitfalls

| Mistake | Result | Fix |
|---------|--------|-----|
| `shot "page.png"` without `browser.` | parse error / undefined function | `browser.shot("page.png")` |
| Screenshot before navigating | capture of a blank page | `browser.go(...)` first |
| Expecting full-page by default | only the visible viewport is saved | scroll + retake, or resize via `evaluate` |
| Putting the screenshot path in a nonexistent dir | runtime error writing the file | ensure the directory exists (`fs.mkdirs(...)`) |
| Leaving browsers running | stray Chromium processes | always `browser.quit()` at the end |