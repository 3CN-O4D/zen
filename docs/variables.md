# Variables in Zen

## Declaration

```zen
let x = 42              // mutable
const PI = 3.14         // immutable
global counter = 0      // global scope
```

## Rules
- `let` variables can be reassigned; `const` cannot
- Names: letters, digits, underscores (must start with letter)
- Scoped to the block where declared
- Shadowing built-ins: `let str = "x"` shadows `str()`

## Destructuring

```zen
let [a, b] = [1, 2]
let {x, y} = {x: 1, y: 2}
const [C, D] = [1, 2]
```
