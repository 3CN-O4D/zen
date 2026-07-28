# Finding Elements

## CSS Selectors (Default)

```
find("div.class")           // first match
find_all("a[href]")         // all matches
find("#id")                 // by ID
find("div > span")          // child combinator
```

## By Visible Text

```
find_by_text("Click Here")             // partial match
find_by_text("Login", exact=true)      // exact match only
```

## Keyword Arguments for find/click/fill

```
find(text="Reg. Number")              // by text
find("#username").fill("admin")       // by CSS
click(text="Log In")                  // click by text
fill(text="Username", with="admin")   // fill by text
```

## Text Selector Objects

```
click by_text("Submit")
fill by_text("Username") with "admin"
```

## By Link URL

```
find_by_url("example.com")                      // partial match
find_by_url("https://example.com/page", partial=false)  // exact
```

## Regex on Text

```
click "/submit|save/i"       // case-insensitive regex match against element text
```

## Search (Flexible)

```
search("python")              // auto-detect (text with spaces → text search)
search("div.result")          // CSS selector
search("/pattern/i")          // regex
search("text=Exact Text")     // explicit text
search("url=example.com")     // by URL
```

## Auto-Detection

When you pass a bare string to `find()` or `click()`, Zen auto-detects:

- Strings with CSS characters (`.`, `#`, `:`, `>`, etc.) → treated as CSS
- Strings with spaces but no CSS characters → treated as text
- Single words → treated as CSS tag name

```
find("div.item")              // CSS (has '.')
find("Log In")                // text (has space, no CSS chars)
click("button")               // CSS tag name
```
