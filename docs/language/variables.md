# Variables

## Declaration

```
let name = "Zen"
let count = 42
```

Bare assignment auto-declares if the variable doesn't exist:

```
name = "Zen"    // same as let name = "Zen"
```

## Constants

```
const PI = 3.14
const MAX_RETRIES = 3
```

Constants cannot be reassigned:

```
const X = 10
X = 20          // Error: Cannot redefine constant 'X'
```

## Reassignment

```
let x = 10
x = 20
```

## Scope Rules

- Variables declared with `let` inside a block `{ }` are scoped to that block
- Functions create their own scope (closures)
- Inner scopes can read from outer scopes
- Assignment to an existing name always targets the innermost scope

```
let x = 10
if true {
    let x = 20    // different x (block-scoped)
    print x       // 20
}
print x           // 10
```

## Special Variables

| Variable | Description |
|----------|-------------|
| `_url` | Current page URL |
| `__url` | Previous page URL |
| `___url` | URL before previous |
| `_time` | Current time (HH:MM:SS) |
| `_date` | Current date (YYYY-MM-DD) |
| `_dir` | Current working directory |
| `_version` | Zen version string |
| `_` | Last expression result (also throwaway in unpacking) |
| `_timeout` | Default timeout (read/write) |

## Tuple Unpacking

Comma-separated targets on the left of `=` unpack values from a list or comma-separated expression on the right:

```
a, b = 1, 2            // a=1, b=2
x, y = [10, 20]        // x=10, y=20
first, second = range(5)   // first=0, second=1
```

Use `_` as a throwaway target to discard values:

```
a, _, c = 1, 2, 3      // a=1, c=3 (2 discarded)
name, _ = ["Alice", "alice@example.com"]
```

The number of targets and values must match exactly. Note: `let` does not support unpacking — use bare assignment.

## Destructuring

### Array Destructuring

```
let [a, b] = [1, 2]           // a=1, b=2
let [x, y, z] = [1, 2, 3]    // x=1, y=2, z=3
```

### Object Destructuring

```
let {name, age} = {name: "Alice", age: 30}   // name="Alice", age=30
let {name, city} = {name: "Bob", city: "NYC"}   // name="Bob", city=null
```

## Compound Assignment

Arithmetic combined with assignment modifies a variable in place:

```
x += 5       // x = x + 5
x -= 3       // x = x - 3
x *= 2       // x = x * 2
x /= 4       // x = x / 4
x %= 10      // x = x % 10
```

Works on variables, member access (`obj.prop += 1`), and index access (`list[i] += 1`).

### Nullish Assignment

Only assigns if the variable is null:

```
let x = null
x ??= "default"   // x = "default"

let y = "existing"
y ??= "other"     // y = "existing" (unchanged)
```

## Postfix Increment / Decrement

```
x++          // x = x + 1
x--          // x = x - 1
```

Valid targets: variables, members, and index expressions.

## Timeout Values

Set navigation/finding timeouts with `_timeout`:

```
_timeout = 5000          // 5000ms (5 seconds)
_timeout = "3s"          // 3 seconds
_timeout = "1.5s"        // 1.5 seconds
_timeout = "500ms"       // 500 milliseconds
_timeout = "2m"          // 2 minutes
print _timeout           // 30000 (default in ms)
```
