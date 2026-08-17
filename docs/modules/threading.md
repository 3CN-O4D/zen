# Threading Module

Complete reference for concurrent execution, locks, events, queues, and producer-consumer patterns in Zen.

## Quick Start

```
// Run a function in a background thread
threading.start(function() {
    print "Running in background"
    sleep(2)
    print "Done"
})

print "Main thread continues immediately"
// Main thread continues while background runs
```

---

## Basic Usage

### Get thread info

```
print threading.active()       // active thread count
print threading.current()      // "MainThread"
print threading.list()         // list of active thread info dicts
```

### Sleep

```
print "Starting..."
threading.sleep(0.5)            // sleep for 0.5 seconds
print "Woke up after 0.5s"
```

---

## Threads

### Fire-and-forget with `threading.start`

```
threading.start(function() {
    for i in 1 -> 5 {
        print "Background: {i}"
        sleep(0.5)
    }
})

// Main thread doesn't wait
print "Main thread done"
```

### Create and manage Thread objects

```
let t = threading.Thread(function() {
    sleep(1)
    return "result from thread"
})

t.start()
print t.is_alive()       // true

t.join()                 // wait for completion
print t.is_alive()       // false
```

### Join with timeout

```
let t = threading.Thread(function() {
    sleep(10)
})

t.start()
t.join(2)                // wait max 2 seconds
print t.is_alive()       // still running (timed out)
```

---

## Locks

Mutex locks prevent race conditions when multiple threads access shared data.

### Basic lock usage

```
let counter = {"value": 0}
let lock = threading.Lock()

function increment() {
    for i in 1 -> 1000 {
        lock.acquire()
        counter.value = counter.value + 1
        lock.release()
    }
}

let t1 = threading.Thread(increment)
let t2 = threading.Thread(increment)
t1.start()
t2.start()
t1.join()
t2.join()

print counter.value    // 2000 (correct!)
```

### Reentrant lock (RLock)

Allows the same thread to acquire the lock multiple times:

```
let lock = threading.RLock()

function recursive(n) {
    lock.acquire()
    if n > 0 {
        recursive(n - 1)    // same thread re-acquires
    }
    lock.release()
}
```

---

## Events

Events allow threads to signal each other.

```
let event = threading.Event()

let waiter = threading.Thread(function() {
    print "Waiting for signal..."
    event.wait()
    print "Got the signal!"
})

let signaling = threading.Thread(function() {
    sleep(1)
    print "Sending signal..."
    event.set()
})

waiter.start()
signaling.start()
waiter.join()
signaling.join()
```

### Event methods

| Method | Description |
|--------|-------------|
| `.set()` | Signal the event |
| `.clear()` | Reset the event |
| `.wait(timeout?)` | Wait for signal (optional timeout) |
| `.is_set()` | Check if signaled |

### Timeout on wait

```
let event = threading.Event()

let t = threading.Thread(function() {
    let got_signal = event.wait(2)    // wait max 2 seconds
    if got_signal {
        print "Got signal"
    } else {
        print "Timed out"
    }
})

t.start()
// event.set() not called — will time out
t.join()
```

---

## Semaphores and Barriers

### Semaphore

Limits concurrent access to a resource:

```
let semaphore = threading.Semaphore(3)    // max 3 concurrent

function access_resource(id) {
    semaphore.acquire()
    print "Thread {id} accessing resource"
    sleep(1)
    print "Thread {id} done"
    semaphore.release()
}

for i in 1 -> 10 {
    threading.Thread(function() {
        access_resource(i)
    }).start()
}
```

### Barrier

Synchronizes threads at a checkpoint:

```
let barrier = threading.Barrier(3)    // wait for 3 threads

function worker(id) {
    print "Thread {id} ready"
    barrier.wait()    // blocks until all 3 arrive
    print "Thread {id} proceeding"
}

for i in 1 -> 3 {
    threading.Thread(function() { worker(i) }).start()
}
```

---

## Queue

Thread-safe FIFO queue for producer-consumer patterns.

### Basic queue usage

```
let q = threading.Queue()

// Producer
threading.start(function() {
    for i in 1 -> 5 {
        q.put(i)
        print "Produced: {i}"
    }
})

// Consumer
threading.start(function() {
    for i in 1 -> 5 {
        let item = q.get()
        print "Consumed: {item}"
    }
})

sleep(2)    // wait for both threads
```

### Queue methods

| Method | Description |
|--------|-------------|
| `.put(item)` | Add item to queue |
| `.get()` | Remove and return item (blocks if empty) |
| `.qsize()` | Approximate queue size |
| `.empty()` | Check if queue is empty |
| `.task_done()` | Mark task as complete |
| `.join()` | Wait until all items are processed |

### Producer/Consumer with join

```
let q = threading.Queue()
let num_items = 5

// Producer
let producer = threading.Thread(function() {
    for i in 1 -> num_items {
        q.put(i * 10)
    }
})

// Consumer
let consumer = threading.Thread(function() {
    let results = []
    for i in 1 -> num_items {
        let item = q.get()
        results.append(item)
        q.task_done()
    }
    return results
})

producer.start()
consumer.start()
q.join()            // wait for all items to be processed
producer.join()
consumer.join()

print "All items processed"
```

---

## Common Patterns

### Parallel HTTP requests

```
let results = []
let lock = threading.Lock()

function fetch(url) {
    let resp = http.get(url)
    lock.acquire()
    results.append({"url": url, "status": resp.status})
    lock.release()
}

let urls = [
    "https://httpbin.org/get",
    "https://httpbin.org/delay/1",
    "https://httpbin.org/status/200"
]

let threads = []
for url in urls {
    let t = threading.Thread(function() { fetch(url) })
    threads.append(t)
    t.start()
}

for t in threads {
    t.join()
}

print "Fetched {results.len} URLs"
for r in results {
    print "  {r.url}: {r.status}"
}
```

### Background task with cancellation

```
let running = true

let bg = threading.Thread(function() {
    while running {
        print "Working..."
        sleep(1)
    }
    print "Stopped"
})

bg.start()

// Let it run for 5 seconds
sleep(5)
running = false
bg.join()
```

### Thread-safe counter

```
let counter = {"value": 0}
let lock = threading.Lock()

function safe_increment(n) {
    for i in 1 -> n {
        lock.acquire()
        counter.value = counter.value + 1
        lock.release()
    }
}

let t1 = threading.Thread(function() { safe_increment(1000) })
let t2 = threading.Thread(function() { safe_increment(1000) })
let t3 = threading.Thread(function() { safe_increment(1000) })

t1.start()
t2.start()
t3.start()
t1.join()
t2.join()
t3.join()

print counter.value    // 3000
```

---

## Pro Tips

1. **Use locks around shared state.** Always acquire/release locks when multiple threads access the same data.
2. **Use Queue for producer-consumer.** It's thread-safe by design.
3. **Use events for signaling.** Cleaner than polling with sleep.
4. **Use `join()` to wait for completion.** Don't assume threads finish before the main thread.
5. **Keep critical sections small.** Hold locks for the minimum time necessary.

---

## Common Mistakes

### Race conditions without locks

```
// BAD — data race
let counter = {"value": 0}

function increment() {
    for i in 1 -> 1000 {
        counter.value = counter.value + 1    // RACE CONDITION
    }
}

// GOOD — use lock
let lock = threading.Lock()
function increment() {
    for i in 1 -> 1000 {
        lock.acquire()
        counter.value = counter.value + 1
        lock.release()
    }
}
```

### Deadlock

```
// BAD — two locks in different order
let lock_a = threading.Lock()
let lock_b = threading.Lock()

// Thread 1: acquire lock_a, then lock_b
// Thread 2: acquire lock_b, then lock_a
// DEADLOCK!

// GOOD — always acquire locks in the same order
```

### Forgetting to join threads

```
// BAD — main thread exits before background finishes
threading.start(function() {
    sleep(5)
    print "This may not print"
})
// Script exits here

// GOOD — join the thread
let t = threading.Thread(function() {
    sleep(5)
    print "This prints"
})
t.start()
t.join()
```

---

## See Also

- [Control Flow](../language/control-flow.md) — Loops and breaks in threads
- [Functions](../language/functions.md) — Closures in thread callbacks
- [Module Overview](overview.md) — All available modules
