# Chapter 16: random Module

The `random` module provides pseudo-random number generation and random
selection utilities.

## 16.1 Random numbers

| Function | Description | Example |
|----------|-------------|---------|
| `random()` | Random float [0.0, 1.0) | `math.random()` |
| `randint(a, b)` | Random int [a, b] inclusive | `math.randint(1, 6)` |
| `randbelow(n)` | Random int [0, n) | `math.randbelow(10)` |
| `uniform(a, b)` | Random float [a, b] | `math.uniform(1.0, 10.0)` |
| `gauss(mu, sigma)` | Gaussian random | `math.gauss(0, 1)` |
| `seed(n)` | Seed the RNG | `math.seed(42)` |

```zen
import random

print(random.random())   # 0.374... (varies)
print(random.randint(1, 6))  # 1-6
print(random.randbelow(10))  # 0-9
print(random.uniform(1.0, 10.0))  # 1.0-10.0
print(random.gauss(0, 1))  # bell curve
```

## 16.2 Random sequences

| Function | Description | Example |
|----------|-------------|---------|
| `choice(seq)` | Random element | `random.choice(["a","b","c"])` |
| `choices(seq, k)` | k elements with replacement | `random.choices("abcd", 3)` |
| `sample(seq, k)` | k unique elements | `random.sample(1..10, 3)` |
| `shuffle(seq)` | Shuffle in place | `random.shuffle(my_list)` |

```zen
import random

let colors = ["red", "green", "blue"]
print(random.choice(colors))   # "green"

print(random.choices(colors, 3))  # ["red", "red", "blue"]

print(random.sample(1..10, 3))   # [3, 7, 1] (unique)

let deck = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
random.shuffle(deck)
print(deck)  # shuffled
```

## 16.3 Deterministic randomness (seed)

```zen
import random

random.seed(42)
print(random.randint(1, 100))  # same every run with seed 42
random.seed(42)
print(random.randint(1, 100))  # same as above
```

Use `seed` for reproducibility in tests, simulations, and games.

## 16.4 Quick reference

```zen
import random

random.random()        # float [0,1)
random.randint(1, 6)   # int [1,6]
random.randbelow(10)   # int [0,10)
random.uniform(1.0, 10.0)  # float [1,10]
random.gauss(0, 1)     # normal distribution
random.choice(["a","b","c"])   # random element
random.choices("abcd", 3)  # with replacement
random.sample(1..10, 3)      # unique sample
random.shuffle(my_list)  # in-place shuffle
random.seed(42)        # deterministic
```

## 16.5 Exercises

1. Simulate rolling two dice 1000 times and count the frequency of each sum.
2. Write a function `pick_winner(entries)` that picks a random entry.
3. Shuffle a deck of cards and deal 5 cards.
4. Generate a random password of 12 characters from letters and digits.
5. Use `random.seed` to make a simulation reproducible.
