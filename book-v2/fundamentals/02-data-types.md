# Chapter 2: Data Types

Zen is a dynamically typed, strongly typed language. Every value carries a type tag at runtime, operations check operand types, and implicit conversions are minimal and explicit. This chapter is the definitive reference for every literal form, every type, every coercion, and every edge case in Zen's value system.

## 2.1 The type system at a glance

Zen values are 64-bit NaN-boxed tagged unions. There are **16 language types**:

| # | Type | Kind | Storage | `typeof()` returns |
|---|------|------|---------|--------------------|
| 1 | `nil` | primitive | inline | `"nil"` |
| 2 | `bool` | primitive | inline | `"bool"` |
| 3 | `int` | primitive | inline (i64) | `"int"` |
| 4 | `float` | primitive | inline (f64) | `"float"` |
| 5 | `string` | heap | `Arc<str>` | `"string"` |
| 6 | `bytes` | heap | `Arc<[u8]>` | `"bytes"` |
| 7 | `list` | heap | `Vec<Value>` | `"list"` |
| 8 | `dict` | heap | `IndexMap` | `"dict"` |
| 9 | `set` | heap | `IndexSet` | `"set"` |
| 10 | `tuple` | heap | `Vec<Value>` | `"tuple"` |
| 11 | `func` | heap | `FuncObj` | `"func"` |
| 12 | `class` | heap | `ClassObj` | `"class"` |
| 13 | `instance` | heap | `InstObj` | `"instance"` |
| 14 | `cell` | heap | `RefCell<Value>` | `"cell"` |
| 15 | `error` | heap | `ErrObj` | `"error"` |
| 16 | `foreign` | heap | `ForeignObj` | `"foreign"` |
| 17 | `channel` | heap | `ChanObj` | `"channel"` |
| 18 | `mutex` | heap | `MutexObj` | `"mutex"` |
| 19 | `handle` | heap | `HandleObj` | `"handle"` |

> Note: internally there are 16 machine tags over a NaN-boxed `u64`; small primitives (`nil`, `bool`, `int`, `float`) are stored inline in the payload bits. Larger values (strings, lists, dicts, etc.) are heap objects with manual reference counting (`retain`/`release`).

### Type introspection

```zen
typeof(42)        # "int"
typeof(3.14)      # "float"
typeof("hello")   # "string"
typeof(true)      # "bool"
typeof(nil)       # "nil"
typeof([1, 2])    # "list"
typeof({"a": 1})  # "dict"
typeof(fn() {})   # "func"
```

`is` checks type membership:

```zen
42 is int         # true
"hello" is string # true
nil is nil        # true
42 is string      # false
```

`is` compares against a type *name* (a string literal or value), not against another value's type:

```zen
let x = 42
x is typeof(x)    # true — compares "int" is "int"
```

## 2.2 Literals

### Nil and booleans

```zen
let nothing = nil
let yes = true
let no = false
```

`nil` is the absence of a value. It is falsy. `true`/`false` are the only boolean values.

### Integers

Integer literals are 64-bit signed (`i64`). Decimal, hex (`0x`), octal (`0o`), and binary (`0b`) bases are supported. Underscores may be used as digit separators.

```zen
let dec = 42
let hex = 0x2A          # 42
let oct = 0o52          # 42
let bin = 0b101010      # 42
let big = 1_000_000_000 # 1000000000
let neg = -17
```

Integer range: `-9,223,372,036,854,775,808` to `9,223,372,036,854,775,807` (`i64::MIN` to `i64::MAX`). Overflow in checked contexts panics; in release builds overflow wraps (two's complement).

```zen
let max = 9223372036854775807   # i64::MAX
let min = -9223372036854775808  # i64::MIN
```

### Floats

Float literals are 64-bit IEEE-754 (`f64`). A decimal point or exponent is required.

```zen
let pi = 3.14159
let big = 1.5e10      # 15000000000.0
let small = 2.5e-3    # 0.0025
let hex_f = 0x1.8p1   # 3.0 (hex float)
```

Special values:

```zen
let inf = 1e400       # overflow → infinity
let neg_inf = -1e400  # −infinity
let nan = 0.0 * inf   # not-a-number
```

NaN semantics:
- `nan != nan` is `true` (IEEE-754)
- `nan < anything` is `false`; `nan > anything` is `false`
- `typeof(nan)` → `"float"`
- Use `x != x` to test for NaN: `let is_nan = (x != x)`

```zen
let bad = 0.0 / 0.0   # NaN
print(bad == bad)     # false
print(bad != bad)     # true  ← NaN detection idiom
print(inf > 1e308)    # true
print(-inf < -1e308)  # true
```

### Strings

String literals use double or single quotes. Escape sequences: `\n`, `\t`, `\\`, `\'`, `\"`, `\0`, `\xHH` (hex byte), `\u{HHHH}` (Unicode scalar).

```zen
let s1 = "hello"
let s2 = 'world'
let s3 = "it's a nice day"
let s4 = "say \"hello\""
let s5 = "line1\nline2\tline3"
let s6 = "byte: \x41"    # "byte: A"
let s7 = "emoji: \u{1F600}"  # "emoji: 😀"
```

Triple-quoted strings (multi-line, raw):

```zen
let poem = """Zen is fast,
native, and safe.
No escaping needed here."""
```

Strings are immutable (`Arc<str>`). Indexing `s[i]` returns the character at byte offset `i` as a one-character string; slicing `s[a:b]` returns a substring. Indexing out of bounds raises `IndexError`.

```zen
let s = "Zen"
print(s[0])   # "Z"
print(s[-1])  # "n"
print(s[0:2]) # "Ze"
```

### Bytes

Byte literals use the `b""` prefix. Each element is a `u8` (0–255).

```zen
let data = b"\x48\x65\x6c\x6c\x6f"  # "Hello" as bytes
let empty = b""
```

Bytes support indexing (`data[0]` → `72` as int) and slicing. Bytes are *not* automatically convertible to strings; use the `string` module or explicit decoding.

### Lists

List literals use square brackets. Heterogeneous contents are allowed.

```zen
let nums = [1, 2, 3]
let mixed = [1, "two", 3.0, true, nil]
let nested = [[1, 2], [3, 4]]
let empty = []
```

Lists are mutable, ordered, and allow duplicates. Indexing supports negative indices (from the end). Slicing `l[a:b]` returns a new list (copy, not a view).

```zen
let l = [10, 20, 30, 40, 50]
print(l[0])    # 10
print(l[-1])   # 50
print(l[1:4])  # [20, 30, 40]
l[2] = 99      # mutation
print(l)       # [10, 20, 99, 40, 50]
```

### Dictionaries

Dict literals use `{key: value, ...}`. Keys must be strings. Values can be any type. Insertion order is preserved.

```zen
let person = {"name": "Zen", "version": 3, "active": true}
let nested = {"user": {"name": "Zen", "id": 1}, "tags": ["a", "b"]}
let empty = {}
```

Access `d["key"]`. Missing keys raise `KeyError`.

```zen
print(person["name"])   # Zen
person["version"] = 4   # update
person["lang"] = "Zen"  # add
print(person["missing"]) # KeyError
```

### Sets

Set literals use `{value, value, ...}` with no colons. Elements must be hashable (strings, ints, floats, bools, tuples of hashables). Sets are unordered and deduplicate.

```zen
let s = {1, 2, 2, 3}
print(s)  # {1, 2, 3} — order not guaranteed, duplicates removed
print(2 in s)   # true
print(5 in s)   # false
```

> Warning: `{}` alone is an empty *dict*, not an empty set. Use `set()` or `{1}` for a set.

### Tuples

Tuple literals use `#(...)` with hash prefix. Immutable, ordered, heterogeneous.

```zen
let point = #(3, 4)
let pair = #("name", 42)
let single = #(5,)      # trailing comma required for 1-tuple
let empty = #()         # empty tuple
```

Access `t[0]`, `t[1]`. Tuples support destructuring in `match` patterns.

```zen
let p = #(10, 20)
print(p[0])   # 10
print(len(p)) # 2
```

## 2.3 Type coercion matrix

Zen does **not** implicitly coerce between most types. The `+` operator stringifies non-string operands when the other side is a string, but arithmetic and comparison require matching numeric types.

| From → To | int | float | string | bool |
|-----------|-----|-------|--------|------|
| int | identity | `42 → 42.0` | `"42"` | `0 → false, else true` |
| float | `3.9 → 3` (trunc) | identity | `"3.9"` | `0.0 → false, else true` |
| string | `"42" → 42` (parse) | `"3.14" → 3.14` | identity | `"" → false, else true` |
| bool | `true → 1, false → 0` | `true → 1.0` | `"true"/"false"` | identity |

Explicit conversion functions:

```zen
int("42")       # 42
int(3.9)        # 3 (truncates toward zero)
float(42)       # 42.0
str(42)         # "42"
str(3.14)       # "3.14"
bool(0)         # false
bool(1)         # true
bool("")        # false
bool("hello")   # true
```

String-to-number parsing failures raise `ValueError`:

```zen
try {
    int("not a number")
} catch as e {
    print(e.msg)  # ValueError: invalid literal for int()
}
```

## 2.4 Truthiness (Zen 3.0 canonical)

Zen uses Python-like truthiness. The falsy values are exactly:

`nil`, `false`, `null`, `0`, `0.0`, `""` (empty string), `bytes([])`, `[]`,
`#{}`, `%{}`, `#()`.

Everything else is truthy (non-zero numbers, non-empty strings, non-empty
collections, non-nil values).

| Value | Truthy? | Reason |
|-------|---------|--------|
| `nil` | false | explicitly falsy |
| `false` | false | explicitly falsy |
| `0` | false | zero is falsy |
| `0.0` | false | zero float is falsy |
| `""` | false | empty string is falsy |
| `[]` | false | empty collection is falsy |
| `#{}` | false | empty dict is falsy |
| `42` | true | non-zero |
| `"hello"` | true | non-empty |
| `[1, 2]` | true | non-empty |

Use `bool(x)` to coerce explicitly:

```zen
print(bool(0))      # false
print(bool(""))     # false
print(bool(nil))    # false
print(bool(false))  # false
print(bool(42))     # true
print(bool("hi"))   # true
print(bool([1]))    # true
```

Note the contrast with `??` (nullish coalescing): `??` only triggers on `nil`/`null`,
not on falsy values, so `0 ?? "default"` yields `0`, not `"default"`.

## 2.5 Equality vs identity

`==` compares *value* (with numeric coercion: `1 == 1.0` is true). `is` compares *type membership* when the right side is a type name. There is no separate object-identity operator for heap objects beyond reference behavior; two equal lists are `==` but distinct objects.

```zen
print(1 == 1.0)   # true (numeric coercion)
print(1 == "1")   # false (type mismatch)
print([] == [])   # true (empty lists equal by value)
print({} == {})   # true (empty dicts equal by value)
```

Dict equality is order-insensitive:
```zen
print({"a": 1, "b": 2} == {"b": 2, "a": 1})  # true
```

## 2.6 Numeric edge cases

### Infinity and NaN

```zen
let inf = 1e400          # +infinity
let neg_inf = -1e400     # −infinity
let nan = 0.0 * inf      # not-a-number

print(inf > 1e308)       # true
print(neg_inf < -1e308)  # true
print(nan == nan)        # false
print(nan != nan)        # true   ← NaN detection
print(inf + inf)         # inf
print(inf - inf)         # nan
print(0.0 * inf)         # nan
```

### Integer overflow

```zen
let max = 9223372036854775807
print(max + 1)  # wraps to -9223372036854775808 (two's complement, release)
```

### Division

```zen
print(7 / 2)    # 3.5 (float division)
print(7 // 2)   # 3 (floor division, integer result)
print(7 % 2)    # 1 (modulo, sign follows dividend)
print(-7 % 2)   # -1
print(-7 // 2)  # -4 (floor toward −inf)
```

Division by zero raises `DivideByZero`:
```zen
try {
    let x = 1 / 0
} catch as e {
    print(e.msg)  # DivideByZero: division by zero
}
```

## 2.7 Type conversion reference

```zen
# Number ↔ string
int("42")           # 42
int("3.9")          # 3 (truncates after float parse)
int(3.9)            # 3
float("3.14")       # 3.14
float(42)           # 42.0
str(42)             # "42"
str(3.14)           # "3.14"
str(true)           # "true"
str(nil)            # "nil"

# Boolean
bool(0)             # false
bool(1)             # true
bool(0.0)           # false
bool("")            # false
bool([])            # false
bool({})            # false
bool(nil)           # false
bool(false)         # false

# Collections
list("abc")         # ["a", "b", "c"]
tuple([1, 2])       # #(1, 2)
set([1, 1, 2])      # {1, 2}
dict([["a", 1]])    # {"a": 1}
```

## 2.8 Common pitfalls

- **`0`, `0.0`, `""`, `[]`, `#{}` are falsy** in Zen (like Python). Use `if x:` to test truthiness; `x == 0` only tests the value zero.
- **`nan != nan`**. Use `x != x` to detect NaN.
- **`{}` is an empty dict**, not a set. Use `{1}` or `set()` for sets.
- **Strings are immutable**. `s[0] = "X"` raises `TypeError`. Build new strings with concatenation.
- **Integer division `//` floors toward negative infinity**: `-7 // 2` is `-4`, not `-3`.
- **List slicing copies**: `l[1:3]` is a new list; mutating it doesn't affect the original.
- **Dict keys must be strings**: `d[42]` raises `TypeError`. Use `d["42"]`.

## 2.9 Quick reference

```zen
let x = 42
let f = 3.14
let s = "hello"
let b = true
let n = nil
let l = [1, 2, 3]
let d = {"a": 1}
let st = {1, 2}
let t = #(1, 2)

typeof(x)      # "int"
x is int       # true
x is string    # false
bool(x)        # true
str(x)         # "42"
int("42")      # 42
float(x)       # 42.0
len(l)         # 3
len(d)         # 1
len(s)         # 5
x == 42.0      # true
x == "42"      # false
```

## 2.10 Exercises

1. Write `is_nan(x)` that returns true only for NaN (hint: `x != x`).
2. Write `safe_int(s)` that parses a string to int, returning `nil` on failure instead of raising.
3. Demonstrate that `0` is falsy: confirm `if 0 { ... }` does NOT execute.
4. Create a list, slice it, mutate the slice, and confirm the original is unchanged.
5. Show that `{"a": 1, "b": 2} == {"b": 2, "a": 1}` (dict order-insensitive equality).
6. Compute `-7 // 2` and `-7 % 2` and explain the results (floor division).
7. Write `type_name(x)` that returns a human-readable string for any value using `typeof()` and special-casing `nan`/`inf`.

## 2.11 Type coercion matrix (extended)

| From | To int | To float | To string | To bool | Notes |
|---|---|---|---|---|---|
| int | identity | widen | decimal | 0→false, else true |  |
| float | trunc toward zero | identity | repr | 0.0→false, NaN→true, else true | NaN is truthy |
| string | parse int (base 10) or error | parse float or error | identity | ""→false, else true |  |
| bool | true→1, false→0 | true→1.0,false→0.0 | "true"/"false" | identity |  |
| nil | error | error | "nil" | false |  |
| list | error | error | repr | empty→false, else true |  |
| dict | error | error | repr | empty→false, else true |  |
| set | error | error | repr | empty→false, else true |  |
| tuple | error | error | repr | empty→false, else true |  |
| bytes | error | error | repr | empty→false, else true |  |

**Note:** Falsy values are `nil`, `false`, `null`, `0`, `0.0`, `""`, `bytes([])`,
`[]`, `#{}`, `%{}`, `#()`. Everything else is truthy (including `nan`).

## 2.12 Bytes and buffers (deep dive)

`bytes` is an immutable sequence of bytes (values 0-255).

```zen
let b = bytes([65, 66, 67])  # "ABC"
print(b[0])                  # 65
let s = b.decode("utf-8")    # "ABC"
let b2 = "ABC".encode("utf-8")
print(len(b))
```

### bytes methods (common)
- `len(b)`, indexing/slicing
- `b.decode(encoding="utf-8", errors="strict")`
- `bytes([..])`, `b.hex()`, `b.fromhex(s)`

## 2.13 Tuples (immutable sequences)

```zen
let t = #(1, "a", true)
print(t[0], len(t))
# t[0] = 5  # error: immutable
```

Tuples are useful for fixed-size heterogeneous data and multiple return values.

## 2.14 Sets

```zen
let s = {1, 2, 2, 3}
print(s)      # {1,2,3} (order may vary)
print(2 in s) # true
s.add(4)
s.remove(2)
```

## 2.15 Numeric edge cases in depth

### Integer precision
Integers are arbitrary precision (big ints) in Zen 3.0 - no fixed 64-bit overflow in normal operations.

```zen
let big = 10 ** 100
print(big + 1)
```

### Float special values
```zen
let inf = 1.0 / 0.0      # inf
let ninf = -1.0 / 0.0    # -inf
let nan = 0.0 / 0.0      # nan
print(inf > 1000000)     # true
print(nan == nan)        # false (IEEE 754)
print(nan != nan)        # true
```

### Division semantics
- `/` is true division returning float (or appropriate numeric type)
- `//` is floor division
- `%` follows floor division (sign of divisor matters in some languages; check)

## 2.16 Type checking patterns

```zen
func describe(x) {
    if typeof(x) == "int" { return "integer" }
    if typeof(x) == "float" { return "float" }
    if typeof(x) == "string" { return "string" }
    if typeof(x) == "bool" { return "boolean" }
    if x is nil { return "nil" }
    if typeof(x) == "list" { return "list" }
    if typeof(x) == "dict" { return "dict" }
    if typeof(x) == "set" { return "set" }
    if typeof(x) == "tuple" { return "tuple" }
    return "unknown"
}
```

`is` operator checks type identity/instance; `typeof()` returns type name string.

## 2.17 Identity vs equality

- `==` structural/value equality
- `is` identity (same object/reference)

```zen
let a = [1,2,3]
let b = [1,2,3]
print(a == b)  # true (value equal)
print(a is b)  # false (different objects)
let c = a
print(a is c)  # true
```

## 2.18 Common pitfalls
1. Modifying a list while iterating over it can skip elements
2. Using mutable default arguments (if supported) - better to use nil sentinel
3. Dict key type errors - only strings work as keys
4. Integer/string concatenation requires explicit conversion
5. Truthiness confusion with 0/"" depending on language - always test
6. Float comparison with == is dangerous; use epsilon

```zen
func approx_eq(a, b, eps) {
    if eps == nil { eps = 1e-9 }
    return abs(a-b) < eps
}
```

## 2.19 Deep exercises
1. Implement a type coercion helper that tries int, then float, then bool for strings.
2. Write a function to detect NaN using `x != x`.
3. Build a mini type registry using dict mapping type names to handlers.
4. Demonstrate integer big number arithmetic with powers.
5. Show the difference between `is` and `==` with lists, strings, ints.
6. Create a tuple and try to mutate - observe error.
7. Implement `flatten_once(lst)` for nested lists.
8. Write `deep_copy` conceptually (lists of lists) - note shallow vs deep.
