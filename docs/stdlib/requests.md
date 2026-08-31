# requests — Simplified HTTP

The `requests` module is a high-level wrapper around the native `http` module,
providing a more intuitive API inspired by Python's Requests library.

```zen
import requests

# 1. Simple GET
var r = requests.get("https://api.github.com/events")
print(r.status_code())
print(r.json())

# 2. POST with JSON
var payload = {key: "value"}
var r = requests.post("https://httpbin.org/post", {json: payload})
```

## Methods

| Function | Description |
|----------|-------------|
| `get(url, params?)` | Sends a GET request. |
| `post(url, data?, json?)` | Sends a POST request. |
| `put(url, data?, json?)` | Sends a PUT request. |
| `delete(url)` | Sends a DELETE request. |
| `head(url)` | Sends a HEAD request. |
| `patch(url, data?, json?)` | Sends a PATCH request. |

## The Response Object

The `requests` methods return a Response object with these methods:

| Method | Description |
|--------|-------------|
| `json()` | Returns the response body parsed as JSON. |
| `text()` | Returns the response body as a string. |
| `status_code()` | Returns the HTTP status code (int). |
| `headers()` | Returns the response headers (dict). |
| `raise_for_status()` | Throws an error if the status is 4xx or 5xx. |

## Sessions

A `Session` allows you to persist certain parameters across multiple requests.

```zen
var s = requests.Session()
s.get("https://httpbin.org/cookies/set/session/1234")
var r = s.get("https://httpbin.org/cookies")
print(r.text()) # Cookies are preserved
```

## See Also
- [http](../modules/http.md) — The native HTTP client.
- [json](../modules/json.md) — JSON encoding/decoding.
