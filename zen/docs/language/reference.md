# Zen Language Reference

Zen is a dynamic, interpreted scripting language with Python-inspired ergonomics
and JavaScript-style object and closure features. This document is the complete
reference for the native Rust runtime. Everything described here runs without a
Python interpreter.

## Programs, comments, whitespace

Zen is newline-delimited. Statements end at a newline; a trailing `;` is
optional and accepted. Blocks are delimited by `{` `}` braces.

```zen
// line comment
# also a comment
/* block comment */

let x = 1   // statement ends at newline
let y = 2;

{
    let z = 3
    print x + y + z   // 6
}
```

## Variables

Declare with `let` (mutable) or `const` (immutable).

```zen
let name = "Ada"
name = "Grace"          // ok: let is mutable

const PI = 3.14159
PI = 3.0                // error: cannot assign to constant: PI
```

> Built-in names (native functions and modules) are locked: `str = "x"` is an
> error. Declaring `let str = "x"` shadows the built-in and unlocks the name.

## Values and types

| Type     | Examples                            |
|----------|-------------------------------------|
| number   | `42`, `3.14`, `-7`, `1e6`, `0x1F`   |
| string   | `"hello"`, `'world'`                |
| bool     | `true`, `false`                     |
| null     | `null`                              |
| list     | `[1, 2, 3]`                         |
| dict     | `{ "name": "Ada", "age": 36 }`      |
| function | named functions, lambdas, natives   |
| object   | class instances                     |
| socket   | network socket                      |

```zen
type(42)          // "number"
type("hi")        // "string"
type([1])         // "list"
type({a: 1})      // "dict"
type(null)        // "null"
type(new Person()) // "object"
typeof x          // same, operator form
```

## Operators

Arithmetic:

```zen
1 + 2        // 3
5 - 3        // 2
4 * 3        // 12
10 / 4       // 2.5
10 % 3       // 1   modulo
2 ** 10      // 1024  power
```

Comparison and equality:

```zen
1 == 1        // true   loose equality
1 === "1"     // false  strict equality (type + value)
1 != 2        // true
1 !== "1"     // true   strict inequality
2 < 3         // true
3 <= 3        // true
4 > 2         // true
2 >= 2        // true
```

Logical (with short-circuiting):

```zen
true and false   // false
true or false    // true
not true         // false
null ?? "default"  // "default"  (nullish: right when left is null)
5 ?? "default"     // 5
```

Bitwise:

```zen
5 & 3        // 1
5 | 3        // 7
5 ^ 3        // 6
~5           // -6
1 << 4       // 16
16 >> 2      // 4
```

Assignment and compound assignment:

```zen
let n = 10
n += 5       // 15
n -= 2       // 13
n *= 2       // 26
n /= 2       // 13
n %= 3       // 1
n++          // post-increment
++n          // pre-increment
n--          // post-decrement
```

Index and member assignment:

```zen
let list = [1, 2, 3]
list[0] = 99          // [99, 2, 3]

let d = { a: 1 }
d.a = 2               // {a: 2}
d["b"] = 3            // {a: 2, b: 3}
```

Membership and `is`:

```zen
"ell" in "hello"      // true
2 in [1, 2, 3]        // true
"a" in {a: 1}         // true
[1, 2] is "list"      // type check
```

## Strings

```zen
let s = "hello world"
s.len()                   // 11  (also s.length())
s.upper()                 // "HELLO WORLD"
s.lower()                 // "hello world"
s.trim()                  // "hello world"
s.trimStart()             // "hello world "
s.trimEnd()               // " hello world"
s.split(" ")              // ["hello", "world"]
s.replace("world", "zen") // "hello zen"
s.contains("ell")         // true
s.startsWith("he")        // true
s.endsWith("ld")          // true
s.indexOf("o")            // 4  (or -1)
s.slice(0, 5)             // "hello"   (also substring/substr)
s.repeat(2)               // "hello worldhello world"
s.toList()                // ["h","e",...]
s.toUpperCase()           // same as upper()
```

## Lists

```zen
let xs = [1, 2, 3]
xs.push(4)                // [1, 2, 3, 4]
xs.pop()                  // 4
xs.first()                // 1
xs.last()                 // 3
xs.contains(2)            // true
xs.join("-")              // "1-2-3"
xs.reverse()              // [3, 2, 1]
xs.sort()                 // sorted copy
xs.skip(1)                // [2, 3]
xs.concat([4, 5])         // [1, 2, 3, 4, 5]
xs.sum()                  // 6
xs.unique()               // deduplicated copy
xs.shift()                // 1 (removes first)
xs.unshift(0)             // [0, 1, 2, 3]
xs.length()               // 3

// higher-order methods with lambdas
[1, 2, 3].map(fn (x) => x * 2)                // [2, 4, 6]
[1, 2, 3, 4].filter(fn (x) => x % 2 == 0)     // [2, 4]
[1, 2, 3].each(fn (x) => print x)             // prints 1 2 3
```

Negative indexing and ranges:

```zen
let xs = [10, 20, 30]
xs[-1]        // 30
xs[0..2]      // [10, 20]
2..5          // [2, 3, 4]
5..1          // [5, 4, 3, 2]
```

## Dictionaries

```zen
let d = { name: "Ada", age: 36 }
d.name                      // "Ada"
d["age"]                    // 36
d.age = 37                  // set
d.keys()                    // ["name", "age"]
d.values()                  // ["Ada", 37]
d.get("name")               // "Ada"
d.get("missing", "default") // "default"
d.has("age")                // true
d.set("city", "London")     // returns new dict with the entry
d.length()                  // 2
```

Keys are strings; bare identifiers are shorthand for string keys.

## Control flow

```zen
if x > 10 {
    print "big"
} else if x > 0 {
    print "positive"
} else {
    print "small"
}
```

```zen
let i = 0
while i < 5 {
    print i
    i += 1
}
```

`for ... in` iterates lists, strings, ranges, and dict keys:

```zen
for item in [1, 2, 3] { print item }
for ch in "abc" { print ch }
for i in 0..10 { print i }
for key in { a: 1, b: 2 } { print key }
```

`break` exits a loop, `continue` skips to the next iteration.

```zen
switch day {
    case "mon":
        print "start of week"
        break
    case "fri":
        print "almost there"
        break
    default:
        print "meh"
}
```

```zen
try {
    risky_call()
} catch (e) {
    print "caught: " + e
} finally {
    print "always runs"
}
```

`catch` and `except` are synonyms. A catch may name a type, bind the error
with `as` or `(var)`, both, or neither; typed catches also match subclasses:

```zen
try {
    risky_call()
} except ZeroDivisionError as e {
    print "division: " + e
} catch ArithmeticError as e {
    print "arithmetic: " + e
} catch as e {
    print "anything else: " + e
}
```

The built-in error types live in the `errors` module (`errors.Error`,
`errors.ValueError`, `errors.TypeError`, `errors.MathError`,
`errors.KeyboardInterrupt`, ...). Every error type is a class, so you can
construct it with `new` and build your own by subclassing:

```zen
class MoneyError extends errors.Error { }
throw new MoneyError("overdraft")
throw new errors.ValueError("bad input")
```

`throw` raises an error:

```zen
function check(n) {
    if n < 0 {
        throw new errors.ValueError("must be non-negative")
    }
    return n
}
```

Uncaught errors print a Python-style traceback with the file, line, column,
and the error's type and message.

## Functions

Named functions (`function` and `func` are synonyms):

```zen
function add(a, b) {
    return a + b
}
print add(2, 3)   // 5
```

Functions are first-class values:

```zen
let f = add
print f(1, 2)          // 3

function twice(f, x) {
    return f(f(x))
}
print twice(lambda n: n + 1, 5)   // 7
```

Lambdas use `lambda` (or `fn`), with either a colon body or a block body:

```zen
let square = lambda x: x * x
print square(4)          // 16

let add = lambda (a, b): a + b
print add(1, 2)          // 3

let clamp = lambda (v, lo, hi) {
    if v < lo { return lo }
    if v > hi { return hi }
    return v
}
```

Recursion:

```zen
function fib(n) {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}
print fib(10)   // 55
```

Spread in list and dict literals (not in call arguments):

```zen
let xs = [1, 2]
let merged = [0, ...xs, 3]     // [0, 1, 2, 3]
let o = { a: 1 }
let cfg = { ...o, b: 2 }       // {a: 1, b: 2}
```

## Classes and objects

Classes have a constructor (`init`), mutable fields (`self.x`), and methods.
Single inheritance is supported with `extends`.

```zen
class Animal {
    function init(name) {
        self.name = name
    }
    function speak() {
        return self.name + " makes a sound"
    }
}

class Dog extends Animal {
    function speak() {
        return self.name + " barks"
    }
}

let a = new Animal("cat")
print a.speak()       // cat makes a sound
print a.name          // cat
a.name = "big cat"

let d = new Dog("rex")
print d.speak()       // rex barks
```

Methods declared inside the class body are the only declarations allowed there.

## Modules

### Built-in module dicts

Modules are available as global dicts without any import: `json`, `fs`, `re`,
`random`, `math`, `time`, `os`, `base64`, `base32`, `crypto`, `cryptography`,
`datetime`, `uuid`, `color`, `csv`, `http`, `decimal`, `threading`,
`statistics`, and `browser`. Each is documented in its own page under
[Modules](../modules/README.md).

```zen
let data = json.parse('{"a": 1}')
print math.sqrt(16)          // 4
print fs.read("notes.txt")
print http.get("https://api.example.com/data").json()
```

### import / from / include

`import` and `from` load `.z` files by path or package name. `include` and
`load` are synonyms for `import`. `as` creates an alias.

```zen
import "greetings.z"
from "greetings" import greet
import "greetings" as g
```

The resolver checks, in order: an installed package, `./<name>`, `std/<name>`
(repo-relative), `<exe_dir>/std/<name>`, and `<exe_dir>/../std/<name>`.

### native declarations

`native function <name>(...)` binds a name to a built-in native routine. It is
used by the bundled std modules to expose native functions as Zen values.

```zen
native function fs_read(path)
let read_file = fs_read
```

## Destructuring

Lists and dicts can be unpacked:

```zen
const [C, D] = [1, 2]
const { E } = { E: 5 }
let [a, b] = [1, 2]
let { x, y } = { x: 1, y: 2 }
```

## Built-in globals

| Function | Description |
|----------|-------------|
| `print` | print a value, newline appended |
| `input` | read a line from stdin |
| `len(x)` | length of string/list/dict |
| `str(x)` | convert to string |
| `int(x)` | truncate to integer |
| `float(x)` | convert to float |
| `bool(x)` | convert to bool |
| `list(x)` | convert to list |
| `abs(x)`, `min(...)`, `max(...)` | numeric helpers |
| `round(x)` | round to nearest |
| `trunc(x)` | truncate |
| `hex(x)` | hex string |
| `range(end)` / `range(start, end)` | inclusive range list |
| `type(x)` / `typeof` | type name |
| `exit(code)` | terminate |
| `sleep(sec)`, `wait(ms)` | pause |

## std browser prelude

The bundled `std/browser.z` is loaded automatically as a prelude, so legacy
scripts that call `go`, `click`, `fill`, `text`, `attr`, `wait_for`, `shot`,
`title`, `url`, `page`, and `browser` work unchanged. See
[Browser automation](../modules/browser.md).