# JSON

## Module Methods

```
json.parse('{"a": 1}')    // {a: 1}
json.encode({a: 1})       // '{"a":1}'
json.encode({a: 1}, true) // pretty-print with indentation
json.load("data.json")    // read & parse JSON file
json.save("out.json", val) // write JSON to file
```

## Flat Functions

```
json_parse('{"a": 1}')    // {a: 1}
json_encode({a: 1})       // '{"a": 1}'
```

## Examples

```
// Parse JSON string
let data = json.parse('{"name": "Zen", "version": "0.1.0"}')
print data["name"]    // Zen

// Pretty print
let config = {debug: true, port: 8080}
print json.encode(config, true)
// {
//     "debug": true,
//     "port": 8080
// }

// Read from file
let users = json.load("users.json")

// Write to file
json.save("output.json", {result: "success"})
```
