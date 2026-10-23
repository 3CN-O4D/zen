# Chapter 2: Data Types

Understanding data types is fundamental to programming in Zen. This chapter covers all built-in types in detail.

## Overview

Zen is dynamically typed - you don't need to declare types explicitly. The interpreter determines types at runtime. However, understanding types is crucial for writing correct programs.

## Primitive Types

### 1. Integers (`int`)

Integers are whole numbers, positive or negative, with no decimal point.

```zen
let x = 42
let y = -17
let z = 0
let big = 9223372036854775807  # Large integers supported

print(type(x))  # "int"
```

**Operations:**
```zen
let a = 10
let b = 3
print(a + b)   # Addition: 13
print(a - b)   # Subtraction: 7
print(a * b)   # Multiplication: 30
print(a / b)   # Division: 3 (integer division in some contexts)
print(a % b)   # Modulo: 1
print(a ** b)  # Exponentiation: 1000 (if supported)
```

### 2. Floats (`float`)

Floating-point numbers for decimals.

```zen
let pi = 3.14159
let temp = -2.5
let rate = 0.05

print(type(pi))  # "float"
```

**Mixing ints and floats:**
```zen
let result = 5 + 2.5  # Becomes float
print(type(result))  # "float"
print(result)        # 7.5
```

### 3. Booleans (`bool`)

True or false values.

```zen
let is_active = true
let is_done = false

print(type(is_active))  # "bool"

# From comparisons
let x = 5 > 3  # true
let y = 10 == 5  # false
```

### 4. Null (`null`)

Represents absence of value or undefined state.

```zen
let nothing = null
print(type(nothing))  # "null"
print(nothing)        # null

# Common use cases
func find_item(id) {
    if id not_found {
        return null
    }
    return item
}
```

### 5. Strings (`string`)

Text data enclosed in double quotes.

```zen
let name = "Alice"
let greeting = "Hello, World!"
let empty = ""
let with_spaces = "  spaces  "

print(type(name))  # "string"
```

See Chapter 7 for detailed string operations.

## Collection Types

### 6. Lists (`list`)

Ordered, mutable collections of values. Can contain mixed types.

```zen
# Empty list
let empty = []

# List with values
let numbers = [1, 2, 3, 4, 5]
let mixed = [1, "hello", true, null, 3.14]
let nested = [[1, 2], [3, 4]]

print(type(numbers))  # "list"
```

**Characteristics:**
- Ordered (index starts at 0)
- Mutable (can add/remove/change elements)
- Heterogeneous (mixed types allowed)
- Can contain any type including lists, dicts, objects

### 7. Dictionaries (`dict`)

Unordered collections of key-value pairs (or ordered in modern implementations).

```zen
# Empty dict
let empty = {}

# Dict with string keys
let person = {
    "name": "Bob",
    "age": 30,
    "city": "NYC"
}

# Dict with int keys
let lookup = {1: "one", 2: "two", 3: "three"}

print(type(person))  # "dict"
```

**Characteristics:**
- Key-value pairs
- Keys must be hashable (strings, ints, etc.)
- Values can be any type
- Mutable
- Access by key

### 8. Bytes (`bytes`)

Binary data.

```zen
# Bytes literals (if supported)
let data = b"hello"
print(type(data))  # "bytes"
```

## Special Types

### Functions as First-Class Values

```zen
func add(a, b) {
    return a + b
}

let f = add  # Assign function to variable
print(f(3, 4))  # Call through variable

let op = fn(x) { return x * 2 }  # Lambda
print(op(5))
```

### Objects (Instances of Classes)

```zen
class Person {}
let p = Person()
print(type(p))  # "object" or class name
```

## Type Checking and Conversion

### Checking Types

```zen
let x = 42
print(type(x))  # Returns type as string

if type(x) == "int" {
    print("It's an integer")
}
```

### Type Conversions

```zen
# String to int
let s = "123"
let n = int(s)
print(n + 7)  # 130

# Int to string
let num = 456
let str_num = str(num)
print("Value: " + str_num)

# String to float
let f_str = "3.14"
let f = float(f_str)
print(f * 2)

# Float to int (truncates)
let f2 = 3.9
let i2 = int(f2)
print(i2)  # 3

# Bool conversions
let b1 = bool(1)      # true (non-zero/non-null is truthy)
let b2 = bool(0)      # false
let b3 = bool("")     # false (empty string is falsy)
let b4 = bool("hi")   # true
let b5 = bool([])     # false
let b6 = bool(null)   # false
```

## Truthiness

Zen follows truthy/falsy rules:

**Falsy values:**
- `false`
- `null`
- `0` (integer zero)
- `0.0` (float zero)
- `""` (empty string)
- `[]` (empty list)
- `{}` (empty dict)

**Everything else is truthy:**
- Non-zero numbers
- Non-empty strings
- Non-empty collections
- Objects
- Functions

```zen
if [] {
    print("Won't print")
} else {
    print("Empty list is falsy")
}

if [1, 2] {
    print("Non-empty list is truthy")
}
```

## Type Coercion

Be aware of how Zen handles mixed types:

```zen
# String concatenation
print("Age: " + 30)  # May need explicit str() conversion

# Better to be explicit
print("Age: " + str(30))
```

## Checking for None/Null

```zen
let value = null

if value == null {
    print("Value is null")
}

if value is null {  # Alternative syntax if supported
    print("Value is null")
}
```

## Working with Types Practically

```zen
func describe(value) {
    let t = type(value)
    if t == "int" {
        print(str(value) + " is an integer")
    } elif t == "string" {
        print("'" + value + "' is a string of length " + len(value))
    } elif t == "list" {
        print("A list with " + len(value) + " elements")
    } elif t == "dict" {
        print("A dictionary")
    } else {
        print(str(value) + " is of type " + t)
    }
}

describe(42)
describe("hello")
describe([1, 2, 3])
describe({"a": 1})
```

## Chapter Summary

- **Primitives:** int, float, bool, null, string
- **Collections:** list (ordered), dict (key-value), bytes (binary)
- **Special:** functions, objects
- Use `type()` to check types
- Use `int()`, `float()`, `str()`, `bool()` for conversions
- Understand truthy/falsy for conditionals
- Zen is dynamically typed but type-aware

**Next:** Chapter 3 - Operators and Expressions
