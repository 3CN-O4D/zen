# Types

## Basic Types

| Type | Examples | Description |
|------|----------|-------------|
| Number | `42`, `3.14`, `-7` | Integer or floating-point |
| String | `"hello"`, `'world'` | Double or single quotes |
| Boolean | `true`, `false` | Truth values (lowercase) |
| Null | `null` | Absence of value |
| List | `[1, 2, 3]` | Ordered collection |
| Dict | `{"a": 1, "b": 2}` | Key-value mapping |

## Strings

### Single and Double Quotes

```
let s1 = "hello"
let s2 = 'world'
```

### Triple-Quoted Strings

Preserve line breaks and indentation:

```
let msg = """Hello,
World!"""
// → "Hello,\nWorld!"
```

### Escape Sequences

| Escape | Meaning | Example |
|--------|---------|---------|
| `\n` | Newline | |
| `\t` | Tab | |
| `\r` | Carriage return | |
| `\\` | Literal backslash | |
| `\"` | Literal double quote | |
| `\'` | Literal single quote | |
| `\0` | Null character | |
| `\xNN` | Hex byte | `\x41` → `A` |
| `\uNNNN` | 16-bit Unicode | `\u0041` → `A` |
| `\UNNNNNNNN` | 32-bit Unicode | `\U0001F600` → 😀 |

### String Interpolation

Embed variables directly with `{name}`:

```
let name = "World"
print "Hello, {name}!"         // Hello, World!

let score = 95
print "Score: {score}/100"     // Score: 95/100
```

Only simple variable names are supported inside `{}` — no expressions.

### Template Literals

Use backticks for template literals with `${expression}` syntax:

```
let name = "World"
print `Hello ${name}!`           // Hello World!

let x = 10
print `${x} + ${x} = ${x + x}`  // 10 + 10 = 20
```

## Numbers

### Integers

```
let n = 42
let negative = -10
```

### Floats

```
let pi = 3.14
let e = 2.718
```

### Number Methods

```
let n = 42
n.str()          // "42"
n.float()        // 42.0
n.type           // "int"

(3.14).round()   // 3
(3.9).round()    // 4
(-3.5).round()   // -4
(3.14159).round(2)   // 3.14

(3.14).trunc()   // 3
(-3.14).trunc()  // -3
```

## Booleans

```
let t = true
let f = false
```

### Truthiness

**Falsy values**: `false`, `null`, `0`, `0.0`, `""` (empty string), `[]` (empty list).

Everything else is **truthy**.

## Null

```
let x = null
```

## Type Conversion

```
str(42)          // "42"
int("42")        // 42
float("3.14")    // 3.14
bool(1)          // true
bool(0)          // false
bool("")         // false
bool(null)       // false
type(42)         // "int"
```

## Universal Methods

Available on all types:

```
value.str()       // → string
value.int()       // → integer
value.float()     // → float
value.bool()      // → boolean
value.type        // → type name string
```

## typeof Operator

Get the type name as a string:

```
typeof 42         // "int"
typeof "hello"    // "string"
typeof [1, 2, 3]  // "list"
typeof {a: 1}     // "dict"
typeof null       // "null"
typeof true       // "bool"
typeof 3.14       // "float"
```
