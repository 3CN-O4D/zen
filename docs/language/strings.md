# Strings

Complete reference for string literals, interpolation, template literals, escape sequences, and all string methods in Zen.

## String Literals

### Double quotes

Support interpolation with `{name}`:

```
let name = "World"
print "Hello, {name}!"        // Hello, World!
```

### Single quotes

No interpolation — literal text only:

```
let name = "World"
print 'Hello, {name}!'        // Hello, {name}!  (literal text)
```

### Triple-quoted strings

Preserve line breaks and indentation:

```
let msg = """Hello,
World!"""
print msg
// Hello,
// World!
```

### Backtick strings (template literals)

Support `${expression}` for full expressions:

```
let x = 10
print `${x} + ${x} = ${x + x}`
// 10 + 10 = 20
```

---

## String Interpolation

### Simple variable interpolation (`{name}`)

Works in double-quoted strings only:

```
let name = "Alice"
let age = 30
print "My name is {name} and I'm {age}."
// My name is Alice and I'm 30.
```

### Member access in interpolation

```
let user = {"name": "Bob", "age": 25}
print "User: {user.name}, Age: {user.age}"
// User: Bob, Age: 25
```

### Function calls in interpolation

```
let greeting = "hello"
print "Uppercase: {greeting.upper()}"
// Uppercase: HELLO
```

### Limitation: No complex expressions

Only simple variables and member access work inside `{}`:

```
let x = 10

// WRONG — {x + x} doesn't work
// print "{x + x}"

// CORRECT — use template literals
print `${x + x}`        // 20
```

---

## Template Literals

Use backticks with `${expression}` for full expressions:

### Basic expressions

```
let x = 10
let y = 20

print `${x} + ${y} = ${x + y}`
// 10 + 20 = 30
```

### Method calls

```
let name = "alice"
print `${name.upper()}.jpg`
// ALICE.jpg
```

### List access

```
let items = [10, 20, 30]
print `First: ${items[0]}, Last: ${items[-1]}`
// First: 10, Last: 30
```

### Dict access

```
let config = {"host": "localhost", "port": 8080}
print `Server: ${config.host}:${config.port}`
// Server: localhost:8080
```

### Multi-line template literals

```
let name = "World"
let msg = `Hello
${name}!
Welcome to Zen.`
print msg
// Hello
// World!
// Welcome to Zen.
```

### Template literals vs interpolation

| Feature | `{name}` | `${expr}` |
|---------|----------|-----------|
| Syntax | `"text {var}"` | `` `text ${expr}` `` |
| Expressions | Simple names only | Full expressions |
| String type | Double-quoted | Backtick |
| Method calls | No | Yes |
| Arithmetic | No | Yes |
| Ternary | No | Yes |

```
let x = 5

// Interpolation — simple variables only
print "Value: {x}"             // Value: 5

// Template literal — full expressions
print `Value: ${x}`            // Value: 5
print `Double: ${x * 2}`       // Double: 10
print `Positive: ${x > 0 ? "yes" : "no"}`  // Positive: yes
```

---

## Escape Sequences

| Escape | Meaning | Example |
|--------|---------|---------|
| `\n` | Newline | `"line1\nline2"` |
| `\t` | Tab | `"col1\tcol2"` |
| `\r` | Carriage return | — |
| `\\` | Literal backslash | `"path\\to\\file"` |
| `\"` | Literal double quote | `"say \"hi\""` |
| `\'` | Literal single quote | `"it\'s"` |
| `\$` | Literal dollar sign | `"\$5"` |
| `\0` | Null character | — |

### Examples

```
print "Hello\nWorld"
// Hello
// World

print "Name:\tAlice"
// Name:   Alice

print "Path: C:\\Users\\me"
// Path: C:\Users\me

print "She said \"hello\""
// She said "hello"

print "Price: \$5.00"
// Price: $5.00
```

---

## String Methods

### Case methods

```
print "hello".upper()           // HELLO
print "HELLO".lower()           // hello
```

### Whitespace methods

```
print "  hello  ".strip()       // hello
print "  hello  ".lstrip()      // hello  (left only)
print "  hello  ".rstrip()      //   hello  (right only)
```

### Search methods

```
let s = "Hello, World!"

print s.find("World")           // 7 (index, or -1 if not found)
print s.find("xyz")             // -1
print s.startswith("Hello")     // true
print s.endswith("!")           // true
print s.includes("World")       // true
print s.includes("xyz")         // false
```

### Replace

```
print "hello world".replace("world", "zen")
// hello zen

print "aabbcc".replace("b", "X")
// aaXXcc

print "hello".replace("l", "L", 1)
// heLlo (replace only first occurrence)
```

### Split and join

```
// Split
print "a,b,c".split(",")       // [a, b, c]
print "hello world".split(" ")  // [hello, world]
print "one,two,,three".split(",")  // [one, two, , three]

// Join
print "-".join(["a", "b", "c"])   // a-b-c
print " ".join(["Hello", "World"])  // Hello World
print "".join(["a", "b", "c"])     // abc
```

### Format

```
print "hello {0}".format("world")        // hello world
print "{0} is {1}".format("Zen", "cool") // Zen is cool
```

### Length

```
print "hello".len     // 5
print "".len          // 0
print "hello".count   // 5 (alias for len)
```

### Type conversion

```
print "42".int()      // 42
print "3.14".float()  // 3.14
print "true".bool()   // true
print (42).str()      // "42"
```

---

## String Properties

### `.len` — length

```
print "hello".len       // 5
print "".len            // 0
print "hello world".len // 11
```

### `.count` — alias for len

```
print "hello".count     // 5
```

---

## String Concatenation

### Using `+` operator

```
let first = "Hello"
let second = "World"
print first + " " + second    // Hello World
```

### Building strings incrementally

```
let result = ""
for i in 1 -> 5 {
    result = result + str(i) + " "
}
print result    // 1 2 3 4 5
```

### Using join for efficiency

```
let parts = ["Hello", "World", "from", "Zen"]
print " ".join(parts)    // Hello World from Zen
```

---

## Number Methods (on numbers)

Numbers also have string-related methods:

```
let n = 42
print n.str()              // "42"
print n.float()            // 42.0

(3.14159).round(2)         // 3.14
(3.9).round()              // 4
(3.9).trunc()              // 3
(-3.9).trunc()             // -3
```

---

## Universal Methods

Available on all types:

```
value.str()       // → string representation
value.int()       // → integer (truncates)
value.float()     // → float
value.bool()      // → boolean
value.type        // → type name string
```

---

## Common String Operations

### Checking if a string is empty

```
let s = ""
print s.len == 0      // true
print s == ""         // true
print !s              // true (empty string is falsy)
```

### Extracting file extension

```
let filename = "document.pdf"
let ext = filename.split(".")[-1]
print ext             // pdf
```

### Capitalizing first letter

```
let name = "alice"
let capitalized = name[0].upper() + name[1:]
print capitalized     // Alice
```

### Checking if string contains only digits

```
let input = "12345"
print re.matches("^\\d+$", input)    // true
```

### Repeating a string

```
let dash = "-".repeat(20)
print dash    // --------------------
```

---

## Pro Tips

1. **Use `{name}` for simple variables.** It's the most readable interpolation.
2. **Use `` `${expr}` `` for complex expressions.** Full expression support.
3. **Use `join()` for building strings from lists.** More efficient than concatenation.
4. **Use `split()` + `[-1]` for file extensions.** Quick and clean.
5. **Use `strip()` before comparisons.** Avoids whitespace issues.
6. **Use `includes()` instead of `find() >= 0`.** More readable.

---

## Common Mistakes

### Interpolation in single quotes

```
// WRONG — single quotes don't interpolate
print 'Hello, {name}!'    // Hello, {name}!  (literal)

// CORRECT — use double quotes
print "Hello, {name}!"    // Hello, World!
```

### Complex expressions in `{}`

```
// WRONG — {x + x} doesn't work
// print "{x + x}"

// CORRECT — use backticks
print `${x + x}`
```

### String immutability

Strings are immutable — methods return new strings:

```
let s = "hello"
s.upper()               // returns "HELLO" but doesn't change s
print s                 // hello

// Must reassign
s = s.upper()
print s                 // HELLO
```

### Forgetting that `.len` is a property, not a method

```
// WRONG
// print "hello".len()

// CORRECT
print "hello".len       // 5
```

---

## See Also

- [Types](types.md) — String type overview
- [Operators](operators.md) — String concatenation and comparison
- [Collections](collections.md) — Split/join patterns
- [Modules: re](../modules/re.md) — Regular expressions on strings
