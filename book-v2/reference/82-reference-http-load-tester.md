# Chapter 82: Reference Implementation — HTTP Load Tester (Benchmark)

A simple HTTP load tester: it fires N concurrent requests against a URL using a
thread pool, measures response times, and reports success count, failure count,
min/avg/max latency, and requests per second. Demonstrates `requests`,
threads, channels, timing, and basic statistics.

## 82.1 The program (`loadtest.z`)

```zen
import requests
import threading
import channels
import time
import sys
import math

var results = channels.Channel(capacity=10000)
let done = channels.Channel(capacity=1)

func worker(url, n_requests) {
    var my_ok = 0
    var my_fail = 0
    var my_min = nil
    var my_max = nil
    var my_total_ms = 0.0
    var i = 0
    while i < n_requests {
        let start = time.time()
        let ok = false
        try {
            let r = requests.get(url, timeout=5000)
            ok = r.status_code >= 200 and r.status_code < 400
        } catch as e {
            ok = false
        }
        let elapsed_ms = (time.time() - start) * 1000
        my_total_ms += elapsed_ms
        if my_min == nil or elapsed_ms < my_min { my_min = elapsed_ms }
        if my_max == nil or elapsed_ms > my_max { my_max = elapsed_ms }
        if ok { my_ok += 1 } else { my_fail += 1 }
        i += 1
    }
    results.send(#{ok: my_ok, fail: my_fail, min_ms: my_min,
                   max_ms: my_max, total_ms: my_total_ms, count: n_requests})
}

func fmt_ms(x) {
    if x == nil { return "--" }
    return str(math.round(x)) + " ms"
}

func main(argv) {
    if len(argv) < 3 {
        print("Usage: zen run loadtest.z <url> <total_requests> [concurrency]")
        print("  Example: zen run loadtest.z http://localhost:8080/ 1000 10")
        return 1
    }
    let url = argv[1]
    let total = int(argv[2])
    let concurrency = int(argv[3] ?? "10")
    if concurrency < 1 { concurrency = 1 }
    if concurrency > total { concurrency = total }

    let per_worker = total // concurrency
    let remainder = total % concurrency

    print("load testing " + url)
    print("  total requests: " + str(total))
    print("  concurrency:    " + str(concurrency))
    print("  starting...")
    let start = time.time()

    let threads = []
    var w = 0
    while w < concurrency {
        let n = per_worker + (if w < remainder { 1 } else { 0 })
        threads.push(threading.Thread(fn() { worker(url, n) }))
        w += 1
    }
    for t in threads { t.start() }

    # collect results
    var total_ok = 0
    var total_fail = 0
    var all_min = nil
    var all_max = nil
    var total_ms = 0.0
    var total_count = 0
    var collected = 0
    while collected < concurrency {
        let r = results.recv()
        if r == nil { break }
        collected += 1
        total_ok += r["ok"]
        total_fail += r["fail"]
        total_ms += r["total_ms"]
        total_count += r["count"]
        if r["min_ms"] != nil and (all_min == nil or r["min_ms"] < all_min) {
            all_min = r["min_ms"]
        }
        if r["max_ms"] != nil and (all_max == nil or r["max_ms"] > all_max) {
            all_max = r["max_ms"]
        }
    }

    let elapsed = time.time() - start
    print("----")
    print("completed in " + str(math.round(elapsed * 1000)) + " ms")
    print("  ok:      " + str(total_ok))
    print("  failed:  " + str(total_fail))
    print("  rps:     " + str(math.round(total_count / elapsed)))
    if all_min != nil {
        print("  min:     " + fmt_ms(all_min))
        print("  max:     " + fmt_ms(all_max))
        print("  avg:     " + fmt_ms(total_ms / total_count))
    }
    return if total_fail == 0 { 0 } else { 1 }
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 82.2 Example session

```
$ zen run loadtest.z http://localhost:8080/ 1000 10
load testing http://localhost:8080/
  total requests: 1000
  concurrency:    10
  starting...
----
completed in 245 ms
  ok:      1000
  failed:  0
  rps:     4081
  min:     1 ms
  max:     45 ms
  avg:     3 ms
```

## 82.3 Design notes

- **Thread pool:** `concurrency` workers each handle a share of the total
  requests; results are collected through a channel.
- **Per-worker stats:** each worker tracks its own min/max/total to avoid a
  shared lock on every request.
- **Timeouts:** every request has a 5-second timeout so a hung endpoint doesn't
  stall the whole test.
- **Not a production load generator:** for serious testing use dedicated tools
  (hey, wrk, k6). This is a teaching example.

## 82.4 Exercises

1. Add `--method POST` with a body.
2. Add percentile reporting (p50, p95, p99) by collecting all latencies.
3. Add `--duration N` to run for N seconds instead of N requests.
4. Add `--output <file>` to write results as JSON.
5. Add a `--ramp-up` that gradually increases concurrency.
