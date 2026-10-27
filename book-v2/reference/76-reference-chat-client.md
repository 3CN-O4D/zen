# Chapter 76: Reference Implementation — TCP Chat Client

A command-line chat client that connects to the chat server (chapter 61),
lets the user set a nickname, join a room, and send/receive messages. It uses a
separate thread to read incoming messages while the main thread reads user
input. Demonstrates sockets, threading, and concurrent I/O.

## 76.1 The program (`chatclient.z`)

```zen
import socket
import threading
import sys

let running = true
let sock = nil

func reader() {
    while running {
        try {
            let data = sock.recv(4096)
            if data == nil or len(data) == 0 {
                print("\n[disconnected]")
                running = false
                break
            }
            print("\n>" + data + "\n> ", end="")
        } catch as e {
            if running { print("\n[read error: " + e.msg + "]") }
            running = false
            break
        }
    }
}

func main(argv) {
    let host = argv[1] ?? "127.0.0.1"
    let port = int(argv[2] ?? "8080")

    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(5000)
    try {
        sock.connect((host, port))
    } catch as e {
        print("connect failed: " + e.msg)
        return 1
    }
    print("connected to " + host + ":" + str(port))
    print("commands: /nick <name> | /join <room> | /quit")
    print("type a message and press enter to send")

    # background thread reads incoming messages
    let reader_thread = threading.Thread(fn() { reader() })
    reader_thread.start()

    # main thread reads user input
    while running {
        let line = read_line()   # conceptual: read a line from stdin
        if line == nil { break }
        let cmd = line.strip()
        if cmd == "" { continue }
        if cmd == "/quit" {
            running = false
            break
        }
        try {
            sock.send(cmd + "\n")
        } catch as e {
            print("send failed: " + e.msg)
            running = false
            break
        }
    }

    running = false
    try { sock.close() } catch as e {}
    reader_thread.join()
    print("goodbye")
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 76.2 Example session

```
$ zen run chatclient.z 127.0.0.1 8080
connected to 127.0.0.1:8080
commands: /nick <name> | /join <room> | /quit
type a message and press enter to send
> /nick Ada
> /join lounge
> hello everyone
> [Bob] hi Ada
> [Carol] welcome to lounge
> /quit
goodbye
```

The `[...]` prefixes are messages received from other clients via the server,
printed by the background reader thread while the user continues typing.

## 76.3 Design notes

- **Concurrent I/O:** a dedicated `reader` thread blocks on `recv()` so the
  main thread can keep reading user input without missing incoming messages.
- **`running` flag:** coordinates shutdown between threads; when set to false,
  both loops exit and the socket is closed.
- **Line-based protocol:** each client message is sent with a trailing newline;
  the server (chapter 61) reads lines and dispatches commands/chat.

## 76.4 Exercises

1. Add message timestamps to received messages.
2. Add colors to distinguish your own messages from others'.
3. Add `/rooms` to list available rooms (server must support it).
4. Add reconnection on disconnect (exponential backoff).
5. Add a `--name` flag to set the nickname without the `/nick` command.
