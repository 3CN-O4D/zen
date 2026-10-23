# Chapter 5: Functions

Functions are reusable blocks of code that perform specific tasks. This chapter covers function definition, parameters, returns, scope, and advanced patterns.

## Defining Functions

Use the `func` keyword:

```zen
func say_hello() {
    print("Hello!")
}

say_hello()  # Call the function
```

## Parameters and Arguments

### Basic Parameters

```zen
func greet(name) {
    print("Hello, " + name + "!")
}

greet("Alice")
greet("Bob")
```

### Multiple Parameters

```zen
func add(a, b) {
    return a + b
}

print(add(5, 3))  # 8
```

### Default Parameters

Provide default values for optional parameters:

```zen
func greet(name, greeting = "Hello") {
    print(greeting + ", " + name + "!")
}

greet("Alice")              # Hello, Alice!
greet("Bob", "Hi")         # Hi, Bob!
greet("Charlie", "Hey")    # Hey, Charlie!
```

### Multiple Defaults

```zen
func create_user(name, age = 18, active = true) {
    return {
        "name": name,
        "age": age,
        "active": active
    }
}

print(create_user("Alice"))
print(create_user("Bob", 25))
print(create_user("Charlie", 30, false))
```

## Return Values

### Single Return Value

```zen
func square(x) {
    return x * x
}

let result = square(7)
print(result)  # 49
```

### Early Return

```zen
func is_adult(age) {
    if age < 0 {
        return false
    }
    if age >= 18 {
        return true
    }
    return false
}

print(is_adult(20))   # true
print(is_adult(15))   # false
print(is_adult(-5))   # false
```

### Returning Multiple Values

Return a list and destructure:

```zen
func get_coords() {
    return [10, 20]
}

let [x, y] = get_coords()
print(x, y)  # 10 20

# Or access directly
let coords = get_coords()
print(coords[0], coords[1])
```

Return a dict:

```zen
func divide(a, b) {
    if b == 0 {
        return {"error": "Division by zero", "ok": false}
    }
    return {"result": a / b, "ok": true}
}

let res = divide(10, 2)
if res.ok {
    print(res.result)
}
```

## Function Scope

### Local Variables

```zen
func test() {
    let local_var = 100
    print(local_var)  # Accessible here
}

test()
# print(local_var)  # Error: not defined
```

### Accessing Outer Scope

```zen
let global = "I'm global"

func read_global() {
    print(global)  # Can read
}

read_global()
```

### Shadowing

```zen
let x = 10

func test() {
    let x = 20  # Shadows outer x
    print(x)    # 20
}

test()
print(x)  # 10
```

## Recursion

Functions can call themselves:

```zen
func factorial(n) {
    if n <= 1 {
        return 1
    }
    return n * factorial(n - 1)
}

print(factorial(5))  # 120
print(factorial(6))  # 720
```

### Fibonacci

```zen
func fib(n) {
    if n <= 1 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

print(fib(0))  # 0
print(fib(1))  # 1
print(fib(7))  # 13
print(fib(10)) # 55
```

**Note:** Deep recursion may hit stack limits. For deep recursion, consider iterative approaches.

## Higher-Order Functions

Functions can take other functions as arguments:

```zen
func apply(func, value) {
    return func(value)
}

func double(x) {
    return x * 2
}

func triple(x) {
    return x * 3
}

print(apply(double, 5))  # 10
print(apply(triple, 5))  # 15
```

Functions can return functions:

```zen
func make_multiplier(factor) {
    func multiplier(x) {
        return x * factor
    }
    return multiplier
}

let times3 = make_multiplier(3)
let times5 = make_multiplier(5)

print(times3(10))  # 30
print(times5(10))  # 50
```

## Practical Examples

### Validation Function

```zen
func validate_email(email) {
    if !email.contains("@") {
        return false
    }
    if !email.contains(".") {
        return false
    }
    return true
}

print(validate_email("test@example.com"))  # true
print(validate_email("invalid"))           # false
```

### Data Processing

```zen
func filter_even(numbers) {
    let result = []
    for n in numbers {
        if n % 2 == 0 {
            result.push(n)
        }
    }
    return result
}

func sum_list(numbers) {
    let total = 0
    for n in numbers {
        total = total + n
    }
    return total
}

let nums = [1, 2, 3, 4, 5, 6]
let evens = filter_even(nums)
print("Evens:", evens)
print("Sum:", sum_list(evens))
```

### Calculator with Operations

```zen
func calculate(a, b, op) {
    if op == "+") { return a + b }
    elif op == "-" { return a - b }
    elif op == "*" { return a * b }
    elif op == "/" { 
        if b == 0 { throw "Division by zero" }
        return a / b 
    }
    else { throw "Unknown operation" }
}

print(calculate(10, 5, "+"))  # 15
print(calculate(10, 5, "*"))  # 50
```

## Function Documentation

While Zen may not have formal docstrings, use comments:

```zen
# Calculates the area of a rectangle
# width: The width in units
# height: The height in units
# Returns: Area as float/int
func rectangle_area(width, height) {
    return width * height
}
```

## Best Practices

1. **Single Responsibility** - Each function should do one thing
2. **Descriptive Names** - Use verbs: `calculate_total()`, `get_user()`
3. **Keep it Short** - If a function is too long, break it down
4. **Avoid Side Effects** - When possible, return values instead of modifying globals
5. **Use Default Parameters** - For optional functionality
6. **Early Returns** - Handle edge cases early

## Chapter Summary

- Define with `func name(params) { body }`
- Call with `name(args)`
- Use `return` to return values
- Support default parameters
- Functions have local scope
- Support recursion and higher-order functions
- Functions are first-class values

**Next:** Chapter 6 - Lambdas and Closures
