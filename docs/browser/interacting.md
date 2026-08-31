# Interacting with the Page

This guide covers driving the browser: clicking, filling in forms, waiting for
elements, and knowing whether an action actually succeeded.

The interaction API is deliberately small and selector-based:

| Function | Description |
|----------|-------------|
| `browser.click(selector)` | Click the first matching element. Returns `true` unless no element matches. |
| `browser.fill(selector, value)` | Set a form field's value and dispatch `input` + `change`. Returns `true`/`false`. |
| `browser.wait_for(selector, maxMs?)` | Poll until the element exists (default 20 s). Returns `true`/`false`. |
| `browser.wait_for_ms(selector, maxMs)` | Same, JS-promise driven; `maxMs` required. |
| `browser.attr(selector, name)` | Read an attribute (useful for reading field state back). |

## Clicking

`browser.click(selector)` clicks the first element matching the selector:

```zen
browser.click("button[type=submit]")
browser.click("#nav .menu a")
browser.click("input[type=checkbox]")   # toggles it
```

### Clicking when it isn't there yet

`click` does *not* wait — it returns `false` immediately if the element is
absent. For dynamic pages, wait first, then click:

```zen
if browser.wait_for(".accept-cookies", 5_000) {
    browser.click(".accept-cookies")
}
```

### Clicking something that is only "openable" via JS

If a plain `.click()` doesn't trigger the SPA's handler, drive the DOM event
explicitly:

```zen
browser.eval("document.querySelector('.submit').click()")
browser.eval("document.querySelector('.tab[data-tab=login]').click()")
```

## Filling in inputs

`browser.fill(selector, value)` sets `.value` on the first match and fires
`input` and `change` events (so frameworks like Vue/React/Angular see the
change):

```zen
browser.fill("#username", "alice")
browser.fill("#password", "hunter2")
browser.fill("input[name='email']", "alice@example.com")
```

### The full login recipe

```zen
browser.go("https://app.example.com/login")

browser.wait_for("form#login", 10_000)     # wait for the form to render
browser.fill("#username", "alice")
browser.fill("#password", "hunter2")
browser.click("button[type=submit]")

# Decide how we know it worked:
if browser.wait_for(".dashboard", 15_000) {
    print "Logged in OK"
} else {
    print "Login failed — still on: " + browser.url()
}
```

### Textareas, selects, checkboxes

`fill` sets the value of any field that has one:

```zen
browser.fill("textarea[name='bio']", "Hello, world!")
browser.fill("#country", "US")              # <select> value
```

For checkboxes/radios, click the input (or its label) rather than filling:

```zen
browser.click("input[name='agree']")        # tick the box
```

## Reading state back

Use `attr` to verify what the browser actually has (values, disabled state,
checked state):

```zen
browser.fill("#email", "bad-email")
var entered = browser.attr("#email", "value")        # "bad-email"

var disabled = browser.attr("button[type=submit]", "disabled")
print (disabled != null and disabled != "")           # true => still disabled
```

## Waiting for elements

Two tools; both return `bool` and never throw on timeout:

```zen
# Zen-side polling (every 100 ms):
if browser.wait_for(".toast.success", 5_000) { ... }

# JS-promise version (must pass a timeout):
if browser.wait_for_ms(".toast.success", 5_000) { ... }
```

Prefer `wait_for_ms` when you're about to evaluate heavy JS, and `wait_for`
in fast loops; the difference is mostly cosmetic.

### Wait for "still not present" / absence checks

The helpers only check for *presence*. To wait for absence, invert the result
in a loop with `wait_for_ms`:

```zen
# Wait until the spinner goes away (poll with the promise-based helper):
var spinner_gone = false
for i in 0..50 {
    var present = browser.eval("!!document.querySelector('.spinner')")
    if not present {
        spinner_gone = true
        break
    }
    sleep 0.1
}
print spinner_gone
```

## Full worked example

A complete script that logs in, waits for data, clicks through pagination, and
collects results:

```zen
launch_helper = browser.launch(false)      # headful so we can watch

browser.go("https://example.com/login")
browser.wait_for("form#login", 10_000)
browser.fill("#username", "alice")
browser.fill("#password", "hunter2")
browser.click("button[type=submit]")

if not browser.wait_for(".table", 20_000) {
    print "Could not reach the table view"
    browser.quit()
    exit 1
}

var pages = 3
var all = []
for p in 0..pages {
    browser.wait_for_ms(".row:last-child", 5_000)
    var texts = browser.query(".row .col-title")
    for t in texts {
        all.push(t)
    }
    browser.eval("document.querySelector('.next')?.click()")
}

print "Collected " + len(all) + " rows"
browser.shot("/tmp/tables.png")
browser.quit()
```

## Common pitfalls

| Mistake | Result | Fix |
|---------|--------|-----|
| `browser.click(".x")` before `.x` exists | `false`, nothing happens | `wait_for` first |
| `browser.fill` on a `<button>` | `false` (no value to set / no element) | `click` the button |
| Checking `browser.attr(el, "disabled") == true` | `disabled` is the string `""` when present | check `!= null and != ""` |
| Element covered by a modal | click "works" but the modal handler swallows it | dismiss the modal first, or `eval` the handler |
| Multiple matching elements, wrong one clicked | `click` uses the first match — the wrong element may be the first | make the selector unique (id, `:nth-child`, attribute) |
| Forgetting `browser.` | `undefined variable` / parse error | every call is `browser.*` |