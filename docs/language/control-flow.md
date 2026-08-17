# Control Flow

Complete reference for every control flow construct in Zen: conditionals, loops, pattern matching, error handling, and more.

## If / Elif / Else

### Basic syntax

```
let score = 85

if score >= 90 {
    print "Grade: A"
} elif score >= 80 {
    print "Grade: B"
} elif score >= 70 {
    print "Grade: C"
} else {
    print "Grade: F"
}
// Grade: B
```

### `else if` (synonymous with `elif`)

```
if score >= 90 {
    print "A"
} else if score >= 80 {
    print "B"
} else {
    print "C"
}
```

### If as expression

`if` returns a value, so it can be used in assignments:

```
let status = if score >= 50 { "pass" } else { "fail" }
print status    // pass
```

### Nested conditions

```
let age = 25
let has_id = true

if age >= 21 {
    if has_id {
        print "Entry allowed"
    } else {
        print "Need ID"
    }
} else {
    print "Too young"
}
// Entry allowed
```

### Conditions don't need parentheses

```
// Both work:
if x > 0 { print "positive" }
if (x > 0) { print "positive" }
```

---

## Switch / Case

Multi-branch selection based on value equality:

```
let command = "start"

switch command {
    case "start" {
        print "Starting server..."
    }
    case "stop" {
        print "Stopping server..."
    }
    case "restart" {
        print "Restarting server..."
    }
    default {
        print "Unknown command: {command}"
    }
}
// Starting server...
```

### Key rules

- The first matching case wins
- `default` is optional
- Cases use `==` for comparison
- No fall-through (unlike C/JS)

```
let day = datetime.weekday()

switch day {
    case 0 { print "Monday" }
    case 1 { print "Tuesday" }
    case 2 { print "Wednesday" }
    case 3 { print "Thursday" }
    case 4 { print "Friday" }
    case 5 { print "Saturday" }
    case 6 { print "Sunday" }
    default { print "Invalid day" }
}
```

### Nested switch

```
let type = "admin"
let action = "delete"

switch type {
    case "admin" {
        switch action {
            case "delete" { print "Admin delete" }
            default { print "Admin action" }
        }
    }
    case "user" {
        print "User action"
    }
    default {
        print "Unknown type"
    }
}
```

---

## With Statement

Temporarily extends scope — useful for isolating variables:

```
with load_config() as cfg {
    print cfg["host"]
    print cfg["port"]
}
// cfg is not accessible here
```

### Without `as` binding

```
with compute_heavy_value() {
    // result is available as the implicit value
    print result
}
```

### Practical usage

```
function process_file(path) {
    with fs.read(path) as content {
        let lines = content.split("\n")
        for line in lines {
            print line
        }
    }
    // content is not accessible here
}
```

---

## While Loops

```
let x = 3
while x > 0 {
    print x
    x = x - 1
}
// 3
// 2
// 1
```

### Infinite loops

```
while true {
    let input = input("Enter command: ")
    if input == "quit" {
        break
    }
    print "You said: {input}"
}
```

### While with complex conditions

```
let attempts = 0
let max_attempts = 5
let success = false

while attempts < max_attempts and !success {
    attempts = attempts + 1
    print "Attempt {attempts}..."
    success = try_operation()
}

if success {
    print "Succeeded after {attempts} attempts"
} else {
    print "Failed after {max_attempts} attempts"
}
```

### Nested while loops

```
let i = 0
while i < 3 {
    let j = 0
    while j < 3 {
        print "{i},{j}"
        j = j + 1
    }
    i = i + 1
}
// 0,0  0,1  0,2  1,0  1,1  1,2  2,0  2,1  2,2
```

---

## For / In Loops

### Iterate over a list

```
for fruit in ["apple", "banana", "cherry"] {
    print fruit
}
// apple
// banana
// cherry
```

### Iterate over a range

```
for i in 1 -> 5 {
    print i
}
// 1, 2, 3, 4, 5
```

### Iterate over dict keys

```
let user = {name: "Alice", age: 30}
for key in user {
    print "{key}: {user[key]}"
}
// name: Alice
// age: 30
```

### Iterate with enumerate

```
let fruits = ["apple", "banana", "cherry"]
for i, fruit in enumerate(fruits) {
    print "{i + 1}. {fruit}"
}
// 1. apple
// 2. banana
// 3. cherry
```

### Iterate over strings (character by character)

```
for char in "hello" {
    print char
}
// h
// e
// l
// l
// o
```

### Iterate over element lists (browser)

```
for link in attrs("a", "href") {
    print link
}
```

### Nested for loops

```
for i in 1 -> 3 {
    for j in 1 -> 3 {
        print "{i} x {j} = {i * j}"
    }
}
```

### For with step

```
for i in 0 -> 20 by 5 {
    print i
}
// 0, 5, 10, 15, 20
```

---

## Break & Continue

### Break — Exit the loop

```
let i = 0
while true {
    i = i + 1
    if i > 5 { break }
    print i
}
// 1, 2, 3, 4, 5
```

### Continue — Skip to next iteration

```
for i in 1 -> 10 {
    if i % 2 == 0 { continue }    // skip even numbers
    print i
}
// 1, 3, 5, 7, 9
```

### Combined break and continue

```
let i = 0
while true {
    i = i + 1
    if i == 2 { continue }     // skip 2
    print i
    if i >= 5 { break }        // stop at 5
}
// 1, 3, 4, 5
```

### Break in nested loops

`break` only exits the innermost loop:

```
for i in 1 -> 3 {
    for j in 1 -> 3 {
        if j == 2 { break }    // only breaks inner loop
        print "{i},{j}"
    }
}
// 1,1
// 2,1
// 3,1
```

---

## Try / Catch / Finally

### Basic try/catch

```
try {
    let data = json.parse("not json")
} catch {
    print "Parse failed"
    print error    // the error message (built-in variable)
}
```

### Named error variable

```
try {
    risky_operation()
} catch err {
    print "Caught: " + err
}
```

### With finally (always runs)

```
let file = null
try {
    file = open_file("data.txt")
    process(file)
} catch err {
    print "Error: " + err
} finally {
    if file != null {
        close_file(file)
    }
    print "Cleanup done"
}
```

### Try as expression

`try` can return a value:

```
let result = try json.parse("bad input") catch { null }
print result    // null (instead of crashing)
```

### Catching specific error types

```
try {
    errors.define("ValidationError", "Error", "validation failed")
    throw new ValidationError("invalid email")
} catch ValidationError as err {
    print "Validation: " + err
} catch {
    print "Other error: " + error
}
```

---

## Throw / Raise

Explicitly raise an exception:

```
throw "Something went wrong"
raise "Invalid input"     // raise is an alias for throw
```

### Throw with error objects

```
throw {code: 404, message: "Not found"}
throw {type: "AuthError", message: "Unauthorized"}
```

### Throw custom error classes

```
errors.define("NotFoundError", "Error", "resource not found")
throw new NotFoundError("user not found")
```

### Throw in functions

```
function divide(a, b) {
    if b == 0 {
        throw "Division by zero"
    }
    return a / b
}

try {
    print divide(10, 0)
} catch err {
    print err    // Division by zero
}
```

---

## Assert

Debug assertions that raise on failure:

```
let x = 10
assert x > 0
print "x is positive"

// With custom message
assert x > 0, "x must be positive"
```

### Assert in functions

```
function set_age(age) {
    assert age >= 0, "Age cannot be negative"
    assert age <= 150, "Age seems unrealistic"
    self.age = age
}
```

---

## Infinite Loops

```
while true {
    let input = input(">> ")
    if input == "exit" { break }
    process(input)
}
```

### Loop with timeout

```
let start = time.now()
let timeout = 30

while true {
    if time.now() - start > timeout {
        print "Timed out"
        break
    }
    // do work
    sleep(0.1)
}
```

---

## Loop Patterns

### Accumulator pattern

```
let total = 0
for num in [10, 20, 30, 40] {
    total = total + num
}
print total    // 100
```

### Finding items

```
let target = "cherry"
let fruits = ["apple", "banana", "cherry", "date"]
let found = null

for fruit in fruits {
    if fruit == target {
        found = fruit
        break
    }
}

print found    // cherry
```

### Filtering into a new list

```
let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
let evens = []

for n in numbers {
    if n % 2 == 0 {
        evens.append(n)
    }
}
print evens    // [2, 4, 6, 8, 10]
```

### Nested iteration

```
let matrix = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]

for row in matrix {
    for cell in row {
        print cell
    }
}
// 1, 2, 3, 4, 5, 6, 7, 8, 9
```

### Early exit with flag

```
let numbers = [1, 3, 5, 7, 8, 9, 11]
let found_even = false

for n in numbers {
    if n % 2 == 0 {
        print "Found even: {n}"
        found_even = true
        break
    }
}

if !found_even {
    print "No even numbers found"
}
```

---

## Pro Tips

1. **Use `elif` for chained conditions.** It's cleaner than nested `if` blocks.
2. **Use `switch` for multiple equality checks.** More readable than many `elif` chains.
3. **Use `with` for scoped variables.** Keeps temporary variables from leaking.
4. **Prefer `for-in` over `while` for iteration.** Less error-prone (no manual counter).
5. **Use `continue` to flatten code.** Instead of `if !condition { ... }`, use `if condition { continue }`.
6. **Always have a `default` in switch.** Catches unexpected values early.
7. **Use `try` as expression.** `let x = try parse(input) catch { null }` is concise.

---

## Common Mistakes

### Missing curly braces

```
// WRONG
if x > 0
    print x

// CORRECT
if x > 0 {
    print x
}
```

### Forgetting that `for` requires a list

```
// WRONG — "hello" is not iterable with for-in
for char in "hello" { print char }

// CORRECT
for i in 0 -> 4 { print "hello"[i] }
```

### Switch case without braces

```
// WRONG
switch x {
    case 1 print "one"    // ERROR
}

// CORRECT
switch x {
    case 1 { print "one" }
}
```

### Break only exits the innermost loop

```
// This only breaks the inner loop:
for i in 1 -> 3 {
    for j in 1 -> 3 {
        if j == 2 { break }
        print "{i},{j}"
    }
}
// 1,1 / 2,1 / 3,1 (inner loop breaks at j=2, outer continues)
```

---

## See Also

- [Functions](functions.md) — Return values and control flow
- [Errors](errors.md) — Try/catch/finally in detail
- [Operators](operators.md) — Ternary conditional operator
- [Variables](variables.md) — Scoping in loops and blocks
