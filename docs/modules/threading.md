# Threading

## Basic Usage

```
threading.active()                      // active thread count
threading.current()                     // "MainThread"
threading.list()                        // list of active thread info dicts
threading.sleep(0.5)                    // sleep seconds (alias for sleep)
```

## Threads

```
threading.start(fn)                     // run fn in a daemon thread
threading.Thread(fn)                    // create a Thread object
  .start()                                // start the thread
  .join(timeout)                          // wait for completion
  .is_alive()                             // still running?
```

## Locks

```
threading.Lock()                        // mutual exclusion lock
  .acquire(blocking=True)                 // acquire the lock
  .release()                              // release the lock

threading.RLock()                       // reentrant lock
```

## Events

```
threading.Event()                       // event signalling
  .set()                                  // signal the event
  .clear()                                // reset
  .wait(timeout)                          // wait for signal
  .is_set()                               // check if set
```

## Semaphores & Barriers

```
threading.Semaphore(n=1)                // counting semaphore
threading.Barrier(n)                    // barrier for n threads
threading.Condition(lock)               // condition variable
```

## Queue

```
threading.Queue(maxsize=0)              // thread-safe FIFO queue
  .put(item)                              // enqueue
  .get()                                  // dequeue (blocks)
  .qsize()                                // approximate size
  .empty()                                // is empty?
  .task_done()                            // mark task complete
  .join()                                 // wait until all processed
```

## Example: Producer/Consumer

```
let q = threading.Queue()
let producer = threading.Thread(function() {
    for i in 1 -> 5 {
        q.put(i)
    }
})
let consumer = threading.Thread(function() {
    let results = []
    for i in 1 -> 5 {
        results.append(q.get())
    }
    results
})
producer.start()
consumer.start()
producer.join()
consumer.join()
```
