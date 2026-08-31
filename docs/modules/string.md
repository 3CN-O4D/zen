# string — String helpers

The `string` module contains useful constants and functions for text manipulation. It is available globally as `string`.

> **Note:** Many of these functions are also available as methods directly on string values (e.g., `"abc".upper()`). Use the `string` module for constants or when you want a functional style.

```zen
# 1. Constants
print(string.digits)            # 0123456789
print(string.ascii_lowercase)   # abcdefghijklmnopqrstuvwxyz

# 2. Case manipulation
print(string.upper("hello"))    # HELLO
print(string.capitalize("zen")) # Zen
```

## Constants

| Constant | Description |
|----------|-------------|
| `digits` | `0123456789` |
| `hexdigits` | `0123456789abcdefABCDEF` |
| `octdigits` | `01234567` |
| `ascii_lowercase` | `abcdefghijklmnopqrstuvwxyz` |
| `ascii_uppercase` | `ABCDEFGHIJKLMNOPQRSTUVWXYZ` |
| `ascii_letters` | Lowercase + Uppercase letters. |
| `punctuation` | Standard punctuation characters. |
| `whitespace` | Space, tab, newline, etc. |
| `printable` | Combination of letters, digits, punctuation, and whitespace. |

## Functions

| Function | Description |
|----------|-------------|
| `upper(s)` / `lower(s)` | Change case. |
| `capitalize(s)` | Capitalize first character. |
| `title(s)` | Capitalize first character of every word. |
| `swapcase(s)` | Swap uppercase and lowercase. |
| `strip(s)` / `lstrip(s)` / `rstrip(s)` | Remove whitespace. |
| `split(s, sep)` | Split string into a list. |
| `join(list, sep)` | Join a list of strings. |
| `replace(s, old, new)` | Replace occurrences. |
| `count(s, sub)` | Count occurrences of a substring. |
| `find(s, sub)` | Find first index of a substring. |
| `startswith(s, pre)` / `endswith(s, suf)` | Check prefix/suffix. |
| `isdigit(s)` / `isalpha(s)` / `isalnum(s)` | Content checks. |

## Examples

### Checking if a string contains only letters
```zen
if string.isalpha("Hello") {
    print("Letters only")
}
```

### Joining a list
```zen
var parts = ["a", "b", "c"]
print(string.join(parts, "-")) # a-b-c
```

## See Also
- [strings](../strings.md) — Core string documentation.
- [re](re.md) — For regex-based text processing.
