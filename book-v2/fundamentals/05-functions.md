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
