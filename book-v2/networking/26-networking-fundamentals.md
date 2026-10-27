# Chapter 26: Networking Fundamentals

Zen's networking stack provides sockets, DNS, HTTP, TLS, and raw packet crafting. This chapter covers the fundamentals: IP addressing, TCP vs UDP, ports, the OSI model mapping, and how Zen's networking modules fit together.

## 26.1 Networking module map

Zen's networking is split across several modules, each at a different layer:

| Module | Layer | Purpose |
|--------|-------|---------|
| `socket` | Transport (L4) | TCP/UDP sockets, `connect`, `bind`, `listen`, `accept`, `send`, `recv` |
| `net` | Transport (L4) | Higher-level TCP/UDP helpers (`tcp_connect`, `tcp_send`, etc.) |
| `net.dns` | Application (L7) | DNS resolution (`resolve`, `gethostbyname`) |
| `requests` | Application (L7) | HTTP client (GET/POST/etc., sessions, TLS) |
| `http` | Application (L7) | Low-level HTTP client |
| `http.server` | Application (L7) | Static file HTTP server |
| `tls` | Security (L4.5) | TLS connections with certificate pinning |
| `raw` / `net_raw` | Network (L3) | Raw IP sockets, packet crafting (`pack_ip`, `pack_tcp`) |
| `sys` | System | File descriptors, pipes, syscalls |

## 26.2 IP addressing

Zen works with IPv4 addresses (dotted-decimal strings like `"192.168.1.1"`). IPv6 support depends on the platform.

```zen
import socket
import dns

// Resolve a hostname to IP addresses
let addrs = dns.resolve("example.com")
for addr in addrs {
    print(addr)  # "93.184.216.34" or "2606:2800:220:1:248:1893:25c8:1946"
}

// localhost
print(dns.resolve("localhost"))  # ["127.0.0.1", "::1"]
```

### Private IP ranges (RFC 1918)

| Range | Purpose |
|-------|---------|
| `10.0.0.0` – `10.255.255.255` | Private (class A) |
| `172.16.0.0` – `172.31.255.255` | Private (class B) |
| `192.168.0.0` – `192.168.255.255` | Private (class C) |
| `127.0.0.0` – `127.255.255.255` | Loopback (localhost) |
| `169.254.0.0` – `169.254.255.255` | Link-local (APIPA) |

```zen
import dns

// Localhost always resolves
print(dns.resolve("127.0.0.1"))   # ["127.0.0.1"]
print(dns.resolve("localhost"))   # ["127.0.0.1", ...]
```

## 26.3 Ports

Ports are 16-bit integers (0–65535). Ports 0–1023 are "well-known" and typically require root privileges to bind.

| Port | Service |
|------|---------|
| 20 | FTP data |
| 21 | FTP control |
| 22 | SSH |
| 23 | Telnet |
| 25 | SMTP |
| 53 | DNS |
| 67/68 | DHCP |
| 80 | HTTP |
| 110 | POP3 |
| 123 | NTP |
| 143 | IMAP |
| 161/162 | SNMP |
| 389 | LDAP |
| 443 | HTTPS |
| 465 | SMTPS |
| 587 | SMTP submission |
| 636 | LDAPS |
| 993 | IMAPS |
| 995 | POP3S |
| 1433 | MSSQL |
| 3306 | MySQL |
| 3389 | RDP |
| 5432 | PostgreSQL |
| 5900 | VNC |
| 6379 | Redis |
| 27017 | MongoDB |
| 8080 | HTTP alternate |

```zen
import socket

// Bind to port 80 typically requires root
let server = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.bind(server, "0.0.0.0", 80)   # may fail without root
socket.bind(server, "0.0.0.0", 8080)  # usually OK
```

## 26.4 TCP vs UDP

| Feature | TCP | UDP |
|---------|-----|-----|
| Connection | Connection-oriented (handshake) | Connectionless |
| Reliability | Reliable (retransmission, ACK) | Unreliable (no guarantee) |
| Ordering | Ordered | Unordered |
| Flow control | Yes | No |
| Congestion control | Yes | No |
| Header overhead | 20+ bytes | 8 bytes |
| Use case | Web, file transfer, email | DNS, streaming, VoIP, games |

```zen
import socket

// TCP socket (stream)
let tcp = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.connect(tcp, "example.com", 80)
socket.send(tcp, "GET / HTTP/1.0\r\n\r\n")
let resp = socket.recv(tcp, 4096)
socket.close(tcp)

// UDP socket (datagram)
let udp = socket.create(socket.AF_INET, socket.SOCK_DGRAM)
socket.send_to(udp, "query", "8.8.8.8", 53)
let [data, addr] = socket.recv_from(udp, 4096)
socket.close(udp)
```

## 26.5 The OSI model and Zen

| OSI Layer | Name | Zen module | Example |
|-----------|------|------------|---------|
| 7 | Application | `requests`, `http`, `http.server`, `dns` | HTTP GET, DNS resolve |
| 6 | Presentation | `base64`, `binascii`, `crypto` | Encoding, encryption |
| 5 | Session | `requests.Session` | HTTP sessions |
| 4 | Transport | `socket`, `net`, `tls` | TCP, UDP, TLS |
| 3 | Network | `raw`, `net_raw` | IP packets, ICMP |
| 2 | Data Link | — | (not exposed) |
| 1 | Physical | — | (not exposed) |

Zen focuses on layers 3–7. Layers 1–2 (physical, data link) are handled by the OS/kernel.

## 26.6 Sockets: file descriptors

Zen sockets use integer file descriptors (handles) into an internal resource table:

```zen
import socket

let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
print(sock)   # 3 (an integer handle)
socket.connect(sock, "example.com", 80)
socket.close(sock)   # release the handle
```

Always close sockets when done to avoid resource leaks:
```zen
import socket

let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
try {
    socket.connect(sock, "example.com", 80)
    socket.send(sock, "GET / HTTP/1.0\r\n\r\n")
    let resp = socket.recv(sock, 4096)
    print(resp)
} finally {
    socket.close(sock)   # always close
}
```

## 26.6 Address families

| Constant | Meaning |
|----------|---------|
| `socket.AF_INET` | IPv4 |
| `socket.AF_INET6` | IPv6 (platform-dependent) |
| `socket.AF_UNIX` | Unix domain sockets (platform-dependent) |

### Socket types

| Constant | Meaning |
|----------|---------|
| `socket.SOCK_STREAM` | TCP (reliable stream) |
| `socket.SOCK_DGRAM` | UDP (unreliable datagram) |
| `socket.SOCK_RAW` | Raw IP (requires root) |

```zen
import socket

let tcp = socket.create(socket.AF_INET, socket.SOCK_STREAM)
let udp = socket.create(socket.AF_INET, socket.SOCK_DGRAM)
let raw = socket.create(socket.AF_INET, socket.SOCK_RAW)  # may need root
```

## 26.6 Timeouts

Socket operations can block indefinitely. Always set timeouts:

```zen
import socket

let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.settimeout(sock, 5000)   # 5 second timeout

try {
    socket.connect(sock, "slow.example.com", 80)
    socket.send(sock, "GET / HTTP/1.0\r\n\r\n")
    let resp = socket.recv(sock, 4096)
    print(resp)
} catch as e {
    print(e.msg)  # timeout or connection error
} finally {
    socket.close(sock)
}
```

## 26.6 Byte order

Network byte order is big-endian. Zen's socket functions handle this internally. When crafting raw packets with `net_raw`, you must manually use big-endian:

```zen
import net_raw

// pack_ip and pack_tcp handle byte order internally
let pkt = net_raw.pack_ip("1.2.3.4", "5.6.7.8", 6)
```

## 26.7 Quick reference

```zen
import socket
import dns
import net_raw

// Resolve
let addrs = dns.resolve("example.com")

// TCP client
let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.settimeout(sock, 5000)
socket.connect(sock, addrs[0], 80)
socket.send(sock, "GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
let resp = socket.recv(sock, 4096)
socket.close(sock)

// UDP
let udp = socket.create(socket.AF_INET, socket.SOCK_DGRAM)
socket.send_to(udp, "ping", "127.0.0.1", 9999)
let [data, addr] = socket.recv_from(udp, 4096)
socket.close(udp)

// Raw IP packet
let ip_pkt = net_raw.pack_ip("1.2.3.4", "5.6.7.8", 6)
let tcp_pkt = net_raw.pack_tcp(12345, 80, "SYN")
```

## 26.8 Exercises

1. Write a function `resolve_all(host)` that prints all IP addresses for a hostname.
2. Write a TCP echo client that sends a line and prints the response.
3. Write a UDP client that sends a datagram and waits for a reply.
4. Write a port scanner for ports 1–1000 on localhost with a 100ms timeout per port.
5. Create a raw IP packet with `net_raw.pack_ip` and inspect the first 20 bytes.
6. Write a function `is_port_open(host, port, timeout)` that returns true if a TCP connect succeeds.
7. Demonstrate that `socket.close` releases the file descriptor by opening and closing multiple sockets.
