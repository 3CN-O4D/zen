# Functions

Functions are first-class values. They can be named, anonymous, lambda-form,
or arrow-form — and closures capture the environment where they were defined.

## Defining a named function

All four keywords are identical:

```zen
function add(a, b) { return a + b }
func add(a, b) { return a + b }
fn add(a, b) { return a + b }
def add(a, b) { return a + b }
```

Prefer `fn` for brevity. A function with no explicit `return` evaluates to
`null`.

```zen
fn add(a, b) {
    return a + b
}
print add(2, 3)        # 5

fn nothing() { }
print nothing()        # null
```

### Order matters: no hoisting

Functions are defined as the script runs, so **define before you call**:

```zen
print double(5)        # Error: undefined function: double

fn double(x) { return x * 2 }
```

## Calling

Parenthesized calls with arguments:

```zen
fn greet(name) { print "hi " + name }
greet("Ada")
```

Parameter-count checks are strict at runtime:

```zen
fn f(a, b) { }
f(1)                  # Error: f expects 2 arguments, got 1
f(1, 2, 3)            # Error: f expects 2 arguments, got 3
```

## Default arguments

Trailing parameters may have defaults:

```zen
fn greet(name = "world") { print "hello " + name }

greet()                # hello world
greet("Ada")           # hello Ada
```

Defaults are simple constants/expressions — they **cannot reference earlier
parameters**:

```zen
fn f(a = 1, b = a + 1) { }   # Error: undefined variable: `a`
```

## Anonymous functions

`function`/`func` with no name is an expression:

```zen
var double = function(x) { return x * 2 }
var triple = func(x) { return x * 3 }
print double(2), triple(2)      # 4 6
```

## Lambdas

Compact anonymous functions:

```zen
var square = lambda(x): x * x       # expression body
var cube   = lambda(x) { return x * x * x }   # block body

print square(3)     # 9
print cube(3)       # 27
```

Zero-parameter lambda:

```zen
var tick = lambda(): 42
print tick()        # 42
```

## Arrow functions

`(params) => expr` — parameters **must be parenthesized** (a bare `x => x`
is a parse error):

```zen
var inc  = (x) => x + 1
var add  = (a, b) => a + b
var big  = (x) => { var v = x * 2; return v > 10 }
```

```zen
var bad = x => x      # Error: expected expression, found `=>`
```

## Closures

Functions capture the surrounding scope — reads **and writes** work:

```zen
var counter = 0

fn increment() {
    counter = counter + 1
}

increment()
increment()
print counter        # 2
```

Arrow and lambda bodies capture too:

```zen
var step = 2
var shift = (x) => x + step
print shift(10)      # 12
```

## Functions as values

Functions can be passed around, stored, and invoked dynamically:

```zen
fn apply(f, value) {
    return f(value)
}

print apply((x) => x + 10, 5)     # 15
print apply(lambda(x): x * 2, 21) # 42
```

The higher-order globals `map`, `filter`, `reduce` and the list methods
`l.map(f)`, `l.filter(f)`, `l.reduce(f)`, `list.each(f)` all take functions
(see [lists](lists.md)).

## Named arguments: how they really work

`name = value` inside a call is **not** matched against parameter names.
Zen collects all named args into a single trailing **dict** argument:

```zen
fn connect(opts) {
    print "host=" + opts.host + " port=" + str(opts.port)
}

connect(host = "localhost", port = 8080)
# host=localhost port=8080
```

Write the function to accept that dict:

```zen
fn render(options) {
    var title = options.get("title", "(untitled)")
    var size  = options.get("size", 12)
    print title, size
}

render(title = "Docs", size = 14)
```

> Named args are sugar for "pass one dict", not Python-style keyword binding.

## Recursion

Plain recursion works:

```zen
fn factorial(n) {
    return n <= 1 ? 1 : n * factorial(n - 1)
}
print factorial(5)       # 120
```

## Nested functions

Define functions inside functions; they close over inner scope:

```zen
fn outer() {
    var secret = 99
    fn inner() {
        return secret + 1
    }
    return inner()
}
print outer()            # 100
```

## Multiple values

A function returns exactly one value. There is **no** `return a, b` — return a
list or dict instead:

```zen
fn minmax(nums) {
    return [min(nums), max(nums)]
}
var [lo, hi] = minmax([4, 1, 9])
print lo, hi             # 1 9
```

## Full worked example

A tiny curried-style accumulator using closures and defaults:

```zen
fn make_counter(start = 0) {
    var n = start
    fn inner(step = 1) {
        n = n + step
        return n
    }
    return inner
}

var c = make_counter(10)
print c()                # 11
print c(5)               # 16
print c()                # 17
```

## Common pitfalls

| Mistake | Result | Fix |
|---------|--------|-----|
| Calling before the `fn` line | `undefined function` (no hoisting) | move the definition above the call |
| `x => x` arrow without parens | parse error | `(x) => x` |
| `return a, b` | parse error | return `[a, b]` and destructure |
| Wrong argument count | runtime count error | match the signature |
| `f(a = 1)` expecting Python keyword binding | runtime count error or wrong param | accept a dict param |
| Default referencing an earlier param | `undefined variable` | compute inside the body |
| `def` keyword confused with "define constant" | it's a function keyword | use `var`/`const` for data |