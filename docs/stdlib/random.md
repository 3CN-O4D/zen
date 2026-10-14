# random — randomness

Randomness in the spirit of Python's `random`.

```zen
import random

random.seed(42)
print random.randint(1, 6)
```

## Functions

| Function | Description |
|----------|-------------|
| `seed(n)` | Seeds the generator (same seed replays the sequence). |
| `random()` | Float in `[0, 1)`. |
| `randint(a, b)` | Integer in `[a, b]`. |
| `randrange(start, stop, step)` | Element of `range(start, stop, step)`. |
| `choice(items)` | One random element. |
| `choices(items, k)` | `k` random elements (with replacement). |
| `sample(items, k)` | `k` unique random elements. |
| `shuffle(items)` | New shuffled list. |
| `uniform(a, b)` | Float in `[a, b]`. |
| `hex(k)` | Random hex string (`k` digits). |

## Example

```zen
import random

random.seed(7)
print random.sample([1, 2, 3, 4, 5], 3)
print random.shuffle(["a", "b", "c"])
```
