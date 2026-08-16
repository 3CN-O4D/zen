# Zen Tutorial: A Wordy, From-Zero Guide

This tutorial assumes you know nothing about Zen. It walks through every corner of
the language with many runnable examples, explaining *why* things work the way
they do. If you are in a hurry, the sibling [reference.md](./reference.md) is the
complete technical checklist; this page is the friendly, padded, learn-by-example
version.

You can follow along in three ways:

1. **Interactive REPL** — run `zen` with no arguments and type expressions.
2. **One-liners** — `zen -e 'print 1 + 1'` evaluates a single expression.
3. **Script files** — `zen hello.z` runs a whole program.

> Tip: everything here runs on the native Rust runtime. No Python, no extra
> interpreter, no downloads. If it is shown in a `zen` code block, you can paste
> it into the REPL or a `.z` file and run it.

---

## 1. What Zen programs look like

A Zen program is a sequence of *statements*. Each statement normally occupies its
own line. Statements end at the end of the line, so you do **not** need a
semicolon (though one is accepted if you like it). Blocks of code — bodies of
`if`, `while`, `for`, and `function` — are wrapped in curly braces `{` and `}`.

```zen
let greeting = "hello"        // one statement
let second = "world"          // another statement
print greeting, second        // prints: hello world
```

Comments help you (and everyone else) understand the code. Zen supports three
comment styles:

```zen
// a line comment: the rest of this line is ignored

# a hash comment works exactly the same

/*
   a block comment spans multiple lines,
   everything here is ignored too
*/
```

Blank lines and indentation are free. Zen does not care how much you indent — the
braces define structure, not whitespace:

```zen
{
let a = 1
        {
            let b = 2
            print a + b      // 3
        }
}
```

But for the sake of your future self, indent consistently anyway. Readers (and
you, three weeks from now) will thank you.

---

## 2. Variables and immutability

Declare a variable with `let` when you want it to change, and `const` when you
want to promise it never changes.

```zen
let mood = "happy"
mood = "sleepy"        // fine — let variables are mutable
print mood             // sleepy

const PI = 3.14159
PI = 3.0               // ERROR: cannot assign to constant: PI
```

Why have two kinds? Because a program is easier to reason about when things that
must not change are *declared* as not changing. The runtime enforces the promise
for you, so a mistake becomes an immediate, loud error instead of a silent bug
weeks later.

There is also a subtle rule about **built-in names**. Native functions and module
names are *locked*: writing `str = "x"` is an error. If you really need a
variable named `str`, you can declare it: `let str = "x"` shadows the built-in
and unlocks the name.

Variable names can contain letters, digits, and underscores, and must not start
with a digit. By convention, constants use UPPER_SNAKE and everything else uses
snake_case, but Zen does not enforce this.

---

## 3. Values and types

Everything in Zen is a value, and every value has a type. The core types are:

| Type     | Examples                               | What it is                      |
|----------|----------------------------------------|---------------------------------|
| number   | `42`, `3.14`, `-7`, `1e6`, `0x1F`      | integers and floats, one type   |
| string   | `"hello"`, `'world'`                   | text, double or single quotes   |
| bool     | `true`, `false`                        | truth values                    |
| null     | `null`                                 | "nothing here"                  |
| list     | `[1, 2, 3]`                            | ordered collection              |
| dict     | `{ "name": "Ada", "age": 36 }`         | key → value mapping             |
| function | `function add(a,b){...}`, `lambda x: x`| callable code                   |
| object   | `new Person()`                         | a class instance                |
| socket   | (created by `net`/`socket` modules)    | a network connection            |

Ask a value what it is with `type()` or the `typeof` operator — they are the same
thing:

```zen
type(42)          // "number"
type("hi")        // "string"
type([1, 2])      // "list"
type({a: 1})      // "dict"
type(null)        // "null"
type(print)       // "function"
type(new Person()) // "object"
```

Notice there is only **one** numeric type. `42` and `3.14` are both `number`.
Zen will happily mix them in arithmetic: `1 + 0.5` is `1.5`.

### Numbers in detail

Integers are the everyday numbers. You can write them in several bases:

```zen
42       // decimal (the default)
0x1F     // hexadecimal — 31
0b101    // binary — 5
0o17     // octal — 15
1e6      // scientific notation — 1000000
```

Floats carry a decimal point or exponent: `3.14`, `-0.5`, `2.5e3`.

### Strings in detail

Strings are text between two double quotes `"..."` or two single quotes
`'...'`. Either quote style works, which is handy when your text contains the
other kind:

```zen
let single = 'she said "hi"'      // fine
let double = "it's a good day"    // fine
```

A string has a rich set of methods for slicing and dicing it (see section 7).

### Booleans and null

`true` and `false` are the only booleans. `null` means "no value" — it is not the
same as `0`, not the same as `""`, and not the same as `false`. It is its own
type. In a boolean context (like an `if` condition), `null` counts as false, and
so do `0` and `""`. Everything else counts as true.

---

## 4. Operators: arithmetic, comparison, logic, bits

### Arithmetic

```zen
1 + 2        // 3        addition
5 - 3        // 2        subtraction
4 * 3        // 12       multiplication
10 / 4       // 2.5      division (always a float, never truncated)
10 % 3       // 1        modulo — remainder after division
2 ** 10      // 1024     power/exponent
```

Division is worth a special callout: `10 / 4` is `2.5`, not `2`. Zen never
silently chops off the decimal. If you want integer truncation you must ask for
it explicitly with `int(10 / 4)`.

### Comparison and equality

Zen has two flavors of equality — **loose** and **strict** — mirroring
JavaScript:

```zen
1 == 1          // true     loose: are the values equal?
1 == "1"        // false    a number is never equal to a string
1 === "1"       // false    strict: type AND value must match
1 === 1         // true
1 != "1"        // true     loose inequality
1 !== "1"       // true     strict inequality
```

Ordering comparisons work on numbers, and also on strings (alphabetically):

```zen
2 < 3          // true
3 <= 3         // true
4 > 2          // true
2 >= 2         // true
"apple" < "banana"   // true — alphabetical order
```

### Logical operators

Zen uses English words for logic (`and`, `or`, `not`), which reads more like a
sentence than `&&`, `||`, `!`. They short-circuit: Zen stops evaluating as soon
as the answer is known, which is important when the right side has side effects.

```zen
true and false      // false
true or false       // true
not true            // false
```

The **nullish coalescing** operator `??` returns the right side only when the
left side is `null` — it is a "give me a fallback" operator:

```zen
null ?? "default"   // "default"   — null, so we fall back
5 ?? "default"      // 5           — not null, keep the 5
0 ?? "default"      // 0           — 0 is NOT null, keep it!
```

Note the subtlety: `0 ?? "default"` is `0`. The `??` operator cares only about
`null`, not about falsiness.

### Bitwise operators

These operate on the individual bits of integers. Useful for flags and packing
small values together:

```zen
5 & 3        // 1        AND
5 | 3        // 7        OR
5 ^ 3        // 6        XOR
~5           // -6       NOT (flips every bit)
1 << 4       // 16       shift left by 4
16 >> 2      // 4        shift right by 2
```

### Assignment and compound assignment

`=` stores a value. The compound operators do an operation *and* an assignment in
one step, and there are handy increment/decrement shortcuts:

```zen
let n = 10
n += 5       // 15        n = n + 5
n -= 2       // 13        n = n - 2
n *= 2       // 26
n /= 2       // 13
n %= 3       // 1
n++          // 1 then becomes 2 (post-increment: the old value is returned)
++n          // 2 then becomes 3 (pre-increment: the new value is returned)
n--          // post-decrement
```

The difference between `n++` and `++n` only matters when you use the result:

```zen
let a = 1
let b = a++         // b = 1, then a becomes 2
let c = ++a         // a becomes 3, then c = 3
```

### Assignment into containers

You can write through an index or a key:

```zen
let list = [1, 2, 3]
list[0] = 99        // now [99, 2, 3]

let d = { a: 1 }
d.a = 2             // dot form
d["b"] = 3          // bracket form — now {a: 2, b: 3}
```

### Membership and type checks

The `in` operator asks "does this container hold this thing?", and `is` checks a
value's type:

```zen
"ell" in "hello"       // true    substring
2 in [1, 2, 3]         // true    list member
"a" in {a: 1}          // true    dict key
[1, 2] is "list"       // true    type check
"abc" is "string"      // true
```

---

## 5. Control flow: making decisions and looping

### if / else if / else

The bread and butter of decisions. Conditions do not need parentheses.

```zen
let score = 85

if score >= 90 {
    print "A"
} else if score >= 80 {
    print "B"
} else if score >= 70 {
    print "C"
} else {
    print "keep studying"
}
```

Any value can serve as a condition; `0`, `""`, `null`, and `false` are false,
everything else is true:

```zen
let name = ""
if name {
    print "name is non-empty"
} else {
    print "name is empty"   // this runs
}
```

### while

Loop as long as a condition stays true. Remember to make progress toward false or
you will loop forever (which is sometimes exactly what you want, e.g. a server):

```zen
let i = 0
while i < 5 {
    print i
    i += 1
}
// prints 0 1 2 3 4

while true {
    let line = input("> ")
    if line == "quit" { break }      // exit the loop
    print "you typed: " + line
}
```

### for ... in

`for ... in` walks over *anything iterable*: lists, strings (character by
character), ranges, and dicts (key by key). This is usually more convenient than
a manual `while`:

```zen
for item in [1, 2, 3] { print item }        // 1 2 3
for ch in "abc" { print ch }                // a b c
for i in 0..10 { print i }                  // 0..9 — see ranges below
for key in { a: 1, b: 2 } { print key }     // a b
```

### Ranges

A range is a compact way to build a list of consecutive numbers. `start..end` is
**inclusive** at both ends:

```zen
2..5          // [2, 3, 4]
5..1          // [5, 4, 3, 2]   descending works too
0..3          // [0, 1, 2, 3]   note: includes BOTH endpoints
```

Because ranges are inclusive, `0..3` has four elements. Many languages use
half-open ranges; Zen deliberately uses inclusive ones, so `0..9` is the ten
digits.

You can also slice a list with a range:

```zen
let xs = [10, 20, 30, 40]
xs[0..2]      // [10, 20, 30]   indices 0, 1, and 2
```

### break and continue

`break` abandons the loop entirely; `continue` skips just the current iteration:

```zen
for i in 0..9 {
    if i == 2 { continue }     // skip 2, keep looping
    if i == 5 { break }        // stop completely at 5
    print i
}
// prints: 0 1 3 4
```

### switch

When you are comparing one value against many possible values, `switch` is
cleaner than a long chain of `else if`:

```zen
let day = "fri"
switch day {
    case "mon":
        print "start of the week"
        break
    case "fri":
        print "almost the weekend"     // this one runs
        break
    default:
        print "a middle day"
}
```

### try / catch / finally (and the `except` synonym)

Errors can be *caught* so a failing operation does not crash your whole program.
The keywords `catch` and `except` are interchangeable — use whichever reads
better to you:

```zen
try {
    let data = json.parse("{not valid json}")
    print data
} catch (e) {
    print "parse failed: " + e
} finally {
    print "we always reach this line"
}
```

`catch (e)` binds the error to the variable `e`. The `finally` block runs
whether or not an error occurred — perfect for cleanup. There are several
shorthand bindings:

```zen
try {
    risky()
} catch e {            // same as `catch (e)`
    print "plain name binding: " + e
} catch as e {         // `as` is the Python-style spelling
    print "as binding: " + e
} catch {              // bare catch, no variable at all
    print "something went wrong"
}
```

### Catching specific error types

A catch can name an error *type*. Only errors of that type (or a child of it)
are caught — anything else keeps propagating up:

```zen
try {
    let result = 1 / 0
} catch ZeroDivisionError as e {
    print "cannot divide by zero: " + e
} catch ArithmeticError as e {
    print "some arithmetic problem: " + e
} catch as e {
    print "anything else: " + e
}
```

You can chain as many typed `catch` / `except` clauses as you need. Each one
may carry a type, a binding, both, or neither. When you catch a specific type,
its *children* are caught too — catching `errors.Error` catches everything.

### The `errors` module

All built-in error types live in the `errors` module (Python's `exceptions`
module, essentially). Refer to them as `errors.TypeError`, `errors.ValueError`,
and so on — no `import` is required, but `import errors` works too:

```zen
import errors
print errors.ValueError     // "ValueError"
print errors.ZeroDivisionError
```

The built-in hierarchy looks like this:

```
errors.Error
├── errors.TypeError
├── errors.ValueError
│   └── errors.RangeError
├── errors.NameError
├── errors.LookupError
│   ├── errors.KeyError
│   └── errors.IndexError
├── errors.ArithmeticError
│   ├── errors.MathError
│   ├── errors.NumberError
│   ├── errors.ZeroDivisionError
│   └── errors.OverflowError
├── errors.IOError
│   └── errors.FileNotFoundError
├── errors.ImportError
├── errors.KeyboardInterrupt
├── errors.RuntimeError
│   └── errors.NotImplementedError
├── errors.StopIteration
├── errors.RecursionError
├── errors.AssertionError
└── errors.SystemExit
```

Every error type can be constructed with `new` and thrown with `throw`:

```zen
throw new errors.ValueError("bad value given")
throw new KeyboardInterrupt()   // message is optional
```

### Custom errors (subclassing the `errors` module)

Define your own error by making it a child of `errors.Error` (or any other
error class) with `extends` — this is Zen's version of Python's
`class MyError(Exception): pass`:

```zen
class MoneyError extends errors.Error {
}
class InsufficientFundsError extends MoneyError {
}

function withdraw(balance, amount) {
    if amount > balance {
        throw new InsufficientFundsError("balance too low")
    }
    return balance - amount
}

try {
    withdraw(10, 50)
} catch InsufficientFundsError as e {
    print "caught exact type: " + e
} catch MoneyError as e {
    print "caught a parent: " + e
} catch errors.Error as e {
    print "caught a grandparent: " + e
}
```

Because `InsufficientFundsError` inherits from `MoneyError`, the first catch
matches. Delete it and the second does. Delete them both and the third —
`errors.Error` is the root, so it catches any error. A custom class needs no
body: the inherited `init(message)` stores the message, which is what `catch
X as e` binds to `e`.

### throw

Raise an error of your own to signal a problem to the caller:

```zen
function setAge(person, n) {
    if n < 0 {
        throw new errors.ValueError("age cannot be negative")
    }
    person.age = n
}

try {
    setAge(me, -5)
} except ValueError as e {
    print "rejected: " + e
}
```

`throw` accepts any value — a string, a dict, an instance of an error class.
Uncaught errors print a Python-style traceback pointing at the exact file,
line, and column:

```
zen: Traceback (most recent call last):
  File "app.z", line 3, in <module>
    throw new errors.ValueError("oops")
                        ^
ValueError: oops
```

---

## 6. Functions: reusable blocks of behavior

### Named functions

Define a function with `function` (or its synonym `func`). Call it by name with
parentheses. A function returns the value of the `return` statement, or `null`
if it falls off the end:

```zen
function add(a, b) {
    return a + b
}

print add(2, 3)        // 5
print add("zen", "!")  // "zen!" — + concatenates strings too
```

### Functions are first-class values

You can store a function in a variable, pass it to another function, and call it
later. This is the engine behind callbacks and higher-order programming:

```zen
let f = add            // now f is add
print f(1, 2)          // 3

function twice(f, x) {
    return f(f(x))     // call f on x, then call f again on the result
}
print twice(lambda n: n + 1, 5)     // 7  (6 then 7)
```

### Lambdas

A *lambda* is a function with no name, written on the fly. Two forms exist. The
arrow/colon form is an expression returning a value:

```zen
let square = lambda x: x * x
print square(4)          // 16

let add = lambda (a, b): a + b
print add(1, 2)          // 3
```

The block form has a full body and can contain multiple statements:

```zen
let clamp = lambda (v, lo, hi) {
    if v < lo { return lo }
    if v > hi { return hi }
    return v
}
print clamp(5, 0, 3)     // 3
```

`fn` is a synonym for `lambda`, so `[1,2,3].map(fn (x) => x * 2)` works and is
common in list code (see section 7).

### Recursion

A function may call itself — that is recursion, and it is the natural way to
express many problems:

```zen
function factorial(n) {
    if n <= 1 { return 1 }
    return n * factorial(n - 1)
}
print factorial(5)       // 120

function fib(n) {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}
print fib(10)            // 55
```

### Spread

You can "spread" a list or dict into a new literal, which is great for copying
and merging:

```zen
let xs = [1, 2]
let merged = [0, ...xs, 3]     // [0, 1, 2, 3]

let o = { a: 1 }
let cfg = { ...o, b: 2 }       // {a: 1, b: 2}
```

(Spread works in list and dict *literals*. It is not available inside call
argument lists.)

---

## 7. The standard containers and their methods

### Strings: the method toolbox

```zen
let s = "  Hello, World!  "

s.len()                   // 19     also s.length()
s.upper()                 // "  HELLO, WORLD!  "
s.lower()                 // "  hello, world!  "
s.trim()                  // "Hello, World!"   chops outer whitespace
s.trimStart()             // "Hello, World!  "
s.trimEnd()               // "  Hello, World!"
s.split(", ")             // ["  Hello", "World!  "]
s.replace("World", "Zen") // "  Hello, Zen!  "
s.contains("ell")         // true
s.startsWith("  He")      // true
s.endsWith("!  ")         // true
s.indexOf("World")        // 9     position, or -1 when absent
s.slice(2, 7)             // "Hello"   from index 2 up to and including 7
s.repeat(2)               // "  Hello, World!    Hello, World!  "
"abc".toList()            // ["a", "b", "c"]
```

`slice`, `substring`, and `substr` are synonyms. Strings are indexed starting at
0, so `"Hello"[0]` is `"H"`.

### Lists: the workhorse container

```zen
let xs = [1, 2, 3]

xs.push(4)                // append → [1, 2, 3, 4]
xs.pop()                  // remove & return last → 4
xs.shift()                // remove & return first → 1
xs.unshift(0)             // insert at front → [0, 2, 3]
xs.first()                // 0
xs.last()                 // 3
xs.contains(2)            // true
xs.join("-")              // "0-2-3"
xs.reverse()              // [3, 2, 0]   (a new reversed copy)
xs.sort()                 // [0, 2, 3]   (a new sorted copy)
xs.concat([4, 5])         // [0, 2, 3, 4, 5]
xs.sum()                  // 5
xs.unique()               // [0, 2, 3]   deduplicated copy
xs.length()               // 3
xs.skip(1)                // [2, 3]      all but the first element
```

**Negative indexing** is the killer feature: `xs[-1]` is the last element,
`xs[-2]` the second-to-last. No more `xs[len(xs)-1]`.

**Higher-order methods** take a lambda and transform the list functionally:

```zen
[1, 2, 3].map(fn (x) => x * 2)              // [2, 4, 6]
[1, 2, 3, 4].filter(fn (x) => x % 2 == 0)   // [2, 4]
[1, 2, 3].each(fn (x) => print x)           // prints 1 2 3 (side effects)

let total = [1, 2, 3].map(fn (x) => x * x).sum()   // 1 + 4 + 9 = 14
```

### Dicts: key → value

```zen
let d = { name: "Ada", age: 36 }

d.name                 // "Ada"     dot access
d["age"]               // 36        bracket access
d.city = "London"      // add a new key
d.keys()               // ["name", "age", "city"]
d.values()             // ["Ada", 36, "London"]
d.get("name")          // "Ada"
d.get("missing", "fallback")   // "fallback"   safe default
d.has("age")           // true
d.length()             // 3
```

Dict keys are always strings. The shorthand `{ name: "Ada" }` is sugar for
`{ "name": "Ada" }`. `d.set(k, v)` returns a *new* dict with the entry added, so
it composes nicely:

```zen
let a = { x: 1 }
let b = a.set("y", 2)   // a is untouched: {x: 1}; b is {x: 1, y: 2}
```

Iterate a dict with `for key in d` (you get the keys), then look up with `d[key]`.

---

## 8. Classes and objects

### A simple class

A class bundles data (fields) with behavior (methods). The `init` method is the
constructor, called when you `new` an instance. Fields are attached to `self`:

```zen
class Animal {
    function init(name) {
        self.name = name
    }

    function speak() {
        return self.name + " makes a sound"
    }
}

let cat = new Animal("whiskers")
print cat.speak()        // whiskers makes a sound
print cat.name           // whiskers
cat.name = "big whiskers"   // fields are mutable
print cat.name           // big whiskers
```

### Inheritance

A class can `extends` another, inheriting its fields and methods, and override
any method it likes:

```zen
class Dog extends Animal {
    function speak() {
        return self.name + " barks"
    }
}

let rex = new Dog("rex")
print rex.speak()        // rex barks   (overridden)
```

The base `Animal` constructor is used unless `Dog` defines its own `init`.

### Rules of classes

- Only method declarations (`function ...`) are allowed inside a class body.
- The constructor is always named `init`.
- An instance is created with `new ClassName(args)`.
- `type(instance)` returns `"object"`.
- Inheritance is single: a class may have at most one parent.

---

## 9. Destructuring: unpacking in style

Instead of reaching into a list or dict one index at a time, unpack it in one
declaration:

```zen
const [C, D] = [1, 2]        // C = 1, D = 2
let [a, b] = [1, 2]          // a = 1, b = 2

const { E } = { E: 5 }       // E = 5
let { x, y } = { x: 1, y: 2 }// x = 1, y = 2
```

This is especially handy when a function returns a small list or dict and you
want its pieces as named variables right away.

---

## 10. Modules and imports

### Built-in module dicts

Zen ships with a large standard library available as ready-made global dicts —
no import needed. Just use them:

```zen
let data = json.parse('{"a": 1}')          // JSON
print math.sqrt(16)                        // 4
print fs.read("notes.txt")                 // file contents
print http.get("https://api.example.com").json()   // HTTP request
print random.randint(1, 6)                 // a die roll
print time.now()                           // epoch seconds
print crypto.sha256("secret")              // hex digest
print uuid.uuid4()                         // a fresh UUID
```

The full list of modules: `json`, `fs`, `re`, `random`, `math`, `time`, `os`,
`base64`, `base32`, `crypto`, `cryptography`, `datetime`, `uuid`, `color`,
`csv`, `http`, `decimal`, `threading`, `statistics`, and `browser`. Each has its
own doc page under `docs/` (e.g. `modules-fs.md`, `modules-data.md`,
`modules-system.md`, `modules-crypto.md`, `modules-http.md`, `browser.md`).

### Importing your own files

`import`, `include`, and `load` are synonyms that pull in another `.z` file.
`from ... import` grabs a specific name, and `as` renames:

```zen
import "greetings.z"          // run everything in greetings.z
from "greetings" import greet // bring in just `greet`
import "greetings" as g       // now call g.greet()
```

The loader looks for the file in this order: an installed package, `./<name>`,
`std/<name>`, the executable's bundled `std/`, and its sibling `std/`.

### native declarations

`native function name(...)` binds a Zen name to a built-in Rust routine. The
bundled std modules use this to expose natives as ordinary Zen values. You will
rarely write this yourself, but you can:

```zen
native function fs_read(path)
let contents = fs_read("data.txt")
```

---

## 11. Built-in globals cheat sheet

| Function | What it does |
|----------|--------------|
| `print` | write a value to the console, newline appended |
| `input` | read a line of text from the keyboard |
| `len(x)` | length of a string, list, or dict |
| `str(x)` | convert a value to its string form |
| `int(x)` | truncate to an integer (`int(3.9)` is `3`) |
| `float(x)` | convert to a float |
| `bool(x)` | convert to a boolean |
| `list(x)` | convert an iterable to a list |
| `abs(x)` | absolute value |
| `min(...)`, `max(...)` | smallest/largest of the arguments |
| `round(x)` | round to nearest integer |
| `trunc(x)` | drop the fractional part |
| `hex(x)` | format a number as hexadecimal text |
| `range(end)` / `range(start, end)` | build an inclusive range list |
| `type(x)` / `typeof x` | the type name as a string |
| `exit(code)` | end the program immediately |
| `sleep(sec)`, `wait(ms)` | pause the program |

---

## 12. Putting it all together

A slightly bigger program that uses variables, functions, lists, dicts, control
flow, and a module:

```zen
// A tiny grade-book program.
const PASS_MARK = 50

class Student {
    function init(name, score) {
        self.name = name
        self.score = score
    }
    function passed() {
        return self.score >= PASS_MARK
    }
}

let students = [
    new Student("ada", 91),
    new Student("grace", 47),
    new Student("alan", 73),
]

print "Results:"
for s in students {
    if s.passed() {
        print "  PASS " + s.name + " (" + s.score + ")"
    } else {
        print "  FAIL " + s.name + " (" + s.score + ")"
    }
}

let scores = students.map(fn (s) => s.score)
print "Average: " + (scores.sum() / scores.length())
```

Save this as `grades.z` and run it:

```bash
zen grades.z
```

Expected output:

```
Results:
  PASS ada (91)
  FAIL grace (47)
  PASS alan (73)
Average: 70.33333333333333
```

---

## Where to go next

- [reference.md](./reference.md) — the complete, compact language reference.
- [../modules-fs.md](../modules-fs.md) — file and directory operations.
- [../modules-data.md](../modules-data.md) — json, csv, re, random, base64, uuid.
- [../modules-system.md](../modules-system.md) — os, time, datetime, math.
- [../modules-crypto.md](../modules-crypto.md) — hashing and encryption.
- [../modules-http.md](../modules-http.md) — HTTP clients and sockets.
- [../browser.md](../browser.md) — browser automation over CDP.
- [../cli.md](../cli.md) — `zen run`, `zen check`, `zen repl`, `zen pm`.
