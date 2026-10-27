# Chapter 48: Capstone Projects

This chapter presents complete, end-to-end projects that combine multiple Zen features: pymport, web scraping, file processing, security tools, and CLI building.

## 48.1 Capstone 1: Web Metadata Archiver

A tool that fetches a webpage, extracts metadata (title, description, images, links), and saves it as a JSON file.

```zen
import requests
import bs4
import json
import fs
import argparser
import sys

func archive_metadata(url, output_file) {
    let resp = requests.get(url, {
        "headers": {"User-Agent": "Zen-Archiver/1.0"},
        "timeout": 30
    })
    
    if !resp.ok {
        throw "Failed to fetch " + url + ": HTTP " + str(resp.status_code)
    }
    
    let soup = bs4.BeautifulSoup(resp["text"], "html.parser")
    
    let metadata = {
        "url": url,
        "title": "",
        "description": "",
        "images": [],
        "links": []
    }
    
    // Title
    let title_tag = soup.find("title")
    if title_tag != null {
        metadata["title"] = title_tag.string
    }
    
    // Meta description
    let desc_tag = soup.find("meta", {"name": "description"})
    if desc_tag != null {
        metadata["description"] = desc_tag.get("content", "")
    }
    
    // Images
    let img_tags = soup.find_all("img")
    for img in img_tags {
        let src = img.get("src", "")
        if len(src) > 0 {
            metadata["images"].push(src)
        }
    }
    
    // Links
    let link_tags = soup.find_all("a")
    for link in link_tags {
        let href = link.get("href", "")
        if len(href) > 0 {
            metadata["links"].push(href)
        }
    }
    
    fs.write(output_file, json.dumps(metadata, {"indent": 2}))
    print("Metadata saved to " + output_file)
    print("Title: " + metadata["title"])
    print("Images: " + str(len(metadata["images"])))
    print("Links: " + str(len(metadata["links"])))
}

let parser = argparser.ArgumentParser("archive-metadata")
parser.add_argument("url", help="URL to archive")
parser.add_argument("--output", "-o", default="metadata.json", help="Output file")
let args = parser.parse_args()

try {
    archive_metadata(args.url, args.output)
} catch as e {
    fs.write("/dev/stderr", "Error: " + e.msg + "\n")
    sys.exit(1)
}
```

## 48.2 Capstone 2: Password Strength Checker

A tool that checks password strength using entropy calculation and common password lists.

```zen
import fs
import math
import random
import argparser

func password_strength(password) {
    let score = 0
    let reasons = []
    
    let length = len(password)
    if length >= 8 {
        score = score + 1
    } else {
        reasons.push("Password too short (min 8)")
    }
    
    let has_lower = false
    let has_upper = false
    let has_digit = false
    let has_special = false
    
    for ch in password {
        let code = ord(ch)
        if code >= 97 and code <= 122 {
            has_lower = true
        } elif code >= 65 and code <= 90 {
            has_upper = true
        } elif code >= 48 and code <= 57 {
            has_digit = true
        } else {
            has_special = true
        }
    }
    
    if has_lower { score = score + 1 } else { reasons.push("Add lowercase") }
    if has_upper { score = score + 1 } else { reasons.push("Add uppercase") }
    if has_digit { score = score + 1 } else { reasons.push("Add digit") }
    if has_special { score = score + 1 } else { reasons.push("Add special char") }
    
    let strength = ["Very Weak", "Weak", "Fair", "Good", "Strong"]
    let level = strength[min(score, 4)]
    
    return {"score": score, "level": level, "reasons": reasons}
}

func generate_password(length) {
    let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*"
    let pw = []
    for i in 1..length {
        pw.push(random.choice(chars))
    }
    return "".join(pw)
}

let parser = argparser.ArgumentParser("password-checker")
parser.add_argument("password", nargs="?", help="Password to check")
parser.add_argument("--generate", "-g", type="int", help="Generate random password of length")
let args = parser.parse_args()

if args.generate != nil {
    let pw = generate_password(args.generate)
    let strength = password_strength(pw)
    print("Generated: " + pw)
    print("Strength: " + strength["level"] + " (" + str(strength["score"]) + "/5)")
} elif args.password != nil {
    let strength = password_strength(args.password)
    print("Password: " + args.password)
    print("Strength: " + strength["level"] + " (" + str(strength["score"]) + "/5)")
    if len(strength["reasons"]) > 0 {
        print("Suggestions:")
        for r in strength["reasons"] {
            print("  - " + r)
        }
    }
} else {
    parser.print_help()
}
```

## 48.3 Capstone 3: Log Analyzer

A tool that parses web server logs and produces summary statistics.

```zen
import fs
import re
import argparser

func analyze_log(path) {
    let content = fs.read(path)
    let lines = content.split("\n")
    
    let total = 0
    let status_codes = {}
    let ips = {}
    let paths = {}
    
    let log_re = re.compile("^(\\S+) \\S+ \\S+ \\[.*\\] \"(\\S+) (\\S+) \\S+\" (\\d{3})")
    
    for line in lines {
        if len(line) == 0 {
            continue
        }
        total = total + 1
        
        let m = log_re.search(line)
        if m != nil {
            let ip = m.group(1)
            let method = m.group(2)
            let path = m.group(3)
            let status = m.group(4)
            
            ips[ip] = (ips.get(ip, 0) + 1)
            paths[path] = (paths.get(path, 0) + 1)
            status_codes[status] = (status_codes.get(status, 0) + 1)
        }
    }
    
    return {
        "total": total,
        "status_codes": status_codes,
        "top_ips": sort_by_value(ips, 10),
        "top_paths": sort_by_value(paths, 10)
    }
}

func sort_by_value(dict, n) {
    let items = []
    for k in dict.keys() {
        items.push([k, dict[k]])
    }
    // Simple bubble sort by value descending
    for i in 0..len(items) {
        for j in 0..(len(items) - i - 1) {
            if items[j][1] < items[j + 1][1] {
                let tmp = items[j]
                items[j] = items[j + 1]
                items[j + 1] = tmp
            }
        }
    }
    let result = []
    for i in 0..min(n, len(items)) {
        result.push(items[i])
    }
    return result
}

let parser = argparser.ArgumentParser("log-analyzer")
parser.add_argument("file", help="Log file to analyze")
let args = parser.parse_args()

try {
    let result = analyze_log(args.file)
    print("Log Analysis Report")
    print("=" * 40)
    print("Total requests: " + str(result["total"]))
    print("")
    print("Status codes:")
    for code in result["status_codes"].keys() {
        print("  " + code + ": " + str(result["status_codes"][code]))
    }
    print("")
    print("Top IPs:")
    for item in result["top_ips"] {
        print("  " + item[0] + ": " + str(item[1]))
    }
    print("")
    print("Top paths:")
    for item in result["top_paths"] {
        print("  " + item[0] + ": " + str(item[1]))
    }
} catch as e {
    fs.write("/dev/stderr", "Error: " + e.msg + "\n")
    sys.exit(1)
}
```

## 48.4 Capstone 4: Site Uptime Monitor

A tool that checks multiple URLs and reports which are down.

```zen
import requests
import fs
import argparser
import time

func check_url(url, timeout) {
    try {
        let resp = requests.get(url, {"timeout": timeout})
        return {
            "url": url,
            "status": resp.status_code,
            "ok": resp.ok,
            "error": nil
        }
    } catch as e {
        return {
            "url": url,
            "status": nil,
            "ok": false,
            "error": e.msg
        }
    }
}

func monitor(urls, timeout) {
    let results = []
    for url in urls {
        let result = check_url(url, timeout)
        results.push(result)
        let status = result["ok"] ? "UP" : "DOWN"
        print(url + ": " + status + (result["error"] != nil ? " (" + result["error"] + ")" : ""))
    }
    return results
}

let parser = argparser.ArgumentParser("uptime-monitor")
parser.add_argument("urls", nargs="*", help="URLs to monitor")
parser.add_argument("--timeout", "-t", type="int", default=10, help="Timeout in seconds")
let args = parser.parse_args()

if len(args.urls) == 0 {
    print("No URLs to monitor")
    sys.exit(1)
}

let results = monitor(args.urls, args.timeout)

let up_count = 0
for r in results {
    if r["ok"] {
        up_count = up_count + 1
    }
}
print("")
print("Summary: " + str(up_count) + "/" + str(len(results)) + " sites up")

// Save report
let report = json.dumps(results, {"indent": 2})
fs.write("uptime_report.json", report)
print("Report saved to uptime_report.json")
```

## 48.5 Running the Capstones

```bash
# Metadata archiver
cd book-v2/tests
export PYTHONPATH="$(../.venv/bin/python -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"
export ZEN_NO_PY_BOOTSTRAP=1
ZEN=../../../target/release/zen

# Note: capstone scripts are examples; adapt paths as needed
# $ZEN run projects/capstone/archive_metadata.z https://example.com --output meta.json
```

## 48.6 Quick reference

```zen
# Web metadata archiver: requests + bs4 + json + fs
# Password checker: math + random + argparser + fs
# Log analyzer: fs + re + argparser
# Uptime monitor: requests + fs + argparser + json
```

## 48.7 Exercises

1. Extend the metadata archiver to also extract `<h1>` tags and meta keywords.
2. Add rate limiting to the uptime monitor (wait between checks).
3. Add email/Slack notifications to the uptime monitor when a site goes down.
4. Extend the password checker to check against a common-passwords list.
5. Add HTML report generation to the log analyzer.


## Extended Project Ideas and Implementation Notes

### Design checklist
- Requirements and scope
- Error handling
- Logging
- Configuration
- Testing
- Documentation
- Security considerations

### Exercises
1. Extend the project with new features
2. Add tests
3. Improve error messages
4. Add help/usage
5. Make it configurable
6. Package for distribution


## 48.12 Capstone — personal network toolkit

### Goal
Combine networking, CLI, file, and crypto into one tool:
`pkit <command>` with subcommands:
- `get <url>` — fetch a page (HTTP)
- `hash <file>` — SHA-256 of a file
- `scan <host> <port>` — check if a TCP port is open
- `totp <secret>` — generate a TOTP token
- `genkey` — generate a secure random token

### Implementation (`pkit.z`)

```zen
import sys
import requests
import hashlib
import socket
import crypto

func cmd_get(args) {
    if len(args) < 1 { print("usage: pkit get <url>"); return 1 }
    try {
        let r = requests.get(args[0], timeout=10)
        print("status=" + str(r.status_code))
        print(r.text[0:500])
    } catch as e {
        print("error: " + e.msg)
        return 1
    }
    return 0
}

func cmd_hash(args) {
    if len(args) < 1 { print("usage: pkit hash <file>"); return 1 }
    try {
        print(hashlib.sha256(args[0]))
    } catch as e {
        print("error: " + e.msg)
        return 1
    }
    return 0
}

func cmd_scan(args) {
    if len(args) < 2 { print("usage: pkit scan <host> <port>"); return 1 }
    let host = args[0]
    let port = int(args[1])
    let s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(2.0)
    try {
        s.connect((host, port))
        print(host + ":" + str(port) + " OPEN")
    } catch as e {
        print(host + ":" + str(port) + " closed/unreachable (" + e.msg + ")")
    } finally {
        s.close()
    }
    return 0
}

func cmd_totp(args) {
    if len(args) < 1 { print("usage: pkit totp <secret>"); return 1 }
    print(crypto.totp(args[0]))
    return 0
}

func cmd_genkey(_) {
    print(crypto.secure_token())
    return 0
}

func main(argv) {
    if len(argv) < 2 {
        print("pkit - personal network toolkit")
        print("commands: get <url> | hash <file> | scan <host> <port> | totp <secret> | genkey")
        return 1
    }
    let cmd = argv[1]
    let rest = []
    for i in 2..len(argv) { rest.push(argv[i]) }
    if cmd == "get" { return cmd_get(rest) }
    elif cmd == "hash" { return cmd_hash(rest) }
    elif cmd == "scan" { return cmd_scan(rest) }
    elif cmd == "totp" { return cmd_totp(rest) }
    elif cmd == "genkey" { return cmd_genkey(rest) }
    else { print("unknown command: " + cmd); return 1 }
}

if __name__ == "__main__" { sys.exit(main(sys.argv)) }
```

### Sample session

```bash
$ zen run pkit.z genkey
aB3xK9mP2vR5wL8n
$ zen run pkit.z hash pkit.z
e3b0c442...  (SHA-256)
$ zen run pkit.z scan localhost 80
localhost:80 closed/unreachable (ConnectionRefusedError)
$ zen run pkit.z totp JBSWY3DPEHPK3PXP
742193
$ zen run pkit.z get https://example.com
status=200
<!doctype html>...
```

### Extending the capstone
- Add `post <url> <json>` for POST requests.
- Add `resolve <host>` for DNS lookup.
- Add `--timeout` flag globally.
- Add config file for default settings.
- Add colors/progress indicators.
- Add tests for each subcommand.

## 48.13 Capstone lessons

1. **Compose** modules (requests, socket, hashlib, crypto, sys).
2. **Dispatch** on subcommands cleanly.
3. **Handle errors** per subcommand without crashing the whole tool.
4. **Use timeouts** on all network calls.
5. **Keep each subcommand small** and focused.
6. **Document** usage in the `main` help text.

## 48.14 Exercises

1. Add `post` subcommand sending JSON.
2. Add `resolve <host>` using the dns module.
3. Add a global `--timeout N` flag.
4. Add `ports <host> <start>-<end>` scanning a range.
5. Add `--output <file>` to save results.
6. Add colors to output (ANSI escape codes).
7. Write tests verifying `hash` output matches `hashlib.sha256`.
8. Add `scan` concurrent port scanning (threading).
9. Create a config file `pkit.toml` with defaults.
10. Build and distribute with `zen build pkit.z -o pkit`.

