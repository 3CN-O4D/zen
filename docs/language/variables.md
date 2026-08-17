# Variables

Complete reference for variable declaration, scoping, destructuring, constants, and all assignment patterns in Zen.

## Declaration

### `let` — Mutable variables

```
let name = "Zen"
let count = 42
let pi = 3.14159
let active = true
let nothing = null
```

### `const` — Immutable constants

```
const PI = 3.14159
const MAX_RETRIES = 3
const API_URL = "https://api.example.com"
```

Constants **cannot** be reassigned:

```
const X = 10
X = 20
// Error: Cannot redefine constant 'X'
```

### Bare assignment — Auto-declaring

If a variable doesn't exist yet, bare assignment creates it:

```
counter = 0            // auto-declares as if `let counter = 0`
counter = counter + 1
print counter          // 1
```

If it already exists, bare assignment reassigns:

```
let name = "Alice"
name = "Bob"           // reassigns, no let needed
print name             // Bob
```

---

## Reassignment

```
let x = 10
x = 20                 // OK
x += 5                 // OK (x is now 25)
x -= 3                 // OK (x is now 22)
x *= 2                 // OK (x is now 44)
x /= 4                 // OK (x is now 11)
x %= 5                 // OK (x is now 1)
```

### Compound assignment operators

| Operator | Equivalent To |
|----------|--------------|
| `x += y` | `x = x + y` |
| `x -= y` | `x = x - y` |
| `x *= y` | `x = x * y` |
| `x /= y` | `x = x / y` |
| `x %= y` | `x = x % y` |
| `x &= y` | `x = x & y` |
| `x \|= y` | `x = x \| y` |
| `x ^= y` | `x = x ^ y` |
| `x <<= y` | `x = x << y` |
| `x >>= y` | `x = x >> y` |
| `x ??= y` | `x = x ?? y` (only if x is null) |

Works on variables, member access, and index access:

```
let obj = {"count": 0}
obj.count += 1
print obj.count    // 1

let list = [10, 20, 30]
list[0] += 5
print list[0]      // 15
```

### Postfix increment/decrement

```
let x = 5
x++                // x is now 6
x--                // x is now 5
```

Valid targets: variables, member access (`obj.count++`), index access (`list[0]++`).

---

## Scope Rules

### Block scoping

Variables declared with `let` inside a `{ }` are scoped to that block:

```
let x = 10
if true {
    let x = 20      // different x (block-scoped)
    print x          // 20
}
print x              // 10 (original x unchanged)
```

### Function scoping

Functions create their own scope:

```
let x = 10

function modify() {
    let x = 20       // local x, doesn't affect outer x
    print x          // 20
}

modify()
print x              // 10
```

### Loop scoping

Loop variables are scoped to the loop:

```
for i in 1 -> 5 {
    let doubled = i * 2
    print doubled
}
// doubled is not accessible here
```

### Nested scopes can read outer scopes

```
let greeting = "Hello"

function greet(name) {
    // Can read `greeting` from outer scope
    return greeting + ", " + name + "!"
}

print greet("World")    // Hello, World!
```

### Assignment targets the innermost scope

```
let x = 10

function update() {
    // This creates a NEW local x, doesn't modify outer x
    x = 20
}

update()
print x              // 10 (outer x unchanged)
```

---

## Destructuring

### Array destructuring with `let`

```
let [a, b] = [1, 2]
print a              // 1
print b              // 2

let [x, y, z] = [10, 20, 30]
print x, y, z       // 10, 20, 30
```

### Dict destructuring with `let`

```
let {name, age} = {name: "Alice", age: 30, email: "a@b.com"}
print name           // Alice
print age            // 30
```

Missing keys become `null`:

```
let {name, city} = {name: "Bob"}
print name           // Bob
print city           // null
```

### Tuple unpacking (bare assignment)

Comma-separated targets on the left of `=`:

```
a, b = 1, 2
print a              // 1
print b              // 2

x, y = [10, 20]
print x              // 10
print y              // 20

first, second = range(5)
print first          // 0
print second         // 1
```

### Throwaway with `_`

```
a, _, c = 1, 2, 3
print a              // 1
print c              // 3 (2 was discarded)

name, _ = ["Alice", "alice@example.com"]
print name           // Alice
```

### Count must match

```
a, b = 1, 2, 3
// Error: mismatched unpacking
```

**Note:** `let` does not support tuple unpacking — use bare assignment for that.

---

## Compound Assignment Details

### Nullish assignment (`??=`)

Only assigns if the variable is currently `null`:

```
let x = null
x ??= "default"
print x              // default

let y = "existing"
y ??= "other"
print y              // existing (unchanged)

let z = 0
z ??= "other"
print z              // 0 (not null, so kept)
```

### Bitwise compound assignment

```
let flags = 0b1010
flags |= 0b0100      // flags = 0b1110
print flags          // 14

flags &= 0b1100      // flags = 0b1100
print flags          // 12

flags ^= 0b0010      // flags = 0b1110
print flags          // 14

let shifted = 1
shifted <<= 4        // shifted = 16
print shifted        // 16

shifted >>= 2        // shifted = 4
print shifted        // 4
```

---

## Special Variables

Zen provides several built-in special variables:

| Variable | Description | Example |
|----------|-------------|---------|
| `_url` | Current page URL | `"https://example.com"` |
| `__url` | Previous page URL | — |
| `___url` | URL before previous | — |
| `_time` | Current time (HH:MM:SS) | `"14:30:00"` |
| `_date` | Current date (YYYY-MM-DD) | `"2026-01-15"` |
| `_dir` | Current working directory | `"/home/user"` |
| `_version` | Zen version string | `"0.x.x"` |
| `_` | Last expression result | — |
| `_timeout` | Default timeout (ms) | `30000` |

### Using `_` (last result)

```
zen ❯ 2 + 2
4
zen ❯ _ * 10
40
zen ❯ _ + 1
41
```

### Using `_timeout`

```
_timeout = 5000          // 5000ms (5 seconds)
_timeout = "3s"          // 3 seconds
_timeout = "1.5s"        // 1.5 seconds
_timeout = "500ms"       // 500 milliseconds
_timeout = "2m"          // 2 minutes
print _timeout           // 30000 (default in ms)
```

---

## Multiple Assignment

### Assign same value to multiple variables

```
let a = b = c = 0
print a              // 0
print b              // 0
print c              // 0
```

### Swap variables

```
let x = 10
let y = 20
x, y = y, x
print x              // 20
print y              // 10
```

---

## Variable Naming Rules

### Valid names

```
let name = "Zen"           // lowercase
let PI = 3.14              // uppercase
let _private = true        // leading underscore
let camelCase = 42         // camelCase
let snake_case = true      // snake_case
let name2 = "v2"           // digits allowed (not first char)
let $special = true        // dollar sign allowed
```

### Invalid names

```
// let 2name = "bad"       // ERROR: can't start with digit
// let my-var = "bad"      // ERROR: hyphens not allowed
// let my var = "bad"      // ERROR: spaces not allowed
// let let = "bad"         // ERROR: reserved keyword
// let function = "bad"    // ERROR: reserved keyword
```

### Reserved keywords

These cannot be used as variable names:

```
let, const, if, elif, else, for, while, function, def, return,
break, continue, class, new, extends, import, from, include,
load, as, native, try, catch, finally, throw, warn, super,
typeof, and, or, not, is, switch, case, default, lambda,
true, false, null, print, go, fill, click, wait, scroll,
back, forward, refresh, shot, download, execute, assert
```

---

## Pro Tips

1. **Use `const` by default.** Only use `let` when you need to reassign.
2. **Use descriptive names.** `user_count` is better than `uc` or `n`.
3. **UPPERCASE for constants.** `MAX_RETRIES = 3` signals immutability.
4. **Use `_` for throwaway values.** `name, _ = get_user()` makes intent clear.
5. **Use `??=` for defaults.** `config = config ?? default_config` is concise.
6. **Beware block scope.** Variables inside `{}` don't leak out.

---

## Common Mistakes

### Reassigning a constant

```
const MAX = 10
MAX = 20
// Error: Cannot redefine constant 'MAX'

// Solution: Use let if you need to reassign
let max = 10
max = 20
```

### Forgetting that bare assignment creates new variables

```
function count() {
    n = n + 1    // This creates a local `n`, doesn't modify outer
    return n
}

let n = 0
count()          // returns 1
count()          // returns 1 (not 2!)
print n          // 0 (outer n unchanged)

// Solution: Pass and return explicitly
function count(n) {
    return n + 1
}
```

### Shadowing variables

```
let x = 10
if true {
    let x = 20     // This shadows outer x
    print x        // 20
}
print x            // 10 (outer x)
```

---

## See Also

- [Types](types.md) — Every type with examples
- [Operators](operators.md) — All operators and precedence
- [Functions](functions.md) — Scoping in closures
- [Control Flow](control-flow.md) — Scoping in loops and conditionals
