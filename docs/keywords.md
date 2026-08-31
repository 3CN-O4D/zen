# Keywords

The complete, authoritative keyword set (from the lexer in `src/runtime.rs`).
If a word isn't on this list it's an ordinary identifier — including `self`,
`error`, and `init`.

```
let const var global          variable declarations
print                         print statement
if elif else                  conditionals
while for in break continue   loops
function func fn def          function declarations
lambda                        anonymous function
class new extends inherit super   OOP
this                          instance reference (alias for self)
import from as include load native   modules
try catch except finally throw        errors
typeof is                     type checks / equality
switch case default           multi-way branching
match when                    match/guard expressions
and or not                    boolean operators
true false null               literals
```

## Declarations

| Keyword | Meaning | Equivalent to |
|---------|---------|---------------|
| `let` | declare a (rebindable) variable | `var` |
| `var` | declare a (rebindable) variable | `let` |
| `const` | declare a constant; **reassignment raises an error** | — |
| `global` | declare at module scope so functions can see/assign it | `var` at top level |

```zen
let x = 1         # same as var
var y = 2
const z = 3
z = 4             # Error: cannot assign to constant: z
```

## Conditions

- `if`, `elif`, `else` — conditional execution. Blocks are required.
- `and`, `or`, `not` — boolean operators (also `&&`, `||`, `!`).

```zen
if x > 0 and y > 0 {
    print "both positive"
} elif x < 0 {
    print "negative"
} else {
    print "zero-ish"
}
```

## Loops

- `while` — loop while a condition is truthy (block required).
- `for ... in ...` — iterate a range, list, dict keys, or unpacked list-of-lists.
- `break` — exit the innermost loop.
- `continue` — skip to the next iteration.

```zen
for i in 0..3 { print i }
for item in [1, 2, 3] { if item == 2 { continue }; print item }
while true { break }
```

## Functions

- `function`, `func`, `fn`, `def` — all declare a named function.
- `lambda` — anonymous function expression: `lambda(x): x * 2` or `lambda (x) { return x * 2 }`.

```zen
fn f() { return 1 }          # fn
func g() { return 2 }        # func
def h() { return 3 }         # def
var d = lambda(x): x * 2     # lambda
```

## Classes

- `class` — class declaration (statement only; no anonymous classes).
- `new` — instantiate: `new Person("Ada")`.
- `extends` / `inherit` — subclass: `class Dog extends Animal { }`.
- `super` — call parent constructor `super(...)` or parent method `super.hi()`.
- `this` / `self` — the current instance (identical at runtime; `self` is just
  an identifier, `this` is the alias).

```zen
class Animal {
    var name
    func init(name) { self.name = name }
    fn speak() { return self.name }
}

class Dog extends Animal {
    fn speak() { return super.speak() + " barks" }
}
```

## Modules

- `import` — `import fs`, `import math as m`, `import "mod.z"`.
- `from` — `from std import sys`.
- `as` — aliasing in imports.
- `include` / `load` — inline-load another source file.
- `native` — declare that a name is provided natively: `native function http_get(...)`.

## Errors

- `try`, `catch` / `except`, `finally` — structured error handling.
- `throw` / `raise` — raise an error value.

```zen
try {
    throw "boom"
} catch as err {
    print "caught: " + err
} finally {
    print "always runs"
}
```

## Branching / matching

- `switch` `case` `default` — statement form.
- `match` / `when` — expression forms with optional guards.

```zen
var v = 3
switch v {
    case 1: print "one"
    default: print "other"
}

var word = match v { 1: "one", x if x > 3: "big", _: "other" }
print word
```

## Value keywords

- `true`, `false`, `null` — literals.

## Operators that are keywords

- `typeof` — `typeof x` returns a type string.
- `is` — equality: `1 is 1` → `true` (no type checking, plain equality).
- `in` — membership for loops and membership tests.

## What is NOT a keyword

- `self` — convention only; the lexer treats it as a normal identifier used by
  the VM as the implicit instance binding.
- `init` — the constructor is `func init(...)`, but `init` is not reserved.
- `error` — the conventional name in `catch` blocks.
- `do`, `repeat`, `goto`, `raise` (`raise` is an alias of `throw`), `wait`,
  `sleep`, `go`, `page`, `title` — none of these are language keywords.

### Aliases table

| Canonical | Alias(es) |
|-----------|-----------|
| `var` | `let` |
| `function` | `func`, `fn`, `def`, `procedure`, `proc` |
| `catch` | `except` |
| `extends` | `inherit` |
| `throw` | `raise` |
| `include` | `load` |
| `&&` | `and` |
| `\|\|` | `or` |
| `!` | `not` |
| `this` | `self` |

> **Note:** `procedure` / `proc` and `raise` are lexed as aliases but are
> primarily accepted for compatibility; prefer `fn` and `throw`.