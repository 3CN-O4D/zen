# Chapter 38: Thread Safety & Patterns

This chapter covers practical thread-safe patterns: worker pools, reader-writer
scenarios, producer-consumer pipelines, and avoiding common concurrency bugs.

## 38.1 Worker pool pattern

A fixed number of worker threads pull jobs from a shared channel:

```zen
import channel
import thread

let job_queue = channel.make()
let results = channel.make()

func worker(id) {
    while true {
        let job = channel.recv(job_queue)
        if job == nil {
            break  # sentinel: shutdown
        }
        let result = job * job  # some work
        channel.send(results, [id, result])
    }
}

func dispatcher(jobs) {
    for j in jobs {
        channel.send(job_queue, j)
    }
    # Send one sentinel per worker
    for i in 1..4 {
        channel.send(job_queue, nil)
    }
}

# Start 4 workers
for i in 1..4 {
    thread.spawn(worker(i))
}

let jobs = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
thread.spawn(dispatcher(jobs))

# Collect results
let completed = 0
while completed < len(jobs) {
    let r = channel.recv(results)
    print("worker " + str(r[0]) + " -> " + str(r[1]))
    completed = completed + 1
}
print("all jobs done")
```

## 38.2 Pipeline pattern

Chain stages where each stage is a thread:

```zen
import channel
import thread

let stage1_out = channel.make()
let stage2_out = channel.make()

func stage1() {
    for i in 1..10 {
        channel.send(stage1_out, i * 2)
    }
    channel.send(stage1_out, nil)
}

func stage2() {
    while true {
        let v = channel.recv(stage1_out)
        if v == nil {
            channel.send(stage2_out, nil)
            break
        }
        channel.send(stage2_out, v + 10)
    }
}

func stage3() {
    while true {
        let v = channel.recv(stage2_out)
        if v == nil { break }
        print("final: " + str(v))
    }
}

thread.spawn(stage1)
thread.spawn(stage2)
thread.spawn(stage3)
# main thread waits implicitly; add joins if needed
```

## 38.3 Reader-writer pattern with shared flag

```zen
import shared
import thread

let active = shared.create(true)

func reader(id) {
    while shared.get(active) {
        # read shared resource
    }
    print("reader " + str(id) + " stopping")
}

func writer() {
    # do some writes
    shared.set(active, false)  # signal shutdown
}

let r1 = thread.spawn(reader(1))
let r2 = thread.spawn(reader(2))
let w = thread.spawn(writer)
thread.join(w)
thread.join(r1)
thread.join(r2)
```

## 38.4 Avoiding deadlocks

Deadlocks happen when two threads each wait for the other. Prevention rules:

1. **Always acquire resources in the same order.**
2. **Don't hold a resource while waiting for another.**
3. **Use timeouts where possible.**
4. **Keep critical sections small.**

Bad (potential deadlock):

```zen
# Thread A: lock A, then wait for B
# Thread B: lock B, then wait for A
```

Good (consistent order):

```zen
# Both threads: lock A, then lock B
```

## 38.5 Avoiding race conditions

A race condition is when the outcome depends on thread timing. Prevention:

1. **Use `shared` for atomic counters** — `shared.increment` instead of
   `get`+`set`.
2. **Use channels to hand off ownership** — send a value, don't share it.
3. **Don't share mutable state** between threads unless protected.

Bad (race):

```zen
let count = 0  # not shared — each thread may see its own copy
func bad_worker() {
    for i in 1..100 {
        count = count + 1  # RACE: lost updates
    }
}
```

Good (atomic):

```zen
import shared
let count = shared.create(0)
func good_worker() {
    for i in 1..100 {
        shared.increment(count, 1)
    }
}
```

## 38.6 Thread-local data

Each thread has its own stack and locals. Variables created inside a thread
function are not visible to other threads unless shared via `shared` or
`channel`:

```zen
import thread

func worker() {
    let local = "only in this thread"  # thread-local
    print(local)
}

let t = thread.spawn(worker)
thread.join(t)
# print(local)  # error: local not defined here
```

## 38.7 Common patterns summary

| Pattern | Use when | Primitive |
|---------|----------|-----------|
| Worker pool | Many small tasks | channel + thread.spawn |
| Pipeline | Sequential stages | channels chained |
| Producer-consumer | One produces, another consumes | channel |
| Shared counter | Statistics, progress | shared.create/increment |
| Shutdown flag | Coordinate exit | shared.create/set/get |

## 38.8 Quick reference

```zen
import shared
import channel
import thread

# Worker pool
let q = channel.make()
for i in jobs { channel.send(q, i) }
for i in 1..N { channel.send(q, nil) }
for i in 1..N { thread.spawn(worker(i)) }

# Atomic counter
let c = shared.create(0)
shared.increment(c, 1)
shared.get(c)

# Join all threads
for t in threads { thread.join(t) }
```

## 38.9 Exercises

1. Write a worker pool with 4 workers processing a list of URLs (simulated).
2. Implement a 3-stage pipeline: generate → transform → print.
3. Write a `parallel_map` that runs a function on each item concurrently.
4. Use a shared flag to stop all workers when one encounters an error.
5. Write a program that counts lines across multiple "files" (lists) in
   parallel threads and sums the total.
