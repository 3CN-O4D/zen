# Chapter 44: Web Scraping & Automation

Web scraping and automation let you extract data from websites, download media, and control browsers programmatically. This chapter covers Zen's HTTP client, **pymport** (Python interoperability), browser automation with Selenium, and driving Chrome over **raw TCP/WebSocket sockets with the Chrome DevTools Protocol (CDP)**.

Every example here is runnable. The complete scripts, setup script, and test runner live in [`book-v2/tests/web-automation/`](../tests/web-automation/); the pymport fundamentals are in [Chapter 42](../interop/42-pymport-python-interoperability.md).

> **Legal and ethical note:** Only scrape sites you are permitted to scrape. Check `robots.txt`, Terms of Service, and applicable laws. The examples target publicly accessible TikTok pages for educational purposes; sites may rate-limit, require authentication, or block automated access.

## 44.1 Setup

From the repository root:

```bash
cd book-v2/tests
./setup.sh
```

`setup.sh` does three things:\n1. builds Zen with the Python bridge (`cargo build --release --features python`);\n2. creates `.venv/` beside the tests;\n3. installs [`requirements.txt`](../tests/requirements.txt).

Then, in the shell you use for the examples:

```bash
cd book-v2/tests/web-automation
export PYTHONPATH="$(../.venv/bin/python -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"
export ZEN_NO_PY_BOOTSTRAP=1
ZEN=../../../target/release/zen
```

Requirements:\n- Zen built with `--features python`\n- Python 3.9+ with `venv`\n- `chromium` for Selenium and CDP\n- `chromedriver` matching Chromium for Selenium\n- Network access

Alternatively, let Zen manage a project environment: this directory has its own `requirements.txt`, so `zen py-sync` creates `.zen_venv/` and `zen run` uses it automatically. See [Chapter 42](../interop/42-pymport-python-interoperability.md).

> **pymport calls are positional-only.** No keyword arguments in v1. Pass `Options()` to `webdriver.Chrome(opts)` positionally, and pass `Popen` arguments positionally.

## 44.2 Run the whole suite

```bash
cd book-v2/tests
./run-all.sh              # hermetic pymport tests
./run-all.sh --network    # also runs the four examples below
```

The runner validates that each downloaded file is an MP4 (`ftyp` box) before reporting success.

## 44.3 1) Scrape + Download with `requests` + `bs4` (pymport)

`bs4` parses HTML. TikTok's embed page (`/embed/v2/<id>`) is accessible over plain HTTP and embeds video metadata in `__FRONTITY_CONNECT_STATE__`. The MP4 CDN URL is in `videoData.itemInfos.video.urls[0]`; `urllib.request.urlretrieve` writes it without re-encoding.

`bs4_tiktok.z`:

```zen
import requests
import fs
pymport bs4
pymport json
pymport urllib.request
pymport socket

socket.setdefaulttimeout(45)

let UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0 Safari/537.36"
let VIDEO = "6718335390845095173"
let url = "https://www.tiktok.com/embed/v2/" + VIDEO

let _ = fs.mkdirs("out")
let out = "out/tiktok_bs4.mp4"

// TikTok's edge intermittently answers 503 to non-browser clients. Retry
// with backoff instead of failing the example on the first response.
let fetch = fn(u) {
    let attempt = 0
    let resp = null
    while attempt < 6 {
        resp = requests.get(u, {
            "headers": {
                "User-Agent": UA,
                "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                "Accept-Language": "en-US,en;q=0.9",
                "Referer": "https://www.tiktok.com/"
            },
            "timeout": 40
        })
        if resp["status_code"] == 200 {
            return resp
        }
        print("http " + str(resp["status_code"]) + " (attempt " + str(attempt + 1) + "), retrying...")
        time.sleep(3 * (attempt + 1))
        attempt += 1
    }
    return resp
}

print("fetching " + url)
let resp = fetch(url)
print("status: " + str(resp["status_code"]) + " bytes: " + str(len(resp["text"])))
if resp["status_code"] != 200 {
    print("giving up: TikTok kept returning " + str(resp["status_code"]))
    exit(1)
}

let soup = bs4.BeautifulSoup(resp["text"], "html.parser")
let tag = soup.find("script", {"id": "__FRONTITY_CONNECT_STATE__"})
if tag == null {
    print("no state script")
    exit(1)
}

let data = json.loads(tag.string)
let page = data["source"]["data"]["/embed/v2/" + VIDEO]
let item = page["videoData"]["itemInfos"]
let author = page["videoData"]["authorInfos"]
let music = page["videoData"]["musicInfos"]
let video_url = item["video"]["urls"][0]

print("id: " + item["id"])
print("desc: " + item["text"])
print("author: @" + author["uniqueId"] + " (" + author["nickName"] + ")")
print("music: " + music["musicName"])
print("plays: " + str(item["playCount"]))
print("likes: " + str(item["diggCount"]))
print("comments: " + str(item["commentCount"]))
print("shares: " + str(item["shareCount"]))
print("mp4: " + video_url[0:80] + "...")

urllib.request.urlretrieve(video_url, out)
print("downloaded: " + out + " (" + str(fs.size(out)) + " bytes)")
```

Run:

```bash
$ZEN run bs4_tiktok.z
file out/tiktok_bs4.mp4
```

Expected: metadata for `@scout2015`, a `downloaded: out/tiktok_bs4.mp4 (3224978 bytes)` line, and `ISO Media, MP4 Base Media v1`.

## 44.4 2) Stateful Scraping + Download with `mechanicalsoup` (pymport)

`MechanicalSoup` wraps `requests` and BeautifulSoup in a stateful browser. `browser.open()` keeps cookies/history, `browser.page` is the current BS4 soup, and `browser.session` can be reused for the CDN fetch. Python `bytes` are written through Python's own file object because native `fs.write_binary` is string-oriented.

`ms_tiktok.z`:

```zen
import fs
import time
pymport mechanicalsoup
pymport json
pymport builtins
pymport socket

socket.setdefaulttimeout(45)

let UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0 Safari/537.36"
let VIDEO = "6718335390845095173"
let embed = "https://www.tiktok.com/embed/v2/" + VIDEO

let _ = fs.mkdirs("out")
let out = "out/tiktok_ms.mp4"

let browser = mechanicalsoup.StatefulBrowser()
browser.session.headers.update({
    "User-Agent": UA,
    "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    "Accept-Language": "en-US,en;q=0.9",
    "Referer": "https://www.tiktok.com/"
})

let resp = browser.open(embed)
let attempt = 0
while resp.status_code != 200 && attempt < 6 {
    print("http " + str(resp.status_code) + " (attempt " + str(attempt + 1) + "), retrying...")
    time.sleep(3 * (attempt + 1))
    resp = browser.open(embed)
    attempt += 1
}

print("opened: " + browser.get_url())
print("status: " + str(resp.status_code) + " bytes: " + str(len(resp.text)))
if resp.status_code != 200 {
    print("giving up: TikTok kept returning " + str(resp.status_code))
    exit(1)
}

let soup = browser.page
let tag = soup.find("script", {"id": "__FRONTITY_CONNECT_STATE__"})
if tag == null {
    print("no state script")
    exit(1)
}

let data = json.loads(tag.string)
let page = data["source"]["data"]["/embed/v2/" + VIDEO]
let item = page["videoData"]["itemInfos"]
let author = page["videoData"]["authorInfos"]
let video_url = item["video"]["urls"][0]

print("id: " + item["id"])
print("author: @" + author["uniqueId"] + " (" + author["nickName"] + ")")
print("plays: " + str(item["playCount"]) + " likes: " + str(item["diggCount"]))

let bin = browser.session.get(video_url)
print("cdn status: " + str(bin.status_code) + " bytes: " + str(len(bin.content)))
if bin.status_code != 200 {
    print("giving up: CDN returned " + str(bin.status_code))
    exit(1)
}

let f = builtins.open(out, "wb")
f.write(bin.content)
f.close()
print("downloaded: " + out + " (" + str(fs.size(out)) + " bytes)")
```

Run:

```bash
$ZEN run ms_tiktok.z
```

## 44.5 3) Browser Automation with Selenium (pymport)

Headless Chromium solves JavaScript challenges and renders pages like a user. Selenium navigates to the real TikTok video page, waits for `__UNIVERSAL_DATA_FOR_REHYDRATION__`, and extracts the full `itemStruct` (author, stats, description). The video page's `playAddr` is signed and rejected by the CDN outside the player, so the script fetches the embed route with plain HTTP for an unsigned MP4 URL.

`selenium_tiktok.z`:

```zen
import fs
import time
import requests
pymport selenium.webdriver as webdriver
pymport selenium.webdriver.chrome.options as options
pymport bs4
pymport json
pymport urllib.request
pymport socket

socket.setdefaulttimeout(45)

let VIDEO = "6718335390845095173"
let UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0 Safari/537.36"
let url = "https://www.tiktok.com/@scout2015/video/" + VIDEO

let _ = fs.mkdirs("out")
let out = "out/tiktok_selenium.mp4"

// Quit the browser and stop the chromedriver service. The embedded
// interpreter exits immediately, so this must happen before exit().
let close_browser = fn(d) {
    d.quit()
    try {
        d.service.stop()
    } catch {
        # already stopped
    }
}

let opts = options.Options()
opts.add_argument("--headless=new")
opts.add_argument("--no-sandbox")
opts.add_argument("--disable-dev-shm-usage")
opts.add_argument("--disable-gpu")
opts.add_argument("--window-size=1280,900")
opts.add_argument("--mute-audio")

print("launching chromium...")
let driver = webdriver.Chrome(opts)
driver.set_page_load_timeout(60)

print("navigating to " + url)
try {
    driver.get(url)
} catch as e {
    print("navigation warning: " + str(e))
}

// The page renders its data after the WAF/JS boot; poll for the payload.
let src = ""
let tries = 0
while tries < 15 {
    src = driver.page_source
    if src.contains("__UNIVERSAL_DATA_FOR_REHYDRATION__") {
        break
    }
    time.sleep(2)
    tries += 1
}
print("title: " + driver.title)
print("page bytes: " + str(len(src)) + " (polls: " + str(tries) + ")")

let soup = bs4.BeautifulSoup(src, "html.parser")
let tag = soup.find("script", {"id": "__UNIVERSAL_DATA_FOR_REHYDRATION__"})
if tag == null {
    print("no rehydration payload - TikTok served a challenge")
    close_browser(driver)
    exit(1)
}

let data = json.loads(tag.string)
let item = data["__DEFAULT_SCOPE__"]["webapp.video-detail"]["itemInfo"]["itemStruct"]
let stats = item["stats"]
let author = item["author"]

print("id: " + item["id"])
print("desc: " + item["desc"])
print("author: @" + author["uniqueId"] + " (" + author["nickname"] + ")")
print("plays: " + str(stats["playCount"]) + " likes: " + str(stats["diggCount"]))
print("comments: " + str(stats["commentCount"]) + " shares: " + str(stats["shareCount"]))

print("fetching embed route for an unsigned mp4 url...")
let embed = "https://www.tiktok.com/embed/v2/" + VIDEO
let embed_headers = {
    "User-Agent": UA,
    "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    "Accept-Language": "en-US,en;q=0.9",
    "Referer": "https://www.tiktok.com/"
}
let eresp = null
let eretry = 0
while eretry < 6 {
    eresp = requests.get(embed, {"headers": embed_headers, "timeout": 40})
    if eresp["status_code"] == 200 {
        break
    }
    print("embed http " + str(eresp["status_code"]) + " (attempt " + str(eretry + 1) + "), retrying...")
    time.sleep(3 * (eretry + 1))
    eretry += 1
}
print("embed status: " + str(eresp["status_code"]))
if eresp["status_code"] != 200 {
    close_browser(driver)
    exit(1)
}
let esoup = bs4.BeautifulSoup(eresp["text"], "html.parser")
let etag = esoup.find("script", {"id": "__FRONTITY_CONNECT_STATE__"})
if etag == null {
    print("no embed state script")
    close_browser(driver)
    exit(1)
}
let edata = json.loads(etag.string)
let epage = edata["source"]["data"]["/embed/v2/" + VIDEO]
let mp4 = epage["videoData"]["itemInfos"]["video"]["urls"][0]
print("mp4: " + mp4[0:80] + "...")

urllib.request.urlretrieve(mp4, out)
print("downloaded: " + out + " (" + str(fs.size(out)) + " bytes)")

close_browser(driver)
time.sleep(1)
print("browser closed")
```

Run (Chromium startup can take 30-40s):

```bash
timeout 300 $ZEN run selenium_tiktok.z
```

Chromedriver must match the Chromium major version. On Debian/Ubuntu: `apt install chromium-driver`.

## 44.6 4) Low-Level Control: CDP over raw TCP + WebSocket sockets

This example uses no automation library. Zen launches Chromium with `--remote-debugging-port`, discovers the page target over HTTP (`/json/list`), upgrades to a WebSocket, sends masked CDP frames through `sys.write_bytes` on the socket fd, and parses unmasked server frames byte by byte.

Key protocol details:\n- **HTTP**: parse headers and stop at `Content-Length`. Chrome may keep the connection open despite `Connection: close`; waiting for EOF produces a timeout (`EAGAIN`).\n- **WebSocket**: client frames must be masked. Extended lengths are 126 + 16-bit or 127 + 64-bit. Server frames are not masked.\n- **Binary writes**: `socket.send` takes a string and would UTF-8 re-encode masked bytes. `sys.write_bytes(fd, hex)` writes raw bytes, so the frame is built as a byte list, hex-encoded, and written to `socket.fileno(sock)`.

`cdp_tiktok.z` helpers:

```zen
// CDP over raw sockets: drive headless Chromium from Zen with no libraries.
import socket
import sys
import json
import time
import fs
pymport subprocess
import base64
import bytes

let PORT = 9223
let PROFILE = "out/cdp-profile"
let VIDEO = "6718335390845095173"
let VIDEO_PAGE = "https://www.tiktok.com/@scout2015/video/" + VIDEO

let HEX = "0123456789abcdef"

let to_hex = fn(bs) {
    let out = ""
    for b in bs {
        out += HEX[b >> 4] + HEX[b & 15]
    }
    return out
}

let ws_send = fn(sock, text) {
    let payload = []
    for ch in list(text) {
        for b in bytes.utf8_bytes(ch.ord()) {
            payload.push(b)
        }
    }
    let mask = [0x37, 0xA1, 0x0C, 0xF2]
    let masked = []
    let i = 0
    while i < len(payload) {
        masked.push(payload[i] ^ mask[i % 4])
        i += 1
    }
    let frame = [0x81]
    let n = len(masked)
    if n < 126 {
        frame.push(0x80 | n)
    } elif n < 65536 {
        frame.push(0x80 | 126)
        frame.push((n >> 8) & 255)
        frame.push(n & 255)
    } else {
        frame.push(0x80 | 127)
        let k = 7
        while k >= 0 {
            frame.push((n >> (8 * k)) & 255)
            k -= 1
        }
    }
    for b in mask {
        frame.push(b)
    }
    for b in masked {
        frame.push(b)
    }
    sys.write_bytes(socket.fileno(sock), to_hex(frame))
}

let recv_exact = fn(sock, n) {
    let out = []
    while len(out) < n {
        let chunk = list(socket.recv(sock, n - len(out)))
        if len(chunk) == 0 {
            throw "websocket closed"
        }
        for b in chunk {
            out.push(b)
        }
    }
    return out
}

let ws_read = fn(sock) {
    let h = recv_exact(sock, 2)
    let opcode = h[0] & 0x0F
    let masked = (h[1] & 0x80) != 0
    let n = h[1] & 0x7F
    if n == 126 {
        let e = recv_exact(sock, 2)
        n = e[0] * 256 + e[1]
    } elif n == 127 {
        let e = recv_exact(sock, 8)
        n = 0
        for b in e {
            n = n * 256 + b
        }
    }
    let mask = []
    if masked {
        mask = recv_exact(sock, 4)
    }
    let data = recv_exact(sock, n)
    if masked {
        let i = 0
        while i < len(data) {
            data[i] = data[i] ^ mask[i % 4]
            i += 1
        }
    }
    return {"opcode": opcode, "data": data}
}

let cdp_call = fn(sock, id, method, params) {
    ws_send(sock, json.stringify({"id": id, "method": method, "params": params}))
    let pending = []
    while true {
        let f = ws_read(sock)
        if f["opcode"] == 8 {
            throw "websocket closed by peer"
        }
        if f["opcode"] == 9 {
            continue
        }
        if f["opcode"] == 1 or f["opcode"] == 0 {
            for b in f["data"] {
                pending.push(b)
            }
            let text = bytes.str_from_bytes(pending)
            try {
                let msg = json.parse(text)
                if msg.get("id", 0) == id {
                    return msg
                }
                pending = []
            } catch {
                # fragmented frame: keep collecting
            }
        }
    }
}
```

The full script then launches Chromium with `pymport subprocess`, performs the HTTP discovery and WebSocket upgrade, and polls `Runtime.evaluate`:

```zen
let _ = fs.mkdirs("out")
let _ = fs.rmtree(PROFILE)
let cmd = "chromium --headless=new --no-sandbox --disable-gpu --disable-dev-shm-usage "
cmd += "--remote-debugging-port=" + str(PORT) + " "
cmd += "--user-data-dir=" + PROFILE + " about:blank"
let argv = cmd.split(" ")
let browser = subprocess.Popen(argv, -1, null, null, null, -3)
print("chromium pid " + str(browser.pid) + " on port " + str(PORT))

try {
    # ... HTTP discovery, WS upgrade ...
    let nav = cdp_call(ws, 1, "Page.navigate", {"url": VIDEO_PAGE})
    let reply = cdp_call(ws, 100, "Runtime.evaluate", {"expression": expr, "returnByValue": true})
    # expr returns id, author, plays, likes, comments, desc
} finally {
    browser.terminate()
}
```

See the complete, tested script: [`tests/web-automation/cdp_tiktok.z`](../tests/web-automation/cdp_tiktok.z).

Run:

```bash
timeout 180 $ZEN run cdp_tiktok.z
```

## 44.7 Tips and Gotchas

- **WAF challenges**: plain `requests` to TikTok video/profile pages may return a WAF challenge. Use `/embed/v2/<id>` or a real browser (Selenium). The embed route is reliable for the MP4 URL.
- **Binary downloads**: `fs.write_binary` expects a string and has UTF-8/string constraints. Prefer `urllib.request.urlretrieve` or `builtins.open(..., "wb").write(py_bytes)`.
- **pymport limits**: positional-only calls (no kwargs). `webdriver.Chrome(opts)` and `Popen(argv, -1, null, null, null, -3)` are positional.
- **Environment bootstrap**: use `book-v2/tests/setup.sh`, or `zen py-sync` with a local `requirements.txt`.
- **Chrome sandbox**: in containers, add `--no-sandbox` and `--disable-dev-shm-usage`.
- **Sockets**: `socket.recv` returns `bytes`; use `list(bytes)` to index. `socket.recv_text` returns a string and is lossy for binary. For raw binary protocols, write through `sys.write_bytes(fd, hex)`.
- **JSON**: the pure-Zen `std/json.z` stringifies numbers with `str()`; nested dict access in CDP responses needs explicit keys.
- **Cleanup**: Selenium can leave `chromedriver` alive when the embedded interpreter exits; the example calls `driver.service.stop()`. For a hard-killed CDP run, remove leftovers with `pkill -x chromium`.
- **Hangs**: `MechanicalSoup`, `urllib.request.urlretrieve`, and `requests` do not impose a default timeout. Set `socket.setdefaulttimeout(45)` (via `pymport socket`) or pass an explicit timeout where supported.

## 44.8 Verified Artifacts

Tested against real TikTok on 2026-10-23:

- `out/tiktok_bs4.mp4` — ISO Media, MP4 Base Media v1 (3,224,978 bytes)
- `out/tiktok_ms.mp4` — ISO Media, MP4 Base Media v1 (3,224,978 bytes)
- `out/tiktok_selenium.mp4` — ISO Media, MP4 Base Media v1 (3,224,978 bytes)
- CDP over raw sockets extracted metadata from `@scout2015/video/6718335390845095173`: 159600 plays, 35200 likes, 5486 comments.

## 44.9 Appendix: Tested Scripts

Everything in this chapter is in the repository:

- [`tests/web-automation/bs4_tiktok.z`](../tests/web-automation/bs4_tiktok.z) — requests + bs4 + urllib download
- [`tests/web-automation/ms_tiktok.z`](../tests/web-automation/ms_tiktok.z) — MechanicalSoup + `browser.session` + `builtins` binary write
- [`tests/web-automation/selenium_tiktok.z`](../tests/web-automation/selenium_tiktok.z) — Selenium headless + embed route
- [`tests/web-automation/cdp_tiktok.z`](../tests/web-automation/cdp_tiktok.z) — raw TCP + WebSocket CDP
- [`tests/web-automation/requirements.txt`](../tests/web-automation/requirements.txt)
- [`tests/setup.sh`](../tests/setup.sh) — build + venv + install
- [`tests/run-all.sh`](../tests/run-all.sh) — run and validate

Quick start:

```bash
git clone https://github.com/3CN-O4D/zen.git
cd zen/book-v2/tests
./setup.sh
./run-all.sh --network
```

See [Chapter 42: pymport](../interop/42-pymport-python-interoperability.md) for setup, `zen py-sync`, and troubleshooting.
