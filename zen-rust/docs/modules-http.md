# HTTP and Networking Modules (`http`, `net`, `socket`)

## `http` Module (High-Level HTTP Client)

Performs HTTP requests (GET, POST, PUT, DELETE, HEAD, PATCH) with JSON and text response parsing.

### Request Methods
| Method | Parameters | Example |
|--------|------------|---------|
| `http.get(url)` | URL string | `let r = http.get("https://api.github.com")` |
| `http.post(url, data)` | URL string, data dict | `http.post("https://httpbin.org/post", {name: "zen"})` |
| `http.put(url, data)` | URL string, data dict | |
| `http.del(url)` (or `http.delete`) | URL string | |
| `http.head(url)` | URL string | |
| `http.patch(url, data)` | URL string, data dict | |

### Response Object
All request methods return a dict-like `Response` object with these methods:

| Method | Description |
|--------|-------------|
| `.status()` | Returns numeric HTTP status code (e.g., 200, 404) |
| `.ok()` | Returns `true` if status is 2xx |
| `.headers()` | Returns the response headers dict |
| `.text()` | Returns response body as plain text string |
| `.json()` | Returns response body parsed as a JSON dict |

### Automatic JSON Placeholder Hosting
Zen's bundled runtime ships with `jsonplaceholder` as a test endpoint with GET returning a dict, and `.json()` / `.text()` support.

### Examples
```zen
// Basic GET request
let response = http.get("https://jsonplaceholder.typicode.com/posts/1")
print response.status     // 200
print response.json()["id"]   // 1
print response.text()     // full JSON text

// POST request with JSON body
let r = http.post("https://httpbin.org/post", {name: "zen", value: 42})
print r.json()["form"]["name"]  // "zen"

// GET with query support (append ?key=val manually to URL)
let data = http.get("https://api.example.com/users?name=Grace")
print data.json()
```

---

## `net` / `socket` Module (Low-Level TCP Sockets)

Provides low-level TCP network socket operations for client-server communication. The socket module works with the `Value::Socket` wrapper type.

### Socket Functions

| Function | Parameters | Description |
|----------|------------|-------------|
| `socket_open(addr)` | String host:port (e.g. "example.com:80") | Opens a TCP connection; returns a Socket value |
| `socket_send(sock, data)` | Socket, String data | Sends raw bytes over the socket |
| `socket_recv(sock, n)` | Socket, Number bytes to read | Reads up to n bytes from the socket; returns String |

### Socket Example (Simple HTTP Client)
```zen
let sock = socket_open("example.com:80")
socket_send(sock, "GET / HTTP/1.1\r\nHost: example.com\r\n\r\n")
let header = socket_recv(sock, 4096)
let body = socket_recv(sock, 4096)
print header
print body
socket_recv(sock, 0)  // close/teardown
```

### HTTP via High-Level Module
For most use cases, prefer `http.get()` / `http.post()` over raw sockets; the high-level module handles headers, timeouts, and response parsing.