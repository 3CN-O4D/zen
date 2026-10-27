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
