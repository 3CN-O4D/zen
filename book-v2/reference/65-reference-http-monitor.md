# Chapter 65: Reference Implementation — HTTP Uptime Monitor

A small uptime monitor that periodically checks a list of HTTP endpoints,
records whether each is up, measures response time, and prints a status table.
It demonstrates the `requests` module, time, threading (periodic checks), and
CLI flags.

## 65.1 The program (`monitor.z`)

```zen
import requests
import time
import sys

# ── Configuration ───────────────────────────────────────────────────────────
# endpoints: list of #{name, url, interval_sec}
var endpoints = #{}
let checks_mu = nil  # placeholder if shared state needed

func check_endpoint(name, url) {
    let start = time.time()
    let result = #{up: false, status: nil, ms: nil, error: nil}
    try {
        let r = requests.get(url, timeout=5000)  # 5 second timeout
        let elapsed_ms = (time.time() - start) * 1000
        result["up"] = r.status_code >= 200 and r.status_code < 400
        result["status"] = r.status_code
        result["ms"] = elapsed_ms
    } catch as e {
        result["up"] = false
        result["error"] = e.msg
    }
    return result
}

func fmt_ms(ms) {
    if ms == nil { return "  --" }
    return str(math.round(ms)) + " ms"
}

func print_table(results) {
    print("HTTP Uptime Monitor")
    print("==================")
    print(" NAME        URL                            UP   STATUS   TIME")
    print(" ----        ---                            --   ------   ----")
    # sort by name for stable output
    let names = []
    for n in results { names.push(n) }
    names.sort()
    for n in names {
        let r = results[n]
        let up_str = if r["up"] { "YES" } else { "NO" }
        let status_str = if r["status"] == nil { "-" } else { str(r["status"]) }
        print(" " + n + "          " + r["url"] + "  " + up_str + "   " + status_str + "    " + fmt_ms(r["ms"]))
    }
}

func run_once(urls) {
    let results = #{}
    for entry in urls {
        let name = entry["name"]
        let url = entry["url"]
        results[name] = check_endpoint(name, url)
    }
    print_table(results)
    return results
}

func run_loop(urls, interval_sec) {
    print("monitoring " + str(len(urls)) + " endpoint(s) every " + str(interval_sec) + "s")
    print("press Ctrl-C to stop")
    while true {
        run_once(urls)
        time.sleep(interval_sec * 1000)
        print("---- next round ----")
    }
}

func main(argv) {
    # minimal arg parsing: URLs as arguments, --interval N optional
    let urls = []
    var interval_sec = 30
    var i = 1
    while i < len(argv) {
        let a = argv[i]
        if a == "--interval" and i + 1 < len(argv) {
            interval_sec = int(argv[i + 1])
            i += 2
        } else {
            urls.push(#{name: "endpoint" + str(len(urls) + 1), url: a})
            i += 1
        }
    }

    if len(urls) == 0 {
        print("Usage: zen run monitor.z [--interval N] <url> [<url> ...]")
        print("  Example: zen run monitor.z https://example.com https://api.example.com/health --interval 10")
        return 1
    }

    run_loop(urls, interval_sec)
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 65.2 Example session

```
$ zen run monitor.z --interval 10 https://example.com https://httpbin.org/status/200 https://httpbin.org/status/500
monitoring 3 endpoint(s) every 30s
press Ctrl-C to stop
HTTP Uptime Monitor
==================
 NAME        URL                            UP   STATUS   TIME
 ----        ---                            --   ------   ----
 endpoint1   https://example.com            YES   200      45 ms
 endpoint2   https://httpbin.org/status/200 YES   200      120 ms
 endpoint3   https://httpbin.org/status/500 NO    500      -- 
---- next round ----
```

## 65.3 Design notes

- **Timeouts on every request:** a 5-second timeout prevents a hung endpoint
  from blocking the whole monitor loop.
- **Up/down semantics:** status 2xx–3xx counts as "up"; 4xx/5xx and exceptions
  count as "down".
- **Response time** is measured client-side (request start to response complete).
- **Interval** between rounds is configurable with `--interval`.

## 65.4 Exercises

1. Add `--output <file>` to write results as JSON.
2. Add alerting: print "ALERT" when a previously-up endpoint goes down.
3. Add average response time tracking over the last N rounds.
4. Add concurrent checks (check all endpoints in parallel threads).
5. Add a `--once` flag to run a single check and exit.
