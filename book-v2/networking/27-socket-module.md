# Chapter 27: socket Module

The `socket` module provides low-level BSD socket API access in Zen. This gives you full control over network connections but requires understanding socket programming concepts.

## Importing the socket Module

```zen
import socket
```

## Socket Types

### Socket Families
- `socket.AF_INET` - IPv4
- `socket.AF_INET6` - IPv6
- `socket.AF_UNIX` - Unix domain sockets (local)

### Socket Types
- `socket.SOCK_STREAM` - TCP (reliable, connection-oriented)
- `socket.SOCK_DGRAM` - UDP (unreliable, connectionless)
- `socket.SOCK_RAW` - Raw sockets

## Creating a Socket

```zen
# TCP socket
let tcp_sock = socket.create(socket.AF_INET, socket.SOCK_STREAM)

# UDP socket
let udp_sock = socket.create(socket.AF_INET, socket.SOCK_DGRAM)
```

## TCP Client Example

```zen
import socket

# Create TCP socket
let s = socket.create(socket.AF_INET, socket.SOCK_STREAM)

# Connect to server
socket.connect(s, "example.com", 80)

# Send HTTP request
let request = "GET / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n"
socket.send(s, request)

# Receive response
let response = socket.recv(s, 4096)
print(response)

# Close socket
socket.close(s)
```

## TCP Server Example

```zen
import socket

# Create socket
let server = socket.create(socket.AF_INET, socket.SOCK_STREAM)

# Set socket options to reuse address
socket.setsockopt(server, socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)

# Bind to address and port
socket.bind(server, "0.0.0.0", 8080)

# Listen for connections (backlog of 5)
socket.listen(server, 5)

print("Server listening on port 8080...")

while true {
    # Accept connection
    let client = socket.accept(server)
    print("Client connected")
    
    # Receive data
    let data = socket.recv(client, 1024)
    print("Received: " + data)
    
    # Send response
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\nHello from Zen socket server!"
    socket.send(client, response)
    
    # Close client connection
    socket.close(client)
}
```

## UDP Example

### UDP Client

```zen
import socket

let s = socket.create(socket.AF_INET, socket.SOCK_DGRAM)

let message = "Hello UDP Server"
socket.sendto(s, message, "localhost", 9999)

let [data, addr] = socket.recvfrom(s, 1024)
print("Received: " + data)
print("From: " + addr)

socket.close(s)
```

### UDP Server

```zen
import socket

let s = socket.create(socket.AF_INET, socket.SOCK_DGRAM)
socket.bind(s, "0.0.0.0", 9999)

print("UDP server listening on port 9999...")

while true {
    let [data, addr] = socket.recvfrom(s, 1024)
    print("Received from " + addr + ": " + data)
    
    # Echo back
    socket.sendto(s, "Echo: " + data, addr[0], addr[1])
}
```

## Socket Options

```zen
import socket

let s = socket.create(socket.AF_INET, socket.SOCK_STREAM)

# Allow address reuse
socket.setsockopt(s, socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)

# Set timeout
socket.settimeout(s, 5)  # 5 seconds

# Get socket option
let val = socket.getsockopt(s, socket.SOL_SOCKET, socket.SO_REUSEADDR)
```

## Socket Methods Reference

| Method | Description |
|--------|-------------|
| `create(family, type)` | Create a new socket |
| `bind(sock, addr, port)` | Bind socket to address/port |
| `listen(sock, backlog)` | Listen for connections |
| `accept(sock)` | Accept incoming connection |
| `connect(sock, addr, port)` | Connect to remote host |
| `send(sock, data)` | Send data on connected socket |
| `recv(sock, size)` | Receive data from connected socket |
| `sendto(sock, data, addr, port)` | Send data to address (UDP) |
| `recvfrom(sock, size)` | Receive data with sender address (UDP) |
| `close(sock)` | Close socket |
| `setsockopt(sock, level, opt, value)` | Set socket option |
| `getsockopt(sock, level, opt)` | Get socket option |
| `settimeout(sock, seconds)` | Set timeout |

## Error Handling

```zen
import socket

try {
    let s = socket.create(socket.AF_INET, socket.SOCK_STREAM)
    socket.connect(s, "invalid.host.example", 80)
} catch (e) {
    print("Socket error: " + e)
    if e.contains("timeout") {
        print("Connection timed out")
    }
}
```

## Practical Example: Simple Chat Server

```zen
import socket

let server = socket.create(socket.AF_INET, socket.SOCK_STREAM)
socket.setsockopt(server, socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
socket.bind(server, "0.0.0.0", 9000)
socket.listen(server, 10)

print("Chat server on port 9000")

let clients = []

while true {
    let client = socket.accept(server)
    clients.push(client)
    print("New client connected")
    
    # Handle client in separate thread
    spawn(fn() {
        while true {
            try {
                let msg = socket.recv(client, 1024)
                if len(msg) == 0 {
                    break
                }
                print("Broadcasting: " + msg)
                # Broadcast to all clients
                for c in clients {
                    if c != client {
                        try {
                            socket.send(c, msg)
                        } catch {}
                    }
                }
            } catch {
                break
            }
        }
        socket.close(client)
    })
}
```

## Best Practices

1. **Always close sockets** - Use try/finally or track closures
2. **Handle errors** - Network operations can fail
3. **Set timeouts** - Avoid hanging indefinitely
4. **Reuse addresses** - Use SO_REUSEADDR for servers
5. **Use threads** - Handle multiple clients concurrently
6. **Validate data** - Don't trust received data
7. **Consider using high-level APIs** - `requests` is easier for HTTP

## When to Use Low-Level Sockets

Use the `socket` module when:
- You need custom protocols
- Working with non-HTTP protocols
- Need fine-grained control
- Building custom servers/clients
- Working with UDP/TCP directly

Use higher-level modules (`requests`, `http`) when:
- Working with HTTP/HTTPS
- Simpler use cases
- Less error-prone

## Chapter Summary

- `socket` module provides BSD socket API
- TCP uses SOCK_STREAM, UDP uses SOCK_DGRAM
- Client: create → connect → send/recv → close
- Server: create → bind → listen → accept → send/recv → close
- Use `sendto`/`recvfrom` for UDP
- Handle errors and use threading for concurrent clients
- Set socket options for flexibility

**Next:** Chapter 28 - HTTP Protocol
