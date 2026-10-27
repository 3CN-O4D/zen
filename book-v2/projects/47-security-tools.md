# Chapter 47: Network Security Tools

*Note: This chapter is for educational and ethical purposes only. Always get proper authorization before testing network security.*

## 47.1 Port Scanner

```zen
import socket

func scan_port(host, port, timeout) {
    try {
        let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
        socket.settimeout(sock, timeout)
        socket.connect(sock, host, port)
        socket.close(sock)
        return true
    } catch {
        return false
    }
}

func scan_ports(host, start_port, end_port) {
    print("Scanning " + host + " from port " + str(start_port) + " to " + str(end_port))
    let open_ports = []
    let port = start_port
    while port <= end_port {
        if scan_port(host, port, 500) {
            print("Port " + str(port) + " is OPEN")
            open_ports.push(port)
        }
        port = port + 1
    }
    return open_ports
}

// Usage (only scan hosts you own or have permission to scan)
// let open = scan_ports("127.0.0.1", 1, 1024)
```

## 44. Subdomain Finder

```zen
import dns
import requests

func find_subdomains(domain, wordlist) {
    let found = []
    for sub in wordlist {
        let hostname = sub + "." + domain
        try {
            let ips = dns.resolve(hostname)
            print("Found: " + hostname + " -> " + ips[0])
            found.push({"subdomain": hostname, "ips": ips})
        } catch {
            # Not found
        }
    }
    return found
}

let wordlist = ["www", "mail", "ftp", "api", "admin", "test", "dev"]
// find_subdomains("example.com", wordlist)
```

## 44. HTTP Header Checker

```zen
import requests

func check_headers(url) {
    try {
        let resp = requests.get(url)
        print("Security Headers for " + url)
        print("=" * 40)
        
        let security_headers = [
            "Strict-Transport-Security",
            "Content-Security-Policy",
            "X-Frame-Options",
            "X-Content-Type-Options",
            "Referrer-Policy"
        ]
        
        for h in security_headers {
            let val = resp.headers[h]
            if val != nil {
                print(h + ": " + val)
            } else {
                print(h + ": MISSING")
            }
        }
    } catch as e {
        print("Error: " + e.msg)
    }
}

// check_headers("https://example.com")
```

## 44.5 Port Scan Report

```zen
import socket
import fs

func scan_and_report(host, start, end, output_file) {
    let open_ports = []
    for port in start..end {
        try {
            let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
            socket.settimeout(sock, 300)
            socket.connect(sock, host, port)
            socket.close(sock)
            open_ports.push(port)
            print("OPEN: " + str(port))
        } catch {
            # closed or filtered
        }
    }
    
    let report = "# Scan Report for " + host + "\n"
    report = report + "Open ports: " + str(len(open_ports)) + "\n"
    for p in open_ports {
        report = report + "  - " + str(p) + "\n"
    }
    
    fs.write(output_file, report)
    print("Report written to " + output_file)
}

// scan_and_report("127.0.0.1", 1, 1000, "scan_report.txt")
```

## 44.6 HTTP Header Analysis

```zen
import requests

func analyze_security(url) {
    try {
        let resp = requests.get(url, {"timeout": 10})
        
        print("URL: " + url)
        print("Status: " + str(resp.status_code))
        print("")
        
        let headers = [
            "Strict-Transport-Security",
            "Content-Security-Policy",
            "X-Frame-Options",
            "X-Content-Type-Options",
            "X-XSS-Protection",
            "Referrer-Policy",
            "Permissions-Policy"
        ]
        
        for h in headers {
            let val = resp.headers[h]
            if val == nil {
                print("[MISSING] " + h)
            } else {
                print("[OK] " + h + ": " + val)
            }
        }
        
        // Check for server header disclosure
        let server = resp.headers["Server"]
        if server != nil {
            print("[INFO] Server header: " + server)
        }
        
    } catch as e {
        print("Error: " + e.msg)
    }
}

// analyze_security("https://example.com")
```

## 44.7 DNS Reconnaissance

```zen
import dns

func dns_recon(domain) {
    print("DNS Recon for " + domain)
    print("")
    
    // A record
    try {
        let addrs = dns.resolve(domain)
        print("A records:")
        for a in addrs {
            print("  " + a)
        }
    } catch {
        print("No A records found")
    }
    
    // MX record (if supported)
    // ...
    
    print("")
    print("FQDN: " + dns.getfqdn(domain))
}

// dns_recon("example.com")
```

## 44.8 Common pitfalls

- **Only scan hosts you own or have explicit written permission to test.**
- **Rate-limit your requests** to avoid overwhelming the target.
- **TLS certificate errors** may occur with self-signed certs.
- **Some WAFs block automated scanning.**

## 44.9 Quick reference

```zen
import socket
import dns
import requests

// Port scan
socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.connect(sock, host, port)
socket.close(sock)

// DNS resolve
dns.resolve(domain)

// HTTP headers
requests.get(url)
resp.headers[header_name]
```

## 44.10 Exercises

1. Write a port scanner that scans common ports (21, 22, 25, 53, 80, 443, 3306, 3389).
2. Write a subdomain enumerator with a custom wordlist.
3. Write a security header checker for a list of URLs.
4. Write a DNS reconnaissance script that gathers A, MX, and NS records.
5. Write a scan report generator that saves results to a file.


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


## 47.12 Full tutorial — hash utility (ethical security tool)

### Goal
Compute and compare file hashes (SHA-256) to verify integrity.

### Implementation (`hashutil.z`)

```zen
import hashlib
import sys
import pathlib

func hash_file(path) {
    let b = pathlib.Path(path)
    if !b.exists() { throw "file not found: " + path }
    # read in chunks for large files
    let data = []
    with open(path, "rb") as f {
        while true {
            let chunk = f.read(8192)
            if chunk == nil || len(chunk) == 0 { break }
            data.push(chunk)
        }
    }
    # note: hashlib.sha256 expects a string in this build; for real binary
    # hashing, feed bytes appropriately per stdlib API.
    let joined = data.join("")   # if bytes join supported
    return hashlib.sha256(joined)
}

func main(argv) {
    if len(argv) < 2 {
        print("usage: hashutil <file> [expected_hash]")
        return 1
    }
    let path = argv[1]
    let expected = argv[2] ?? nil
    try {
        let actual = hash_file(path)
        print("SHA-256(" + path + ") = " + actual)
        if expected != nil {
            if actual == expected {
                print("MATCH: file integrity verified")
                return 0
            } else {
                print("MISMATCH: file may be corrupted or tampered")
                return 1
            }
        }
    } catch as e {
        print("error: " + e.msg)
        return 1
    }
    return 0
}

if __name__ == "__main__" { sys.exit(main(sys.argv)) }
```

### Sample run

```bash
$ zen run hashutil.z document.pdf
SHA-256(document.pdf) = a3f5...c9
$ zen run hashutil.z document.pdf a3f5...c9
SHA-256(document.pdf) = a3f5...c9
MATCH: file integrity verified
```

### Ethical use
- Only hash files you own or have permission to analyze.
- Hashing verifies integrity, not "hacking."
- Don't use MD5/SHA-1 for security; prefer SHA-256.

## 47.13 Security tool principles

1. **Authorization:** only operate on systems/data you're permitted to test.
2. **Integrity over secrecy:** hashing confirms a file hasn't changed.
3. **Use strong algorithms:** SHA-256+, Argon2id, AES-GCM.
4. **Never log secrets:** avoid printing keys/passwords/tokens.
5. **Constant-time comparison** for secrets (avoid timing leaks).
6. **Input validation:** sanitize file paths, URLs, and user input.

## 47.14 Exercises

1. Add MD5 and SHA-1 options (note: not for security).
2. Compare two files and report if identical.
3. Hash all files in a directory and print a manifest.
4. Verify a file against a manifest file (hash per line).
5. Add a `--quiet` flag printing only MATCH/MISMATCH.
6. Implement constant-time string compare (conceptually).
7. Hash a string input (not file) with SHA-256.
8. Add `pbkdf2`-based password hashing (from crypto module).
9. Create a "file changed" watcher: hash, wait, re-hash, report diff.
10. Document ethical boundaries in a README for the tool.

