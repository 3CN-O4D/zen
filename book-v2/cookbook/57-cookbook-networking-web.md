# Chapter 57: Cookbook — Networking & Web

Complete recipes for HTTP clients/servers, sockets, DNS, and URL handling.

## 57.1 Recipe: GET a URL and print the body

```zen
import requests
func get(url) {
    try {
        let r = requests.get(url, timeout=10)
        print("status=" + str(r.status_code))
        print(r.text)
    } catch as e {
        print("error: " + e.msg)
    }
}
get("https://example.com")
```

Expected output:
```
status=200
<!doctype html>...
```

## 57.2 Recipe: POST JSON to an API

```zen
import requests
func post_json(url, payload) {
    let r = requests.post(url, json=payload, timeout=10)
    print("status=" + str(r.status_code))
    print(r.text)
}
post_json("https://api.example.com/submit", #{name: "Zen", value: 42})
```

## 57.3 Recipe: Reuse a connection with a Session

```zen
import requests
let s = requests.Session()
s.headers.update({"User-Agent": "ZenBot/1.0"})
let r1 = s.get("https://example.com/page1")
let r2 = s.get("https://example.com/page2")
print(r1.status_code, r2.status_code)
```

## 57.4 Recipe: Download a file with streaming

```zen
import requests
func download(url, dest) {
    let r = requests.get(url, stream=true, timeout=30)
    with open(dest, "wb") as f {
        for chunk in r.iter_content() {
            f.write(chunk)
        }
    }
    print("downloaded " + dest)
}
download("https://example.com/file.zip", "file.zip")
```

## 57.5 Recipe: TCP echo server

```zen
import socket
let s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.bind(("127.0.0.1", 9000))
s.listen(5)
print("echo server on 9000")
let (c, addr) = s.accept()
let data = c.recv(4096)
c.send(data)
c.close()
s.close()
```

Test:
```bash
$ echo "hi" | nc 127.0.0.1 9000
hi
```

## 57.6 Recipe: TCP echo client

```zen
import socket
let s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.connect(("127.0.0.1", 9000))
s.send(b"hello")
print(s.recv(4096))   # hello
s.close()
```

## 57.7 Recipe: Resolve a domain to IP (DNS)

```zen
import dns
let ips = dns.resolve("example.com", "A")
print(ips[0])   # 93.184.216.34 (example)
```

## 57.8 Recipe: Parse a URL into components

```zen
import url
let u = url.parse("https://example.com:8080/path?q=1#sec")
print(u.scheme)   # https
print(u.host)     # example.com
print(u.port)     # 8080
print(u.path)     # /path
print(u.query)    # q=1
print(u.fragment) # sec
```

## 57.9 Recipe: Percent-encode a query value

```zen
import url
print(url.quote("hello world & foo=bar"))   # hello%20world%20%26%20foo%3Dbar
print(url.unquote("hello%20world"))         # hello world
```

## 57.10 Recipe: Minimal HTTP server with routing

```zen
import http.server
let routes = #{}
func route(path, handler) { routes[path] = handler }
route("/", fn(req) { return #(200, #{}, "<h1>Hello</h1>") })
route("/time", fn(req) {
    import datetime
    return #(200, #{}, datetime.now())
})
let server = http.server.Server(port=8080, handler=fn(req) {
    let h = routes[req.path] ?? fn(r) { return #(404, #{}, "Not Found") }
    return h(req)
})
server.start()
```

Test:
```bash
$ curl http://localhost:8080/
<h1>Hello</h1>
$ curl http://localhost:8080/time
2024-...
```

## 57.11 Recipe: Check if a TCP port is open

```zen
import socket
func is_port_open(host, port) {
    let s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(2.0)
    try {
        s.connect((host, port))
        s.close()
        return true
    } catch as e {
        s.close()
        return false
    }
}
print(is_port_open("127.0.0.1", 8080))   # true or false
```

## 57.12 Recipe: Fetch HTTPS with certificate verification

```zen
import requests
let r = requests.get("https://example.com", verify=true, timeout=10)
print("status=" + str(r.status_code))
```

Never use `verify=false` in production.
