# http — HTTP client

The `http` module provides a high-level interface for making HTTP requests. It is available globally as `http`.

```zen
# 1. Simple GET request
var resp = http.get("https://example.com")
if resp.ok {
    print(resp.text())
}

# 2. POST with JSON
var data = '{"name": "Zen"}'
var resp = http.post("https://api.example.com/users", {
    headers: { "Content-Type": "application/json" },
    json: data
})
```

## Methods

| Function | Description |
|----------|-------------|
| `get(url, opts?)` | Sends a GET request. |
| `post(url, opts?)` | Sends a POST request. |
| `put(url, opts?)` | Sends a PUT request. |
| `del(url, opts?)` | Sends a DELETE request. |
| `patch(url, opts?)` | Sends a PATCH request. |
| `head(url, opts?)` | Sends a HEAD request. |

### The `opts` dictionary
You can pass an optional dictionary to any request method:
- `headers`: A dictionary of HTTP headers.
- `json`: A **string** representing JSON data (automatically sets Content-Type).
- `timeout`: A number (milliseconds) for the request timeout.

## The Response Dictionary
When a request completes, it returns a dictionary with these fields and methods:

| Field/Method | Type | Description |
|--------------|------|-------------|
| `status` | `int` | The HTTP status code (e.g., 200, 404). |
| `ok` | `bool` | `true` if the status is between 200 and 299. |
| `headers` | `dict` | A dictionary of response headers. |
| `text()` | Method | Returns the response body as a string. |
| `json()` | Method | Parses the response body as JSON and returns a dict/list. |

```zen
var resp = http.get("https://api.github.com")
print(resp.status)   # 200
print(resp.ok)       # true
print(resp.headers["Content-Type"])

var data = resp.json()
print(data.repository_url)
```

## Error Handling
Requests that fail due to network issues (DNS, connection refused, etc.) will throw an error.

```zen
try {
    http.get("http://invalid-domain-name.foo")
} catch as e {
    print("Network error: ${e}")
}
```

## See Also
- [json](json.md) — For encoding/decoding JSON.
- [urllib](urllib.md) — For low-level URL manipulation.
