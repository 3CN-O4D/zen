# Variables

Variables hold values. There are three declaration keywords — `let`, `var`,
and `const` — plus the very detailed rules about what's expected at runtime:
**`let` and `var` are identical** (`let` is sugar), and **`const` is enforced**.

## Declaring variables

```zen
var x = 10          # a rebindable variable
let y = "text"      # let == var
const z = 3.14      # a constant

z = 1               # Error: cannot assign to constant: z
```

### `var` vs `let`

There is no behavioral difference. `let` is accepted as an alias for `var`,
so code that mixes styles is fine:

```zen
let a = 1
var b = 2
a = 3               # fine — let is rebindable
```

### `const`

`const` binds a name to a value that cannot be changed after initialization.
Any attempt to reassign a `const` variable is a compile-time error.

```zen
const PI = 3.14159
PI = 3              # Error: cannot assign to constant: PI
```

### Scope rules

Variables are scoped to the block they are declared in. Blocks are created
by function bodies, `if`/`else` branches, `while`/`for` loops, and explicit
`{ ... }` blocks. Inner scopes can shadow outer names.

```zen
var x = 1
if true {
    var x = 2       # shadows the outer x
    print x         # 2
}
print x             # 1
```

### Mutability vs reassignment

`var`/`let` control rebinding (changing which value a name points to).
Mutable containers (lists, dicts) can still be mutated through a `const`
binding — the binding is constant, not the object.

```zen
const xs = [1, 2]
xs.push(3)          # ok: mutating the list, not rebinding xs
xs = [4]            # Error: cannot assign to constant: xs
```

### Destructuring

Zen supports destructuring in declarations:

```zen
let [a, b, c] = [1, 2, 3]
let {x, y} = {"x": 10, "y": 20}
```

### Variable interpolation

Variables can be interpolated in string literals using `${}`:

```zen
let name = "Ada"
print "Hello, ${name}!"  # Hello, Ada!
```

### Constant expressions

`const` variables can be initialized with constant expressions (literals
and other `const` variables):

```zen
const WIDTH = 800
const HEIGHT = 600
const AREA = WIDTH * HEIGHT  # evaluated at compile time
```