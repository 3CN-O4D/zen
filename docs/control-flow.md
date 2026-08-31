# Control Flow

Zen's control flow is statement-based (`if`, `while`, `for`, `switch`) with
expression-level matching (`match` / `when`) and structured error handling
(`try` / `catch` / `finally`).

## If / elif / else

`if` blocks are required — a single statement does **not** implicitly form a
block:

```zen
if score >= 90 {
    print "A"
} elif score >= 80 {
    print "B"
} else {
    print "C"
}
```

```zen
if true print 1        # Error: expected `{`, found `print`
```

Both `elif` and `else if` spellings work:

```zen
if a { ... } elif b { ... } else { ... }
if a { ... } else if b { ... } else { ... }
```

Conditions use truthiness: `null`, `0`, `""`, `[]`, `{}` are falsy; everything
else is truthy.

### If as an expression

`if ... { } else { }` can return a value — each block's final expression (or
`return`) is the branch value:

```zen
var parity = if n % 2 == 0 { "even" } else { "odd" }
print parity
```

(For compact branches prefer the ternary forms — see operators.)

## While

A `while` loop repeats while its condition is truthy. Block required:

```zen
var i = 0
while i < 5 {
    print i
    i = i + 1
}
```

There is no `do ... while` — restructure with an explicit first iteration or a
`for` loop.

## For

`for` iterates until a list is exhausted. **Iteration sources must be lists**:

```zen
for x in [10, 20, 30] { print x }
for i in 0 .. 5 { print i }            # [0,1,2,3,4]
for i in 1 -> 3 { print i }            # [1,2,3]
```

### Multiple loop variables

`for a, b in listOfPairs` unpacks each element:

```zen
for name, age in [["ada", 36], ["grace", 79]] {
    print name + " is " + str(age)
}
```

### Iterating dicts and strings

`for` only accepts lists — to iterate a dict's keys/values use its methods:

```zen
var d = { a: 1, b: 2 }

for k in d.keys() { print k }          # a  b
for v in d.values() { print v }        # 1  2
for pair in d.items() { print pair }   # [a, 1]  [b, 2]

for ch in "abc" { print ch }           # Error: for requires a list
```

Use `"abc".split("")` → `["a", "b", "c"]` if you genuinely need characters.

### break and continue

Work in the innermost `for`/`while`:

```zen
for x in [1, 2, 3, 4] {
    if x == 2 { continue }
    if x == 4 { break }
    print x                            # 1 3
}
```

## Switch

A classic multi-way branch. `case` arms can use a colon body or a block:

```zen
var name = "b"
switch name {
    case "a": print "Alice"
    case "b": print "Bob"
    default:  print "someone else"
}
```

Block form:

```zen
switch status {
    case 200 { print "ok" }
    case 404 { print "missing" }
    default  { print "other" }
}
```

Rules:

- Arms are tested in order; the first match wins.
- There is **no fall-through** — after a matching arm runs, the switch ends.
- `case` values use normal equality (deep, typed).
- `default` is optional.

## Match and when (expressions)

`match` is the expression form of `switch` — it returns a value. Arms are
`pattern: value` (or `=>`), may be guarded, and commas/newlines separate them:

```zen
var x = 3
var word = match x {
    1: "one",
    3: "three",
    _: "other"
}
print word                          # three
```

### Guards with `when`-style predicates

```zen
var score = 87
var grade = match score {
    n if n >= 90 => "A",
    n if n >= 80 => "B",
    _            => "C"
}
print grade                          # B
```

### Matching lists and other values

Patterns reuse deep equality:

```zen
print match [1, 2] {
    [1, 2]: "pair"
    _: "other"
}                                    # pair
```

### `when` — no subject

`when { }` matches on conditions alone:

```zen
var v = 3
print when {
    v > 5: "big"
    _: "small"
}                                    # small
```

Use `match`/`when` anywhere an expression is expected — conditionals, returns,
even print arguments.

## Commands & `command_call`

A built-in function may be *called* without parentheses as a statement:

```zen
print "no parens"        # print is a statement keyword anyway
sleep 2                  # same as sleep(2)
exit 1                   # same as exit(1)
assert 1 == 1            # same as assert(1 == 1)
```

This only works for **built-in** functions (`sleep`, `exit`, `assert`, ...),
not for your own functions:

```zen
fn greet(name) { print name }
greet "Ada"              # Error: expected expression / undefined
greet("Ada")             # ok
```

## try / catch / finally

`try` blocks are protected by `catch` (alias `except`) and `finally`:

```zen
try {
    risky_call()
} catch as err {
    print "caught: " + err
} finally {
    print "always runs"
}
```

All catch forms:

```zen
try { throw "x" } catch { }                    # ignore errors
try { throw "x" } catch as e { print e }       # bind the value
try { throw "x" } catch e { print e }          # same as `catch as e`
try { throw "x" } catch errors.TypeError { }   # typed: only that error class
try { throw "x" } catch errors.IndexError as e { print e }
```

`finally` is optional and runs whether or not an error was raised:

```zen
try {
    work()
} finally {
    cleanup()
}
```

See [errors](errors.md) for the full model, including custom error types.

## Gotchas

| Situation | Reality |
|-----------|---------|
| `if cond stmt` (no block) | parse error — braces are required |
| `for k in dict` | `for requires a list` — use `d.keys()` |
| `for ch in "abc"` | `for requires a list` — use `split("")` |
| `switch` fall-through | doesn't happen |
| `do ... while` | not supported |
| `goto` / `label` | not supported |
| Variables declared in a loop body leak to the enclosing scope | there is no block scope |