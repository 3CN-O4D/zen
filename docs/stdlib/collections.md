# collections — container helpers

Container utilities in the spirit of Python's `collections`. `Counter` keeps
parity with the native tier; keys are stored in string form.

```zen
import collections

print collections.Counter([1, 1, 2])  # {"1": 2, "2": 1}
```

## Functions

| Function | Description |
|----------|-------------|
| `Counter(items)` | Frequency map of a list (keys stringified). |
| `chain(iterables)` | Concatenates a list of lists (non-list items are appended as-is). |
| `flatten(list)` | Recursively flattens nested lists. |

## Example

```zen
import collections

print collections.Counter(["a", "b", "a"])  # {"a": 2, "b": 1}
print collections.chain([[1, 2], [3]])      # [1, 2, 3]
print collections.flatten([[1, [2]], [3]])  # [1, 2, 3]
```
