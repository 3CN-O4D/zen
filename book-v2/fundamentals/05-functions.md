# Chapter 5: Functions

Functions are first-class values in Zen. They can be declared with `fn`, assigned to variables, passed as arguments, returned from other functions, and stored in collections. This chapter covers declaration, calling, closures, default arguments, variadic behavior, and the `return` statement.

## 5.1 Declaration and calling

```zen
func add(a, b) {
    return a + b
}

print(add(2, 3))   # 5
```

- `func name(params) { body }` declares a function.
- The body is a block of statements; the last expression is **not** automatically returned — use `return`.
- Functions are values: `let f = add` then `f(2, 3)` works.

### Calling with wrong argument count

```zen
try {
    add(1)            # missing argument
} catch as e {
    print(e.msg)      # ArgumentCount: expected 2, got 1
}

try {
    add(1, 2, 3)      # too many
} catch as e {
    print(e.msg)      # ArgumentCount: expected 2, got 3
}
```

Zen does **not** have keyword arguments. All calls are positional.

## 5.2 Return statements

`return` exits the function immediately with a value:

```zen
func abs(x) {
    if x < 0 {
        return -x
    }
    return x
}

print(abs(-5))  # 5
print(abs(3))   # 3
```

A function without `return` returns `nil`:

```zen
func nop() {
    print("side effect only")
}
print(nop())    # nil
```

`return` inside nested blocks exits the **enclosing function**, not just the block:

```zen
func find(items, target) {
    for i in 0..len(items) {
        if items[i] == target {
            return i          # exits find(), not just the if
        }
    }
    return -1
}
```

## 5.3 Default arguments

Zen supports default values for trailing parameters:

```zen
func greet(name, greeting = "Hello") {
    return greeting + ", " + name + "!"
}

print(greet("Zen"))            # "Hello, Zen!"
print(greet("Zen", "Hi"))      # "Hi, Zen!"
```

Rules:
- Defaults must be on trailing parameters (no required parameter after a defaulted one).
- Default expressions are evaluated at call time, not definition time:
  ```zen
  let counter = 0
  func next(n = counter + 1) {
      counter = counter + 1
      return n
  }
  print(next())   # 1
  print(next())   # 2   (counter incremented between calls)
  ```

## 5.4 Variadic behavior

Zen does **not** have `*args` or `**kwargs` syntax. All parameters are fixed-position. However, you can pass a list and unpack it manually:

```zen
func sum_all(nums) {
    let total = 0
    for n in nums {
        total = total + n
    }
    return total
}

let vals = [1, 2, 3, 4]
print(sum_all(vals))   # 10
```

If you need to forward arguments to another function, pass them explicitly or as a list.

## 5.5 Functions as first-class values

```zen
let f = add
print(f(10, 20))   # 30

let ops = [add, sub, mul]
print(ops[0](5, 3))  # 8
```

You can store functions in lists, dicts, and pass them to other functions:

```zen
func apply(f, x, y) {
    return f(x, y)
}

print(apply(add, 3, 4))   # 7
print(apply(sub, 3, 4))   # -1
```

## 5.6 Closures

Functions capture variables from their enclosing scope. Captured variables are shared (boxed) cells, so mutations are visible across calls:

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
print(c1())   # 3   (c1 and c2 have independent count cells)
```

Each call to `make_counter` creates a **new** `count` cell. `c1` and `c2` capture different cells.

Closure over loop variables — be careful:

```zen
let funcs = []
for i in 1..3 {
    funcs.push(fn() { return i })
}
print(funcs[0]())   # 3 (all capture the same final i)
print(funcs[1]())   # 3
print(funcs[2]())   # 3
```

To capture per-iteration values, use a helper:

```zen
func make_fn(val) {
    return fn() { return val }
}

let funcs = []
for i in 1..3 {
    funcs.push(make_fn(i))
}
print(funcs[0]())   # 1
print(funcs[1]())   # 2
print(funcs[2]())   # 3
```

## 5.7 Higher-order functions

```zen
func twice(f, x) {
    return f(f(x))
}

func add_one(x) {
    return x + 1
}

print(twice(add_one, 5))   # 7

func map_list(f, items) {
    let result = []
    for x in items {
        result.push(f(x))
    }
    return result
}

print(map_list(add_one, [1, 2, 3]))   # [2, 3, 4]
```

## 5.8 Anonymous functions (lambdas)

Zen supports `lambda(params) { body }`:

```zen
let double = lambda(x) { return x * 2 }
print(double(21))   # 42
```

Lambdas are functions too — first-class, closures, passable. The difference from `fn` is syntactic: `lambda` is an expression, `fn` is a statement.

```zen
# lambda as expression (inline)
let result = (lambda(x, y) { return x + y })(3, 4)
print(result)   # 7
```

## 5.9 `return` in closures

`return` inside a lambda returns from the lambda, not the enclosing function:

```zen
func outer() {
    let f = lambda(x) {
        if x < 0 { return -1 }   # returns from the lambda
        return x * 2
    }
    return f
}

let f = outer()
print(f(-5))   # -1
print(f(5))    # 10
```

## 5.10 Common pitfalls

- **No keyword arguments.** All calls are positional. `add(a=1, b=2)` is a parse error.
- **Default args are evaluated at call time.** Mutable default expressions can surprise.
- **`return` exits the enclosing function**, not just the current block.
- **Closures capture by reference (shared cell)**, not by value. Mutating a captured var affects all closures over it.
- **Loop variable capture**: all closures in a loop share the loop var unless you use a helper function to bind per-iteration values.
- **Functions without `return` return `nil`.**

## 5.11 Quick reference

```zen
func name(a, b = default) { ... return value }
let f = name
f(1, 2)

lambda(x) { return x * 2 }
lambda(x, y) { return x + y }(3, 4)   # inline call

func make_counter(start) {
    var count = start
    return fn() { count = count + 1; return count }
}

func apply(f, x, y) { return f(x, y) }

return               # returns nil
return value         # returns value
```

## 5.12 Exercises

1. Write `compose(f, g)` that returns a function `h(x) = f(g(x))`.
2. Write `repeater(n)` that returns a function which, when called, returns the call count (1, 2, 3, ...).
3. Write `once(f)` that returns a wrapper calling `f` only on the first invocation and caching the result.
4. Write `curry2(f)` that takes a two-arg function and returns a function taking one arg that returns a function taking the second.
5. Write `memoize(f)` that caches results by argument (assume a single hashable arg).
6. Fix the loop-variable capture bug: create 3 functions returning 1, 2, 3 respectively using a helper.
7. Write `pipeline(funcs, x)` that applies each function in `funcs` to `x` in sequence.

## 5.11 Function declaration forms

Zen accepts several spellings for the same construct. They are aliases and
behave identically:

```zen
func add(a, b) { return a + b }
def add2(a, b) { return a + b }
function add3(a, b) { return a + b }
```

Anonymous functions use `fn`:

```zen
let add = fn(a, b) { return a + b }
let add2 = fn(a, b) => a + b
let add3 = (a, b) => a + b
let legacy = lambda a, b : a + b
```

`fn(params) => expr` and `(params) => expr` are expression-bodied and return
the expression without an explicit `return`. The parentheses around the
parameter list are required for the `(a, b) => ...` form; `fn a => ...` is not
valid.

## 5.12 Parameters: defaults, variadic, and destructuring

### Default values

Defaults are evaluated at call time, in the scope of the function, left to
right. A default may refer to an earlier parameter.

```zen
func greet(name, greeting = "Hello", punct = "!") {
    return "${greeting}, ${name}${punct}"
}

print(greet("Ada"))                 # Hello, Ada!
print(greet("Ada", "Hi"))           # Hi, Ada!
print(greet("Ada", "Hi", "."))      # Hi, Ada.

func box(w, h = w) { return w * h }
print(box(4))      # 16
print(box(4, 5))   # 20
```

Do not use a mutable container as a default when you expect a fresh one each
call; use `nil` and create inside:

```zen
func append_item(x, acc = nil) {
    if acc == nil { acc = [] }
    acc.push(x)
    return acc
}
```

### Variadic parameters

A trailing `...` parameter collects extra positional arguments into a list.
Check the VM/parser for the exact syntax supported in your build; the common
form is:

```zen
func sum_all(...nums) {
    var total = 0
    for n in nums { total += n }
    return total
}

print(sum_all(1, 2, 3, 4))   # 10
print(sum_all())             # 0
```

### Destructuring parameters

List and tuple patterns can be destructured directly in the parameter list in
newer builds:

```zen
func dist(#(x1, y1), #(x2, y2)) {
    let dx = x2 - x1
    let dy = y2 - y1
    return (dx*dx + dy*dy) ** 0.5
}
```

If your build does not support this, destructure inside the body:

```zen
func dist2(a, b) {
    let x1 = a[0]; let y1 = a[1]
    let x2 = b[0]; let y2 = b[1]
    let dx = x2 - x1
    let dy = y2 - y1
    return (dx*dx + dy*dy) ** 0.5
}
```

## 5.13 Return values

A function without an explicit `return` returns `nil`. `return` with no value
also returns `nil`.

```zen
func log(msg) { print(msg) }          # returns nil
func nothing() { return }             # returns nil
```

Multiple return values are expressed with a tuple:

```zen
func divmod(a, b) {
    return #(a // b, a % b)
}

let q, r = divmod(17, 5)
print(q, r)   # 3 2
```

If tuple destructuring in assignment is unavailable, index the tuple:

```zen
let qr = divmod(17, 5)
print(qr[0], qr[1])
```

## 5.14 First-class functions and higher-order functions

Functions are values. They can be stored in variables, passed as arguments,
returned, and placed in containers.

```zen
func apply(f, x) { return f(x) }
print(apply(fn(x) => x * 2, 21))   # 42

let ops = #{
    add: fn(a, b) => a + b,
    sub: fn(a, b) => a - b,
    mul: fn(a, b) => a * b,
}
print(ops["add"](2, 3))   # 5
```

### Building pipelines

```zen
func pipe(value, fns) {
    var acc = value
    for f in fns { acc = f(acc) }
    return acc
}

let result = pipe(10, [
    fn(x) => x + 1,
    fn(x) => x * 2,
    fn(x) => "result=${x}",
])
print(result)   # result=22
```

## 5.15 Recursion

```zen
func fact(n) {
    if n <= 1 { return 1 }
    return n * fact(n - 1)
}
print(fact(10))   # 3628800
```

### Tail recursion

The VM may or may not optimize tail calls. For large depths, rewrite as a loop.

```zen
func fact_iter(n) {
    var acc = 1
    var i = n
    while i > 1 {
        acc *= i
        i -= 1
    }
    return acc
}
```

### Mutual recursion

```zen
func is_even(n) {
    if n == 0 { return true }
    return is_odd(n - 1)
}
func is_odd(n) {
    if n == 0 { return false }
    return is_even(n - 1)
}
```

Forward references are fine because resolution happens at call time.

### Recursion depth

Deep recursion (tens of thousands of frames) can overflow the native stack.
Add an explicit depth parameter or convert to iteration.

```zen
func walk(node, depth = 0) {
    if depth > 1000 { throw "too deep" }
    for child in node.children {
        walk(child, depth + 1)
    }
}
```

## 5.16 Closures

A closure captures variables from its defining scope. In Zen, captures are
by reference to the binding (like Python nonlocal semantics); mutating a
captured `var` from the closure affects the outer binding when the language
permits it.

```zen
func make_counter() {
    var n = 0
    return fn() {
        n += 1
        return n
    }
}

let c = make_counter()
print(c())   # 1
print(c())   # 2
print(c())   # 3

let d = make_counter()
print(d())   # 1 — independent state
```

### The late-binding loop variable trap

```zen
let fns = []
for i in 0..3 {
    fns.push(fn() => i)
}
for f in fns { print(f()) }
```

If all closures print the same value, the loop variable is shared. Fix by
capturing a copy through a factory:

```zen
func make_constant(v) { return fn() => v }
let fns2 = []
for i in 0..3 { fns2.push(make_constant(i)) }
for f in fns2 { print(f()) }   # 0 1 2
```

## 5.17 `self` in methods

Inside a method, the receiver is bound to `self` (`this` is an accepted alias).
Access fields with `self.field`; call sibling methods with `self.method()`.

```zen
class Counter {
    var n = 0
    func inc() { self.n += 1 }
    func inc_by(k) { self.n += k }
    func value() { return self.n }
}
```

## 5.18 Function metadata and introspection

Functions expose a type of `"func"` via `typeof`/`type`. Some builds expose
`name`, `arity`, and parameter info; treat this as optional and avoid relying
on it for portability.

```zen
let f = fn(x) => x
print(typeof(f))   # "func"
```

## 5.19 Overloading and dispatch

Zen does not support traditional static overloading by parameter types. Two
functions with the same name replace one another in the same scope. Use
runtime type checks or distinct names.

```zen
func area(shape) {
    if typeof(shape) == "dict" {
        if shape["kind"] == "circle" { return 3.14159 * shape["r"] ** 2 }
        if shape["kind"] == "rect" { return shape["w"] * shape["h"] }
    }
    throw "unsupported shape"
}
```

## 5.20 Default arguments and `nil` sentinels

Because defaults are evaluated per call, `nil` is a safe sentinel. Distinguish
"not provided" from "explicitly nil" with a sentinel object if needed.

```zen
let MISSING = %{__missing__}

func get(d, key, default = MISSING) {
    if key in d { return d[key] }
    if default is MISSING { throw "missing key: ${key}" }
    return default
}
```

## 5.21 Pure functions and side effects

A pure function depends only on its arguments and returns a value without
observable side effects. Pure functions are easier to test, cache, and
parallelize.

```zen
# pure
func add(a, b) { return a + b }

# impure: reads global state and prints
var total = 0
func add_to_total(x) {
    total += x
    print("total=${total}")
}
```

Aim for a pure core and a thin impure shell around I/O.

## 5.22 Memoization

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

Note the self-reference: `fib` must be bound before the inner call runs, which
it is by the time `fib(50)` executes.

## 5.23 Function composition

```zen
func compose(f, g) { return fn(x) => f(g(x)) }
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

## 5.24 Currying and partial application

```zen
func curry(f) {
    return fn(a) => fn(b) => f(a, b)
}

let add = fn(a, b) => a + b
let add5 = curry(add)(5)
print(add5(10))   # 15
```

## 5.25 Error handling in functions

Decide on a contract: throw on invalid input, or return `nil`/a result type.
Be consistent.

```zen
# throwing contract
func sqrt_strict(x) {
    if x < 0 { throw "sqrt of negative" }
    return x ** 0.5
}

# result contract
func sqrt_safe(x) {
    if x < 0 { return nil }
    return x ** 0.5
}
```

## 5.26 Documentation comments

Document parameters, return values, and failure modes. A consistent style:

```zen
# Computes the arithmetic mean of a non-empty list.
# Throws ValueError when xs is empty.
func mean(xs) {
    if len(xs) == 0 { throw "mean of empty list" }
    return sum(xs) / len(xs)
}
```

## 5.27 Performance considerations

- Avoid allocating closures in hot loops when a plain function will do.
- Prefer iterative solutions for large n.
- Pass big structures by reference (they are reference-counted handles).
- Avoid repeated global lookups; bind to a local for hot paths.
- Keep default argument expressions cheap; they run on every call.
- Do not use exceptions for ordinary control flow in hot loops.

```zen
# bind frequently used functions locally in tight loops
let push = fn(lst, v) { lst.push(v) }
```

## 5.28 Common function mistakes

1. Forgetting `return`, resulting in `nil`.
2. Mutating a default list/dict argument (use `nil` sentinel).
3. Assuming closures capture by value.
4. Deep recursion causing a stack overflow.
5. Reassigning a `let` binding inside a function.
6. Calling a method without `self.` inside the class.
7. Confusing a function value with its result (missing `()`).
8. Relying on argument names at call sites (Zen uses positional args).
9. Shadowing a builtin unintentionally.
10. Defining two functions with the same name in one scope.

## 5.29 Exercises

1. Write `is_prime(n)` and test it on a range.
2. Implement `gcd` and `lcm` recursively and iteratively; compare.
3. Write `map_list(xs, f)` without using `.map`.
4. Write `filter_list(xs, pred)` without using `.filter`.
5. Implement `reduce_list(xs, f, init)`.
6. Build `compose(f, g)` and verify with numeric functions.
7. Write `memoize(f)` and apply it to a slow function.
8. Implement `partial(f, a)` returning a unary function.
9. Write `make_adder(n)` and create several adders.
10. Implement `compose_all([...])` and test order.
11. Write a recursive `flatten` and an iterative one.
12. Implement `safe_div` with a result tuple and with exceptions; compare.
13. Write `debounce(f, ms)` conceptually using timestamps.
14. Implement `once(f)` that only runs `f` the first time.
15. Write `retry(f, n)` and test with a flaky function.
