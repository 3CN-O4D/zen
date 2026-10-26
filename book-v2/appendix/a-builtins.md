# Appendix A: Built-in Functions

Zen's built-in functions are always available without importing. They cover
type conversion, iteration, introspection, and basic I/O.

## Type conversion

| Function | Description | Example |
|----------|-------------|---------|
| `int(x)` | Convert to integer | `int("42")` → `42` |
| `float(x)` | Convert to float | `float("3.14")` → `3.14` |
| `str(x)` | Convert to string | `str(42)` → `"42"` |
| `bool(x)` | Convert to boolean | `bool(1)` → `true` |
| `list(x)` | Convert to list | `list("abc")` → `["a", "b", "c"]` |
| `tuple(x)` | Convert to tuple | `tuple([1, 2])` → `#(1, 2)` |
| `set(x)` | Convert to set | `set([1, 1, 2])` → `{1, 2}` |
| `dict(x)` | Convert from pairs | `dict([["a", 1]])` → `{"a": 1}` |

## Iteration

| Function | Description | Example |
|----------|-------------|---------|
| `len(x)` | Length | `len("abc")` → `3` |
| `range(stop)` or `range(start, stop)` or `range(start, stop, step)` | Range | `range(5)` → `0..4` |
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

## Introspection

| Function | Description | Example |
|----------|-------------|---------|
| `typeof(x)` | Type name | `typeof(42)` → `"int"` |
| `isinstance(x, type)` | Type check | `isinstance(42, "int")` → `true` |
| `dir(x)` | Attributes | `dir("abc")` → methods |
| `hasattr(x, name)` | Has attribute | `hasattr("abc", "upper")` → `true` |
| `getattr(x, name)` | Get attribute | `getattr("abc", "upper")` |
| `setattr(x, name, value)` | Set attribute | `setattr(obj, "x", 1)` |
| `delattr(x, name)` | Delete attribute | `delattr(obj, "x")` |
| `callable(x)` | Is callable | `callable(print)` → `true` |
| `id(x)` | Object identity | `id(42)` |

## I/O

| Function | Description | Example |
|----------|-------------|---------|
| `print(*args)` | Print to stdout | `print("hello", 42)` |
| `input(prompt?)` | Read stdin | `input("name: ")` |
| `open(path, mode)` | Open file | `open("f.txt", "r")` |
| `exit(code?)` | Exit process | `exit(0)` |

## Examples

```zen
# Type conversion
print(int("42"))      # 42
print(float("3.14"))  # 3.14
print(str(42))        # "42"
print(bool(0))        # false

# Iteration
print(len("hello"))   # 5
print(list(range(5))) # [0, 1, 2, 3, 4]
print(list(enumerate(["a","b"]))) # [[0,"a"],[1,"b"]]
print(sorted([3,1,2])) # [1, 2, 3]
print(sum([1,2,3]))   # 6
print(min([3,1,2]))   # 1
print(max([3,1,2]))   # 3

# Introspection
print(typeof(42))     # "int"
print(callable(print)) # true

# I/O
print("hello")
# let name = input("name: ")
exit(0)
```

## Truthiness

Everything is truthy except `nil` and `false`:

```zen
print(bool(nil))    # false
print(bool(false))  # false
print(bool(0))      # true
print(bool(""))     # true
print(bool([]))     # true
print(bool({}))     # true
```
