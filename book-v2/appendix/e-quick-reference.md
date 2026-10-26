# Appendix E: Quick Reference Card

## E.1 Syntax at a glance

```zen
# Variables
let x = 42          # immutable
var y = 10          # mutable
const PI = 3.14     # constant

# Functions
func add(a, b) {
    return a + b
}

# Lambdas
let double = lambda(x) { return x * 2 }

# Conditionals
if x > 0 { ... } elif x == 0 { ... } else { ... }
let status = if ok { "good" } else { "bad" }

# Loops
while i < 10 { ... }
for v in [1,2,3] { ... }
for i in 0..5 { ... }
break
continue

# Pattern matching
match x {
    1: "one",
    [a, b]: "pair",
    _: "other"
}

# When (conditional expression)
let grade = when { score >= 90: "A", _: "F" }

# Error handling
try {
    risky()
} catch as e {
    print(e.msg)
} catch TypeError {
    print("type error")
} finally {
    cleanup()
}

throw "error message"

# Classes
class Person extends Entity {
    var name
    func init(name) { super.init(); this.name = name }
    func greet() { return "Hi, " + this.name }
}
let p = new Person("Zen")

# Imports
import fs
from json import loads
import requests as req
pymport bs4
pymport os

# FFI
import ffi
let lib = ffi.load("libc.so.6")
let addr = ffi.symbol(lib, "puts")

# Safe mode
# zen run --safe script.z
```

## E.2 Truthiness

Everything is truthy except `nil` and `false`.

## E.3 Operators

| Operator | Meaning |
|----------|---------|
| `+ - * / // % **` | Arithmetic |
| `== != < <= > >=` | Comparison |
| `and or not !` | Logic |
| `+ -` (string) | Concatenation / repeat (`"a" * 3`) |
| `?.` | Safe member access |
| `??` | Nullish coalescing |
| `is` | Type check |
| `in / !in` | Membership |

## E.4 String methods

`upper lower title capitalize trim lstrip rstrip replace split split_whitespace join starts_with ends_with contains find count ord chr int float str`

## E.5 List methods

`push pop insert remove clear sort reverse copy index count extend len`

## E.6 Dict methods

`keys values items get has len copy clear pop update`

## E.7 Key modules

| Module | Purpose |
|--------|---------|
| `fs` | File system |
| `os` | OS / environment / paths |
| `socket` | Sockets |
| `requests` | HTTP client |
| `http.server` | HTTP server |
| `dns` | DNS resolution |
| `crypto` | Cryptography |
| `tls` | TLS / cert pinning |
| `json` | JSON |
| `math` | Math |
| `random` | Random |
| `string` | String utils |
| `re` | Regex |
| `thread` | Threads |
| `channel` | Channels |
| `shared` | Atomic shared values |
| `ffi` | C FFI |
| `sys` | Syscalls |
| `pymport` | Python interop |

## E.8 CLI

```bash
zen run script.z
zen --safe script.z
zen check script.z
zen lint script.z
zen explain Z0401
zen fmt script.z
zen build script.z -o out
zen py-sync
zen --version
```

## E.9 Error bands

`Z01xx` lexer · `Z02xx` parser · `Z03xx` compile · `Z04xx` runtime · `Z05xx` CLI
