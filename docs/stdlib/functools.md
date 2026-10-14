# functools — higher-order functions

Function tools in the spirit of Python's `functools`.

```zen
import functools
import operator

print functools.reduce(operator.add, [1, 2, 3], 0)  # 6
```

## Functions

| Function | Description |
|----------|-------------|
| `reduce(f, seq, initial)` | Left fold (`initial` required-ish; pass `null` to start from the first element). |
| `partial(f, a)` / `partial2(f, a, b)` / `partial3(f, a, b, c)` | Partial application (no variadics in Zen, so arities are fixed). |
| `lru_cache(maxsize)` / `lru_cache_unary(maxsize)` | Memoizing decorator (unary fast path included). |
| `wraps(wrapped)` / `update_wrapper(wrapper, wrapped)` | Metadata copying for decorators. |
| `total_ordering(cls)` / `cmp_to_key(cmp)` | Ordering helpers. |

## Example

```zen
import functools
import operator

print functools.reduce(operator.add, [1, 2, 3], 0)  # 6

let add1 = functools.partial(fn(a, b) { return a + b }, 1)
print add1(41)  # 42
```
