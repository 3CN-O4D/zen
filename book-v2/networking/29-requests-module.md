# Chapter 29: requests Module

The `requests` module is a Python-requests-parity HTTP client. It supports GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS, sessions with cookies and headers, timeouts, redirects, JSON, TLS, and more.

## 29.1 Quick requests

```zen
import requests

// GET
let resp = requests.get("https://example.com")
print(resp.status_code)   // 200
print(resp.text)          // response body
print(resp.headers)       // headers dict

// GET with query params
let resp = requests.get("https://example.com/search", {"params": {"q": "zen"}})

// POST with JSON
let resp = requests.post("https://example.com/api", {
    "json": {"name": "Zen", "version": 3},
    "headers": {"Content-Type": "application/json"}
})
print(resp.status_code)

// POST with form data
let resp = requests.post("https://example.com/api", {
    "data": {"name": "Zen", "version": "3"}
})

// PUT / PATCH / DELETE / HEAD / OPTIONS
requests.put("https://example.com/api/1", {"json": {"name": "Zen"}})
requests.patch("https://example.com/api/1", {"json": {"version": 4}})
requests.delete("https://example.com/api/1")
requests.head("https://example.com")
requests.options("https://example.com")
```

### Response object

| Attribute | Description |
|-----------|-------------|
| `status_code` | HTTP status code |
| `reason` | Reason phrase |
| `text` | Body as string |
| `content` | Body as bytes |
| `headers` | Headers dict |
| `ok` | True if 200–299 |
| `url` | Final URL (after redirects) |
| `elapsed` | Request time |
| `cookies` | Cookies dict |
| `json()` | Parsed JSON body |

```zen
import requests

let resp = requests.get("https://api.example.com/user/1")
if resp.ok {
    let user = resp.json()
    print(user["name"])
} else {
    print("error: " + str(resp.status_code))
}
```

## 29.2 Options dictionary

All request functions accept an options dict:

| Option | Description |
|--------|-------------|
| `params` | Query parameters dict |
| `data` | Form data dict |
| `json` | JSON body dict |
| `headers` | HTTP headers dict |
| `timeout` | Timeout in seconds |
| `allow_redirects` | Follow redirects (default true) |
| `auth` | `[user, password]` list |
| `cookies` | Cookies dict |

```zen
import requests

let resp = requests.get("https://example.com", {
    "params": {"q": "zen", "page": 1},
    "headers": {"User-Agent": "Zen/3.0"},
    "timeout": 10,
    "allow_redirects": true
})
```

## 29.3 Sessions

`requests.Session` persists cookies, headers, and auth across requests:

```zen
import requests

let sess = requests.Session()
sess.headers = {"User-Agent": "Zen/3.0"}
sess.cookies = {"sessionid": "abc123"}

let r1 = requests.sess_get(sess, "https://example.com/page1")
let r2 = requests.sess_post(sess, "https://example.com/api", {
    "json": {"action": "login"}
})
requests.session_close(sess)
```

### Session helpers

| Function | Description |
|----------|-------------|
| `requests.Session()` | Create a session |
| `requests.sess_get(sess, url, opts?)` | GET with session |
| `requests.sess_post(sess, url, opts?)` | POST with session |
| `requests.sess_put(sess, url, opts?)` | PUT with session |
| `requests.sess_patch(sess, url, opts?)` | PATCH with session |
| `requests.sess_delete(sess, url, opts?)` | DELETE with session |
| `requests.sess_head(sess, url, opts?)` | HEAD with session |
| `requests.sess_options(sess, url, opts?)` | OPTIONS with session |
| `requests.session_headers(sess, opts?)` | Get/update headers |
| `requests.session_cookies(sess, opts?)` | Get/update cookies |
| `requests.session_close(sess)` | Close session |

```zen
import requests

let sess = requests.Session()
sess.headers = {"Authorization": "Bearer token123"}
sess.cookies = {"session": "abc123"}

let r1 = requests.sess_get(sess, "https://api.example.com/me")
print(r1.json()["username"])

requests.session_close(sess)
```

## 29.4 JSON helpers

```zen
import requests

// POST JSON
let resp = requests.post("https://example.com/api", {
    "json": {"name": "Zen", "version": 3}
})

// Parse response JSON
let data = resp.json()
print(data["id"])

// Raise on error status
requests.raise_for_status(resp)
```

### Status codes

```zen
import requests

let codes = requests.codes
print(codes.ok)      // 200
print(codes.created) // 201
print(codes.not_found) // 404
print(codes.server_error) // 500
```

## 29.5 URL encoding utilities

```zen
import requests

let encoded = requests.utils_quote("hello world")
print(encoded)  // hello%20world

let decoded = requests.utils_unquote("hello%20world")
print(decoded)  // hello world

print(requests.default_user_agent())
```

## 29.6 Common patterns

### Download a file

```zen
import requests
import fs

let resp = requests.get("https://example.com/file.zip")
fs.write_binary("file.zip", resp.content)
```

### Upload JSON

```zen
import requests

let resp = requests.post("https://example.com/api", {
    "json": {"name": "Zen", "version": 3},
    "headers": {"Authorization": "Bearer token123"}
})
```

### With timeout and error handling

```zen
import requests

let resp = requests.get("https://example.com", {
    "timeout": 30
})
if !resp.ok {
    print("HTTP " + str(resp.status_code))
} else {
    print(resp.text)
}

// Catch network errors
try {
    let resp = requests.get("https://example.com", {"timeout": 5})
    print(resp.text)
} catch as e {
    print("request error: " + e.msg)
}
```

### Basic auth

```zen
import requests

let resp = requests.get("https://example.com", {
    "auth": ["user", "pass"]
})
```

## 29.7 Error handling

```zen
import requests

try {
    let resp = requests.get("https://example.com", {"timeout": 5})
    if !resp.ok {
        print("HTTP " + str(resp.status_code))
    } else {
        print(resp.text)
    }
} catch as e {
    print("request error: " + e.msg)
}
```

## 29.8 Quick reference

```zen
import requests

requests.get(url, opts?)
requests.post(url, opts?)
requests.put(url, opts?)
requests.patch(url, opts?)
requests.delete(url, opts?)
requests.head(url, opts?)
requests.options(url, opts?)

let sess = requests.Session()
requests.sess_get(sess, url, opts?)
requests.session_close(sess)

resp.status_code
resp.text
resp.content
resp.headers
resp.ok
resp.url
resp.json()
resp.cookies

requests.utils_quote(s)
requests.utils_unquote(s)
requests.default_user_agent()
requests.codes.ok
requests.raise_for_status(resp)
```

## 29.9 Exercises

1. Fetch a public JSON API and print a specific field.
2. POST JSON to a test endpoint and check the response.
3. Use a session to make two requests that share cookies.
4. Download an image and save it with `fs.write_binary`.
5. Add basic auth and custom headers to a request.
6. Write a function `get_json(url)` that fetches and parses JSON, returning `nil` on failure.
7. Write a retry function that retries a GET up to 3 times on failure.


## 29.10 Advanced usage patterns

### Sessions (connection reuse)
```zen
import requests
let s = requests.Session()
s.headers.update({"User-Agent": "ZenBot/1.0"})
let r1 = s.get("https://httpbin.org/headers")
let r2 = s.get("https://httpbin.org/cookies/set?x=1")
```

### Custom headers and auth
```zen
let r = requests.get(url, headers={"Authorization": "Bearer token"}, auth=("user","pass"))
```

### Query parameters
```zen
let r = requests.get("https://httpbin.org/get", params={"q": "zen", "page": 1})
```

### POST forms and JSON
```zen
let r = requests.post(url, data={"key": "val"})  # form
let r = requests.post(url, json={"key": "val"}) # JSON
```

### File uploads
```zen
let files = {"file": open("doc.txt", "rb")}
let r = requests.post(url, files=files)
```

### Timeouts and retries
```zen
let r = requests.get(url, timeout=5)
```

### Streaming
```zen
let r = requests.get(url, stream=true)
# iterate over r.iter_content()
```

### Response object details
- `r.status_code`, `r.reason`
- `r.text`, `r.content` (bytes)
- `r.json()`
- `r.headers`, `r.cookies`
- `r.url`, `r.elapsed`

### SSL verification
```zen
let r = requests.get(url, verify=true)  # default
# let r = requests.get(url, verify=false)  # dangerous, only for testing
```

### Proxies
```zen
let proxies = {"http": "http://proxy:8080", "https": "http://proxy:8080"}
let r = requests.get(url, proxies=proxies)
```

### Redirects
```zen
let r = requests.get(url, allow_redirects=true)
```

## 29.11 Error handling
```zen
import requests
try {
    let r = requests.get("https://invalid.example", timeout=2)
    r.raise_for_status()
} catch(e) {
    print(e)
}
```

## 29.12 Best practices
- Always use timeouts
- Handle exceptions
- Check status codes
- Respect robots.txt and rate limits
- Use sessions for multiple requests
- Don't disable verify in production

## 29.13 Exercises
1. GET request and parse JSON
2. POST JSON to API
3. Use session with cookies
4. Download file with streaming
5. Handle errors and timeouts
6. Set custom headers
7. Follow redirects and inspect history
