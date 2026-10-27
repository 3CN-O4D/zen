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
