# collections — Data structures

The `collections` module provides specialized data structures and helpers for working with containers. It is available globally as `collections`.

```zen
# 1. Counting occurrences
var counts = collections.Counter(["a", "b", "a", "c", "a"])
print(counts) # {a: 3, b: 1, c: 1}

# 2. Flattening a list
var flat = collections.flatten([[1, 2], [3, 4]])
print(flat) # [1, 2, 3, 4]
```

## Functions

| Function | Description |
|----------|-------------|
| `Counter(list)` | Counts the frequency of items in a list and returns a dict. |
| `flatten(list)` | Flattens a list of lists by one level. |
| `chain(...lists)` | Lazily (or greedily in current implementation) chains multiple lists together. |

## Examples

### Finding most common items
```zen
var text = "apple banana apple cherry banana apple"
var words = text.split(" ")
var freq = collections.Counter(words)

print("Apples: ${freq.get('apple')}")
```

## See Also
- [lists](../lists.md) — Core list documentation.
- [itertools](itertools.md) — More iterator helpers.
