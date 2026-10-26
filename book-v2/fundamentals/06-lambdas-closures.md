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
