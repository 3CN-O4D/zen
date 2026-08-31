# Operators

Zen's operators are largely familiar from C/Python. This page is the complete
reference, including precedence and the surprising corner cases.

## Precedence (low → high)

```
range           ->   ..
ternary         ? :          cond ? a : b
ternary-if      a if cond else b
nullish         ??
or              or  ||
and             and  &&
not             not  !  (prefix)
comparison      ==  !=  ===  !==  is  <  >  <=  >=  in
bitwise-or      |
bitwise-xor     ^
bitwise-and     &
shifts          <<  >>
additive        +  -
multiplicative  *  /  %
unary           -  !  not  ~  typeof
power           **   (right-associative)
postfix         .name  ?.name  [index]  (call)
```

## Arithmetic

| Operator | Meaning | Notes |
|----------|---------|-------|
| `+` | addition / string concatenation | `1 + 2` → `3`, `"a" + 1` → `"a1"`, `[1] + [2]` → `[1, 2]` |
| `-` | subtraction | |
| `*` | multiplication / string repetition | `"ab" * 3` → `"ababab"`. **`[1] * 2` is an error** |
| `/` | division | always numeric |
| `%` | remainder | sign follows the dividend: `-7 % 3` → `-1` |
| `**` | power | right-associative: `2 ** 3 ** 2` → `2**(3**2)` = 512 |

```zen
print 10 / 3          # 3.333...
print 10 % 3          # 1
print -7 % 3          # -1
print 2 ** 10         # 1024
print "ha" * 3        # hahaha
print "line " + 12    # line 12
```

> There is no floor-division `//` operator — `//` is a comment. Round down
> with `floor(x)`.

## Ranges

Two range operators build lists.

| Operator | Meaning | Example | Result |
|----------|---------|---------|--------|
| `..` | **exclusive** end | `1 .. 4` | `[1, 2, 3]` |
| `->` | **inclusive** end | `1 -> 4` | `[1, 2, 3, 4]` |

Both **auto-descend** when the end is smaller than the start:

```zen
print 5 .. 1      # [5, 4, 3, 2]
print 4 -> 1      # [4, 3, 2, 1]
```

Use ranges in loops directly:

```zen
for i in 0 .. 5 { print i }        # 0 1 2 3 4

for i in 5 -> 1 { print i }        # 5 4 3 2 1
```

There are **no** `to`, `by`, or `@` range operators.

## Comparison & equality

| Operator | Meaning |
|----------|---------|
| `==` / `!=` | value equality across types |
| `===` / `!==` | strict equality |
| `is` | plain value equality (same as `==`) |
| `<` `>` `<=` `>=` | ordering |
| `in` | membership |

Equality is **typed and deep** — different types are never equal:

```zen
print 1 == "1"              # false (types differ)
print "5" == 5              # false
print 1 is 1.0              # true (both numbers)
print [1, 2] == [1, 2]      # true (deep)
print {} == {}              # true (deep)
```

`===` additionally checks the runtime type discriminant, but because `==` is
already type-strict, `1 === 1.0` is `true` and `1 === "1"` is `false`.

### Chained comparisons

`1 < x < 10` composes correctly:

```zen
var x = 5
print 1 < x < 10            # true
```

### `is` vs `==`

`is` is plain equality, not identity:

```zen
print [] is []              # true
print 1 is "1"              # false
```

There is no `is not`, no `not in`, and no `&&=` style chaining —
`not in` is a parse error, use `not (1 in [1, 2])`.

### `in` (membership)

Works on strings (substring), lists (element), and dicts (**key** presence):

```zen
print "ell" in "hello"       # true
print 2 in [1, 2, 3]         # true
print "a" in { a: 1 }        # true   (checks keys, not values)
print "b" in { a: 1 }        # false
```

## Logical operators

| Operator | Aliases | Behaviour |
|----------|---------|-----------|
| `and` / `&&` | – | short-circuiting boolean AND |
| `or` / `\|\|` | – | short-circuiting boolean OR |
| `not` / `!` | – | boolean negation |

```zen
if x > 0 and y > 0 { ... }
if a or b { ... }
if not done { ... }
if !done { ... }
```

Truthiness (for conditions and `filter`):

- `null`, `0`, `""`, `[]`, `{}` are **falsy**
- everything else is truthy (`"false"`, `[0]`, etc.)

## Nullish coalescing

`??` returns the right-hand value only when the left is `null`:

```zen
var a = null ?? "default"      # "default"
var b = false ?? "default"     # false  (false is not null!)
var c = 0 ?? 99                # 0
var port = config.get("port") ?? 8080
```

`??` is a value-level "or" — it does **not** trigger on `false`, `0`, or `""`.

## Ternary

Two spellings, both supported:

```zen
var msg = ok ? "yes" : "no"          # C style
var msg = "yes" if ok else "no"      # Python style
```

The `if`-style ternary is part of a larger expression; longer alternate
branches are readably expressed with match (see control-flow).

## Bitwise & shifts

| Operator | Meaning |
|----------|---------|
| `&` | AND |
| `\|` | OR |
| `^` | XOR |
| `~` | NOT |
| `<<` / `>>` | shift left / right |

```zen
print 6 & 3        # 2
print 6 | 3        # 7
print 6 ^ 3        # 5
print ~0           # -1
print 1 << 4       # 16
print 256 >> 4     # 16
```

## Assignment operators

Plain `=` plus the compound set:

```
=  +=  -=  *=  /=  %=   ??=  &=  |=  ^=  <<=  >>=
```

```zen
var n = 10
n += 5          # 15
n *= 2          # 30
n ??= 99        # leaves 30 (n is not null)
n %= 7          # 2
```

### Increment / decrement

`++` and `--` work on **variables and members**, not on indexed targets:

```zen
var i = 1
i++              # 2
i--              # 1

var d = { x: 1 }
d.x++            # d.x becomes 2   (member ok)
```

```zen
var l = [1]
l[0]++           # Error: increment/decrement requires a variable
l[0] += 1        # Error: invalid assignment target
d["x"] += 1      # Error: invalid assignment target
```

Plain indexed assignment **does** work:

```zen
var l = [10, 20]
l[0] = 99
print l          # [99, 20]

var d = { a: 1 }
d["a"] = 2
print d.a        # 2
```

## Unary operators

| Operator | Meaning |
|----------|---------|
| `-x` | numeric negation |
| `!x` / `not x` | logical negation |
| `~x` | bitwise NOT |
| `typeof x` | runtime type name (also usable as `typeof(x)`) |

```zen
print typeof "s"        # string
print -5                 # -5
print not false          # true
```

## Member access

- `.name` — dict key or object field access.
- `?.name` — safe access; returns `null` instead of erroring when the
  left-hand side is `null`.
- `[index]` — list index / string index / dict key. Negative list/string
  indices count from the end. **Slices (`s[1:4]`) are not supported** — use
  `.slice(1, 4)` / `s.slice(1, 4)` on strings and lists.

```zen
var d = { a: { b: 5 } }
print d.a.b            # 5
print d?.missing       # null  (safe access)

var s = "hello"
print s[1]             # "e"
print s[-1]            # "o"
print s.slice(1, 3)    # "el"
```

## Common pitfalls

| Bad expression | Why | Correct |
|----------------|-----|---------|
| `1 not in [1,2]` | `not in` isn't a token | `not (1 in [1,2])` |
| `a is not b` | no `is not` | `not (a is b)` or `a != b` |
| `l[0] += 1` | compound/index assignment unsupported | `l = l.slice(...)` / functional style |
| `x && y` with non-bools | returns boolean result of and-ing | fine — both operands truthiness-based |
| `"1" + 2` for arithmetic | string + number concatenation | `int("1") + 2` |
| `//` in an expression | `//` starts a comment | use `/` |
| `5.to`, `1 by 2` range | not supported | `5 .. 1`, `1 -> 4`, or `range(...)` |
| `[1,2] * 2` to repeat a list | not supported | `loop`/`concat` or literal expansion |