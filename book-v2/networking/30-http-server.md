# Chapter 30: http Server Module

The `http.server` module provides a simple static file HTTP server. It's
useful for serving files, directory listings, and quick local servers.

## 30.1 Starting a server

```zen
import http.server

# Serve current directory on port 8000
http.server.serve(".", 8000)
```

This blocks — the server runs until the process is interrupted.

## 30.2 Server options

```zen
import http.server

# Serve a specific directory on a specific host/port
http.server.serve("/var/www", 8080, "127.0.0.1")

# Serve once (single request) then return
let port = http.server.serve_once(".", 0)
print("served on port " + str(port))
```

## 30.3 What it serves

- **Files** — serves the file with appropriate `Content-Type` based on
  extension.
- **Directories** — generates an HTML directory listing with links.
- **MIME types** — common extensions mapped (`.html`, `.css`, `.js`, `.json`,
  `.png`, `.jpg`, `.txt`, etc.).
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

# Port 0 means "pick a free port"
let port = http.server.serve_once(".", 0)
print("One-shot server on port " + str(port))
```

## 30.6 MIME type guessing

```zen
import http.server

print(http.server.guess_type("index.html"))   # text/html
print(http.server.guess_type("style.css"))    # text/css
print(http.server.guess_type("app.js"))       # application/javascript
print(http.server.guess_type("data.json"))    # application/json
print(http.server.guess_type("image.png"))    # image/png
```

## 30.7 Common pitfalls

- **`serve` blocks.** Run it in a thread if you need to do other work.
- **Directory traversal is blocked.** `safe_join` prevents `..` escapes.
- **No HTTPS.** Use `tls` or a reverse proxy for production.
- **Single-threaded.** One request at a time.

## 30.8 Quick reference

```zen
import http.server

http.server.serve(root, port, host?)
let port = http.server.serve_once(root, port?)
http.server.guess_type(filename)
```

## 30.9 Exercises

1. Serve a local directory and open it in a browser.
2. Use `serve_once` to get a free port, then make a request to it.
3. Create an HTML file, serve it, and verify the `Content-Type` header.
4. Write a script that serves a directory in a thread while doing other work.
5. Test that `..` in a URL path is rejected (directory traversal protection).
