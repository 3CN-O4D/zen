# Chapter 30: http Server Module

The `http.server` module provides a simple static file HTTP server. It's useful for serving files, directory listings, and quick local servers.

## 30.1 Starting a server

```zen
import http.server

// Serve current directory on port 8000
http.server.serve(".", 8000)
```

This blocks — the server runs until the process is interrupted.

## 30.2 Server options

```zen
import http.server

// Serve a specific directory on a specific host/port
http.server.serve("/var/www", 8080, "127.0.0.1")

// Serve once (single request) then return
let port = http.server.serve_once(".", 0)
print("served on port " + str(port))
```

### `serve(root, port, host?)`

Blocks forever serving files from `root` on `port:host`.

### `serve_once(root, port?)`

Serves a single request then returns the port used. Port 0 means "pick a free port."

## 30.3 What it serves

- **Files** — serves the file with appropriate `Content-Type` based on extension.
- **Directories** — generates an HTML directory listing with links.
- **MIME types** — common extensions mapped (`.html`, `.css`, `.js`, `.json`, `.png`, `.jpg`, `.txt`, etc.).
- **404** — returns a 404 page for missing files.

## 30.4 Example: serve a build directory

```zen
import http.server

print("Serving build/ on http://localhost:8000")
http.server.serve("build", 8000)
```

Then open `http://localhost:8000` in a browser.

## 30.5 Example: serve once and get the port

```zen
import http.server

// Port 0 means "pick a free port"
let port = http.server.serve_once(".", 0)
print("One-shot server on port " + str(port))
```

## 30.6 MIME type guessing

```zen
import http.server

print(http.server.guess_type("index.html"))   // text/html
print(http.server.guess_type("style.css"))    // text/css
print(http.server.guess_type("app.js"))       // application/javascript
print(http.server.guess_type("data.json"))    // application/json
print(http.server.guess_type("image.png"))    // image/png
```

### Supported MIME types

| Extension | MIME type |
|-----------|-----------|
| `.html` | `text/html` |
| `.css` | `text/css` |
| `.js` | `application/javascript` |
| `.json` | `application/json` |
| `.txt` | `text/plain` |
| `.png` | `image/png` |
| `.jpg`, `.jpeg` | `image/jpeg` |
| `.gif` | `image/gif` |
| `.svg` | `image/svg+xml` |
| `.pdf` | `application/pdf` |
| `.mp3` | `audio/mpeg` |
| `.mp4` | `video/mp4` |
| `.zip` | `application/zip` |
| `.tar.gz` | `application/gzip` |
| `.woff2` | `font/woff2` |
| `.ico` | `image/x-icon` |
| (unknown) | `application/octet-stream` |

## 30.7 Common pitfalls

- **`serve` blocks.** Run it in a thread if you need to do other work.
- **Directory traversal is blocked.** `safe_join` prevents `..` escapes.
- **No HTTPS.** Use `tls` or a reverse proxy for production.
- **Single-threaded.** One request at a time.

## 30.8 Directory traversal protection

The server rejects paths containing `..` to prevent accessing files outside the root directory:

```zen
// Requesting GET /../../etc/passwd returns 403 or 404
```

## 30.9 Quick reference

```zen
import http.server

http.server.serve(root, port, host?)
let port = http.server.serve_once(root, port?)
http.server.guess_type(filename)
```

## 30.10 Exercises

1. Serve a local directory and open it in a browser.
2. Use `serve_once` to get a free port, then make a request to it.
3. Create an HTML file, serve it, and verify the `Content-Type` header.
4. Write a script that serves a directory in a thread while doing other work.
5. Test that `..` in a URL path is rejected (directory traversal protection).


## 30.10 Building a more complete server

### Routing
```zen
# simple router pattern
fn handle(req) {
    if req.path == "/" { return "index" }
    if req.path == "/api/data" { return json.dumps({"ok": true}) }
    return "not found"
}
```

### Serving static files
Read file, set Content-Type based on extension, return bytes/text.

### Handling POST data
Parse form data or JSON from body based on Content-Type.

### Middleware pattern
```zen
fn middleware(next_handler) {
    return fn(req) {
        print(req.method + " " + req.path)
        return next_handler(req)
    }
}
```

### CORS headers
```zen
# Add headers: Access-Control-Allow-Origin, etc.
```

### Error responses
Return proper status codes and JSON error bodies.

## 30.11 Production considerations
- Use proper server (gunicorn-like) not just dev server
- Handle concurrent requests (threading/async)
- Set timeouts
- Validate input
- Add logging
- Use HTTPS
- Rate limiting
- Security headers

## 30.12 Exercises
1. Multi-route server
2. JSON API endpoints (GET/POST)
3. Serve static HTML/CSS/JS
4. Form handling
5. 404 and error pages
6. Request logging middleware
7. Simple REST CRUD API


## 30.12 Worked example — a small HTTP server with routing

```zen
import http.server

let routes = #{}

func route(path, handler) {
    routes[path] = handler
}

func not_found(req) {
    return #(404, #{}, "Not Found")
}

func handle(req) {
    let h = routes[req.path] ?? not_found
    return h(req)
}

route("/", fn(req) {
    return #(200, #{}, "<h1>Zen HTTP Server</h1>")
})

route("/time", fn(req) {
    import datetime
    return #(200, #{}, "time=" + datetime.now())
})

route("/echo", fn(req) {
    return #(200, {"Content-Type": "text/plain"}, req.body ?? "")
})

let server = http.server.Server(port=8080, handler=handle)
print("server on http://localhost:8080")
server.start()
```

Test with curl:
```bash
$ curl http://localhost:8080/
<h1>Zen HTTP Server</h1>
$ curl http://localhost:8080/time
time=...
$ curl -d "hello" http://localhost:8080/echo
hello
```

## 30.13 Troubleshooting HTTP servers

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| connection refused | server not started / wrong port | start server, check port |
| port already in use | another process on 8080 | use SO_REUSEADDR or a different port |
| 404 on known route | path mismatch (trailing slash) | normalize paths |
| client hangs | handler never returns | check for blocking code |
| large payload truncated | read limit | read full body / stream |

## 30.14 Exercises

1. Add a `/health` route returning `{status: "ok"}`.
2. Return JSON from a route (`Content-Type: application/json`).
3. Add request logging (method + path) for every request.
4. Handle POST body parsing for `/echo`.
5. Add a 500 error route handler for exceptions.
6. Make the port configurable via an argument/env var.
