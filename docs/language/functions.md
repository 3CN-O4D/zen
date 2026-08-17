# Functions

## Named Functions

```
function greet(name) {
    return "Hello, " + name + "!"
}

let msg = greet("Zen")
print msg   // Hello, Zen!
```

`def` is an alias for `function`:

```
def add(a, b) {
    return a + b
}
```

## Anonymous Functions

```
let double = function(n) {
    return n * 2
}
print double(21)   // 42
```

## Callbacks

```
find_all(".item").each(function(el, i) {
    print (i+1) + ". " + el.text
})
```

## Default Parameters

```
function greet(name = "World") {
    return "Hello, " + name + "!"
}
```

## Closures

Functions capture their surrounding scope:

```
function make_counter(start) {
    let count = start
    return function() {
        count = count + 1
        return count
    }
}

let counter = make_counter(0)
print counter()   // 1
print counter()   // 2
```

## Return

Functions without an explicit `return` return `null`. `return` with no value also returns `null`.

## Lambda Expressions

Short anonymous functions:

```
let double = lambda x: x * 2
let add = lambda x, y: x + y

print double(5)    // 10
print add(2, 3)    // 5
```

## Arrow Functions

Even shorter syntax:

```
let double = (x) => x * 2
let add = (x, y) => x + y
let greet = () => "Hello!"
```

Multi-statement arrows:

```
let process = (x) => {
    let result = x * 2
    return result + 1
}
```

## Higher-Order Functions

Functions that take or return other functions:

```
function apply_twice(fn, x) {
    return fn(fn(x))
}

let double = (x) => x * 2
print apply_twice(double, 3)   // 12
```

## Method Chaining

All action methods return the element itself for chaining:

```
find("#user").fill("admin").check()
find(".login-form").find("button").click()
```

## Recursive Functions

```
function factorial(n) {
    if n <= 1 { return 1 }
    return n * factorial(n - 1)
}

print factorial(5)   // 120
```
