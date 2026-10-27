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

Zen uses Python-like truthiness (see §3.13):

> **Falsy:** `false`, `nil`, `null`, `0`, `0.0`, `""`, `[]`, `#{}`, `%{}`, `#()`.
> **Truthy:** everything else.

| Operator | Meaning | Short-circuits? |
|----------|---------|-----------------|
| `and` | logical and | yes — returns first falsy operand, else last |
| `or` | logical or | yes — returns first truthy operand, else last |
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
print("" or "fallback")      # "fallback"  ("" is falsy)
print(nil or "fallback")     # "fallback"
print(0 or "fallback")       # "fallback"  (0 is falsy)
print(42 or "fallback")      # 42  (42 is truthy, short-circuits)
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

## 3.12 Complete precedence table (canonical)

The parser uses the following levels, from lowest binding to highest. This
matches `docs/03-syntax.md` exactly.

| Level | Operators | Associativity |
|-------|-----------|---------------|
| 1 | `?:` ternary, `if expr else expr` | right |
| 2 | `??` nullish coalescing | left |
| 3 | `or`, `||` | left |
| 4 | `and`, `&&` | left |
| 5 | `is`, `is not`, `===`, `!==`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `in`, `not in` | left |
| 6 | `|` bitwise or | left |
| 7 | `^` bitwise xor | left |
| 8 | `&` bitwise and | left |
| 9 | `<<`, `>>` shifts | left |
| 10 | `+`, `-` | left |
| 11 | `*`, `/`, `%` | left |
| 12 | unary `-`, `!`, `not`, `~`, `typeof` | right |
| 13 | `**` power | right |
| 14 | `.` `?.` `[]` call `()` | left (postfix) |
| 15 | literals, identifiers, grouping, containers, lambdas, `new` | — |

A common source of bugs is that `**` binds tighter than unary minus, so
`-2 ** 2` parses as `-(2 ** 2)` = `-4`, not `(-2) ** 2` = `4`. Use
parentheses when in doubt.

### Worked examples

```zen
print(2 + 3 * 4)          # 14  (* before +)
print((2 + 3) * 4)        # 20
print(2 ** 3 ** 2)        # 512 (right associative: 3**2=9, 2**9=512)
print(-2 ** 2)            # -4
print((-2) ** 2)          # 4
print(1 < 2 and 3 < 4)    # true
print(1 < 2 ? "yes" : "no")  # yes
print(nil ?? "fallback")  # "fallback"
print(0 ?? "fallback")    # 0  (0 is not nullish)
print(false ?? true)      # false
```

## 3.13 Truthiness rules (Zen 3.0 canonical)

Zen's `if`, `while`, `and`, `or`, `not`, `!`, and the `TO_BOOL` bytecode
opcode all use a single "truthy" rule, matching the language spec
(`docs/04-value-and-types.md`):

> **Falsy:** `false`, `nil`, `null`, `0`, `0.0`, `""` (empty string),
> `bytes([])`, `[]`, `#{}`, `%{}`, `#()`.
> **Truthy:** every other value.

This matches Python-like truthiness and surprises beginners coming from
languages where `0` is falsy but collections are not — here both are falsy.

```zen
if 0 { print("never") }              # does not print — 0 is falsy
if "" { print("never") }             # does not print
if [] { print("never") }             # does not print
if #{} { print("never") }            # does not print
if false { print("never") }          # does not print
if nil { print("never") }            # does not print
if 42 { print("42 is truthy") }      # prints
if "hi" { print("non-empty truthy")} # prints
if [1] { print("non-empty truthy") } # prints
```

Use `bool(x)` for an explicit conversion; it follows the same rule.

If you want Python-style emptiness checks, be explicit:

```zen
func is_empty(x) {
    if x == nil { return true }
    if typeof(x) == "string" || typeof(x) == "list" || typeof(x) == "dict" || typeof(x) == "set" || typeof(x) == "tuple" {
        return len(x) == 0
    }
    return false
}
```

## 3.14 Equality: `==`, `===`, `is`

Zen distinguishes three comparison operators that are easy to confuse.

| Operator | Name | Semantics |
|----------|------|-----------|
| `==` | equality | structural/value equality with numeric promotion |
| `===` | strict equality | same type *and* same value |
| `is` | identity/plain equality | object identity for containers; value equality for scalars |
| `!=`, `!==`, `is not` | negations | — |

### Numeric promotion in `==`

`1 == 1.0` is `true`; `1 === 1.0` is `false` because the runtime types differ.

```zen
print(1 == 1.0)     # true
print(1 === 1.0)    # false
print("1" == 1)     # false (no implicit string→int)
print("1" === 1)    # false
print(true == 1)    # false (booleans are not integers)
```

### Container equality

Lists, tuples, dicts, and sets compare structurally with `==`. Dicts are
order-insensitive; sets are order-insensitive; lists and tuples are
order-sensitive.

```zen
print([1,2] == [1,2])                 # true
print([1,2] == [2,1])                 # false
print(#{a:1, b:2} == #{b:2, a:1})     # true
print(%{1,2} == %{2,1})               # true
print(#(1,2) == #(1,2))               # true
```

### Identity with `is`

For heap values (lists, dicts, sets, instances, functions) `is` compares
references. Two separately constructed equal lists are not `is`-identical.

```zen
let a = [1,2,3]
let b = [1,2,3]
let c = a
print(a == b)   # true
print(a is b)   # false
print(a is c)   # true
```

For scalars (int, float, bool, nil, string interning aside) `is` behaves like
`==`. Prefer `==` unless you specifically mean identity.

## 3.15 Short-circuit evaluation

`and`/`&&` and `or`/`||` short-circuit. The right operand is not evaluated
when the result is already determined. This is the basis of guard idioms and
also the reason side effects on the right may not occur.

```zen
func boom() { throw "boom" }

# safe: boom() never runs because the left side is false
if false and boom() { print("no") }

# safe: boom() never runs because the left side is true
if true or boom() { print("yes") }

# nil guarding
let name = nil
if name != nil and len(name) > 0 { print(name) }
```

`??` is also short-circuiting: it returns the left operand unless it is
`nil`/`null`, in which case it evaluates and returns the right operand.

```zen
let port = config_port ?? 8080
```

`??=` assigns only when the target is currently nullish:

```zen
var count = nil
count ??= 0
count ??= 10
print(count)   # 0 — the second assignment is skipped
```

## 3.16 Chained comparisons

Comparisons chain Python-style: `a < b < c` means `(a < b) and (b < c)` and
evaluates `b` only once.

```zen
let x = 5
print(1 < x < 10)     # true
print(1 < x > 3)      # true
print(10 < x < 20)    # false
```

## 3.17 Bitwise operators

Bitwise operators work on integers. Negative integers use two's complement
semantics for shifts and masks.

| Operator | Meaning | Example | Result |
|----------|---------|---------|--------|
| `&` | and | `0b1100 & 0b1010` | `0b1000` = 8 |
| `|` | or | `0b1100 | 0b1010` | `0b1110` = 14 |
| `^` | xor | `0b1100 ^ 0b1010` | `0b0110` = 6 |
| `~` | not | `~0` | `-1` |
| `<<` | left shift | `1 << 4` | `16` |
| `>>` | arithmetic right shift | `-8 >> 1` | `-4` |

### Bit tricks

```zen
func is_even(n) { return (n & 1) == 0 }
func is_odd(n)  { return (n & 1) == 1 }

func set_bit(n, i)    { return n | (1 << i) }
func clear_bit(n, i)  { return n & ~(1 << i) }
func toggle_bit(n, i) { return n ^ (1 << i) }
func test_bit(n, i)   { return (n & (1 << i)) != 0 }

func low_mask(bits)   { return (1 << bits) - 1 }
func popcount(n) {
    let c = 0
    var x = n
    while x != 0 { c += (x & 1); x = x >> 1 }
    return c
}
```

### Shifts and edge cases

- Shifting by a negative amount is an error.
- Large shift counts are defined by the runtime; do not rely on wraparound.
- `>>` is arithmetic for signed values (sign extension).

```zen
print(1 << 10)    # 1024
print(1024 >> 3)  # 128
print(-8 >> 1)    # -4
print(~5)         # -6
```

## 3.18 Range operators

Two range operators exist:

| Operator | Meaning | Example | Values |
|----------|---------|---------|--------|
| `..` | exclusive end | `0..5` | 0,1,2,3,4 |
| `->` | inclusive end | `0->5` | 0,1,2,3,4,5 |

`->` auto-descends when the start is greater than the end.

```zen
for i in 0..3 { print(i) }   # 0 1 2
for i in 0->3 { print(i) }   # 0 1 2 3
for i in 5->2 { print(i) }   # 5 4 3 2
```

Ranges used for slicing follow the same end-exclusivity as `..`; see the
strings and lists chapters.

## 3.19 Compound assignment operators

All compound assignments evaluate the target once.

| Operator | Equivalent |
|----------|------------|
| `+=` | `x = x + y` |
| `-=` | `x = x - y` |
| `*=` | `x = x * y` |
| `/=` | `x = x / y` |
| `%=` | `x = x % y` |
| `??=` | `x = x ?? y` |
| `&=` | `x = x & y` |
| `|=` | `x = x | y` |
| `^=` | `x = x ^ y` |
| `<<=` | `x = x << y` |
| `>>=` | `x = x >> y` |

`++` and `--` increment/decrement by one, prefix or postfix.

```zen
var i = 0
print(i++)   # 0, then i becomes 1
print(++i)   # 2
print(i--)   # 2, then i becomes 1
print(--i)   # 0
```

## 3.20 Type coercion rules in operators

| Left | Op | Right | Result |
|------|----|-------|--------|
| int | `+ - * / % **` | int | int (except `/` → float when not exact? see note) |
| int | `+ - * / % **` | float | float |
| float | any arith | int/float | float |
| string | `+` | string | string |
| string | `+` | any | string (right stringified) |
| any | `+` | string | string (left stringified) |
| string | `*` | int | repeated string |
| list | `+` | list | concatenated list |
| list | `*` | int | repeated list |
| int | `& | ^ << >> ~` | int | int |
| nil | `??` | any | any |
| any | `==` | any | bool |

**Note on `/`:** division always produces a float in Zen, even for exact
integer division. `4 / 2` is `2.0`, while `4 // 2` is `2`.

```zen
print(4 / 2)    # 2.0
print(4 // 2)   # 2
print(4 / 2 == 2)  # true (numeric promotion)
print(typeof(4 / 2))  # "float"
```

## 3.21 Optional chaining and null safety

`?.` accesses a field or calls a method only if the receiver is not nil;
otherwise the whole expression is nil.

```zen
let user = nil
print(user?.name)          # nil
print(user?.profile?.age)  # nil

let u = #{name: "Ada", profile: #{age: 36}}
print(u?.profile?.age)     # 36
print(u?.missing?.deep)    # nil
```

Optional chaining composes with `??` for defaulting:

```zen
let city = user?.address?.city ?? "Unknown"
```

## 3.22 Membership: `in` and `not in`

`in` tests membership for strings (substring), lists (element), sets
(element), dicts (key), and tuples (element). Ranges support `in` by value.

```zen
print("ell" in "hello")     # true
print(3 in [1,2,3])         # true
print(4 not in [1,2,3])     # true
print("a" in #{a: 1})       # true
print(1 in %{1,2})          # true
print(2 in 0..5)            # true
print(9 in 0..5)            # false
```

## 3.23 Interaction of `and`/`or` with non-booleans

Unlike some languages, Zen's `and`/`or` return **booleans**, not the operands.
If you want Python-style operand return values use the ternary operator or
`??`.

```zen
print(1 and 2)     # true
print(nil or "x")  # true
# to get "x" when a is nil:
let a = nil
print(a ?? "x")    # "x"
```

## 3.24 Common operator mistakes

1. **Assuming `and` returns an operand.** It returns `bool`.
2. **Using `/` when integer division is intended.** Use `//`.
3. **Confusing `==` and `is`.** Containers compare by value with `==`.
4. **Forgetting that `0`, `0.0`, `""`, `[]`, `#{}` are falsy** (like Python).
5. **Precedence of `??` vs `or`.** `??` binds tighter than `or`, looser than `and`.
6. **Chained assignment.** `a = b = 1` is not supported; assign separately.
7. **Modulo of negatives.** Check the sign convention for your use case.
8. **String repetition with huge counts.** Can exhaust memory; validate inputs.
9. **Relying on float equality.** Use an epsilon comparison.
10. **Bitwise ops on non-integers.** They raise a type error.

## 3.25 Operator overloading in classes

Classes may define dunder-style methods to control operators. The exact set
supported depends on the runtime; the common ones are `__add__`, `__sub__`,
`__mul__`, `__truediv__`, `__floordiv__`, `__mod__`, `__pow__`, `__eq__`,
`__lt__`, `__le__`, `__gt__`, `__ge__`, `__str__`, `__len__`, `__getitem__`,
`__setitem__`, `__contains__`, `__call__`.

```zen
class Vec2 {
    var x = 0.0
    var y = 0.0
    func init(x, y) { self.x = x; self.y = y }
    func __add__(other) { return new Vec2(self.x + other.x, self.y + other.y) }
    func __eq__(other) { return self.x == other.x and self.y == other.y }
    func __str__() { return "Vec2(${self.x}, ${self.y})" }
}

let a = new Vec2(1.0, 2.0)
let b = new Vec2(3.0, 4.0)
print(a + b)        # Vec2(4.0, 6.0)
print(a == b)       # false
```

Check the classes chapter for the authoritative list supported by the VM.

## 3.26 Exercises

1. Without running the code, compute `2 + 3 * 4 ** 2 // 5 % 3` and then verify.
2. Write a function that swaps two variables without a temporary using XOR.
3. Implement `sign(x)` returning -1, 0, or 1 using comparisons and ternary.
4. Use `??` to build a config lookup with nested defaults.
5. Explain why `if 0 { ... }` runs, and rewrite a Python-style check correctly.
6. Write `clamp(x, lo, hi)` using only comparisons and ternary.
7. Implement `is_power_of_two(n)` with a bit trick.
8. Show the difference between `==`, `===`, and `is` for `1`, `1.0`, and lists.
9. Use chained comparisons to validate a score in `[0, 100]`.
10. Use a range with `->` to print a countdown.
11. Implement a tiny bignum-less hash using `<<`, `^`, and `*` primes.
12. Write `safe_div(a, b)` returning `nil` on division by zero using `try`.
