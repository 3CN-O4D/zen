# HTTP Module

Complete reference for making HTTP requests in Zen — GET, POST, PUT, DELETE, custom headers, JSON, timeouts, and streaming.

## Quick Start

```
// GET request
let resp = http.get("https://httpbin.org/get")
print resp.status       // 200
print resp.body         // response body as string
print resp.ok           // true (2xx status)
```

---

## Making Requests

### GET

```
let resp = http.get("https://api.github.com/repos/rust-lang/rust")
print resp.status       // 200
let data = resp.json()
print data.name         // rust
print data.stars        // number of stars
```

### POST with JSON body

```
let resp = http.post("https://httpbin.org/post",
    json={"name": "Zen", "version": "1.0"})
print resp.status       // 200
let data = resp.json()
print data["json"]["name"]    // Zen
```

### POST with form data

```
let resp = http.post("https://httpbin.org/post",
    data="key=value&foo=bar")
print resp.status       // 200
```

### PUT

```
let resp = http.put("https://api.example.com/users/1",
    json={"name": "Updated Name"})
print resp.status       // 200
```

### DELETE

```
let resp = http.del("https://api.example.com/users/1")
print resp.status       // 200
```

### HEAD

```
let resp = http.head("https://example.com")
print resp.status       // 200
print resp.headers      // dict of response headers
```

### PATCH

```
let resp = http.patch("https://api.example.com/users/1",
    json={"name": "Patched"})
print resp.status       // 200
```

---

## Request Options

### Custom headers

```
let resp = http.get("https://api.github.com/user",
    headers={
        "Authorization": "Bearer YOUR_TOKEN",
        "Accept": "application/json",
        "X-Custom-Header": "value"
    })
print resp.status       // 200
```

### Timeout (seconds)

```
let resp = http.get("https://slow-api.example.com", timeout=30)
// Times out after 30 seconds
```

### Combining options

```
let resp = http.post("https://api.example.com/data",
    json={"key": "value"},
    headers={"Authorization": "Bearer token123"},
    timeout=10)
print resp.status
```

---

## Response Object

### Properties

| Property | Type | Description |
|----------|------|-------------|
| `.status` | number | HTTP status code (200, 404, etc.) |
| `.body` | string | Response body as string |
| `.headers` | dict | Response headers |
| `.ok` | bool | `true` if status 200–399 |

### Methods

| Method | Description |
|--------|-------------|
| `.json()` | Parse body as JSON |

### Status code checking

```
let resp = http.get("https://api.example.com/data")

if resp.ok {
    let data = resp.json()
    print "Success: {data}"
} else {
    print "Error: {resp.status}"
    print "Body: {resp.body}"
}
```

### Accessing headers

```
let resp = http.get("https://example.com")

print resp.headers["content-type"]    // text/html; charset=utf-8
print resp.headers["server"]          // nginx
```

---

## JSON Requests and Responses

### Sending JSON

```
let payload = {
    "name": "Alice",
    "email": "alice@example.com",
    "scores": [95, 87, 92]
}

let resp = http.post("https://api.example.com/users",
    json=payload)
```

### Receiving JSON

```
let resp = http.get("https://api.github.com/repos/rust-lang/rust")
let repo = resp.json()

print repo.name           // rust
print repo.description    // "Empowering everyone to build reliable..."
print repo.stars          // star count
```

### Chaining JSON operations

```
let users = http.get("https://api.example.com/users").json()

let active_names = []
for user in users {
    if user["active"] {
        active_names.append(user["name"])
    }
}

print "Active users: " + json.encode(active_names)
```

---

## Common Patterns

### API authentication

```
// Bearer token
let resp = http.get("https://api.example.com/me",
    headers={"Authorization": "Bearer " + token})

// Basic auth
let resp = http.get("https://api.example.com/me",
    headers={"Authorization": "Basic " + base64.encode("user:pass")})
```

### File download

```
let resp = http.get("https://example.com/file.txt")
if resp.ok {
    fs.write("downloaded.txt", resp.body)
    print "Downloaded {resp.body.len} bytes"
} else {
    print "Download failed: {resp.status}"
}
```

### Retry with exponential backoff

```
function fetch_with_retry(url, max_attempts) {
    let attempt = 0
    while attempt < max_attempts {
        attempt = attempt + 1
        try {
            let resp = http.get(url, timeout=5)
            if resp.ok {
                return resp.json()
            }
            print "HTTP {resp.status}, retrying..."
        } catch err {
            print "Request failed: {err}, retrying..."
        }
        sleep(2 ** attempt)    // 2, 4, 8, 16 seconds
    }
    throw "Failed after {max_attempts} attempts"
}

let data = fetch_with_retry("https://api.example.com/data", 3)
```

### Paginated API

```
function fetch_all_pages(base_url) {
    let all_items = []
    let page = 1

    while true {
        let resp = http.get("{base_url}?page={page}")
        if !resp.ok { break }

        let data = resp.json()
        let items = data["items"]

        if items.len == 0 { break }

        for item in items {
            all_items.append(item)
        }

        page = page + 1
    }

    return all_items
}

let items = fetch_all_pages("https://api.example.com/items")
print "Fetched {items.len} total items"
```

### POST with form data

```
let resp = http.post("https://httpbin.org/post",
    data="username=admin&password=secret",
    headers={"Content-Type": "application/x-www-form-urlencoded"})
print resp.status    // 200
```

---

## Error Handling

### Network errors

```
try {
    let resp = http.get("https://nonexistent.invalid")
} catch err {
    print "Network error: " + err
}
```

### HTTP errors

```
let resp = http.get("https://api.example.com/missing")

if resp.status == 404 {
    print "Not found"
} elif resp.status == 401 {
    print "Unauthorized"
} elif resp.status == 500 {
    print "Server error"
} elif !resp.ok {
    print "HTTP error: {resp.status}"
}
```

### Timeout handling

```
try {
    let resp = http.get("https://slow-api.example.com", timeout=5)
    let data = resp.json()
} catch err {
    print "Request timed out or failed: " + err
}
```

---

## Pro Tips

1. **Always check `resp.ok` before using the response.** Not all HTTP errors throw.
2. **Use `timeout` to prevent hanging.** Default timeout may be very long.
3. **Use `resp.json()` for API responses.** It parses JSON automatically.
4. **Cache responses when possible.** HTTP requests are expensive.
5. **Use `http.head()` for existence checks.** Lighter than a full GET.
6. **Set `Content-Type` for POST/PUT.** APIs may reject requests without it.

---

## Common Mistakes

### Not checking status

```
// BAD — assumes success
let resp = http.get("https://api.example.com/data")
let data = resp.json()    // may fail if status is 404/500

// GOOD — check first
let resp = http.get("https://api.example.com/data")
if resp.ok {
    let data = resp.json()
} else {
    print "Error: {resp.status}"
}
```

### Missing headers

```
// BAD — API may reject
http.post("https://api.example.com/data",
    json={"key": "value"})

// GOOD — set Content-Type
http.post("https://api.example.com/data",
    json={"key": "value"},
    headers={"Content-Type": "application/json"})
```

### Not handling timeouts

```
// BAD — may hang forever
let resp = http.get("https://slow-api.example.com")

// GOOD — set a timeout
let resp = http.get("https://slow-api.example.com", timeout=10)
```

---

## See Also

- [json Module](json.md) — JSON parsing and encoding
- [fs Module](fs.md) — Saving responses to files
- [crypto Module](crypto.md) — Authentication tokens
- [Module Overview](overview.md) — All available modules
