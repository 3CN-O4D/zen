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
