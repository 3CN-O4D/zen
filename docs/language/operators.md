# Operators

Complete reference for every operator in Zen, including precedence, associativity, and gotchas.

## Arithmetic Operators

| Operator | Name | Example | Result |
|----------|------|---------|--------|
| `+` | Addition / Concatenation | `2 + 3` | `5` |
| `-` | Subtraction | `5 - 3` | `2` |
| `*` | Multiplication | `4 * 3` | `12` |
| `/` | Division | `10 / 3` | `3.333...` |
| `%` | Modulo | `10 % 3` | `1` |
| `**` | Exponentiation | `2 ** 10` | `1024` |

### String concatenation

The `+` operator concatenates strings:

```
print "Hello" + " " + "World"    // Hello World
print "Score: " + str(95)         // Score: 95
```

### Division is always floating-point

```
print 10 / 2        // 5
print 10 / 3        // 3.3333333333333335
print 10 / 4        // 2.5
```

### Modulo with floats

```
print 10 % 3        // 1
print 7.5 % 2.5     // 0
print -7 % 3        // -1 (sign follows the dividend)
```

### Exponentiation

```
print 2 ** 8        // 256
print 9 ** 0.5      // 3 (square root)
print 2 ** -1       // 0.5
```

---

## Comparison Operators

| Operator | Name | Example | Result |
|----------|------|---------|--------|
| `==` | Loose equality | `1 == 1.0` | `true` |
| `!=` | Loose inequality | `1 != "1"` | `true` |
| `===` | Strict equality | `1 === "1"` | `false` |
| `!==` | Strict inequality | `1 !== "1"` | `true` |
| `<` | Less than | `1 < 2` | `true` |
| `>` | Greater than | `2 > 1` | `true` |
| `<=` | Less or equal | `2 <= 2` | `true` |
| `>=` | Greater or equal | `3 >= 2` | `true` |

### Loose vs strict equality

```
// Loose (==) compares values
print 1 == 1.0       // true (same numeric value)
print "1" == 1       // false (different types)
print null == null    // true

// Strict (===) compares value AND type
print 1 === 1.0      // true (both numbers, same value)
print "1" === 1      // false (string vs number)
print true === 1      // false (bool vs number)
```

### Chained comparisons

```
print 1 < 5 < 10        // true: (1 < 5) and (5 < 10)
print 0 <= x < 100      // true if x is between 0 and 99
print 10 > 5 > 2        // true
print a == b == c        // true if all three are equal
print 1 < 5 > 3         // true: (1 < 5) and (5 > 3)
```

### String comparison

Comparisons on strings are lexicographic:

```
print "apple" < "banana"     // true
print "Z" < "a"              // true (uppercase before lowercase in ASCII)
print "abc" == "abc"         // true
```

---

## Identity Operators

| Operator | Name | Description |
|----------|------|-------------|
| `is` | Identity | Same object in memory |
| `is not` | Non-identity | Different objects |

```
let a = [1, 2, 3]
let b = [1, 2, 3]
let c = a

print a == b           // true (value equality)
print a === b          // false (different objects)
print a is b           // false (different objects)
print a is c           // true (same object)

print null is null     // true
print 1 is not "1"     // true
```

---

## Membership Operators

| Operator | Description |
|----------|-------------|
| `in` | Contained in |
| `not in` | Not contained in |

### String substring check

```
print "world" in "hello world"        // true
print "xyz" in "hello world"          // false
print "world" not in "hello world"    // false
```

### List contains check

```
let fruits = ["apple", "banana", "cherry"]
print "apple" in fruits               // true
print "grape" in fruits               // false
print "grape" not in fruits           // true
```

### Dict key check

```
let user = {"name": "Alice", "age": 30}
print "name" in user                  // true
print "email" in user                 // false
print "email" not in user             // true
```

---

## Logical Operators

| Operator | Alias | Name |
|----------|-------|------|
| `and` | `&&` | Logical AND |
| `or` | `\|\|` | Logical OR |
| `not` | `!` | Logical NOT |

### Short-circuit evaluation

The right side is only evaluated if needed:

```
// or: returns first truthy value
true or print("never runs")        // true (print not called)
false or print("runs")             // runs, prints null

// and: returns first falsy value, or last value
false and print("never runs")     // false (print not called)
true and print("runs")            // runs, prints null
```

### Practical uses

```
// Default value pattern
let name = user_name or "Anonymous"

// Guard pattern
let items = data and data["items"] or []

// Conditional execution
is_admin and delete_user(user_id)
```

### Truthiness in logical operators

```
print 0 or "default"       // "default"
print "" or "default"      // "default"
print null or "default"    // "default"
print false or "default"   // "default"
print 42 or "default"      // 42
print "hello" or "default" // hello
```

---

## Nullish Coalescing (`??`)

Returns the right side only when the left is `null`:

```
null ?? "default"          // "default"
"hello" ?? "default"       // hello
0 ?? "default"             // 0 (not null, kept)
"" ?? "default"            // "" (not null, kept)
false ?? "default"         // false (not null, kept)
```

### `??` vs `||`

```
// || returns the first truthy value
0 || "default"             // "default" (0 is falsy)

// ?? returns the first non-null value
0 ?? "default"             // 0 (not null, kept)

// This makes ?? better for defaults when 0 or "" are valid values
let timeout = config.timeout ?? 30000
// If config.timeout is 0, timeout stays 0
// With ||, timeout would become 30000
```

---

## Range Operators

| Operator | Alias | Description |
|----------|-------|-------------|
| `->` | inclusive range | Both ends included |
| `..` | inclusive range | Same as `->` |
| `to` | inclusive range | Same as `->` |

### Basic ranges

```
print 1 -> 5          // [1, 2, 3, 4, 5]
print 1 .. 10         // [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
print 1 to 10         // same
```

### Descending ranges

Direction is auto-detected:

```
print 5 -> 1          // [5, 4, 3, 2, 1]
print 10 -> 5         // [10, 9, 8, 7, 6, 5]
```

### Ranges with step

```
print 1 -> 10 by 2    // [1, 3, 5, 7, 9]
print 0 -> 10 by 3    // [0, 3, 6, 9]
print 10 -> 0 by -2   // [10, 8, 6, 4, 2, 0]
print 1 .. 10 @ 3     // [1, 4, 7, 10]
```

### Using `range()` function

```
print range(5)          // [0, 1, 2, 3, 4] (exclusive end)
print range(2, 5)       // [2, 3, 4]
print range(1, 10, 2)   // [1, 3, 5, 7, 9]
```

**Difference:** `1 -> 5` includes 5 (`[1, 2, 3, 4, 5]`), but `range(1, 5)` excludes 5 (`[1, 2, 3, 4]`).

---

## Spread Operator (`...`)

Unpacks iterables inside list and dict literals:

### List spread

```
let a = [1, 2, 3]
let b = [...a, 4, 5]
print b               // [1, 2, 3, 4, 5]

let c = [...a, ...a]
print c               // [1, 2, 3, 1, 2, 3]
```

### Dict spread

```
let defaults = {"color": "blue", "size": "medium"}
let overrides = {"color": "red"}
let merged = {...defaults, ...overrides}
print merged          // {color: red, size: medium}

// Overrides win (later keys take precedence)
let final = {...defaults, ...overrides, "weight": 10}
print final           // {color: red, size: medium, weight: 10}
```

---

## Ternary Conditional

```
let grade = "pass" if score >= 50 else "fail"
let label = "high" if x > 100 else "low" if x > 50 else "zero"
let max = a if a > b else b
```

### Nested ternary

```
let category =
    "senior" if age >= 60 else
    "adult" if age >= 18 else
    "child"

print category    // "adult" if age is 30
```

### Ternary as expression

Ternary returns a value, so it can be used anywhere an expression is valid:

```
print "Score: " + ("A" if score >= 90 else "B" if score >= 80 else "C")
```

---

## Bitwise Operators

| Operator | Name | Example | Result |
|----------|------|---------|--------|
| `&` | AND | `5 & 3` | `1` |
| `\|` | OR | `5 \| 3` | `7` |
| `^` | XOR | `5 ^ 3` | `6` |
| `~` | NOT | `~5` | `-6` |
| `<<` | Left shift | `5 << 1` | `10` |
| `>>` | Right shift | `5 >> 1` | `2` |

### Bitwise AND (`&`)

```
print 0b1010 & 0b1100    // 0b1000 = 8
print 5 & 3              // 1
```

### Bitwise OR (`|`)

```
print 0b1010 | 0b1100    // 0b1110 = 14
print 5 | 3              // 7
```

### Bitwise XOR (`^`)

```
print 0b1010 ^ 0b1100    // 0b0110 = 6
print 5 ^ 3              // 6
```

### Bitwise NOT (`~`)

```
print ~5                 // -6
print ~0                 // -1
```

### Shift operators

```
print 1 << 3             // 8 (1 * 2^3)
print 16 >> 2            // 4 (16 / 2^2)
print 0b1010 << 1        // 0b10100 = 20
```

---

## Safe Navigation (`?.`)

Access properties or call methods on values that might be `null`:

```
let user = null
print user?.name         // null (no error)

let data = {"info": null}
print data?.info?.city   // null (no error)
```

### Regular access crashes on null

```
let user = null
print user.name
// Error: cannot access member of null
```

### Safe navigation prevents the crash

```
let user = null
print user?.name         // null
print user?.name?.len    // null
```

---

## typeof Operator

Returns the type name as a string:

```
print typeof 42         // "number"
print typeof 3.14       // "number"
print typeof "hello"    // "string"
print typeof true       // "bool"
print typeof null       // "null"
print typeof [1, 2, 3]  // "list"
print typeof {a: 1}     // "dict"
print typeof (x) => x   // "function"
```

---

## Operator Precedence

Highest to lowest (top binds tightest):

| Precedence | Operators | Associativity |
|-----------|-----------|---------------|
| 1 (highest) | `**` | Right |
| 2 | `-` `!` `not` `typeof` `~` (unary) | Right |
| 3 | `*` `/` `%` | Left |
| 4 | `+` `-` | Left |
| 5 | `->` `..` `to` (range) | Right |
| 6 | `==` `!=` `<` `>` `<=` `>=` | Left |
| 7 | `is` `is not` | Left |
| 8 | `in` `not in` | Left |
| 9 | `&` | Left |
| 10 | `^` | Left |
| 11 | `\|` | Left |
| 12 | `<<` `>>` | Left |
| 13 | `and` `&&` | Left |
| 14 | `or` `\|\|` | Left |
| 15 | `??` | Left |
| 16 | `if` `else` (ternary) | Right |
| 17 (lowest) | `=` `+=` `-=` `*=` `/=` `%=` `??=` | Right |

### Precedence examples

```
// * before +
print 2 + 3 * 4        // 14, not 20

// ** is right-associative
print 2 ** 3 ** 2       // 512 (= 2 ** 9), not 64 (= 8 ** 2)

// Comparison before logical
print 1 < 2 and 3 > 2   // true: (1 < 2) and (3 > 2)

// Use parentheses to clarify
print (2 + 3) * 4       // 20
```

---

## Pro Tips

1. **Use `??` instead of `||` for defaults.** `||` treats `0`, `""`, and `false` as falsy.
2. **Chain comparisons.** `0 <= x < 100` reads naturally and works correctly.
3. **Use `?.` for safe access.** Prevents crashes when dealing with potentially null data.
4. **Use spread `{...}` for merging.** `{...defaults, ...overrides}` is cleaner than manual merging.
5. **Use `is` for identity.** `a is b` checks if two variables point to the same object.
6. **Remember `in` works on strings too.** `"@" in email` is a quick substring check.

---

## Common Mistakes

### Confusing `==` and `===`

```
print 1 == "1"      // true (loose — different types)
print 1 === "1"     // false (strict — different types)

// Use === when you care about type
if typeof x === "string" {
    print "x is a string"
}
```

### `||` vs `??`

```
let count = 0

// WRONG: || treats 0 as falsy
let result = count || 10    // result is 10, not 0!

// CORRECT: ?? only triggers on null
let result = count ?? 10    // result is 0
```

### Range is inclusive

```
// This includes 10:
for i in 1 -> 10 { print i }
// Prints: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10

// For exclusive end, use range():
for i in range(1, 10) { print i }
// Prints: 1, 2, 3, 4, 5, 6, 7, 8, 9
```

---

## See Also

- [Types](types.md) — Type system and coercion
- [Variables](variables.md) — Assignment and compound operators
- [Control Flow](control-flow.md) — Using operators in conditions
- [Collections](collections.md) — Spread operator with lists and dicts
