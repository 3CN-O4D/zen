# Appendix E: Quick Reference Card

Zen 3.0 — the essentials on one page (expanded).

## E.1 Syntax at a glance

```zen
# Comments
# line comment        // line comment
/* block comment */

# Declarations
let x = 10            # immutable binding
var y = 20            # mutable binding
const MAX = 100       # constant
shared counter = 0    # thread-shared mutable

# Functions
func add(a, b) { return a + b }
let add2 = fn(a, b) => a + b
let add3 = (a, b) => a + b

# Defaults
func greet(name, greeting = "Hi") { return greeting + ", " + name }

# Variadic
func sum_all(...nums) { var t = 0; for n in nums { t += n }; return t }

# Classes
class Point {
    var x = 0.0
    var y = 0.0
    func init(x, y) { self.x = x; self.y = y }
    func dist() { return (self.x**2 + self.y**2) ** 0.5 }
    func __str__() { return "Point(${x},${y})" }
}
let p = new Point(3, 4)

# Modules
import math
import math as m
from math import sin, pi
pub func exported() {}   # visible outside module
```

## E.2 Literals

| Type | Literal | Example |
|------|---------|---------|
| nil | `nil` / `null` | `let x = nil` |
| bool | `true` / `false` | `let ok = true` |
| int | `42`, `0xFF`, `0b1010`, `0o755`, `1_000` | `let n = 0x2A` |
| float | `3.14`, `1.5e-10` | `let f = 2.5` |
| string | `"hi ${name}"`, `'raw'`, `"""triple"""`, `` `backtick` `` | `let s = "Zen"` |
| bytes | `bytes([1,2,0xFF])` | `let b = bytes([0x41])` |
| list | `[a, b, c]` | `let l = [1,2,3]` |
| dict | `#{k: v}` | `let d = #{a:1}` |
| set | `%{a, b}` | `let s = %{1,2}` |
| tuple | `#(a, b)` | `let t = #(1,"x")` |

## E.3 Operators (precedence high→low)

```
() [] . ?.         postfix / grouping
**                 power (right)
- ! not ~ typeof   unary (right)
* / %              multiplicative
+ -                additive
<< >>              shifts
&                  bitwise and
^                  bitwise xor
|                  bitwise or
is is not === !== == != < > <= >= in not in   comparison
and &&             logical and
or ||              logical or
??                 nullish coalescing
cond ? a : b / a if c else b   ternary (right)
= += -= *= ...     assignment (right)
```

Truthiness: ONLY `false`, `nil`, `null` are falsy. Everything else (including
`0`, `0.0`, `""`, `[]`, `#{}`, `%{}`, `#()`) is truthy.

## E.4 Control flow

```zen
if c { } elif c { } else { }
while c { }
for x in iter { }          # for k, v in d.items()
for i in 0..5 { }          # exclusive end
for i in 0->5 { }          # inclusive end
break / continue
return [expr]
throw expr
try { } catch [T] as e { } finally { }
match v { pat => expr, _ => expr }
when { c => expr, _ => expr }
```

## E.5 Built-ins (selected)

`print`, `str`, `int`, `float`, `bool`, `list`, `tuple`, `set`, `dict`,
`len`, `range`, `enumerate`, `zip`, `sorted`, `reversed`, `any`, `all`, `sum`,
`min`, `max`, `abs`, `typeof`, `type`, `chr`, `ord`, `open`, `exit`,
`assert`, `repr`.

## E.6 String methods

`upper lower title capitalize swapcase strip lstrip rsplit split splitlines
join replace count find rfind startswith endswith contains ljust rjust center
zfill isdigit isalpha isalnum isspace islower isupper`.

## E.7 List methods

`push pop insert remove clear map filter reduce each join index count`.

## E.8 Dict methods

`keys values items get pop clear update`.

Iterating a dict yields keys; use `.items()` for key/value.

## E.9 Socket basics

```zen
import socket
let s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.connect(("host", 80))
s.send(b"GET / HTTP/1.1\r\nHost: host\r\n\r\n")
let data = s.recv(4096)
s.close()
```

Options: `SO_REUSEADDR`, `SO_KEEPALIVE`, `TCP_NODELAY`, `SO_RCVBUF`,
`SO_SNDBUF`, `IP_TTL`, `IP_HDRINCL`.

## E.10 Error codes (Z-ranges)

- Z01xx lexer | Z02xx parser | Z03xx runtime | Z04xx I/O/system | Z05xx FFI/interop

Run: `zen explain Z####`

## E.11 CLI commands

`zen` `zen run <f>` `zen -m <mod>` `zen -e <code>` `zen check <f>` `zen lint <f>`
`zen explain <code>` `zen build <f>` `zen test` `zen fmt` `zen help`
`zen --version`

Env: `ZEN_PATH`, `ZEN_DEBUG`, `PYTHONPATH`, `VIRTUAL_ENV`.

## E.12 Truthiness cheat

```
value        bool(value)
----         -----------
false        false
nil / null   false
everything else   true   (0, 0.0, "", [], #{}, %{}, #() all true)
```

## E.13 Equality cheat

| Operator | Meaning | `1 == 1.0` | `[1] == [1]` | `[1] is [1]` |
|----------|---------|------------|--------------|--------------|
| `==` | value equality | true | true | — |
| `===` | strict equality | false | — | — |
| `is` | identity | true | false | false |

## E.14 Common gotchas (one-liners)

- `0` and `""` are **truthy** in Zen.
- Strings are **immutable**: `s[0] = "X"` errors.
- `b = a` shares a list; use `list(a)` to copy (shallow).
- `and`/`or` return **booleans**, not operands; use `??` for defaults.
- `-2 ** 2` is `-4`; use `(-2) ** 2` for `4`.
- Dict keys must be **strings**.
- `s[0:100]` clamps; `s[100]` errors.
- `4 / 2` is `2.0`; `4 // 2` is `2`.
- Modules are **cached**; re-import returns the cached module.
- `pub` exports; without it, symbols are file-private.

## E.15 Exercises

1. Without running, state the truthiness of `0`, `""`, `[]`, `false`, `nil`.
2. Write `clamp(x, lo, hi)` in one line using ternary.
3. Show `is` vs `==` for two equal lists.
4. Write `safe_div(a, b)` returning nil on `b == 0`.
5. Convert `"1,2,3"` to `[1, 2, 3]` using split + map/int.
6. Reverse a string without `.reverse()`.
7. Count word frequencies in a sentence using a dict.
8. Merge two dicts into a third.
9. Write `is_prime(n)` and test up to 100.
10. Memorize the Z-code ranges: Z01xx/Z02xx/Z03xx/Z04xx/Z05xx.

'''
print('appendix e done')