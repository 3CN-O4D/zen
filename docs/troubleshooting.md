# Troubleshooting

Common issues, error messages, and solutions for Zen development.

## Parse Errors

### `Expected <token> after <expression>`

Missing required syntax (closing paren, bracket, etc.).

```
// WRONG
let x = (1 + 2

// CORRECT
let x = (1 + 2)

// WRONG
let arr = [1, 2, 3

// CORRECT
let arr = [1, 2, 3]
```

### `Unexpected token`

Wrong syntax or misplaced character.

```
// WRONG — missing comma
let obj = {a: 1 b: 2}

// CORRECT
let obj = {a: 1, b: 2}

// WRONG — ternary without parentheses
print x > 5 ? "big" : "small"

// CORRECT
print (x > 5 ? "big" : "small")
```

### `Invalid assignment target`

Trying to assign to a non-assignable expression.

```
// WRONG
5 = x
(1 + 2) = x

// CORRECT
let x = 5
```

---

## Runtime Errors

### `Undefined variable 'x'`

Variable used before declaration.

```
// WRONG
print x
let x = 5

// CORRECT
let x = 5
print x
```

### `Index out of bounds`

Array index exceeds length.

```
let arr = [1, 2, 3]
print arr[5]    // ERROR: index out of bounds

// CORRECT — check length first
if arr.len > 5 {
    print arr[5]
}
```

### `Cannot call non-function`

Trying to call something that isn't a function.

```
let x = 5
x()    // ERROR: x is not a function

// CORRECT
let fn = function() { print "hello" }
fn()
```

### `Property 'x' on non-object`

Accessing property on wrong type.

```
let x = 5
print x.name    // ERROR: number has no properties

// CORRECT
let obj = {name: "hello"}
print obj.name
```

### `Division by zero`

```
let x = 10 / 0    // ERROR

// CORRECT — check denominator
if y != 0 {
    let result = x / y
}
```

### `Invalid hex escape`

Bad escape sequence in string.

```
// WRONG
let s = "\xGG"

// CORRECT — valid hex
let s = "\x41"    // A
```

---

## Module Errors

### `Module not found: xyz`

Module file doesn't exist.

```
// WRONG — file doesn't exist
fs.load_module("nonexistent")

// CORRECT — verify path
print fs.exists("nonexistent.z")    // false
```

### `Module failed to load`

Module has parse errors.

```
// Fix syntax errors in the module file
// Then reload:
let module = fs.load_module("fixed_module.z")
```

---

## Type Errors

### `Unsupported operand types`

Wrong types for operation.

```
// WRONG
let result = "5" + 3    // string + number

// CORRECT — convert types
let result = "5" + str(3)
let result = int("5") + 3
```

### `Cannot iterate over non-iterable`

Trying to iterate over wrong type.

```
// WRONG — number is not iterable
for x in 5 {
    print x
}

// CORRECT
for x in [1, 2, 3] {
    print x
}
```

---

## Network Errors

### `Connection refused`

Server not running or wrong port.

```
// Check if server is running
let result = http.get("http://localhost:8080")
print result.status

// Check port
net.port_available(8080)
```

### `TLS error`

SSL/TLS handshake failed.

```
// Use http:// for local dev servers
http.get("http://localhost:8080")

// Use https:// for production
http.get("https://example.com")
```

### `Timeout`

Request took too long.

```
// Default timeout is 30 seconds
http.get("https://slow-server.com")

// Can't set custom timeout in Zen currently
// Check network connectivity
```

---

## File Errors

### `File not found`

```
// WRONG — file doesn't exist
let content = fs.read("missing.txt")

// CORRECT — check existence first
if fs.exists("file.txt") {
    let content = fs.read("file.txt")
} else {
    print "File not found"
}
```

### `Permission denied`

```
// Check file permissions
!ls -la file.txt

// Make sure file is writable
!chmod 644 file.txt
```

### `Is a directory`

Trying to read/write a directory.

```
// WRONG
let content = fs.read("my_directory")

// CORRECT — read a file, not directory
let content = fs.read("my_directory/file.txt")
```

---

## Performance Issues

### Slow loops

```
// SLOW — repeated string concatenation
let result = ""
for i in 1 -> 10000 {
    result = result + str(i) + ", "
}

// FASTER — collect in list, join at end
let items = []
for i in 1 -> 10000 {
    items.append(str(i))
}
let result = items.join(", ")
```

### Memory issues with large files

```
// SLOW — loads entire file
let data = fs.read("huge.log")

// FASTER — use line-by-line reading
let f = fs.open("huge.log", "r")
while !f.eof() {
    let line = f.readline()
    // process line
}
f.close()
```

### Excessive object creation

```
// SLOW — creating objects in tight loop
for i in 1 -> 10000 {
    let obj = {"i": i}
}

// BETTER — reuse or flatten
let results = []
for i in 1 -> 10000 {
    results.append(i)
}
```

---

## Debugging Tips

### Print variable values

```
print "x = {x}"
print "type: {typeof(x)}"
print "length: {x.len}"
```

### Use assert for validation

```
let x = 5
assert(x == 5, "x should be 5")
```

### Add logging

```
function process(data) {
    print "DEBUG: process called with {data}"
    let result = transform(data)
    print "DEBUG: transform returned {result}"
    return result
}
```

### Check types

```
print typeof(value)    // "string", "number", "list", etc.
print value.type       // same as typeof()
```

---

## Pro Tips

1. **Start simple.** Get basic code working before adding complexity.
2. **Print early, print often.** Quick debugging without tools.
3. **Read error messages carefully.** They usually tell you exactly what's wrong.
4. **Check types.** Many errors are type mismatches.
5. **Use the shell for testing.** Try code snippets before putting them in scripts.

---

## Common Gotchas

### Variables are scoped to blocks

```
if true {
    let x = 5
}
print x    // ERROR: x not defined
```

### Closures capture by reference

```
let fns = []
for i in 1 -> 3 {
    fns.append(function() { print i })
}
fns[0]()    // prints 3, not 1!
fns[1]()    // prints 3
fns[2]()    // prints 3

// FIX — capture value
let fns = []
for i in 1 -> 3 {
    let val = i    // create new variable
    fns.append(function() { print val })
}
fns[0]()    // prints 1
```

### Method chaining order

```
// Chain order matters
let result = "Hello World"
    .to_lower()          // "hello world"
    .replace("world", "there")  // "hello there"
    .trim()
print result
```

### String immutability

```
// Strings are immutable
let s = "hello"
s[0] = "H"    // ERROR: can't assign to string index

// Create new string instead
let s = "hello"
s = "H" + s[1:]    // "Hello"
```

---

## See Also

- [CLI Reference](cli.md) — Command-line options and flags
- [Functions](language/functions.md) — Scope and closures
- [Error Handling](language/errors.md) — try/catch patterns
- [Getting Started](getting-started/installation.md) — Installation help
