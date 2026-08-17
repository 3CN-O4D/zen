# Scraping Examples

## Basic Scraping

```
go "https://example.com"
let title = find("h1").text
print "Title: " + title
```

## Scrape All Links

```
go "https://example.com"
let links = find_all("a")
for link in links {
    print link.text + " → " + link.attr("href")
}
```

## Scrape with Pagination

```
go "https://books.toscrape.com/"
let all_titles = []
let page_num = 1

while page_num <= 3 {
    print "--- Page " + page_num + " ---"
    for book in find_all("article.product_pod h3 a") {
        all_titles.append(book.attr("title"))
    }

    let next = find(".next a")
    if next.exists {
        click next
        wait 1000
        page_num = page_num + 1
    } else {
        break
    }
}
write_file("titles.txt", all_titles.join("\n"))
```

## Scrape to JSON

```
go "https://quotes.toscrape.com/"
let data = []

for quote in find_all(".quote") {
    data.append({
        "text": quote.find(".text").text,
        "author": quote.find(".author").text,
        "tags": quote.find_all(".tag").texts
    })
}

write_file("quotes.json", json_encode(data))
print "Saved " + data.len + " quotes"
```

## Dynamic Content (SPA)

```
go "https://example.com/spa"
wait_for ".app-root"

scroll to bottom
execute("document.querySelector('.load-more').click()")
wait 2000

let items = find_all(".item")
items.each(function(item, i) {
    print (i+1) + ". " + item.text
})
print "Loaded " + items.count + " items"
```

## Form Submission

```
go "https://portal.example.com"
page.inputs        // see all form fields
page.buttons       // see all buttons

fill("#username", "user@example.com")
fill("#password", "secret")
check("#remember-me")
click("#login-button")
wait_for(".dashboard")
```

## Search and Extract

```
go "https://example.com"
fill("#search", "query")
click("button[type='submit']")
wait_for(".results")

let results = find_all(".result-item")
for result in results {
    print result.find(".title").text
    print result.find(".description").text
    print "---"
}
```
