# Chapter 36: Channels

Channels provide a safe way for threads to communicate by sending and receiving values. They help avoid race conditions by synchronizing access to data.

## What are Channels?

Channels are like pipes - one thread can send data, another can receive it. They handle synchronization automatically.

## Creating Channels

```zen
let ch = channel()
```

## Sending and Receiving

### Basic Communication

```zen
let ch = channel()

spawn(fn() {
    ch.send("Hello from thread!")
    print("Message sent")
})

let msg = ch.recv()
print("Received: " + msg)
```

### Bidirectional Communication

```zen
let ch1 = channel()
let ch2 = channel()

spawn(fn() {
    let msg = ch1.recv()
    print("Worker received: " + msg)
    ch2.send("Response from worker")
})

ch1.send("Request from main")
let response = ch2.recv()
print("Main received: " + response)
```

## Blocking Behavior

`send()` and `recv()` are blocking by default:
- `send()` blocks if channel is full (for buffered channels)
- `recv()` blocks until a value is available

```zen
let ch = channel()

spawn(fn() {
    # This will block until main receives
    ch.send("data")
    print("Send completed")
})

print("Main about to receive")
let data = ch.recv()
print("Main received: " + data)
```

## Producer-Consumer Pattern

This is the most common use case for channels:

```zen
let ch = channel()

# Producer
spawn(fn() {
    let i = 1
    while i <= 5 {
        print("Producing: " + i)
        ch.send(i)
        i = i + 1
    }
    ch.close()
    print("Producer done")
})

# Consumer
spawn(fn() {
    while true {
        let val = ch.recv()
        if val == null {
            break  # Channel closed
        }
        print("Consuming: " + val)
    }
    print("Consumer done")
})
```

## Multiple Producers, Multiple Consumers

```zen
let ch = channel()

# Producer 1
spawn(fn() {
    let i = 0
    while i < 3 {
        ch.send("P1: " + i)
        i = i + 1
    }
})

# Producer 2
spawn(fn() {
    let i = 0
    while i < 3 {
        ch.send("P2: " + i)
        i = i + 1
    }
})

# Consumer
let i = 0
while i < 6 {
    print(ch.recv())
    i = i + 1
}
```

## Closing Channels

Close a channel when no more values will be sent:

```zen
let ch = channel()

spawn(fn() {
    ch.send(1)
    ch.send(2)
    ch.send(3)
    ch.close()  # Signal no more data
})

# Receive until closed
while true {
    let v = ch.recv()
    if v == null {
        break
    }
    print(v)
}
```

## Buffered vs Unbuffered Channels

**Unbuffered (default):** Sender blocks until receiver is ready
```zen
let ch = channel()  # Unbuffered
```

**Buffered:** Can hold multiple values without blocking
```zen
let ch = channel(10)  # Buffer size of 10
```

```zen
let ch = channel(3)

spawn(fn() {
    ch.send(1)
    ch.send(2)
    ch.send(3)
    print("Sent 3 items")
    ch.send(4)  # Blocks if buffer full and no receiver
    print("Sent 4th item")
})

# Consumer reads later
let i = 0
while i < 4 {
    print("Received: " + ch.recv())
    i = i + 1
}
```

## Select/Multiplexing Pattern

While Zen may not have a built-in `select`, you can use patterns:

```zen
let ch1 = channel()
let ch2 = channel()

spawn(fn() {
    ch1.send("from ch1")
})

spawn(fn() {
    ch2.send("from ch2")
})

# Check which channel has data
# Use non-blocking patterns or just receive in order
```

## Practical Example: Work Queue

```zen
let jobs = channel()
let results = channel()

# Worker pool
func worker(id) {
    while true {
        let job = jobs.recv()
        if job == null {
            break
        }
        print("Worker " + id + " processing job " + job)
        # Simulate work
        let result = job * 2
        results.send(result)
    }
    print("Worker " + id + " done")
}

spawn(fn() { worker(1) })
spawn(fn() { worker(2) })
spawn(fn() { worker(3) })

# Send jobs
let i = 1
while i <= 10 {
    jobs.send(i)
    i = i + 1
}
jobs.close()

# Collect results
let i = 0
while i < 10 {
    print("Result: " + results.recv())
    i = i + 1
}
```

## Practical Example: Timeout Pattern

```zen
let ch = channel()
let timeout_ch = channel()

spawn(fn() {
    # Simulate slow operation
    # Would sleep here if sleep exists
    ch.send("slow result")
})

spawn(fn() {
    # Simulate timeout
    timeout_ch.send("timeout")
})

# Race between result and timeout
let result = ch.recv()
print(result)  # Gets whichever comes first
```

## Best Practices

1. **Close channels from sender side** - Only senders should close
2. **Don't send on closed channels** - Will cause errors
3. **Use channels for communication, mutexes for shared state** - Prefer channels
4. **Buffer appropriately** - Use buffered channels to reduce blocking
5. **Handle channel closure** - Always check for null when receiving
6. **One direction per channel** - Keeps design simple

## Chapter Summary

- Channels enable safe thread communication
- Use `channel()` to create, `send()` to send, `recv()` to receive
- Close with `close()`, check for null on receive
- Support buffered channels with `channel(size)`
- Perfect for producer-consumer and worker pool patterns
- Automatically handle synchronization

**Next:** Chapter 37 - Mutexes & Locks
