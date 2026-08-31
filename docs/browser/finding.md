# Finding Elements

The `browser` module finds elements with **CSS selectors**. All of the DOM
lookup is delegated to the browser itself via `document.querySelector` /
`querySelectorAll`, so you get real browser selector semantics for free.

## The three lookup functions

| Function | Selector engine | Returns |
|----------|-----------------|---------|
| `browser.text(selector)` / `browser.get_text(selector)` | `querySelector` (first match) | text of first match, or `null` |
| `browser.query(selector)` | `querySelectorAll` (all matches) | list of texts |
| `browser.attr(selector, name)` | `querySelector` (first match) | attribute value, or `null` |

## CSS selectors work like in the browser

Any selector valid in `document.querySelector` is valid here:

```zen
browser.text(".title")                    # class
browser.text("#main > h1")                # direct child
browser.text("a[href^='https']")          # attribute prefix
browser.text("input:checked")             # pseudo-class
browser.text("ul li:first-child")         # structural
browser.text("tr:nth-child(2) td:last-child")
```

## Reading text: first match vs all matches

`browser.text()` returns the **first** match's text, or `null` when nothing
matches. `browser.query()` returns **all** matches' texts as a list.

```zen
# --browser.text(): first match -----------------------------------------
var h1 = browser.text("h1")
if h1 != null {
    print "Heading: " + h1
}

# --browser.query(): every match ----------------------------------------
var rows = browser.query("table tbody tr td:first-child")
print rows
print len(rows)

# Empty selector / no matches:
print browser.query(".does-not-exist")    # []
```

> **Gotcha:** `browser.text()` on an element that exists but is empty returns
> an empty string `""`, not `null`. `null` means *no matching element*.

## Reading attributes

`browser.attr(selector, name)` gets one attribute from the first match:

```zen
print browser.attr("img.logo", "src")
print browser.attr("a[href]", "href")
print browser.attr("input#user", "value")     # pre-filled value
print browser.attr("form", "action")          # null if the form has no action
```

Use it to extract everything you need from a repeating element:

```zen
browser.go("https://example.com/search?q=zen")

# Every result row: title + link
var links = browser.query("article h3 a")
for offset in 0..len(links) {
    var href = browser.attr("article h3 a:nth-of-type(" + str(offset+1) + ")",
                            "href")
    print str(offset+1) + ". " + links[offset] + " -> " + href
}
```

## The "extract a list of things" pattern

There are no element objects, so to collect several attributes of the same
item you re-query by index (as above) or use `browser.evaluate`, which can
build richer structures in one round-trip:

```zen
var products = browser.eval(
    "Array.from(document.querySelectorAll('li.product')).map(li => ({"
    + " name: li.querySelector('h3').textContent,"
    + " price: li.querySelector('.price').textContent,"
    + " url: li.querySelector('a').href }))"
)

for p in products {
    print p.name, "—", p.price
    print "  ", p.url
}
```

`browser.evaluate` returns a JS object as a Zen dict, so iterating works
exactly like any Zen list of dicts.

## Text vs HTML

- `browser.text(sel)` / `browser.query(sel)` → **visible text** (`innerText`).
- `browser.html()` → the **whole page** HTML (`outerHTML` of `<html>`).

There is no per-element inner-HTML getter. If you need inner HTML of one
element, use JavaScript:

```zen
var card = browser.eval("document.querySelector('.card')?.outerHTML")
```

## Waiting for elements before reading

Pages change after load. Combine finding with `wait_for`:

```zen
browser.go("https://example.com/app")

browser.wait_for("div[data-loaded='true']", 10_000)
var count = browser.query("div[data-loaded='true'] .item")
print len(count)
```

## Common mistakes

| Mistake | Result | Fix |
|---------|--------|-----|
| `browser.text("title")` to get the page title | `null` (no `<title>` element matched by text lookup) | `browser.title()` |
| Passing raw text instead of a selector | `null` — text isn't a selector | use `browser.eval("document.querySelector('...')")` or pick a real selector |
| `browser.query("a").attr(...)` | lists have no `attr` method | use `browser.attr("a", "href")` or the `evaluate` pattern above |
| Calling `browser.text("h1")` before navigating | `null` | navigate first |
| Forgetting `browser.` in front | `undefined variable` / parse error | every call is `browser.something(...)` |