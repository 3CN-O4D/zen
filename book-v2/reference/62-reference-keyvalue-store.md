# Chapter 62: Reference Implementation — Persistent Key-Value Store

A small, persistent key-value store with a TCP interface. Clients send commands
(`GET`, `SET`, `DEL`, `KEYS`) over TCP; the server stores data in an in-memory
dict and periodically snapshots it to disk as JSON. It demonstrates file I/O,
JSON serialization, concurrency, sockets, and error handling.

## 62.1 The program (`kv.z`)

```zen
import socket
import threading
import mutexes
import json
import os
import pathlib
import sys
import time

# ── In-memory store ─────────────────────────────────────────────────────────
var store = #{}
let store_mu = mutexes.Mutex()

let SNAPSHOT_FILE = "kv_snapshot.json"
let SNAPSHOT_INTERVAL_SEC = 30

func kv_get(key) {
    store_mu.lock()
    let v = store[key]
    store_mu.unlock()
    return v
}

func kv_set(key, value) {
    store_mu.lock()
    store[key] = value
    store_mu.unlock()
}

func kv_del(key) {
    store_mu.lock()
    let existed = key in store
    if existed {
        # rebuild without key
        let new_store = #{}
        for k in store { if k != key { new_store[k] = store[k] } }
        store = new_store
    }
    store_mu.unlock()
    return existed
}

func kv_keys() {
    store_mu.lock()
    let ks = []
    for k in store { ks.push(k) }
    store_mu.unlock()
    return ks
}

func kv_snapshot() {
    store_mu.lock()
    let copy = #{}
    for k in store { copy[k] = store[k] }
    store_mu.unlock()
    let tmp = SNAPSHOT_FILE + ".tmp"
    json.save(tmp, copy)
    os.rename(tmp, SNAPSHOT_FILE)
    print("snapshot written to " + SNAPSHOT_FILE)
}

func kv_load_snapshot() {
    let p = pathlib.Path(SNAPSHOT_FILE)
    if p.exists() {
        try {
            let data = json.load(SNAPSHOT_FILE)
            store_mu.lock()
            store = data
            store_mu.unlock()
            print("loaded " + str(len(kv_keys())) + " keys from snapshot")
        } catch as e {
            print("could not load snapshot: " + e.msg)
        }
    }
}

# ── Snapshot background thread ─────────────────────────────────────────────
func snapshot_loop() {
    while true {
        time.sleep(SNAPSHOT_INTERVAL_SEC * 1000)
        kv_snapshot()
    }
}

# ── Client handler ─────────────────────────────────────────────────────────
func handle_client(conn, addr) {
    print("kv client connected: " + str(addr))
    var connected = true
    while connected {
        try {
            let line = conn.recv_line()
            if line == nil { connected = false; break }
            let parts = line.strip().split(" ")
            if len(parts) == 0 { continue }
            let cmd = parts[0].upper()

            if cmd == "GET" and len(parts) == 2 {
                let v = kv_get(parts[1])
                if v == nil { conn.send("END\n") }
                else { conn.send("VALUE " + str(v) + "\n") }
            } elif cmd == "SET" and len(parts) == 3 {
                kv_set(parts[1], parts[2])
                conn.send("OK\n")
            } elif cmd == "DEL" and len(parts) == 2 {
                let existed = kv_del(parts[1])
                if existed { conn.send("OK\n") } else { conn.send("NOT_FOUND\n") }
            } elif cmd == "KEYS" {
                let ks = kv_keys()
                conn.send("COUNT " + str(len(ks)) + "\n")
                for k in ks { conn.send(k + "\n") }
                conn.send("END\n")
            } elif cmd == "QUIT" {
                connected = false
            } else {
                conn.send("ERROR unknown command\n")
            }
        } catch as e {
            connected = false
        }
    }
    conn.close()
    print("kv client disconnected: " + str(addr))
}

# ── Server ─────────────────────────────────────────────────────────────────
func main(argv) {
    let port = argv[1] ?? "9090"
    let port_num = int(port)

    kv_load_snapshot()

    # start background snapshot thread
    threading.Thread(fn() { snapshot_loop() }).start()

    let server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(("0.0.0.0", port_num))
    server.listen(128)
    print("kv store listening on port " + str(port_num))

    while true {
        try {
            let (conn, addr) = server.accept()
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

## 62.2 Example session

Terminal 1 (server):
```
$ zen run kv.z 9090
loaded 0 keys from snapshot
kv store listening on port 9090
kv client connected: 127.0.0.1:54321
```

Terminal 2 (client, using `nc`):
```
$ nc 127.0.0.1 9090
SET foo bar
OK
GET foo
VALUE bar
GET missing
END
DEL foo
OK
GET foo
END
KEYS
COUNT 0
END
QUIT
```

## 62.3 Design notes

- The store dict is mutex-guarded; reads snapshot the value under the lock and
  return it, so callers get a consistent view.
- `kv_del` rebuilds the dict without the key (Zen dicts are not mutable-delete
  in this build in a lock-safe way; the rebuild keeps the lock held briefly).
- Snapshots run in a background thread every 30 seconds; a load happens at
  startup.
- JSON is used for the on-disk format, which is human-readable and portable.

## 62.4 Exercises

1. Add a `FLUSH` command that clears the entire store.
2. Add value expiration (TTL) with a lazy cleanup on read.
3. Add persistence on every `SET` (append-only log) instead of periodic snapshots.
4. Add authentication (a `PASS` command before other commands).
5. Add a `STATS` command reporting total keys, memory estimate, uptime.
