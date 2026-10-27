# Appendix B: Error Codes Reference

Zen errors carry codes in the format `Z####` where the first digit indicates the error band:

- `Z01xx` — Lexer errors
- `Z02xx` — Parser errors
- `Z03xx` — Compile-time errors
- `Z04xx` — Runtime errors
- `Z05xx` — CLI/tool errors

Use `zen explain <code>` to see the full explanation for any code.

## B.1 Lexer errors (Z01xx)

| Code | Meaning |
|------|---------|
| `Z0101` | Invalid character in source |
| `Z0102` | Unterminated string literal |
| `Z0103` | Unterminated comment |
| `Z0104` | Invalid number literal |
| `Z0105` | Invalid escape sequence |

## B.2 Parser errors (Z02xx)

| Code | Meaning |
|------|---------|
| `Z0201` | Unexpected token / missing `)` |
| `Z0202` | Expected expression |
| `Z0203` | Invalid expression |
| `Z0204` | Expected `:` or `=>` (match arm) |
| `Z0205` | Duplicate key in dict literal |
| `Z0206` | Invalid pattern in match |
| `Z0207` | Unmatched delimiter |

## B.3 Compile-time errors (Z03xx)

| Code | Meaning |
|------|---------|
| `Z0301` | Undefined variable |
| `Z0302` | Duplicate definition |
| `Z0303` | Import not found |
| `Z0304` | Circular import |
| `Z0305` | Type mismatch in declaration |
| `Z0306` | Invalid `return` outside function |
| `Z0307` | `break`/`continue` outside loop |
| `Z0308` | Undefined function |
| `Z0309` | Invalid import path |
| `Z0310` | Duplicate import |
| `Z0324` | Module not found |
| `Z0325` | Cannot read file |

## B.4 Runtime errors (Z04xx)

| Code | Meaning |
|------|---------|
| `Z0401` | Type mismatch |
| `Z0402` | Index out of bounds |
| `Z0403` | Key not found |
| `Z0404` | Division by zero |
| `Z0405` | Value error |
| `Z0406` | Attribute error |
| `Z0407` | Native error |
| `Z0408` | Argument count |
| `Z0409` | Handle error |
| `Z0410` | I/O mismatch |
| `Z0411` | Native module error (general) |
| `Z0412` | Handle error |
| `Z0413` | PyImport error |
| `Z0414` | Python import failed |

## B.5 CLI/tool errors (Z05xx)

| Code | Meaning |
|------|---------|
| `Z0501` | Usage error |
| `Z0502` | Unknown command |
| `Z0503` | Build error |
| `Z0504` | Lint error |

## Using `zen explain`

```bash
zen explain Z0401
zen explain Z0301
zen explain Z0501
zen explain --list   # list all codes
```

Each explanation includes:
- The error title
- A detailed description
- A `= help:` suggestion with a code example

## Looking up an error

When a script fails, Zen prints:

```
zen: error: TypeError: expected number, got string[Z0401]
  --> script.z:5:5
   |
5 | print("caught: " + e)
   |     ^^^^^^^^^^^^^^^^^
   |
   = help: check operand types with `typeof(x)` and convert explicitly
   = note: run `zen explain Z0401` for detail
```

Run `zen explain Z0401` for the full explanation.

## Common runtime errors and fixes

### Z0401 — Type mismatch

```zen
# Wrong
print(42 + "hello")

# Right
print(str(42) + "hello")
```

### Z0402 — Index out of bounds

```zen
# Wrong
let a = [1, 2, 3]
print(a[5])

# Right
let a = [1, 2, 3]
if 5 < len(a) {
    print(a[5])
}
```

### Z0403 — Key not found

```zen
# Wrong
let d = {"a": 1}
print(d["b"])

# Right
let d = {"a": 1}
print(d.get("b", 0))
```

### Z0404 — Division by zero

```zen
# Wrong
let x = 1 / 0

# Right
if b != 0 {
    let x = a / b
}
```

## Exercises

1. Write a script that intentionally triggers each error type and catches it.
2. Use `zen explain` on three different error codes and summarize them.
3. Write `safe_div(a, b)` that returns `nil` on division by zero.
4. Write `safe_index(lst, i, default)` that returns `default` on index error.

## B.12 Error code reference Z01xx – Z05xx (exhaustive)

Zen's coded diagnostics use the prefix `Z` followed by three digits. Use
`zen explain Z####` to see the full explanation, common causes, and a fix.

### Z01xx — Lexer / lexical errors

| Code | Name | Meaning | Common cause | Fix |
|------|------|---------|--------------|-----|
| Z0101 | InvalidCharacter | A character not allowed in source | stray control char, smart quotes | remove/replace the character |
| Z0102 | UnterminatedString | String literal never closed | missing closing quote | add closing quote |
| Z0103 | InvalidNumber | Malformed numeric literal | `1.2.3`, `0x`, trailing dot | use valid literal form |
| Z0104 | InvalidEscape | Bad escape in string | `\q` instead of `\n` | use a valid escape |
| Z0105 | InvalidIdentifier | Identifier starts with digit or bad char | `1abc`, `foo-bar` | rename identifier |
| Z0106 | NumericLiteralOverflow | Integer literal beyond i48 range | `99999999999999999999` | use a smaller literal or big-int at runtime |
| Z0107 | MultiDotRejected | `1.2.3` style version number | `1.2.3` | not a number; use string |
| Z0108 | BadComment | malformed block comment | unclosed `/* */` | close the block comment |

### Z02xx — Parser / syntax errors

| Code | Name | Meaning | Common cause | Fix |
|------|------|---------|--------------|-----|
| Z0201 | UnexpectedToken | token where parser didn't expect it | missing operator, extra keyword | correct syntax |
| Z0202 | MissingSemicolon | statement separator required | newline after non-continuable token | add `;` or restructure |
| Z0203 | UnbalancedBraces | `{`/`}` or `[`/`]` or `(`/`)` mismatch | missing closing brace | add the closing token |
| Z0204 | UnexpectedEOF | source ended mid-construct | unclosed block, string, or call | complete the construct |
| Z0205 | InvalidStatement | statement not allowed here | `return` outside function | move into a function |
| Z0206 | DuplicateParameter | same parameter name twice | `func f(a, a) {}` | rename parameters |
| Z0207 | InvalidDefault | default value not allowed | `func f(a = 1 + 2) {}` (complex default) | use a literal/simple default |
| Z0208 | InvalidMatchArm | bad match arm syntax | missing `=>`/`->`/`:` | add arm separator |
| Z0209 | InvalidImport | malformed import statement | `import` with no module | write valid import |
| Z0210 | Redefinition | redefining `let`/const | `let x = 1; let x = 2` | use `var` or rename |

### Z03xx — Runtime errors

| Code | Name | Meaning | Common cause | Fix |
|------|------|---------|--------------|-----|
| Z0301 | TypeError | operation on wrong type | `"a" + 1`, `nil.x` | check types before operating |
| Z0302 | IndexError | index out of range | `xs[10]` on 3-element list | check `len` first |
| Z0303 | KeyError | missing dict key | `d["missing"]` | use `in` or `??` |
| Z0304 | DivideByZero | division by zero | `x / 0` | check divisor != 0 |
| Z0305 | ValueError | bad value for operation | `int("abc")`, `sqrt(-1)` | validate input |
| Z0306 | AttributeError | object has no attribute | `obj.missing` | define the attribute/method |
| Z0307 | NotCallable | calling a non-function | `x = 5; x()` | call a function value |
| Z0308 | ArgumentCount | wrong number of arguments | `f(1, 2)` when `f` takes 1 | match arity |
| Z0309 | UnboundVariable | variable used before assignment | `print(y)` when `y` unset | initialize first |
| Z0310 | MaxRecursionDepth | recursion too deep | infinite recursion | convert to iteration or raise limit |
| Z0311 | OverflowError | arithmetic overflow | huge int/float result | use bounded math |
| Z0312 | DomainError | math domain error | `log(0)`, `sqrt(-1)` | check domain |
| Z0313 | RangeError | value out of allowed range | index negative on some ops | clamp value |
| Z0314 | EncodingError | can't encode/decode | bad UTF-8 bytes | use valid encoding / errors param |
| Z0315 | ImmutableError | mutating immutable value | `s[0] = "X"` on string | build a new value |
| Z0316 | ConcurrentModification | collection changed during iteration | removing while looping | iterate over a copy |
| Z0317 | StackOverflow | native stack exhausted | deep recursion | iterative approach |

### Z04xx — I/O / system errors

| Code | Name | Meaning | Common cause | Fix |
|------|------|---------|--------------|-----|
| Z0401 | FileNotFound | file does not exist | wrong path | check path, create file |
| Z0402 | PermissionDenied | insufficient rights | read-only file, root needed | fix permissions |
| Z0403 | FileExists | file already exists (create-only) | `os.mkdir` on existing dir | use `exist_ok=true` |
| Z0404 | IsADirectory | expected file, got directory | opening a dir as file | use a file path |
| Z0405 | NotADirectory | expected directory, got file | `os.listdir("file.txt")` | use a directory |
| Z0406 | DirectoryNotEmpty | can't remove non-empty dir | `os.rmdir` on populated dir | remove contents first |
| Z0407 | BrokenPipe | write to closed pipe/socket | peer closed connection | handle disconnection |
| Z0408 | ConnectionRefused | nothing listening on port | server not started | start server / check port |
| Z0409 | ConnectionReset | connection reset by peer | peer aborted | retry / handle error |
| Z0410 | Timeout | operation timed out | network delay, no response | increase timeout / retry |
| Z0411 | NetworkUnreachable | network not available | offline, wrong interface | check connectivity |
| Z0412 | HostUnreachable | host not reachable | wrong IP, host down | check host |
| Z0413 | AddressInUse | bind to occupied port | two servers same port | use SO_REUSEADDR / different port |
| Z0414 | DiskFull | no space left | filesystem full | free space |
| Z0415 | TooManyOpenFiles | file descriptor limit | leaking file handles | close files with `with` |
| Z0416 | InvalidArgument | invalid syscall argument | bad flag, bad path | check argument |
| Z0417 | SignalInterrupted | syscall interrupted by signal | EINTR | retry the operation |

### Z05xx — FFI / interoperability errors

| Code | Name | Meaning | Common cause | Fix |
|------|------|---------|--------------|-----|
| Z0501 | SymbolNotFound | C function symbol missing | wrong library name, not exported | check library, export symbol |
| Z0502 | HandleInvalid | invalid FFI handle | handle closed, never opened | track handle lifetime |
| Z0503 | CallingConvention | wrong calling convention | cdecl vs stdcall mismatch | match platform convention |
| Z0504 | TypeMismatch | FFI argument type wrong | passing int where pointer expected | fix argument types |
| Z0505 | MemoryAccess | invalid memory read/write | null pointer, freed memory | check pointers |
| Z0506 | PymportNotFound | Python module not found | module not installed, wrong path | install module, set PYTHONPATH |
| Z0507 | PymportMarshal | can't marshal value to Python | unsupported Zen type | convert to supported type |
| Z0508 | PythonError | Python raised an exception | error inside Python call | inspect chained traceback |
| Z0509 | GILContention | pymport GIL issue | deadlock with Python lock | avoid nested GIL acquisition |
| Z0510 | ForeignTypeUnsupported | foreign type not supported | struct/union not mapped | use bytes/opaque handle |

## B.13 How to read a Zen error

A Zen error message looks like:

```
Z0301: TypeError: cannot add string and int
  at main.z:42:10
```

Parts:
- `Z0301` — the error code; run `zen explain Z0301`.
- `TypeError` — the error category/name.
- `cannot add string and int` — human-readable message.
- `main.z:42:10` — file, line, column where it occurred.

Use `zen explain` for deeper context:

```bash
zen explain Z0301
zen explain Z0408
```

## B.14 Error handling patterns

### Catch and rethrow with context

```zen
try {
    load_config(path)
} catch as e {
    throw "load_config failed: " + e.msg
}
```

### Catch specific types first

```zen
try {
    risky()
} catch [FileNotFound] as e {
    print("missing file")
} catch as e {
    print("other error: " + e.msg)
}
```

### Result-style (no exceptions)

```zen
fn result_style() -> Dict {
    try {
        let v = fallible()
        return #{ok: true, value: v}
    } catch as e {
        return #{ok: false, error: e.msg}
    }
}
```

### Finally for cleanup

```zen
let f = open(path)
try {
    return f.read()
} finally {
    f.close()
}
```

## B.15 Exercises

1. Run `zen explain Z0301`, `Z0408`, `Z0506` and record the explanations.
2. Write a function that deliberately triggers each Z03xx error and catches it.
3. Build a table mapping error code → suggested fix for your codebase.
4. Demonstrate `finally` running after `return`.
5. Implement a `retry` that catches Z0410 (Timeout) and retries.
6. Write a safe wrapper around `int()` returning nil on Z0305.
7. Show what happens when you access a missing dict key (Z0303).
8. Demonstrate division by zero (Z0304) and guard against it.
9. Write a program that handles FileNotFound (Z0401) gracefully.
10. Create an error lookup tool: input a code, print name + meaning.

