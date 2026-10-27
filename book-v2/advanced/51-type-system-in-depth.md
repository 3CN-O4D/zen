# Chapter 51: Zen Type System In Depth

*A detailed look at Zen 3.0's types, type tags, coercion, equality, identity,
and how types flow through the runtime.*

## 16.1 The type taxonomy

Zen 3.0 is dynamically typed: types are associated with values, not variables.
A variable can hold any type at any time (unless declared `const`).

### Primitive (immediate) types

| Type | Tag | Literal example | Notes |
|------|-----|-----------------|-------|
| `nil` | nil | `nil`, `null` | absence of value; falsy |
| `bool` | boolean | `true`, `false` | logical values |
| `int` | integer | `42`, `0x2A`, `1_000` | arbitrary-precision in runtime; literals ≤ i48 |
| `float` | floating-point | `3.14`, `1e-10` | IEEE-754 double |
| `string` | text | `"hi"`, `'hi'`, `"""..."""` | immutable, UTF-8 |

### Container (heap) types

| Type | Tag | Literal example | Notes |
|------|-----|-----------------|-------|
| `list` | list | `[1, 2, 3]` | ordered, mutable, heterogeneous |
| `dict` | dict | `#{a: 1}` | string keys → any value |
| `set` | set | `%{1, 2}` | unordered unique values |
| `tuple` | tuple | `#(1, "x")` | fixed-size, immutable, heterogeneous |
| `bytes` | bytes | `bytes([1,2])` | immutable byte sequence |

### Other runtime types

| Type | Example | Notes |
|------|---------|-------|
| `func` | `fn(x) => x` | function value / closure |
| `class` | `class C {}` | class object |
| `instance` | `new C()` | object instance |
| `module` | `import math` | loaded module |
| `foreign` | FFI handle | opaque pointer to C resource |

`typeof(x)` returns the type name as a string; `type(x)` is an alias.

## 16.2 Type tags and immediate vs heap

Every runtime `Value` carries a tag. Immediate values (nil, bool, int, float)
fit inside the `Value` itself and are copied on assignment. Heap values
(string, list, dict, set, tuple, bytes, instance, func, module, foreign) are
pointers to heap-allocated objects and are copied by reference.

Consequence: two list literals `[1,2]` and `[1,2]` are `==` (equal by value)
but not `is` (different heap objects).

```zen
let a = [1, 2]
let b = [1, 2]
print(a == b)   # true
print(a is b)   # false
```

## 16.3 Type introspection

| Operation | Purpose | Example |
|-----------|---------|---------|
| `typeof(x)` | type name string | `typeof(42)` → `"int"` |
| `type(x)` | alias for typeof | `type("x")` → `"string"` |
| `x is T` | identity / instance check | `x is list` |
| `x is not T` | negation | `x is not nil` |
| `typeof(x) == "int"` | string comparison | works but slower than `is` |

Prefer `is` for type checks when supported; fall back to `typeof() == "..."`.

## 16.4 Type coercion

### Explicit conversion (builtins)

```zen
int("42")       # 42
float(3)        # 3.0
str(42)         # "42"
bool(0)         # true   (0 is truthy!)
list("ab")      # ["a","b"]
tuple([1,2])    # #(1,2)
set([1,1,2])    # {1,2}
dict([["a",1]]) # {"a":1}
```

String → number parsing failures raise `ValueError` (Z0305).

### Implicit coercion in operators

| Context | Rule |
|---------|------|
| `+` with a string | the other operand is stringified via `str()` |
| `*` string × int | string repetition |
| comparison `==` | numeric promotion: int and float compare numerically |
| truthiness (`if`, `and`, `or`) | only `false`/`nil`/`null` are falsy |

```zen
print("age: " + 25)     # "age: 25"
print("hi" * 3)         # "hihihi"
print(1 == 1.0)         # true
print(1 === 1.0)        # false (strict: types differ)
```

## 16.5 Equality: `==`, `===`, `is`

| Operator | Semantics | `1 == 1.0` | `[1] == [1]` | `[1] is [1]` |
|----------|-----------|------------|--------------|--------------|
| `==` | value equality with promotion | true | true | — |
| `===` | strict equality (type + value) | false | — | — |
| `is` | identity (same object) | true | false | false |

Container equality:
- lists/tuples: order-sensitive, element-wise
- dicts: order-insensitive, key/value match
- sets: order-insensitive, element match

## 16.6 Numeric details

### Integers

Runtime integers are arbitrary precision (big ints), so `10 ** 100` works
without overflow. Integer *literals* are limited to i48 range at parse time
(Z0106 `NumericLiteralOverflow` for larger literals); build large values at
runtime instead.

```zen
let big = 10 ** 100     # fine (runtime)
# let x = 99999999999999999999   # Z0106 literal overflow
```

### Floats

IEEE-754 double precision. Special values:
- `inf` = `1.0 / 0.0`
- `-inf` = `-1.0 / 0.0`
- `nan` = `0.0 / 0.0`

IEEE rules:
- `nan == nan` is `false`
- `nan != nan` is `true`
- comparisons with `nan` are `false` (except `!=`)
- `inf > 1e308` is `true`

```zen
print(nan == nan)   # false
print(math.isnan(nan))   # true
print(math.isinf(inf))   # true
```

### Division

- `/` true division → float (`4 / 2` is `2.0`)
- `//` floor division → int (`4 // 2` is `2`, `-7 // 2` is `-4`)
- `%` modulo (sign follows dividend in Zen)

```zen
print(4 / 2)    # 2.0
print(4 // 2)   # 2
print(-7 // 2)  # -4
print(-7 % 2)   # -1
```

## 16.7 String type details

Strings are immutable sequences of Unicode code points. Indexing and slicing
operate on *characters*, not bytes.

```zen
let s = "Zën"
print(len(s))   # 3 (code points, not bytes)
print(s[1])     # "ë"
```

Convert to/from bytes explicitly:
```zen
let b = s.encode("utf-8")
let s2 = b.decode("utf-8")
```

## 16.8 Container type details

### List
Ordered, mutable, heterogeneous. Supports indexing, slicing (copies), push,
pop, insert, remove.

### Dict
Unordered mapping string → value. Keys must be strings. Order-insensitive
equality. Iteration yields keys; use `.items()` for pairs.

### Set
Unordered collection of unique values. Order-insensitive equality.

### Tuple
Fixed-size, immutable, heterogeneous. Good for fixed records and multiple
return values.

## 16.9 Type stability and performance

Dynamic typing means every operation pays a type-dispatch cost. The JIT
mitigates this with inline caches for hot call sites. For performance:
- Keep types stable at a call site (don't alternate int/string in the same
  variable).
- Avoid `typeof()` checks in hot loops.
- Prefer homogeneous containers (a list of all ints) when possible.

## 16.10 Common type mistakes

1. **Assuming `0` is falsy.** It's truthy in Zen. Use `x == nil` or `x is nil`.
2. **Confusing `==` and `is`.** Use `==` for value, `is` for identity.
3. **Comparing `nan` with `==`.** Always false. Use `math.isnan`.
4. **Dict keys must be strings.** `d[42]` raises TypeError; use `d["42"]`.
5. **Integer literals beyond i48.** Cause Z0106; compute at runtime.
6. **Mutable default arguments.** Use `nil` sentinel.
7. **Assuming containers copy on assignment.** `b = a` shares the object.

## 16.11 Exercises

1. Write `type_name(x)` returning a human-readable string for any value.
2. Demonstrate `nan != nan` and how to detect NaN (`x != x`).
3. Show `1 == 1.0` but `1 === 1.0` is false.
4. Prove two equal lists are not `is`-identical.
5. Write `safe_int(s)` returning nil on parse failure.
6. Demonstrate `-7 // 2` is `-4` (floor, not truncation).
7. Write `is_power_of_two(n)` using bit tricks.
8. Show `typeof` on each of: int, float, string, bool, nil, list, dict,
   set, tuple, func.
9. Implement `deep_equals(a, b)` handling nested containers.
10. Explain why `bool([])` is `true` in Zen.

'''
print('type system done')

## 16.12 The machine-tag view of every type

Zen is dynamically typed: types travel with values, not variables. Every
runtime `Value` is a NaN-boxed 64-bit word (see Chapter 50). The machine tag
decides the fast path; heap objects add a `ObjHeader.kind` discriminant.

### Immediate (in-`Value`) types

| Type | Tag | Payload | `typeof()` | Falsy? |
|------|-----|---------|------------|--------|
| `nil` / `null` | `0b000` | none | `"nil"` | yes |
| `false` | `0b001` | none | `"bool"` | yes |
| `true` | `0b010` | none | `"bool"` | yes |
| `int` | `0b011` | i48, sign-extended | `"int"` | `0` only |
| `float` | `0b100` | raw f64 bits | `"float"` | `0.0` only (NaN is truthy) |

### Thin-pointer types (no `ObjHeader`)

| Type | Tag | Payload | `typeof()` | Falsy? |
|------|-----|---------|------------|--------|
| `string` | `0b101` | `*const u8` (first byte) | `"string"` | `""` only |
| `bytes` | `0b110` | `*const u8` (first byte) | `"bytes"` | empty only |

Length and refcount live behind the thin pointer (`ptr − 8` for length), so
`is_string()` / `is_bytes()` is a tag compare with **zero loads**.

### Heap types (tag `0b111`, `ObjHeader.kind` discriminates)

| Type | `ObjHeader.kind` | `typeof()` | Falsy? |
|------|------------------|------------|--------|
| `list` | 0 List | `"list"` | empty |
| `dict` | 1 Dict | `"dict"` | empty |
| `set` | 2 Set | `"set"` | empty |
| `tuple` | 3 Tuple | `"tuple"` | empty |
| `func` | 4 Func | `"func"` | no |
| `instance` | 5 Instance | `"instance"` | no |
| `error` | 6 Error | `"error"` | no |
| `channel` | 7 Channel | `"channel"` | no |
| `mutex` | 8 Mutex | `"mutex"` | no |
| `handle` | 9 Handle | `"handle"` | no |

### Honest types (ADR-0004)

A `handle` reports `"handle"`, never `"int"`. The old blueprint's
`typeof(handle) == "int"` was wrong and was corrected. `typeof(x)` returns
the honest language-level name.

## 16.13 Truthiness — canonical rule

Falsy: `nil`, `false`, `null`, `0`, `0.0`, `""`, `bytes([])`, `[]`, `#{}`,
`%{}`, `#()`.
Truthy: everything else — including `nan`, non-zero numbers, non-empty
strings, non-empty collections.

This matches Python-like truthiness. `if x:`, `and`, `or`, `not`, and the
`TO_BOOL` opcode all use this rule.

Important contrast with `??` (nullish coalescing): `??` triggers only on
`nil`/`null`, not on falsy values. So `0 ?? "default"` yields `0`, and
`"" ?? "default"` yields `""`.

```zen
print(bool(0))        # false
print(bool(0.0))      # false
print(bool(""))       # false
print(bool([]))       # false
print(bool(#{}))      # false
print(bool(false))    # false
print(bool(nil))      # false
print(bool(float("nan")))   # true   (NaN is truthy)
print(0 ?? "default") # 0        (? is a nil-default, not a falsy-default)
```

## 16.14 Type coercion

### Explicit conversion builtins

| Expression | Meaning | Example | Result |
|------------|---------|---------|--------|
| `int(x)` | parse/truncate to int | `int("42")` | `42` |
| `int(3.9)` | truncate toward zero | `int(3.9)` | `3` |
| `float(x)` | to float | `float(42)` | `42.0` |
| `str(x)` | to string | `str(42)` | `"42"` |
| `bool(x)` | truthiness | `bool(0)` | `false` |
| `list(x)` | to list | `list("ab")` | `["a","b"]` |
| `tuple(x)` | to tuple | `tuple([1,2])` | `#(1,2)` |
| `set(x)` | to set | `set([1,1,2])` | `{1,2}` |
| `dict(x)` | from pairs | `dict([["a",1]])` | `{"a":1}` |

String→number parse failures raise `ValueError` (Z0305).

### Implicit coercion in operators

| Context | Rule |
|---------|------|
| `+` with a string | other operand stringified via `str()` |
| `*` string × int | string repetition (`"x" * 3 == "xxx"`) |
| `*` list × int | list repetition |
| `==` int vs float | numeric promotion (`1 == 1.0` is true) |
| truthiness (`if`, `and`, `or`) | canonical rule above |

```zen
print("age: " + 25)     # "age: 25"
print("hi" * 3)         # "hihihi"
print(1 == 1.0)         # true
print(1 === 1.0)        # false (strict: types differ)
```

## 16.15 Equality: `==`, `===`, `is`

| Operator | Semantics | `1 == 1.0` | `[1] == [1]` | `[1] is [1]` |
|----------|-----------|------------|--------------|--------------|
| `==` | value equality, numeric promotion | true | true | — |
| `===` | strict equality (type + value) | false | — | — |
| `is` | identity (same object / pointer) | true (scalars) | false | false |

Container equality rules:
- lists/tuples: order-sensitive, element-wise
- dicts: order-insensitive, key/value match
- sets: order-insensitive, element match

For heap values `is` compares pointers. Two separately constructed equal
lists are `==` but not `is`. For scalars (`int`, `float`, `bool`, `nil`) `is`
behaves like `==`.

## 16.16 Numeric semantics in depth

### Integers

- i48, sign-extended, range ±140,737,488,355,327.
- Overflow promotes to float; never wraps.
- `10 / 4 == 2.5` (true division). `10 // 4 == 2` (floor, toward −∞).
- `-7 // 2 == -4`, `-7 % 3 == -1` (modulo follows the dividend sign, legacy
  `fmod` behaviour).
- `%` by zero raises `ZeroDivisionError` (Z0304), checked before the op.

### Floats

- IEEE-754 double precision.
- `nan == nan` is false; `nan != nan` is true. Detect NaN with `x != x`.
- `inf = 1.0 / 0.0`, `-inf = -1.0 / 0.0`.
- `math.isnan`, `math.isfinite`, `math.isinf` for classification.

```zen
print(nan == nan)      # false
print(nan != nan)      # true
print(0.0 / 0.0 != nan)  # depends on canonicalisation; use math.isnan
```

## 16.17 String semantics

Strings are immutable sequences of Unicode code points. Indexing/slicing
operate on characters, not bytes.

```zen
let s = "Zën"
print(len(s))   # 3 (code points), not the UTF-8 byte count
print(s[1])     # "ë"
```

Convert explicitly: `s.encode("utf-8")` → bytes; `b.decode("utf-8")` → string.

String comparison is lexicographic by code point; uppercase sorts before
lowercase. For case-insensitive: `a.lower() == b.lower()`.

## 16.18 Type stability and JIT performance

Dynamic typing means every op pays a type-dispatch cost. The JIT (Chapter 54)
mitigates this with inline caches guarded by `TYPE_GUARD`. For performance:

- Keep types stable at a call site (don't alternate int/string in one
  variable).
- Avoid `typeof()` in hot loops; prefer `is`.
- Prefer homogeneous containers (a list of all ints).
- A monomorphic `d.a` on a shaped instance is a load + compare + predicted
  branch; a polymorphic or dynamic-key `d[k]` is a hash probe with no IC help.

## 16.19 Common type mistakes

1. `0`, `0.0`, `""`, `[]`, `#{}` are falsy (like Python), not truthy.
2. `nan == nan` is false; use `math.isnan(x)` or `x != x`.
3. Dict keys must be strings: `d[42]` raises TypeError; use `d["42"]`.
4. `int` literals beyond i48 are a lexer error (Z0106).
5. `int / int` → float; use `//` for integer division.
6. `b = a` shares a heap object; use `list(a)` / `deep_copy` to copy.
7. `==` is value, `is` is identity — don't confuse them for lists.
8. Strings are immutable; `s[i] = x` errors.

## 16.20 Exercises

1. Write `type_name(x)` returning a human-readable string for any value.
2. Detect NaN with `x != x`; confirm `math.isnan` agrees.
3. Show `1 == 1.0` but `1 === 1.0` is false; `true == 1` is false.
4. Prove two equal lists are `==` but not `is`.
5. Confirm `0`, `0.0`, `""`, `[]`, `#{}` are falsy; `nan` is truthy.
6. Show `0 ?? "default"` yields `0` (nil-default, not falsy-default).
7. Implement `safe_int(s)` returning nil on parse failure.
8. Show `-7 // 2 == -4` (floor) and `-7 % 3 == -1` (dividend sign).
9. `typeof()` on each of: int, float, string, bool, nil, list, dict, set,
   tuple, func, instance, error, channel, mutex, handle.
10. Implement `deep_equals(a, b)` handling nested containers.

