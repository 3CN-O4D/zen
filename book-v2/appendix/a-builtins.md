# Appendix A: Built-in Functions

Zen's built-in functions are always available without importing. They cover type conversion, iteration, introspection, and basic I/O.

## A.1 Type conversion

| Function | Description | Example | Result |
|----------|-------------|---------|--------|
| `int(x)` | Convert to integer | `int("42")` | `42` |
| `float(x)` | Convert to float | `float("3.14")` | `3.14` |
| `str(x)` | Convert to string | `str(42)` | `"42"` |
| `bool(x)` | Convert to boolean | `bool(1)` | `true` |
| `list(x)` | Convert to list | `list("abc")` | `["a", "b", "c"]` |
| `tuple(x)` | Convert to tuple | `tuple([1, 2])` | `#(1, 2)` |
| `set(x)` | Convert to set | `set([1, 1, 2])` | `{1, 2}` |
| `dict(x)` | Convert from pairs | `dict([["a", 1]])` | `{"a": 1}` |

```zen
print(int("42"))      # 42
print(float("3.14"))  # 3.14
print(str(42))        # "42"
print(bool(0))        # false
print(list("abc"))    # ["a", "b", "c"]
print(tuple([1, 2]))  # #(1, 2)
print(set([1, 1, 2])) # {1, 2}
print(dict([["a", 1]])) # {"a": 1}
```

String-to-number parsing failures raise `ValueError`:
```zen
try {
    int("not a number")
} catch as e {
    print(e.msg)
}
```

## A.2 Iteration

| Function | Description | Example |
|----------|-------------|---------|
| `len(x)` | Length | `len("abc")` → `3` |
| `range(stop)` | Range 0..stop-1 | `range(5)` → `0..4` |
| `range(start, stop)` | Range start..stop-1 | `range(2, 5)` → `2..4` |
| `range(start, stop, step)` | Range with step | `range(0, 10, 2)` → `0..8` |
| `enumerate(iter)` | Index + value | `enumerate(["a","b"])` → `[[0,"a"],[1,"b"]]` |
| `zip(*iterables)` | Aggregate | `zip([1,2], ["a","b"])` → `[[1,"a"],[2,"b"]]` |
| `sorted(iter)` | Sorted list | `sorted([3,1,2])` → `[1,2,3]` |
| `reversed(iter)` | Reversed | `reversed([1,2,3])` → `[3,2,1]` |
| `any(iter)` | Any true | `any([false, true])` → `true` |
| `all(iter)` | All true | `all([true, true])` → `true` |
| `sum(iter)` | Sum | `sum([1,2,3])` → `6` |
| `min(iter)` | Minimum | `min([3,1,2])` → `1` |
| `max(iter)` | Maximum | `max([3,1,2])` → `3` |
| `map(func, iter)` | Map | `map(lambda x: x*2, [1,2])` → `[2,4]` |
| `filter(func, iter)` | Filter | `filter(lambda x: x>1, [1,2,3])` → `[2,3]` |

```zen
print(len("hello"))   # 5
print(list(range(5))) # [0, 1, 2, 3, 4]
print(list(range(2, 5))) # [2, 3, 4]
print(list(range(0, 10, 2))) # [0, 2, 4, 6, 8]
print(list(enumerate(["a","b"]))) # [[0,"a"],[1,"b"]]
print(list(zip([1,2], ["a","b"]))) # [[1,"a"],[2,"b"]]
print(sorted([3,1,2])) # [1, 2, 3]
print(reversed([1,2,3])) # [3, 2, 1]
print(any([false, true])) # true
print(all([true, true])) # true
print(sum([1,2,3]))   # 6
print(min([3,1,2]))   # 1
print(max([3,1,2]))   # 3
print(list(map(lambda x: x*2, [1,2]))) # [2, 4]
print(list(filter(lambda x: x>1, [1,2,3]))) # [2, 3]
```

## A.3 Introspection

| Function | Description | Example |
|----------|-------------|---------|
| `typeof(x)` | Type name | `typeof(42)` → `"int"` |
| `callable(x)` | Is callable | `callable(print)` → `true` |
| `id(x)` | Object identity | `id(42)` |

```zen
print(typeof(42))     # "int"
print(typeof("hi"))   # "string"
print(callable(print)) # true
print(callable(42))   # false
print(id(42))         # some integer
```

## A.4 I/O

| Function | Description | Example |
|----------|-------------|---------|
| `print(*args)` | Print to stdout | `print("hello", 42)` |
| `input(prompt?)` | Read stdin | `input("name: ")` |
| `exit(code?)` | Exit process | `exit(0)` |

```zen
print("hello", 42)     # hello 42
// let name = input("name: ")
exit(0)
```

## A.5 Truthiness

Everything is truthy except `nil` and `false`:

```zen
print(bool(nil))    # false
print(bool(false))  # false
print(bool(0))      # true   (unlike Python)
print(bool(""))     # true   (unlike Python)
print(bool([]))     # true   (unlike Python)
print(bool({}))     # true   (unlike Python)
print(bool(42))     # true
print(bool("hello")) # true
print(bool([1, 2])) # true
```

Use `bool(x)` to coerce explicitly.

## A.6 Quick reference

```zen
# Type conversion
int("42")      # 42
float("3.14")  # 3.14
str(42)        # "42"
bool(0)        # false
list("abc")    # ["a", "b", "c"]
tuple([1, 2])  # #(1, 2)
set([1, 1, 2]) # {1, 2}
dict([["a", 1]]) # {"a": 1}

# Iteration
len("hello")     # 5
list(range(5))   # [0, 1, 2, 3, 4]
list(enumerate(["a","b"])) # [[0,"a"],[1,"b"]]
list(zip([1,2], ["a","b"])) # [[1,"a"],[2,"b"]]
sorted([3,1,2])  # [1, 2, 3]
reversed([1,2,3]) # [3, 2, 1]
any([false, true]) # true
all([true, true]) # true
sum([1,2,3])     # 6
min([3,1,2])     # 1
max([3,1,2])     # 3

# Introspection
typeof(42)     # "int"
callable(print) # true
id(42)         # some integer

# I/O
print("hello", 42)
exit(0)
```

## A.7 Exercises

1. Write `safe_int(s, default=0)` that parses an int or returns default.
2. Write `chunk_list(lst, n)` that splits a list into chunks of size n using range and slicing.
3. Write `interleave(a, b)` using zip.
4. Write `all_unique(lst)` using set and len.
5. Write `flatten_dict(d)` that flattens a nested dict using recursion and dict methods.

## A.12 Complete built-in function reference

All built-ins are available without importing.

| Function | Category | Purpose | Example | Result |
|----------|----------|---------|---------|--------|
| `print(*vals)` | I/O | print values to stdout | `print("hi", 42)` | hi 42 |
| `str(x)` | conversion | to string | `str(42)` | `"42"` |
| `int(x)` | conversion | to integer | `int("42")` | `42` |
| `float(x)` | conversion | to float | `float("3.14")` | `3.14` |
| `bool(x)` | conversion | to boolean | `bool(0)` | `false` |
| `list(x)` | conversion | to list | `list("ab")` | `["a","b"]` |
| `tuple(x)` | conversion | to tuple | `tuple([1,2])` | `#(1,2)` |
| `set(x)` | conversion | to set | `set([1,1,2])` | `{1,2}` |
| `dict(x)` | conversion | from pairs | `dict([["a",1]])` | `{"a":1}` |
| `len(x)` | iteration | length | `len("abc")` | `3` |
| `range(...)` | iteration | range | `range(5)` | `0..4` |
| `enumerate(it)` | iteration | index+value | `enumerate(["a","b"])` | `[[0,"a"],[1,"b"]]` |
| `zip(...)` | iteration | aggregate | `zip([1,2],["a","b"])` | `[[1,"a"],[2,"b"]]` |
| `sorted(x)` | iteration | sorted list | `sorted([3,1,2])` | `[1,2,3]` |
| `reversed(x)` | iteration | reversed | `reversed([1,2,3])` | `[3,2,1]` |
| `any(it)` | iteration | any true | `any([f,t])` | `true` |
| `all(it)` | iteration | all true | `all([t,t])` | `true` |
| `sum(it)` | math | sum | `sum([1,2,3])` | `6` |
| `min(it/a,b)` | math | minimum | `min([3,1,2])` | `1` |
| `max(it/a,b)` | math | maximum | `max([3,1,2])` | `3` |
| `abs(x)` | math | absolute value | `abs(-5)` | `5` |
| `typeof(x)` | introspection | type name | `typeof(42)` | `"int"` |
| `type(x)` | introspection | type (alias) | `type("x")` | `"string"` |
| `chr(n)` | string | int to char | `chr(65)` | `"A"` |
| `ord(s)` | string | char to int | `ord("A")` | `65` |
| `exit(n)` | system | exit process | `exit(0)` | exits |
| `open(path, mode)` | I/O | open file | `open("f.txt")` | file handle |
| `assert(cond, msg?)` | debugging | assert condition | `assert x > 0` | raises if false |
| `repr(x)` | debugging | developer repr | `repr("hi")` | `"\"hi\""` |

### Type conversion details

```zen
print(int("42"))      # 42
print(float("3.14"))  # 3.14
print(str(42))        # "42"
print(bool(0))        # false
print(list("abc"))    # ["a","b","c"]
print(tuple([1, 2]))  # #(1, 2)
print(set([1, 1, 2])) # {1, 2}
print(dict([["a", 1]])) # {"a": 1}
```

Parsing failures raise `ValueError` (Z0305):
```zen
try { int("not a number") } catch as e { print(e.msg) }
```

### Truthiness reference

Falsy: `false`, `nil`, `null`.
Truthy: everything else — including `0`, `0.0`, `""`, `[]`, `#{}`, `%{}`, `#()`.

```zen
print(bool(0))      # true
print(bool(""))     # true
print(bool([]))     # true
print(bool(false))  # false
print(bool(nil))    # false
```

### `ord`/`chr`

```zen
print(ord("A"))   # 65
print(chr(65))    # "A"
```

### `assert`

```zen
assert x > 0, "x must be positive"
```

### `exit`

```zen
exit(0)   # success
exit(1)   # failure
```

## A.13 Built-in constants

| Constant | Value |
|----------|-------|
| `true` | boolean true |
| `false` | boolean false |
| `nil` / `null` | absence of value |

## A.14 Exercises

1. Predict the result of `bool([])`, `bool("")`, `bool(0)` then verify.
2. Write `safe_int(s)` returning nil on parse failure.
3. Demonstrate `chr`/`ord` round-trip for several characters.
4. Use `enumerate` to print lines with line numbers.
5. Use `zip` to pair two lists into a dict.
6. Compare `min`/`max` on a list vs two scalars.
7. Write `assert_positive(x)` using `assert`.
8. Demonstrate `reversed` on a list and a string.
9. Write `all_even(xs)` using `all`.
10. Show `type`/`typeof` on int, float, string, list, dict, nil, func.

