# Variables in Zen

Variables store data in the Zen programming language.

- `let` creates a mutable variable: `let x = 1`
- `const` creates an immutable constant: `const PI = 3.14`
- `global` creates a globally visible variable
- `let` variables can be reassigned; `const` variables cannot
- Variable names must start with a letter and contain letters, digits, underscores
- Variables are scoped to the block where they're declared
- Declaring `let str = "x"` shadows the built-in `str()` function
- Variables have no explicit type; they take the type of their assigned value

```zen
let x = 42              // mutable integer
x = 100                 // ok

const y = 99            // immutable
// y = 88               // error: const reassigned
```
