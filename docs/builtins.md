# Built-in functions

Every Zen program has a set of **global builtins** available without importing
anything. There are three layers:

1. **Core globals** (`str`, `len`, `range`, ...) — the functions documented
   here.
2. **Native functions** — a large Rust library (`fs_*`, `math_*`, `random_*`,
   `http_get`, ...) that the modules wrap.
3. **Modules** — dicts like `math`, `fs`, `re`, `json`, ... that are **also
   globally available** without any `import`, exposing the native library
   behind them.

## Type conversion

| Function | Behavior | Example → result |
|----------|----------|------------------|
| `str(x)` | stringify any value | `str(42)` → `"42"` |
| `int(x)` | truncate / parse an integer | `int("42")` → `42`; `int(3.9)` → `3` |
| `float(x)` | parse a float | `float("1.5")` → `1.5` |
| `bool(x)` | truthiness | `bool(0)` → `false` |
| `char(code)` | Unicode code point → char | `char(65)` → `"A"` |
| `chr(code)` | same as `char` | `chr(97)` → `"a"` |
| `ord(ch)` | char → code point | `ord("A")` → `65` |
| `hex(n)` | int → hex string | `hex(255)` → `"ff"` |

```zen
print(str([1, 2]))     # [1, 2]
print(int("5") + 1)    # 6
print(char(65))        # A
```

## Inspecting values

| Function | Behavior |
|----------|----------|
| `type(x)` / `typeof(x)` | type name string: `int` `float` `string` `bool` `list` `dict` `null` `function` `object` ... |
| `len(x)` | length of string (chars), list, or dict |
| `has(c, key)` | `true` if list has the item or dict has the key |

```zen
print(type([1]))       # list
print(len("héllo"))    # 5
print(has({a: 1}, "a"))  # true
print(has([1, 2], "2"))  # false   (list membership is strict)
```

> `len()` is a **function**, while `.len`/`.length()` work as a member on the
> value. All three are equivalent; different call styles.

## Collections

| Function | Behavior | Example |
|----------|----------|---------|
| `list(x)` | build a list (from a list, string, or iterable) | `list("ab")` → `["a", "b"]` |
| `dict()` | build an empty dict | `dict()` → `{}` |
| `keys(d)` | dict keys as a list | `keys({a: 1, b: 2})` → `["a", "b"]` |
| `values(d)` | dict values as a list | `values({a: 1})` → `[1]` |
| `items(d)` | dict as `[key, value]` pairs | `items({a: 1})` → `[["a", 1]]` |
| `push(l, x)` | new list with `x` appended | `push([1], 2)` → `[1, 2]` |
| `pop(l)` | last element of `l` | `pop([1, 2])` → `2` |
| `slice(x, a[, b])` | sub-list / sub-string | `slice([0,1,2,3],1,3)` → `[1, 2]` |
| `range(a[, b[, step]])` | numeric range as a list | `range(0,10,2)` → `[0,2,4,6,8]` |
| `enumerate(l)` | `[[i, item], ...]` | `enumerate(["a"])` → `[[0, a]]` |

```zen
for i, item in enumerate(["x", "y"]) {
    print(i, item)         # 0 x  then  1 y
}
```

## Arithmetic

Unary math functions on a single number:

```zen
abs(-3)      # 3
round(3.6)   # 4
trunc(3.9)   # 3
floor(3.9)   # 3
ceil(3.1)    # 4
sqrt(9)      # 3
sin(0)       # 0
cos(0)       # 1
tan(0)       # 0
log(1)       # 0   (natural log)
log10(100)   # 2
exp(0)       # 1
```

`min`/`max` accept any mix of numbers and lists — they flatten lists and
reduce:

```zen
print(min([3, 1, 2]))    # 1
print(max(3, 1, 2))      # 3
print(min([5], 2, [1]))  # 1   (flattens lists + variadic)
```

> These duplicates exist in the `math` module (`math.sqrt`, `math.abs`, ...).
> The globals are convenient sub‑set; `math` additionally has `pow`, `gcd`,
> `log2`, `hypot`, `atan2`, constants `pi`/`e`, and inf/nan flags.

## Control

| Function | Behavior |
|----------|----------|
| `sleep(seconds)` | block for **seconds** (float allowed) |
| `wait(milliseconds)` | block for **milliseconds** |
| `exit(code?=0)` | terminate the program with a status code |
| `assert(cond, msg?)` | fail with `zen: assertion failed` if falsy |
| `input(prompt?)` | read one line from stdin (trailing newline stripped) |
| `throw` | is a statement, not a function — `throw value` |

```zen
sleep(0.5)      # pause half a second
wait(250)       # pause 250 milliseconds
assert(x > 0, "x must be positive")
```

## Help text

`help()` prints an overview of built-ins and operators. `help(someValue)`
describes a value/module:

```zen
help()          # built-in overview
help(math)      # "dict with 42 keys: ..." (the exported members)
help("re")      # string info
```

> `help` is callable but not a bound variable — `print(help)` fails.

## What about all those `math_*`, `fs_*`, `regex_*` names?

The runtime registers ~411 native functions internally (`math_sqrt`,
`fs_read`, `regex_match`, `b64_encode`, ...). **You almost never call these
directly** — they are the implementation behind the modules. The modules
**and** their functions are already globals, so just call them:

```zen
math.sqrt(9)              # works with no import
fs.read("file.txt")
re.match("^a", "abc")
random.randint(1, 10)
```

The names are the module dict members (e.g. `math` is a dict whose `sqrt` key
is the function). `import math` is optional and harmless for these.

If you genuinely need raw access to a lower-level native, `native func
name(args)` binds one for you:

```zen
native func list_modules()
print(list_modules())    # every registered module
```

## Common pitfalls

| Mistake | Reality |
|---------|---------|
| `sleep(500)` expecting ms | `sleep` is **seconds** — use `wait(500)` for ms |
| `len str` rather than `len("x")` | it's a function call |
| `has([1], "1")` checking string membership | list membership is strict (`false`) |
| `print(help)` | `help` isn't a variable — call `help()` |
| `min()` / `max()` with one scalar | fine — but they reduce/flatten via lists |
| `min([1,2])` as a *method* | `l.min()` is not a method — use global `min(l)` |