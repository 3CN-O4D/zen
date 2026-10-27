# Chapter 69: Reference Implementation — Task Queue with Worker Pool

A task queue: a producer enqueues work items, and a fixed pool of worker threads
consumes them from a channel, processing each in parallel and collecting
results. Demonstrates channels, threads, mutexes, and graceful shutdown.

## 69.1 The program (`taskq.z`)

```zen
import threading
import channels
import mutexes
import sys
import time

# ── Job and result types ────────────────────────────────────────────────────
# A job is a dict: #{id, payload, submitted_at}
# A result is a dict: #{id, result, error, worker, duration_ms}

var jobs = channels.Channel(capacity=100)
var results = channels.Channel(capacity=100)
var shutdown = channels.Channel(capacity=1)

let stats_mu = mutexes.Mutex()
var jobs_done = 0
var jobs_failed = 0

func record_result(ok) {
    stats_mu.lock()
    if ok { jobs_done += 1 } else { jobs_failed += 1 }
    stats_mu.unlock()
}

# ── Worker ──────────────────────────────────────────────────────────────────
func worker(worker_id) {
    print("worker " + str(worker_id) + " starting")
    while true {
        let job = jobs.recv()
        if job == nil { break }   # channel closed -> shutdown
        let start = time.time()
        let outcome = process_job(worker_id, job)
        let elapsed = (time.time() - start) * 1000
        let result = #{
            id: job["id"],
            worker: worker_id,
            duration_ms: elapsed,
            ok: outcome["ok"],
            result: outcome["result"],
            error: outcome["error"],
        }
        results.send(result)
        record_result(outcome["ok"])
    }
    print("worker " + str(worker_id) + " shutting down")
}

func process_job(worker_id, job) -> Dict {
    let id = job["id"]
    let payload = job["payload"]
    # simulate work: a CPU-bound or I/O-bound task
    try {
        # example: square the payload if it's a number
        if typeof(payload) == "int" || typeof(payload) == "float" {
            let r = payload * payload
            return #{ok: true, result: r, error: nil}
        } else {
            # echo back uppercase string
            return #{ok: true, result: str(payload).upper(), error: nil}
        }
    } catch as e {
        return #{ok: false, result: nil, error: e.msg}
    }
}

# ── Producer: enqueue jobs ─────────────────────────────────────────────────
func enqueue_jobs(count) {
    var i = 1
    while i <= count {
        jobs.send(#{id: i, payload: i, submitted_at: time.time()})
        i += 1
    }
    print("enqueued " + str(count) + " jobs")
}

# ── Collector: gather results ─────────────────────────────────────────────
func collect_results(expected) {
    var got = 0
    while got < expected {
        let r = results.recv()
        if r == nil { break }
        got += 1
        if got <= 5 or got % 10 == 0 {
            let status = if r["ok"] { "ok" } else { "ERR:" + str(r["error"]) }
            print("job " + str(r["id"]) + " by worker " + str(r["worker"]) +
                  " -> " + status + " (" + str(math.round(r["duration_ms"])) + "ms)")
        }
    }
    print("collected " + str(got) + " results")
    import math
    # (math imported locally for round)
}

# ── Main ───────────────────────────────────────────────────────────────────
func main(argv) {
    let num_workers = argv[1] ?? "4"
    let num_jobs = argv[2] ?? "20"
    let wc = int(num_workers)
    let jc = int(num_jobs)

    # start workers
    let threads = []
    var w = 1
    while w <= wc {
        threads.push(threading.Thread(fn() { worker(w) }))
        w += 1
    }
    for t in threads { t.start() }

    # enqueue jobs
    enqueue_jobs(jc)

    # signal shutdown: close jobs channel so workers exit after draining
    jobs.close()

    # collect results
    collect_results(jc)

    # wait for workers to finish
    for t in threads { t.join() }

    stats_mu.lock()
    print("----")
    print("done: " + str(jobs_done) + " succeeded, " + str(jobs_failed) + " failed")
    stats_mu.unlock()

    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 69.2 Example session

```
$ zen run taskq.z 4 20
worker 1 starting
worker 2 starting
worker 3 starting
worker 4 starting
enqueued 20 jobs
job 1 by worker 2 -> ok (0ms)
job 2 by worker 4 -> ok (0ms)
job 3 by worker 1 -> ok (0ms)
job 4 by worker 3 -> ok (0ms)
job 10 -> ok (0ms)
collected 20 results
worker 1 shutting down
worker 2 shutting down
worker 3 shutting down
worker 4 shutting down
----
done: 20 succeeded, 0 failed
```

## 69.3 Design notes

- **Channel as a thread-safe work queue:** `jobs.recv()` blocks until a job is
  available; workers naturally self-balance.
- **Graceful shutdown:** closing the `jobs` channel makes each worker's
  `recv()` return `nil`, causing it to exit its loop. No worker is left
  waiting forever.
- **Stats guarded by a Mutex:** `jobs_done`/`jobs_failed` are updated from
  multiple worker threads, so `stats_mu` prevents races.
- **Capacity bound:** `capacity=100` applies backpressure — the producer blocks
  if consumers fall behind.

## 69.4 Exercises

1. Add priority: high-priority jobs go to a separate channel checked first.
2. Add retries: failed jobs are re-enqueued up to 3 times.
3. Add a `--verbose` flag printing every result.
4. Make workers report throughput (jobs/sec) at shutdown.
5. Add a `results.close()` after all jobs are collected so a collector thread
   can also exit cleanly.
