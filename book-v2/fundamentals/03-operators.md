# Chapter 3: Operators and Expressions

Zen's operators cover arithmetic, comparison, logic, string handling, and
safe member access. This chapter lists every operator, its precedence, and
common gotchas.

## 3.1 Arithmetic

| Operator | Meaning | Example | Result |
|----------|---------|---------|--------|
| `+` | add / concat | `1 + 2`, `"a" + "b"` | `3`, `"ab"` |
| `-` | subtract | `5 - 3` | `2` |
| `*` | multiply / repeat | `3 * 2`, `"x" * 3` | `6`, `"xxx"` |
| `/` | divide (float) | `7 / 2` | `3.5` |
| `%` | modulo | `7 % 3` | `1` |
| `**` | power | `2 ** 3` | `8` |
| `-` | negate | `-x` | `-x` |

Floor division: `int(7 / 2)` → `3` (there is no `//` operator; `//` starts a comment).

Division by zero raises `DivideByZero`, catchable with `try`/`catch`.

## 3.2 Comparison

| Operator | Meaning |
|----------|---------|
| `==` | equal (numbers coerce: `1 == 1.0`) |
| `!=` | not equal |
| `<` | less than |
| `<=` | less than or equal |
| `>` | greater than |
| `>=` | greater than or equal |

Comparing incompatible types (e.g. string vs number) raises `TypeError`.

## 3.3 Logic

| Operator | Meaning | Notes |
|----------|---------|-------|
| `and` | logical and | short-circuits |
| `or` | logical or | short-circuits |
| `not` | logical not | prefix |
| `!` | logical not | alias for `not` |

Zen truthiness: everything is truthy except `nil` and `false`.

## 3.4 String concatenation: `+`

When either side of `+` is a string, the other side is stringified:

```zen
print("caught: " + e)       # error → its message
print("count: " + 42)       # int → "42"
print("flag: " + true)      # bool → "true"
print(nil + "end")          # nil → "nil"
```

## 3.5 Safe member access: `?.` (optional chaining)

`?.` reads a field only if the object is non-nil; otherwise it short-circuits
to `nil` instead of raising. Use it for nested data that might be missing.

```zen
let user = nil
print(user?.name)           # → nil  (no error)

let user = { name: "Zen" }
print(user?.name)           # → "Zen"

# nested: stops at the first nil
let data = nil
print(data?.a?.b?.c)        # → nil
```

Compare with strict access, which raises on nil:

```zen
let user = nil
print(user.name)   # TypeError: cannot read field on nil
```

## 3.6 Nullish coalescing: `??`

`??` returns its left side if it's non-nil, otherwise the right side. It's a
nil-default, not a falsy-default — `0`, `false`, and `""` are all returned as-is.

```zen
print(nil ?? "default")     # → "default"
print("set" ?? "default")   # → "set"
print(0 ?? "default")       # → 0      (0 is not nil)
print(false ?? "default")   # → false
print(""  ?? "default")     # → ""     (empty string is not nil)
```

Chain multiple defaults:

```zen
print(nil ?? nil ?? "fallback")   # → "fallback"
```

Common use: reading config with a fallback:

```zen
let timeout = config?.timeout ?? 30
```

## 3.7 Precedence (high to low)

1. `()` grouping, `[]` indexing, `.` / `?.` field access
2. `**` power
3. `-` unary negate, `not` / `!`
4. `*`, `/`, `//`, `%`
5. `+`, `-`
6. `==`, `!=`, `<`, `<=`, `>`, `>=`
7. `and`
8. `or`
9. `??` (nullish coalescing)
10. `if ... else` / `when` (conditional expressions)

Use parentheses whenever precedence is unclear.

## Quick reference

```zen
a?.b.c          # safe field access: nil if a is nil
x ?? default    # nil default: x if non-nil, else default
"n: " + 5       # string concat with stringification
try { ... } catch as e { print(e.msg) }   # error handling (Chapter 11)
```
