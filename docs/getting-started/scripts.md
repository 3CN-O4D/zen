# Scripts

## File Extension

Zen scripts use the `.z` extension.

## Running Scripts

```bash
zen run script.z
```

With options:

```bash
zen run script.z --headful
```

## Inline Evaluation

Use `-e` / `--eval` for inline code:

```bash
zen run -e 'print "hello"' -e 'print 1 + 1'
```

Multi-line:

```bash
zen run -e '
    for i in 1 -> 5 {
        print i
    }
'
```

Shorthand (without `run`):

```bash
zen -e 'print "Hello from Zen!"'
```

## Including Files

Use `include` to load other files:

```
include "lib/str.z"
include "lib/dict.z"
include "helpers.z"
```

## Script Structure

A typical script:

```
// 1. Configuration
let BASE_URL = "https://example.com"

// 2. Helper functions
function scrape_page(url) {
    go url
    wait_for("body")
    return find_all(".item")
}

// 3. Main logic
go BASE_URL
let items = scrape_page(BASE_URL + "/page1")
print "Found " + items.count + " items"
```

## Examples

### Hello World

```
print "Hello, World!"
```

### Scrape Links

```
function scrape_links(url) {
    go url
    wait_for("body")
    let links = find_all("a")
    return links.attr("href")
}

let links = scrape_links("https://example.com")
write_file("links.txt", links.join("\n"))
```

### Form Fill

```
go "https://example.com/login"
fill("#username", "admin")
fill("#password", "secret")
click("button[type='submit']")
wait_for(".dashboard")
```

### Paginated Crawl

```
go "https://example.com/list"
let all_items = []
let page_num = 1

while page_num <= 5 {
    for item in find_all(".item") {
        all_items.append(item.text)
    }

    let next = find(".next")
    if next.exists {
        click next
        wait 1000
        page_num = page_num + 1
    } else {
        break
    }
}

write_file("items.txt", all_items.join("\n"))
```
