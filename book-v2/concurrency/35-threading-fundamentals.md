# Chapter 35: Threading Fundamentals

Zen supports multi-threaded programming, allowing your programs to execute multiple tasks concurrently. This chapter covers the fundamentals of threading in Zen.

## Understanding Concurrency vs Parallelism

- **Concurrency**: Dealing with many things at once (structure)
- **Parallelism**: Doing many things at once (execution)

Threads enable both concurrent and parallel execution depending on your system's CPU cores.

## Creating Threads with `spawn()`

The `spawn()` function creates a new OS thread to execute a function.

### Basic Thread Creation

```zen
func worker() {
    print("Hello from worker thread!")
}

spawn(worker)
print("Hello from main thread!")
```

The order of output is non-deterministic - threads run concurrently.

### Spawning with Anonymous Functions

```zen
spawn(fn() {
    print("Anonymous thread running")
    let i = 0
    while i < 3 {
        print("Thread: " + i)
        i = i + 1
    }
})

print("Main thread continues")
```

### Passing Data to Threads

Since threads have separate execution contexts, you need to be careful about sharing data:

```zen
func print_message(msg, count) {
    let i = 0
    while i < count {
        print(msg + ": " + i)
        i = i + 1
    }
}

spawn(fn() { print_message("Worker1", 5) })
spawn(fn() { print_message("Worker2", 5) })
```

## Thread Lifecycle

1. **Spawn**: Thread created and scheduled
2. **Running**: Executing code
3. **Finished**: Function returns, thread terminates

```zen
func task() {
    print("Task started")
    # Do work
    let i = 0
    while i < 1000000 {
        i = i + 1
    }
    print("Task finished")
}

spawn(task)
print("Main continues immediately")
```

## Multiple Threads

```zen
func countdown(name, start) {
    let i = start
    while i > 0 {
        print(name + ": " + i)
        i = i - 1
    }
    print(name + ": Done!")
}

spawn(fn() { countdown("A", 3) })
spawn(fn() { countdown("B", 3) })
spawn(fn() { countdown("C", 3) })
```

## Thread Safety Considerations

Threads share:
- Global variables (need synchronization)
- Heap objects (be careful with mutable state)
- Native modules

Each thread has its own:
- Stack
- Local variables
- Register file

## Practical Example: Parallel Computation

```zen
let results = []
let lock = mutex.new(null)

func compute_square(n) {
    let result = n * n
    # Add to shared results with lock
    let m = mutex.lock(lock)
    results.push({n: n, square: result})
    mutex.unlock(m)
    print("Computed square of " + n)
}

# Spawn threads for parallel computation
spawn(fn() { compute_square(1) })
spawn(fn() { compute_square(2) })
spawn(fn() { compute_square(3) })
spawn(fn() { compute_square(4) })
spawn(fn() { compute_square(5) })
```

## Best Practices for Threading

1. **Minimize shared mutable state** - Use channels for communication
2. **Use synchronization primitives** - Mutexes when sharing is necessary
3. **Keep threads short-lived** - Or design for long-running workers
4. **Avoid race conditions** - Always protect shared data
5. **Consider thread pool** - Don't spawn too many threads
6. **Handle errors** - Threads can have errors too

## Common Pitfalls

### Race Condition Example

```zen
let counter = 0

func increment() {
    let i = 0
    while i < 1000 {
        counter = counter + 1  # NOT thread-safe!
        i = i + 1
    }
}

spawn(increment)
spawn(increment)
# counter may not be 2000 due to race condition
```

Use mutex to fix:

```zen
let counter = 0
let lock = mutex.new(counter)

func increment_safe() {
    let i = 0
    while i < 1000 {
        let m = mutex.lock(lock)
        let val = mutex.get(m)
        mutex.set(m, val + 1)
        counter = mutex.get(m)
        mutex.unlock(m)
        i = i + 1
    }
}
```

## Chapter Summary

- Use `spawn(func)` to create threads
- Threads run concurrently and independently
- Be careful with shared mutable state
- Use channels or mutexes for synchronization
- Thread order is non-deterministic
- Each thread has its own stack and local variables

**Next:** Chapter 36 - Channels
