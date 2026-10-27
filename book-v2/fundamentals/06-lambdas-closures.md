# Chapter 6: Lambdas and Closures

This chapter is a deep dive into anonymous functions and closure semantics: how captures work, what is shared vs copied, the `lambda` expression vs `fn` statement, and how closures interact with the JIT and reference counting.

## 6.1 Lambda expressions

A lambda is an anonymous function defined as an expression:

```zen
let double = lambda(x) { return x * 2 }
print(double(21))   # 42
```

Syntax: `lambda(params) { body }`. The body is a block; use `return` to produce a value (omitting `return` yields `nil`).

### Lambda as expression vs fn as statement

| | `lambda` | `fn` |
|---|----------|------|
| Kind | Expression | Statement |
| Can appear inline | Yes (`(lambda(x){...})(1)`) | No |
| Assignable | Yes | Yes (`let f = fn() {...}`) |
| First-class | Yes | Yes |
| Closure capture | Yes | Yes |
| `return` target | The lambda itself | The enclosing function |

Because `lambda` is an expression, you can call it immediately:

```zen
let result = (lambda(x, y) { return x + y })(3, 4)
print(result)   # 7
```

And pass it directly:

```zen
print((lambda(x) { return x * 2 })(5))   # 10
```

## 6.2 Closure capture semantics

Zen functions (both `fn` and `lambda`) capture variables from their enclosing scope by **shared reference** (a boxed `cell`). Mutating a captured variable is visible to every closure that captured it, and persists across calls.

### Basic capture

```zen
func make_counter(start) {
    var count = start
    func inc() {
        count = count + 1
        return count
    }
    return inc
}

let c1 = make_counter(0)
let c2 = make_counter(100)

print(c1())   # 1
print(c1())   # 2
print(c2())   # 101
print(c1())   # 3   (c1 and c2 are independent)
```

Each call to `make_counter` allocates a fresh `count` cell. `c1` and `c2` point to different cells, so they count independently.

### Multiple closures share the same cell

```zen
func make_pair() {
    var value = 0
    func get() { return value }
    func set(v) { value = v }
    return #(get, set)
}

let #(get, set) = make_pair()
print(get())     # 0
set(42)
print(get())     # 42
```

`get` and `set` share the same `value` cell — mutation through one is visible through the other.

## 6.3 The loop-variable capture trap

This is the most common closure bug. A loop variable is a single cell; every closure created in the loop captures **that same cell**, so they all see the final value:

```zen
let funcs = []
for i in 1..3 {
    funcs.push(fn() { return i })
}
print(funcs[0]())   # 3   ← not 1!
print(funcs[1]())   # 3
print(funcs[2]())   # 3
```

All three closures return `3` because `i` ended at `3` (the loop exits when `i == 3`).

### Fix: bind per-iteration with a helper function

```zen
func make_fn(val) {
    return fn() { return val }
}

let funcs = []
for i in 1..3 {
    funcs.push(make_fn(i))   # i is copied into `val` at each call
}
print(funcs[0]())   # 1
print(funcs[1]())   # 2
print(funcs[2]())   # 3
```

`make_fn(i)` copies the current `i` into a fresh `val` parameter cell, so each closure captures its own `val`.

## 6.4 `return` target in closures

`return` inside a lambda returns from the lambda, **not** from the enclosing function:

```zen
func outer() {
    let f = lambda(x) {
        if x < 0 { return -1 }   # returns from lambda
        return x * 2
    }
    return f
}

let f = outer()
print(f(-5))   # -1
print(f(5))    # 10
print(outer()(-3))  # -1
```

If you need the enclosing function to return from inside a closure, use an error or a sentinel value instead.

## 6.5 Closures and reference counting

A closure retains every variable it captures. As long as the closure is alive, its captured variables stay alive:

```zen
func make_big() {
    let big = [0 for _ in 1..1000000]   # large allocation
    return fn() { return len(big) }     # keeps `big` alive
}

let f = make_big()
print(f())   # 1000000   (big still in memory)
```

When `f` is dropped (goes out of scope or reassigned), the captured cell is released, and `big` can be collected.

```zen
f = nil
# now `big` can be freed
```

## 6.6 Closures in collections

Functions are first-class, so they can live in lists and dicts:

```zen
let ops = {
    "add": lambda(a, b) { return a + b },
    "sub": lambda(a, b) { return a - b },
}

print(ops["add"](5, 3))   # 8
print(ops["sub"](5, 3))   # 2
```

## 6.7 Closures and threads

Closures can be passed to `thread.spawn`, but captured shared variables become concurrently accessible — use `shared` for atomic counters:

```zen
import shared
import thread

let counter = shared.create(0)

for i in 1..3 {
    thread.spawn(fn() {
        shared.increment(counter, 1)
    })
}
# ... join threads ...
print(shared.get(counter))   # 3
```

Without `shared`, each thread would capture its own copy and updates would be lost.

## 6.8 JIT lowering note

Zen's Cranelift JIT lowers hot closures to native code. Closures that are called repeatedly in a loop get tiered up; the capture cells are accessed through a stable context pointer. You generally don't need to think about this, but it means:

- A closure called once stays interpreted.
- A closure called millions of times becomes native code with equivalent capture semantics.
- `ZEN_NO_JIT=1` disables tiering (all interpreted, same semantics).

## 6.9 Quick reference

```zen
# Lambda expression
let f = lambda(x) { return x * 2 }
print(f(5))           # 10

# Inline call
print((lambda(x) { return x + 1 })(41))   # 42

# Closure with shared cell
func make_counter(start) {
    var count = start
    return fn() { count = count + 1; return count }
}

# Fix loop capture
let funcs = []
for i in 1..3 {
    funcs.push((lambda(v) { return fn() { return v } })(i))
}
print(funcs[0]())   # 1

# return from lambda, not enclosing func
let f = fn(x) { if x < 0 { return -1 } return x * 2 }
```

## 6.10 Exercises

1. Write `make_adder(n)` returning a lambda that adds `n` to its argument.
2. Write `make_accumulator()` returning `fn(x)` that adds `x` to a running total and returns the total.
3. Write `repeater(n)` returning `fn()` that returns `n` on every call, but after `k` calls returns `nil` (capture a counter).
4. Write `compose(f, g)` returning `fn(x) { return f(g(x)) }`.
5. Demonstrate the loop-capture bug and fix it with `make_fn`.
6. Write `twice(f)` returning `fn(x) { return f(f(x)) }`.
7. Write `curry(f)` for a two-argument function: `curry(f)(a)(b)` equals `f(a, b)`.


## 6.12 Lambda and anonymous-function syntax — complete reference

Zen offers several spellings for anonymous functions. All are values and can be stored,
passed, returned, and called immediately.

| Form | Example | Notes |
|------|---------|-------|
| `fn(params) { ... }` | `fn(x) { return x * 2 }` | block body, explicit return |
| `fn(params) => expr` | `fn(x) => x * 2` | expression body, implicit return |
| `(params) => expr` | `(x, y) => x + y` | parens required around params |
| `lambda params : expr` | `lambda x : x * 2` | legacy alias |

### Expression-bodied vs block-bodied

```zen
# block body: explicit return, multiple statements allowed
let a = fn(x) { let y = x + 1; return y * 2 }

# expression body: implicit return
let b = fn(x) => (x + 1) * 2
let c = (x) => x * x
```

### Arrow operator vs range operator

`->` in signature/range position is also the inclusive range operator; in the
context `(x) => ...` it is parsed as the arrow, not a range. When you write
ranges, `0 -> 5` means inclusive values 0..5; `0 .. 5` means exclusive 0..4.

## 6.13 First-class functions as values

Functions can be stored in variables, lists, dicts, returned from other
functions, and passed as arguments.

```zen
let ops = #{
    add: fn(a, b) => a + b,
    sub: fn(a, b) => a - b,
    mul: fn(a, b) => a * b,
}
let pick = ops["add"]
print(pick(2, 3))   # 5

func apply(op, a, b) { return op(a, b) }
print(apply(ops["mul"], 4, 5))   # 20
```

A function value can itself return a function:

```zen
func multiplier(n) {
    return fn(x) => n * x
}

let double = multiplier(2)
let triple = multiplier(3)
print(double(7))   # 14
print(triple(7))   # 21
```


## 6.14 Closures and capture semantics

A closure "closes over" variables from its defining scope. The inner function
remembers those bindings even after the outer function returns.

```zen
func make_counter(start = 0) {
    var n = start
    return fn() {
        n += 1
        return n
    }
}

let c = make_counter()
print(c())   # 1
print(c())   # 2
print(c())   # 3

let d = make_counter(10)
print(d())   # 11
print(d())   # 12
```

Each call to `make_counter` creates a fresh binding, so each counter is independent.

### The late-binding loop variable trap

```zen
let fns = []
for i in 0..3 {
    fns.push(fn() => i)
}
for f in fns { print(f()) }
```

If all closures print the same final value, the loop variable is shared across
iterations. Fix by capturing a copy through a factory function:

```zen
func make_constant(v) { return fn() => v }

let fns2 = []
for i in 0..3 { fns2.push(make_constant(i)) }
for f in fns2 { print(f()) }   # 0 1 2
```

## 6.15 Partial application and currying

```zen
func partial(f, a) { return fn(b) => f(a, b) }
func curry(f) { return fn(a) => fn(b) => f(a, b) }

let add = fn(a, b) => a + b
let add5 = partial(add, 5)
print(add5(10))   # 15

let curried = curry(add)
print(curried(3)(4))   # 7
```

## 6.16 Function composition

```zen
func compose(f, g) { return fn(x) => f(g(x)) }

let double = fn(x) => x * 2
let inc = fn(x) => x + 1
let inc_then_double = compose(double, inc)
print(inc_then_double(3))   # 8  (double(inc(3)) = double(4) = 8)

func compose_all(fns) {
    return fn(x) {
        var acc = x
        for i in (len(fns) - 1)..-1 {
            acc = fns[i](acc)
        }
        return acc
    }
}
```

## 6.17 Memoization

```zen
func memoize(f) {
    let cache = #{}
    return fn(n) {
        let key = str(n)
        if key in cache { return cache[key] }
        let v = f(n)
        cache[key] = v
        return v
    }
}

let fib = memoize(fn(n) {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
})

print(fib(50))   # 12586269025
```

The self-reference works because `fib` is bound before any call executes.

## 6.18 Callbacks and sorting with comparator functions

```zen
func sort_by(xs, key_fn) {
    # simple insertion sort using the key function
    let out = list(xs)
    for i in 1..len(out) {
        let key = out[i]
        var j = i - 1
        while j >= 0 and key_fn(out[j]) > key_fn(key) {
            out[j + 1] = out[j]
            j -= 1
        }
        out[j + 1] = key
    }
    return out
}

let words = ["banana", "apple", "cherry", "kiwi"]
print(sort_by(words, fn(w) => len(w)))   # shorter first
```

Passing comparator/key functions is the standard way to customize behavior.

## 6.19 Closures over mutable state

Captured `var` bindings can be mutated from inside the closure, creating shared
state between the closure and its enclosing scope.

```zen
func adder_factory(init) {
    var total = init
    return fn(x) {
        total += x
        return total
    }
}

let acc = adder_factory(0)
print(acc(5))   # 5
print(acc(7))   # 12
print(acc(3))   # 15
```

## 6.20 Common closure pitfalls

1. **Late binding**: loop variables are shared; use a factory to capture values.
2. **Stale closures**: a closure captures the binding, not a snapshot, so later
   mutations are visible inside the closure.
3. **Mutating captured variables**: surprising when multiple closures share one
   variable — prefer explicit state passed as arguments.
4. **Returning closures that reference `nil`-initialized variables**: initialize
   before returning.
5. **Deeply nested closures**: hard to read; name intermediate functions.

## 6.21 Exercises

1. Write `make_multiplier(n)` returning a function that multiplies by `n`.
2. Implement `compose(f, g)` and verify `compose(double, inc)(3) == 8`.
3. Write `repeater(f, n)` returning a function that calls `f` `n` times.
4. Build `memoize` and apply it to Fibonacci; time the difference.
5. Implement `curry2(f)` for two-argument functions.
6. Write `pipe(value, fns)` that threads a value through functions left-to-right.
7. Sort a list of dicts by a chosen key using a comparator closure.
8. Write `once(f)` that invokes `f` only on the first call.
9. Create `counter()` closures that can increment, decrement, and reset.
10. Demonstrate the late-binding trap and two ways to fix it.

