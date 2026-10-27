# Chapter 67: Reference Implementation — Website Link Checker

A link checker that fetches a page, extracts all links, and verifies each one
returns a successful status. It demonstrates the `requests` module, regex,
threading (parallel checks), and error handling. Intended for checking sites
you own or have permission to test.

## 67.1 The program (`links.z`)

```zen
import requests
import re
import threading
import mutexes
import sys
import time

# ── Extract links from HTML ─────────────────────────────────────────────────
let LINK_RE = r'href=["\']([^"\']+)["\']'

func extract_links(html) {
    let links = []
    for m in re.findall(LINK_RE, html) {
        links.push(m)
    }
    return links
}

func normalize_link(base, link) {
    # very crude normalization; real use needs a proper URL parser
    if link.startswith("http") { return link }
    if link.startswith("/") {
        # scheme + host from base
        let m = re.match(r"^https?://[^/]+", base)
        if m != nil { return m[0] + link }
    }
    return link
}

# ── Shared results ──────────────────────────────────────────────────────────
var results = #{}
let results_mu = mutexes.Mutex()

func check_link(url) {
    let status = "error"
    let code = nil
    try {
        let r = requests.get(url, timeout=5000)
        code = str(r.status_code)
        status = if r.status_code >= 200 and r.status_code < 400 { "ok" } else { "bad" }
    } catch as e {
        code = e.msg
    }
    results_mu.lock()
    results[url] = #{status: status, code: code}
    results_mu.unlock()
}

func main(argv) {
    if len(argv) < 2 {
        print("Usage: zen run links.z <url>")
        print("  Example: zen run links.z https://example.com")
        return 1
    }
    let base = argv[1]

    print("fetching " + base + " ...")
    let html = nil
    try {
        let r = requests.get(base, timeout=10000)
        html = r.text
    } catch as e {
        print("failed to fetch " + base + ": " + e.msg)
        return 1
    }

    let links = extract_links(html)
    print("found " + str(len(links)) + " link(s); checking...")

    let threads = []
    for link in links {
        let url = normalize_link(base, link)
        threads.push(threading.Thread(fn() { check_link(url) }))
    }
    for t in threads { t.start() }
    for t in threads { t.join() }

    let ok_count = 0
    let bad_count = 0
    let error_count = 0
    let sorted_urls = []
    for u in results { sorted_urls.push(u) }
    sorted_urls.sort()
    for u in sorted_urls {
        let r = results[u]
        let mark = if r["status"] == "ok" { "✓" } else { "✗" }
        print(mark + " " + str(r["code"]) + "  " + u)
        if r["status"] == "ok" { ok_count += 1 }
        elif r["status"] == "bad" { bad_count += 1 }
        else { error_count += 1 }
    }
    print("----")
    print("ok: " + str(ok_count) + "  bad: " + str(bad_count) + "  errors: " + str(error_count))
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 67.2 Example session

```
$ zen run links.z https://example.com
fetching https://example.com ...
found 3 link(s); checking...
✓ 200  https://example.com/
✓ 200  https://www.iana.org/domains/example
✗ 404  https://example.com/missing
----
ok: 2  bad: 1  errors: 0
```

## 67.3 Design notes

- **Parallel checks:** each link is checked in its own thread so slow or dead
  links don't block the rest.
- **Timeouts:** 10 seconds to fetch the page, 5 seconds per link check.
- **Crude normalization:** relative URLs are resolved against the base scheme +
  host; real usage needs a proper URL parser (see `url` module).
- **Ethical use:** only check sites you own or have permission to test; don't
  hammer a site with thousands of concurrent requests.

## 67.4 Exercises

1. Add `--concurrency N` to limit simultaneous checks.
2. Add `--output <file>` to write results as JSON or CSV.
3. Add recursion: follow links on the same domain up to a depth limit.
4. Add `User-Agent` header identifying your tool.
5. Add `--exclude <pattern>` to skip links matching a regex.
