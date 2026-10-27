# Chapter 77: Reference Implementation — Static File HTTP Server

A minimal HTTP server that serves files from a directory (`public/` by default).
It handles `GET` requests, maps URLs to files, sets `Content-Type` by extension,
and returns 404 for missing files. Demonstrates `http.server`, file I/O, MIME
types, and path safety.

## 77.1 The program (`static.z`)

```zen
import http.server
import pathlib
import os
import sys

let ROOT = "public"

func mime_type(path) {
    let ext = pathlib.Path(path).suffix.lower()
    return #{".html": "text/html", ".css": "text/css", ".js": "application/javascript",
             ".json": "application/json", ".png": "image/png", ".jpg": "image/jpeg",
             ".gif": "image/gif", ".svg": "image/svg+xml", ".txt": "text/plain",
             ".ico": "image/x-icon"}[ext] ?? "application/octet-stream"
}

func safe_join(root, url_path) {
    # prevent path traversal: reject .. and absolute paths
    if url_path == "/" { url_path = "/index.html" }
    if url_path.contains("..") or url_path.startsWith("/") {
        return nil
    }
    return pathlib.Path(root) / url_path[1:]   # strip leading /
}

func handle(req) {
    let method = req.method ?? "GET"
    if method != "GET" {
        return #(405, #{}, "method not allowed")
    }
    let url_path = req.path ?? "/"
    let fs_path = safe_join(ROOT, url_path)
    if fs_path == nil {
        return #(400, #{}, "bad request")
    }
    let p = pathlib.Path(str(fs_path))
    if !p.exists() or !p.is_file() {
        return #(404, #{}, "not found")
    }
    try {
        let data = read_all_bytes(str(p))
        return #(200, #{"Content-Type": mime_type(str(p))}, data)
    } catch as e {
        return #(500, #{}, "error reading file: " + e.msg)
    }
}

func read_all_bytes(path) {
    with open(path, "rb") as f {
        let parts = []
        while true {
            let chunk = f.read(8192)
            if chunk == nil or len(chunk) == 0 { break }
            parts.push(chunk)
        }
        return parts.join("")
    }
}

func main(argv) {
    let port = argv[1] ?? "8080"
    if len(argv) > 1 and argv[1] != "" and argv[1] != "--help" {
        ROOT = argv[1]
    }
    let server = http.server.Server(port=int(port), handler=handle)
    print("serving " + ROOT + " on http://localhost:" + str(port))
    server.start()
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 77.2 Example session

```
$ mkdir -p public
$ echo '<h1>Hello</h1>' > public/index.html
$ zen run static.z 8080
serving public on http://localhost:8080
```

```bash
$ curl -i http://localhost:8080/
HTTP/1.1 200 OK
Content-Type: text/html

<h1>Hello</h1>

$ curl -i http://localhost:8080/missing.html
HTTP/1.1 404 Not Found
```

## 77.3 Design notes

- **Path safety:** `safe_join` rejects `..` and absolute paths to prevent
  directory traversal attacks (e.g., `GET /../../../etc/passwd`).
- **MIME types:** content type is chosen by file extension; unknown extensions
  default to `application/octet-stream`.
- **Root directory:** configurable via the first argument; defaults to `public/`.
- **Index file:** requesting `/` serves `index.html`.

## 77.4 Exercises

1. Add directory listing (auto-index) when the path is a folder.
2. Add `HEAD` support (headers only, no body).
3. Add `Range` requests for resuming downloads.
4. Add access logging (log each request with IP, path, status).
5. Add `--root` flag instead of positional argument.
