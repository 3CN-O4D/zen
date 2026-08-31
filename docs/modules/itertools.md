# itertools — Iterator helpers

The `itertools` module provides functions that create and manipulate iterators for efficient looping. It is available globally as `itertools`.

```zen
# 1. Enumerate with index
for pair in itertools.enumerate(["a", "b"]):
    print(pair) # [0, a], [1, b]

# 2. Zip two lists
for pair in itertools.zip([1, 2], ["x", "y"]):
    print(pair) # [1, x], [2, y]
```

## Functions

| Function | Description |
|----------|-------------|
| `enumerate(list)` | Pairs each element with its index. |
| `zip(...lists)` | Iterates over multiple lists in parallel. |
| `range(stop)` | Alias for the global `range()`. |
| `chain(...lists)` | Chains multiple lists together. |
| `repeat(val, n)` | Returns a list containing `val` repeated `n` times. |
| `product(...lists)` | Cartesian product of the input lists. |
| `permutations(l, k?)` | All possible permutations of length `k`. |
| `combinations(l, k)` | All possible combinations of length `k`. |
| `accumulate(l)` | Running sums/accumulations. |
| `take(n, l)` | Returns the first `n` items. |
| `drop(n, l)` | Skips the first `n` items. |

## Examples

### Generating a grid (Cartesian Product)
```zen
var x = [1, 2]
var y = ["a", "b"]
var points = itertools.product(x, y)
# [[1, a], [1, b], [2, a], [2, b]]
```

## See Also
- [collections](collections.md) — For `Counter` and `flatten`.
- [lists](../lists.md) — For core list methods like `.map()` and `.filter()`.
