# Socket Module (`socket`)

TCP sockets.

```zen
let s = socket.open("host", 80)
socket.send(s, "data")
let data = socket.recv(s, 1024)
socket.close(s)
```
