# Examples Overview

This section contains practical examples of using Zen for common tasks.

## Categories

- [Scraping](scraping.md) - Extracting data from websites
- [Automation](automation.md) - Automating browser workflows
- [APIs](apis.md) - Working with web APIs

## Quick Examples

### Hello World

```
print "Hello, World!"
```

### Scrape a Page

```
go "https://example.com"
print page.text
```

### Login

```
go "https://example.com/login"
fill("#username", "admin")
fill("#password", "secret")
click("button[type='submit']")
```

### Take a Screenshot

```
go "https://example.com"
shot "screenshot.png"
```

### HTTP Request

```
let resp = http.get("https://api.github.com/repos/3CN-O4D/zen")
print resp.json()["stargazers_count"]
```
