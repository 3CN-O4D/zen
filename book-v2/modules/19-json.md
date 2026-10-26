# Chapter 19: json Module

The `json` module provides JSON parsing and serialization. Zen ships a
pure-Zen implementation (`std/json.z`) with numbers stringified via `str()`.

## 19.1 Parsing JSON

```zen
import json

let data = json.loads('{"name": "Zen", "version": 3, "tags": ["ffi", "pymport"]}')
print(data["name"])     # Zen
print(data["version"])  # 3
print(data["tags"][1])  # pymport
```

`json.loads(text)` parses a JSON string into Zen values:
- JSON object → Zen dict
- JSON array → Zen list
- JSON string → Zen string
- JSON number → Zen int or float
- JSON `true`/`false` → Zen bool
- JSON `null` → Zen nil

## 19.2 Serializing JSON

```zen
import json

let obj = {"name": "Zen", "version": 3, "tags": ["ffi", "pymport"]}
let text = json.dumps(obj)
print(text)  # {"name": "Zen", "version": 3, "tags": ["ffi", "pymport"]}
```

`json.dumps(value)` serializes Zen values to a JSON string:
- Zen dict → JSON object (string keys only)
- Zen list → JSON array
- Zen string → JSON string
- Zen int/float → JSON number
- Zen bool → JSON `true`/`false`
- Zen nil → JSON `null`

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

## 19.5 Numbers

The pure-Zen `json.z` stringifies numbers via `str()`. This means:
- `json.dumps(42)` → `"42"`
- `json.dumps(3.14)` → `"3.14"`
- Nested dict access uses explicit keys: `data["a"]["b"]`

## 19.6 Common pitfalls

- **Dict keys must be strings.** Non-string keys are rejected.
- **`json.loads` raises `ValueError`** on malformed JSON. Use `try`/`catch`.
- **Numbers round-trip as strings** in some edge cases — verify critical
  numeric payloads.
- **`nil` serializes as `null`.**

## 19.7 Quick reference

```zen
import json

json.loads('{"a": 1}')     # {"a": 1}
json.dumps({"a": 1})       # '{"a": 1}'
json.loads("[1, 2, 3]")    # [1, 2, 3]
json.dumps([1, 2, 3])      # '[1, 2, 3]'

import fs
fs.write("data.json", json.dumps(obj))
json.loads(fs.read("data.json"))
```

## 19.8 Exercises

1. Write `load_config(path)` that reads a JSON config file with `get`
   defaults for missing keys.
2. Write `save_scores(path, scores)` that appends a score to a JSON array
   file.
3. Write `merge_json(a, b)` that merges two JSON objects (b wins on conflict).
4. Write a script that fetches JSON from an HTTP endpoint and parses it.
5. Write `json_to_csv(data)` that converts a list of flat dicts to CSV text.
