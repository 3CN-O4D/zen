# Chapter 9: Dictionaries and Maps

Dictionaries are Zen's key-value mappings. They use string keys, preserve insertion order, and support fast lookup, insertion, and deletion. This chapter covers dictionaries in depth, including key rules, iteration, and common patterns.

## 9.1 Dictionary literals

```zen
let empty = {}
let person = {"name": "Zen", "version": 3, "active": true}
let nested = {"user": {"name": "Zen", "id": 1}, "tags": ["a", "b"]}
```

Keys must be strings. Values can be any type. `{}` alone is an empty **dict**, not a set (use `{1}` or `set()` for a set).

## 9.2 Access and mutation

```zen
let person = {"name": "Zen", "version": 3}

print(person["name"])    # Zen
print(person["version"]) # 3

person["version"] = 4    # update
person["lang"] = "Zen"   # add

print(person)            # {"name": "Zen", "version": 4, "lang": "Zen"}
```

Missing keys raise `KeyError`:

```zen
try {
    print(person["missing"])
} catch as e {
    print(e.msg)  # KeyError: missing
}
```

## 9.3 Dictionary methods

| Method | Signature | Description | Example |
|--------|-----------|-------------|---------|
| `keys()` | `dict → list` | List of keys | `d.keys()` |
| `values()` | `dict → list` | List of values | `d.values()` |
| `items()` | `dict → list` | List of [key, value] pairs | `d.items()` |
| `get(key, default?)` | `dict → value` | Value or default | `d.get("missing", 0)` |
| `has(key)` | `dict → bool` | Key exists | `d.has("name")` |
| `len(d)` | builtin | Number of entries | `len(d)` |
| `copy()` | `dict → dict` | Shallow copy | `d.copy()` |
| `clear()` | `dict → nil` | Remove all | `d.clear()` |
| `pop(key)` | `dict → value` | Remove and return value | `d.pop("name")` |
| `update(other)` | `dict → nil` | Merge another dict | `d.update({"a": 1})` |

### Examples

```zen
let d = {"a": 1, "b": 2, "c": 3}

print(d.keys())      # ["a", "b", "c"]
print(d.values())    # [1, 2, 3]
print(d.items())     # [["a", 1], ["b", 2], ["c", 3]]

print(d.get("a"))           # 1
print(d.get("z", 0))        # 0 (default)
print(d.has("b"))           # true
print(d.has("z"))           # false
print(len(d))               # 3

let d2 = d.copy()
d2.update({"c": 30, "d": 4})
print(d2)  # {"a": 1, "b": 2, "c": 30, "d": 4}

let popped = d.pop("b")
print(popped)  # 2
print(d)       # {"a": 1, "c": 3}
```

`pop(key)` raises `KeyError` if the key is missing:
```zen
try {
    d.pop("nonexistent")
} catch as e {
    print(e.msg)  # KeyError: nonexistent
}
```

`get(key)` returns `nil` if the key is missing and no default is given:
```zen
print(d.get("nonexistent"))  # nil
print(d.get("nonexistent", 42))  # 42
```

## 9.3 Iteration

```zen
let d = {"a": 1, "b": 2, "c": 3}

# Iterate keys
for k in d {
    print(k)
}

# Iterate keys explicitly
for k in d.keys() {
    print(k)
}

# Iterate values
for v in d.values() {
    print(v)
}

# Iterate items
for kv in d.items() {
    print(kv[0], kv[1])
}
```

Iteration order is **insertion order**, not sorted:
```zen
let d = {"z": 1, "a": 2, "m": 3}
for k in d {
    print(k)   # z, a, m  (insertion order)
}
```

## 9.4 Common patterns

### Counting frequencies

```zen
let words = ["zen", "lang", "zen", "script", "lang", "zen"]
var counts = {}

for w in words {
    if d.has(w) {
        d[w] = d[w] + 1
    } else {
        d[w] = 1
    }
}
print(d)  # {"zen": 3, "lang": 2, "script": 1}
```

More concisely with `get`:
```zen
var counts = {}
for w in words {
    counts[w] = counts.get(w, 0) + 1
}
```

### Grouping by key

```zen
let items = [
    {"category": "fruit", "name": "apple"},
    {"category": "fruit", "name": "banana"},
    {"category": "veg", "name": "carrot"}
]
var groups = {}

for item in items {
    let cat = item["category"]
    if !groups.has(cat) {
        groups[cat] = []
    }
    groups[cat].push(item["name"])
}
print(groups)  # {"fruit": ["apple", "banana"], "veg": ["carrot"]}
```

### Merging dictionaries

```zen
let a = {"x": 1, "y": 2}
let b = {"y": 3, "z": 4}

let merged = a.copy()
merged.update(b)
print(merged)  # {"x": 1, "y": 3, "z": 4}  (b wins on conflict)
```

`update` overwrites existing keys and adds new ones.

### Default values with `get`

```zen
let config = {"host": "localhost", "port": 8080}
let host = config.get("host", "0.0.0.0")
let timeout = config.get("timeout", 30)
print(host)    # localhost
print(timeout) # 30
```

### Inverting a dictionary

```zen
let d = {"a": 1, "b": 2, "c": 3}
var inverted = {}
for k in d {
    inverted[d[k]] = k
}
print(inverted)  # {1: "a", 2: "b", 3: "c"}
```

> Warning: if values are not unique, later keys overwrite earlier ones.

## 9.5 Key rules

- **Keys must be strings.** `d[42]` raises `TypeError`. Use `d["42"]`.
- **Keys are compared by string content**, not identity.
- **Missing keys raise `KeyError`** on direct access `d["missing"]`. Use `get(key, default)` or `has(key)` to avoid.
- **Insertion order is preserved** and is the iteration order.

## 9.6 Common pitfalls

- **Dicts are mutable.** `b = a` aliases; use `a.copy()` for independence.
- **`len(d)` is a builtin**, not a method.
- **Iteration order** is insertion order, not sorted.
- **`pop(key)` raises `KeyError`** if missing. Use `get` with a default if you want `nil` instead.
- **Non-string keys are rejected.** `d[42]` → TypeError. Even `d[true]` → TypeError.

## 9.6 Quick reference

```zen
let d = {"a": 1, "b": 2}

d["a"]             # 1
d["c"] = 3         # add/update
d.get("z", 0)      # 0
d.has("a")         # true
d.keys()           # ["a", "b", "c"]
d.values()         # [1, 2, 3]
d.items()          # [["a", 1], ...]
len(d)             # 3
d.copy()           # shallow copy
d.pop("a")         # 1 (removes "a")
d.update({"z": 9}) # merge (later wins)
for k in d { ... } # iterate keys (insertion order)
```

## 9.7 Exercises

1. Write `invert_dict(d)` that swaps keys and values (assumes values are unique strings).
2. Write `sub_dict(d, keys)` that returns a new dict with only the given keys.
3. Write `deep_get(d, path)` that navigates a nested dict using a list of keys, returning `nil` if any key is missing.
4. Write `word_frequencies(text)` that counts words in a string.
5. Write `merge_dicts(*dicts)` that merges multiple dicts (later ones win).
6. Write `dict_intersection(d1, d2)` that returns a dict with keys present in both, using values from d1.
7. Write `dict_diff(d1, d2)` that returns keys in d1 but not d2.
8. Demonstrate that dict equality is order-insensitive: `{"a":1,"b":2} == {"b":2,"a":1}`.

## 9.12 Dictionaries — complete reference

A dict is an unordered collection of key/value pairs. Keys must be strings.

```zen
let empty = #{}
let person = #{name: "Ada", age: 36, city: "London"}
let mixed = #{a: 1, b: "two", c: true, d: [1, 2]}
```

### Creation

| Form | Result |
|------|--------|
| `#{}` | empty dict |
| `#{k: v, ...}` | literal with string keys |
| `dict(pairs)` | from list of `[k, v]` pairs |
| `list(d)` | list of keys |

Note: Legacy `{k: v}` still parses with a deprecation warning; prefer `#{k: v}`.

## 9.13 Access and mutation

```zen
let d = #{name: "Ada", age: 36}
print(d["name"])    # Ada
d["age"] = 37       # update
d["city"] = "London"  # add
print(d)            # #{name: "Ada", age: 37, city: "London"}
```

Missing key access raises an error. Use `in` or `??` to be safe:

```zen
print("name" in d)          # true
print("country" in d)       # false
print(d["country"] ?? "UK") # UK  (default when missing)
```

## 9.14 Dict methods

| Method | Purpose | Example |
|--------|---------|---------|
| `keys()` | list of keys | `d.keys()` |
| `values()` | list of values | `d.values()` |
| `items()` | list of `[k, v]` pairs | `d.items()` |
| `get(k, default)` | value or default | `d.get("x", 0)` |
| `pop(k)` | remove and return value | `d.pop("age")` |
| `clear()` | remove all entries | `d.clear()` |
| `update(other)` | merge another dict | `d.update(#{x: 1})` |

```zen
for k, v in d.items() {
    print(k, v)
}
```

## 9.15 Dict iteration

Iterating a dict yields keys. Use `.items()` for key/value, `.values()` for values.

```zen
for k in d { print(k) }              # keys
for k, v in d.items() { print(k, v) } # pairs
for v in d.values() { print(v) }     # values
```

Iteration order is not guaranteed across versions — do not rely on it.

## 9.16 Dict equality and hashing

Dicts compare order-insensitively with `==`:

```zen
print(#{a: 1, b: 2} == #{b: 2, a: 1})   # true
```

Keys must be strings; non-string keys raise a TypeError. There is no hashing
of arbitrary keys in Zen 3.0.

## 9.17 Common dict patterns

### Counting frequencies

```zen
func count_words(words) {
    let freqs = #{}
    for w in words {
        freqs[w] = (freqs[w] ?? 0) + 1
    }
    return freqs
}
print(count_words(["a", "b", "a", "c", "b", "a"]))
# #{a: 3, b: 2, c: 1}
```

### Grouping by key

```zen
func group_by(xs, key_fn) {
    let groups = #{}
    for x in xs {
        let k = key_fn(x)
        if k in groups {
            groups[k].push(x)
        } else {
            groups[k] = [x]
        }
    }
    return groups
}
```

### Merging dicts

```zen
let base = #{a: 1, b: 2}
let overrides = #{b: 20, c: 3}
let merged = #{}
for k in base { merged[k] = base[k] }
for k in overrides { merged[k] = overrides[k] }
print(merged)   # #{a: 1, b: 20, c: 3}
```

### Default-dict pattern

```zen
func get_or_default(d, k, default) {
    if k in d { return d[k] }
    return default
}
```

## 9.18 Nested dicts

```zen
let config = #{database: #{host: "localhost", port: 5432}}
print(config["database"]["host"])   # localhost
```

Safely navigate nested dicts:

```zen
func deep_get(d, path, default = nil) {
    var cur = d
    for p in path {
        if typeof(cur) != "dict" || !(p in cur) { return default }
        cur = cur[p]
    }
    return cur
}
print(deep_get(config, ["database", "host"]))   # localhost
print(deep_get(config, ["database", "user"], "n/a"))   # n/a
```

## 9.19 Performance notes

- Key lookup is O(1) on average.
- Dicts use more memory than lists for small collections.
- Avoid rebuilding dicts in hot loops; mutate in place.
- `in` on a dict checks keys, not values.

## 9.20 Common dict mistakes

1. **Accessing a missing key** without `??` or `in` raises an error.
2. **Assuming iteration order.** Sort keys if order matters.
3. **Non-string keys** are not allowed.
4. **Aliasing:** `b = a` shares the dict; copy manually for independence.
5. **Mutating while iterating** over keys.
6. **Confusing `.get(k, default)` with `d[k] ?? default`** — equivalent but `??`
   also triggers on explicit `nil` values stored in the dict.

## 9.21 Exercises

1. Write `invert_dict(d)` swapping keys and values (values must be strings).
2. Implement `subtract_dicts(a, b)` removing b's keys from a.
3. Write `deep_merge(a, b)` recursively merging nested dicts.
4. Implement `word_frequencies(text)` using split + counting.
5. Write `filter_dict(d, pred)` keeping entries where pred(k, v) is true.
6. Implement `dict_from_pairs(pairs)` from `[k, v]` list.
7. Write `common_keys(a, b)` returning keys present in both.
8. Implement `rename_keys(d, mapping)` renaming per a mapping dict.
9. Write `nested_set(d, path, value)` setting a value at a dotted path.
10. Implement `dict_diff(a, b)` showing added/removed/changed keys.

