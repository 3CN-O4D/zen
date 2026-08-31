# Web Scraping Examples

Zen's native performance and clean syntax make it ideal for scraping structured data from the web.

## Simple Page Scraping

Extracting titles and links from a page.

```zen
browser.launch()
browser.navigate("https://news.ycombinator.com")

# 1. Get the page title
print("Page: ${browser.get_title()}")

# 2. Extract all article titles using a CSS selector
var headlines = browser.query(".titleline > a")

for title in headlines {
    print("- ${title}")
}

browser.close()
```

## Scraping with Pagination

Navigating multiple pages to collect data.

```zen
browser.launch()
browser.navigate("https://example.com/products")

var all_products = []

for i in 1 -> 5 {
    print("Scraping page ${i}...")
    
    # Collect products on current page
    var page_products = browser.query(".product-name")
    all_products = all_products.concat(page_products)
    
    # Click next if available
    if i < 5 {
        browser.click(".next-page")
        browser.wait_for(".product-name")
    }
}

print("Total products found: ${all_products.len}")
browser.close()
```

## Scraping JavaScript-Heavy Sites

Zen handles dynamic content naturally by waiting for elements to appear.

```zen
browser.launch()
browser.navigate("https://api-dashboard.example.com")

# Wait for the chart data to load via JS
browser.wait_for(".data-point", 10000)

# Extract specific attribute (e.g., data-value)
var values = []
var items = browser.query(".data-point")
for item in items {
    # browser.attr returns the attribute of the first match
    # for full scraping, use browser.evaluate to run JS directly
    var val = browser.evaluate("Array.from(document.querySelectorAll('.data-point')).map(e => e.dataset.value)")
    values = val
    break
}

print("Live values: ${values}")
browser.close()
```

## Headless Data Export

Combining scraping with the `csv` module.

```zen
import csv

browser.launch({headless: true})
browser.navigate("https://example.com/prices")

var rows = [["Product", "Price"]]
var names = browser.query(".name")
var prices = browser.query(".price")

for i in 0 .. names.len {
    rows = rows.push([names[i], prices[i]])
}

csv.write("prices.csv", rows)
print("Data exported to prices.csv")
browser.close()
```

## See Also
- [Finding Elements](../browser/finding.md) — Advanced selectors and text search.
- [Evaluating JS](../browser/interacting.md#evaluating-javascript) — Running custom logic in the page.
- [CSV Module](../modules/csv.md) — Saving your scraped data.
