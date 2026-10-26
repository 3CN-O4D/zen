# Appendix B: Error Codes Reference

Zen errors carry codes in the format `Z####` where the first digit indicates
the error band:

- `Z01xx` — Lexer errors
- `Z02xx` — Parser errors
- `Z03xx` — Compile-time errors
- `Z04xx` — Runtime errors
- `Z05xx` — CLI/tool errors

Use `zen explain <code>` to see the full explanation for any code.

## B.1 Lexer errors (Z01xx)

| Code | Meaning |
|------|---------|
| `Z0101` | Invalid character |
| `Z0102` | Unterminated string |
| `Z0103` | Unterminated comment |
| `Z0104` | Invalid number |
| `Z0105` | Invalid escape sequence |

## B.2 Parser errors (Z02xx)

| Code | Meaning |
|------|---------|
| `Z0201` | Unexpected token / missing `)` |
| `Z0202` | Expected expression |
| `Z0203` | Invalid expression |
| `Z0204` | Expected `:` or `=>` |
| `Z0205` | Duplicate key in dict |
| `Z0206` | Invalid pattern |
| `Z0207` | Unmatched delimiter |

## B.3 Compile-time errors (Z03xx)

| Code | Meaning |
|------|---------|
| `Z0301` | Undefined variable |
| `Z0302` | Duplicate definition |
| `Z0303` | Import not found |
| `Z0304` | Circular import |
| `Z0305` | Type mismatch in declaration |
| `Z0306` | Invalid return outside function |
| `Z0307` | Break/continue outside loop |
| `Z0308` | Undefined function |
| `Z0309` | Invalid import path |
| `Z0310` | Duplicate import |
| `Z0325` | Cannot read file |
| `Z0324` | Module not found |

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
| `Z0411` | Native module error |
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
