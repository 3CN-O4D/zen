# Chapter 16: random Module

The `random` module provides pseudo-random number generation and random selection utilities.

## 16.1 Random numbers

| Function | Description | Example |
|----------|-------------|---------|
| `random()` | Random float [0.0, 1.0) | `random.random()` |
| `randint(a, b)` | Random int [a, b] inclusive | `random.randint(1, 6)` |
| `randbelow(n)` | Random int [0, n) | `random.randbelow(10)` |
| `uniform(a, b)` | Random float [a, b] | `random.uniform(1.0, 10.0)` |
| `gauss(mu, sigma)` | Gaussian random | `math.gauss(0, 1)` |
| `seed(n)` | Seed the RNG | `random.seed(42)` |

```zen
import random

print(random.random())   # 0.374... (varies each run)
print(random.randint(1, 6))  # 1-6
print(random.randbelow(10))  # 0-9
print(random.uniform(1.0, 10.0))  # 1.0-10.0
print(random.gauss(0, 1))   # bell curve
```

### Result ranges

| Function | Range | Inclusive? |
|----------|-------|------------|
| `random()` | [0.0, 1.0) | lower inclusive, upper exclusive |
| `randint(a, b)` | [a, b] | both inclusive |
| `randbelow(n)` | [0, n) | lower inclusive, upper exclusive |
| `uniform(a, b)` | [a, b] | both inclusive (may include endpoint) |

```zen
import random

# randint is inclusive on both ends
for i in 1..1000 {
    let r = random.randint(1, 6)
    if r < 1 or r > 6 {
        print("out of range!")
        break
    }
}

# randbelow is [0, n)
for i in 1..1000 {
    let r = random.randbelow(10)
    if r < 0 or r >= 10 {
        print("out of range!")
        break
    }
}
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

### choice
Picks one random element from a non-empty sequence. Raises if the sequence is empty.

```zen
try {
    random.choice([])
} catch as e {
    print(e.msg)  # IndexError or ValueError
}
```

### choices (with replacement)
Returns `k` elements, each independently chosen. Duplicates are possible.

```zen
print(random.choices(["a", "b"], 5))   # ["a", "b", "a", "a", "b"]
```

### sample (without replacement)
Returns `k` unique elements. Raises if `k > len(seq)`.

```zen
try {
    random.sample([1, 2], 5)
} catch as e {
    print(e.msg)  # sample larger than population
}
```

### shuffle (in place)
Shuffles the list in place using Fisher-Yates. Returns `nil`.

```zen
let deck = [1, 2, 3, 4, 5]
random.shuffle(deck)
print(deck)  # e.g. [3, 1, 5, 2, 4]
print(len(deck))  # 5 (same length, reordered)
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

```zen
import random

random.seed(12345)
let a = random.randint(1, 1000)
random.seed(12345)
let b = random.randint(1, 1000)
print(a == b)  # true
```

## 16.4 Common patterns

### Roll dice

```zen
import random

func roll_dice(sides, count) {
    let total = 0
    for i in 1..count {
        total = total + random.randint(1, sides)
    }
    return total
}

print(roll_dice(6, 2))   # 2-12
```

### Pick a random element

```zen
func pick_winner(entries) {
    if len(entries) == 0 {
        return nil
    }
    return random.choice(entries)
}

print(pick_winner(["Alice", "Bob", "Carol"]))
```

### Shuffle and deal

```zen
func deal(deck, hand_size) {
    random.shuffle(deck)
    let hand = deck[0:hand_size]
    let remaining = deck[hand_size:]
    return #(hand, remaining)
}

let deck = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13] * 4
let #(hand, rest) = deal(deck, 5)
print(hand)   # 5 cards
print(len(rest))  # 47
```

### Generate a random password

```zen
import random

let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%"

func random_password(length) {
    let pw = []
    for i in 1..length {
        pw.push(random.choice(chars))
    }
    return "".join(pw)
}

print(random_password(12))
```

### Simulate coin flips

```zen
import random

func flip_coin() {
    return random.randint(0, 1) == 0 ? "heads" : "tails"
}

let heads = 0
let tails = 0
for i in 1..1000 {
    if flip_coin() == "heads" {
        heads = heads + 1
    } else {
        tails = tails + 1
    }
}
print("heads: " + str(heads) + ", tails: " + str(tails))
```

## 16.5 Quick reference

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

## 16.6 Exercises

1. Simulate rolling two dice 1000 times and count the frequency of each sum.
2. Write a function `pick_winner(entries)` that picks a random entry.
3. Shuffle a deck of cards and deal 5 cards.
4. Generate a random password of 12 characters from letters and digits.
5. Use `random.seed` to make a simulation reproducible.
6. Write `weighted_choice(items, weights)` that picks an item with probability proportional to its weight.
7. Write `bootstrap_sample(data, k)` that samples `k` elements with replacement from `data`.


## 16.12 random module — complete reference

| Function | Purpose | Example |
|----------|---------|---------|
| `seed(n)` | seed the generator | `random.seed(42)` |
| `random()` | float in [0, 1) | `random.random()` |
| `randint(a, b)` | int in [a, b] inclusive | `random.randint(1, 6)` |
| `randrange(start, stop=None, step=1)` | int from range | `random.randrange(0, 10, 2)` |
| `choice(seq)` | random element | `random.choice(["a","b"])` |
| `choices(seq, k=1)` | k elements with replacement | `random.choices([1,2], k=3)` |
| `sample(seq, k)` | k unique elements | `random.sample([1,2,3], 2)` |
| `shuffle(seq)` | shuffle in place | `random.shuffle(xs)` |
| `uniform(a, b)` | float in [a, b] | `random.uniform(1.0, 2.0)` |
| `hex(n)` | n random hex chars | `random.hex(16)` |

### Usage

```zen
import random
random.seed(42)
print(random.random())     # deterministic given seed
print(random.randint(1, 100))
print(random.choice(["rock","paper","scissors"]))
let deck = list(range(1, 53))
random.shuffle(deck)
print(deck[:5])
```

### Cryptographic randomness

For security-sensitive tokens, do NOT use `random`. Use `crypto` module
functions (`crypto.generate_secret`, `crypto.secure_token`, `crypto.bytes`).

### Exercises

1. Roll a die 1000 times and count frequencies.
2. Shuffle a list and verify it's a permutation.
3. Generate a random password of length 12 from letters+digits.
4. Pick a random element weighted by a list of weights (manual).
5. Simulate coin flips until 3 heads in a row; count flips.
6. Generate 6 unique lottery numbers from 1..49.
7. Compare `random()` distribution (bucket into 10 bins).
8. Seed twice with same value; confirm identical sequences.
9. Implement `dice(sides, count)` returning sum.
10. Use `crypto.secure_token()` for a session token instead of `random`.



## 16.12 Worked example — dice and shuffle

```zen
import random

random.seed(42)
print(random.randint(1, 6))       # die roll
let deck = list(range(1, 53))
random.shuffle(deck)
print(deck[:5])                   # first 5 shuffled cards
print(random.choice(["rock","paper","scissors"]))
print(random.uniform(1.0, 2.0))   # random float
```

## 16.13 Troubleshooting randomness

| Symptom | Cause | Fix |
|---------|-------|-----|
| same sequence every run | seed fixed | use different seed or no seed |
| not secure | `random` is not CSPRNG | use `crypto.secure_token` for secrets |
| out of range | bad args to randint | check bounds |

Never use `random` for passwords/tokens; use `crypto`.

## 16.14 Exercises

1. Roll a die 1000 times; count frequencies.
2. Shuffle a list; verify it's a permutation.
3. Generate a 12-char password (letters+digits).
4. Pick 6 unique lottery numbers from 1..49.
5. Simulate coin flips until 3 heads in a row.
