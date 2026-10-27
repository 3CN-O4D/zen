# Chapter 37: Mutexes & Synchronization

Zen provides two synchronization primitives for concurrent programming:
`shared` (atomic shared values) and `channel` (message passing between threads). Together they let you write safe multi-threaded programs without manual locks.

## 37.1 The `shared` module — atomic shared values

```zen
import shared

# Create a shared integer counter
let counter = shared.create(0)

# Read the current value
print(shared.get(counter))   # 0

# Set a new value
shared.set(counter, 5)
print(shared.get(counter))   # 5

# Atomic increment (returns the new value)
shared.increment(counter, 1)
print(shared.get(counter))   # 6

shared.increment(counter, 10)
print(shared.get(counter))   # 16
```

### Shared functions

| Function | Description | Example |
|----------|-------------|---------|
| `shared.create(value)` | Create a shared atomic value | `shared.create(0)` |
| `shared.get(shared)` | Read current value | `shared.get(counter)` |
| `shared.set(shared, value)` | Set a new value | `shared.set(counter, 5)` |
| `shared.increment(shared, delta)` | Add delta, return new value | `shared.increment(counter, 1)` |

Shared values are safe to read and write from multiple threads — operations are atomic, so there are no torn reads or races on the value itself.

```zen
import shared
import thread

let counter = shared.create(0)

func worker(id) {
    for i in 1..100 {
        shared.increment(counter, 1)
    }
    print("worker " + str(id) + " done")
}

let threads = []
for i in 1..5 {
    let t = thread.spawn(worker(i))
    threads.push(t)
}

for t in threads {
    thread.join(t)
}

print(shared.get(counter))  # 500 (5 workers × 100 increments)
```

Because `shared.increment` is atomic, the final count is exactly 500 — no lost updates even with 5 threads incrementing concurrently.

## 37.2 The `channel` module — message passing

Channels send values between threads. A channel is created, values are sent into it, and received from it on the other end.

```zen
import channel

let ch = channel.make()

channel.send(ch, "hello")
channel.send(ch, 42)
channel.send(ch, true)

print(channel.recv(ch))  # hello
print(channel.recv(ch))  # 42
print(channel.recv(ch))  # true
```

### Channel functions

| Function | Description | Example |
|----------|-------------|---------|
| `channel.make()` | Create a new channel | `channel.make()` |
| `channel.send(ch, value)` | Send a value | `channel.send(ch, 42)` |
| `channel.recv(ch)` | Receive a value (blocks) | `channel.recv(ch)` |

`recv` blocks until a value is available, making channels useful for coordinating work between threads.

## 37.3 Threads: `thread` module

```zen
import thread

func worker(name) {
    print("Worker " + name + " starting")
    return "done: " + name
}

let t = thread.spawn(worker("A"))
let result = thread.join(t)
print(result)  # done: A

print(thread.current())  # nil or current thread info
```

### Thread functions

| Function | Description | Example |
|----------|-------------|---------|
| `thread.spawn(func)` | Start a function in a thread | `thread.spawn(worker)` |
| `thread.join(thread)` | Wait for thread, get result | `thread.join(t)` |
| `thread.current()` | Current thread info | `thread.current()` |

## 37.4 Producer-consumer with channels

```zen
import channel
import thread

let ch = channel.make()

func producer() {
    for i in 1..5 {
        channel.send(ch, i)
        print("produced: " + str(i))
    }
    channel.send(ch, nil)  // sentinel: done
}

func consumer() {
    while true {
        let v = channel.recv(ch)
        if v == nil {
            break
        }
        print("consumed: " + str(v))
    }
}

let t1 = thread.spawn(producer)
let t2 = thread.spawn(consumer)
thread.join(t1)
thread.join(t2)
print("all done")
```

## 37.5 Shared counter across threads

```zen
import shared
import thread

let counter = shared.create(0)

func worker(id) {
    for i in 1..100 {
        shared.increment(counter, 1)
    }
    print("worker " + str(id) + " done")
}

let threads = []
for i in 1..5 {
    let t = thread.spawn(worker(i))
    threads.push(t)
}

for t in threads {
    thread.join(t)
}

print(shared.get(counter))  # 500 (5 workers × 100 increments)
```

Because `shared.increment` is atomic, the final count is exactly 500 — no lost updates even with 5 threads incrementing concurrently.

## 37.6 Common patterns

### Guarded shutdown flag

```zen
import shared
import thread

let running = shared.create(true)

func worker() {
    while shared.get(running) {
        # do work
    }
    print("worker stopped")
}

let t = thread.spawn(worker)
// ... later ...
shared.set(running, false)
thread.join(t)
```

### Work queue with channel

```zen
import channel
import thread

let queue = channel.make()

func dispatcher(jobs) {
    for job in jobs {
        channel.send(queue, job)
    }
    channel.send(queue, nil)  // done
}

func worker(queue) {
    while true {
        let job = channel.recv(queue)
        if job == nil { break }
        print("processing: " + str(job))
    }
}

let jobs = [1, 2, 3, 4, 5, 6, 7, 8]
let td = thread.spawn(dispatcher(jobs))
let tw1 = thread.spawn(worker(queue))
let tw2 = thread.spawn(worker(queue))
thread.join(td)
thread.join(tw1)
thread.join(tw2)
```

## 37.8 Common pitfalls

- **`recv` blocks forever** if no value is ever sent. Always ensure a sender exists, or use a sentinel value (like `nil`) to signal completion.
- **`shared` values are atomic individually**, but compound operations (read-then-write) are not atomic. Use `increment` instead of `get`+`set`+counter when you need to add.
- **`join` blocks** until the thread finishes. Join all threads before your program exits, or their work may be cut short.
- **Channels are unbuffered.** `send` blocks until a `recv` is waiting, and `recv` blocks until a `send` happens.

## 37.9 Quick reference

```zen
import shared
import channel
import thread

# Shared atomic values
let c = shared.create(0)
shared.get(c)
shared.set(c, 10)
shared.increment(c, 1)

# Channels
let ch = channel.make()
channel.send(ch, "msg")
channel.recv(ch)

# Threads
let t = thread.spawn(func() { return "result" })
thread.join(t)
thread.current()
```

## 37.10 Exercises

1. Write a program that spawns 5 threads, each incrementing a shared counter 1000 times, and verify the final count is 5000.
2. Use a channel to implement a producer-consumer with 2 producers and 1 consumer.
3. Write a `parallel_map(func, items)` that runs `func` on each item in a separate thread and collects results via a channel.
4. Use a shared boolean flag to gracefully stop a background thread.
5. Write a thread-safe counter class using `shared.create` and methods.


## Extended Content

This section provides additional depth, patterns, debugging tips, and exercises for production use.

### Common pitfalls
- Race conditions
- Deadlocks (circular wait)
- Starvation
- Data races
- Forgetting to join threads

### Debugging concurrent code
- Use logging with thread identifiers
- Test with different interleavings
- Consider thread sanitizers conceptually
- Design for thread safety from start

### Exercises
1. Producer-consumer with channels
2. Thread pool pattern
3. Parallel map/reduce
4. Avoiding deadlocks with lock ordering
5. Thread-safe cache
6. Benchmark sequential vs parallel


## 37.12 Worked examples — mutexes and locks

### Example 1: Fixing the race counter with a Mutex

```zen
import threading
import mutexes

var counter = 0
let mu = mutexes.Mutex()

func increment(times) {
    for i in 0..times {
        mu.lock()
        counter = counter + 1
        mu.unlock()
    }
}

let t1 = threading.Thread(fn() { increment(100000) })
let t2 = threading.Thread(fn() { increment(100000) })
t1.start()
t2.start()
t1.join()
t2.join()
print(counter)   # 200000 — correct!
```

### Example 2: Lock with try/finally (always unlock)

```zen
func safe_increment() {
    mu.lock()
    try {
        counter = counter + 1
    } finally {
        mu.unlock()
    }
}
```

### Example 3: Deadlock — and how to avoid it

```zen
# BAD: potential deadlock
let mu1 = mutexes.Mutex()
let mu2 = mutexes.Mutex()

func thread_a() {
    mu1.lock()
    mu2.lock()   # if thread_b holds mu2 and waits for mu1 -> deadlock
    ...
    mu2.unlock()
    mu1.unlock()
}

func thread_b() {
    mu2.lock()
    mu1.lock()   # opposite order!
    ...
    mu1.unlock()
    mu2.unlock()
}
```

**Fix:** Always acquire locks in the same global order (mu1 before mu2).

```zen
# GOOD
func thread_b_fixed() {
    mu1.lock()   # same order as thread_a
    mu2.lock()
    ...
    mu2.unlock()
    mu1.unlock()
}
```

### Example 4: Read-write pattern (conceptual)

If your build has `RWMutex`, multiple readers can hold it simultaneously, but
a writer needs exclusive access. Otherwise, use a plain Mutex for all access.

## 37.13 Locking guidelines

1. **Lock order:** global, consistent order prevents deadlocks.
2. **Fine-grained locks:** lock only what's necessary; don't lock the whole
   data structure for a small update.
3. **Short critical sections:** minimize work inside `lock/unlock`.
4. **Never call blocking code** (sleep, network) while holding a lock.
5. **Use `finally`** to guarantee unlock even on error.
6. **Prefer channels** over shared+lock when the design allows.

## 37.14 Exercises

1. Protect a shared counter with a Mutex; verify exact final value.
2. Implement a thread-safe stack using a Mutex and a list.
3. Implement a thread-safe cache (dict + Mutex) with get/put.
4. Demonstrate deadlock with two locks in opposite order, then fix it.
5. Write `with_lock(mu, fn)` that locks, runs fn, unlocks in finally.
6. Protect a shared bank account: deposit/transfer from many threads.
7. Implement `increment_n(mu, counter, n)` using lock/unlock.
8. Show that reading without a lock can see torn/stale values (conceptually).
9. Implement a thread-safe counter class wrapping Mutex + int.
10. Benchmark locked vs unlocked (single-threaded) to measure lock cost.

