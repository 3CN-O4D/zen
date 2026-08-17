# JSON Module

Complete reference for encoding, decoding, loading, and saving JSON data in Zen.

## Quick Start

```
// Parse a JSON string
let data = json.parse('{"name": "Alice", "age": 30}')
print data.name    // Alice

// Encode a value to JSON
let json_str = json.encode({"lang": "zen", "version": 1})
print json_str     // {"lang":"zen","version":1}

// Load from file
let config = json.load("config.json")

// Save to file
json.save("output.json", data)
```

---

## `json.parse(str)`

Decodes a JSON string into a Zen value.

### Basic types

```
json.parse('"hello"')     // "hello" (string)
json.parse("42")          // 42 (number)
json.parse("3.14")        // 3.14 (number)
json.parse("-7")          // -7 (number)
json.parse("true")        // true (boolean)
json.parse("false")       // false (boolean)
json.parse("null")        // null
```

### Objects

```
let obj = json.parse('{"x": 1, "y": 2}')
print obj.x               // 1
print obj["y"]            // 2
```

### Arrays

```
let arr = json.parse('[10, 20, 30]')
print arr[0]              // 10
print arr.len             // 3
```

### Nested structures

```
let deep = json.parse('{"users": [{"name": "Bob", "active": true}, {"name": "Eve", "active": false}]}')
print deep.users[0].name     // Bob
print deep.users[1].active   // false
```

### Escape sequences

```
json.parse('"line1\\nline2"')      // "line1\nline2"
json.parse('"tab\\there"')         // "tab\there"
json.parse('"quote: \\""')         // "quote: \""
json.parse('"back\\\\slash"')      // "back\\slash"
json.parse('"unicode \\u0041"')    // "unicode A"
```

### Error handling

```
json.parse("not json")
// Error: invalid JSON value

json.parse("42 hello")
// Error: trailing characters in JSON

json.parse('"oops')
// Error: unterminated JSON string

json.parse('{"a": 1 "b": 2}')
// Error: expected ',' or '}' in JSON object
```

### Safe parsing with try/catch

```
let input = "definitely not json"
let result = try json.parse(input) catch err {
    print "Parse failed: " + err
    null
}
print result    // null
```

---

## `json.encode(val)` / `json.stringify(val)`

Encodes a Zen value to a JSON string. `json.stringify` is an alias.

### Basic encoding

```
json.encode("hello")      // "hello"
json.encode(42)           // 42
json.encode(3.14)         // 3.14
json.encode(-100)         // -100
json.encode(true)         // true
json.encode(false)        // false
json.encode(null)         // null
json.encode([1, 2, 3])   // [1,2,3]
json.encode({"a": 1})    // {"a":1}
```

### Encoding nested data

```
let user = {
    "name": "Alice",
    "scores": [95, 87, 92],
    "address": {"city": "Nairobi", "zip": "00100"}
}
print json.encode(user)
// {"name":"Alice","scores":[95,87,92],"address":{"city":"Nairobi","zip":"00100"}}
```

### Pretty-printing

```
let data = {"name": "Alice", "hobbies": ["reading", "hiking"]}

// Compact (default)
print json.encode(data)
// {"name":"Alice","hobbies":["reading","hiking"]}

// Pretty-printed
print json.encode(data, {"pretty": true})
// {
//   "name": "Alice",
//   "hobbies": [
//     "reading",
//     "hiking"
//   ]
// }
```

### String escaping

```
json.encode('He said "hi"')     // "He said \"hi\""
json.encode("path\\to\\file")   // "path\\\\to\\\\file"
json.encode("line1\nline2")     // "line1\nline2"
```

### Non-serializable values

Functions, instances, and sockets become placeholder strings:

```
json.encode(print)        // "<native:print>"
json.encode([1, print])   // [1,"<native:print>"]
```

### Empty containers

```
json.encode({})    // {}
json.encode([])    // []
```

---

## `json.load(path)`

Reads a JSON file from disk and parses it.

### Basic usage

```
// Given config.json: {"port": 8080, "debug": true}
let config = json.load("config.json")
print config.port       // 8080
print config.debug      // true
```

### Error handling

```
// File doesn't exist
let data = try json.load("missing.json") catch err {
    print "Could not load: " + err
    {}
}
print data    // {}

// Invalid JSON in file
let result = try json.load("bad.json") catch err {
    print "Parse error: " + err
    null
}
```

---

## `json.save(path, val)`

Encodes a value to JSON and writes it to a file.

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

### Pretty output to file

`json.save` always writes compact JSON. For pretty output:

```
let data = {"name": "Zen", "version": "1.0"}
fs.write("config.json", json.encode(data, {"pretty": true}))
```

### Error handling

```
let ok = try json.save("/nonexistent/dir/data.json", {"a": 1}) catch err {
    print "Write failed: " + err
    false
}
print ok    // false
```

---

## Common Patterns

### Read-transform-write pipeline

```
let data = json.load("input.json")

// Transform
let result = []
for item in data {
    if item["active"] {
        result.append({
            "name": item["name"],
            "processed": true
        })
    }
}

json.save("output.json", result)
```

### Round-trip deep copy

```
let original = {"a": [1, 2, 3]}
let copy = json.parse(json.encode(original))

copy.a.push(4)
print original.a    // [1, 2, 3] (unchanged)
print copy.a        // [1, 2, 3, 4]
```

### Merging configs

```
let defaults = {"host": "localhost", "port": 8080, "debug": false}
let overrides = json.load("overrides.json")

// Merge: overrides win
for key in overrides {
    defaults[key] = overrides[key]
}

json.save("config.json", defaults)
```

### Incremental building

```
let result = {}
result.status = "ok"
result.count = 0
result.items = []

for i in range(3) {
    let item = {}
    item.id = i
    item.value = (i + 1) * 10
    result.items.push(item)
    result.count += 1
}

print json.encode(result, {"pretty": true})
```

---

## Tips and Gotchas

### JSON keys are always strings

```
let d = {1: "one"}
print json.encode(d)    // {"1":"one"} (key is string "1")
```

### Trailing commas are invalid

```
json.parse("[1, 2, 3,]")     // ERROR
json.parse('{"a": 1,}')      // ERROR

// Zen lists allow trailing commas, but JSON does not
```

### Dict key order

Dicts use `BTreeMap` internally, so keys are sorted alphabetically:

```
let d = {"z": 1, "a": 2}
print json.encode(d)    // {"a":2,"z":1}
```

### `json.save` writes compact JSON

For pretty output, use `fs.write(path, json.encode(val, {"pretty": true}))`.

### Functions become strings

You can't serialize a function and get it back:

```
json.encode(print)    // "<native:print>"
```

---

## Pro Tips

1. **Use `try/catch` around `json.parse`.** Invalid JSON throws errors.
2. **Use `json.load` over `fs.read` + `json.parse`.** One call does both.
3. **Use `json.save` for simple writes.** For pretty output, use `fs.write` + `json.encode`.
4. **Round-trip for deep copy.** `json.parse(json.encode(val))` creates a deep copy.
5. **Check `resp.ok` before `resp.json()`.** Not all HTTP responses are valid JSON.

---

## Common Mistakes

### Forgetting trailing commas

```
// WRONG — trailing comma is invalid JSON
json.parse('[1, 2, 3,]')

// CORRECT
json.parse('[1, 2, 3]')
```

### Not handling parse errors

```
// WRONG — crashes on invalid JSON
let data = json.parse(user_input)

// CORRECT — handle errors
let data = try json.parse(user_input) catch { {} }
```

### Assuming key order

```
// Dicts are sorted alphabetically
let d = {"b": 2, "a": 1}
print json.encode(d)    // {"a":1,"b":2} (not {"b":2,"a":1})
```

---

## See Also

- [fs Module](fs.md) — Reading/writing files
- [http Module](http.md) — API responses
- [Module Overview](overview.md) — All available modules
