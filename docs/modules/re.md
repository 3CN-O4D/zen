# re — Regular expressions

The `re` module provides powerful pattern matching and text manipulation using regular expressions. It is available globally as `re`.

> **IMPORTANT:** Because `match` is a reserved keyword in Zen, you cannot use dot-notation for `re.match`. Use bracket notation instead: `re["match"](pattern, string)`. Other functions like `re.search` work normally.

```zen
# 1. Simple search
var found = re.search("\\d+", "abc123def")
print(found)  # 123

# 2. Global find all
var all = re.findall("\\w+", "hello world")
print(all)    # [hello, world]

# 3. Splitting
var parts = re.split("[,;]", "a,b;c")
print(parts)  # [a, b, c]
```

## Functions

| Function | Signature | Returns | Description |
|----------|-----------|---------|-------------|
| `re["match"]` | `(pat, s)` | `bool` | Checks if the pattern matches at the start of the string. |
| `re.matches` | `(pat, s)` | `bool` | Alias for `re["match"]`. |
| `re.search` | `(pat, s)` | `string` | Returns the first match found anywhere, or `null`. |
| `re.findall` | `(pat, s)` | `list` | Returns a list of all matching strings. |
| `re.find` | `(pat, s)` | `list` | Returns first group matches. |
| `re.replace` | `(pat, s, rep)` | `string` | Replaces matches in `s` with `rep`. |
| `re.sub` | `(pat, s, rep)` | `string` | Alias for `re.replace`. |
| `re.split` | `(pat, s)` | `list` | Splits the string by the pattern. |

## Detailed Usage

### `re["match"](pattern, string)`
Returns `true` if the pattern matches from the **beginning** of the string.

```zen
print(re["match"]("a", "abc")) # true
print(re["match"]("b", "abc")) # false
```

### `re.search(pattern, string)`
Returns the first occurrence of the pattern as a string.

```zen
print(re.search("\\d+", "order #42nd")) # 42
```

### `re.findall(pattern, string)`
Returns a list of all non-overlapping matches.

```zen
var tags = re.findall("#\\w+", "love #zen and #code")
print(tags) # [#zen, #code]
```

### `re.replace(pattern, string, replacement)`
Replaces matches in the string. **Note:** Replacement is literal; backreferences (like `$1`) are currently not supported in the native replacement string.

```zen
var clean = re.replace("\\s+", "   spaced   ", " ")
print(clean) # " spaced "
```

## Syntax Note
Always use double backslashes for special characters in your patterns, as Zen strings process backslashes.

```zen
"\\d"    # Represents \d (digit)
"\\s"    # Represents \s (whitespace)
"\\w"    # Represents \w (word character)
```

## See Also
- [string](string.md) — For literal (non-regex) text operations.
