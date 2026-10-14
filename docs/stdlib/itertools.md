# itertools — iterator tools

Combinatorial iterators in the spirit of Python's `itertools`. Everything
operates on (and returns) plain lists.

```zen
import itertools

print itertools.take(3, [1, 2, 3, 4])  # [1, 2, 3]
```

## Functions

| Function | Description |
|----------|-------------|
| `take(n, list)` / `drop(n, list)` | Prefix / remainder. |
| `enumerate(list)` | `[index, value]` pairs. |
| `zip(a, b)` | Paired elements (stops at the shorter). |
| `chain(iterables)` | Concatenates a list of lists. |
| `repeat(value, n)` | `n` copies. |
| `product(a, b)` | Cartesian pairs. |
| `permutations(list, r)` | Ordered arrangements (`r` defaults to full length). |
| `combinations(list, r)` | Unordered selections. |
| `accumulate(list)` | Running totals. |
| `range(start, stop, step)` | Numeric ranges (mirrors the `..` operator). |

## Example

```zen
import itertools

print itertools.combinations([1, 2, 3], 2)  # [[1, 2], [1, 3], [2, 3]]
print itertools.accumulate([1, 2, 3, 4])    # [1, 3, 6, 10]
print itertools.zip([1, 2], ["a", "b"])     # [[1, a], [2, b]]
```
