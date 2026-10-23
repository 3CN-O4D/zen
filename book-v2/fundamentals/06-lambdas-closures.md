# Chapter 6: Lambdas and Closures

Lambdas (anonymous functions) and closures are powerful features for writing concise, flexible code.

## Lambda Functions

Lambdas are functions without names, defined inline with `fn`.

### Basic Lambda

```zen
let double = fn(x) { return x * 2 }
print(double(5))  # 10

let square = fn(x) { return x * x }
print(square(7))  # 49
```

### Multiple Parameters

```zen
let add = fn(a, b) { return a + b }
let multiply = fn(a, b) { return a * b }

print(add(3, 4))       # 7
print(multiply(3, 4))  # 12
```

### No Parameters

```zen
let greet = fn() { return "Hello!" }
print(greet())
```

### Lambdas with Blocks

```zen
let process = fn(x) {
    let result = x * 2 + 10
    return result
}

print(process(5))  # 20
```

## Using Lambdas as Arguments

Lambdas are perfect for passing to functions:

```zen
func apply_twice(func, value) {
    return func(func(value))
}

print(apply_twice(fn(x) { return x + 1 }, 5))  # 7
print(apply_twice(fn(x) { return x * 2 }, 3))  # 12
```

## Higher-Order Functions with Lambdas

### Map-like Pattern

```zen
func map_list(list, func) {
    let result = []
    for item in list {
        result.push(func(item))
    }
    return result
}

let numbers = [1, 2, 3, 4, 5]
let squared = map_list(numbers, fn(x) { return x * x })
let doubled = map_list(numbers, fn(x) { return x * 2 })

print(squared)  # [1, 4, 9, 16, 25]
print(doubled)  # [2, 4, 6, 8, 10]
```

### Filter-like Pattern

```zen
func filter_list(list, predicate) {
    let result = []
    for item in list {
        if predicate(item) {
            result.push(item)
        }
    }
    return result
}

let numbers = [1, 2, 3, 4, 5, 6, 7, 8]
let evens = filter_list(numbers, fn(x) { return x % 2 == 0 })
let odds = filter_list(numbers, fn(x) { return x % 2 == 1 })

print(evens)  # [2, 4, 6, 8]
print(odds)   # [1, 3, 5, 7]
```

### Reduce-like Pattern

```zen
func reduce_list(list, func, initial) {
    let acc = initial
    for item in list {
        acc = func(acc, item)
    }
    return acc
}

let numbers = [1, 2, 3, 4, 5]
let sum = reduce_list(numbers, fn(a, b) { return a + b }, 0)
let product = reduce_list(numbers, fn(a, b) { return a * b }, 1)

print(sum)      # 15
print(product)  # 120
```

## Closures

A closure is a function that captures variables from its surrounding scope.

### Basic Closure

```zen
func make_counter() {
    let count = 0
    func increment() {
        count = count + 1
        return count
    }
    return increment
}

let counter = make_counter()
print(counter())  # 1
print(counter())  # 2
print(counter())  # 3
```

Each call to `make_counter()` creates a new closure with its own `count`:

```zen
let counter1 = make_counter()
let counter2 = make_counter()

print(counter1())  # 1
print(counter1())  # 2
print(counter2())  # 1 (separate counter)
print(counter1())  # 3
```

### Closure with Parameters

```zen
func make_adder(x) {
    func adder(y) {
        return x + y
    }
    return adder
}

let add5 = make_adder(5)
let add10 = make_adder(10)

print(add5(3))   # 8
print(add10(3))  # 13
print(add5(20))  # 25
```

### Multiple Captured Variables

```zen
func make_calculator(a, b) {
    func add() { return a + b }
    func subtract() { return a - b }
    func multiply() { return a * b }
    func divide() { return a / b }
    
    return {
        "add": add,
        "subtract": subtract,
        "multiply": multiply,
        "divide": divide
    }
}

let calc = make_calculator(10, 2)
print(calc.add())       # 12
print(calc.subtract())  # 8
print(calc.multiply())  # 20
print(calc.divide())    # 5
```

## Practical Applications

### Event Handlers Pattern

```zen
func create_button(label) {
    func on_click() {
        print("Button '" + label + "' clicked!")
    }
    return on_click
}

let save_btn = create_button("Save")
let cancel_btn = create_button("Cancel")

save_btn()    # Button 'Save' clicked!
cancel_btn()  # Button 'Cancel' clicked!
```

### Private Variables

Closures can simulate private state:

```zen
func bank_account(initial_balance) {
    let balance = initial_balance
    
    func deposit(amount) {
        if amount > 0 {
            balance = balance + amount
            return balance
        }
        return balance
    }
    
    func withdraw(amount) {
        if amount > 0 && amount <= balance {
            balance = balance - amount
            return balance
        }
        return balance
    }
    
    func get_balance() {
        return balance
    }
    
    return {
        "deposit": deposit,
        "withdraw": withdraw,
        "get_balance": get_balance
    }
}

let account = bank_account(100)
print(account.get_balance())  # 100
account.deposit(50)
print(account.get_balance())  # 150
account.withdraw(30)
print(account.get_balance())  # 120
# balance is not directly accessible from outside
```

### Memoization with Closures

```zen
func memoize(func) {
    let cache = {}
    
    func memoized(x) {
        let key = str(x)
        if cache.contains(key) {
            print("Cache hit for " + key)
            return cache[key]
        }
        print("Computing for " + key)
        let result = func(x)
        cache[key] = result
        return result
    }
    
    return memoized
}

func expensive(x) {
    # Simulate expensive computation
    return x * x
}

let fast_expensive = memoize(expensive)
print(fast_expensive(5))  # Computing for 5, returns 25
print(fast_expensive(5))  # Cache hit for 5, returns 25
print(fast_expensive(6))  # Computing for 6, returns 36
```

## Lambda Short Syntax (If Available)

Some implementations may support shorter lambda syntax, but the `fn` form is standard in Zen:

```zen
# Standard form
let double = fn(x) { return x * 2 }

# If arrow syntax supported (check Zen version)
# let double = x => x * 2
```

## When to Use Lambdas vs Functions

**Use named functions when:**
- Function is complex or long
- Function will be reused in multiple places
- You want clear documentation
- Function needs a descriptive name

**Use lambdas when:**
- Function is short and simple
- Used as argument to higher-order function
- One-time use
- Want to keep code concise

## Common Pitfalls

### Variable Capture in Loops

Be careful capturing loop variables:

```zen
let funcs = []
let i = 0
while i < 3 {
    let j = i  # Capture in new scope
    funcs.push(fn() { return j })
    i = i + 1
}

print(funcs[0]())  # 0
print(funcs[1]())  # 1
print(funcs[2]())  # 2
```

## Chapter Summary

- Lambdas are anonymous functions created with `fn(params) { body }`
- Can be assigned to variables and passed as arguments
- Closures capture variables from their enclosing scope
- Useful for higher-order functions, callbacks, and private state
- Each closure maintains its own captured state
- Great for functional programming patterns

**Next:** Part 2 - Core Concepts (Strings, Lists, Dictionaries)
