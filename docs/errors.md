# Errors

Zen's error system is built on `try`/`catch`/`finally` plus a runtime
exception model that treats any thrown value as an error. Typed `catch` blocks
and the `errors` module give you structured, Python-flavored error handling.

## Throwing

`throw` raises any value — a string, a dict, or an error object:

```zen
throw "boom"
throw { type: "ValueError", message: "bad input" }
```

When you throw a **dict**, Zen uses its `type` key to decide which typed
`catch` clause matches (defaulting to `Error` when absent), and binds the
`message` key into a catch variable:

```zen
try {
    throw { type: "KeyError", message: "missing" }
} catch KeyError as e {
    print("KeyError:", e)      # KeyError: missing
} catch as e {
    print("other:", e)
}
```

## try / catch / finally

```zen
try {
    risky_thing()
} catch as e {
    print("caught:", e)        # catch-all binding form
} finally {
    cleanup()
}
```

The parser accepts several `catch` shapes:

| Form | Meaning |
|------|---------|
| `catch as e { }` | catch **any** error, bind value in `e` |
| `catch (e) { }` | catch any error, bind value in `e` |
| `catch e { }` | same — bare name binding |
| `catch TypeError as e { }` | catch only a typed error |
| `catch TypeError { }` | catch a typed error, no binding |
| `catch { }` | catch any, ignore the value |

`finally` runs whether or not an error was thrown:

```zen
try {
    throw "x"
} finally {
    print("always")            # runs even though we rethrew implicitly
}
```

> **Bare `catch(e)` with parentheses is NOT typed catch** — a parenthesized
> name is a *binding*. Typed catch requires the type name followed by the
> binding: `catch TypeError as e`.

## The `errors` module

`errors` is a dict (import it like any module) whose keys are the built-in
error type names plus `define`:

```zen
import errors
print(errors.keys())
# [OverflowError, RecursionError, MathError, RangeError, KeyError,
#  ValueError, NameError, IOError, SystemExit, RuntimeError,
#  KeyboardInterrupt, ImportError, Error, AssertionError, define, ...]
```

Built-in type names:

```
Error            TypeError        ValueError       RangeError
NameError        LookupError      KeyError         IndexError
ArithmeticError  MathError        NumberError      ZeroDivisionError
OverflowError    IOError          FileNotFoundError ImportError
KeyboardInterrupt RuntimeError    NotImplementedError StopIteration
RecursionError   AssertionError   SystemExit
```

## Typed catch & custom errors

A typed `catch` matches a thrown value whose effective type equals the named
class (or inherits it). To get a typed error with a readable payload, **throw a
dict** with a `type` key equal to the error class name and a `message` key:

```zen
import errors

errors.define("DBError")
class DBError extends errors.Error {}

try {
    throw { type: "DBError", message: "connection refused" }
} catch DBError as e {
    print("typed:", e)                # typed: connection refused
} catch as e {
    print("caught", e)                # fallback
}
```

Matching is done on the thrown value's type name (the `type` key of a dict, or
the class name of an instance). `errors.define("Name")` registers a bare name
so an `extends errors.Error` class can be used as a `catch` type.

> **Tip:** When a typed `catch` binds a dict error, the bound variable holds
> the `message` string directly, not the whole dict. To keep rich fields,
> inspect the dict before throwing (store it in a variable and read `type` /
> `message` yourself).

## The default (uncaught) behavior

Uncaught errors stop the program with a rustc-style diagnostic: message,
`--> file:line:col`, source snippet with a caret, and `= help:` / `= note:`
suggestions:

```
$ zen bad.z
error: dictionary has no member: `b`
 --> bad.z:2:0
  |
2 | print d.b
  | ^~~~~~~~~
  |
  = note:  list indices start at 0, not 1
```

Caught errors (`catch as e`) always see the plain message string — the
rendering above only happens at the top-level boundary.

## Modules, dicts, functions, objects

Zen differentiates its data types in error messages:

| Mistake | Message |
|---------|---------|
| `sqlite.open_memory()` (typo for `openMemory`) | ``module `sqlite` has no function: `open_memory` `` + did-you-mean |
| `data.nope()` on a user dict | ``dictionary has no method: `nope` `` |
| `data.nope` on a user dict | ``dictionary has no member: `nope` `` |
| `f()` where `f` holds `5` | ``cannot call `f`: value of type `int` is not a function `` |
| `nosuchfn()` | ``undefined function: `nosuchfn` `` |

`typeof` reports the vocabulary directly: `"function"` for functions,
`"object"` for class instances, `"dict"` for dicts. Modules are dicts whose
functions are native or Zen-defined — the error layer just knows their names,
so a miss on one says `module`, never `dictionary`.

`exit(code)` terminates with a status code; `SystemExit` (thrown) has the same
effect.

## Assertions

`assert(condition, message?)` fails with an error when the condition is
falsy:

```zen
assert(2 + 2 == 4, "math is broken")
```

## Common pitfalls

| Mistake | Reality |
|---------|---------|
| `try ... catch (e) { }` expecting a *type* | the parentheses bind the value — use `catch TypeError as e` for typed |
| `throw errors.ValueError("x")` | `errors.ValueError` is a name string, **not** callable — throw a dict or class instance instead |
| expecting the error message in `e` | binding depends on form: dicts bind `message`; others bind the raw value |
| `except` keyword | Zen uses `catch` (there is no `except`) |