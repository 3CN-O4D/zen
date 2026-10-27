# Chapter 35: Threading Fundamentals

Zen supports OS threads via the `thread` module. Threads run concurrently (subject to the OS scheduler) and communicate through `shared` atomic values and `channel` message passing.

## 35.1 The `thread` module

```zen
import thread

func worker(name) {
    print("Worker " + name + " starting")
    return "done: " + name
}

let t = thread.spawn(worker("A"))
let result = thread.join(t)
print(result)  # done: A

print(thread.current())  # current thread info (nil or handle)
```

### Thread functions

| Function | Description | Example |
|----------|-------------|---------|
| `thread.spawn(func)` | Start `func` in a new thread | `thread.spawn(worker)` |
| `thread.join(thread)` | Wait for thread, get return value | `thread.join(t)` |
| `thread.current()` | Current thread info | `thread.current()` |

```zen
import thread

func count(n) {
    let total = 0
    for i in 1..n {
        total = total + i
    }
    return total
}

let t = thread.spawn(count(100))
let result = thread.join(t)
print(result)  # 5050
```

## 35.2 Thread lifecycle

```zen
import thread

func slow() {
    // simulate work
    return "finished"
}

let t = thread.spawn(slow)

// Do other work in main thread...

let result = thread.join(t)  // blocks until slow() returns
print(result)
```

`thread.join` blocks the calling thread until the target thread completes, then returns the target's return value. If the target raised an error, `join` propagates it.

```zen
import thread

func failing() {
    throw "boom"
}

let t = thread.spawn(failing)
try {
    thread.join(t)
} catch as e {
    print(e.msg)  # boom
}
```

## 35.3 Thread-local data

Variables created inside a thread function are local to that thread:

```zen
import thread

func worker() {
    let local = "only in this thread"
    print(local)
}

let t = thread.spawn(worker)
thread.join(t)
// local is not accessible here
```

To share data between threads, use `shared` (atomic values) or `channel` (message passing).

## 35.3 Multiple threads

```zen
import thread

func worker(id) {
    print("Worker " + str(id) + " started")
    return "worker " + str(id) + " done"
}

let threads = []
for i in 1..5 {
    let t = thread.spawn(worker(i))
    threads.push(t)
}

for t in threads {
    let result = thread.join(t)
    print(result)
}
print("All workers done")
```

## 35.4 Common pitfalls

- **`thread.join` blocks.** Join all threads before your program exits, or their work may be cut short.
- **Exceptions propagate.** If a thread function raises, `thread.join` re-raises it in the joining thread.
- **No shared state by default.** Each thread has its own stack and locals. Use `shared` or `channel` to communicate.
- **Thread count.** Don't spawn more threads than necessary; OS threads are relatively heavyweight.

## 35.5 Quick reference

```zen
import thread

let t = thread.spawn(func() { return "result" })
let result = thread.join(t)
let cur = thread.current()
```

## 35.6 Exercises

1. Write a program that spawns 5 threads, each printing its ID, and joins them all.
2. Write `parallel_sum(nums)` that splits a list into chunks, sums each chunk in a thread, and combines results.
3. Write a thread that sleeps for 1 second then prints "done".
4. Demonstrate that `thread.join` propagates errors from the child thread.
5. Write a function `run_concurrent(funcs)` that runs a list of zero-arg functions in threads and returns their results in order.


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


## 35.12 Worked examples — threading

### Example 1: Basic thread creation and join

```zen
import threading

func worker(name) {
    print("Thread ${name} starting")
    # simulate work
    for i in 0..3 {
        print("${name} tick ${i}")
    }
    print("Thread ${name} finished")
}

let t1 = threading.Thread(fn() { worker("A") })
let t2 = threading.Thread(fn() { worker("B") })
t1.start()
t2.start()
t1.join()
t2.join()
print("both threads done")
```

Expected output (order may vary):
```
Thread A starting
Thread B starting
A tick 0
B tick 0
...
both threads done
```

### Example 2: Sharing data — the race condition

```zen
import threading

var shared_counter = 0

func increment(times) {
    var shared_counter = shared_counter   # read global
    for i in 0..times {
        shared_counter = shared_counter + 1   # NOT atomic!
    }
}

let t1 = threading.Thread(fn() { increment(100000) })
let t2 = threading.Thread(fn() { increment(100000) })
t1.start()
t2.start()
t1.join()
t2.join()
print(shared_counter)   # likely LESS than 200000 — race condition!
```

**Why:** read-modify-write is not atomic. Two threads read the same value,
both add 1, and one update is lost.

### Example 3: Fix with a mutex (see Chapter 37)

Use `mutexes.Mutex` to guard the counter (full example in 37.12).

### Example 4: Daemon threads

Daemon threads are killed when the main program exits.

```zen
let t = threading.Thread(fn() {
    while true { print("background..."); sleep(1000) }
})
t.daemon = true
t.start()
print("main exits; daemon thread dies")
```

### Example 5: Thread-local storage

Each thread gets its own copy of a value.

```zen
let local = threading.local()

func worker() {
    local.value = threading.current_id()
    print("thread sees value " + str(local.value))
}
```

## 35.13 Common threading bugs

1. **Race condition:** unsynchronized read-modify-write → lost updates.
2. **Deadlock:** two threads each hold a lock the other needs → both wait forever.
   Fix: always acquire locks in the same order.
3. **Livelock:** threads keep retrying but make no progress.
4. **Starvation:** a thread never gets CPU/lock access.
5. **Forgetting `join`:** main exits before workers finish → partial output.
6. **Modifying shared collections without a lock.**
7. **Holding a lock too long** → poor concurrency.
8. **Calling code that blocks while holding a lock.**

## 35.14 Debugging threaded programs

- Log with a thread identifier so you can tell which thread printed what.
- Reproduce races by running many iterations; races are timing-dependent.
- Use `shared` for cross-thread state rather than plain globals.
- Keep critical sections small.
- Prefer message passing (channels) over shared memory when possible.

## 35.15 Exercises

1. Create 5 threads that each print their ID 3 times; join all.
2. Write a function `run_parallel(fns)` that starts a thread per function and
   joins all.
3. Demonstrate the race counter example and confirm the result < expected.
4. Fix the race counter using a mutex.
5. Write a daemon thread that prints every second; let main exit.
6. Implement `sum_parallel(list)` splitting work across N threads and merging.
7. Write `thread_id()` printing `threading.current_id()`.
8. Simulate a bank account with deposit/transfer from multiple threads.
9. Show what happens if you join a thread twice (error? no-op?).
10. Benchmark a CPU-heavy task sequential vs 2 threads.

