# Functions in Zen

## Named Functions

```zen
function greet(name) {
    return "Hello, " + name
}
// Aliases: func, fn, def
```

## Lambdas

```zen
let square = lambda x { x * x }
let add = fn(a, b) { a + b }
```

## First-Class Values

```zen
function call_twice(f, val) { f(f(val)) }
call_twice(lambda x { x * 2 }, 3)   // 12
```

## Closures

```zen
function counter(n) {
    return lambda() { n = n + 1; return n }
}
let c = counter(0)
c()   // 1
c()   // 2
```
