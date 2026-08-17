# Functions

Complete reference for defining, calling, and composing functions in Zen — including named functions, lambdas, closures, higher-order functions, and recursion.

## Named Functions

### Basic syntax

```
function greet(name) {
    return "Hello, " + name + "!"
}

print greet("Zen")    // Hello, Zen!
```

### `def` is an alias for `function`

```
def add(a, b) {
    return a + b
}

print add(2, 3)    // 5
```

### Functions without `return`

Functions without an explicit `return` return `null`:

```
function say_hello(name) {
    print "Hello, " + name
}

let result = say_hello("World")
print result    // null
```

### `return` without a value

```
function check(x) {
    if x < 0 {
        return     // returns null, exits early
    }
    print x
}
```

---

## Parameters

### Basic parameters

```
function add(a, b) {
    return a + b
}

print add(2, 3)    // 5
```

### Default parameters

```
function greet(name = "World") {
    return "Hello, " + name + "!"
}

print greet()           // Hello, World!
print greet("Alice")    // Hello, Alice!
```

### Multiple defaults

```
function configure(host = "localhost", port = 8080, debug = false) {
    print "host={host} port={port} debug={debug}"
}

configure()                    // host=localhost port=8080 debug=false
configure("0.0.0.0")           // host=0.0.0.0 port=8080 debug=false
configure("0.0.0.0", 3000)     // host=0.0.0.0 port=3000 debug=false
configure(debug = true)        // host=localhost port=8080 debug=true
```

### Parameters are passed by value

Zen passes values by reference for mutable types (lists, dicts, instances):

```
function modify(list) {
    list.append(4)
}

let items = [1, 2, 3]
modify(items)
print items    // [1, 2, 3, 4] — the original list was modified!
```

But reassigning the parameter doesn't affect the caller:

```
function reassign(x) {
    x = 99
}

let val = 10
reassign(val)
print val    // 10 — unchanged
```

---

## Lambda Expressions

Short anonymous functions using the `lambda` keyword:

```
let double = lambda x: x * 2
print double(5)     // 10

let add = lambda x, y: x + y
print add(2, 3)     // 5

let greet = lambda: "Hello!"
print greet()        // Hello!
```

### Lambda with complex bodies

Lambdas support single expressions only. For multi-line logic, use arrow functions or `function`:

```
// This works (single expression):
let abs = lambda x: x if x >= 0 else -x
print abs(-5)    // 5

// For multiple statements, use arrow function:
let process = (x) => {
    let result = x * 2
    return result + 1
}
```

---

## Arrow Functions

Even shorter syntax using `=>`:

### Single expression

```
let double = (x) => x * 2
let add = (x, y) => x + y
let say_hello = () => "Hello!"

print double(5)     // 10
print add(2, 3)     // 5
print say_hello()   // Hello!
```

### Multi-statement arrows

```
let process = (x) => {
    let result = x * 2
    print "Processing: {x} -> {result}"
    return result + 1
}

print process(5)    // Processing: 5 -> 10 /n 11
```

### Arrow functions as callbacks

```
let numbers = [1, 2, 3, 4, 5]

let doubled = numbers.map((x) => x * 2)
print doubled    // [2, 4, 6, 8, 10]

let evens = numbers.filter((x) => x % 2 == 0)
print evens      // [2, 4]

let sum = numbers.reduce((acc, x) => acc + x)
print sum        // 15
```

### Arrow functions in sorting

```
let people = [
    {name: "Alice", age: 30},
    {name: "Bob", age: 25},
    {name: "Charlie", age: 35}
]

let sorted = people.sorted((a, b) => a.age < b.age)
print sorted[0].name    // Bob (youngest)
```

---

## Closures

Functions capture their surrounding scope:

### Basic closure

```
function make_counter(start) {
    let count = start
    return function() {
        count = count + 1
        return count
    }
}

let counter = make_counter(0)
print counter()    // 1
print counter()    // 2
print counter()    // 3
```

### Closure over multiple variables

```
function make_bank_account(balance) {
    return {
        deposit: function(amount) {
            balance = balance + amount
            return balance
        },
        withdraw: function(amount) {
            if amount > balance {
                throw "Insufficient funds"
            }
            balance = balance - amount
            return balance
        },
        get_balance: function() {
            return balance
        }
    }
}

let account = make_bank_account(100)
print account.deposit(50)        // 150
print account.withdraw(30)       // 120
print account.get_balance()      // 120
```

### Closure in loops

```
let functions = []
for i in 1 -> 5 {
    let x = i
    functions.append(function() { return x * 2 })
}

print functions[0]()    // 2
print functions[3]()    // 8
```

### Closure capturing outer variable

```
let multiplier = 3

function make_tripler() {
    return function(x) {
        return x * multiplier   // captures multiplier
    }
}

let tripler = make_tripler()
print tripler(5)    // 15

multiplier = 10     // changes the captured variable
print tripler(5)    // 50 (uses the updated value!)
```

---

## Higher-Order Functions

Functions that take or return other functions:

### Function as parameter

```
function apply_twice(fn, x) {
    return fn(fn(x))
}

let double = (x) => x * 2
print apply_twice(double, 3)    // 12 (3 -> 6 -> 12)
```

### Function as return value

```
function make_multiplier(n) {
    return function(x) {
        return x * n
    }
}

let double = make_multiplier(2)
let triple = make_multiplier(3)

print double(5)    // 10
print triple(5)    // 15
```

### Map, filter, reduce

```
let nums = [1, 2, 3, 4, 5]

// map: transform each item
let squared = nums.map((x) => x ** 2)
print squared    // [1, 4, 9, 16, 25]

// filter: keep items where function returns truthy
let evens = nums.filter((x) => x % 2 == 0)
print evens      // [2, 4]

// reduce: fold to single value
let sum = nums.reduce((acc, x) => acc + x)
print sum        // 15

// reduce with initial value
let product = nums.reduce((acc, x) => acc * x, 1)
print product    // 120
```

### Built-in higher-order functions

```
// enumerate: adds index
let items = ["a", "b", "c"]
for i, item in enumerate(items) {
    print "{i}: {item}"
}
// 0: a
// 1: b
// 2: c

// enumerate with start index
for i, item in enumerate(items, 1) {
    print "{i}. {item}"
}
// 1. a
// 2. b
// 3. c

// zip: combine lists
let names = ["Alice", "Bob"]
let scores = [95, 87]
let combined = zip(names, scores)
print combined    // [[Alice, 95], [Bob, 87]]
```

---

## Recursion

### Basic recursion

```
function factorial(n) {
    if n <= 1 { return 1 }
    return n * factorial(n - 1)
}

print factorial(5)    // 120
print factorial(0)    // 1
```

### Fibonacci

```
function fibonacci(n) {
    if n <= 1 { return n }
    return fibonacci(n - 1) + fibonacci(n - 2)
}

for i in 0 -> 10 {
    print fibonacci(i)
}
// 0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55
```

### Tree traversal

```
function sum_tree(node) {
    let total = node["value"]
    for child in node["children"] ?? [] {
        total = total + sum_tree(child)
    }
    return total
}

let tree = {
    "value": 1,
    "children": [
        {"value": 2, "children": []},
        {"value": 3, "children": [
            {"value": 4, "children": []}
        ]}
    ]
}

print sum_tree(tree)    // 10 (1+2+3+4)
```

### Mutual recursion

```
function is_even(n) {
    if n == 0 { return true }
    return is_odd(n - 1)
}

function is_odd(n) {
    if n == 0 { return false }
    return is_even(n - 1)
}

print is_even(4)    // true
print is_odd(3)     // true
```

---

## Method Chaining

Action methods return the element itself for chaining:

```
find("#user")
    .fill("admin")
    .check()
    .click()
```

### Chaining with custom functions

```
function Builder() {
    let parts = []
    return {
        add: function(part) {
            parts.append(part)
            return this    // enables chaining
        },
        build: function() {
            return parts.join("-")
        }
    }
}

let result = Builder()
    .add("hello")
    .add("world")
    .add("zen")
    .build()

print result    // hello-world-zen
```

---

## Method Binding

Instance methods are automatically bound when accessed via `instance.method()`:

```
class Logger {
    __init__ = function(self, prefix) {
        self.prefix = prefix
    }
    log = function(self, msg) {
        print self.prefix + ": " + msg
    }
}

let logger = new Logger("INFO")

// Pass method as callback — self is preserved
["hello", "world"].each(logger.log)
// INFO: hello
// INFO: world
```

---

## Class as Expression

Classes can be defined inline:

```
let Dog = class {
    __init__ = function(self, name) {
        self.name = name
    }
    speak = function(self) {
        return self.name + " says woof"
    }
}

let rex = new Dog("Rex")
print rex.speak()    // Rex says woof
```

---

## Pro Tips

1. **Use arrow functions for callbacks.** `(x) => x * 2` is cleaner than `function(x) { return x * 2 }`.
2. **Use `lambda` for simple one-liners.** `lambda x: x * 2` is the most concise.
3. **Closures capture by reference.** Changes to captured variables are visible to the closure.
4. **Use `reduce` for accumulation.** Sum, product, concatenation — anything that folds a list.
5. **Name your functions clearly.** `calculate_tax` is better than `calc` or `f`.
6. **Use default parameters wisely.** They make APIs more convenient without sacrificing flexibility.

---

## Common Mistakes

### Forgetting `return`

```
// WRONG — returns null
function double(x) {
    x * 2
}

// CORRECT
function double(x) {
    return x * 2
}
```

### Recursive base case

```
// WRONG — infinite recursion
function countdown(n) {
    print n
    countdown(n - 1)    // no base case!
}

// CORRECT
function countdown(n) {
    if n <= 0 { return }
    print n
    countdown(n - 1)
}
```

### Confusing `lambda` with arrow functions

```
// lambda: single expression
let f = lambda x, y: x + y

// arrow: can have block body
let g = (x, y) => {
    let result = x + y
    return result
}
```

### Closure variable capture

```
// WRONG — captures by reference, not value
let funcs = []
for i in 1 -> 3 {
    funcs.append(function() { return i })
}
print funcs[0]()    // 3 (all closures see i=3!)

// CORRECT — capture current value
let funcs = []
for i in 1 -> 3 {
    let captured = i
    funcs.append(function() { return captured })
}
print funcs[0]()    // 1
```

---

## See Also

- [Control Flow](control-flow.md) — Return, break, continue
- [Classes](classes.md) — Methods and constructors
- [Collections](collections.md) — Map, filter, reduce on lists
- [Variables](variables.md) — Scope and closures
