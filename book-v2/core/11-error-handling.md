# Chapter 11: Error Handling

Zen is a "safe by default" language: instead of crashing, runtime problems
become **catchable errors**. This chapter covers `try`/`catch`/`throw`/`finally`,
how to read the real failure out of a caught error, and the error types Zen
raises.

## 11.1 Catching errors: `try` / `catch`

Wrap risky code in `try`. If anything inside raises an error, execution jumps
to the matching `catch` block.

```zen
try {
    let x = 1 / 0        # raises DivideByZero
    print("never reached")
} catch {
    print("division failed")
}
# prints: division failed
```

A bare `catch` handles **any** error. If the `try` body completes normally,
`catch` is skipped:

```zen
var ran = false
try {
    print("ok")
    ran = true
} catch {
    ran = false
}
print(ran)   # → true
```

## 11.2 Reading the real failure: `catch as e`

Without a binding, you only know *that* something failed, not *what*. Bind the
error with `catch as e` to inspect it:

```zen
try {
    let y = 1 / 0
} catch as e {
    print(str(e))   # → DivideByZero: division by zero
}
```

`str(e)` renders the real message. You can also concatenate a string with the
error directly:

```zen
catch as e {
    print("caught: " + e)   # → caught: DivideByZero: division by zero
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
    print(e.msg)   # → IndexError: index 5 out of bounds for length 1
    print(e.code)  # → 0
    print(e.val)   # → nil
}
```

## 11.4 Catching specific error types

You can add typed clauses to handle only certain errors. Unmatched clauses fall
through to the next one, and a clause whose type doesn't match lets the error
keep propagating.

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

Zen's built-in error types include `TypeError`, `IndexError`, `KeyError`,
`DivideByZero`, `ValueError`, `NativeError`, `ForeignAttribute`, and
`UserError` (raised by `throw`).

## 11.5 Raising errors: `throw`

Use `throw` to raise your own errors. The value becomes `e.msg` (prefixed with
`UserError:` when it's a string).

```zen
func require_positive(n) {
    if n < 0 {
        throw "value must be positive, got " + str(n)
    }
}

try {
    require_positive(-3)
} catch as e {
    print(e.msg)   # → UserError: value must be positive, got -3
}
```

`throw` accepts any value, not just strings:

```zen
throw { code: 42, detail: "retry later" }
```

## 11.6 Cleanup: `finally`

A `finally` block runs whether the `try` succeeded or failed, and whether or
not `catch` handled the error. It must come **after** any `catch` clauses.

```zen
var log = ""
try {
    throw "boom"
} catch {
    log = log + "c"
} finally {
    log = log + "f"
}
print(log)   # → cf
```

```zen
# finally also runs when nothing failed
var log2 = ""
try {
    log2 = "b"
} finally {
    log2 = log2 + "f"
}
print(log2)   # → bf
```

## 11.7 Full example

```zen
func safe_divide(a, b) {
    try {
        return a / b
    } catch as e {
        print("safe_divide failed: " + e.msg)
        return nil
    } finally {
        print("safe_divide cleanup")
    }
}

print(safe_divide(10, 2))  # → 5  (then "safe_divide cleanup")
print(safe_divide(10, 0))  # → nil (after printing the error + cleanup)
```

## 11.8 Common pitfalls

- **`e` only exists inside the `catch` block.** Don't reference it outside.
- **`finally` goes after `catch`, not between `try` and `catch`.**
- **A bare `catch` swallows everything.** Use typed clauses when you want to
  let unexpected errors propagate.
- **`str(e)` is the readable message.** Never rely on `typeof(e)` (it's just
  `"error"`) — use `e.msg` instead.

## Quick reference

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
} finally {
    # always runs
}

throw "something went wrong"   # raise your own error
```
