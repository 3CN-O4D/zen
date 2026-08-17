# Error Handling

Complete reference for try/catch/finally, throw/raise, custom error classes, and best practices for error handling in Zen.

## Error Types

| Error Type | Source | Description |
|-----------|--------|-------------|
| `LexerError` | Lexer | Unexpected character, unterminated string |
| `ParseError` | Parser | Syntax error, missing token |
| `RuntimeError` | Interpreter | Undefined variable, type error, etc. |
| `errors.*` | Built-in | Standard error hierarchy |

### Built-in error classes

```
// Available in the errors module:
errors.Error                  // Base error class
errors.TypeError              // Type mismatches
errors.ValueError             // Invalid values
errors.IndexError             // Out-of-bounds access
errors.KeyError               // Missing dict key
errors FileNotFoundError      // File not found
errors.ZeroDivisionError      // Division by zero
errors.ArithmeticError        // Math errors
errors.RuntimeError           // General runtime errors
errors.NotImplementedError    // Unimplemented features
errors.StopIteration          // Iterator exhausted
errors.AssertionError         // Assert failed
errors.ImportError            // Module not found
errors.RecursionError         // Stack overflow
errors.OSError                // OS-level errors
errors.SystemExit             // Program exit
errors.OverflowError          // Numeric overflow
errors.IOError                // I/O errors
errors.KeyboardInterrupt      // Ctrl+C
```

---

## Try / Catch / Finally

### Basic try/catch

```
try {
    let data = json.parse("not valid json")
} catch {
    print "Parse failed"
    print error    // built-in variable with error message
}
```

### Named error variable

```
try {
    risky_operation()
} catch err {
    print "Caught: " + err
}
```

### With finally

The `finally` block runs whether or not an error occurred:

```
let resource = null
try {
    resource = acquire_resource()
    use(resource)
} catch err {
    print "Error: " + err
} finally {
    if resource != null {
        release_resource(resource)
    }
    print "Cleanup done"
}
```

### Try as expression

`try` returns a value — the expression result or the catch block result:

```
let result = try json.parse("bad input") catch { null }
print result    // null (instead of crashing)
```

### Catch with no variable

```
try {
    risky_operation()
} catch {
    print "Something went wrong"
    print error    // still available as built-in
}
```

---

## Throw / Raise

### Throw a string

```
throw "Something went wrong"
raise "Invalid input"     // raise is an alias for throw
```

### Throw an error object

```
throw {code: 404, message: "Not found"}
throw {type: "AuthError", message: "Unauthorized", status: 401}
```

### Throw in functions

```
function validate_age(age) {
    if age < 0 {
        throw "Age cannot be negative"
    }
    if age > 150 {
        throw "Age seems unrealistic"
    }
    return true
}

try {
    validate_age(-5)
} catch err {
    print err    // Age cannot be negative
}
```

### Re-throw

```
try {
    try {
        throw "original error"
    } catch err {
        print "Inner catch: " + err
        throw err    // re-throw to outer handler
    }
} catch err {
    print "Outer catch: " + err
}
// Inner catch: original error
// Outer catch: original error
```

---

## Custom Error Classes

### Defining custom errors

```
errors.define("ValidationError", "Error", "validation failed")
errors.define("NotFoundError", "Error", "resource not found")
errors.define("AuthError", "Error", "authentication required")
```

### Using custom errors

```
errors.define("ValidationError", "Error", "validation failed")

function validate_email(email) {
    if !email.includes("@") {
        throw new ValidationError("invalid email: " + email)
    }
    return true
}

try {
    validate_email("not-an-email")
} catch ValidationError as err {
    print "Validation: " + err
} catch {
    print "Other error: " + error
}
// Validation: validation failed
```

### Error inheritance

Custom errors inherit from their parent:

```
errors.define("BaseError", "Error", "base error")
errors.define("ChildError", "BaseError", "child error")

try {
    throw new ChildError("details")
} catch BaseError as err {
    // This catches both ChildError and BaseError
    print "Caught: " + err
}
```

### Catching specific error types

```
errors.define("NetworkError", "Error", "network failure")
errors.define("TimeoutError", "NetworkError", "request timed out")

try {
    throw new TimeoutError("request took too long")
} catch TimeoutError as err {
    print "Timeout: " + err
} catch NetworkError as err {
    print "Network: " + err
} catch {
    print "Other: " + error
}
// Timeout: request timed out
```

---

## Assert

Debug assertions that raise on failure:

```
let x = 10
assert x > 0
print "x is positive"

// With custom message
assert x > 0, "x must be positive"
```

### Assert in functions

```
function divide(a, b) {
    assert b != 0, "Division by zero"
    return a / b
}

try {
    divide(10, 0)
} catch err {
    print err    // Division by zero
}
```

---

## Error Display

Zen shows detailed error information with source context:

```
error[undefined variable: foo]
 --> script.z:5:5
  |
5 | print foo
        ^
  |
  = error: undefined variable: foo
```

### Error format

```
error[<error type>: <message>]
 --> <file>:<line>:<col>
  |
<line> | <source code>
  |     ^
  |
  = <detailed message>
```

---

## Error Handling Patterns

### Guard clause

```
function process_file(path) {
    if !fs.exists(path) {
        throw "File not found: " + path
    }

    let content = fs.read(path)
    // ... process content
}
```

### Try with default

```
let config = try json.parse(raw_config) catch {
    print "Invalid config, using defaults"
    {"host": "localhost", "port": 8080}
}
```

### Retry pattern

```
function retry(fn, max_attempts) {
    let attempt = 0
    while attempt < max_attempts {
        attempt = attempt + 1
        try {
            return fn()
        } catch err {
            print "Attempt {attempt} failed: {err}"
            if attempt < max_attempts {
                sleep(1)
            }
        }
    }
    throw "Failed after {max_attempts} attempts"
}

let data = retry(function() {
    let resp = http.get("https://api.example.com/data")
    if !resp.ok { throw "HTTP " + str(resp.status) }
    return resp.json()
}, 3)
```

### Cleanup pattern

```
function process() {
    let temp = null
    try {
        temp = create_temp_file()
        do_work(temp)
    } catch err {
        print "Error: " + err
    } finally {
        if temp != null {
            fs.remove(temp)
        }
    }
}
```

### Typed error handling

```
errors.define("AppError", "Error", "application error")
errors.define("ConfigError", "AppError", "configuration error")
errors.define("DataError", "AppError", "data processing error")

try {
    load_config()
} catch ConfigError as err {
    print "Config problem: " + err
    // use defaults
} catch DataError as err {
    print "Data problem: " + err
    // skip bad data
} catch AppError as err {
    print "App error: " + err
} catch {
    print "Unexpected: " + error
}
```

---

## Best Practices

### 1. Be specific with catch types

```
// BAD — catches everything
try {
    risky_operation()
} catch {
    print "error"
}

// GOOD — catches specific errors
try {
    risky_operation()
} catch ValidationError as err {
    handle_validation(err)
} catch {
    handle_other(error)
}
```

### 2. Always clean up resources

```
// BAD — resource leak
let file = fs.open("data.txt")
process(file)

// GOOD — always clean up
let file = null
try {
    file = fs.open("data.txt")
    process(file)
} finally {
    if file != null { file.close() }
}
```

### 3. Don't swallow errors silently

```
// BAD — hides problems
try {
    risky_operation()
} catch {}

// GOOD — at least log it
try {
    risky_operation()
} catch err {
    print "Warning: " + err
}
```

### 4. Use guard clauses for preconditions

```
// BAD — nested if
function transfer(from, to, amount) {
    if from != null {
        if to != null {
            if amount > 0 {
                // actual logic
            }
        }
    }
}

// GOOD — guard clauses
function transfer(from, to, amount) {
    if from == null { throw "Source account required" }
    if to == null { throw "Destination account required" }
    if amount <= 0 { throw "Amount must be positive" }
    // actual logic (flat, clear)
}
```

### 5. Use try as expression for defaults

```
// GOOD — concise default
let config = try json.parse(raw) catch { {} }

// GOOD — explicit handling
let config = try json.parse(raw) catch err {
    print "Parse error: " + err
    {}
}
```

---

## Pro Tips

1. **Use `error` (built-in) when catch has no variable.** It's always available in catch blocks.
2. **`raise` is an alias for `throw`.** Use whichever reads better.
3. **Custom errors enable typed catch.** Define error hierarchies for structured error handling.
4. **`finally` always runs.** Even if the catch block throws.
5. **`try` as expression.** `let x = try risky() catch { default }` is clean and functional.
6. **Error objects carry context.** `{code: 404, message: "Not found"}` is more informative than strings.

---

## Common Mistakes

### Catching but not handling

```
// BAD — error is caught but nothing is done
try {
    risky_operation()
} catch {}

// This silently swallows the error
```

### Forgetting that finally runs even if catch throws

```
try {
    throw "error"
} catch {
    throw "another error"    // this also throws
} finally {
    print "runs anyway"     // this still runs
}
```

### Using try/catch for flow control

```
// BAD — using exceptions for normal logic
try {
    let val = list[10]
} catch {
    // handle missing index
}

// GOOD — check first
if list.len > 10 {
    let val = list[10]
}
```

---

## See Also

- [Control Flow](control-flow.md) — Try/catch/finally syntax
- [Classes](classes.md) — Custom error classes with inheritance
- [Variables](variables.md) — Error variable scoping
