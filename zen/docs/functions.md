# Functions in Zen

Functions are defined with `function`, `func`, `fn`, or `def` keywords.

```zen
function greet(name) {
    return "Hello, " + name
}

// Lambda / anonymous function
let square = lambda x { x * x }

// Default parameters
function add(a, b) { return a + b }

// Functions can be passed as arguments
function call_twice(f, val) { return f(f(val)) }
```

Functions are first-class values: they can be assigned to variables, passed to other functions, and returned from functions.
