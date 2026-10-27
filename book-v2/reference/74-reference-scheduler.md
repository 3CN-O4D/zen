# Chapter 74: Reference Implementation — Simple Task Scheduler (Cron-like)

A scheduler that runs registered tasks at fixed intervals in the background,
using threads and `time.sleep`. Tasks are Zen functions; the scheduler keeps
running until interrupted. Demonstrates threads, timing, and a simple task
registry.

## 74.1 The program (`sched.z`)

```zen
import threading
import time
import sys
import mutexes

# ── Task registry ───────────────────────────────────────────────────────────
var tasks = #{}
let tasks_mu = mutexes.Mutex()
let stop = channels.Channel(capacity=1)

func every(interval_sec, name, fn) {
    tasks_mu.lock()
    tasks[name] = #{fn: fn, interval_ms: interval_sec * 1000, next_run: time.time()}
    tasks_mu.unlock()
    print("scheduled '" + name + "' every " + str(interval_sec) + "s")
}

func unschedule(name) {
    tasks_mu.lock()
    if name in tasks {
        # rebuild without name
        let new_tasks = #{}
        for k in tasks { if k != name { new_tasks[k] = tasks[k] } }
        tasks = new_tasks
    }
    tasks_mu.unlock()
}

# ── Scheduler loop ──────────────────────────────────────────────────────────
func scheduler_loop() {
    print("scheduler started")
    while true {
        # check for stop signal (non-blocking would be ideal; here we use a
        # short sleep so shutdown is prompt)
        time.sleep(250)
        let now = time.time()
        tasks_mu.lock()
        let names = []
        for n in tasks { names.push(n) }
        tasks_mu.unlock()
        for n in names {
            tasks_mu.lock()
            let t = tasks[n]
            tasks_mu.unlock()
            if t != nil and now >= t["next_run"] {
                try {
                    t["fn"]()
                } catch as e {
                    print("task '" + n + "' failed: " + e.msg)
                }
                tasks_mu.lock()
                if n in tasks {
                    tasks[n]["next_run"] = now + (tasks[n]["interval_ms"] / 1000)
                }
                tasks_mu.unlock()
            }
        }
    }
}

# ── Sample tasks ────────────────────────────────────────────────────────────
func heartbeat() {
    print("[" + time.strftime("%H:%M:%S", time.now()) + "] heartbeat")
}

func cleanup() {
    print("[" + time.strftime("%H:%M:%S", time.now()) + "] cleanup tick")
}

func main(argv) {
    # register sample tasks
    every(1, "heartbeat", heartbeat)
    every(3, "cleanup", cleanup)

    # start scheduler in a background thread
    let sched = threading.Thread(fn() { scheduler_loop() })
    sched.start()

    print("scheduler running. press Ctrl-C to stop.")
    print("main thread is free to do other work...")

    # keep main alive
    while true {
        time.sleep(1000)
    }
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 74.2 Example session

```
$ zen run sched.z
scheduled 'heartbeat' every 1s
scheduled 'cleanup' every 3s
scheduler started
scheduler running. press Ctrl-C to stop.
main thread is free to do other work...
[10:00:01] heartbeat
[10:00:02] heartbeat
[10:00:03] heartbeat
[10:00:03] cleanup tick
[10:00:04] heartbeat
[10:00:05] heartbeat
[10:00:06] heartbeat
[10:00:06] cleanup tick
... (Ctrl-C to stop)
```

## 74.3 Design notes

- **Polling scheduler:** every 250 ms it checks which tasks are due and runs
  them. Simple and dependency-free; not precise for sub-second timing.
- **Task registry guarded by a Mutex:** tasks can be added/removed while the
  scheduler runs (though this sample only adds at startup).
- **Errors isolated:** a failing task is caught and logged, so one bad task
  doesn't kill the scheduler.
- **Main thread stays free:** the scheduler runs in a background thread, so
  the main thread can accept connections, serve HTTP, etc.

## 74.4 Exercises

1. Add cron-style expressions (e.g., "0 * * * *") instead of fixed intervals.
2. Add task results: collect each task's return value into a results channel.
3. Add `--once` mode: run all due tasks once and exit.
4. Persist the schedule to a file so it survives restarts.
5. Add a `stop` channel so the scheduler exits cleanly on signal.
