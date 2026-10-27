# Chapter 27: socket Module

The `socket` module provides low-level TCP/UDP/raw sockets. It's Tier 1 of Zen's networking stack — thin wrappers over the OS socket API with integer file descriptors (handles).

## 27.1 Socket creation

```zen
import socket

// TCP socket (IPv4)
let tcp = socket.create(socket.AF_INET, socket.SOCK_STREAM)

// UDP socket (IPv4)
let udp = socket.create(socket.AF_INET, socket.SOCK_DGRAM)

// Raw socket (requires root on most OSes)
let raw = socket.create(socket.AF_INET, socket.SOCK_RAW)
```

### Address families and socket types

| Constant | Meaning |
|----------|---------|
| `socket.AF_INET` | IPv4 |
| `socket.AF_INET6` | IPv6 (platform-dependent) |
| `socket.AF_UNIX` | Unix domain sockets (platform-dependent) |
| `socket.SOCK_STREAM` | TCP (reliable stream) |
| `socket.SOCK_DGRAM` | UDP (unreliable datagram) |
| `socket.SOCK_RAW` | Raw IP (requires root) |

### Common combinations

| Family | Type | Protocol | Use |
|--------|------|----------|-----|
| `AF_INET` | `SOCK_STREAM` | TCP | Web, file transfer, email |
| `AF_INET` | `SOCK_DGRAM` | UDP | DNS, streaming, games |
| `AF_INET` | `SOCK_RAW` | IP | Packet crafting, ping |
| `AF_INET6` | `SOCK_STREAM` | TCP over IPv6 | IPv6 services |
| `AF_UNIX` | `SOCK_STREAM` | Local | IPC on same machine |

## 27.2 Socket handle operations

| Function | Description |
|----------|-------------|
| `socket.create(domain, type)` | Create socket, returns handle (int) |
| `socket.connect(sock, host, port)` | Connect TCP socket to remote |
| `socket.bind(sock, host, port)` | Bind socket to local address |
| `socket.listen(sock, backlog)` | Listen for incoming connections |
| `socket.accept(sock)` | Accept incoming connection, returns new socket handle |
| `socket.send(sock, data)` | Send data on connected socket |
| `socket.recv(sock, n)` | Receive up to n bytes (returns bytes) |
| `socket.recv_text(sock, n)` | Receive text |
| `socket.recv_from(sock, n)` | Receive from address (UDP), returns `{data, addr}` |
| `socket.send_to(sock, data, host, port)` | Send to address (UDP) |
| `socket.settimeout(sock, ms)` | Set timeout in milliseconds |
| `socket.close(sock)` | Close socket and release handle |
| `socket.fileno(sock)` | Get raw file descriptor |

## 27.3 TCP client

```zen
import socket

let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.settimeout(sock, 5000)   // 5 second timeout

socket.connect(sock, "example.com", 80)
socket.send(sock, "GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")

let response = ""
while true {
    let chunk = socket.recv(sock, 4096)
    if len(chunk) == 0 {
        break
    }
    response = response + chunk
}

print(response)
socket.close(sock)
```

### recv returns bytes

`socket.recv` returns a byte list (list of ints 0–255), not a string. Use `socket.recv_text` for text, or convert:

```zen
import socket

let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.connect(sock, "example.com", 80)
socket.send(sock, "GET / HTTP/1.0\r\n\r\n")

let bytes = socket.recv(sock, 1024)
// bytes is a list like [72, 84, 84, 80, ...]

let text = socket.recv_text(sock, 1024)
// text is a string

socket.close(sock)
```

To convert bytes to string manually:
```zen
import socket

let bs = socket.recv(sock, 1024)
let s = ""
for b in bs {
    s = s + chr(b)
}
print(s)
```

## 27.4 TCP server

```zen
import socket

let server = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.bind(server, "0.0.0.0", 8080)
socket.listen(server, 5)

print("Listening on 0.0.0.0:8080")

let client = socket.accept(server)
let req = socket.recv_text(client, 4096)
print("Request: " + req)

socket.send(client, "HTTP/1.0 200 OK\r\nContent-Type: text/plain\r\n\r\nHello, Zen!")
socket.close(client)
socket.close(server)
```

### Accept loop

```zen
import socket

let server = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.bind(server, "0.0.0.0", 8080)
socket.listen(server, 5)

while true {
    let client = socket.accept(server)
    let req = socket.recv_text(client, 4096)
    socket.send(client, "HTTP/1.0 200 OK\r\n\r\npong")
    socket.close(client)
}
```

## 27.5 UDP

UDP is connectionless — no `connect`, `listen`, or `accept`. Use `send_to` and `recv_from`.

### UDP client

```zen
import socket

let sock = socket.create(socket.AF_INET, socket.SOCK_DGRAM)
socket.settimeout(sock, 5000)

socket.send_to(sock, "ping", "127.0.0.1", 9999)

let [data, addr] = socket.recv_from(sock, 4096)
print("Response from " + addr.addr + ": " + data)

socket.close(sock)
```

`recv_from` returns a dict `{data: <bytes>, addr: <ip_string>}`.

### UDP server

```zen
import socket

let sock = socket.create(socket.AF_INET, socket.SOCK_DGRAM)
socket.bind(sock, "0.0.0.0", 9999)

while true {
    let [data, addr] = socket.recv_from(sock, 4096)
    print("From " + addr.addr + ": " + data)
    socket.send_to(sock, "pong", addr.addr, 9999)
}
```

## 27.6 Socket options

```zen
import socket

let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)

// Set timeout
socket.settimeout(sock, 5000)  // 5 seconds

// Get file descriptor (for raw operations)
let fd = socket.fileno(sock)
print(fd)  // integer

socket.close(sock)
```

## 27.7 Common patterns

### Read until connection closes

```zen
import socket

func read_all(sock) {
    let result = ""
    while true {
        let chunk = socket.recv_text(sock, 4096)
        if len(chunk) == 0 {
            break
        }
        result = result + chunk
    }
    return result
}

let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.connect(sock, "example.com", 80)
socket.send(sock, "GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
let body = read_all(sock)
print(body)
socket.close(sock)
```

### Read exactly n bytes

```zen
import socket

func recv_exact(sock, n) {
    let buf = []
    while len(buf) < n {
        let chunk = socket.recv(sock, n - len(buf))
        if len(chunk) == 0 {
            break
        }
        for b in chunk {
            buf.push(b)
        }
    }
    return buf
}
```

### Port scanner

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

for port in 1..1024 {
    if scan_port("127.0.0.1", port, 100) {
        print("Port " + str(port) + " is open")
    }
}
```

### HTTP client (manual, no requests module)

```zen
import socket

func http_get(host, path) {
    let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
    socket.connect(sock, host, 80)
    
    let request = "GET " + path + " HTTP/1.0\r\nHost: " + host + "\r\nConnection: close\r\n\r\n"
    socket.send(sock, request)
    
    let response = ""
    while true {
        let chunk = socket.recv_text(sock, 4096)
        if len(chunk) == 0 {
            break
        }
        response = response + chunk
    }
    
    socket.close(sock)
    return response
}

print(http_get("example.com", "/"))
```

## 27.8 Common pitfalls

- **Always close sockets.** Unclosed sockets leak file descriptors.
- **Set timeouts.** Without timeouts, `connect`/`recv` can block forever.
- **TCP is a stream.** `recv` may return fewer bytes than requested — loop until you have all data.
- **`recv` returns bytes**, not strings. Use `recv_text` or convert manually.
- **UDP is connectionless.** Use `send_to`/`recv_from`, not `connect`/`send`/`recv`.
- **`recv_from` returns `{data, addr}`**, not just data.
- **Raw sockets (`SOCK_RAW`) require root** on most operating systems.

## 27.8 Quick reference

```zen
import socket

let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.settimeout(sock, 5000)
socket.connect(sock, "example.com", 80)
socket.send(sock, "GET / HTTP/1.0\r\n\r\n")
let data = socket.recv(sock, 4096)
let text = socket.recv_text(sock, 4096)
socket.close(sock)

// UDP
socket.send_to(sock, "data", "127.0.0.1", 9999)
let [data, addr] = socket.recv_from(sock, 4096)

// Server
socket.bind(sock, "0.0.0.0", 8080)
socket.listen(sock, 5)
let client = socket.accept(sock)
```

## 26.9 Exercises

1. Write a TCP echo client that sends a line and prints the response.
2. Write a UDP client that sends a datagram and waits for a reply.
3. Write a port scanner for ports 1–1000 on localhost.
4. Write a simple HTTP/0.9 client that fetches the root page.
5. Write a TCP echo server that handles one connection at a time.
6. Write a UDP server that echoes back datagrams.
7. Combine client and server in one script (server in a thread, client in main).
