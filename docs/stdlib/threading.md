# threading — threads

Threads in the spirit of Python's `threading`.

```zen
import threading

let t = new threading.Thread(fn() {
    print "worker runs"
})
t.start()
t.join()
```

## Thread methods

| Method | Description |
|--------|-------------|
| `start()` | Launches the thread. |
| `join()` | Waits for completion (returns success). |
| `is_alive()` | Whether the handle is still tracked as live. |
| `run()` | Runs the target inline (no new thread). |

## Functions

| Function | Description |
|----------|-------------|
| `current_thread()` | The calling thread's handle. |
| `enumerate()` | All live thread handles. |
| `active_count()` | Number of live threads. |

Shared values cross threads by deep copy at the boundary — mutate through
thread-safe channels or join before reading worker results.
