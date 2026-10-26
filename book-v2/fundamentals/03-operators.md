# Chapter 3: Operators and Expressions

Zen's operators cover arithmetic, comparison, logic, string handling, and safe member access. This chapter lists every operator, its precedence, associativity, and edge cases.

## 3.1 Operator precedence (high to low)

| Precedence | Operator(s) | Associativity | Description |
|------------|-------------|---------------|-------------|
| 1 | `()` `[]` `.` `?.` | left | Grouping, indexing, field access, safe access |
| 2 | `**` | right | Power |
| 3 | `-` (unary) `not` `!` | right | Negation, logical not |
| 4 | `*` `/` `//` `%` | left | Multiply, divide, floor-divide, modulo |
| 5 | `+` `-` | left | Add, subtract, concatenate |
| 6 | `==` `!=` `<` `<=` `>` `>=` | left | Comparison |
| 7 | `and` | left | Logical and (short-circuits) |
| 8 | `or` | left | Logical or (short-circuits) |
| 9 | `??` | left | Nullish coalescing |
| 10 | `if-else` (ternary) | right | Conditional expression |
| 11 | `=` `+=` `-=` ... | right | Assignment |

Use parentheses whenever precedence is unclear.

## 3.2 Arithmetic

| Operator | Meaning | Example | Result | Notes |
|----------|---------|---------|--------|-------|
| `+` | add | `1 + 2` | `3` | also string concat |
| `-` | subtract | `5 - 3` | `2` | |
| `*` | multiply | `3 * 2` | `6` | also string repeat `"x"*3` |
| `/` | divide (float) | `7 / 2` | `3.5` | div by zero → `DivideByZero` |
| `//` | floor divide | `7 // 2` | `3` | floors toward −inf |
| `%` | modulo | `7 % 3` | `1` | sign follows dividend |
| `**` | power | `2 ** 3` | `8` | right-associative |
| `-` | negate | `-x` | `-x` | unary |

### Floor division and modulo edge cases

```zen
print(7 // 2)    # 3
print(-7 // 2)   # -4   (floor toward −inf, not truncation)
print(7 % 2)     # 1
print(-7 % 2)    # -1   (sign follows dividend)
print(7.5 // 2)  # 3.0  (floor works on floats too)
```

### Division by zero

```zen
try {
    print(1 / 0)
} catch as e {
    print(e.msg)  # DivideByZero: division by zero
}
```

## 3.3 String concatenation and repetition

When either side of `+` is a string, the other side is stringified:

```zen
print("count: " + 42)       # "count: 42"
print("flag: " + true)      # "flag: true"
print(nil + "end")          # "nilend"
print("Zen " + "3.0")       # "Zen 3.0"
```

`42 + "count"` is a **type error** (number + string without string on the left). Use `str(42) + "count"`.

`*` repeats strings:

```zen
print("x" * 5)      # "xxxxx"
print("ab " * 3)    # "ab ab ab "
print(3 * "ab")     # also works: "ababab"
```

## 3.4 Comparison

| Operator | Meaning | Notes |
|----------|---------|-------|
| `==` | equal | numeric coercion: `1 == 1.0` → true |
| `!=` | not equal | |
| `<` | less than | |
| `<=` | less than or equal | |
| `>` | greater than | |
| `>=` | greater than or equal | |

Numeric coercion in `==`:
```zen
print(1 == 1.0)    # true
print(1 == "1")    # false (type mismatch, no coercion to string)
print([] == [])    # true (empty lists equal by value)
print({} == {})    # true (empty dicts equal, order-insensitive)
print({"a":1,"b":2} == {"b":2,"a":1})  # true
```

NaN: `nan == nan` is false; `nan != nan` is true.

## 3.5 Logic

Zen truthiness: everything is truthy except `nil` and `false`. Notably `0`, `0.0`, `""`, `[]`, `{}` are **truthy**.

| Operator | Meaning | Short-circuits? |
|----------|---------|-----------------|
| `and` | logical and | yes — returns first falsy or last |
| `or` | logical or | yes — returns first truthy or last |
| `not` | logical not | prefix, returns bool |
| `!` | logical not | alias for `not` |

```zen
print(true and false)   # false
print(true or false)    # true
print(not true)         # false
print(!false)           # true

# Short-circuit: right side not evaluated if left decides
print(false and error())  # false — error() never called
print(true or error())    # true  — error() never called
```

`and`/`or` return the operand value, not necessarily a bool:
```zen
print("hello" and "world")   # "world"
print("" or "fallback")      # "fallback"  (note: "" is truthy in Zen!)
print(nil or "fallback")     # "fallback"
```

## 3.6 Safe member access: `?.` (optional chaining)

`?.` reads a field only if the object is non-nil; otherwise short-circuits to `nil`.

```zen
let user = nil
print(user?.name)           # nil  (no error)

let user = { name: "Zen" }
print(user?.name)           # "Zen"

let data = nil
print(data?.a?.b?.c)        # nil  (stops at first nil)
```

Compare with strict access, which raises on nil:
```zen
let user = nil
print(user.name)   # TypeError: cannot read field on nil
```

`?.` works on dicts, instances, and any object with fields. It does **not** work on lists for indexing — use `list[i]` directly or check bounds first.

## 3.7 Nullish coalescing: `??`

`??` returns its left side if non-nil, otherwise the right side. It's a **nil-default**, not a falsy-default — `0`, `false`, and `""` are returned as-is.

```zen
print(nil ?? "default")     # "default"
print("set" ?? "default")   # "set"
print(0 ?? "default")       # 0      (0 is not nil)
print(false ?? "default")   # false
print(""  ?? "default")     # ""     (empty string is not nil)
print(nil ?? nil ?? "fb")   # "fb"   (chaining)
```

Common use: config fallback:
```zen
let timeout = config?.timeout ?? 30
```

## 3.8 Conditional expression (ternary)

```zen
let status = if score >= 60 { "pass" } else { "fail" }
```

The condition must be braced. The result is the value of the taken branch; both branches must be type-compatible (Zen is dynamically typed, so any values work, but mixing types may surprise downstream code).

## 3.9 Assignment and augmented assignment

```zen
let x = 10
x += 5      # x = 15
x -= 3      # x = 12
x *= 2      # x = 24
x /= 4      # x = 6.0  (float division)
x //= 2     # x = 3
x %= 2      # x = 1
```

Augmented assignment works on any lvalue (variable, field, index):
```zen
let d = {"count": 0}
d["count"] += 1   # d["count"] is now 1
```

## 3.10 `is` — type membership

`x is TypeName` checks whether `x`'s runtime type matches the named type:

```zen
print(42 is int)          # true
print(3.14 is float)      # true
print("hi" is string)     # true
print([] is list)         # true
print({} is dict)         # true
print(true is bool)       # true
print(nil is nil)         # true
print(42 is string)       # false
```

The right side must be a type keyword/literal, not a variable:
```zen
let t = "int"
print(42 is t)   # ParseError — type name required literally
```
Use `typeof(x) == t` instead for dynamic type-name comparison.

## 3.11 `in` / `!in` — membership

```zen
print(2 in [1, 2, 3])      # true
print(5 in [1, 2, 3])      # false
print(5 !in [1, 2, 3])     # true
print("a" in {"a": 1})     # true (dict key membership)
print("z" in "Zen")        # true (substring)
```

For strings, `in` checks substring membership. For lists/sets/dicts, it checks element/key membership.

## 3.12 Quick reference

```zen
# Arithmetic
1 + 2       # 3
7 / 2       # 3.5
7 // 2      # 3
7 % 3       # 1
2 ** 3      # 8
-5          # -5

# Strings
"a" + "b"   # "ab"
"x" * 3     # "xxx"
"count: " + 42   # "count: 42"

# Comparison
1 == 1.0    # true
"a" != "b"  # true

# Logic
true and false   # false
true or false    # true
not true         # false

# Safe access
nil?.name        # nil
{name: "Z"}?.name  # "Z"

# Coalescing
nil ?? "d"       # "d"
0 ?? "d"         # 0

# Ternary
if ok { "yes" } else { "no" }

# Type check
42 is int        # true

# Membership
2 in [1,2,3]     # true
"a" in "Zen"     # true
```

## 3.13 Exercises

1. Write `sign(x)` returning `1`, `-1`, or `0` using only comparison operators.
2. Demonstrate that `0` is truthy: `if 0 { print("zero is truthy") }` prints.
3. Write `safe_div(a, b)` returning `a // b` or `nil` if `b == 0`, using `try`/`catch`.
4. Show that `"0" ?? "default"` returns `"0"` (string "0" is not nil).
5. Prove `nan != nan` using `0.0 * inf`.
6. Write `in_range(x, lo, hi)` using chained comparisons (Zen does **not** chain `lo < x < hi` — use `lo < x and x < hi`).
7. Show the difference between `and` short-circuiting: `false and error()` does not evaluate `error()`.
