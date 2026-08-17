# Lists & Dicts

Complete reference for Zen's two collection types — lists and dicts — including all methods, comprehensions, destructuring, spread, and nested patterns.

## Lists

### Creating lists

```
let empty = []
let nums = [1, 2, 3]
let mixed = [1, "two", true, 3.14, null]
let nested = [[1, 2], [3, 4], [5, 6]]
```

### Trailing commas are allowed

```
let nums = [1, 2, 3,]    // same as [1, 2, 3]
```

### Accessing elements

```
let items = [10, 20, 30]

print items[0]           // 10 (first)
print items[-1]          // 30 (last)
print items[-2]          // 20 (second from last)
print items[1]           // 20 (second)
```

### Negative indexing

```
let letters = ["a", "b", "c", "d", "e"]

print letters[-1]       // e
print letters[-2]       // d
print letters[-3]       // c
```

### Modifying elements

```
let items = [10, 20, 30]
items[0] = 99
print items             // [99, 20, 30]

items[-1] = 100
print items             // [99, 20, 100]
```

---

## List Methods

| Method | Description | Example |
|--------|-------------|---------|
| `.append(x)` | Add to end | `[1,2].append(3)` → `[1,2,3]` |
| `.push(x)` | Alias for append | `[1,2].push(3)` → `[1,2,3]` |
| `.pop()` | Remove and return last | `[1,2,3].pop()` → `3` |
| `.shift()` | Remove and return first | `[1,2,3].shift()` → `1` |
| `.unshift(x)` | Add to front | `[2,3].unshift(1)` → `[1,2,3]` |
| `.insert(i, x)` | Insert at index | `[1,3].insert(1,2)` → `[1,2,3]` |
| `.sort()` | Sort in-place | `[3,1,2].sort()` → `[1,2,3]` |
| `.reverse()` | Reverse in-place | `[1,2,3].reverse()` → `[3,2,1]` |
| `.clear()` | Remove all | `[1,2,3].clear()` → `[]` |
| `.len` | Number of items | `[1,2,3].len` → `3` |
| `.count` | Alias for len | `[1,2,3].count` → `3` |
| `.includes(x)` | Check existence | `[1,2,3].includes(2)` → `true` |
| `.indexOf(x)` | Find index | `[1,2,3].indexOf(2)` → `1` |
| `.join(sep)` | Join into string | `["a","b"].join("-")` → `"a-b"` |
| `.sorted()` | Return new sorted list | `[3,1,2].sorted()` → `[1,2,3]` |
| `.map(fn)` | Transform each | `[1,2,3].map((x) => x*2)` → `[2,4,6]` |
| `.filter(fn)` | Keep matching | `[1,2,3].filter((x) => x>1)` → `[2,3]` |
| `.reduce(fn)` | Fold to value | `[1,2,3].reduce((a,b) => a+b)` → `6` |

### append vs push

They are identical — `push` is an alias for `append`:

```
let items = [1, 2]
items.append(3)    // [1, 2, 3]
items.push(4)      // [1, 2, 3, 4]
```

### sort and sorted

`sort()` modifies in-place and returns nothing useful. `sorted()` returns a new sorted list:

```
let a = [3, 1, 2]
a.sort()
print a              // [1, 2, 3] (modified)

let b = [3, 1, 2]
let c = b.sorted()
print b              // [3, 1, 2] (unchanged)
print c              // [1, 2, 3] (new list)
```

### map, filter, reduce

```
let nums = [1, 2, 3, 4, 5]

// map: transform each element
let doubled = nums.map((x) => x * 2)
print doubled       // [2, 4, 6, 8, 10]

// filter: keep elements where function returns truthy
let evens = nums.filter((x) => x % 2 == 0)
print evens         // [2, 4]

// reduce: accumulate into single value
let sum = nums.reduce((acc, x) => acc + x)
print sum           // 15

// reduce with initial value
let product = nums.reduce((acc, x) => acc * x, 1)
print product       // 120
```

---

## Spread Operator for Lists

Unpacks a list inside another list:

```
let a = [1, 2, 3]
let b = [...a, 4, 5]
print b              // [1, 2, 3, 4, 5]

let c = [...a, ...a]
print c              // [1, 2, 3, 1, 2, 3]

let d = [0, ...a]
print d              // [0, 1, 2, 3]
```

### Shallow copy with spread

```
let original = [1, 2, 3]
let copy = [...original]
copy.append(4)

print original       // [1, 2, 3] (unchanged)
print copy           // [1, 2, 3, 4]
```

---

## List Comprehensions

Create lists from expressions with optional filtering:

### Basic comprehension

```
let squares = [x ** 2 for x in 1 -> 5]
print squares    // [1, 4, 9, 16, 25]
```

### With filter

```
let evens = [x for x in 1 -> 10 if x % 2 == 0]
print evens      // [2, 4, 6, 8, 10]
```

### Complex expressions

```
let phrases = [name.upper() for name in ["alice", "bob", "charlie"]]
print phrases    // [ALICE, BOB, CHARLIE]
```

### Nested comprehension

```
let flat = [cell for row in matrix for cell in row]
```

### Comprehension with function calls

```
let lengths = [word.len for word in ["hello", "world", "zen"]]
print lengths    // [5, 5, 3]
```

---

## Destructuring with Lists

### Array destructuring

```
let [a, b] = [1, 2]
print a              // 1
print b              // 2

let [x, y, z] = [10, 20, 30]
print x, y, z       // 10, 20, 30
```

### Throwaway with `_`

```
let [first, _, third] = [1, 2, 3]
print first          // 1
print third          // 3
```

### In for loops

```
let pairs = [[1, "a"], [2, "b"], [3, "c"]]
for num, letter in pairs {
    print "{num}: {letter}"
}
// 1: a
// 2: b
// 3: c
```

---

## Dicts

### Creating dicts

```
let empty = {}
let config = {"host": "localhost", "port": 8080}
let user = {name: "Alice", age: 30}    // bare keys (no quotes)
```

### Bare keys vs quoted keys

```
// Both are equivalent:
let a = {name: "Alice"}        // bare key
let b = {"name": "Alice"}      // quoted key

// Bare keys only work for valid identifiers:
let c = {valid_key: 1}         // OK
let d = {"invalid-key": 1}     // OK (must quote)
let e = {123: 1}               // OK (must quote)
```

### Accessing values

```
let user = {"name": "Alice", "age": 30}

print user["name"]       // Alice
print user.name           // Alice (dot notation)
print user["missing"]    // null (no error for missing keys)
```

### Modifying dicts

```
let data = {}

// Add
data["key"] = "value"
data.new_key = "another"    // dot notation

// Update
data["key"] = "updated"

// Delete
delete data["key"]          // remove a key
```

---

## Dict Methods

| Method | Description | Example |
|--------|-------------|---------|
| `.keys()` | List of keys | `{a:1,b:2}.keys()` → `[a,b]` |
| `.values()` | List of values | `{a:1,b:2}.values()` → `[1,2]` |
| `.items()` | List of [k,v] pairs | `{a:1}.items()` → `[[a,1]]` |
| `.get(key)` | Get value or null | `{a:1}.get("b")` → `null` |
| `.get(key, default)` | Get value or default | `{a:1}.get("b",0)` → `0` |
| `.put(key, val)` | Set and return dict | `{a:1}.put("b",2)` → `{a:1,b:2}` |
| `.has(key)` | Check if key exists | `{a:1}.has("a")` → `true` |
| `.len` | Number of entries | `{a:1,b:2}.len` → `2` |
| `.count` | Alias for len | `{a:1,b:2}.count` → `2` |
| `.clear()` | Remove all entries | `{a:1}.clear()` → `{}` |
| `.is_empty()` | `true` if no entries | `{}.is_empty()` → `true` |

### Iterating over dicts

```
let user = {name: "Alice", age: 30, city: "NYC"}

// Iterate keys
for key in user {
    print key
}
// name, age, city

// Iterate with values
for key in user {
    print "{key}: {user[key]}"
}
// name: Alice
// age: 30
// city: NYC

// Using items()
for key, value in user.items() {
    print "{key}: {value}"
}
```

### get vs bracket access

```
let data = {"a": 1}

// Bracket: returns null for missing keys
print data["b"]         // null

// get: returns null (or default) for missing keys
print data.get("b")     // null
print data.get("b", 0)  // 0

// Both return null — but get is clearer for defaults
let val = data.get("b") ?? "not found"
```

### Dict member priority

Dict keys are checked before built-in methods. So `http.get` resolves to the HTTP GET function, not `dict.get()`:

```
let d = {"get": "custom"}
print d.get    // "custom" (dict key takes priority)
```

---

## Spread Operator for Dicts

```
let defaults = {"color": "blue", "size": "medium"}
let overrides = {"color": "red"}

// Merge dicts (overrides win)
let result = {...defaults, ...overrides}
print result    // {color: red, size: medium}

// Add new keys
let final = {...defaults, ...overrides, "weight": 10}
print final     // {color: red, size: medium, weight: 10}
```

### Shallow copy with spread

```
let original = {"a": 1, "b": 2}
let copy = {...original}
copy["c"] = 3

print original    // {a: 1, b: 2} (unchanged)
print copy        // {a: 1, b: 2, c: 3}
```

---

## Nested Collections

### Nested lists

```
let matrix = [
    [1, 2, 3],
    [4, 5, 6],
    [7, 8, 9]
]

print matrix[0][0]    // 1
print matrix[1][2]    // 6
print matrix[2][1]    // 8
```

### Nested dicts

```
let users = {
    "alice": {"age": 30, "active": true},
    "bob": {"age": 25, "active": false}
}

print users["alice"]["age"]       // 30
print users.bob.active            // false
```

### Deep nesting

```
let data = {
    "company": {
        "departments": [
            {
                "name": "Engineering",
                "employees": [
                    {"name": "Alice", "role": "Senior"},
                    {"name": "Bob", "role": "Junior"}
                ]
            }
        ]
    }
}

print data.company.departments[0].employees[0].name    // Alice
```

---

## Destructuring with Dicts

```
let {name, age} = {name: "Alice", age: 30, email: "a@b.com"}
print name           // Alice
print age            // 30

// Missing keys become null
let {name, city} = {name: "Bob"}
print city           // null
```

---

## Pro Tips

1. **Use `get(key, default)` for safe access.** Avoids null checks.
2. **Use spread for merging.** `{...defaults, ...overrides}` is clean and clear.
3. **Use comprehensions for transforms.** `[f(x) for x in list]` is more readable than loops.
4. **Use `items()` for key-value iteration.** Cleaner than `for key in dict { dict[key] }`.
5. **Use `{}` for default values.** `config = json.parse(raw) catch { {} }` is safe.
6. **`includes()` for membership.** More readable than `indexOf() >= 0`.

---

## Common Mistakes

### Lists are reference types

```
let a = [1, 2, 3]
let b = a              // b points to the same list
b.append(4)
print a                // [1, 2, 3, 4] — a was modified!

// Use spread to copy:
let c = [...a]
c.append(5)
print a                // [1, 2, 3, 4] — a is unchanged
```

### Missing key returns null, not error

```
let data = {"a": 1}
print data["b"]        // null (no error)
print data.b           // null (no error)
```

### Dict keys are always strings

```
let d = {1: "one", 2: "two"}
print d[1]             // "one" (key is the string "1", not number 1)
```

### sort() modifies in-place

```
let a = [3, 1, 2]
a.sort()
print a                // [1, 2, 3] — original is modified!

// Use sorted() to keep original unchanged
let b = [3, 1, 2]
let c = b.sorted()
print b                // [3, 1, 2] — unchanged
print c                // [1, 2, 3] — new list
```

---

## See Also

- [Types](types.md) — List and dict type information
- [Operators](operators.md) — Spread operator and membership
- [Functions](functions.md) — Map, filter, reduce
- [Variables](variables.md) — Destructuring assignment
