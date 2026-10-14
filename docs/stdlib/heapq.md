# heapq — binary heaps

Binary-heap operations in the spirit of Python's `heapq`. Heaps are plain
ascending lists with the smallest element at `h[0]`.

Zen passes arguments by value, so mutation cannot propagate back to the
caller: every function takes a heap and **returns** the updated heap —
rebind it (`let h = heapq.heappush(h, x)`). `heappop` returns a
`[top, heap]` pair for the same reason.

```zen
import heapq

let h = heapq.heapify([5, 1, 4])
let h = heapq.heappush(h, 0)
let r = heapq.heappop(h)
print r[0]  # 0
```

## Functions

| Function | Description |
|----------|-------------|
| `heapify(items)` | Builds a heap from a list. |
| `heappush(h, x)` | Returns the heap with `x` added. |
| `heappop(h)` | Returns `[smallest, heap]`; throws on an empty heap. |
| `heapreplace(h, x)` | Pops the smallest and pushes `x`; returns `[old_top, heap]`. |
| `heappushpop(h, x)` | Pushes `x` then pops; returns `[top, heap]`. |
| `nsmallest(n, items)` | The `n` smallest items, ascending. |
| `nlargest(n, items)` | The `n` largest items, descending. |

## Example

```zen
import heapq

let out = []
let h = heapq.heapify([5, 4, 3, 2, 1])
let k = 0
while k < 5 {
    let r = heapq.heappop(h)
    out.push(r[0])
    h = r[1]
    k += 1
}
print out  # [1, 2, 3, 4, 5]
```
