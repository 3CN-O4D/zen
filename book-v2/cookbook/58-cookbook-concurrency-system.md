# Chapter 58: Cookbook — Concurrency & System

Complete recipes for threads, channels, mutexes, and system operations.

## 58.1 Recipe: Run N functions in parallel and wait for all

```zen
import threading
func run_parallel(fns) {
    let threads = []
    for f in fns { threads.push(threading.Thread(fn() { f() })) }
    for t in threads { t.start() }
    for t in threads { t.join() }
}
run_parallel([
    fn() { print("task 1") },
    fn() { print("task 2") },
    fn() { print("task 3") },
])
```

Expected output (order may vary):
```
task 1
task 2
task 3
```

## 58.2 Recipe: Thread-safe counter

```zen
import threading, mutexes
class Counter {
    var n = 0
    var mu = mutexes.Mutex()
    func inc() { self.mu.lock(); self.n += 1; self.mu.unlock() }
    func get() { self.mu.lock(); let v = self.n; self.mu.unlock(); return v }
}
let c = new Counter()
c.inc(); c.inc()
print(c.get())   # 2
```

## 58.3 Recipe: Producer-consumer with a channel

```zen
import threading, channels
let ch = channels.Channel()
func producer() {
    for i in 0..5 { ch.send(i); print("produced " + str(i)) }
    ch.close()
}
func consumer() {
    while true {
        let v = ch.recv()
        if v == nil { break }
        print("consumed " + str(v))
    }
}
let tp = threading.Thread(fn() { producer() })
let tc = threading.Thread(fn() { consumer() })
tp.start()
tc.start()
tp.join()
tc.join()
print("done")
```

Expected output (interleaved):
```
produced 0
consumed 0
...
done
```

## 58.4 Recipe: Worker pool processing a job queue

```zen
import threading, channels
func worker_pool(num_workers, jobs, handle) {
    let job_ch = channels.Channel()
    let done = channels.Channel()
    for w in 0..num_workers {
        threading.Thread(fn() {
            while true {
                let job = job_ch.recv()
                if job == nil { break }
                handle(job)
            }
            done.send(nil)
        }).start()
    }
    for j in jobs { job_ch.send(j) }
    job_ch.close()
    for w in 0..num_workers { done.recv() }
}
worker_pool(4, [1,2,3,4,5,6,7,8], fn(j) { print("job " + str(j)) })
```

Expected output:
```
job 1
job 2
...
job 8
```
(order may vary)

## 58.5 Recipe: Parallel map over a list

```zen
import threading, channels
func parallel_map(xs, f, num_workers) {
    let results = channels.Channel(capacity=len(xs))
    let job_ch = channels.Channel()
    let done = channels.Channel()
    for w in 0..num_workers {
        threading.Thread(fn() {
            while true {
                let item = job_ch.recv()
                if item == nil { break }
                results.send(f(item))
            }
            done.send(nil)
        }).start()
    }
    for x in xs { job_ch.send(x) }
    job_ch.close()
    for w in 0..num_workers { done.recv() }
    results.close()
    let out = []
    while true {
        let r = results.recv()
        if r == nil { break }
        out.push(r)
    }
    return out
}
print(parallel_map([1,2,3,4,5], fn(x) => x * x, 2))   # [1,4,9,16,25]
```

Expected output:
```
[1, 4, 9, 16, 25]
```
(order may vary)

## 58.6 Recipe: Read command-line arguments

```zen
import sys
func main(argv) {
    print("program: " + argv[0])
    print("args: " + str(argv[1:]))
}
if __name__ == "__main__" { main(sys.argv) }
```

```bash
$ zen run app.z foo bar
program: app.z
args: ["foo", "bar"]
```

## 58.7 Recipe: Read an environment variable with default

```zen
import sys
func get_env(name, default) {
    return sys.environ[name] ?? default
}
let port = get_env("PORT", "8080")
print("port=" + port)
```

## 58.8 Recipe: Detect the platform

```zen
import sys
if sys.platform == "linux" { print("Linux") }
elif sys.platform == "darwin" { print("macOS") }
elif sys.platform == "windows" { print("Windows") }
else { print("other: " + sys.platform) }
```

## 58.9 Recipe: Graceful exit with a code

```zen
import sys
func main() {
    # ... work ...
    sys.exit(0)
}
if __name__ == "__main__" { main() }
```

## 58.10 Recipe: List loaded modules

```zen
import sys
print(sys.modules)
```

## 58.11 Recipe: Portable path construction

```zen
import pathlib
import os
let p = pathlib.Path(os.getcwd()) / "output" / "result.txt"
print(str(p))   # /cwd/output/result.txt
```

## 58.12 Recipe: Daemon thread that dies when main exits

```zen
import threading
let t = threading.Thread(fn() {
    while true { print("background"); time.sleep(1000) }
})
t.daemon = true
t.start()
print("main exiting")
# background thread is killed when main exits
```

Expected output:
```
main exiting
```
(background prints a few times, then stops)
