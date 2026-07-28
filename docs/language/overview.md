# Language Overview

Zen is a lightweight, interpreted language designed for browser automation. It combines Python-like readability with JavaScript-inspired features.

## Design Philosophy

Zen was designed to make browser automation feel natural. Instead of writing Python/JS boilerplate to drive a browser, you write intentions:

```
go "https://example.com"
find("h1").text
fill "#search" with "query"
click "button"
```

## Key Features

### Clean Syntax

```
// Variables
let name = "Zen"
const PI = 3.14

// Functions
function greet(name) {
    return "Hello, " + name + "!"
}

// Control flow
if score >= 90 {
    print "A"
} elif score >= 80 {
    print "B"
} else {
    print "C"
}
```

### Expressive Operators

```
1 -> 5           // [1, 2, 3, 4, 5] (range)
"x" ?? "default" // "default" (nullish coalescing)
a === b          // strict equality
typeof x         // "int", "string", etc.
```

### Modern Features

```
// Arrow functions
let double = (x) => x * 2

// Template literals
print `Hello ${name}!`

// List comprehensions
let squares = [x ** 2 for x in 1 -> 5]

// Destructuring
let [a, b] = [1, 2]
let {name, age} = person

// Nullish coalescing
let value = config["key"] ?? "default"
```

### Browser Automation

```
// Finding elements
find("h1")                    // CSS selector
find(text="Click Here")       // by text
find_by_url("example.com")    // by URL

// Interacting
click("button")
fill("#input", "value")
check("#checkbox")

// Navigation
go "https://example.com"
back
forward
refresh
```

## Performance

Zen's runtime includes a **tree-walk interpreter** and an optional **Zen→Python bytecode compiler** that compiles hot paths to native Python bytecode for a 100–250× speedup on compute-heavy code (loops, arithmetic, ranges).

The compiler is automatic: any statement or expression that can be compiled safely is compiled on first execution and cached.

## What's Next?

- [Types](types.md) - Data types in Zen
- [Variables](variables.md) - Variables and scope
- [Operators](operators.md) - All operators
- [Control Flow](control-flow.md) - if, for, while, switch
- [Functions](functions.md) - Functions and closures
- [Classes](classes.md) - Object-oriented programming
