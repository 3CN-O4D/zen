# Chapter 34: Raw Sockets

The `socket` module provides low-level TCP/UDP/raw sockets, and `net_raw` provides IP/TCP packet construction. This chapter covers raw socket programming and packet crafting.

## 34.1 TCP client

```zen
import socket

let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.settimeout(sock, 5000)
socket.connect(sock, "example.com", 80)

socket.send(sock, "GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
let response = socket.recv(sock, 4096)
print(socket.recv_text(sock, 4096))

socket.close(sock)
```

### Socket functions

| Function | Description |
|----------|-------------|
| `socket.create(domain, type)` | Create socket, returns handle |
| `socket.connect(sock, host, port)` | Connect TCP |
| `socket.bind(sock, host, port)` | Bind to local address |
| `socket.listen(sock, backlog)` | Listen for connections |
| `socket.accept(sock)` | Accept incoming connection |
| `socket.send(sock, data)` | Send data |
| `socket.recv(sock, n)` | Receive up to n bytes |
| `socket.recv_text(sock, n)` | Receive text |
| `socket.recv_from(sock, n)` | Receive from address (UDP) |
| `socket.send_to(sock, data, host, port)` | Send to address (UDP) |
| `socket.settimeout(sock, ms)` | Set timeout |
| `socket.close(sock)` | Close socket |
| `socket.fileno(sock)` | Get file descriptor |

### Address families and socket types

| Constant | Meaning |
|----------|---------|
| `socket.AF_INET` | IPv4 |
| `socket.AF_INET6` | IPv6 |
| `socket.AF_UNIX` | Unix domain sockets |
| `socket.SOCK_STREAM` | TCP |
| `socket.SOCK_DGRAM` | UDP |
| `socket.SOCK_RAW` | Raw IP (requires root) |

## 34.2 TCP client

```zen
import socket

let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.settimeout(sock, 5000)
socket.connect(sock, "example.com", 80)

socket.send(sock, "GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
let response = socket.recv(sock, 4096)
print(socket.recv_text(sock, 4096))

socket.close(sock)
```

### recv returns bytes

`socket.recv` returns a byte list (list of ints 0–255). Use `socket.recv_text` for strings:

```zen
import socket

let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.connect(sock, "example.com", 80)
socket.send(sock, "GET / HTTP/1.0\r\n\r\n")

let bytes = socket.recv(sock, 1024)   // list of ints
let text = socket.recv_text(sock, 1024) // string

socket.close(sock)
```

## 34.3 TCP server

```zen
import socket

let server = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.bind(server, "0.0.0.0", 8080)
socket.listen(server, 5)

print("Listening on 0.0.0.0:8080")

let client = socket.accept(server)
let req = socket.recv_text(client, 4096)
print("Request: " + req)

socket.send(client, "HTTP/1.0 200 OK\r\n\r\nHello")
socket.close(client)
socket.close(server)
```

## 34.4 UDP

```zen
import socket

let sock = socket.create(socket.AF_INET, socket.SOCK_DGRAM)
socket.settimeout(sock, 5000)

socket.send_to(sock, "hello", "127.0.0.1", 9999)

let [data, addr] = socket.recv_from(sock, 4096)
print(data)
print(addr.addr)

socket.close(sock)
```

`recv_from` returns `{data: <bytes>, addr: <ip_string>}`.

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

## 34.5 Raw sockets and packet crafting (net_raw)

```zen
import net_raw

// Build an IP packet
let ip_pkt = net_raw.pack_ip("10.0.0.1", "10.0.0.2", 6)  // proto 6 = TCP
print(len(ip_pkt))  // 20 bytes

// Build a TCP segment
let tcp_pkt = net_raw.pack_tcp(12345, 80, "SYN")
print(len(tcp_pkt))  // 20 bytes
```

### `pack_ip(src, dst, proto)`

Builds a 20-byte IPv4 header:
- `src`, `dst`: IP address strings
- `proto`: protocol number (6 = TCP, 17 = UDP, 1 = ICMP)

### `pack_tcp(sport, dport, flags)`

Builds a 20-byte TCP header:
- `sport`, `dport`: port numbers
- `flags`: string containing any of FIN, SYN, RST, PSH, ACK, URG

```zen
import net_raw

let ip = net_raw.pack_ip("1.2.3.4", "5.6.7.8", 6)
print(len(ip))   // 20

let tcp = net_raw.pack_tcp(12345, 80, "SYN")
print(len(tcp))  // 20

// IPv4 header layout:
// Bytes 0-1: Version/IHL + TOS
// Bytes 2-3: Total length
// Bytes 4-5: Identification
// Bytes 6-7: Flags/Fragment offset
// Byte 8: TTL
// Byte 9: Protocol
// Bytes 10-11: Header checksum
// Bytes 12-15: Source IP
// Bytes 16-19: Destination IP

// TCP header layout:
// Bytes 0-1: Source port
// Bytes 2-3: Dest port
// Bytes 4-7: Sequence number
// Bytes 8-11: Acknowledgment number
// Byte 12: Data offset + reserved
// Byte 13: Flags
// Bytes 14-15: Window size
// Bytes 16-17: Checksum
// Bytes 18-19: Urgent pointer
```

### Raw socket flags

```zen
import net_raw

let syn = net_raw.pack_tcp(12345, 80, "SYN")
let syn_ack = net_raw.pack_tcp(80, 12345, "SYN,ACK")
let ack = net_raw.pack_tcp(80, 12345, "ACK")
let fin = net_raw.pack_tcp(80, 12345, "FIN,ACK")
let rst = net_raw.pack_tcp(80, 12345, "RST")
```

## 34.6 Common patterns

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

### HTTP client (manual)

```zen
import socket

func http_get(host, path) {
    let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
    socket.connect(sock, host, 80)
    
    let request = "GET " + path + " HTTP/1.0\r\nHost: " + host + "\r\n\r\n"
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

## 34.7 Common pitfalls

- **Always close sockets.** Leaks file descriptors.
- **Set timeouts.** Without timeouts, `connect`/`recv` can block forever.
- **TCP is a stream.** `recv` may return partial data; loop until you have everything.
- **`socket.recv` returns bytes.** Use `recv_text` for strings, or convert manually.
- **UDP is connectionless.** Use `send_to`/`recv_from`, not `connect`/`send`/`recv`.
- **Raw sockets need root** on most OSes for `SOCK_RAW` and packet crafting.

## 34.8 Quick reference

```zen
import socket
import net_raw

let sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.connect(sock, "example.com", 80)
socket.send(sock, "GET / HTTP/1.0\r\n\r\n")
let data = socket.recv(sock, 4096)
socket.close(sock)

// UDP
socket.send_to(sock, "data", "127.0.0.1", 9999)
let from = socket.recv_from(sock, 4096)

// Packet crafting
let ip = net_raw.pack_ip("1.2.3.4", "5.6.7.8", 6)
let tcp = net_raw.pack_tcp(1234, 80, "SYN")
```

## 34.8 Exercises

1. Write a TCP echo client that sends a line and prints the response.
2. Write a UDP client that sends a datagram and waits for a reply.
3. Write a port scanner for ports 1–1000 on localhost.
4. Build an IP+TCP packet with `net_raw` and inspect the bytes.
5. Write a simple HTTP/0.9 client that fetches the root page.
6. Write a TCP echo server that handles one connection at a time.
7. Write a UDP server that echoes back datagrams.
