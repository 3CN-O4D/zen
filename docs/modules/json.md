# json — JSON encode/decode

The `json` module provides functions for converting between Zen values (dicts, lists, etc.) and JSON strings. It is available globally as `json`.

```zen
# 1. Encoding a dict to a JSON string
var user = { name: "Ada", age: 36 }
var text = json.stringify(user)
print(text)  # {"name":"Ada","age":36}

# 2. Parsing a JSON string back to a dict
var data = json.parse(text)
print(data.name)  # Ada
```

## Functions

| Function | Description |
|----------|-------------|
| `parse(string)` | Decodes a JSON string into a Zen value (dict, list, string, number, etc.). |
| `encode(value)` | Encodes a Zen value into a compact JSON string. |
| `stringify(value)` | Alias for `encode`. |
| `load(path)` | Reads a file and parses its contents as JSON. |
| `save(path, value)` | Encodes a value as JSON and writes it to a file. |

## Type Mapping

| JSON Type | Zen Type |
|-----------|----------|
| Object | `dict` |
| Array | `list` |
| String | `string` |
| Number | `int` or `float` |
| true / false | `bool` |
| null | `null` |

## Examples

### Reading and updating a JSON config
```zen
# Load existing config
var config = json.load("config.json")

# Update a value
config.last_run = time.now()

# Save it back
json.save("config.json", config)
```

### Parsing JSON from an API
```zen
var resp = http.get("https://api.example.com/data")
if resp.ok {
    var data = json.parse(resp.text())
    print("Total items: ${data.total}")
}
```

## See Also
- [http](http.md) — Using `.json()` on response objects.
- [fs](fs.md) — For manual file reading/writing.
