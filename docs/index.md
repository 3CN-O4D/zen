# Zen

**A fast, pure native scripting language with an expressive, Python-flavoured
syntax — plus batteries-included modules for HTTP, files, regex, JSON,
threading, sockets, crypto, and CDP-driven browser automation.**

Zen is implemented from scratch in Rust (`src/runtime.rs` is a single
self-contained interpreter — parser, bytecode-free tree-walking VM, and the
standard modules). There is **no Python, no DrissionPage, no Chrome-less magic**:
the browser module talks to Chromium directly over the Chrome DevTools Protocol.

## Hello, Zen

```zen
print "Hello, world!"

var name = "Ada"
print "Welcome, ${name}!"          # string interpolation

fn greet(who) {
    return "Hi, " + who
}
print greet("Bob")
```

Run it:

```bash
$ zen hello.z
Hello, world!
Welcome, Ada!
Hi, Bob
```

## Why Zen?

- **Zero-dependency runtime.** One `cargo build --release` gives you a single
  self-contained `zen` binary with the language *and* ~60 modules.
- **Scripting that reads like the problem, not the host language.** Python-like
  `if`/`elif`/`for`/`while`, a real `switch`/`match`, list comprehensions, and
  clean functional list methods.
- **Typed-feeling values without the ceremony.** `int`, `string`, `bool`,
  `list`, `dict`, plus class instances.
- **Practical batteries.** `fs`, `http`, `json`, `re`, `os`, `time`, `random`,
  `math`, `crypto`, `base64`, `hashlib`, `socket`, `threading`, `csv`,
  `datetime`, `uuid`, `pathlib`, `shutil`, `glob`, `itertools`,
  `collections`, and more.
- **Browser automation as a module, not a fork of the language.** Drive
  Chromium via CDP with `browser.launch()`, `browser.go()` and friends — the
  core language stays pure.

## A quick feature tour

### Variables & scope

```zen
let x = 1              # let + var are the same (rebindable)
const y = 2            # constants are enforced
var a, b = 1, 2        # multi-assignment
global count = 0       # module-global, visible inside functions
```

### Control flow

```zen
var n = 3

if n > 5 { print "big" } elif n > 2 { print "medium" } else { print "small" }

switch n {
    case 1:  print "one"
    case 2:  print "two"
    default: print "other"
}

print match n { 1: "one", _: "other" }      # expressions!

for i in 0..3 { print i }                    # 0 1 2  (exclusive ..)
for x in [10, 20, 30] { print x }
while n > 0 { n = n - 1 }
```

### Functions

```zen
fn add(a, b) { return a + b }                # also: func add, def add
var square = lambda(x): x * x                # lambda
var triple = (x) => x * 3                    # arrow (params must be parenthesized)
fn greet(name = "world") { print "hello " + name }
```

### Lists & dicts

```zen
var nums = [3, 1, 2]
var sorted = nums.sorted()                   # [1, 2, 3] (functional, not in-place)
var evens = nums.filter((x) => x % 2 == 0)
print nums.push(4)                           # new list [3, 1, 2, 4]

var config = { host: "localhost", port: 8080 }
print config.host                            # localhost  (dot access)
print config.get("port", 0)                  # 8080
```

### Strings

```zen
var s = "hello world"
print s.upper()                              # HELLO WORLD
print s.split(" ")                           # [hello, world]
print s.replace(" ", "-")                    # hello-world
print "v1 = ${1 + 1}"                        # v1 = 2
```

### Browser automation

```zen
browser.launch()
browser.go("https://example.com")
print browser.title()
browser.shot("/tmp/s.png")
browser.quit()
```

## Where to go next

- [Installation & CLI](cli.md) — running scripts, the REPL, and the package manager
- [Keyword reference](keywords.md) — every reserved word, and what it does
- [Language reference](grammar.md) — the full EBNF, generated from the runtime
- [Variables](variables.md) · [Operators](operators.md) · [Control flow](control-flow.md)
- [Functions](functions.md) · [Lists](lists.md) · [Strings](strings.md) · [Dicts](dicts.md)
- [Classes](classes.md) · [Errors](errors.md) · [Imports](imports.md)
- [Built-ins](builtins.md) — the global functions
- [Modules](modules/overview.md) — the standard library
- [Browser automation](browser/overview.md) — driving Chromium via CDP
- [Troubleshooting](troubleshooting.md) — common errors and how to fix them

## Conventions used in this documentation

- Code blocks tagged ```` ```zen ```` are runnable examples. Blocks tagged
  ```` ``` ```` are outputs or terminal sessions.
- `#` comments in examples are `Zen` comments (both `#` and `//` work).
- Truth in the docs is **the native Rust runtime**. If the installed
  `/usr/bin/zen` (2.1.0) misbehaves, rebuild with `cargo build --release`
  inside the repo — the source is newer than the shipped binary.