# Variables

Variables hold values. There are three declaration keywords — `let`, `var`,
and `const` — plus the very detailed rules about what's expected at runtime:
**`let` and `var` are identical** (`let` is sugar), and **`const` is enforced**.

## Declaring variables

```zen
var x = 10          # a rebindable variable
let y = "text"      # let == var
const z = 3.14      # a constant

z = 1               # Error: cannot assign to constant: z
```

### `var` vs `let`

There is no behavioral difference. `let` is accepted as an alias for `var`,
so code that mixes styles is fine:

```zen
let a = 1
var b = 2
a = 3               # fine — let is rebindable
b = 4               # fine
```

### `const`

Constants can be declared once and never reassigned (or redeclared):

```zen
const API_URL = "https://api.example.com"
API_URL = "https://other.example"      # Error: cannot assign to constant

const PI = 3.14159                     # conventional uppercase, not required
```

> **Gotcha:** `const` is enforced at *assignment/rebinding* time, not at
> *parse* time. The value itself (a list/dict) is still mutable in place via
> methods — `const` only stops you from reassigning the name.

```zen
const bag = []
var list = bag.push(1)      # bag itself is unchanged (functional methods)
```

## Type inference: there are none

Zen infers types at runtime. `var`, `let`, and `const` never annotate types,
and any variable can hold any value type at any time:

```zen
var item = 1
item = "now a string"       # fine
item = [1, 2, 3]            # fine
item = { a: 1 }             # fine
```

Inspect the runtime type with `typeof` (keyword) or `type()` (function):

```zen
print typeof 1                        # int
print typeof "s"                      # string
print type([1])                       # list
print type(new Object())              # object
```

Type names: `null`, `bool`, `int`, `string`, `list`, `dict`, `object`,
`socket`, `udp_socket`, `listener`, `function`.

## Multiple assignment

Assign several variables at once (index-aligned):

```zen
var a, b = 1, 2
print a, b                            # 1 2

var x, y, z = 1, 2, 3
print x, y, z                         # 1 2 3
```

## Destructuring

List and dict patterns unpack a value into named variables:

```zen
let [first, second] = [1, 2, 3]
print first, second                   # 1 2

let [head, ...rest] = [1, 2, 3]       # Error: rest patterns not supported

let {name, age} = { name: "Ada", age: 36, city: "London" }
print name, age                       # Ada 36
```

Rules:

- List patterns take elements by index: `let [a, b] = [1, 2, 3]` binds the
  first two.
- Dict patterns take keys by name: `let {x} = {x: 42, y: 0}` binds `x`.
- **Rest patterns (`...rest`) are not supported.**

## Scoping rules

Zen has **function-level scoping with closures**, and **no block scope**.

### Functions share enclosing variables (closures)

A function can read **and write** variables from the scope where it was
defined:

```zen
var counter = 0

fn increment() {
    counter = counter + 1        # writes to the enclosing counter
}

increment()
increment()
print counter                    # 2
```

This is how counters, accumulators and memoized caches work without `global`.

### `global` is an alias for `var`

The `global` keyword is accepted and maps to the same token as `var`. It does
not introduce any special module-global machinery:

```zen
global counter = 0        # equivalent to: var counter = 0
```

> Use `global` where you mean "this belongs to the whole module" for
> readability; it changes nothing at runtime.

### `{ }` blocks do not create scope

Variables created inside `if`/`for`/`while`/`try` blocks **leak** into the
enclosing scope:

```zen
if true {
    var inner = 5
}
print inner                       # 5  (leaked into the outer scope)

for i in 0..2 {
    var item = i
}
print item                        # 1  (leaked)
```

If you want to contain a name, do it inside a function:

```zen
fn scoped() {
    var tmp = do_something()
    return tmp * 2
}
```

### Parameters are always local

Function parameters shadow anything outside:

```zen
var name = "outer"
fn greet(name) {
    print name                     # the parameter wins
}
greet("inner")                     # inner
```

## Variable names

- Start with a letter or `_`; then letters, digits, `_`.
- **Reserved words cannot be used as names** (see [keywords](keywords.md)):
  `if`, `for`, `match`, `print`, `true`, etc. are syntax.
- `self` / `this` are special inside class methods (see
  [classes](classes.md)) but otherwise ordinary identifiers.
- `_` alone is a valid (throwaway) name — e.g. `for _ in 0..5 { ... }`.

```zen
var snake_case = 1
var _private_like = 2
var camelCase = 3
var ALL_CAPS = 4
var _ = 5                  # valid
```

## Referencing an undeclared variable

Reading a name that was never declared is a runtime error:

```zen
print not_declared
# zen: undefined variable: `not_declared`
```

There is no implicit global creation when you assign to a new name *inside a
function* — a bare `new_name = 5` where `new_name` doesn't exist yet is an
"undefined variable" error:

```zen
fn bad() {
    mystery = 1            # Error: undefined variable: `mystery`
}
```

Declare it first (`var mystery = 1`) or with `var`/`const`.

## Common pitfalls

| Symptom | Cause | Fix |
|---------|-------|-----|
| `cannot assign to constant: x` | reassigning a `const` | make it `var`/`let`, or declare a new name |
| `undefined variable: `x`` | reading an unset name | declare with `var`/`let`/`const` first |
| Variable is different inside a function than outside | reading a pointing name / function-local param | use a distinct name or parameter |
| Variable changed where I didn't expect | block scoping doesn't exist — declarations leak | wrap the logic in a function |
| `expected `{`, found ...` at the end of the line | you wrote `if cond var x = 1` — `var` statements need to be standalone statements, not a single-statement `if` body | put the assignment in a block |
| `rest patterns are not supported` | `let [head, ...rest] = ...` | list out the names explicitly |