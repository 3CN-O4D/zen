# Dictionaries

Dicts are key→value maps, created with `{ }`. Keys are strings; values can be
anything. Like lists, dicts are **value-like**: methods return **new** dicts
instead of mutating the receiver.

```zen
var config = { host: "localhost", port: 8080 }
print config.host          # localhost
print config["port"]       # 8080
```

## Building dicts

Name keys and string keys are equivalent:

```zen
var a = { name: "Ada", age: 36 }        # name keys
var b = { "name": "Ada", "age": 36 }    # string keys (identical)

var nested = { server: { host: "db", port: 5432 } }
print nested.server.host                # db
```

Values can be any expression; interpolation works:

```zen
var user = { name: "Ada", tags: ["math", "cs"], active: true }
var id = { "user_${user.name}": 1 }
```

> Keys that aren't plain names must be quoted. **Expression keys are not
> supported** — write `{ key: value }`, not `{ expr: value }`.

## Access

| Operation | Result |
|-----------|--------|
| `d.k` | value, or **error** if missing |
| `d["k"]` | value, or **error** if missing |
| `d?.k` | value, or `null` if `d` is `null` |
| `d.get("k")` | value, or `null` if missing |
| `"k" in d` | `bool` — key presence |
| `d.has("k")` | `bool` — key presence |

```zen
var d = { a: 1 }
print d.a            # 1
print d["a"]         # 1
print d.get("b")     # null
print "b" in d       # false
print d.has("a")     # true
```

> **Gotcha:** `d.get("k", default)` — the default argument is **currently
> ignored**; a missing key returns `null`. Use the nullish operator for
> defaults: `d.get("port") ?? 8080`.

```zen
var port = d.get("port") ?? 8080
```

## Setting & removing

```zen
d["k"] = v           # in-place insert/replace (indexed assignment works)
d.k = v              # member assignment also works
```

Functional methods (return new dicts):

| Method | Description |
|--------|-------------|
| `set(key, value)` | new dict with key set |
| `put(key, value)` | same as `set` |
| `delete(key)` / `remove(key)` | new dict without the key |
| `update(other)` / `merge(other)` | new dict with `other` merged on top |
| `clear()` | new empty dict |
| `pick(...keys)` | new dict with only those keys |
| `omit(...keys)` | new dict without those keys |
| `invert()` | values become keys, keys become values |

```zen
print { a: 1 }.set("b", 2)        # {a: 1, b: 2}
print { a: 1, b: 2 }.delete("a")  # {b: 2}
print { a: 1 }.merge({ b: 2 })    # {a: 1, b: 2}
print { a: 1, b: 2, c: 3 }.pick("a", "c")   # {a: 1, c: 3}
print { a: 1, b: 2 }.omit("a")    # {b: 2}
print { a: 1 }.invert()           # {"1": "a"}
```

## Size, keys, values, items

```zen
var d = { a: 1, b: 2 }

print len(d)          # 2
print d.len           # 2
print d.length()      # 2

print d.keys()        # [a, b]
print d.values()      # [1, 2]
print d.items()       # [[a, 1], [b, 2]]
```

Iterate in `for` loops (dicts themselves are not directly iterable):

```zen
for k in d.keys() { print k }
for v in d.values() { print v }
for pair in d.items() {
    print pair[0] + "=" + str(pair[1])
}

for k in d { ... }    # Error: for requires a list
```

The global helpers mirror the methods:

```zen
print keys({ a: 1 })    # [a]
print values({ a: 1 })  # [1]
print items({ a: 1 })   # [[a, 1]]
print has({ a: 1 }, "a")  # true
```

## Missing keys and default patterns

```zen
var settings = { theme: "dark" }

# get + ?? is the idiomatic default:
var font = settings.get("font") ?? "monospace"
print font                                  # monospace

# membership guard:
if settings.has("theme") {
    print settings.theme
}
```

## Spread into a dict

`...` copies another dict's pairs:

```zen
var defaults = { retries: 3, timeout: 30 }
var request  = { ...defaults, path: "/api" }
print request            # {retries: 3, timeout: 30, path: /api}
```

## JSON round trip

Dicts ↔ JSON are natural partners:

```zen
var payload = { name: "Ada", skills: ["math"] }
var text = json.stringify(payload)     # '{"name":"Ada","skills":["math"]}'
var back = json.parse(text)
print back.name                        # Ada
```

## Dot access vs indexed access

Dot access picks a key literally named `.xyz`. For programmatic keys use `[]`:

```zen
var d = { "first name": "Ada" }
print d["first name"]                  # Ada   (dot access impossible)

var key = "name"
print d[key]                           # dynamic lookup
```

## Truthiness & falsiness

Empty dict `{}` is falsy; a dict with any key is truthy.

```zen
if {} { print "no" } else { print "yes" }    # yes
if { a: 0 } { print "yes" }                  # yes (key present)
```

## Common pitfalls

| Mistake | Reality |
|---------|---------|
| `d.missing` | error — `dictionary has no member: missing` |
| `d.get("k", fallback)` | default arg currently ignored → use `??` |
| `for k in d` | `for requires a list` — use `d.keys()` |
| `{"a": { ... }}` with expression key | expression keys unsupported |
| `.put()` expecting in-place | returns a new dict — bind it |
| `d["k"] += 1` | compound assignment through index unsupported |
| `{} == {}` | true — dicts compare by content, not identity |