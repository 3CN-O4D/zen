# Socket Module (`socket`)

TCP sockets.

```zen
let s = socket.open("example.com", 80)
socket.send(s, "GET / HTTP/1.1\r\nHost: example.com\r\n\r\n")
let data = socket.recv(s, 1024)
socket.close(s)
```
