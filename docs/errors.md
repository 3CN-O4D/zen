# Error Handling

## Try / Catch

```zen
try {
    risky_operation()
} catch ValueError as e {
    print "caught: " + e
} catch as e {
    print "fallback"
} finally {
    cleanup()
}
```

## Throwing Errors

```zen
throw new errors.ValueError("bad input")
raise new errors.TypeError("wrong type")
```

## Custom Errors

```zen
errors.define("AuthError", "Error", "auth failed")
throw new AuthError("bad token")
```

## Built-in Error Types
`Error`, `TypeError`, `ValueError`, `IndexError`, `KeyError`,
`FileNotFoundError`, `ZeroDivisionError`, `RuntimeError`,
`NotImplementedError`, `AssertionError`, `ImportError`, `OSError`
