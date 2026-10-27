# Chapter 98: Reference Implementation — Personal API Dashboard (Full-Stack)

A complete personal dashboard: a single Zen program that (1) serves static files
from a `public/` directory, (2) exposes a JSON health API that checks configured
endpoints, and (3) serves a dashboard HTML page that auto-refreshes. It combines
the static file server (chapter 77), HTTP health checks (chapter 65/96), and
JSON API patterns (chapter 71/89) into one cohesive application.

## 98.1 The program (`dashboard.z`)

```zen
import http.server
import json
import requests
import mutexes
import pathlib
import os
import sys
import time
import threading
import math

# ── Configuration ────────────────────────────────────────────────────────────
let STATIC_DIR = "public"
let PORT = 8080
let CHECK_INTERVAL_SEC = 30

# Health check results: name => #{status, code, ms, checked_at}
var health = #{}
let health_mu = mutexes.Mutex()

let default_endpoints = #{
    example: "https://example.com",
    httpbin: "https://httpbin.org/status/200",
}

var endpoints = #{}
let endpoints_mu = mutexes.Mutex()

# ── Health checking ──────────────────────────────────────────────────────────
func check_one(name, url) {
    let start = time.time()
    let result = #{status: "down", code: nil, ms: nil}
    try {
        let r = requests.get(url, timeout=5000)
        result["status"] = if r.status_code >= 200 and r.status_code < 400
                              { "up" } else { "down" }
        result["code"] = str(r.status_code)
        result["ms"] = (time.time() - start) * 1000
    } catch as e {
        result["status"] = "down"
        result["code"] = "err"
    }
    result["checked_at"] = time.time()
    health_mu.lock()
    health[name] = result
    health_mu.unlock()
}

func check_all() {
    endpoints_mu.lock()
    let names = []
    for n in endpoints { names.push(n) }
    let urls = #{}
    for n in names { urls[n] = endpoints[n] }
    endpoints_mu.unlock()

    let threads = []
    for n in names {
        threads.push(threading.Thread(fn() { check_one(n, urls[n]) }))
    }
    for t in threads { t.start() }
    for t in threads { t.join() }
}

# ── HTTP handlers ────────────────────────────────────────────────────────────
func health_api(req) {
    health_mu.lock()
    let data = #{}
    for n in health { data[n] = health[n] }
    health_mu.unlock()
    return #(200, #{"Content-Type": "application/json"}, json.stringify(data))
}

func dashboard_page(req) {
    health_mu.lock()
    let names = []
    for n in health { names.push(n) }
    names.sort()
    health_mu.unlock()
    let rows = ""
    for n in names {
        health_mu.lock()
        let r = health[n]
        health_mu.unlock()
        let status_class = if r != nil and r["status"] == "up" { "up" } else { "down" }
        let status_text = if r != nil then r["status"] else "pending"
        let code = if r != nil then str(r["code"]) else "-"
        let ms = if r != nil and r["ms"] != nil then str(math.round(r["ms"])) + " ms" else "-"
        rows = rows + "<tr><td>" + n + "</td><td class='" + status_class +
                   "'>" + status_text + "</td><td>" + code +
                   "</td><td>" + ms + "</td></tr>"
    }
    let html = """<!DOCTYPE html>
<html>
<head>
<meta http-equiv="refresh" content="15">
<style>
body { font-family: system-ui, sans-serif; padding: 2rem; max-width: 900px;
       margin: 0 auto; }
table { border-collapse: collapse; width: 100%; }
th, td { border: 1px solid #ddd; padding: 0.75rem; text-align: left; }
th { background: #eee; }
.up { color: green; font-weight: bold; }
.down { color: red; font-weight: bold; }
a { color: #0366d6; text-decoration: none; }
a:hover { text-decoration: underline; }
</style>
</head>
<body>
<h1>Personal Dashboard</h1>
<p><a href="/">files</a> | <a href="/api/health">health JSON</a> |
   <a href="/dashboard">dashboard</a> | auto-refreshes every 15s</p>
<table>
<tr><th>Endpoint</th><th>Status</th><th>Code</th><th>Time</th></tr>
""" + rows + """
</table>
</body>
</html>"""
    return #(200, #{"Content-Type": "text/html"}, html)
}

func serve_file(req) {
    let path = req.path ?? "/"
    if path == "/api/health" { return health_api(req) }
    if path == "/dashboard" { return dashboard_page(req) }
    if path == "/" { return #(302, #{"Location": "/dashboard"}, "") }

    let fs_path = pathlib.Path(STATIC_DIR) / path[1:]
    let p = pathlib.Path(str(fs_path))
    # path safety
    if path.contains("..") {
        return #(400, #{}, "bad request")
    }
    if p.exists() and p.is_file() {
        let data = read_all_bytes(str(p))
        return #(200, #{"Content-Type": mime_type(str(p))}, data)
    }
    return #(404, #{}, "not found")
}

func mime_type(path) {
    let ext = pathlib.Path(path).suffix.lower()
    return #{".html": "text/html", ".css": "text/css", ".js": "application/javascript",
             ".json": "application/json", ".png": "image/png", ".jpg": "image/jpeg",
             ".txt": "text/plain"}[ext] ?? "application/octet-stream"
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

# ── Background health loop ───────────────────────────────────────────────────
func health_loop() {
    while true {
        check_all()
        time.sleep(CHECK_INTERVAL_SEC * 1000)
    }
}

func main(argv) {
    if len(argv) > 1 { STATIC_DIR = argv[1] }
    if len(argv) > 2 { PORT = int(argv[2]) }

    # seed endpoints from defaults + args
    endpoints_mu.lock()
    for n in default_endpoints { endpoints[n] = default_endpoints[n] }
    var i = 3
    while i < len(argv) {
        let parts = argv[i].split(":")
        if len(parts) >= 2 { endpoints[parts[0]] = parts[1] }
        i += 1
    }
    endpoints_mu.unlock()

    # initial check
    check_all()

    # background loop
    threading.Thread(fn() { health_loop() }).start()

    let server = http.server.Server(port=PORT, handler=serve_file)
    print("dashboard on http://localhost:" + str(PORT))
    print("  GET /          -> dashboard (redirects to /dashboard)")
    print("  GET /dashboard -> health status board")
    print("  GET /api/health -> health JSON")
    print("  static files from " + STATIC_DIR)
    server.start()
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 98.2 Example session

```
$ mkdir -p public
$ echo '<h1>Welcome</h1>' > public/index.html
$ zen run dashboard.z public 8080
dashboard on http://localhost:8080
  GET /          -> dashboard (redirects to /dashboard)
  GET /dashboard -> health status board
  GET /api/health -> health JSON
  static files from public
```

```bash
$ curl http://localhost:8080/dashboard
<html>...<table>...example: up, 200, 45 ms...</table>...

$ curl http://localhost:8080/api/health
{"example":{"status":"up","code":"200","ms":45,...},...}

$ curl http://localhost:8080/
<h1>Welcome</h1>   # served from public/index.html
```

## 98.3 Design notes

- **Three concerns in one program:** static file serving, health API, and
  dashboard UI — showing how chapters 77, 71, and 96 compose.
- **Background health loop:** checks all endpoints every 30 seconds in a
  dedicated thread; results are stored under a mutex.
- **Auto-refresh dashboard:** the HTML uses `<meta http-equiv="refresh">`.
- **JSON API:** `/api/health` returns machine-readable results for scripts or
  other frontends.
- **Path safety:** `..` is rejected to prevent directory traversal.

## 98.4 Exercises

1. Add authentication (basic auth) to the health API.
2. Add endpoint management via the dashboard (add/remove endpoints at runtime).
3. Persist health history to a file and show uptime percentages.
4. Add email/console alerts when an endpoint flips down.
5. Add `--config <file>` to load endpoints and static dir from JSON.
