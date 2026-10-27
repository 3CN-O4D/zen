# Chapter 68: Networking Deep Dive — Full Packet Trace & Socket Labs

A start-to-finish walkthrough of a single HTTP request as it travels through all
7 OSI layers, followed by hands-on socket programming labs you can type and run.

## 68.1 The scenario

You type `https://example.com/` in a browser (or run `zen run httpclient.z`).
Here is exactly what happens, layer by layer, from your application down to the
wire and back.

## 68.2 Layer 7 — Application (HTTP)

The browser builds an HTTP/1.1 request:

```
GET / HTTP/1.1
Host: example.com
User-Agent: ZenBrowser/1.0
Accept: text/html
Connection: keep-alive
```

This is plain text. The application layer does not know about IP addresses,
MAC addresses, or Ethernet frames — it only knows HTTP methods, headers, and
bodies.

Zen equivalent (raw socket):
```zen
import socket
let s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.connect(("93.184.216.34", 443))   # example.com (conceptually; real HTTPS uses TLS)
s.send(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n")
let response = s.recv(4096)
print(response)
s.close()
```

## 68.3 Layer 6 — Presentation

HTTP itself has no presentation sub-layer in practice, but this is where:
- Character encoding is handled (UTF-8).
- TLS/SSL encryption would happen for HTTPS (here the HTTP payload is encrypted
  before being handed to TCP).
- Compression (gzip) is applied if `Accept-Encoding: gzip` was sent.

For HTTPS, the plaintext HTTP request is encrypted into TLS records at this
layer before TCP sees it.

## 68.4 Layer 5 — Session

The session layer manages the dialogue: open → exchange → close. For HTTP/1.1
with `keep-alive`, the TCP connection stays open for multiple requests. The
browser tracks which responses belong to which requests (especially with
pipelining or HTTP/2 multiplexing).

## 68.5 Layer 4 — Transport (TCP)

The presentation data (encrypted HTTP for HTTPS, or plain HTTP) is handed to
TCP. TCP:

1. Breaks the data into segments (each ≤ MSS, e.g. 1460 bytes over Ethernet).
2. Adds a TCP header (20 bytes minimum):

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |          Source Port          |       Destination Port        |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |                        Sequence Number                        |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |                     Acknowledgment Number                     |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |  Data |           |   CWR |ECE|URG|ACK|PSH|RST|SYN|FIN| Window |
 | Offset| Reserved  |                                             |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |           Checksum            |         Urgent Pointer        |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

3. Performs the three-way handshake:
   - Client → Server: `SYN` (synchronize)
   - Server → Client: `SYN-ACK`
   - Client → Server: `ACK`
4. Provides reliable, ordered delivery with retransmission, flow control, and
   congestion control.

Zen TCP client (connect + send):
```zen
import socket
let s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.settimeout(5000)
s.connect(("93.184.216.34", 80))
s.send(b"GET / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n")
let data = b""
while true {
    let chunk = s.recv(4096)
    if chunk == nil or len(chunk) == 0 { break }
    data = data + chunk
}
print(data)
s.close()
```

## 68.6 Layer 3 — Network (IP)

TCP segments are handed to IP, which:
- Adds an IP header (20 bytes minimum):

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |Version|  IHL  |Type of Service|          Total Length         |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |         Identification        |Flags|     Fragment Offset     |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |  Time to Live |    Protocol   |         Header Checksum       |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |                     Source Address                            |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |                   Destination Address                         |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

- Chooses the next-hop router via the routing table.
- Handles fragmentation if a segment exceeds the link MTU.
- Source = your IP (e.g. `192.168.1.42`), Destination = `93.184.216.34`.

IP is unreliable and connectionless — it makes a best-effort attempt and does
not guarantee delivery. Reliability is TCP's job (layer 4).

## 68.7 Layer 2 — Data Link (Ethernet)

IP packets are encapsulated in Ethernet frames:
- Destination MAC (your gateway/router's MAC, learned via ARP).
- Source MAC (your network card).
- EtherType = `0x0800` (IPv4).
- The IP packet becomes the Ethernet frame payload.
- Frame Check Sequence (CRC) for error detection.

ARP (Address Resolution Protocol) maps the next-hop IP to a MAC address:
- Your machine broadcasts: "Who has 192.168.1.1? Tell 192.168.1.42."
- The router replies: "192.168.1.1 is at aa:bb:cc:dd:ee:ff."

## 68.8 Layer 1 — Physical

Bits are placed on the physical medium:
- Ethernet over twisted pair: electrical voltages.
- Wi-Fi: radio waves.
- Fiber: light pulses.

The bits travel to your router, then through the ISP's network, across the
internet backbone, to the server hosting example.com. At the server, the
process unwinds in reverse order (physical → data link → network → transport →
session → presentation → application).

## 68.9 The return path

The server's response travels back down its own stack, across the internet, and
up your machine's stack. The browser renders the HTML, completing the round
trip — typically tens to hundreds of milliseconds.

## 68.10 Layer summary for an HTTP GET over Ethernet

| Layer | Protocol | Unit | Key header fields |
|-------|----------|------|-------------------|
| 7 Application | HTTP | message | method, path, headers |
| 6 Presentation | TLS / encoding | data | encryption, encoding |
| 5 Session | (in TCP) | dialogue | connection lifecycle |
| 4 Transport | TCP | segment | ports, seq, ack, flags |
| 3 Network | IP | packet | src/dst IP, TTL, protocol |
| 2 Data Link | Ethernet | frame | src/dst MAC, EtherType, CRC |
| 1 Physical | Ethernet PHY | bits | voltages/light on wire |

## 68.11 Socket programming labs

### Lab 1: Daytime client (read the time from a server)

```zen
import socket
let s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.settimeout(5000)
s.connect(("time.nist.gov", 13))   # Daytime protocol (plain text)
let data = s.recv(4096)
print("daytime: " + data)
s.close()
```

### Lab 2: Minimal HTTP/1.0 client (raw socket, no TLS)

```zen
import socket
func http_get(host, path) {
    let s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(5000)
    s.connect((host, 80))
    let request = "GET " + path + " HTTP/1.0\r\nHost: " + host +
                  "\r\nConnection: close\r\n\r\n"
    s.send(request)
    let data = b""
    while true {
        let chunk = s.recv(4096)
        if chunk == nil or len(chunk) == 0 { break }
        data = data + chunk
    }
    s.close()
    return data
}
let response = http_get("example.com", "/")
print(response[0:500])   # first 500 bytes
```

### Lab 3: UDP echo (send/receive datagrams)

UDP has no connection — you `sendto` and `recvfrom`:

```zen
import socket
# server side
let s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.bind(("127.0.0.1", 9999))
print("udp echo on 9999")
let (data, addr) = s.recvfrom(4096)
print("received from " + str(addr) + ": " + data)
s.sendto(data, addr)   # echo back
s.close()
```

Client:
```zen
let s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.sendto(b"ping", ("127.0.0.1", 9999))
let (data, addr) = s.recvfrom(4096)
print("echo: " + data)
s.close()
```

### Lab 4: Port scanner (educational, localhost only)

```zen
import socket
func is_open(host, port) {
    let s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(500)
    try {
        s.connect((host, port))
        s.close()
        return true
    } catch as e {
        s.close()
        return false
    }
}
for p in 1->1024 {
    if is_open("127.0.0.1", p) {
        print("open: " + str(p))
    }
}
```
Only scan hosts you own or have permission to test.

### Lab 5: Raw ICMP ping concept (requires root/admin)

Raw sockets let you construct ICMP packets directly:

```zen
import socket
# requires root/administrator privileges
let s = socket.socket(socket.AF_PACKET, socket.SOCK_RAW, socket.IPPROTO_ICMP)
# construct IP header + ICMP echo request, compute checksum, send, wait for reply
```
Raw sockets are powerful but dangerous — never send spoofed or flood traffic on
networks you don't own.

## 68.12 Troubleshooting socket code

| Symptom | Cause | Fix |
|---------|-------|-----|
| Connection refused | Nothing listening | start server; check port |
| Address already in use | previous TIME_WAIT | set SO_REUSEADDR |
| recv returns empty bytes | peer closed | check for nil/empty, close |
| send breaks pipe | peer closed already | catch BrokenPipe |
| DNS fails | resolver issue | try IP directly; check /etc/resolv.conf |
| Timeout | no response / firewall | increase timeout; check connectivity |

## 68.13 Exercises

1. Trace an HTTPS request: where does encryption happen (layer 6)?
2. Write a raw TCP client that speaks HTTP/1.0 and prints the status line.
3. Extend Lab 3 (UDP echo) to handle multiple clients.
4. Draw the full header stack (Ethernet + IP + TCP + HTTP) with byte offsets.
5. Explain why ARP is needed between layers 3 and 2.
6. Implement `recv_until(sock, delimiter)` reading byte-by-byte.
7. Build a tiny chat client using Lab 2's pattern.
