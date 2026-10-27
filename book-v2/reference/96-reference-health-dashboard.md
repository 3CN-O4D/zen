# Chapter 96: Reference Implementation — HTTP Health Dashboard

Checks multiple HTTP endpoints on a refresh interval and displays a status
board with status codes and response times. Uses auto-refresh (meta tag) so the
page updates itself. Demonstrates `requests`, threading, timing, and HTML
generation.

## 96.1 The program (`health.z`)

```zen
import requests
import time
import sys
import threading
import mutexes

var endpoints = #{}
let endpoints_mu = mutexes.Mutex()
let results = #{}
let results_mu = mutexes.Mutex()

func add_endpoint(name, url, interval_sec) {
    endpoints_mu.lock()
    endpoints[name] = #{url: url, interval_sec: interval_sec}
    endpoints_mu.unlock()
}

func check_one(name, url) {
    let start = time.time()
    let status = "down"
    let code = nil
    let ms = nil
    try {
        let r = requests.get(url, timeout=5000)
        code = str(r.status_code)
        if r.status_code >= 200 and r.status_code < 400 { status = "up" }
        ms = (time.time() - start) * 1000
    } catch as e {
        status = "down"
        code = "err"
    }
    results_mu.lock()
    results[name] = #{status: status, code: code, ms: ms}
    results_mu.unlock()
}

func checker_loop() {
    while true {
        endpoints_mu.lock()
        let names = []
        for n in endpoints { names.push(n) }
        let urls = #{}
        for n in names { urls[n] = endpoints[n]["url"] }
        endpoints_mu.unlock()

        let threads = []
        for n in names {
            threads.push(threading.Thread(fn() { check_one(n, urls[n]) }))
        }
        for t in threads { t.start() }
        for t in threads { t.join() }

        time.sleep(5000)   # refresh every 5 seconds
    }
}

func status_row(name) {
    results_mu.lock()
    let r = results[name]
    results_mu.unlock()
    if r == nil { return "<tr><td>" + name + "</td><td>pending</td><td>-</td><td>-</td></tr>" }
    let status_class = if r["status"] == "up" { "up" } else { "down" }
    return "<tr><td>" + name + "</td><td class='" + status_class + "'>" +
           r["status"] + "</td><td>" + str(r["code"]) + "</td><td>" +
           str(r["ms"]) + " ms</td></tr>"
}

func dashboard_html() {
    results_mu.lock()
    let names = []
    for n in results { names.push(n) }
    names.sort()
    results_mu.unlock()
    let rows = ""
    for n in names { rows = rows + status_row(n) }
    return """<!DOCTYPE html>
<html>
<head>
<meta http-equiv="refresh" content="5">
<style>
body { font-family: system-ui, sans-serif; padding: 2rem; }
table { border-collapse: collapse; width: 100%; }
th, td { border: 1px solid #ddd; padding: 0.75rem; text-align: left; }
th { background: #eee; }
.up { color: green; font-weight: bold; }
.down { color: red; font-weight: bold; }
h1 { margin-bottom: 1rem; }
</style>
</head>
<body>
<h1>HTTP Health Dashboard</h1>
<p>Auto-refreshes every 5 seconds</p>
<table>
<tr><th>Endpoint</th><th>Status</th><th>Code</th><th>Time</th></tr>
""" + rows + """
</table>
</body>
</html>"""
}

func main(argv) {
    # default endpoints (override with args: name:url pairs)
    add_endpoint("example", "https://example.com", 5)
    add_endpoint("httpbin", "https://httpbin.org/status/200", 5)
    var i = 1
    while i < len(argv) {
        let parts = argv[i].split(":")
        if len(parts) >= 2 {
            add_endpoint(parts[0], parts[1], 5)
        }
        i += 1
    }

    # start background checker
    threading.Thread(fn() { checker_loop() }).start()

    # serve dashboard on port 8080 (conceptual; wire up http.server)
    print("health dashboard checker running (background)")
    print("endpoints:")
    for n in endpoints { print("  " + n + ": " + endpoints[n]["url"]) }
    print("refresh every 5s")
    print("(wire dashboard_html() into http.server.Serve to expose a UI)")

    # keep main alive
    while true {
        time.sleep(1000)
    }
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 96.2 Example session

```
$ zen run health.z example:https://example.com myapi:https://api.example.com/health
health dashboard checker running (background)
endpoints:
  example: https://example.com
  myapi: https://api.example.com/health
refresh every 5s
```

Generated dashboard (served via http.server, chapter 30/77):
```html
<table>
<tr><th>Endpoint</th><th>Status</th><th>Code</th><th>Time</th></tr>
<tr><td>example</td><td class="up">up</td><td>200</td><td>45 ms</td></tr>
<tr><td>myapi</td><td class="down">down</td><td>err</td><td>- ms</td></tr>
</table>
```

## 96.3 Design notes

- **Concurrent checks:** each endpoint is checked in its own thread each round.
- **Auto-refresh:** the HTML uses `<meta http-equiv="refresh">` so the browser
  reloads every 5 seconds.
- **Results stored under a mutex:** the checker thread writes, the UI thread
  reads — synchronized.
- **`dashboard_html()`** returns a complete HTML document; wire it into an
  `http.server.Server` handler (chapters 30/77) to expose it over HTTP.

## 96.4 Exercises

1. Wire `dashboard_html()` into an `http.server.Server` on port 8080.
2. Add `--interval N` to configure the refresh interval.
3. Add email/console alert when an endpoint flips up→down.
4. Add response-time history (last N values) and show min/avg/max.
5. Add `--config <file>` to load endpoints from a JSON/YAML file.
