# Chapter 80: Reference Implementation — In-Memory URL Shortener

An HTTP URL shortener: `POST /shorten` with a JSON body `{url}` returns a short
code; `GET /<code>` redirects to the original URL. Codes are random base62
strings. Demonstrates `http.server`, `crypto` (secure random), JSON, and an
in-memory store.

## 80.1 The program (`shorten.z`)

```zen
import http.server
import json
import crypto
import mutexes
import sys

var urls = #{}
let urls_mu = mutexes.Mutex()

let CODE_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
let CODE_LEN = 6

func generate_code() {
    let code = ""
    var i = 0
    while i < CODE_LEN {
        let idx = crypto.int(0, len(CODE_CHARS) - 1)
        code = code + CODE_CHARS[idx]
        i += 1
    }
    return code
}

func shorten(long_url) {
    urls_mu.lock()
    # avoid collisions: regenerate if code exists
    var code = generate_code()
    var tries = 0
    while code in urls and tries < 100 {
        code = generate_code()
        tries += 1
    }
    urls[code] = long_url
    urls_mu.unlock()
    return code
}

func resolve(code) {
    urls_mu.lock()
    let url = urls[code]
    urls_mu.unlock()
    return url
}

func handle(req) {
    let method = req.method ?? "GET"
    let path = req.path ?? "/"

    if method == "POST" and path == "/shorten" {
        let body = req.body ?? ""
        let parsed = nil
        try { parsed = json.parse(body) } catch as e {}
        if parsed == nil or !("url" in parsed) {
            return #(400, #{"Content-Type": "application/json"},
                     json.stringify(#{error: "missing url"}))
        }
        let code = shorten(parsed["url"])
        let short = "http://localhost:8080/" + code
        return #(201, #{"Content-Type": "application/json"},
                 json.stringify(#{code: code, short: short}))
    }

    if method == "GET" and len(path) > 1 {
        let code = path[1:]   # strip leading /
        let url = resolve(code)
        if url == nil {
            return #(404, #{}, "not found")
        }
        # 302 redirect
        return #(302, #{"Location": url}, "")
    }

    if method == "GET" and path == "/" {
        return #(200, #{"Content-Type": "text/html"},
                 "<h1>URL Shortener</h1><p>POST /shorten with {url}</p>")
    }

    return #(404, #{}, "not found")
}

func main(argv) {
    let port = argv[1] ?? "8080"
    let server = http.server.Server(port=int(port), handler=handle)
    print("url shortener on http://localhost:" + str(port))
    server.start()
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 80.2 Example session

```
$ zen run shorten.z 8080
url shortener on http://localhost:8080
```

```bash
# create a short link
$ curl -X POST http://localhost:8080/shorten \
  -H 'Content-Type: application/json' \
  -d '{"url":"https://example.com/very/long/page"}'
{"code":"aB3xK9","short":"http://localhost:8080/aB3xK9"}

# follow the redirect
$ curl -i http://localhost:8080/aB3xK9
HTTP/1.1 302 Found
Location: https://example.com/very/long/page

# unknown code
$ curl -i http://localhost:8080/nope
HTTP/1.1 404 Not Found
```

## 80.3 Design notes

- **Secure random codes:** `crypto.int` uses a CSPRNG, so short codes aren't
  guessable (important if short URLs expose private targets).
- **Collision handling:** regenerates the code if it already exists (up to 100
  tries).
- **In-memory only:** codes are lost on restart; for persistence, save the map
  to a file (see chapter 62).
- **302 redirect:** the browser follows the `Location` header automatically.

## 80.4 Exercises

1. Add custom alias support: `POST /shorten` accepts optional `{code}`.
2. Add click tracking: count how many times each short link is followed.
3. Add expiration: codes expire after N seconds or N clicks.
4. Persist codes to a JSON file on each shorten and load at startup.
5. Add a `--base-url` flag so short links use the correct host.
