# Error Handling

## Error Types

| Error | Source | Description |
|-------|--------|-------------|
| `LexerError` | Lexer | Unexpected character, bad token |
| `ParseError` | Parser | Syntax error, missing token |
| `ZenError` | Interpreter | Runtime error (undefined var, type error) |
| `DrissionPage errors` | Browser | Timeout, element not found, etc. |

## Try/Catch/Finally

```
try {
    let el = find(".might-not-exist")
    el.click()
} catch {
    print "Element was missing"
    print error    // the error message
}
```

With named error variable:

```
try {
    risky_operation()
} catch err {
    print "Caught: " + err
}
```

With finally (always runs):

```
try {
    open_file()
} catch err {
    print "Error: " + err
} finally {
    print "This always runs"
}
```

Inside the `catch` block, the error is bound to the variable name you give, or `error` by default.

## Throw / Raise

Explicitly raise an exception:

```
throw "Something went wrong"
raise "Invalid input"
```

With custom error objects:

```
throw {code: 404, message: "Not found"}
```

## Assert

Debug assertions that raise on failure:

```
let x = 10
assert x > 0
assert x > 0, "x must be positive"
```

## Error Display

Errors show source context with line/column markers:

```
Parse Error: Expected RPAREN, got ASSIGN('=')
  >>> find(text="hello"
                     ^
  Error: Expected RPAREN, got ASSIGN('=')
```

## Common Errors

**"Undefined variable"** — variable name is misspelled or not declared:

```
print greeting    // Error: Undefined variable: greeting
```

**"Not callable"** — trying to call something that isn't a function:

```
let x = 42
x()                // Error: Not callable: 42
```

**"Element not found"** — the selector didn't match any element.

**"Cannot redefine constant"** — trying to reassign a `const`:

```
const X = 10
X = 20             // Error: Cannot redefine constant 'X'
```
