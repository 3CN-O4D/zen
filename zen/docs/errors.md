# Error Handling in Zen

Built-in error types live in the `errors` module: `Error`, `ValueError`, `TypeError`, `IndexError`, `KeyError`, `FileNotFoundError`, `ZeroDivisionError`, `ArithmeticError`, `RuntimeError`, `NotImplementedError`, `StopIteration`, `AssertionError`, `ImportError`, `RecursionError`, `OSError`, `SystemExit`.

Custom errors via subclassing:
```zen
class MyError extends errors.Error {}
```

Or via `errors.define()`:
```zen
errors.define("MyError", "Error", "message")
```

Throwing and catching:
```zen
throw new errors.ValueError("bad input")
try { ... } catch ValueError as e { ... } catch as e { ... }
```
