# random — Random numbers

The `random` module provides generators for various distributions. It is available globally as `random`.

```zen
# 1. Random float between 0.0 and 1.0
print(random.random())

# 2. Random integer between 1 and 10 (inclusive)
print(random.randint(1, 10))

# 3. Pick a random item from a list
var colors = ["red", "green", "blue"]
print(random.choice(colors))
```

## Functions

| Function | Description |
|----------|-------------|
| `random()` | Returns a random float in the range [0.0, 1.0). |
| `randint(a, b)` | Returns a random integer in the range [a, b] (inclusive). |
| `randrange(stop)` | Returns a random integer from `range(stop)`. |
| `uniform(a, b)` | Returns a random float in the range [a, b]. |
| `choice(list)` | Returns a random element from a non-empty list. |
| `choices(list, k)` | Returns a list of `k` elements chosen with replacement. |
| `sample(list, k)` | Returns a list of `k` unique elements chosen without replacement. |
| `shuffle(list)` | Shuffles a list in-place (returns a new shuffled list). |
| `hex(n)` | Returns a random hex string of length `n`. |
| `seed(n)` | Seeds the random number generator for reproducible results. |

## Examples

### Picking multiple items
```zen
var deck = ["A", "K", "Q", "J", "10"]
var hand = random.sample(deck, 2)
print("Drawn: ${hand}")
```

### Random hex string (e.g., for IDs)
```zen
var id = random.hex(8)
print("Generated ID: ${id}")
```

## See Also
- [crypto](crypto.md) — For cryptographically secure randomness.
- [uuid](uuid.md) — For unique identifiers.
