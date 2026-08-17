# Page Info

## The `page` Module

Access page information through the `page` object (clean property access):

| Property | Description |
|----------|-------------|
| `page.html` | Full page HTML |
| `page.text` | Visible text with `{[media]}` markers |
| `page.links` | All link URLs on page |
| `page.images` | All image URLs on page |
| `page.forms` | All forms with their inputs |
| `page.inputs` | All input/select/textarea fields |
| `page.buttons` | All buttons and clickable elements |
| `page.title` | Page title |
| `page.url` | Current page URL |
| `page.source` | Alias for page.html |

Examples:

```
page.title           // "MOI University | Student Portal"
page.inputs          // [{id: "username", type: "text", ...}, ...]
page.buttons         // [{id: "btnLogin", text: "Log In"}, ...]
```

## Special Variables (Legacy)

```
_page_html           // Full page HTML
_page_text           // Visible text with {[media]} markers
_page_links          // All link URLs
_page_images         // All image URLs
_page_forms          // All forms
_page_inputs         // All input fields
_page_buttons        // All buttons
_page_urls           // All URLs visited this session
```

## Function Equivalents (Legacy)

```
page_html()
page_text()
page_links()
page_images()
page_forms()
```

## ZenList (Element Lists)

### Getting a List

```
let list = find_all("a")        // CSS
let list = find_all("div.item") // all matching elements
```

### Properties

| Property | Description |
|----------|-------------|
| `.count` | Number of elements |
| `.len` | Same as count |
| `.first` | First element (ZenElement or null) |
| `.texts` | List of inner texts |
| `.htmls` | List of inner HTMLs |
| `.tags` | List of tag names |

### Methods

| Method | Description |
|--------|-------------|
| `.nth(n)` | Element at index n |
| `.attr("href")` | List of attribute values |
| `.attrs("href")` | Same as attr() |
| `.each(callback)` | Iterate: `fn(element, index)` |
| `.sorted()` | Sorted by text |
| `.to_list()` | Convert to plain list |

### Iteration

```
find_all("a").each(function(link, i) {
    print (i+1) + ". " + link.text + " → " + link.attr("href")
})
```

### With for/in

```
for link in find_all("a") {
    print link.text + " → " + link.attr("href")
}
```
