# Chapter 47: Network Security Tools

*Note: This chapter is for educational and ethical purposes only. Always get proper authorization before testing network security.*

## Port Scanner

```zen
import socket
import time

func scan_port(host, port, timeout=1) {
    try {
        let s = socket.create(socket.AF_INET, socket.SOCK_STREAM)
        socket.settimeout(s, timeout)
        socket.connect(s, host, port)
        socket.close(s)
        return true
    } catch {
        return false
    }
}

func scan_ports(host, start_port, end_port) {
    print("Scanning " + host + " from port " + start_port + " to " + end_port)
    let open_ports = []
    let port = start_port
    while port <= end_port {
        if scan_port(host, port) {
            print("Port " + port + " is OPEN")
            open_ports.push(port)
        }
        port = port + 1
    }
    return open_ports
}

# Usage (only scan hosts you own or have permission to scan)
# let open = scan_ports("127.0.0.1", 1, 1024)
```

## Subdomain Finder

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
# find_subdomains("example.com", wordlist)
```

## HTTP Header Checker

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
            "X-XSS-Protection",
            "Referrer-Policy",
            "Permissions-Policy"
        ]
        
        for h in security_headers {
            if resp.headers.contains(h) {
                print("✓ " + h + ": " + resp.headers[h])
            } else {
                print("✗ " + h + ": MISSING")
            }
        }
    } catch (e) {
        print("Error: " + e)
    }
}

check_headers("https://example.com")
```

## Directory Brute Forcer

```zen
import requests

func brute_directories(url, wordlist) {
    let found = []
    for word in wordlist {
        let test_url = url + "/" + word
        try {
            let resp = requests.get(test_url)
            if resp.status_code == 200 || resp.status_code == 301 || resp.status_code == 302 {
                print("Found: " + test_url + " [" + resp.status_code + "]")
                found.push(test_url)
            }
        } catch {}
    }
    return found
}

let dirs = ["admin", "login", "config", "backup", "uploads"]
# brute_directories("https://example.com", dirs)
```

## SSL Certificate Checker

```zen
import crypto.x509
import socket
import ssl

func check_ssl_cert(host, port=443) {
    try {
        # Get certificate (simplified)
        let cert_pem = get_server_cert(host, port)
        let cert = crypto.x509.load_cert(cert_pem)
        
        print("Certificate for " + host)
        print("Subject:", cert.subject)
        print("Issuer:", cert.issuer)
        print("Valid until:", cert.not_after)
        
        # Check expiration
        let days_left = (cert.not_after_timestamp - time.time()) / 86400
        if days_left < 30 {
            print("WARNING: Expires in " + int(days_left) + " days!")
        }
    } catch (e) {
        print("Error: " + e)
    }
}
```

## Ethical Guidelines

1. **Only test with permission** - Written authorization required
2. **Stay in scope** - Only test authorized targets
3. **Report responsibly** - Report vulnerabilities properly
4. **No malicious use** - These tools for defense/testing only
5. **Follow laws** - Computer crime laws apply
6. **Educational purpose** - Learn to defend, not attack

## Chapter Summary

- Port scanning for network reconnaissance
- Subdomain enumeration
- HTTP security header analysis
- Directory enumeration
- SSL certificate inspection
- Always practice ethical security

**Next:** Chapter 48 - Capstone Projects
