# JSON Module

The JSON module provides functions for encoding and decoding JSON data. These functions are available as globals — no import needed.

---

## Quick Start

```
// Parse a JSON string
let data = json.parse('{"name": "Alice", "age": 30}')
print data.name  // Alice

// Encode a dict to JSON string
let info = {"lang": "zen", "version": 1}
print json.encode(info)  // {"lang":"zen","version":1}

// Read a JSON file from disk
let config = json.load("config.json")
print config.theme  // dark

// Write data to a JSON file
json.save("output.json", data)
```

---

## `json.parse(str)`

Decodes a JSON string into a Zen value. JSON objects become dicts, JSON arrays become lists, and JSON primitives map to their Zen equivalents.

### Basic types

```
// String
json.parse('"hello"')  // "hello"

// Number
json.parse("42")       // 42
json.parse("3.14")     // 3.14
json.parse("-7")       // -7

// Booleans
json.parse("true")     // true
json.parse("false")    // false

// Null
json.parse("null")     // null
```

### Objects and arrays

```
// Simple object
let obj = json.parse('{"x": 1, "y": 2}')
print obj.x   // 1
print obj.y   // 2

// Array
let arr = json.parse('[10, 20, 30]')
print arr[0]   // 10
print len(arr) // 3

// Nested structures
let deep = json.parse('{"users": [{"name": "Bob", "active": true}, {"name": "Eve", "active": false}]}')
print deep.users[0].name  // Bob
print deep.users[1].active // false

// Empty containers
json.parse('{}')  // {}
json.parse('[]')  // []
```

### Escape sequences in strings

```
// The parser handles standard JSON escapes
json.parse('"line1\\nline2"')     // "line1\nline2" (literal newline)
json.parse('"tab\\there"')        // "tab\there" (literal tab)
json.parse('"quote: \\""')        // "quote: \""
json.parse('"back\\\\slash"')     // "back\\slash"
json.parse('"slash \\/ here"')    // "slash / here"
json.parse('"unicode \\u0041"')   // "unicode A"
```

### Error cases

```
// Invalid JSON returns an error
json.parse("not json")
// Error: invalid JSON value

// Trailing characters
json.parse("42 hello")
// Error: trailing characters in JSON

// Unterminated string
json.parse('"oops')
// Error: unterminated JSON string

// Missing comma
json.parse('{"a": 1 "b": 2}')
// Error: expected ',' or '}' in JSON object

// Missing colon
json.parse('{"a" 1}')
// Error: expected ':' in JSON object

// Trailing comma (not valid JSON)
json.parse('[1, 2, 3,]')
// Error: expected ',' or ']' in JSON array
```

### Handling parse errors with try/catch

```
let input = "definitely not json"
let result = try json.parse(input) catch err {
    print "Parse failed: " + err
    null
}
print result  // null
```

---

## `json.encode(val)` / `json.stringify(val)`

Encodes a Zen value to a JSON string. `json.stringify` is an alias — they do the same thing.

### Basic encoding

```
// Strings
json.encode("hello")      // "hello"

// Numbers
json.encode(42)           // 42
json.encode(3.14)         // 3.14
json.encode(-100)         // -100

// Booleans
json.encode(true)         // true
json.encode(false)        // false

// Null
json.encode(null)         // null

// Lists
json.encode([1, 2, 3])   // [1,2,3]

// Dicts
json.encode({"a": 1})    // {"a":1}
```

### Encoding nested data

```
let user = {
    "name": "Alice",
    "scores": [95, 87, 92],
    "address": {
        "city": "Nairobi",
        "zip": "00100"
    }
}

print json.encode(user)
// Output: {"name":"Alice","scores":[95,87,92],"address":{"city":"Nairobi","zip":"00100"}}
```

### String escaping

Special characters in strings are automatically escaped:

```
// Quotes inside strings
json.encode('He said "hi"')    // "He said \"hi\""

// Backslashes
json.encode("path\\to\\file")  // "path\\\\to\\\\file"

// Newlines become \n
json.encode("line1\nline2")    // "line1\nline2"
```

### Non-serializable values

Functions, instances, and sockets are encoded as descriptive strings:

```
// These become JSON strings describing the type
json.encode(print)        // "<native:print>"
```

### Pretty-printing

Pass an options dict with `"pretty": true` to get indented output:

```
let data = {"name": "Alice", "hobbies": ["reading", "hiking"], "active": true}

print json.encode(data)
// Compact: {"name":"Alice","hobbies":["reading","hiking"],"active":true}

print json.encode(data, {"pretty": true})
// Output:
// {
//   "name": "Alice",
//   "hobbies": [
//     "reading",
//     "hiking"
//   ],
//   "active": true
// }
```

Pretty-printing with deeply nested data:

```
let config = {
    "database": {
        "host": "localhost",
        "port": 5432,
        "options": {
            "timeout": 30,
            "pool_size": 10
        }
    },
    "debug": false
}

print json.encode(config, {"pretty": true})
// Output:
// {
//   "database": {
//     "host": "localhost",
//     "port": 5432,
//     "options": {
//       "timeout": 30,
//       "pool_size": 10
//     }
//   },
//   "debug": false
// }
```

### Encoding empty containers

```
json.encode({})   // {}
json.encode([])   // []
```

---

## `json.load(path)`

Reads a JSON file from disk, parses it, and returns the decoded value. This is a convenience for `json.parse(fs.read(path))` — if the file doesn't exist or can't be read, it returns an error.

### Basic usage

Given a file `users.json` with content `[{"name": "Alice"}, {"name": "Bob"}]`:

```
let users = json.load("users.json")
print users[0].name  // Alice
print users[1].name  // Bob
print len(users)     // 2
```

### Working with a config file

Given `config.json`:
```json
{
  "app_name": "MyApp",
  "version": "2.1.0",
  "features": {
    "dark_mode": true,
    "notifications": false
  }
}
```

```
let config = json.load("config.json")

print config.app_name            // MyApp
print config.version             // 2.1.0
print config.features.dark_mode  // true
```

### Error handling

```
// File doesn't exist
let data = try json.load("missing.json") catch err {
    print "Could not load: " + err
    {}
}
print data  // {}

// File contains invalid JSON
// If bad.json contains: {broken
let result = try json.load("bad.json") catch err {
    print "Parse error: " + err
    null
}
```

---

## `json.save(path, val)`

Encodes a value to JSON and writes it to a file. Returns `true` on success. Creates the file if it doesn't exist, overwrites if it does.

### Basic usage

```
let data = {"score": 100, "level": 5}
json.save("game_state.json", data)
// Creates game_state.json with: {"score":100,"level":5}
```

### Saving a list

```
let todos = [
    {"task": "Write docs", "done": true},
    {"task": "Ship feature", "done": false}
]
json.save("todos.json", todos)
```

### Pretty-printing to file

```
let report = {
    "total_sales": 15420,
    "items": [
        {"name": "Widget A", "qty": 45},
        {"name": "Widget B", "qty": 23}
    ]
}

json.save("report.json", json.parse(json.encode(report, {"pretty": true})))
```

Or more practically, since `json.save` encodes internally, build the pretty version manually:

```
// json.save always writes compact JSON
// For pretty output, write the encoded string directly
fs.write("report_pretty.json", json.encode(report, {"pretty": true}))
```

### Error handling

```
// Writing to a path where the directory doesn't exist
let ok = try json.save("/nonexistent/dir/data.json", {"a": 1}) catch err {
    print "Write failed: " + err
    false
}
print ok  // false
```

---

## Common Patterns

### Reading and transforming API responses

```
// Fetch JSON from an API
let resp = http.get("https://api.example.com/users")
let users = resp.json()

// Filter active users
let active = []
for user in users {
    if user.active {
        active.push(user.name)
    }
}

print "Active users: " + json.encode(active)
```

### Round-tripping data (encode then parse)

Useful for deep-copying a value:

```
let original = {"a": [1, 2, 3]}
let copy = json.parse(json.encode(original))

copy.a.push(4)
print original.a  // [1,2,3]  (unchanged)
print copy.a      // [1,2,3,4]
```

### Building JSON incrementally

```
let result = {}
result.status = "ok"
result.count = 0
result.items = []

// Add items
for i in range(3) {
    let item = {}
    item.id = i
    item.value = (i + 1) * 10
    result.items.push(item)
    result.count += 1
}

print json.encode(result, {"pretty": true})
// {
//   "status": "ok",
//   "count": 3,
//   "items": [
//     {
//       "id": 0,
//       "value": 10
//     },
//     {
//       "id": 1,
//       "value": 20
//     },
//     {
//       "id": 2,
//       "value": 30
//     }
//   ]
// }
```

### Parsing nested JSON from web scraping

```
// JavaScript returns JSON, parse it in Zen
let raw = js("JSON.stringify(document.querySelector('.data').dataset)")
let parsed = json.parse(raw)

for key in parsed {
    print key + ": " + parsed[key]
}
```

### Merging multiple JSON files

```
let defaults = json.load("defaults.json")
let overrides = json.load("overrides.json")

// Simple merge: overrides win
for key in overrides {
    defaults[key] = overrides[key]
}

json.save("merged.json", defaults)
```

### Logging structured data

```
let log_entry = {
    "timestamp": time.now(),
    "level": "info",
    "message": "User logged in",
    "user_id": 12345
}

// Append as newline-delimited JSON
let existing = try json.load("app.log") catch { [] }
existing.push(log_entry)
json.save("app.log", existing)
```

---

## Tips and Gotchas

**JSON keys are always strings.** When you parse `{"name": "Alice"}`, the key `"name"` is a string. This is standard JSON behavior.

**JSON numbers are all numbers.** There's no distinction between integers and floats. When encoding, whole numbers are written without decimals (`42` not `42.0`).

**`json.save` always writes compact JSON.** It doesn't accept options. For pretty-printed output, use `fs.write(path, json.encode(val, {"pretty": true}))`.

**Trailing commas are not valid JSON.** `[1, 2, 3,]` and `{"a": 1,}` will fail to parse. This is stricter than some languages.

**Round-trip order matters for dicts.** Dicts in Zen use `BTreeMap`, so keys are sorted alphabetically. Encoding then parsing a dict preserves key order.

```
let d = {"z": 1, "a": 2}
print json.encode(d)  // {"a":2,"z":1}
```

**Use `try/catch` around `json.parse` and `json.load`.** Invalid JSON returns errors, not null values.

**Encode before writing.** `json.save(path, val)` encodes internally. If you need pretty output, write the encoded string yourself with `fs.write`.

**Round-tripping creates a deep copy.** `json.parse(json.encode(val))` is a reliable way to deep-copy a value, since all nested structures are fully serialized and reconstructed.

**Functions and instances become strings.** You can't serialize a function to JSON and get it back. The encoded form is a placeholder string like `"<function:myFunc>"`.
