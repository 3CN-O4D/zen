# socket — Low-level networking

The `socket` module provides raw TCP and UDP networking capabilities. It is available globally as `socket`.

## TCP Client

Connecting to a server and sending/receiving data:

```zen
var s = socket.open("example.com", 80)
s.send("GET / HTTP/1.1\r\nHost: example.com\r\n\r\n")

var resp = s.recv(1024)
print(resp)
s.close()
```

## TCP Server

Listening for incoming connections:

```zen
var server = socket.listen("0.0.0.0", 9000)
print("Listening on 9000...")

while true {
    var client = socket.accept(server)
    print("Client connected!")
    client.send("Hello from Zen server!\n")
    client.close()
}
```

## Functions

| Function | Description |
|----------|-------------|
| `open(host, port)` | Opens a TCP connection to the host. Returns a socket dict. |
| `listen(host, port)` | Creates a TCP server listening on the port. |
| `accept(server)` | Waits for a client connection on a listening socket. Returns a client socket. |
| `send(socket, data)` | Sends a string through the socket. (Also available as `s.send(data)`). |
| `recv(socket, bytes)` | Receives up to N bytes from the socket. (Also available as `s.recv(n)`). |
| `close(socket)` | Closes the connection. (Also available as `s.close()`). |
| `scan(host, port_range)` | Scans a range of ports (e.g., "1-1000") for the host. |
| `set_timeout(socket, ms)` | Sets the read/write timeout in milliseconds. |

## UDP Networking

```zen
var udp = socket.open_udp()
socket.send_to(udp, "1.1.1.1", 53, "ping")
var resp = socket.recv_from(udp, 1024)
print(resp.data)
```

## See Also
- [http](http.md) — Higher-level HTTP client.
- [dns](dns.md) — DNS resolution.
