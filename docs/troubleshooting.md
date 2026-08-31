# Troubleshooting

Real, verified error messages and gotchas for the current Zen runtime. Each
entry shows an actual failing snippet and the fix.

Run everything against the **rebuilt** binary — the system-installed one may be
stale. From the source tree:

```bash
cargo build --release
./target/release/zen script.z
```

## Naming: `var` not `let`, `func` not `function`

Zen uses `var`/`global` for declarations and `func`/`fn`/`function`/`def` for
functions. Old examples written for other dialects use `let` and
`function() {}`:

```zen
let x = 5        # Error: expected expression, found `let`
x = 5            # this works (implicit declaration), but prefer:
var x = 5

function f() {}  # works — `function` is aliased to `func`
```

## No block scope — variables leak out

`if`/`for`/`while` bodies do **not** create a scope. A `var` inside a block is
visible after it:

```zen
if true {
    var x = 5
}
print(x)         # 5   (not an error!)
```

This is intentional but surprising to Python/JS/Go users.

## Closures capture variables lexically (by reference, with writes)

A closure reads and writes the **shared** variable, not a snapshot. Late-bound
loops all point at the same final value:

```zen
var fns = []
for i in 0 .. 3 {
    fns = fns.push(fn() { return i })
}
print(fns[0]())   # 2   (all see the final i)
print(fns[1]())   # 2
print(fns[2]())   # 2
```

There's also no block scope to capture a fresh copy per iteration. If you need
a snapshot, pass it as a parameter:

```zen
var fns = []
for i in 0 .. 3 {
    var make = fn(v) { return fn() { return v } }
    fns = fns.push(make(i))
}
print(fns[0]())   # 0
```

## Lists update functionally — bind the result

`l.push(x)` returns a **new** list; the original is unchanged. This is the #1
source of "why is my list empty" bugs:

```zen
var l = [1, 2]
l.append(3)          # does nothing visible!
print(l)             # [1, 2]

# Fix — reassign:
l = l.push(3)        # or l.append(3) then bind:  l = l.append(3)
print(l)             # [1, 2, 3]
```

Same for `pop`, `insert`, `sort`, `clear`, `reverse`, etc.

## Missing-key errors

Reading a missing dict key or list index **throws** (it does not return
`null`):

```zen
print({a: 1}.b)     # zen: dictionary has no member: `b`
print([1, 2][5])    # zen: list index out of bounds
```

Use `.get(key)` (returns `null`) or the `??` operator for safe access:

```zen
print({a: 1}.get("b") ?? 0)   # 0
if ({a:1}.len > 0) { ... }
```

## Calling a non-function

```zen
var x = 5
print(x())          # zen: undefined function: x
```

## `len()` is a function; `.len` is a property

`len(x)` is a built-in **function call**. `x.len` is a **member** of a
string/list/dict.

```zen
print(len([1, 2]))     # 2
print([1, 2].len)      # 2
print([1, 2].length()) # 2
```

Extra arguments to `len` are ignored (`len([1],[2])` → `1`).

## Division by zero is not an error

Floating-point division by zero yields `inf`/`-inf` rather than throwing:

```zen
print(1 / 0)      # inf
print(-1 / 0)     # -inf
```

There is no integer/floor division operator (`//`) and no `divmod`.

## `+` on strings and numbers concatenates

`"5" + 3` is accepted and gives `"53"` (string coercion), not an error and not
`8`. Convert explicitly when you need math:

```zen
print("5" + 3)        # 53
print(int("5") + 3)   # 8
```

## `for` only iterates lists

`for` over a dict or a string fails:

```zen
for k in {a: 1} { }   # Error: for requires a list
for ch in "abc" { }   # Error: for requires a list
```

Use `d.keys()` / `d.values()` / `d.items()`, and `"abc".split("")` for chars:

```zen
for k in keys({a: 1}) { print(k) }        # a
for ch in "abc".split("") { print(ch) }   # a b c
```

## `.match` is a keyword parse error

`match` is a reserved keyword, so `.match` member access fails:

```zen
dict.match            # Error: expected member name, found match
re["match"]           # works  (re is a dict)
```

## Constructor must be `func init`

```zen
class A {
    init() {}          # Error: expected func
}
class A {
    func init() {}     # correct
}
```

`static` is unsupported: `class A { static func f() {} }` → error.

## Functions are not hoisted

Define before calling:

```zen
print(double(2))       # zen: undefined function: double
fn double(x) { return x * 2 }
```

## `import` is a statement, not a function

```zen
var m = import("math")    # Error: expected expression, found Import
import math               # correct statement form
```

Use `import math as m` or `from math import pi` for aliasing/names.

## REPL & CLI

- `check` validates without running; use it in CI.
- `lint` reports suspicious patterns (still exits 0 on common issues).
- Inside the REPL, `:help modules` lists every registered module.

## Module not found

`import name` resolves: native modules → `std/*.z` → `name.z`/`name/main.z` →
installed packages. If a module is missing, it's usually because the name
isn't registered or the file isn't on the path:

```zen
import os.path          # Error: module not found: os.path  (only `os` exists)
import "std/logging.z"  # Error: module not found  (use bare name: `import logging`)
import logging          # loads std/logging.z
```

## Stale binary warning

If behavior disagrees with this doc, rebuild and retest:

```bash
cargo build --release
./target/release/zen -e 'print(1/0)'      # inf
```

## See also

- [variables.md](variables.md) — declaration, const, scoping
- [lists.md](lists.md) — functional updates, iteration
- [dicts.md](dicts.md) — safe access, defaults
- [control-flow.md](control-flow.md) — loops and `for` requirements
- [errors.md](errors.md) — try/catch, typed errors
- [cli.md](cli.md) — check, lint, repl, package manager