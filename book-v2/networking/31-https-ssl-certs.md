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
