# Quick Start

A complete beginner guide to Zen. Every concept is explained with code and expected output. By the end, you'll build a real-world project.

## Your First Script

Create a file called `hello.z`:

```
print "Hello, World!"
```

Run it:

```bash
zen run hello.z
```

Output:

```
Hello, World!
```

That's it — no semicolons, no `main()` function, no boilerplate. Just write and run.

---

## Variables

### Declaring variables with `let`

```
let name = "Zen"
let version = 1
let pi = 3.14159
let active = true
let nothing = null

print name       // Zen
print version    // 1
print pi         // 3.14159
print active     // true
print nothing    // null
```

### Constants with `const`

Constants cannot be reassigned:

```
const PI = 3.14159
const MAX_RETRIES = 3

print PI         // 3.14159

// This would cause an error:
// PI = 3.0       // Error: Cannot redefine constant 'PI'
```

### Bare assignment

If a variable doesn't exist, bare assignment creates it (same as `let`):

```
counter = 0       // auto-declares
counter = counter + 1
print counter     // 1
```

---

## Types

Zen has these core types:

| Type | Examples | Description |
|------|----------|-------------|
| Number | `42`, `3.14`, `-7` | Integer or floating-point |
| String | `"hello"`, `'world'` | Text data |
| Boolean | `true`, `false` | Logical values |
| Null | `null` | Absence of value |
| List | `[1, 2, 3]` | Ordered collection |
| Dict | `{"a": 1}` | Key-value mapping |
| Function | `function() {}` | Callable |

```
print typeof 42         // number
print typeof "hello"    // string
print typeof true       // bool
print typeof null       // null
print typeof [1, 2]     // list
print typeof {a: 1}     // dict
```

### Type conversion

```
print str(42)           // "42"
print int("42")         // 42
print float("3.14")     // 3.14
print bool(1)           // true
print bool(0)           // false
print bool("")          // false
print bool(null)        // false
```

---

## Strings

### Basic strings

```
let greeting = "Hello, World!"
let name = 'Zen'

print greeting           // Hello, World!
```

### String interpolation

Use `{variable}` to embed values inside double-quoted strings:

```
let name = "Alice"
let age = 30

print "My name is {name} and I'm {age} years old."
// My name is Alice and I'm 30 years old.
```

### Template literals

Use backticks with `${expression}` for full expressions:

```
let x = 10
let y = 20

print `${x} + ${y} = ${x + y}`
// 10 + 20 = 30
```

### String methods

```
print "hello".upper()                    // HELLO
print "HELLO".lower()                    // hello
print "  hello  ".strip()                // hello
print "a,b,c".split(",")                 // [a, b, c]
print "-".join(["a", "b", "c"])          // a-b-c
print "hello".replace("h", "H")          // Hello
print "hello".startswith("he")           // true
print "hello".endswith("lo")             // true
print "hello".find("ll")                 // 2
print "hello".len                        // 5
```

---

## If / Elif / Else

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

### Ternary expressions

```
let status = "pass" if score >= 50 else "fail"
print status   // pass

// Chained ternary
let level = "high" if age > 60 else "medium" if age > 30 else "young"
```

---

## Loops

### While loop

```
let count = 3
while count > 0 {
    print count
    count = count - 1
}
// 3
// 2
// 1
```

### For-in loop

```
for fruit in ["apple", "banana", "cherry"] {
    print fruit
}
// apple
// banana
// cherry
```

### For-in with ranges

```
for i in 1 -> 5 {
    print i
}
// 1, 2, 3, 4, 5

// Descending
for i in 5 -> 1 {
    print i
}
// 5, 4, 3, 2, 1

// With step
for i in 0 -> 10 by 2 {
    print i
}
// 0, 2, 4, 6, 8, 10
```

### Break and continue

```
for i in 1 -> 10 {
    if i == 3 { continue }   // skip 3
    if i == 7 { break }      // stop at 7
    print i
}
// 1, 2, 4, 5, 6
```

---

## Functions

### Named functions

```
function greet(name) {
    return "Hello, " + name + "!"
}

print greet("Zen")    // Hello, Zen!
```

### Default parameters

```
function greet(name = "World") {
    return "Hello, " + name + "!"
}

print greet()           // Hello, World!
print greet("Alice")    // Hello, Alice!
```

### Arrow functions

```
let double = (x) => x * 2
print double(5)          // 10

let add = (x, y) => x + y
print add(3, 4)          // 7

let say_hello = () => "Hello!"
print say_hello()        // Hello!
```

### Lambda expressions

```
let triple = lambda x: x * 3
print triple(4)          // 12
```

### Closures

Functions capture their surrounding scope:

```
function make_counter() {
    let count = 0
    return function() {
        count = count + 1
        return count
    }
}

let counter = make_counter()
print counter()    // 1
print counter()    // 2
print counter()    // 3
```

### Higher-order functions

```
function apply_twice(fn, x) {
    return fn(fn(x))
}

let double = (x) => x * 2
print apply_twice(double, 3)    // 12
```

---

## Lists

### Creating and accessing

```
let fruits = ["apple", "banana", "cherry"]
print fruits[0]         // apple
print fruits[-1]        // cherry (negative indexing)
print fruits.len         // 3
```

### Modifying

```
let items = [1, 2, 3]
items.append(4)          // [1, 2, 3, 4]
items.pop()              // returns 4, items is [1, 2, 3]
items.push(5)            // [1, 2, 3, 5]
items.sort()             // [1, 2, 3, 5]
items.reverse()          // [5, 3, 2, 1]
```

### List methods

```
let nums = [3, 1, 4, 1, 5, 9]

print nums.includes(4)      // true
print nums.indexOf(5)       // 4
print nums.count             // 6 (alias for len)
print nums.join("-")        // 3-1-4-1-5-9
```

### List comprehensions

```
let squares = [x ** 2 for x in 1 -> 5]
print squares    // [1, 4, 9, 16, 25]

let evens = [x for x in 1 -> 10 if x % 2 == 0]
print evens      // [2, 4, 6, 8, 10]
```

---

## Dicts

### Creating and accessing

```
let user = {"name": "Alice", "age": 30, "active": true}
print user["name"]       // Alice
print user.name           // dot notation also works
```

### Modifying

```
user["email"] = "alice@example.com"
user["age"] = 31
print user.len            // 4
```

### Dict methods

```
let data = {"a": 1, "b": 2, "c": 3}

print data.keys()         // [a, b, c]
print data.values()       // [1, 2, 3]
print data.items()        // [[a, 1], [b, 2], [c, 3]]
print data.has("a")       // true
print data.get("z")       // null
print data.get("z", 0)    // 0
```

---

## Imports and Modules

Zen has built-in modules available as globals — no import needed:

```
// File system
let content = fs.read("config.json")
fs.write("output.txt", "Hello!")

// HTTP
let resp = http.get("https://api.github.com")
print resp.status        // 200

// JSON
let data = json.parse('{"name": "Zen"}')
print data.name          // Zen

// Crypto
let hash = crypto.sha256("hello")
print hash               // 2cf24dba...

// Regular expressions
let match = re.search("\\d+", "abc 123 def")
print match.match        // 123
```

### Importing custom modules

```
// In utils.z:
function add(a, b) {
    return a + b
}

// In main.z:
import utils
print utils.add(2, 3)    // 5

// Or with from-import:
from utils import add
print add(2, 3)          // 5
```

---

## File I/O

### Reading files

```
let content = fs.read("data.txt")
print content
```

### Writing files

```
fs.write("output.txt", "Hello, World!")
print fs.read("output.txt")    // Hello, World!
```

### Appending to files

```
fs.write("log.txt", "")
fs.append("log.txt", "[INFO] Started\n")
fs.append("log.txt", "[INFO] Done\n")
print fs.read("log.txt")
```

### Checking file existence

```
if fs.exists("config.json") {
    let config = json.parse(fs.read("config.json"))
    print config
} else {
    print "No config found"
}
```

---

## HTTP Requests

### GET request

```
let resp = http.get("https://httpbin.org/get")
print resp.status       // 200
print resp.body         // response body as string
print resp.ok           // true (2xx status)
```

### POST request

```
let resp = http.post("https://httpbin.org/post",
    json={"name": "Zen", "version": "1.0"})
print resp.status       // 200
let data = resp.json()
print data["json"]["name"]    // Zen
```

### Custom headers

```
let resp = http.get("https://api.github.com/user",
    headers={"Authorization": "Bearer YOUR_TOKEN"})
```

---

## Error Handling

```
try {
    let content = fs.read("nonexistent.txt")
} catch err {
    print "Error: " + err
}
// Error: file not found: nonexistent.txt
```

### With finally

```
let file = null
try {
    file = fs.open("data.txt")
    // work with file
} catch err {
    print "Failed: " + err
} finally {
    print "Cleanup complete"
    // close file if needed
}
```

### Throwing errors

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

## Real-World Project: Weather Data Fetcher

This script fetches weather data, processes it, and saves a report:

```
// weather_report.z — Fetch and summarize weather data

// Configuration
let city = "Nairobi"
let api_url = "https://wttr.in/{city}?format=j1"

// Fetch weather data
print "Fetching weather for {city}..."
let resp = http.get(api_url)

if !resp.ok {
    print "Failed to fetch weather data"
    exit 1
}

let weather = resp.json()
let current = weather["current_condition"][0]

// Extract data
let temp_c = current["temp_C"]
let humidity = current["humidity"]
let desc = current["weatherDesc"][0]["value"]
let wind_speed = current["windspeedKmph"]

// Build report
let report = {
    "city": city,
    "temperature_c": temp_c,
    "humidity": humidity,
    "description": desc,
    "wind_speed_kmh": wind_speed,
    "timestamp": datetime.now()
}

// Pretty-print
print "\n=== Weather Report ==="
print "City: {city}"
print "Temperature: {temp_c}°C"
print "Humidity: {humidity}%"
print "Conditions: {desc}"
print "Wind: {wind_speed} km/h"

// Save to file
fs.mkdirs("reports")
json.save("reports/weather.json", report)
print "\nReport saved to reports/weather.json"
```

Run it:

```bash
zen run weather_report.z
```

---

## What's Next?

- [Shell Usage](shell.md) — Interactive REPL with tab completion and history
- [Scripts](scripts.md) — Running scripts, arguments, shebangs
- [Language Overview](../language/overview.md) — Deep dive into the language
- [Types](../language/types.md) — Every type explained in detail
- [Functions](../language/functions.md) — Complete function reference
- [Modules](../modules/overview.md) — All available modules
- [CLI Reference](../cli.md) — Every CLI command and flag

---

## Common Mistakes

### Semicolons

Zen does not use semicolons. Don't add them:

```
// WRONG
let x = 5;

// CORRECT
let x = 5
```

### Curly braces for blocks

Every `if`, `for`, `while`, and function body requires curly braces — even for single lines:

```
// WRONG — this won't work
if x > 0
    print x

// CORRECT
if x > 0 {
    print x
}
```

### String interpolation with expressions

Use `{name}` for simple variables, backticks with `${expr}` for expressions:

```
let x = 10

// WRONG — {x + x} doesn't work
// print "{x + x}"

// CORRECT
print `${x + x}`        // 20
```

### Forgetting `let` or `const`

```
// Bare assignment auto-declares, but explicit is clearer
let name = "Zen"

// This also works (auto-declares):
name = "Zen"
```

---

## Pro Tips

1. **Use the REPL for experimentation.** Run `zen shell` and try things interactively.
2. **Backticks are your friend.** `${expr}` works for complex expressions inside strings.
3. **`??` for defaults.** `config["key"] ?? "default"` returns `"default"` if the key is missing.
4. **Use `?.` for safe access.** `user?.address?.city` returns `null` instead of crashing.
5. **`for i in 0 -> 10` is inclusive.** Both 0 and 10 are included. Use `range(10)` for 0–9.
