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
