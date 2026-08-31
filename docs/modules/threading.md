# threading — Background execution

The `threading` module allows you to run Zen functions in the background without blocking the main execution path. This is useful for long-running tasks like network requests or file processing.

This module is available globally as `threading`.

```zen
func long_task(name) {
    print("${name} started...")
    sleep(2)  # Simulate work
    print("${name} finished!")
}

# Start a background thread
var t1 = threading.start(fn() { long_task("Task 1") })
var t2 = threading.start(fn() { long_task("Task 2") })

print("Main thread continues immediately...")

# Wait for threads to finish
threading.join(t1)
threading.join(t2)

print("Everything done.")
```

## Functions

| Function | Description |
|----------|-------------|
| `start(function)` | Launches the provided function in a new thread. Returns a thread descriptor (dict). |
| `join(thread_dict)` | Blocks the calling thread until the specified background thread completes. |

## The Thread Descriptor
When you call `threading.start()`, it returns a dictionary containing information about the new thread:

```zen
var t = threading.start(fn() { sleep(1) })
print(t.name)   # Thread-__lambda_0
print(t.id)     # thread-1
print(t.daemon) # true
```

## Shared Memory & Safety
In Zen, threads share the same global state and closure variables. Since there is no Global Interpreter Lock (GIL) like in Python, you should be careful when multiple threads write to the same variable at once.

```zen
var counter = 0
var t = threading.start(fn() {
    counter = counter + 1
})
threading.join(t)
print(counter)  # 1
```

## See Also
- [time](time.md) — For `sleep()` and timestamps.
- [subprocess](subprocess.md) — For running external system processes.
