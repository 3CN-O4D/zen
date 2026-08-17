# Errors Module (`errors`)

Error handling with custom error definitions.

Built-in error classes: `Error`, `TypeError`, `ValueError`, `IndexError`, `KeyError`, `FileNotFoundError`, `ZeroDivisionError`, `ArithmeticError`, `RuntimeError`, `NotImplementedError`, `StopIteration`, `AssertionError`, `ImportError`, `RecursionError`, `OSError`, `SystemExit`.

Custom errors:
```zen
errors.define("MyError", "Error", "default message")
throw new MyError("custom msg")
```

Catch syntax:
```zen
try { ... } catch TypeError as e { ... } catch as e { ... }
```

Subclassing:
```zen
class CustomError extends errors.Error { }
```
