# bisect — binary search

Binary search over sorted lists in the spirit of Python's `bisect`.
Insertion returns a **new** list — rebind it (`let xs = bisect.insort(xs, x)`),
because call arguments arrive by value.

```zen
import bisect

print bisect.bisect_left([1, 3, 5], 4)  # 2
```

## Functions

| Function | Description |
|----------|-------------|
| `bisect_left(a, x)` | Leftmost insertion index for `x` in sorted `a`. |
| `bisect_right(a, x)` | Rightmost insertion index for `x` in sorted `a`. |
| `insort_left(a, x)` | New list with `x` inserted at the leftmost position. |
| `insort_right(a, x)` | New list with `x` inserted at the rightmost position. |

## Example

```zen
import bisect

let xs = [1, 3, 5, 7]
print bisect.bisect_left(xs, 5)    # 2
print bisect.bisect_right(xs, 5)   # 3
print bisect.insort_left(xs, 4)    # [1, 3, 4, 5, 7]
print bisect.insort_right([1, 3, 3, 5], 3)  # [1, 3, 3, 3, 5]
```
