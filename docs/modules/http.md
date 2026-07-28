# HTTP Requests (http)

The `http` module provides lightweight HTTP requests using Python's standard library (`urllib`). No external dependencies.

## GET

```
let resp = http.get("https://api.github.com/repos/zen/zen")
print resp.status       // 200
print resp.json()["name"]  // parsed JSON response
```

## POST

```
let resp = http.post("https://httpbin.org/post",
    json={"name": "Zen", "version": "0.1.0"})
```

## PUT

```
let resp = http.put("https://api.example.com/update", json={"key": "value"})
```

## DELETE

```
let resp = http.del("https://api.example.com/delete")
```

## HEAD

```
let resp = http.head("https://example.com")
```

## PATCH

```
let resp = http.patch("https://api.example.com/patch", data="partial")
```

## Custom Headers

```
let resp = http.get("https://api.example.com",
    headers={"Authorization": "Bearer token123"})
```

## Timeout

```
let resp = http.get("https://slow.example.com", timeout=10)
// times out after 10 seconds
```

## Response Object

| Property | Description |
|----------|-------------|
| `.status` | HTTP status code (e.g. 200, 404) |
| `.body` | Response body as string |
| `.headers` | Dict of response headers |
| `.ok` | `true` if status 200-399 |
| `.json()` | Parse body as JSON |
