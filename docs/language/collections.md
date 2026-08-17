# Lists & Dicts

## Lists

```
let items = [1, "two", true, 4.0]
print items[0]        // 1
print items[-1]       // 4.0 (negative indexing)
items.append(5)       // [1, "two", true, 4.0, 5]
items.len             // 5
items.count           // 5 (alias for len)
```

Lists accept trailing commas and spread (`...`) for unpacking:

```
let nums = [1, 2, 3,]
let merged = [...nums, 4, 5]     // [1, 2, 3, 4, 5]
let double = [...nums, ...nums]  // [1, 2, 3, 1, 2, 3]
```

## List Comprehensions

Create lists from expressions with optional filtering:

```
let squares = [x ** 2 for x in 1 -> 5]
// [1, 4, 9, 16, 25]

let evens = [x for x in 1 -> 10 if x % 2 == 0]
// [2, 4, 6, 8, 10]

let names = [person.name for person in people if person.age > 18]
```

## List Methods

| Method | Description |
|--------|-------------|
| `.append(x)` | Add to end |
| `.pop()` | Remove and return last item |
| `.push(x)` | Alias for append |
| `.shift()` | Remove and return first item |
| `.unshift(x)` | Add to front |
| `.sort()` | Sort in-place |
| `.reverse()` | Reverse in-place |
| `.clear()` | Remove all items |
| `.len` | Number of items |
| `.includes(x)` | Check if item exists |
| `.indexOf(x)` | Find index of item (-1 if not found) |
| `.join(sep)` | Join items into string |
| `.sorted()` | Return new sorted list |
| `.to_list()` | Convert to Python list |
| `.map(fn)` | Apply function to each item |
| `.filter(fn)` | Keep items where function returns truthy |
| `.reduce(fn)` | Fold list to single value |

## Dicts

```
let config = {"host": "localhost", "port": 8080}
print config["host"]        // localhost
config["port"] = 9090       // modify
config["debug"] = true      // add key
config.len                  // 3
```

Bare keys work too (like JavaScript):

```
let user = {name: "Alice", age: 30}
print user.name             // Alice
```

## Dict Methods

| Method | Description |
|--------|-------------|
| `.keys()` | List of key names |
| `.values()` | List of values |
| `.items()` | List of [key, value] pairs |
| `.get(key)` | Get value or null |
| `.get(key, default)` | Get value or default |
| `.put(key, value)` | Set and return dict (for chaining) |
| `.has(key)` | Check if key exists |
| `.len` | Number of entries |
| `.clear()` | Remove all entries |
| `.is_empty()` | `true` if dict has no entries |

## Dict Member Priority

Dict keys are checked before built-in methods. So `http.get` resolves to the HTTP GET function, not `dict.get()`.
