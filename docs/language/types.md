# Types

Zen values and what they do. `typeof(x)` reports the lowercase name.

| Type | `typeof` | Literals & notes |
|------|----------|------------------|
| null | `"null"` | `null` (a.k.a. `nil` in older docs). |
| bool | `"bool"` | `true`, `false`. |
| int | `"int"` | `42`, `-7`. 64-bit. |
| float | `"float"` | `2.5`, `1e3`. Full precision on print. |
| string | `"string"` | `"hi"`, interpolation `"${x}"`, indexing `s[0]`, slices `s[1:3]`. |
| list | `"list"` | `[1, "a"]`. `push` mutates; `sort`/`reverse`/`insert` return new lists. |
| dict | `"dict"` | `{"a": 1}`. `.has()`/`.get()`/`.keys()`/`.values()`; `d[k]` raises on missing keys. |
| func | `"func"` | `fn(x) { return x }`, named `func name() {}`. Closures capture locals. |
| instance | `"instance"` | Class instances (`new C()` or `C()`). |
| class | `"class"` | Class objects themselves. |

## Strings

Indexing yields one-character strings; negative indices count from the end.
`"x" * 3` repeats; `+` concatenates; `in` tests substring membership.

```zen
print "hello"[1]      # e
print "ab" * 3        # ababab
print "ell" in "hello"  # true
```

## Lists

`push` appends in place and `pop` removes (default: last). `sort`, `reverse`,
and `insert` return **new** lists — rebind the result. `for x in list`
iterates; `0..n` ranges count up.

```zen
let xs = [3, 1, 2]
print xs.sort()  # [1, 2, 3]
print xs         # [3, 1, 2] — unchanged
```

## Dicts

String keys are typical; any value type works as a key where the operation
supports it. Missing-key reads raise — use `.get(k, default)` for a fallback.

```zen
let d = {"a": 1}
print d.has("b")       # false
print d.get("b", 99)   # 99
d["b"] = 2
print d                # {a: 1, b: 2}
```

## Truthiness

`false`, `null`, `0`, `0.0`, `""`, `[]`, and `{}` are falsy; everything else
is truthy. `and` / `or` / `not` follow short-circuit rules.
