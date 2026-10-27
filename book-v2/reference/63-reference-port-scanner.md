# Chapter 63: Reference Implementation — TCP Port Scanner (Educational)

A small TCP port scanner that checks which ports are open on a target host. It
is strictly educational: only scan hosts you own or have explicit permission to
test. It demonstrates sockets, timeouts, concurrency (one thread per port
batch), and CLI argument handling.

## 63.1 The program (`scan.z`)

```zen
import socket
import threading
import mutexes
import sys
import time

# ── Shared results ──────────────────────────────────────────────────────────
var open_ports = []
let results_mu = mutexes.Mutex()
let print_mu = mutexes.Mutex()

func scan_port(host, port) {
    let s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(800)   # 800 ms
    let result = "closed"
    try {
        s.connect((host, port))
        result = "open"
    } catch as e {
        # connection refused / timeout / unreachable -> closed
    } finally {
        s.close()
    }

    results_mu.lock()
    if result == "open" {
        open_ports.push(port)
    }
    results_mu.unlock()

    print_mu.lock()
    print("port " + str(port) + ": " + result)
    print_mu.unlock()
}

func scan_range(host, start_port, end_port, num_threads) {
    print("scanning " + host + " ports " + str(start_port) + "-" + str(end_port))
    let ports = []
    var p = start_port
    while p <= end_port {
        ports.push(p)
        p += 1
    }

    # simple work queue: split ports into batches, one thread per batch
    let batch_size = (len(ports) + num_threads - 1) // num_threads
    let threads = []
    var bi = 0
    while bi < len(ports) {
        let batch = []
        var i = 0
        while i < batch_size and bi + i < len(ports) {
            batch.push(ports[bi + i])
            i += 1
        }
        bi += batch_size
        let b = batch   # capture for closure
        threads.push(threading.Thread(fn() {
            for port in b { scan_port(host, port) }
        }))
    }

    for t in threads { t.start() }
    for t in threads { t.join() }

    # sort and report
    open_ports.sort()   # if supported; else sort manually
    print_mu.lock()
    print("----")
    print("scan complete: " + str(len(open_ports)) + " open port(s)")
    for port in open_ports { print("  open: " + str(port)) }
    print_mu.unlock()
}

func main(argv) {
    if len(argv) < 3 {
        print("Usage: zen run scan.z <host> <start-port> [end-port] [threads]")
        print("  Example: zen run scan.z 127.0.0.1 1 1024 50")
        return 1
    }
    let host = argv[1]
    let start_port = int(argv[2])
    let end_port = argv[3] ?? argv[2]
    let end_port_num = int(end_port)
    let threads = argv[4] ?? "50"
    let thread_count = int(threads)

    if thread_count < 1 { thread_count = 1 }
    if thread_count > 200 { thread_count = 200 }

    scan_range(host, start_port, end_port_num, thread_count)
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 63.2 Example session

```
$ zen run scan.z 127.0.0.1 80 90 10
scanning 127.0.0.1 ports 80-90
port 80: open
port 81: closed
...
port 8080: open
----
scan complete: 2 open port(s)
  open: 80
  open: 8080
```

## 63.3 Design notes

- **Timeouts matter:** without `settimeout`, a scan of a filtered port can hang
  for minutes. 800 ms per port keeps the scan bounded.
- **Mutex-protected output:** `print_mu` prevents interleaved lines from
  different threads.
- **Thread cap:** limited to 200 threads to avoid file-descriptor exhaustion.
- **Educational use only:** scanning hosts you don't own is illegal in many
  jurisdictions and violates acceptable-use policies.

## 63.4 Exercises

1. Add banner grabbing: after connecting, read the first bytes to identify the
   service (HTTP, SSH, etc.).
2. Add `-o <file>` to write results to a file.
3. Add concurrent resolution: resolve host to IP before scanning.
4. Add a `--top-ports` mode scanning only the 100 most common ports.
5. Add rate limiting (a small delay between connection attempts).
