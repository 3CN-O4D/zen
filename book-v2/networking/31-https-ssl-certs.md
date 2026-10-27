# Chapter 31: HTTPS & SSL/TLS Certificates

Zen's `tls` module provides encrypted TCP connections using rustls, with optional certificate pinning. This chapter covers TLS connections, certificate pinning, and secure HTTPS practices.

## 31.1 The `tls` module

```zen
import tls

fd = tls.connect("example.com", 443)
tls.write(fd, "GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
let resp = tls.read(fd, 4096)
print(resp)
tls.close(fd)
```

### TLS functions

| Function | Description |
|----------|-------------|
| `tls.connect(host, port)` | Plain TLS connection, returns fd |
| `tls.connect_pinned(host, port, pins)` | TLS with certificate pinning |
| `tls.write(fd, data)` | Send data, returns nil |
| `tls.read(fd, n)` | Read up to n bytes, returns string |
| `tls.close(fd)` | Close connection, returns nil |

```zen
import tls

let fd = tls.connect("example.com", 443)
tls.write(fd, "GET / HTTP/1.0\r\nHost: example.com\r\nConnection: close\r\n\r\n")
let resp = tls.read(fd, 4096)
print(resp)
tls.close(fd)
```

## 31.2 Certificate pinning

Certificate pinning ensures you only connect to servers with a specific certificate, preventing man-in-the-middle attacks even if the CA is compromised.

```zen
import tls

let pins = ["sha256:AAAA...BBBB..."]  // SHA-256 of server's SPKI
let fd = tls.connect_pinned("example.com", 443, pins)
tls.write(fd, "GET / HTTP/1.0\r\n\r\n")
let resp = tls.read(fd, 4096)
tls.close(fd)
```

### How pinning works

1. `tls.connect_pinned` connects to the server.
2. During the TLS handshake, the server presents its certificate.
3. Zen computes the SHA-256 hash of the certificate's SPKI (Subject Public Key Info).
4. If the hash matches one of the pins, the connection proceeds.
5. If no pin matches, the handshake fails.

### Getting a certificate's SPKI hash

```zen
// You can compute this externally with openssl:
// openssl s_client -connect example.com:443 -showcerts
// Then hash the SPKI of the leaf certificate.

// Example pin format:
let pins = ["sha256:a1b2c3d4e5f6..."]
```

## 31.3 Plain TLS vs pinned TLS

| Feature | `tls.connect` | `tls.connect_pinned` |
|---------|---------------|----------------------|
| Verification | Platform root store (webpki-roots) | Pin SPKI hash match |
| Trust model | CA hierarchy | Explicit pins |
| Fallback | Accepts any CA-signed cert | Only pinned certs |
| Use case | General browsing | Known services (APIs, internal) |

```zen
import tls

// General browsing - trusts any CA-signed cert
let fd1 = tls.connect("example.com", 443)

// Known service - only accept specific cert
let pins = ["sha256:expected-spki-hash"]
let fd2 = tls.connect_pinned("api.internal.com", 443, pins)
```

## 31.4 Complete HTTPS client example

```zen
import tls

func https_get(host, path) {
    let fd = tls.connect(host, 443)
    
    let request = "GET " + path + " HTTP/1.0\r\n" +
                  "Host: " + host + "\r\n" +
                  "Connection: close\r\n\r\n"
    
    tls.write(fd, request)
    
    let response = ""
    while true {
        let chunk = tls.read(fd, 4096)
        if len(chunk) == 0 {
            break
        }
        response = response + chunk
    }
    
    tls.close(fd)
    return response
}

print(https_get("example.com", "/"))
```

### HTTPS with pinning

```zen
import tls

func https_get_pinned(host, path, pins) {
    let fd = tls.connect_pinned(host, 443, pins)
    
    let request = "GET " + path + " HTTP/1.0\r\n" +
                  "Host: " + host + "\r\n" +
                  "Connection: close\r\n\r\n"
    
    tls.write(fd, request)
    
    let response = ""
    while true {
        let chunk = tls.read(fd, 4096)
        if len(chunk) == 0 {
            break
        }
        response = response + chunk
    }
    
    tls.close(fd)
    return response
}

let pins = ["sha256:AAAA...BBBB..."]
print(https_get_pinned("example.com", "/", pins))
```

## 31.5 Error handling

```zen
import tls

try {
    let fd = tls.connect("nonexistent.example.invalid", 443)
    tls.write(fd, "GET / HTTP/1.0\r\n\r\n")
    let resp = tls.read(fd, 4096)
    tls.close(fd)
} catch as e {
    print("TLS error: " + e.msg)
    // Could be: DNS failure, connection refused, TLS handshake failure,
    // certificate verification failure, pin mismatch
}

// Pin mismatch example
try {
    let pins = ["sha256:wrong-hash"]
    let fd = tls.connect_pinned("example.com", 443, pins)
} catch as e {
    print(e.msg)  // certificate pin mismatch
}
```

## 31.6 Quick reference

```zen
import tls

let fd = tls.connect("example.com", 443)
tls.write(fd, "GET / HTTP/1.0\r\n\r\n")
let resp = tls.read(fd, 4096)
tls.close(fd)

// With pinning
let pins = ["sha256:AAAA..."]
let fd2 = tls.connect_pinned("example.com", 443, pins)
tls.close(fd2)
```

## 31.7 Exercises

1. Write an HTTPS client that fetches a page and prints the status line.
2. Get a certificate's SPKI hash using openssl, then use `tls.connect_pinned` to connect only to that cert.
3. Write a function `https_get_pinned(host, path, pins)` that returns the response body.
4. Demonstrate that `tls.connect_pinned` fails with a wrong pin.
5. Write a script that connects to an HTTPS API and parses the JSON response.


## 31.12 HTTPS / SSL/TLS — deep reference

HTTPS is HTTP over TLS. It provides encryption, authentication (via
certificates), and integrity.

### TLS handshake (simplified)

1. ClientHello: client sends supported versions, cipher suites, random.
2. ServerHello: server picks version/cipher, sends its certificate.
3. Server proves identity (certificate signed by a trusted CA).
4. Key exchange: client and server derive shared keys.
5. Encrypted HTTP traffic begins.

### Certificates

| Concept | Meaning |
|---------|---------|
| Certificate | binds a public key to an identity (domain/org) |
| CA | Certificate Authority that signs certificates |
| Chain | leaf → intermediate → root CA |
| Self-signed | signed by itself; not trusted by default |
| SAN | Subject Alternative Names (multiple domains) |
| Expiry | certificates expire; must be renewed |

### Verifying certificates

```zen
import requests
# verify=true (default): validates cert against system CAs
let r = requests.get("https://example.com", verify=true)

# verify=false: skips validation — DANGEROUS, testing only
# let r = requests.get("https://self-signed.local", verify=false)
```

### Common TLS errors

| Error | Cause |
|-------|-------|
| certificate verify failed | untrusted/self-signed/expired cert |
| hostname mismatch | cert doesn't match the domain |
| protocol version mismatch | client/server disagree on TLS version |
| handshake failure | no common cipher suite |

### Best practices

1. Always `verify=true` in production.
2. Never disable verification to "fix" a cert error — fix the cert.
3. Use modern TLS (1.2+/1.3).
4. Renew certificates before expiry.
5. Use strong cipher suites; disable weak ones.

### Exercises

1. Fetch an HTTPS page and print status + first 200 chars.
2. Compare `verify=true` vs `verify=false` on a self-signed site (theory).
3. Inspect a certificate's issuer, subject, expiry (using openssl).
4. Explain the TLS handshake steps in your own words.
5. List why a cert might fail verification.
6. Write a helper that fetches HTTPS with timeout and error handling.
7. Research Let's Encrypt and how to get a free cert.
8. Explain HSTS and why it matters.



## 31.12 Worked example — fetching HTTPS with verification

```zen
import requests

func fetch_secure(url) {
    try {
        let r = requests.get(url, verify=true, timeout=10)
        print("status=" + str(r.status_code))
        print(r.text[0:200])
    } catch as e {
        print("TLS/HTTP error: " + e.msg)
    }
}

fetch_secure("https://example.com")
```

Expected output:
```
status=200
<!doctype html>...
```

## 31.13 Troubleshooting TLS/HTTPS

| Symptom | Cause | Fix |
|---------|-------|-----|
| certificate verify failed | self-signed/expired/untrusted cert | fix cert or add to trust store |
| hostname mismatch | cert doesn't match domain | use correct URL |
| protocol mismatch | client/server TLS version differ | enable TLS 1.2/1.3 |
| handshake failure | no common cipher | update client/server |

Never disable `verify=true` in production. For testing only, `verify=false`
bypasses validation (dangerous).

## 31.14 Exercises

1. Fetch https://example.com and print status + first 200 chars.
2. Compare behavior with verify=true vs verify=false on a self-signed site.
3. List reasons a certificate fails verification.
4. Write a helper `safe_get(url)` with timeout and error handling.
5. Explain the TLS handshake in your own words.
