# Chapter 4: Control Flow

Zen supports conditional execution, looping, pattern matching, and structured
exception handling. This chapter covers `if`/`elif`/`else`, `while`, `for`,
`break`/`continue`, `match`, `when`, and `try`/`catch`/`throw`/`finally`.

## 4.1 Conditionals: `if` / `elif` / `else`

```zen
let score = 87

if score >= 90 {
    print("A")
} elif score >= 80 {
    print("B")
} elif score >= 70 {
    print("C")
} else {
    print("Below C")
}
# prints: B
```

- `if` / `elif` / `else` chains are evaluated in order; the first true branch
  runs and the rest are skipped.
- The condition must be a truthy value. Zen truthiness: everything is truthy
  except `nil` and `false`.
- Braces `{ }` are required around bodies.

### `if` as an expression

`if` can appear in expression position via a ternary form:

```zen
let status = if score >= 60 { "pass" } else { "fail" }
print(status)  # pass
```

## 4.2 Loops: `while` and `for`

### `while`

```zen
let i = 0
let total = 0
while i < 100 {
    total = total + i
    i = i + 1
}
print(total)  # 4950
```

`while` loops continue while the condition is truthy.

### `for`

`for` iterates over iterable values: lists, ranges, strings, dicts, and more.

```zen
# Iterate a list
for v in [1, 2, 3, 4] {
    print(v)
}

# Iterate a range
for i in 0..5 {
    print(i)  # 0, 1, 2, 3, 4
}

# Iterate a string
for ch in "Zen" {
    print(ch)
}

# Iterate dict items
let d = {"a": 1, "b": 2}
for k in d {
    print(k, d[k])
}
```

### `break` and `continue`

```zen
# break: exit the loop early
for i in 0..10 {
    if i * i > 30 {
        break
    }
    print(i)
}
# prints: 0, 1, 2, 3, 4, 5

# continue: skip to the next iteration
let odd_sum = 0
for i in 1..10 {
    if i % 2 == 0 {
        continue
    }
    odd_sum = odd_sum + i
}
print(odd_sum)  # 25
```

### Nested loops

```zen
let pairs = 0
for a in [1, 2, 3] {
    for b in [1, 2, 3] {
        if b > a {
            continue
        }
        pairs = pairs + 1
    }
}
print(pairs)  # 6
```

## 4.3 Pattern matching: `match`

`match` compares a subject against a series of patterns. The first matching
arm determines the result.

```zen
let status = match "ready" {
    "loading": "spinner",
    "ready" => "check",
    _: "unknown"
}
print(status)  # check
```

### Pattern forms

| Pattern | Matches |
|---------|---------|
| `"literal"` | Exact string equality |
| `42` | Exact numeric equality |
| `_` | Wildcard — matches anything |
| `n if cond` | Guard: binds `n`, then checks `cond` |
| `[a, b]` | List of exactly two elements |
| `[a, ...rest]` | List with head `a` and tail `rest` |
| `#(x, y)` | Tuple of exactly two elements |
| `pat as whole` | Binds `whole` to the entire matched value |
| `a \| b` | Either `a` or `b` |

### Examples

Numeric patterns:

```zen
func label_of(n) {
    return match n {
        1: "one",
        2 => "two",
        _: "many"
    }
}
print(label_of(1))  # one
print(label_of(2))  # two
print(label_of(9))  # many
```

Guard patterns:

```zen
bound = match 42 {
    n if n > 100: "big",
    n if n == 42: "forty-two",
    _: "other"
}
print(bound)  # forty-two
```

Guard false falls through:

```zen
bound = match "err" {
    "err" if false: "guarded",
    "err": "plain",
    _: "other"
}
print(bound)  # plain
```

List destructuring:

```zen
func head_tail(xs) {
    return match xs {
        []: "empty",
        [only]: "one:" + str(only),
        [a, b]: "pair:" + str(a) + str(b),
        [a, ...rest]: "rest:" + str(a) + ":" + str(len(rest))
    }
}
print(head_tail([]))        # empty
print(head_tail([5]))       # one:5
print(head_tail([1, 2]))    # pair:12
print(head_tail([1, 2, 3])) # rest:1:2
```

Tuple destructuring:

```zen
func shape_of(x) {
    return match x {
        #(0, 0): "origin",
        #(x, y) if x == y: "on-diagonal",
        #(_, _): "some-pair"
    }
}
print(shape_of(#(0, 0)))    # origin
print(shape_of(#(4, 4)))    # on-diagonal
print(shape_of(#(4, 9)))    # some-pair
print(shape_of([1, 2]))     # some-pair (list matches tuple pattern)
print(shape_of(7))          # nil (non-iterable declines)
```

`as` pattern (bind whole):

```zen
func bind_whole(x) {
    return match x {
        [a, b] as whole: str(a) + str(b) + "/" + str(len(whole)),
        n as single: "single:" + str(single),
        _: "none"
    }
}
print(bind_whole([1, 2]))        # 12/2
print(bind_whole(7))             # single:7
print(bind_whole([1, 2, 3]))     # single:<list>
```

OR pattern (`a \| b`):

```zen
func classify(n) {
    return match n {
        1 \| 2: "small-prime",
        3 \| 5 \| 7: "other-prime",
        n if n % 2 == 0: "even",
        _: "odd"
    }
}
print(classify(1))  # small-prime
print(classify(4))  # even
print(classify(9))  # odd
```

Block bodies:

```zen
let printed = ""
let got = match 3 {
    1: { printed = "one" },
    2: { printed = "two" },
    3: { printed = "three" }
}
print(printed)  # three
print(got)      # nil (block bodies return nil unless they `return`)
```

Dict-literal bodies:

```zen
let usr = "bob"
let info = match usr {
    "bob": { name: "Bob", id: 7 },
    _: { name: "stranger", id: 0 }
}
print(info["name"])  # Bob
print(info["id"])    # 7
```

## 4.4 `when`: conditional expressions

`when` evaluates a series of condition arms and returns the value of the
first true one. It's like `if`/`elif` but as an expression.

```zen
let score = 87
let grade = when {
    score >= 90: "A",
    score >= 85 => "A-",
    score >= 80: "B+",
    _: "C"
}
print(grade)  # A-
```

Wildcard fallback:

```zen
grade = when {
    score > 1000: "impossible",
    _: "fallback"
}
print(grade)  # fallback
```

Expression position:

```zen
let sz = when {
    score >= 85: "large",
    _: "small"
}
print(sz)  # large
```

## 4.5 Error handling: `try` / `catch` / `throw` / `finally`

Zen is "safe by default": runtime problems become catchable errors instead of
crashes.

### Bare `catch`

```zen
var caught = false
try {
    var bad = 1 / 0
    caught = bad == 0
} catch {
    caught = true
}
print(caught)  # true
```

A body that completes normally skips the catch:

```zen
var ok = true
try {
    var five = 10 / 2
    ok = five == 5
} catch {
    ok = false
}
print(ok)  # true
```

### `catch as e` — bind the error

```zen
var em = ""
try {
    let boom = 1 / 0
} catch as e {
    em = str(e)
}
print(em)  # DivideByZero: division by zero
```

The caught error exposes `msg`, `code`, and `val`:

```zen
try {
    let y = 1 / 0
} catch as e {
    print(str(e))   # DivideByZero: division by zero
    print(e.msg)    # DivideByZero: division by zero
    print(e.code)   # 0
    print(e.val)    # nil
}
```

String + error concatenation works:

```zen
try {
    let y = 1 / 0
} catch as e {
    print("caught: " + e)  # caught: DivideByZero: division by zero
}
```

### `throw`

`throw` raises your own errors. Strings become `UserError` messages:

```zen
var msg = ""
try {
    throw "custom failure"
} catch {
    msg = "handled"
}
print(msg)  # handled

var tm = ""
try {
    throw "custom boom"
} catch as e {
    tm = e.msg
}
print(tm)  # UserError: custom boom
```

`throw` accepts any value:

```zen
throw { code: 42, detail: "retry later" }
```

### Typed catch clauses

You can add typed clauses to handle only certain errors. Unmatched clauses
fall through:

```zen
try {
    risky()
} catch TypeError {
    print("bad type")
} catch UserError {
    print("user raised")
} catch {
    print("everything else")
}
```

A clause whose type does not match lets the error keep propagating outward.

### `finally`

`finally` runs whether the `try` succeeded or failed, and whether or not
`catch` handled the error. It must come **after** any `catch` clauses.

```zen
var log = ""
try {
    throw "boom"
} catch {
    log = log + "c"
} finally {
    log = log + "f"
}
print(log)  # cf

# finally also runs when nothing failed
var log2 = ""
try {
    log2 = "b"
} finally {
    log2 = log2 + "f"
}
print(log2)  # bf
```

### Nested handlers

Inner handlers run before outer ones:

```zen
var order = ""
try {
    try { throw "inner" } catch { order = order + "i" }
    throw "outer"
} catch { order = order + "o" }
print(order)  # io
```

### Errors from callees

Errors raised inside a function are catchable by the caller:

```zen
func deep() {
    return [1][5]
}
var hit9 = false
try {
    deep()
} catch IndexError {
    hit9 = true
}
print(hit9)  # true
```

### Error types

Zen's built-in error types:

- `TypeError` — wrong type for an operation
- `IndexError` — sequence index out of range
- `KeyError` — missing dict key
- `DivideByZero` — division by zero
- `ValueError` — inappropriate value
- `NativeError` — raised by a native module
- `ForeignAttribute` — failed foreign attribute access
- `PyImportError` — Python import failure
- `UserError` — raised by `throw`

## 4.6 Common pitfalls

- **`e` only exists inside its `catch` block.** Don't reference it outside.
- **`finally` goes after `catch`, not between `try` and `catch`.**
- **A bare `catch` swallows everything.** Use typed clauses when you want
  unexpected errors to propagate.
- **`str(e)` is the readable message.** Never rely on `typeof(e)` (it's just
  `"error"`) — use `e.msg` instead.

## 4.7 Quick reference

```zen
try {
    # risky code
} catch as e {
    print(str(e))   # real message
    print(e.msg)    # same as str(e)
    print(e.code)   # numeric code
    print(e.val)    # attached value
} catch TypeError {
    # only type errors
} finally {
    # always runs
}

throw "something went wrong"   # raise your own error

# match
let result = match value {
    pattern: expression,
    _: fallback
}

# when
let grade = when {
    cond1: "A",
    cond2: "B",
    _: "C"
}
```

## 4.8 Exercises

1. Write a `safe_divide(a, b)` function that returns `nil` on division by
   zero instead of crashing.
2. Use `match` to implement a simple router: map URL paths like `"/home"`,
   `"/about"`, `"/contact"` to page titles, with a 404 fallback.
3. Use `when` to implement letter-grade conversion from a numeric score.
4. Use `for` with `continue` to sum only the odd numbers in a list.
5. Write a function that uses `try`/`catch`/`finally` to guarantee a
   cleanup message prints regardless of success or failure.

## 4.12 `if` / `elif` / `else` — complete semantics

The condition is evaluated with the truthiness rule from Chapter 3: only
`false`, `nil`, and `null` are falsy. `elif` is evaluated only if all previous
conditions were falsy. `else` runs when no branch matched.

```zen
func classify(n) {
    if n < 0 {
        return "negative"
    } elif n == 0 {
        return "zero"
    } elif n < 10 {
        return "small"
    } else {
        return "large"
    }
}
```

Braces are mandatory; there is no significant indentation. Single-statement
bodies may be written on one line with `;`.

```zen
if x > 0 { print("positive") }
if x > 0 { print("positive"); print("indeed") }
```

The ternary form is an expression, not a statement:

```zen
let label = x > 0 ? "pos" : "non-pos"
let label2 = "pos" if x > 0 else "non-pos"   # Python-style, also valid
```

`if` itself can also be used as an expression in many positions:

```zen
let msg = if x > 0 { "positive" } else { "non-positive" }
```

## 4.13 `while` loops

`while cond { ... }` re-evaluates `cond` before every iteration. `break`
leaves the innermost loop; `continue` jumps to the next condition check.

```zen
var i = 0
while i < 5 {
    i += 1
    if i == 3 { continue }
    if i == 4 { break }
    print(i)     # 1, 2
}
```

### Guard against infinite loops

Because `while true { ... }` is common for servers and REPLs, always ensure a
reachable `break`, `return`, `throw`, or external stop condition.

```zen
var attempts = 0
while true {
    attempts += 1
    if attempts > 3 { break }
}
```

### Loop with a post-condition

Zen has no `do/while`. Emulate it:

```zen
var n = 0
while true {
    n += 1
    if n >= 3 { break }
}
```

## 4.14 `for` loops

`for x in iterable { ... }` iterates over lists, tuples, sets, dict keys,
strings (characters), ranges, and generators. The loop variable is a fresh
binding per iteration for scalars.

```zen
for x in [10, 20, 30] { print(x) }
for ch in "abc" { print(ch) }
for k in #{a: 1, b: 2} { print(k) }
for i in 0..3 { print(i) }
```

### Multi-variable iteration

Dict `.items()` yields `#(key, value)` tuples, but the loop can destructure
them directly:

```zen
for k, v in #{a: 1, b: 2}.items() {
    print(k, v)
}
```

The same works for lists of pairs:

```zen
for a, b in [#(1, "x"), #(2, "y")] {
    print(a, b)
}
```

### `enumerate`

`enumerate(iter)` returns a list of `[index, value]` pairs.

```zen
for i, v in enumerate(["a", "b"]) {
    print(i, v)   # 0 a / 1 b
}
```

### Nested loops and labels

Zen has no loop labels. Use a flag or a helper function to break out of outer
loops.

```zen
func find_pair(rows, target) {
    for i, row in enumerate(rows) {
        for j, v in enumerate(row) {
            if v == target { return #(i, j) }
        }
    }
    return nil
}
```

### Mutating a collection while iterating

Do not add/remove elements of a list or dict while iterating it. Iterate over
a copy instead:

```zen
for x in list(items) {        # copy
    if x < 0 { items.remove(x) }
}
```

For dicts, collect keys first:

```zen
for k in list(d.keys()) {
    if d[k] == nil { d.remove(k) }
}
```

## 4.15 `break`, `continue`, and `return` inside loops

- `break` exits the nearest enclosing loop.
- `continue` skips to the next iteration of the nearest loop.
- `return` exits the entire function immediately, even from nested loops.

```zen
func first_negative(xs) {
    for x in xs {
        if x < 0 { return x }
    }
    return nil
}
```

## 4.16 `match` expressions

`match` compares a value against patterns. Arms are separated by `=>`, `->`,
or `:`; each arm yields an expression or a block.

```zen
func http_text(code) {
    return match code {
        200 => "OK",
        201 => "Created",
        301 | 302 | 307 | 308 => "Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}
```

### Pattern kinds

| Pattern | Example | Matches |
|---------|---------|---------|
| wildcard | `_` | anything |
| binding | `n` | anything, binds to `n` |
| literal int | `42` | exactly 42 |
| literal string | `"ok"` | exactly `"ok"` |
| literal bool/nil | `true`, `nil` | exactly that |
| list | `[a, b]` | two-element list, binds a,b |
| list with rest | `[head, ...tail]` | non-empty list |
| tuple | `#(x, y)` | two-tuple |
| dict | `#{type: t}` | dict containing key `type` |
| or-pattern | `1 | 2 | 3` | any of them |
| guard | `n if n > 0` | pattern plus condition |

### List destructuring

```zen
func describe(xs) {
    return match xs {
        [] => "empty",
        [x] => "one: ${x}",
        [x, y] => "two: ${x}, ${y}",
        [head, ...rest] => "many starting with ${head}",
    }
}
```

### Dict patterns

```zen
func handle_event(ev) {
    return match ev {
        #{type: "click", x, y} => "click at ${x},${y}",
        #{type: "key", key} => "key ${key}",
        #{type: t} => "unknown event ${t}",
        _ => "malformed",
    }
}
```

### Guards

```zen
func sign(n) {
    return match n {
        x if x < 0 => -1,
        0 => 0,
        _ => 1,
    }
}
```

### Exhaustiveness

The runtime does not require exhaustiveness, but an unmatched value raises an
error. Always include `_` unless you are certain the domain is closed.

## 4.17 `when` expressions

`when` is a cond-style chain: each arm has a boolean condition rather than a
pattern.

```zen
let grade = when {
    score >= 90 => "A",
    score >= 80 => "B",
    score >= 70 => "C",
    score >= 60 => "D",
    _ => "F",
}
```

`when` is often clearer than a long `if/elif` chain when you want a value.

## 4.18 `try` / `catch` / `finally` as control flow

`try` is covered fully in Chapter 11, but control-flow interactions matter:
`return` inside `try` still runs `finally`; `break`/`continue` inside `try`
also run `finally`.

```zen
func f() {
    try {
        return 1
    } finally {
        print("cleanup")   # runs before returning
    }
}
```

## 4.19 Early-return style

Deeply nested `if`s are hard to read. Prefer guard clauses with early return.

```zen
# hard to read
func process(user) {
    if user != nil {
        if user.active {
            if user.email != nil {
                send(user.email)
            }
        }
    }
}

# easier
func process2(user) {
    if user == nil { return }
    if !user.active { return }
    if user.email == nil { return }
    send(user.email)
}
```

## 4.20 State machines with `match`

`match` on a state string is a clean way to write state machines.

```zen
class TrafficLight {
    var state = "red"

    func tick() {
        self.state = match self.state {
            "red" => "green",
            "green" => "yellow",
            "yellow" => "red",
            _ => "red",
        }
    }
}
```

For larger machines, define transition tables declaratively:

```zen
let transitions = #{
    red: #{green: "green"},
    green: #{yellow: "yellow"},
    yellow: #{red: "red"},
}

func next_state(state, event) {
    let row = transitions[state] ?? #{}
    return row[event] ?? state
}
```

## 4.21 Retry loops

A common pattern: retry an operation with backoff and a maximum attempt count.

```zen
func retry(fn, attempts, delay_ms) {
    var last_err = nil
    for i in 0..attempts {
        try {
            return fn()
        } catch as e {
            last_err = e
            sleep(delay_ms * (i + 1))
        }
    }
    throw last_err
}
```

## 4.22 Infinite event loops and shutdown

Servers and UIs run an event loop until a stop flag is set.

```zen
var running = true

func shutdown() { running = false }

while running {
    let event = poll_event()
    if event == nil { continue }
    handle(event)
}
```

For multi-threaded programs, use an atomic/`shared` flag or a channel close
signal instead of a plain bool.

## 4.23 Loop performance notes

- Prefer `for x in list` over index loops when you do not need the index.
- Cache `len(xs)` in a local if the loop body cannot change it.
- Avoid building strings with `+=` in a hot loop; collect into a list and
  `join`.
- `break` early when searching.
- Avoid `try` inside tight loops when an error is expected frequently; test a
  condition first.

```zen
# slow
var s = ""
for w in words { s += w + " " }

# fast
let parts = []
for w in words { parts.push(w) }
let s2 = parts.join(" ")
```

## 4.24 Common control-flow mistakes

1. Using `=` instead of `==` in a condition (assignment is not an expression).
2. Forgetting braces.
3. Modifying a list while iterating it.
4. Assuming `0` is falsy.
5. Missing `_` in `match`, causing runtime errors on unhandled values.
6. Using `continue` inside a `finally` (avoid; confusing semantics).
7. Infinite loops without a reachable exit.
8. Off-by-one with `..` (exclusive) vs `->` (inclusive).
9. Deep recursion instead of iteration for large inputs.
10. Relying on dict iteration order across versions (use `.items()` if order matters).

## 4.25 Exercises

1. Write `fizzbuzz(n)` using `when` or `match`.
2. Implement a guessing game loop that exits on correct input or `quit`.
3. Write `sum_evens(xs)` using a `for` loop and `continue`.
4. Implement `binary_search(sorted, target)` with a `while` loop.
5. Rewrite a nested `if` pyramid using guard clauses.
6. Implement a simple calculator state machine with `match`.
7. Write `count_words(text)` using a dict and a loop.
8. Implement `flatten(nested)` with recursion and a loop.
9. Build a retry helper with exponential backoff.
10. Write `first_duplicate(xs)` returning the first repeated element or nil.
11. Implement `collatz_steps(n)` and guard against non-positive input.
12. Print a multiplication table using nested loops with aligned columns.
