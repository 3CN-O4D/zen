# urllib — URL handling

The `urllib` module provides tools for parsing, encoding, and making simple URL-based requests. It is available globally as `urllib`.

```zen
# 1. Parsing a URL
var u = urllib.parse("https://example.com:8080/path?query=1")
print(u.host)   # example.com
print(u.port)   # 8080
print(u.query)  # query=1

# 2. Quoting strings for URLs
print(urllib.quote("hello world!")) # hello%20world%21
```

## Functions

| Function | Description |
|----------|-------------|
| `parse(url)` | Parses a URL string into a dict of its components. |
| `quote(s)` | Percent-encodes a string for use in a URL. |
| `unquote(s)` | Decodes a percent-encoded string. |
| `urlencode(dict)` | Converts a dict of parameters into a query string. |
| `parse_qs(query)` | Parses a query string back into a dict. |
| `urlopen(url)` | Opens a URL and returns the response body (simple GET). |

## Examples

### Building a query string
```zen
var params = { "q": "zen language", "lang": "en" }
var qs = urllib.urlencode(params)
var url = "https://google.com/search?" + qs
print(url) # https://google.com/search?q=zen%20language&lang=en
```

## See Also
- [http](http.md) — For a more powerful HTTP client.
- [json](json.md) — Often used for API query parameters.
