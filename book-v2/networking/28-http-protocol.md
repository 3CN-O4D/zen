# Chapter 28: HTTP Protocol

Zen's `http` module provides a low-level HTTP/1.1 client. For a higher-level experience with sessions, cookies, and JSON, use the `requests` module (Chapter 29).

## 28.1 HTTP basics

HTTP (Hypertext Transfer Protocol) is the foundation of the web. A client sends a request; a server responds.

### Request anatomy

```
GET /index.html HTTP/1.1
Host: example.com
User-Agent: Zen/3.0
Accept: text/html

```

- **Method**: GET, POST, PUT, DELETE, HEAD, OPTIONS, PATCH
- **Path**: `/index.html`
- **Version**: `HTTP/1.1`
- **Headers**: `Host`, `User-Agent`, `Accept`, etc.
- **Body**: empty for GET, present for POST/PUT

### Response anatomy

```
HTTP/1.1 200 OK
Content-Type: text/html
Content-Length: 137

<html><body>Hello</body></html>
```

- **Status line**: `HTTP/1.1 200 OK`
- **Headers**: `Content-Type`, `Content-Length`, etc.
- **Body**: the actual content (may be empty)

## 28.2 The `http` module

Zen's `http` module provides low-level HTTP/1.1 client functions:

```zen
import http

let resp = http.get("https://example.com")
print(resp.status_code)   # 200
print(resp.body)          # response body
print(resp.headers)       # headers dict
```

### http functions

| Function | Description |
|----------|-------------|
| `http.get(url, headers?)` | GET request |
| `http.post(url, body, headers?)` | POST request |
| `http.put(url, body, headers?)` | PUT request |
| `http.delete(url, headers?)` | DELETE request |
| `http.head(url, headers?)` | HEAD request |
| `http.options(url, headers?)` | OPTIONS request |
| `http.patch(url, body, headers?)` | PATCH request |

### Response object

| Field | Description |
|-------|-------------|
| `status_code` | HTTP status code (int) |
| `reason` | Reason phrase (string) |
| `headers` | Response headers (dict) |
| `body` | Response body (string) |
| `ok` | True if 200–299 |

```zen
import http

let resp = http.get("https://example.com")
print(resp.status_code)   # 200
print(resp.reason)        # OK
print(resp.body)          # HTML body
print(resp.headers)       # headers dict
print(resp.ok)            # true

if resp.ok {
    print("Success!")
} else {
    print("Error: " + str(resp.status_code) + " " + resp.reason)
}
```

## 28.3 Making requests with headers

```zen
import http

let resp = http.get("https://api.example.com/user/1", {
    "User-Agent": "Zen/3.0",
    "Accept": "application/json",
    "Authorization": "Bearer token123"
})

print(resp.status_code)
print(resp.body)
```

## 28.4 POST with body

```zen
import http

let body = "name=Zen&version=3"
let resp = http.post("https://example.com/api", body, {
    "Content-Type": "application/x-www-form-urlencoded",
    "Content-Length": str(len(body))
})

print(resp.status_code)
print(resp.body)
```

## 28.5 Status codes

| Code | Name | Meaning |
|------|------|---------|
| 200 | OK | Success |
| 201 | Created | Resource created |
| 204 | No Content | Success, no body |
| 301 | Moved Permanently | Redirect |
| 302 | Found | Temporary redirect |
| 304 | Not Modified | Cache hit |
| 400 | Bad Request | Client error |
| 401 | Unauthorized | Authentication required |
| 403 | Forbidden | Access denied |
| 404 | Not Found | Resource not found |
| 500 | Internal Server Error | Server error |
| 502 | Bad Gateway | Upstream error |
| 503 | Service Unavailable | Server overloaded |

```zen
import http

let resp = http.get("https://example.com/nonexistent")
print(resp.status_code)   # 404
print(resp.ok)            # false
```

## 28.6 HTTP vs HTTPS

Zen's `http` module supports both `http://` and `https://` URLs. HTTPS uses TLS under the hood (certificate validation via the platform root store).

```zen
import http

// HTTP (port 80)
let r1 = http.get("http://example.com")

// HTTPS (port 443, TLS)
let r2 = http.get("https://example.com")
```

For certificate pinning, use the `tls` module directly (Chapter 25) or `requests` with verify options (Chapter 29).

## 28.7 Common patterns

### Simple GET

```zen
import http

let resp = http.get("https://example.com")
if resp.ok {
    print(resp.body)
} else {
    print("HTTP " + str(resp.status_code))
}
```

### POST JSON

```zen
import http
import json

let body = json.dumps({"name": "Zen", "version": 3})
let resp = http.post("https://example.com/api", body, {
    "Content-Type": "application/json",
    "Content-Length": str(len(body))
})

print(resp.status_code)
print(resp.body)
```

### Check if resource exists (HEAD)

```zen
import http

let resp = http.head("https://example.com/image.png")
if resp.ok {
    print("Resource exists, size: " + resp.headers["Content-Length"])
} else {
    print("Resource not found")
}
```

## 28.8 Common pitfalls

- **`http` is low-level.** No automatic redirect following, no session cookies, no connection pooling. Use `requests` for those features.
- **Always check `resp.ok`.** A non-200 response still returns a response object — it doesn't raise automatically.
- **HTTPS requires TLS support.** If Zen is built without TLS, `https://` URLs will fail.
- **`Content-Length` must match** the actual body length when sending POST/PUT.
- **Redirects are not followed** automatically by `http`. Use `requests` for redirect following.

## 28.9 Quick reference

```zen
import http

let resp = http.get("https://example.com", {
    "User-Agent": "Zen/3.0"
})

resp.status_code   # 200
resp.reason        # "OK"
resp.headers       # headers dict
resp.body          # response body
resp.ok            # true if 200-299

let resp2 = http.post("https://example.com/api", "data", {
    "Content-Type": "application/json"
})
```

## 28.10 Exercises

1. Write a function `fetch_title(url)` that fetches a page and extracts the `<title>` tag.
2. Write a script that checks if a list of URLs return 200 or 404.
3. Write a `head_check(url)` that returns the `Content-Length` header without downloading the body.
4. Write a POST script that sends form data (`application/x-www-form-urlencoded`).
5. Compare `http.get` vs `requests.get` — note the differences in convenience features.
