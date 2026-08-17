# Types

A comprehensive reference for every data type in Zen, including coercion rules, gotchas, and real-world usage patterns.

## Type Overview

| Type | Zen Name | Examples | Description |
|------|----------|----------|-------------|
| Number | `number` / `int` / `float` | `42`, `3.14`, `-7` | All numbers are 64-bit floats internally |
| String | `string` | `"hello"`, `'world'` | Immutable text, supports interpolation |
| Boolean | `bool` | `true`, `false` | Lowercase only |
| Null | `null` | `null` | Absence of value |
| List | `list` | `[1, 2, 3]` | Ordered, mutable collection |
| Dict | `dict` | `{"a": 1}` | Key-value mapping, keys are strings |
| Function | `function` | `(x) => x * 2` | Callable |
| Object | `object` | `new Dog("Rex")` | Class instance |
| Socket | `socket` | — | Network socket |

---

## Numbers

All numbers in Zen are 64-bit floating-point (`f64`), but whole numbers display without a decimal point.

### Integer literals

```
let n = 42
let negative = -7
let zero = 0
print typeof n    // number
```

### Float literals

```
let pi = 3.14159
let e = 2.718
let small = 0.001
let neg = -3.14
```

### Number display rules

Whole numbers display without a decimal:

```
print 42          // 42 (not 42.0)
print 42.0        // 42
print 3.14        // 3.14
print -7.0        // -7
```

### Large and small numbers

```
print 1e10        // 10000000000
print 1e-6        // 0.000001
print 1e308       // 1e308 (near max f64)
print 1e-308      // 1e-308 (near min positive f64)
```

### Special values

```
print math.nan       // nan
print math.inf       // inf
print math.pi        // 3.141592653589793
print math.e         // 2.718281828459045
```

### Number methods

```
let n = 42

n.str()              // "42"
n.float()            // 42.0 (no-op, already a number)
n.int()              // 42 (truncates decimal)
n.bool()             // true
n.type               // "number"

(3.14).round()       // 3
(3.5).round()        // 4 (rounds away from zero)
(3.14159).round(2)   // 3.14 (round to 2 decimal places)
(3.14159).round(4)   // 3.1416

(3.9).trunc()        // 3
(-3.9).trunc()       // -3

(42).abs()           // 42
(-42).abs()          // 42
```

### Number gotchas

```
// All numbers are floats — no integer division
print 10 / 3     // 3.3333333333333335
print 10 / 2     // 5 (displays as whole number)

// Floating-point precision
print 0.1 + 0.2           // 0.30000000000000004
print (0.1 + 0.2) == 0.3  // false!

// Use round() for comparisons
print (0.1 + 0.2).round(10) == 0.3  // true
```

---

## Strings

### Quoting

Double quotes enable interpolation, single quotes do not:

```
let name = "World"
print "Hello, {name}!"        // Hello, World!

let literal = 'Hello, {name}!'
print literal                 // Hello, {name}!  (literal text, no interpolation)
```

### Triple-quoted strings

Preserve line breaks and indentation:

```
let html = """
<html>
  <body>
    <p>Hello</p>
  </body>
</html>
"""
print html
```

### Escape sequences

| Escape | Meaning | Example |
|--------|---------|---------|
| `\n` | Newline | `"line1\nline2"` |
| `\t` | Tab | `"col1\tcol2"` |
| `\r` | Carriage return | — |
| `\\` | Literal backslash | `"path\\to\\file"` |
| `\"` | Literal double quote | `"say \"hi\""` |
| `\'` | Literal single quote | `"it's"` in single-quoted |
| `\$` | Literal dollar sign | `"\$5"` |
| `\0` | Null character | — |

```
print "Hello\nWorld"
// Hello
// World

print "Name:\tAlice"
// Name:   Alice

print "Path: C:\\Users\\me"
// Path: C:\Users\me
```

### String interpolation (`{name}`)

Embed variables in double-quoted strings:

```
let name = "Alice"
let age = 30
print "My name is {name} and I'm {age}."
// My name is Alice and I'm 30.

// Works with member access
let user = {"name": "Bob"}
print "User: {user.name}"
// User: Bob

// Works with function calls
print "Length: {"hello".len}"
// Length: 5
```

**Limitation:** Only simple variable/member expressions work inside `{}`. No operators or complex expressions.

### Template literals (`` ` ` ``)

Backtick strings support `${expression}` for full expressions:

```
let x = 10
print `${x} + ${x} = ${x + x}`
// 10 + 10 = 20

let items = [1, 2, 3]
print `Count: ${items.len}, Sum: ${items[0] + items[1] + items[2]}`
// Count: 3, Sum: 6
```

### String methods

| Method | Description | Example |
|--------|-------------|---------|
| `.upper()` | Uppercase | `"hello".upper()` → `"HELLO"` |
| `.lower()` | Lowercase | `"HELLO".lower()` → `"hello"` |
| `.strip()` | Trim whitespace | `"  hi  ".strip()` → `"hi"` |
| `.split(sep)` | Split into list | `"a,b,c".split(",")` → `["a", "b", "c"]` |
| `.join(list)` | Join list | `"-".join(["a","b"])` → `"a-b"` |
| `.replace(old, new)` | Replace substring | `"hello".replace("l", "L")` → `"heLLo"` |
| `.startswith(s)` | Starts with? | `"hello".startswith("he")` → `true` |
| `.endswith(s)` | Ends with? | `"hello".endswith("lo")` → `true` |
| `.find(s)` | Find index | `"hello".find("ll")` → `2` |
| `.len` | Length | `"hello".len` → `5` |
| `.count` | Length (alias) | `"hello".count` → `5` |
| `.format(args)` | Positional format | `"hello {0}".format("world")` → `"hello world"` |
| `.str()` | Identity | `"hello".str()` → `"hello"` |
| `.int()` | Parse as int | `"42".int()` → `42` |
| `.float()` | Parse as float | `"3.14".float()` → `3.14` |
| `.bool()` | Parse as bool | `"true".bool()` → `true` |

```
let s = "Hello, World!"

print s.upper()              // HELLO, WORLD!
print s.lower()              // hello, world!
print s.replace("World", "Zen")  // Hello, Zen!
print s.split(", ")          // [Hello, World!]
print s.find("World")        // 7
print s.startswith("Hello")  // true
print s.endswith("!")        // true
print s.len                  // 13
```

---

## Booleans

### Literals

```
let t = true
let f = false
```

**Important:** Only lowercase `true` and `false` work. `True`, `TRUE`, `1` are not boolean literals.

### Truthiness

Every value has a truthiness:

| Falsy Values | Truthy Values |
|-------------|---------------|
| `false` | `true` |
| `null` | Any non-zero number |
| `0` | Any non-empty string |
| `0.0` | Any non-empty list |
| `""` (empty string) | Any non-empty dict |
| `[]` (empty list) | Functions |
| `{}` (empty dict) | Class instances |

```
print bool(0)          // false
print bool(1)          // true
print bool(-1)         // true (non-zero)
print bool("")         // false
print bool("hello")    // true
print bool([])         // false
print bool([1])        // true
print bool({})         // false
print bool({"a": 1})   // true
print bool(null)       // false
```

### Logical operators

```
print true and true      // true
print true or false      // true
print not true           // false

// JS-style aliases
print true && true       // true
print true || false      // true
print !true              // false
```

---

## Null

```
let x = null
print x                  // null
print typeof x           // null
print x == null          // true
print x === null         // true
```

### Null truthiness

```
print bool(null)         // false
print null ?? "default"  // "default"
```

### Nullish coalescing (`??`)

Returns the right side only when the left is `null`:

```
let config = {}
let host = config["host"] ?? "localhost"
print host               // localhost

let port = config["port"] ?? 8080
print port               // 8080

// Only null triggers the default
print 0 ?? "default"     // 0 (not null, kept)
print "" ?? "default"    // "" (not null, kept)
print false ?? "default" // false (not null, kept)
```

### Nullish assignment (`??=`)

Only assigns if the variable is null:

```
let x = null
x ??= "default"
print x                  // default

let y = "existing"
y ??= "other"
print y                  // existing (unchanged)
```

---

## Lists

### Creating lists

```
let empty = []
let nums = [1, 2, 3]
let mixed = [1, "two", true, 3.14, null]
let nested = [[1, 2], [3, 4], [5, 6]]
```

### Accessing elements

```
let items = [10, 20, 30]

print items[0]           // 10 (first)
print items[-1]          // 30 (last)
print items[-2]          // 20 (second from last)
```

### Modifying lists

```
let items = [1, 2, 3]
items[0] = 10            // [10, 2, 3]
items.append(4)          // [10, 2, 3, 4]
items.pop()              // returns 4, items is [10, 2, 3]
```

### List methods

| Method | Description |
|--------|-------------|
| `.append(x)` | Add to end |
| `.push(x)` | Alias for append |
| `.pop()` | Remove and return last item |
| `.shift()` | Remove and return first item |
| `.unshift(x)` | Add to front |
| `.insert(i, x)` | Insert at index |
| `.sort()` | Sort in-place |
| `.reverse()` | Reverse in-place |
| `.clear()` | Remove all items |
| `.len` | Number of items |
| `.count` | Alias for len |
| `.includes(x)` | Check if item exists |
| `.indexOf(x)` | Find index (-1 if not found) |
| `.join(sep)` | Join into string |
| `.sorted()` | Return new sorted list |
| `.map(fn)` | Transform each item |
| `.filter(fn)` | Keep items where fn returns truthy |
| `.reduce(fn)` | Fold to single value |

### List comprehensions

```
let squares = [x ** 2 for x in 1 -> 5]
// [1, 4, 9, 16, 25]

let evens = [x for x in 1 -> 10 if x % 2 == 0]
// [2, 4, 6, 8, 10]

let names = [p.name for p in people if p.age >= 18]
```

---

## Dicts

### Creating dicts

```
let empty = {}
let config = {"host": "localhost", "port": 8080}
let user = {name: "Alice", age: 30}  // bare keys
```

### Accessing values

```
let data = {"name": "Alice", "age": 30}

print data["name"]       // Alice
print data.name           // Alice (dot notation)
print data["missing"]    // null (no error)
```

### Modifying dicts

```
data["email"] = "alice@example.com"  // add
data["age"] = 31                     // update
```

### Dict methods

| Method | Description |
|--------|-------------|
| `.keys()` | List of key strings |
| `.values()` | List of values |
| `.items()` | List of [key, value] pairs |
| `.get(key)` | Get value or null |
| `.get(key, default)` | Get value or default |
| `.put(key, value)` | Set and return dict |
| `.has(key)` | Check if key exists |
| `.len` | Number of entries |
| `.count` | Alias for len |
| `.clear()` | Remove all entries |
| `.is_empty()` | `true` if no entries |

---

## Type Conversion

### Built-in conversion functions

```
// To string
print str(42)           // "42"
print str(3.14)         // "3.14"
print str(true)         // "true"
print str(null)         // "null"
print str([1, 2])       // "[1, 2]"

// To number (integer)
print int("42")         // 42
print int("3.9")        // 3 (truncates)
print int(true)         // 1
print int(false)        // 0
print int(null)         // 0

// To number (float)
print float("3.14")     // 3.14
print float("42")       // 42.0

// To boolean
print bool(1)           // true
print bool(0)           // false
print bool("yes")       // true
print bool("")          // false
print bool([])          // false
print bool(null)        // false
```

### The `.type` property

```
print (42).type         // "number"
print "hello".type      // "string"
print true.type         // "bool"
print null.type         // "null"
print [1, 2].type       // "list"
print {a: 1}.type       // "dict"
```

### The `typeof` operator

```
print typeof 42         // "number"
print typeof "hello"    // "string"
print typeof true       // "bool"
print typeof null       // "null"
print typeof [1, 2]     // "list"
print typeof {a: 1}     // "dict"
```

### The `type()` function

```
print type(42)          // "number"
print type("hello")     // "string"
print type([])          // "list"
```

---

## Type Coercion Rules

### Arithmetic operators

Numbers and strings combine differently:

```
// Number + Number → Number
print 1 + 2              // 3

// String + String → String (concatenation)
print "hello" + " " + "world"  // hello world

// Number + String → Error (use interpolation instead)
// print 1 + "2"          // ERROR

// Use string conversion
print str(1) + "2"       // 12
```

### Comparison operators

```
// Loose equality (==)
print 1 == 1.0           // true (same numeric value)
print "1" == 1           // false (different types)
print null == null        // true

// Strict equality (===)
print 1 === 1.0          // true
print "1" === 1          // false (different types)
print true === 1          // false (different types)

// Comparison operators
print 1 < 2              // true
print "a" < "b"          // true (lexicographic)
```

### Common type gotchas

```
// Empty string is falsy, but "0" is truthy
print bool("")           // false
print bool("0")          // true

// null vs 0 vs "" are all different
print null == 0          // false
print null == ""         // false
print 0 == ""            // false

// List identity vs equality
let a = [1, 2]
let b = [1, 2]
print a == b             // true (value equality)
print a === b            // false (different objects)
print a is b             // false (different objects)
```

---

## Universal Methods

Available on all types:

```
value.str()       // string representation
value.int()       // integer conversion
value.float()     // float conversion
value.bool()      // boolean conversion
value.type        // type name string
```

---

## Pro Tips

1. **Use `typeof` for runtime type checks.** `typeof x === "string"` is more reliable than `x.type`.
2. **Be careful with floating-point comparisons.** Use `(a - b).abs() < 0.0001` for approximate equality.
3. **`??` is safer than `||` for defaults.** `0 || "default"` returns `"default"`, but `0 ?? "default"` returns `0`.
4. **Use `str()` before concatenating numbers.** `"Count: " + str(count)` is clearer than `"Count: " + count`.
5. **Empty collections are falsy.** Use `list.len > 0` or `!list.is_empty()` for explicit checks.

---

## See Also

- [Variables](variables.md) — Variable declaration and scope
- [Operators](operators.md) — All operators and precedence
- [Strings](strings.md) — Deep dive into string operations
- [Collections](collections.md) — Lists and dicts in detail
