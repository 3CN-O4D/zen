# Navigation

## Page Navigation

```
go "https://example.com"
go url_variable                     // variable holding a URL

// Go to a different page
refresh                             // reload current page
back                                // go back in history
forward                             // go forward in history
```

## Waiting

```
wait 2000                           // wait 2000ms
wait "2s"                           // wait 2 seconds
wait_for ".loaded"                  // wait for element to appear
wait_for(text="Loading complete")   // wait for text to appear
wait_for_network()                  // wait for network to idle
```

## Scrolling

```
scroll to top
scroll to bottom
scroll by 0, 500                    // scroll 500px down
scroll_to("bottom")                 // function form
scroll_to(0, 500)                   // scroll to coordinates
```

## JavaScript Execution

```
let title = execute("document.title")
let height = execute("document.body.scrollHeight")
execute("document.querySelector('form').submit()")
```

## User Agent & Headers

```
user_agent()                              // → "Mozilla/5.0 ..."
set_user_agent("Mozilla/5.0 Custom Bot")  // override UA
set_headers({"Authorization": "Bearer ..."})  // set extra HTTP headers
headers()                                 // → currently set headers
```

`set_headers()` applies to all subsequent requests on the page. Useful for API authentication, custom headers, or bot detection bypass.

## Download

```
download "https://example.com/file.zip" to "/tmp/file.zip"
```

## Search (Flexible Element Finder)

`search()` is a more powerful `find_all()` — it auto-detects the selector type and always returns a ZenList:

```
search("Login")              // visible text (auto-detect)
search("div.result")         // CSS selector
search("/pattern/i")         // regex text match
search("text=Exact Text")    // explicit text
search("url=example.com")    // link by href
```

It waits for elements to appear before returning.
