# Chapter 53: Garbage Collection and Memory Management

*How Zen 3.0 reclaims memory, the collector design, reference cycles, and how
to write memory-friendly code.*

## 18.1 Overview

Zen is a garbage-collected language. You do not `free` memory manually. The
runtime automatically reclaims heap objects (lists, dicts, strings, instances,
etc.) when they become unreachable.

The exact collector implementation is build-configured, but the semantics are
consistent: unreachable memory is reclaimed; reachable memory is kept.

## 18.2 What gets collected

Heap-allocated objects:
- strings, lists, dicts, sets, tuples, bytes
- class instances
- closures / function objects
- modules
- FFI foreign handles (with finalizers)

Immediate values (nil, bool, int, float) are not heap-allocated and need no
collection.

## 18.3 Reachability

An object is **reachable** if it can be reached from a root:
- global/module variables
- local variables on the stack
- currently executing frames
- registers holding values
- other reachable objects (transitively)

An object is **unreachable** (garbage) when no root can reach it, directly or
indirectly. The collector reclaims unreachable objects.

## 18.4 Reference counting vs tracing

Zen's collector may use reference counting, tracing (mark-and-sweep), or a
hybrid, depending on build:

| Approach | How it works | Pros | Cons |
|----------|--------------|------|------|
| Reference counting | count references; free at zero | prompt reclamation | cycles leak; per-ref overhead |
| Tracing (mark-sweep) | mark reachable from roots, sweep rest | handles cycles | pause-time; throughput cost |
| Hybrid | refcount + periodic cycle collection | prompt + cycle-safe | more complex |

## 18.5 Reference cycles (the classic leak)

Two or more objects referencing each other form a cycle. With pure reference
counting, their counts never reach zero even if nothing outside references them.

```zen
class Node {
    var peer = nil
}
let a = new Node()
let b = new Node()
a.peer = b
b.peer = a   # cycle
# drop external refs:
a = nil
b = nil
# a and b still reference each other → leak (pure refcount)
```

With a tracing or hybrid collector, the cycle is detected and collected.
With pure refcounting, break it manually:

```zen
a.peer = nil
b.peer = nil
```

## 18.6 `shared` and thread-shared memory

`shared x = expr` wraps the value in an `Arc<Mutex<_>>`-like container. The
wrapper itself is reference counted; the inner `Mutex` protects the value for
concurrent access. The wrapped value is freed when the last shared reference
drops.

## 18.7 Writing memory-friendly code

1. **Drop large references when done:** `big = nil` after processing a huge
   list helps the collector reclaim it sooner.
2. **Avoid unbounded growth:** don't append to a global list forever; cap it or
   evict old entries.
3. **Break cycles in graphs:** null out back-references when tearing down
   object graphs.
4. **Close resources promptly:** files, sockets, FFI handles — use `with` /
   `finally` rather than waiting for GC.
5. **Prefer immutable values:** fewer surprise aliases means easier reasoning
   about lifetime.
6. **Don't micro-manage:** trust the collector; optimize only after measuring.

## 18.8 Memory leaks to watch for

| Leak | Cause | Fix |
|------|-------|-----|
| reference cycle | A↔B mutual refs | break one side |
| growing global cache | no eviction | size-limit / LRU |
| unremoved event listeners | callbacks hold refs | unsubscribe |
| open files/sockets | not closed | `with` / `finally` |
| FFI handle not freed | missing finalizer | call release in `finally` |
| closure capturing large object | closure outlives need | nil the captured ref |

## 18.9 GC and performance

Collection pauses can affect latency-sensitive code. If your build exposes GC
tuning:
- increase the allocation threshold to collect less often (higher throughput,
  larger pauses)
- decrease it for more frequent, smaller pauses (better latency)

Most applications need no tuning.

## 18.10 Exercises

1. Draw a reference cycle between two objects and mark which refs to break.
2. Write a tiny object graph, drop external refs, and explain what the GC sees.
3. Show how `big = nil` helps reclaim a large list.
4. Explain why a global cache that never evicts leaks memory.
5. Write `with_resource` using `try/finally` for a file.
6. Describe the difference between reachable and unreachable.
7. Name three heap types and three immediate types.
8. Why does `shared` need both refcounting and a mutex?
9. Implement an LRU cache with eviction (conceptually).
10. What can you do if GC pauses hurt your application's latency?

'''
print('gc done')
