# json — JSON parsing and output

Pure-Zen JSON that shadows the native tier (which stays reachable as
`json.native.*`). Full `parse`/`stringify` parity with the native
implementation, including pretty printing.

```zen
import json

let v = json.parse("{\"a\": [1, 2]}")
print json.stringify(v)  # {"a":[1,2]}
```

## Functions

| Function | Description |
|----------|-------------|
| `parse(s)` | Parses JSON text (objects, arrays, strings with escapes, numbers, `true`/`false`/`null`). Throws on invalid input. |
| `stringify(v, indent)` | Renders compact JSON, or indented with `indent` spaces. |
| `pretty(v)` | Indented rendering. |
| `encode(v)` | Alias for compact `stringify`. |

## Example

```zen
import json

print json.stringify({"b": [1, 2], "a": true})
# {"b":[1,2],"a":true}
print json.stringify({"b": [1, 2]}, 2)
# {
#   "b": [
#     1,
#     2
#   ]
# }
```
