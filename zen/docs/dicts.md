# Dictionaries in Zen

Key-value pairs with `{}` syntax.

```zen
let d = {name: "Ada", age: 36}
d.keys()                     // ["name", "age"]
d.values()                   // ["Ada", 36]
d.len()                      // 2 (or .length property)
d.contains("name")           // true
d.get("name", "default")     // value or default
{...d, city: "London"}        // spread
```
