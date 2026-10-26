# Chapter 9: Dictionaries and Maps

Dictionaries are Zen's key-value mappings. They use string keys, preserve
insertion order, and support fast lookup, insertion, and deletion. This
chapter covers dictionaries in depth.

## 9.1 Dictionary literals

```zen
let empty = {}
let person = {"name": "Zen", "version": 3, "active": true}
let nested = {"user": {"name": "Zen", "id": 1}, "tags": ["a", "b"]}
```

Keys must be strings. Values can be any type.

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
} catch KeyError {
    print("key not found")
}
```

## 9.3 Dictionary methods

| Method | Description | Example |
|--------|-------------|---------|
| `keys()` | List of keys | `d.keys()` |
| `values()` | List of values | `d.values()` |
| `items()` | List of [key, value] pairs | `d.items()` |
| `get(key, default?)` | Value or default | `d.get("missing", 0)` |
| `has(key)` | Key exists | `d.has("name")` |
| `len(d)` | Number of entries (builtin) | `len(d)` |
| `copy()` | Shallow copy | `d.copy()` |
| `clear()` | Remove all | `d.clear()` |
| `pop(key)` | Remove and return value | `d.pop("name")` |
| `update(other)` | Merge another dict | `d.update({"a": 1})` |

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

Iteration order is insertion order.

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
print(merged)  # {"x": 1, "y": 3, "z": 4}
```

### Default values with `get`

```zen
let config = {"host": "localhost", "port": 8080}
let host = config.get("host", "0.0.0.0")
let timeout = config.get("timeout", 30)
print(host)    # localhost
print(timeout) # 30
```

## 9.5 Common pitfalls

- **Keys must be strings.** `d[42]` raises `TypeError`. Use `d["42"]`.
- **Missing keys raise `KeyError`.** Use `get(key, default)` or `has(key)`
  to avoid.
- **Dicts are mutable.** `b = a` aliases; use `a.copy()` for independence.
- **`len(d)` is a builtin**, not a method.
- **Iteration order** is insertion order, not sorted.

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
d.update({"z": 9}) # merge
for k in d { ... } # iterate keys
```

## 9.7 Exercises

1. Write `invert_dict(d)` that swaps keys and values (assumes values are
   unique strings).
2. Write `sub_dict(d, keys)` that returns a new dict with only the given
   keys.
3. Write `deep_get(d, path)` that navigates a nested dict using a list of
   keys, returning `nil` if any key is missing.
4. Write `word_frequencies(text)` that counts words in a string.
5. Write `merge_dicts(*dicts)` that merges multiple dicts (later ones win).
