# Chapter 33: url Module

The `url` module provides URL parsing, building, and percent-encoding: `urllib.parse` utilities.

## 33.1 Parsing URLs

```zen
import urllib.parse

let url = "https://user:pass@example.com:8080/path/to/page?q=1#section"
let parts = urllib.parse.urlparse(url)

print(parts.scheme)    # https
print(parts.netloc)    # user:pass@example.com:8080
print(parts.path)      # /path/to/page
print(parts.params)    # (empty)
print(parts.query)     # q=1
print(parts.fragment)  # section
print(parts.username)  # user
print(parts.password)  # pass
print(parts.hostname)  # example.com
print(parts.port)      # 8080
```

### ParseResult fields

| Field | Description | Example |
|-------|-------------|---------|
| `scheme` | URL scheme | `"https"` |
| `netloc` | Network location | `"user:pass@example.com:8080"` |
| `path` | Hierarchical path | `"/path/to/page"` |
| `params` | Parameters | `""` |
| `query` | Query string | `"q=1"` |
| `fragment` | Fragment | `"section"` |
| `username` | Username | `"user"` |
| `password` | Password | `"pass"` |
| `hostname` | Hostname | `"example.com"` |
| `port` | Port | `8080` |

```zen
import urllib.parse

let p = urllib.parse.urlparse("https://user:pass@example.com:8080/path?q=1#frag")
print(p.scheme)     # "https"
print(p.netloc)     # "user:pass@example.com:8080"
print(p.path)       # "/path"
print(p.params)     # ""
print(p.query)      # "q=1"
print(p.fragment)   # "frag"
print(p.username)   # "user"
print(p.password)   # "pass"
print(p.hostname)   # "example.com"
print(p.port)       # 8080
```

## 23.2 Building URLs

```zen
import urllib.parse

let url = urllib.parse.urlunparse({
    "scheme": "https",
    "netloc": "example.com",
    "path": "/search",
    "params": "",
    "query": "q=zen",
    "fragment": ""
})
print(url)  # https://example.com/search?q=zen
```

`urlunparse` takes a dict with keys: `scheme`, `netloc`, `path`, `params`, `query`, `fragment`.

## 33.3 Query strings

```zen
import urllib.parse

// Encode query params
let qs = urllib.parse.urlencode({"q": "zen lang", "page": 1})
print(qs)  // q=zen+lang&page=1

// Parse query string
let params = urllib.parse.parse_qs("q=zen+lang&page=1")
print(params)  // {"q": ["zen lang"], "page": ["1"]}
```

### urlencode

```zen
import urllib.parse

print(urllib.parse.urlencode({"q": "hello world", "page": 2}))
// q=hello+world&page=2

print(urllib.parse.urlencode({"a": 1, "b": 2, "c": 3}))
// a=1&b=2&c=3
```

### parse_qs

```zen
import urllib.parse

print(urllib.parse.parse_qs("q=zen+lang&page=1"))
// {"q": ["zen lang"], "page": ["1"]}

print(urllib.parse.parse_qs("a=1&a=2&b=3"))
// {"a": ["1", "2"], "b": ["3"]}
```

Values are always lists (to support repeated keys).

## 33.4 Joining URLs

```zen
import urllib.parse

let full = urllib.parse.urljoin("https://example.com/base/", "page.html")
print(full)  // https://example.com/page.html

let full2 = urllib.parse.urljoin("https://example.com/base/", "/abs.html")
print(full2)  // https://example.com/abs.html
```

```zen
import urllib.parse

// Relative to base
print(urllib.parse.urljoin("https://example.com/a/b/", "c.html"))  
// https://example.com/a/b/c.html

print(urllib.parse.urljoin("https://example.com/a/b/", "../c.html"))  
// https://example.com/a/c.html

print(urllib.parse.urljoin("https://example.com/", "page.html"))  
// https://example.com/page.html
```

## 33.5 Percent encoding

```zen
import urllib.parse

let encoded = urllib.parse.quote("hello world")
print(encoded)  // hello%20world

let decoded = urllib.parse.unquote("hello%20world")
print(decoded)  // hello world
```

```zen
import urllib.parse

print(urllib.parse.quote("hello world"))     # hello%20world
print(urllib.parse.quote("a/b?c=d&e=f"))     # a%2Fb%3Fc%3De%26f
print(urllib.parse.quote("中文"))             # %E4%B8%AD%E6%96%87

print(urllib.parse.unquote("hello%20world"))   # hello world
print(urllib.parse.unquote("%E4%B8%AD%E6%96%87"))  # 中文
```

## 33.6 Splitting and joining netloc

```zen
import urllib.parse

let parts = urllib.parse.splituser("user:pass@example.com:8080")
print(parts)  // ["user:pass", "example.com:8080"]

let splitpass = urllib.parse.splitpasswd("user:pass@example.com:8080")
print(splitpass)  // ["user", "pass@example.com:8080"]

let port = urllib.parse.splitport("example.com:8080")
print(port)  // ["example.com", "8080"]
```

## 33.6 Common patterns

### Build a search URL

```zen
import urllib.parse

let base = "https://example.com/search"
let qs = urllib.parse.urlencode({"q": "Zen language", "page": 2})
let url = base + "?" + qs
print(url)  // https://example.com/search?q=Zen+language&page=2
```

### Extract domain from URL

```zen
import urllib.parse

let parts = urllib.parse.urlparse("https://sub.example.com/path")
print(parts.hostname)  // sub.example.com
```

### Add a query param to existing URL

```zen
import urllib.parse

let url = "https://example.com/page?a=1"
let parts = urllib.parse.urlparse(url)
let params = urllib.parse.parse_qs(parts.query)
params["b"] = ["2"]
let new_qs = urllib.parse.urlencode(params)
let new_url = urllib.parse.urlunparse({
    "scheme": parts.scheme,
    "netloc": parts.netloc,
    "path": parts.path,
    "params": parts.params,
    "query": new_qs,
    "fragment": parts.fragment
})
print(new_url)
```

## 33.7 Quick reference

```zen
import urllib.parse

urllib.parse.urlparse(url)        // parse into parts
urllib.parse.urlunparse(parts)    // build from parts
urllib.parse.urljoin(base, ref)   // resolve relative URL
urllib.parse.urlencode(params)    // encode query string
urllib.parse.parse_qs(qs)         // parse query string
urllib.parse.quote(s)             // percent-encode
urllib.parse.unquote(s)           // percent-decode

let p = urllib.parse.urlparse("https://example.com/path?q=1")
p.scheme      // https
p.netloc      // example.com
p.path        // /path
p.query       // q=1
p.fragment    // (empty)
p.hostname    // example.com
p.port        // (none)
```

## 33.8 Exercises

1. Write `extract_domain(url)` that returns the hostname.
2. Write `add_param(url, key, value)` that adds/replaces a query parameter.
3. Write `build_api_url(base, endpoint, params)` that constructs a full API URL.
4. Write `decode_query(qs)` that returns a dict of query parameters.
5. Write `normalize_url(url)` that ensures a scheme and trailing slash.
6. Write `get_query_param(url, key)` that extracts a single query parameter value.
7. Write `merge_url_params(url, params)` that merges new params into existing URL.


## 33.12 URL module — deep reference

### URL anatomy

```
https://user:pass@example.com:8080/path/to/page?q=1&x=2#section
 \__/   \___/ \_______/ \_______/ \_____/ \______/\______/
scheme  auth     host       port    path   query    fragment
```

### Components

| Component | Purpose |
|-----------|---------|
| scheme | protocol (http, https, ftp, ws) |
| host | domain or IP |
| port | TCP port (default 80/443) |
| path | resource path |
| query | key=value pairs after `?` |
| fragment | anchor after `#` |
| username/password | basic auth (deprecated/insecure) |

### Usage

```zen
import url
let u = url.parse("https://example.com:8080/path?q=1#sec")
print(u.scheme)    # https
print(u.host)      # example.com
print(u.port)      # 8080
print(u.path)      # /path
print(u.query)     # q=1
print(u.fragment)  # sec

let built = url.build(scheme="https", host="example.com", path="/search", query="q=zen")
print(built)   # https://example.com/search?q=zen

let joined = url.join("https://example.com/base/", "sub/page")
print(joined)  # https://example.com/sub/page

let encoded = url.quote("hello world")
print(encoded)   # hello%20world
let decoded = url.unquote("hello%20world")
print(decoded)   # hello world

let params = url.parse_qs("q=1&x=2&q=3")
print(params)   # #{q: ["1","3"], x: ["2"]}
```

### Encoding rules

- `quote`: percent-encode a path segment.
- `quote_plus`: like quote but space → `+`.
- `unquote`/`unquote_plus`: reverse.

### Exercises

1. Parse a URL and print all components.
2. Build a URL from components.
3. Join a base URL with a relative path.
4. Percent-encode/decode a search query.
5. Parse query string into a dict of lists.
6. Write `extract_domain(url)` returning just the host.
7. Write `add_query_param(url, k, v)` adding/updating a param.
8. Explain why `&` and `=` need encoding in query values.
9. Write a tiny URL shortener encoder (conceptually).
10. Compare `quote` vs `quote_plus`.

