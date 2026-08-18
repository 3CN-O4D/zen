# Random Module (`random`)

Pseudo-random number generation.

```zen
random.random()              // float in [0.0, 1.0)
random.randint(1, 10)         // int in [1, 10]
random.randrange(1, 10)       // int in [1, 10)
random.choice(["a", "b"])    // one random element
random.choices(["a", "b"], 3) // 3 picks (with replacement)
random.sample([1,2,3,4], 2)   // 2 unique elements
random.shuffle(list)          // shuffles in place
random.uniform(2.5, 5.0)      // float in range
random.hex(8)                 // 16-char hex string
random.seed(42)               // seed the RNG
```
