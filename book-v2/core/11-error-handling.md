# Chapter 11: Error Handling

Zen is "safe by default": runtime problems become catchable errors instead of crashes. This chapter covers `try`/`catch`/`throw`/`finally` exhaustively, error fields, typed catch clauses, error types, and the interaction between `exit()` and `finally`.

## 11.1 Catching errors: `try` / `catch`

Wrap risky code in `try`. If anything inside raises an error, execution jumps to the matching `catch` block.

```zen
try {
    let x = 1 / 0        # raises DivideByZero
    print("never reached")
} catch {
    print("division failed")
}
# prints: division failed
```

### Bare `catch`

A bare `catch` handles **any** error. If the `try` body completes normally, `catch` is skipped:

```zen
var caught = false
try {
    var bad = 1 / 0
    caught = bad == 0
} catch {
    caught = true
}
print(caught)  # true

var ok = true
try {
    var five = 10 / 2
    ok = five == 5
} catch {
    ok = false
}
print(ok)  # true
```

## 11.2 Reading the real failure: `catch as e`

Without a binding, you only know *that* something failed, not *what*. Bind the error with `catch as e` to inspect it:

```zen
try {
    let y = 1 / 0
} catch as e {
    print(str(e))   # DivideByZero: division by zero
}
```

`str(e)` renders the real message. You can also concatenate a string with the error directly:

```zen
try {
    let y = 1 / 0
} catch as e {
    print("caught: " + e)   # caught: DivideByZero: division by zero
}
```

## 11.3 Error fields: `e.msg`, `e.code`, `e.val`

A caught error is an object with three readable fields:

| Field | Meaning | Example |
|-------|---------|---------|
| `e.msg` | Human-readable message | `"DivideByZero: division by zero"` |
| `e.code` | Numeric error code | `0` |
| `e.val` | Extra attached value (often `nil`) | `nil` |

```zen
try {
    let a = [1][5]   # IndexError
} catch as e {
    print(e.msg)   # IndexError: index 5 out of bounds for length 1
    print(e.code)  # 0
    print(e.val)   # nil
}
```

## 11.4 String + error concatenation

```zen
try {
    let y = 1 / 0
} catch as e {
    print("caught: " + e)   # caught: DivideByZero: division by zero
}
```

## 11.5 Catching specific error types

You can add typed clauses to handle only certain errors. Unmatched clauses fall through to the next one:

```zen
try {
    risky()
} catch TypeError {
    print("bad type")
} catch UserError {
    print("user raised")
} catch {
    print("everything else")
}
```

A clause whose type does not match lets the error keep propagating outward:

```zen
try {
    throw "z"
} catch TypeError {   # doesn't match UserError
    print("t")        # skipped
} catch UserError {
    print("u")        # catches it
}
print("u")  # u
```

## 11.6 Raising errors: `throw`

Use `throw` to raise your own errors. Strings become `UserError` messages:

```zen
var msg = ""
try {
    throw "custom failure"
} catch {
    msg = "handled"
}
print(msg)  # handled

var tm = ""
try {
    throw "custom boom"
} catch as e {
    tm = e.msg
}
print(tm)  # UserError: custom boom
```

`throw` accepts any value:

```zen
throw { code: 42, detail: "retry later" }
```

The value is stringified into `e.msg`; `e.val` is `nil` for `throw`:
```zen
try {
    throw { code: 42 }
} catch as e {
    print(e.msg)   # UserError: { code: 42, detail: ... }
    print(e.val)   # nil
}
```

## 11.7 Cleanup: `finally`

A `finally` block runs whether the `try` succeeded or failed, and whether or not `catch` handled the error. It must come **after** any `catch` clauses.

```zen
var log = ""
try {
    throw "boom"
} catch {
    log = log + "c"
} finally {
    log = log + "f"
}
print(log)  # cf
```

```zen
# finally also runs when nothing failed
var log2 = ""
try {
    log2 = "b"
} finally {
    log2 = log2 + "f"
}
print(log2)  # bf
```

## 11.8 `exit()` and `finally`

`exit(code)` terminates the process immediately with the given code. Because `exit` terminates the process, `finally` blocks do **not** run after `exit()`:

```zen
try {
    exit(3)
} finally {
    print("this will NOT print")   # process already exited
}
```

```zen
# exit inside catch does not run finally
try {
    throw "boom"
} catch {
    exit(1)
} finally {
    print("this will NOT print")   # process exited in catch
}
```

If you need cleanup before exiting, do it before calling `exit()`:

```zen
try {
    risky()
} catch as e {
    cleanup_resources()   # explicit cleanup before exit
    exit(1)
}
```

## 11.9 Nested handlers

Inner handlers run before outer ones:

```zen
var order = ""
try {
    try { throw "inner" } catch { order = order + "i" }
    throw "outer"
} catch { order = order + "o" }
print(order)  # io
```

## 11.10 Errors from callees

Errors raised inside a function are catchable by the caller:

```zen
func deep() {
    return [1][5]
}
var hit9 = false
try {
    deep()
} catch IndexError {
    hit9 = true
}
print(hit9)  # true
```

## 11.11 Error types

Zen's built-in error types:

| Type | Meaning | Typical cause |
|------|---------|---------------|
| `TypeError` | Wrong type for an operation | `"a" + 1`, `nil.field` |
| `IndexError` | Sequence index out of bounds | `[1][5]`, `"ab"[5]` |
| `KeyError` | Missing dict key | `d["missing"]` |
| `DivideByZero` | Division by zero | `1 / 0`, `1 % 0` |
| `ValueError` | Inappropriate value | `int("not a number")` |
| `NativeError` | Raised by a native module | fs, socket, etc. |
| `ForeignAttribute` | Failed foreign attribute access | `py_obj.bad_attr` |
| `PyImportError` | Python import failure | `pymport nonexistent` |
| `UserError` | Raised by `throw` | `throw "msg"` |

## 11.12 Full error-code reference

Every error carries a code `Z####`. See Appendix B for the complete list. Common runtime codes:

| Code | Type | Meaning |
|------|------|---------|
| `Z0401` | TypeError | Type mismatch |
| `Z0402` | IndexError | Index out of bounds |
| `Z0403` | KeyError | Key not found |
| `Z0404` | DivideByZero | Division by zero |
| `Z0405` | ValueError | Value error |
| `Z0406` | Attribute error | Attribute access |
| `Z0407` | NativeError | Native module error |
| `Z0408` | ArgumentCount | Wrong arg count |
| `Z0409` | HandleError | Handle error |
| `Z0410` | IOMismatch | I/O mismatch |
| `Z0411` | NativeError | Native module error (general) |
| `Z0414` | PyImportError | Python import failed |

Use `zen explain Z0401` to see the full explanation for any code.

## 11.13 Common pitfalls

- **`e` only exists inside its `catch` block.** Don't reference it outside.
- **`finally` goes after `catch`, not between `try` and `catch`.**
- **A bare `catch` swallows everything.** Use typed clauses when you want unexpected errors to propagate.
- **`str(e)` is the readable message.** Never rely on `typeof(e)` (it's just `"error"`) — use `e.msg` instead.
- **`exit()` skips `finally`**. Do cleanup before calling `exit()`.
- **`throw` with a string** creates `UserError: <message>`; `e.val` is `nil`.
- **`throw` with an object** stringifies it into `e.msg`; `e.val` is `nil`.

## 11.14 Quick reference

```zen
try {
    # risky code
} catch as e {
    print(str(e))   # real message
    print(e.msg)    # same as str(e)
    print(e.code)   # numeric code
    print(e.val)    # attached value
} catch TypeError {
    # only type errors
} catch UserError {
    # only user-thrown errors
} catch {
    # everything else
} finally {
    # always runs (unless exit() was called)
}

throw "something went wrong"   # raise your own error
throw { code: 42 }             # throw any value

try {
    exit(1)   # process exits; finally does NOT run
}
```

## 11.15 Exercises

1. Write `safe_divide(a, b)` that returns `nil` on division by zero instead of crashing.
2. Use `try`/`catch` to read a file that might not exist, returning a default value.
3. Write a function that uses `try`/`catch`/`finally` to guarantee a cleanup message prints regardless of success or failure.
4. Demonstrate that `exit()` skips `finally`.
5. Write `parse_int_or_default(s, default)` that uses `try`/`catch` around `int(s)`.
6. Create a custom error type by throwing an object and inspecting `e.msg`.
7. Write a nested try/catch where the inner catch handles one error type and the outer catch handles another.

## 11.12 Error handling — complete reference

Zen uses `try`/`catch`/`finally` and `throw`. Errors carry coded diagnostics
`Z01xx`–`Z05xx` and `zen explain` shows details.

### Syntax

```zen
try {
    # code that may fail
} catch [Type] as name {
    # handle specific error type
} catch as name {
    # handle any error
} finally {
    # always runs (cleanup)
}
```

Multiple `catch` clauses are checked in order; the first matching type wins.

### Throwing

```zen
throw "something went wrong"
throw Error("detail", code: "Z0308")   # if structured errors supported
throw ValueError("bad input")
```

Catching:

```zen
try {
    int("not a number")
} catch as e {
    print(e.msg)      # error message
    print(e.code)     # e.g. Z0308, if available
}
```

### Error codes Z01xx–Z05xx

| Range | Category | Examples |
|-------|----------|----------|
| Z01xx | Lexer / syntax | invalid token, bad number literal |
| Z02xx | Parser | unexpected token, unbalanced braces |
| Z03xx | Runtime | type mismatch, division by zero, index out of range |
| Z04xx | I/O / system | file not found, network, permission |
| Z05xx | FFI / interop | symbol not found, pymport failure, handle error |

Use `zen explain Z0308` to see the full explanation and suggested fix.

```bash
zen explain Z0308
```

### `finally` semantics

`finally` runs whether the `try` block completes normally, throws, returns,
breaks, or continues. It is the place for resource cleanup.

```zen
func read_file(path) {
    let f = open(path)
    try {
        return f.read()
    } catch as e {
        print("read failed: " + e.msg)
        return nil
    } finally {
        f.close()
    }
}
```

Even `return` inside `try` still runs `finally` before returning.

### Best practices

1. **Catch specific errors first**, broad `catch as e` last.
2. **Don't swallow errors silently** — log or rethrow.
3. **Use `finally` for cleanup** (close files, sockets, locks).
4. **Throw early, catch late** — validate at boundaries, handle at policy level.
5. **Don't use exceptions for control flow** in hot loops.
6. **Include context** when rethrowing: `throw "load_config failed: " + e.msg`.
7. **Document which errors a function throws.**

### Result-style pattern (no exceptions)

For fallible operations, return `nil` or a `#{ok, value}` / `#{ok, error}` dict:

```zen
func safe_div(a, b) {
    if b == 0 { return #{ok: false, error: "division by zero"} }
    return #{ok: true, value: a / b}
}

let result = safe_div(10, 0)
if !result["ok"] { print(result["error"]) }
```

### Error chaining (pymport)

Python errors raised under `pymport` are chained under Zen frames, preserving
the original traceback. See Chapter 42.

## 11.13 Common runtime errors

| Error | Cause | Fix |
|-------|-------|-----|
| Z0301 TypeError | wrong type for operation | check types before operating |
| Z0302 IndexError | index out of range | check `len` first |
| Z0303 KeyError | missing dict key | use `in` or `??` |
| Z0304 DivideByZero | dividing by zero | check divisor != 0 |
| Z0305 ValueError | bad value (e.g. `int("x")`) | validate input |
| Z0401 FileNotFound | file doesn't exist | check before open |
| Z0402 PermissionDenied | insufficient rights | fix permissions |

## 11.14 Defensive programming

Validate inputs at function boundaries:

```zen
func process_user(user) {
    if typeof(user) != "dict" { throw "user must be a dict" }
    if !("name" in user) { throw "user missing 'name'" }
    if typeof(user["name"]) != "string" { throw "name must be string" }
    # ... proceed safely
}
```

Use assertions for internal invariants:

```zen
assert len(xs) > 0, "xs must not be empty"
```

## 11.15 Debugging with `zen explain`

```bash
zen explain Z0308
zen explain Z0401
```

Each code prints: meaning, common causes, and a suggested fix.

## 11.16 Exercises

1. Write `safe_div(a, b)` returning `nil` on division by zero.
2. Write `safe_int(s)` returning `nil` on parse failure.
3. Implement `read_first_line(path)` handling FileNotFound and empty file.
4. Build a `validate_age(age)` throwing if age < 0 or > 150.
5. Demonstrate `finally` running after `return`.
6. Write `retry(fn, n)` catching errors and retrying up to `n` times.
7. Implement `partition(xs, pred)` without exceptions.
8. Create a function that throws different error types based on input.
9. Show `zen explain` output for Z0301 and Z0401.
10. Write a batch processor that logs errors per item but continues.

