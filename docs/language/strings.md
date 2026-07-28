# Strings

## String Methods

```
"hello".upper()                     // "HELLO"
"HELLO".lower()                     // "hello"
"  zen  ".strip()                   // "zen"
"a,b,c".split(",")                  // ["a", "b", "c"]
"-".join(["a", "b", "c"])           // "a-b-c"
"hello world".replace("world", "zen")
"hello".startswith("he")            // true
"hello".endswith("lo")              // true
"hello".find("ll")                  // 2 (index)
"hello".len                         // 5
"hello".count                       // 5 (alias for len)
"hello {0}".format("world")         // "hello world"
```

## String Interpolation

Embed variables directly in strings with `{name}`:

```
let name = "World"
print "Hello, {name}!"         // Hello, World!

let x = 10
print "{x} + {x} = {x + x}"    // ERROR: only simple names, not expressions

let score = 95
print "Score: {score}/100"     // Score: 95/100
```

Only simple variable names are supported inside `{}` — no expressions.

## Template Literals (JS-style)

Use backticks for template literals with `${expression}` syntax. Unlike `{name}`, `${}` supports full expressions:

```
let name = "World"
print `Hello ${name}!`           // Hello World!

let x = 10
print `${x} + ${x} = ${x + x}`  // 10 + 10 = 20

let items = [1, 2, 3]
print `Count: ${items.len}`      // Count: 3
```

## Escape Sequences

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

## Number Methods

```
let n = 42
n.str()          // "42"
n.float()        // 42.0
n.type           // "int"

3.times(function(i) {
    print i
})
// 0, 1, 2

(3.14).round()   // 3
(3.9).round()    // 4
(-3.5).round()   // -4
(3.14159).round(2)   // 3.14
(3.14159).round(4)   // 3.1416

(3.14).trunc()   // 3
(-3.14).trunc()  // -3
(3.9).trunc()    // 3

(42.0).int()     // 42
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

Additionally, `.len` and `.count` are available on strings, lists, dicts, and ZenLists (element lists) as property accesses.
