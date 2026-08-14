# Browser Automation Module (`browser`)

The `browser` module provides automation of Chromium via the Chrome DevTools Protocol (CDP). The bundled `std/browser.z` prelude is automatically loaded when the interpreter starts, so scripts using `go`, `click`, `fill`, etc. work unchanged.

## Loading the Prelude

The browser prelude is loaded automatically. No explicit `import` is needed.

## Functional Helpers

| Function | Description |
|----------|-------------|
| `go(url)` | Navigate to a URL |
| `back()` | Go back in browser history |
| `forward()` | Go forward in browser history |
| `refresh()` | Reload the current page |
| `uri()` | Return the current URL |
| `title()` | Return the page `<title>` |
| `execute(code)` | Execute arbitrary JavaScript in the page context |
| `js(code)` | Alias for `execute` |
| `page_html()` | Return the page's `<html>` outerHTML |
| `page_text()` | Return the page's `body.innerText` |
| `shot(path, full?)` | Take a screenshot; `full=true` captures the full scrollable page |
| `text(sel)` | Get the text content of the first element matching `sel` |
| `texts(sel)` | Return a List of text from all elements matching `sel` |
| `attr(sel, name)` | Get the `name` attribute of the element matching `sel` |
| `fill(sel, val)` | Fill an input/select element with `val` |
| `click(sel)` | Click the element matching `sel`; returns `self` for chaining |
| `select(sel, val)` | Set a select element's value |
| `check(sel)` | Check a checkbox/radiobutton |
| `uncheck(sel)` | Uncheck a checkbox/radiobutton |
| `hover(sel)` | Hover over the element matching `sel` |
| `wait(ms)` | Pause execution for `ms` milliseconds |
| `wait_for(sel)` | Pause until the element matching `sel` appears |
| `close_browser()` | Close the browser session |

## Page and Element Objects

Zen's browser module also provides object models for page interactions, enabling chainable method calls.

### Page Object

| Method | Description |
|--------|-------------|
| `page.go(url)` | Navigate from the current page |
| `page.find(sel)` | Return a new `Element` matching the CSS selector `sel` |
| `page.find_first(sel)` | Same as `find` |
| `page.click(sel)` | Click the element matching `sel`; returns `self` for chaining |
| `page.fill(sel, val)` | Fill the element matching `sel` with `val`; returns `self` |

### Element Object

Methods on an `Element` return `self` (the element) to allow chaining:

| Method | Description |
|--------|-------------|
| `element.click()` | Click the element; returns self |
| `element.fill(val)` | Fill an input/select with `val`; returns self |
| `element.text()` | Return the text content of the element |
| `element.attr(name)` | Return the value of the given `name` attribute |
| `element.html()` | Return the element's `outerHTML` |
| `element.value()` | Return the element's `value` property (for inputs) |
| `element.hover()` | Hover over the element; returns self |
| `element.exists()` | Return `true` if the element is present in the DOM |
| `element.is_visible()` | Return `true` if the element is visible (width > 0 and height > 0) |
| `element.wait(ms)` | Wait `ms` milliseconds; returns self |
| `element.scroll_to(y)` | Scroll to y coordinate; returns self |
| `element.scroll_bottom()` | Scroll to bottom; returns self |
| `element.scroll_top()` | Scroll to top; returns self |

## Examples

### Basic Navigation and Interaction
```zen
go("https://example.com")
click("button.submit")
fill("input#username", "Grace")
fill("input#password", "secret")
click("button.submit")
wait(1000)
```

### Using the Page/Element Objects
```zen
let page = new Page()
page.go("https://httpbin.org/forms/post")
let el = page.find("input[name=custname]")
el.fill("Grace Hopper")
let page2 = page.click("button.submit")
```

### Screenshot
```zen
shot("screenshot.png")           // viewport screenshot
shot("full.png", true)           // full scrollable screenshot
```

### Wait and Check Workflow
```zen
go("https://example.com")
wait_for("h1")         // wait until h1 appears
let h1_text = text("h1")
print h1_text
```