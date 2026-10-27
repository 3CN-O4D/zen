# Chapter 36: Channels

Channels send values between threads. A channel is created, values are sent into it, and received from it on the other end. Channels are unbuffered — `send` blocks until a `recv` is waiting, and vice versa.

## 36.1 The `channel` module

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
| `channel.send(ch, value)` | Send a value (blocks until received) | `channel.send(ch, 42)` |
| `channel.recv(ch)` | Receive a value (blocks until sent) | `channel.recv(ch)` |

```zen
import channel

let ch = channel.make()

channel.send(ch, "first")
channel.send(ch, "second")
channel.send(ch, "third")

print(channel.recv(ch))  # first
print(channel.recv(ch))  # second
print(channel.recv(ch))  # third
```

## 36.2 Blocking semantics

Channels are unbuffered and synchronous:

- `send(ch, v)` blocks until some thread calls `recv(ch)`.
- `recv(ch)` blocks until some thread calls `send(ch, v)`.
- The value is handed directly from sender to receiver.

```zen
import channel
import thread

let ch = channel.make()

// Sender thread
thread.spawn(fn() {
    channel.send(ch, "ping")
    print("sent")
})

// Receiver (main thread)
let msg = channel.recv(ch)
print("received: " + msg)  // received: ping
```

## 36.3 Producer-consumer

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

Output:
```
produced: 1
consumed: 1
produced: 2
consumed: 2
...
produced: 5
consumed: 5
all done
```

## 36.4 Multiple producers and consumers

```zen
import channel
import thread

let ch = channel.make()

func producer(id, count) {
    for i in 1..count {
        channel.send(ch, #(id, i))
    }
}

func consumer(id) {
    while true {
        let v = channel.recv(ch)
        if v == nil {
            break
        }
        print("consumer " + str(id) + " got: " + str(v[1]))
    }
}

// Start 2 producers
thread.spawn(producer(1, 3))
thread.spawn(producer(2, 3))

// Send 2 sentinels (one per consumer)
channel.send(ch, nil)
channel.send(ch, nil)

// Start 2 consumers
thread.spawn(consumer(1))
thread.spawn(consumer(2))
```

## 36.5 Channels and shared values

Use `shared` for counters, `channel` for message passing:

```zen
import channel
import shared
import thread

let ch = channel.make()
let counter = shared.create(0)

func worker(id) {
    for i in 1..10 {
        shared.increment(counter, 1)
        channel.send(ch, #(id, i))
    }
}

let t1 = thread.spawn(worker(1))
let t2 = thread.spawn(worker(2))

// Collect results
for i in 1..20 {
    let v = channel.recv(ch)
    print("got: " + str(v))
}

thread.join(t1)
thread.join(t2)
print("counter: " + str(shared.get(counter)))  # 20
```

## 36.6 Common pitfalls

- **`recv` blocks forever** if no value is ever sent. Always ensure a sender exists, or use a sentinel value (like `nil`) to signal completion.
- **`send` blocks forever** if no receiver is waiting.
- **Channels are unbuffered.** There's no queue — sender and receiver must rendezvous.
- **Forgetting to join threads.** The program may exit before threads finish sending.

## 36.7 Quick reference

```zen
import channel

let ch = channel.make()
channel.send(ch, "msg")
let v = channel.recv(ch)

// Producer-consumer pattern
channel.send(ch, nil)  // sentinel to signal completion
```

## 36.8 Exercises

1. Write a program with 1 producer and 1 consumer using a channel.
2. Write a program with 2 producers and 1 consumer.
3. Write `parallel_map(func, items)` that runs `func` on each item in a thread and sends results via a channel.
4. Write a pipeline: stage1 generates numbers, stage2 squares them, stage3 prints them. Connect stages with channels.
5. Write a program where multiple workers pull jobs from a channel channel and send results to another channel.
