# Chapter 19: json Module

The `json` module provides JSON parsing and serialization. Zen ships a pure-Zen implementation (`std/json.z`) with numbers stringified via `str()`.

## 19.1 Parsing JSON

```zen
import json

let data = json.loads('{"name": "Zen", "version": 3, "tags": ["ffi", "pymport"]}')
print(data["name"])     # Zen
print(data["version"])  # 3
print(data["tags"][1])  # pymport
```

`json.loads(text)` parses a JSON string into Zen values:

| JSON type | Zen type |
|-----------|----------|
| object | Zen dict (string keys only) |
| array | Zen list |
| string | Zen string |
| number | Zen int or float |
| true/false | Zen bool |
| null | Zen nil |

```zen
import json

let obj = json.loads('{"name": "Zen", "version": 3, "tags": ["ffi", "pymport"]}')
print(obj["name"])     # Zen
print(obj["version"])  # 3
print(obj["tags"][1])  # pymport

let arr = json.loads('[1, 2, 3]')
print(arr[0])   # 1

let bool_val = json.loads('true')
print(bool_val)   # true

let null_val = json.loads('null')
print(null_val)   # nil
```

## 19.2 Serializing JSON

```zen
import json

let obj = {"name": "Zen", "version": 3, "tags": ["ffi", "pymport"]}
let text = json.dumps(obj)
print(text)  # {"name": "Zen", "version": 3, "tags": ["ffi", "pymport"]}
```

`json.dumps(value)` serializes Zen values to a JSON string:

| Zen type | JSON type |
|----------|-----------|
| dict (string keys) | object |
| list | array |
| string | string |
| int/float | number |
| bool | true/false |
| nil | null |

```zen
import json

let obj = {"players": 4, "active": true, "scores": [10, 20, 30], "name": null}
let text = json.dumps(obj)
print(text)  # {"players": 4, "active": true, "scores": [10, 20, 30], "name": null}
```

## 19.3 Round-trip example

```zen
import json

let original = {"players": 4, "active": true, "scores": [10, 20, 30]}
let text = json.dumps(original)
let parsed = json.loads(text)

print(parsed["players"])   # 4
print(parsed["active"])    # true
print(parsed["scores"][1]) # 20
```

## 19.4 Reading/writing JSON files

```zen
import json
import fs

# Write JSON to a file
let data = {"config": {"theme": "dark", "font": 12}}
fs.write("config.json", json.dumps(data))

# Read JSON from a file
let text = fs.read("config.json")
let config = json.loads(text)
print(config["config"]["theme"])  # dark
```

### Complete config example

```zen
import json
import fs

func load_config(path, defaults) {
    if !fs.exists(path) {
        return defaults
    }
    let text = fs.read(path)
    return json.loads(text)
}

func save_config(path, config) {
    fs.write(path, json.dumps(config))
}

let defaults = {"theme": "light", "font": 12, "debug": false}
let config = load_config("config.json", defaults)
print(config["theme"])

config["debug"] = true
save_config("config.json", config)
```

## 19.5 Numbers

The pure-Zen `json.z` stringifies numbers via `str()`. This means:
- `json.dumps(42)` → `"42"`
- `json.dumps(3.14)` → `"3.14"`
- Nested dict access uses explicit keys: `data["a"]["b"]`

```zen
import json

print(json.dumps(42))       # "42"
print(json.dumps(3.14))     # "3.14"
print(json.dumps(0))        # "0"
print(json.dumps(-1))       # "-1"
print(json.dumps(1e10))     # "10000000000"
```

Numbers round-trip correctly:
```zen
import json

let original = 3.14159
let text = json.dumps(original)
let parsed = json.loads(text)
print(parsed == original)  # true
```

## 19.6 Common pitfalls

- **Dict keys must be strings.** Non-string keys are rejected.
- **`json.loads` raises `ValueError`** on malformed JSON. Use `try`/`catch`.
- **Numbers round-trip as strings** in some edge cases — verify critical numeric payloads.
- **`nil` serializes as `null`.**
- **`json.dumps` does not support indentation/pretty-printing** in this version. Output is compact.

```zen
import json

try {
    json.loads("{not json}")
} catch as e {
    print(e.msg)  # ValueError: ...
}
```

```zen
import json

print(json.dumps({"a": 1, "b": nil}))  # {"a": 1, "b": null}
print(json.dumps({"a": true, "b": false}))  # {"a": true, "b": false}
```

## 19.7 Quick reference

```zen
import json

json.loads('{"a": 1}')     # {"a": 1}
json.dumps({"a": 1})       # '{"a": 1}'
json.loads("[1, 2, 3]")    # [1, 2, 3]
json.dumps([1, 2, 3])      # '[1, 2, 3]'
json.loads('true')         # true
json.loads('null')         # nil

import fs
fs.write("data.json", json.dumps(obj))
json.loads(fs.read("data.json"))
```

## 19.8 Exercises

1. Write `load_config(path)` that reads a JSON config file with `get` defaults for missing keys.
2. Write `save_scores(path, scores)` that appends a score to a JSON array file.
3. Write `merge_json(a, b)` that merges two JSON objects (b wins on conflict).
4. Write a script that fetches JSON from an HTTP endpoint and parses it.
5. Write `json_to_csv(data)` that converts a list of flat dicts to CSV text.
6. Write `validate_json(s)` that returns true if `s` is valid JSON, false otherwise (using try/catch).
7. Write `extract_field(obj, path)` that navigates a nested dict using a dot-separated path like "a.b.c".
