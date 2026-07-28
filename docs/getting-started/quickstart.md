# Quick Start

## Your First Script

Create a file `hello.z`:

```
print "Hello from Zen!"
go "https://example.com"
print "Title: " + title()
```

Run it:

```bash
zen run hello.z
```

## Interactive Shell

Start the shell:

```bash
zen shell
```

Try some commands:

```
zen ❯ print 2 + 2
4

zen ❯ go "https://example.com"
true

zen ❯ title()
"Example Domain"

zen ❯ page.text
"Example Domain\nThis domain is for use in illustrative examples..."
```

## Browser Automation in 3 Lines

```
go "https://example.com"
print title()
print page.text
```

## Login Automation

```
go "https://the-internet.herokuapp.com/login"
find("#username").fill("tomsmith")
find("#password").fill("SuperSecretPassword!")
click(".radius")
wait_for(".flash.success")
```

## Scrape Data

```
go "https://quotes.toscrape.com/"
let quotes = []

for quote in find_all(".quote") {
    quotes.append({
        "text": quote.find(".text").text,
        "author": quote.find(".author").text
    })
}

write_file("quotes.json", json_encode(quotes))
print "Saved " + quotes.len + " quotes"
```

## Take a Screenshot

```bash
zen shot https://example.com -o screenshot.png
```

## Run Inline Code

```bash
zen -e 'print "Hello from Zen!"'
```

## What's Next?

- [Shell Usage](shell.md) - Interactive shell features
- [Scripts](scripts.md) - Writing and running scripts
- [Language Overview](../language/overview.md) - Learn the Zen language
