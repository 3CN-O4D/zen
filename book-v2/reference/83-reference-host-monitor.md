# Chapter 83: Reference Implementation — Host Availability Monitor

A tool that checks whether multiple hosts:ports are reachable (TCP connect) and
prints a status table. It checks hosts concurrently with a thread pool and a
per-connect timeout. Useful for verifying that your services are up.

## 83.1 The program (`hostmon.z`)

```zen
import socket
import threading
import mutexes
import time
import sys
import math

var results = #{}
let results_mu = mutexes.Mutex()

func check_host(host, port) {
    let s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.settimeout(2000)   # 2 second timeout
    let up = false
    let ms = nil
    let start = time.time()
    try {
        s.connect((host, port))
        up = true
    } catch as e {
        up = false
    } finally {
        s.close()
    }
    let elapsed = (time.time() - start) * 1000
    results_mu.lock()
    results[host + ":" + str(port)] = #{up: up, ms: elapsed}
    results_mu.unlock()
}

func main(argv) {
    if len(argv) < 2 {
        print("Usage: zen run hostmon.z <host:port> [<host:port> ...]")
        print("  Example: zen run hostmon.z localhost:8080 example.com:443 127.0.0.1:5432")
        return 1
    }
    let targets = []
    var i = 1
    while i < len(argv) {
        let parts = argv[i].split(":")
        if len(parts) != 2 {
            print("invalid target (expected host:port): " + argv[i])
        } else {
            targets.push(#{host: parts[0], port: int(parts[1])})
        }
        i += 1
    }
    if len(targets) == 0 { return 1 }

    print("checking " + str(len(targets)) + " host(s)")
    print("========================")
    print(" HOST:PORT         STATUS   TIME")
    print(" --------------------  ------  ----")

    let threads = []
    for t in targets {
        let h = t["host"]
        let p = t["port"]
        threads.push(threading.Thread(fn() { check_host(h, p) }))
    }
    for t in threads { t.start() }
    for t in threads { t.join() }

    # sort and print
    let keys = []
    for k in results { keys.push(k) }
    keys.sort()
    var any_down = false
    for k in keys {
        let r = results[k]
        let status = if r["up"] { "UP" } else { "DOWN" }
        if !r["up"] { any_down = true }
        let ms = if r["ms"] == nil { "--" } else { str(math.round(r["ms"])) + " ms" }
        print(" " + k + "   " + status + "   " + ms)
    }
    print("========================")
    print(any_down ? "some hosts are DOWN" : "all hosts are UP")
    return if any_down { 1 } else { 0 }
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 83.2 Example session

```
$ zen run hostmon.z localhost:8080 example.com:443 127.0.0.1:9999
checking 3 host(s)
========================
 HOST:PORT         STATUS   TIME
 --------------------  ------  ----
 127.0.0.1:9999     DOWN    --
 example.com:443     UP      45 ms
 localhost:8080      UP      2 ms
========================
some hosts are DOWN
```

Exit code is 1 if any host is down — useful in shell scripts and CI.

## 83.3 Design notes

- **Concurrent checks:** each host:port is checked in its own thread so slow
  or blocked hosts don't serialise the whole run.
- **Per-connect timeout:** 2 seconds prevents a filtered/dropped connection
  from hanging indefinitely.
- **Exit code:** 0 if all up, 1 if any down — suitable for monitoring scripts.
- **TCP connect only:** this checks port reachability, not application health.
  For HTTP health, combine with chapter 65 (HTTP monitor).

## 83.4 Exercises

1. Add `--timeout N` to configure the connect timeout.
2. Add `--interval N` to re-check every N seconds (watch mode).
3. Add `--output <file>` to write results as JSON.
4. Add SMTP/HTTP-specific checks (send `HELO` or `GET` and validate response).
5. Add a configuration file listing hosts to check.
