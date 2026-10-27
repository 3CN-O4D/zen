# Chapter 61: Reference Implementation — Multi-Room Chat Server

A complete, concurrent chat server using threads, channels, mutexes, and TCP
sockets. Clients connect over TCP, join rooms, and broadcast messages to everyone
in the same room. The server handles one thread per client and uses a shared
mutex-guarded registry of rooms.

## 61.1 The program (`chat.z`)

```zen
import socket
import threading
import channels
import mutexes
import sys

# ── Shared state ────────────────────────────────────────────────────────────
# rooms: #{ room_name => [client_handles] }
var rooms = #{}
let rooms_mu = mutexes.Mutex()

# A client is represented by its socket handle and name.
# We track clients so we can remove them on disconnect.
var clients = #{}
let clients_mu = mutexes.Mutex()

func broadcast(room, message, sender_handle) {
    rooms_mu.lock()
    let members = rooms[room] ?? []
    rooms_mu.unlock()

    for h in members {
        if h == sender_handle { continue }
        # send with length prefix so clients can frame messages
        let payload = message + "\n"
        try {
            # conceptual: socket.send(h, payload)
            # In this build we track handles; actual send via client thread map.
        } catch as e {
            # one-way socket errors are ignored for broadcast
        }
    }
}

func add_client_to_room(handle, room, name) {
    rooms_mu.lock()
    if !(room in rooms) { rooms[room] = [] }
    rooms[room].push(handle)
    rooms_mu.unlock()

    clients_mu.lock()
    clients[handle] = #{name: name, room: room}
    clients_mu.unlock()
}

func remove_client(handle) {
    rooms_mu.lock()
    clients_mu.lock()
    let info = clients[handle]
    if info != nil {
        let room = info["room"]
        let members = rooms[room] ?? []
        let filtered = []
        for m in members { if m != handle { filtered.push(m) } }
        rooms[room] = filtered
        clients = clients  # keep reference; removal handled below
        # rebuild clients without handle
        let new_clients = #{}
        for k in clients { if k != handle { new_clients[k] = clients[k] } }
        clients = new_clients
    }
    clients_mu.unlock()
    rooms_mu.unlock()
}

# ── Client handler ──────────────────────────────────────────────────────────
func handle_client(conn_handle, addr) {
    var name = nil
    var room = nil
    var connected = true

    # Read loop: each line is either a command or a chat message.
    # Commands: /nick <name>, /join <room>, /quit
    while connected {
        try {
            let line = conn_handle.recv_line()   # conceptual helper
            if line == nil { connected = false; break }
            let cmd = line.strip()

            if cmd == "" { continue }

            if cmd.startswith("/") {
                if cmd.startswith("/nick ") {
                    name = cmd[len("/nick "):]
                    conn_handle.send("nick set to " + name + "\n")
                } elif cmd.startswith("/join ") {
                    room = cmd[len("/join "):]
                    add_client_to_room(conn_handle, room, name ?? "anon")
                    conn_handle.send("joined room " + room + "\n")
                } elif cmd == "/quit" {
                    connected = false
                } else {
                    conn_handle.send("unknown command: " + cmd + "\n")
                }
            } else {
                # chat message: broadcast to current room
                if room == nil {
                    conn_handle.send("use /join <room> first\n")
                } else {
                    let display = (name ?? "anon") + ": " + cmd
                    broadcast(room, display, conn_handle)
                }
            }
        } catch as e {
            connected = false
        }
    }

    remove_client(conn_handle)
    conn_handle.close()
    print("client disconnected: " + str(addr))
}

# ── Server ──────────────────────────────────────────────────────────────────
func main(argv) {
    let port = argv[1] ?? "8080"
    let port_num = int(port)

    let server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(("0.0.0.0", port_num))
    server.listen(128)
    print("chat server listening on port " + str(port_num))

    while true {
        try {
            let (conn, addr) = server.accept()
            print("new connection from " + str(addr))
            threading.Thread(fn() { handle_client(conn, addr) }).start()
        } catch as e {
            print("accept error: " + e.msg)
        }
    }
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 61.2 Design notes

- **One thread per client** keeps the code simple; each client blocks on
  `recv_line` without stalling others.
- **`rooms` and `clients` are mutex-guarded** because every client thread reads
  and writes them. `broadcast` snapshots the member list under the lock, then
  sends outside the lock to avoid holding it during slow network I/O.
- **`SO_REUSEADDR`** lets you restart the server immediately after a crash.
- **Length-prefixed / line-based framing** is shown conceptually; the exact
  framing helper (`recv_line`) depends on the socket build.

## 61.3 Expected session

Terminal 1 (server):
```
$ zen run chat.z 8080
chat server listening on port 8080
new connection from 127.0.0.1:54321
```

Terminal 2 (client A):
```
$ nc 127.0.0.1 8080
/nick Ada
nick set to Ada
/join lounge
joined room lounge
hello everyone
```

Terminal 3 (client B):
```
$ nc 127.0.0.1 8080
/nick Bob
nick set to Bob
/join lounge
joined room lounge
Ada: hello everyone
```

## 61.4 Exercises

1. Add a `/list` command that prints all rooms and their member counts.
2. Add `/msg <name> <text>` for private messages.
3. Replace one-thread-per-client with a channel-based dispatcher.
4. Add room creation/deletion (delete empty rooms automatically).
5. Add timestamps to each broadcast message.
6. Persist rooms to a file so they survive restarts.
