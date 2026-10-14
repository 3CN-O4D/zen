# operator — operator functions

First-class operator functions in the spirit of Python's `operator`, for use
with higher-order helpers like `functools.reduce`. `itemgetter`/`attrgetter`
take a single key; there is no `delitem` (no delete-key primitive).

```zen
import operator
import functools

print functools.reduce(operator.mul, [1, 2, 3, 4], 1)  # 24
```

## Functions

| Function | Description |
|----------|-------------|
| `add` / `sub` / `mul` / `truediv` / `pow` | Arithmetic (`/` is float division). |
| `floordiv` / `mod` / `divmod` | Python-style floor division, modulo, and both. |
| `neg` / `pos` / `abs` | Unary ops. |
| `lshift` / `rshift` | Bit shifts. |
| `and_` / `or_` / `xor` / `not_` / `truth` | Logic (trailing underscores avoid keywords). |
| `eq` / `ne` / `lt` / `le` / `gt` / `ge` | Comparisons. |
| `is_` / `is_not` | Equality / inequality. |
| `getitem` / `setitem` / `contains` | Indexing, index assignment (returns `null`), membership. |
| `getslice` / `concat` / `repeat` | Slicing via `substring`, concatenation, repetition. |
| `itemgetter(key)` / `attrgetter(key)` | Single-key getters returning accessor functions. |
| `methodcaller(name)` | Returns the named member (throws when absent). |
| `length_hint` / `countOf` / `indexOf` | Sequence helpers. |

## Example

```zen
import operator

print operator.floordiv(-7, 2)  # -4
print operator.mod(-7, 2)       # 1
print operator.divmod(7, 2)     # [3, 1]
```
