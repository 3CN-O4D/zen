# Chapter 17: string Module

The `string` module provides string utilities: case conversion, trimming,
splitting, joining, padding, alignment, and character classification.

## 17.1 Case conversion

| Function | Description | Example |
|----------|-------------|---------|
| `upper(s)` | Uppercase | `string.upper("zen")` → `"ZEN"` |
| `lower(s)` | Lowercase | `string.lower("ZEN")` → `"zen"` |
| `title(s)` | Title case | `string.title("zen lang")` → `"Zen Lang"` |
| `capitalize(s)` | Capitalize first | `string.capitalize("zen")` → `"Zen"` |
| `swapcase(s)` | Invert case | `string.swapcase("ZEN")` → `"zen"` |

```zen
import string

print(string.upper("hello"))      # HELLO
print(string.lower("WORLD"))      # world
print(string.title("hello world"))# Hello World
print(string.capitalize("hello")) # Hello
```

## 17.2 Trimming

| Function | Description | Example |
|----------|-------------|---------|
| `trim(s)` | Whitespace both ends | `string.trim("  x  ")` → `"x"` |
| `lstrip(s)` | Whitespace left | `string.lstrip("  x")` → `"x"` |
| `rstrip(s)` | Whitespace right | `string.rstrip("x  ")` → `"x"` |

```zen
import string

print(string.trim("  hello  "))   # hello
print(string.lstrip("  hello"))   # hello
print(string.rstrip("hello  "))   # hello
```

## 17.3 Splitting and joining

| Function | Description | Example |
|----------|-------------|---------|
| `split(s, sep?)` | Split into list | `string.split("a,b,c", ",")` |
| `split_whitespace(s)` | Split on whitespace | `string.split_whitespace("a  b")` |
| `join(sep, list)` | Join list with sep | `string.join(", ", ["a","b"])` |
| `partition(s, sep)` | Split into 3 parts | `string.partition("a=1", "=")` |
| `rpartition(s, sep)` | Split from right | `string.rpartition("a=1=b", "=")` |

```zen
import string

print(string.split("a,b,c", ","))         # ["a", "b", "c"]
print(string.split_whitespace("a  b c"))  # ["a", "b", "c"]
print(string.join(", ", ["a", "b", "c"])) # a, b, c
print(string.partition("a=1", "="))       # ["a", "=", "1"]
print(string.rpartition("a=1=b", "="))    # ["a=1", "=", "b"]
```

## 17.4 Padding and alignment

| Function | Description | Example |
|----------|-------------|---------|
| `ljust(s, width, fill?)` | Left-justify | `string.ljust("x", 5, "-")` → `"x----"` |
| `rjust(s, width, fill?)` | Right-justify | `string.rjust("x", 5, "-")` → `"----x"` |
| `center(s, width, fill?)` | Center | `string.center("x", 5, "-")` → `"--x--"` |
| `zfill(s, width)` | Zero-pad left | `string.zfill("42", 5)` → `"00042"` |

```zen
import string

print(string.ljust("x", 5, "-"))   # x----
print(string.rjust("x", 5, "-"))   # ----x
print(string.center("x", 5, "-"))  # --x--
print(string.zfill("42", 5))       # 00042
```

## 17.5 Case checking

| Function | Description | Example |
|----------|-------------|---------|
| `isupper(s)` | All uppercase | `string.isupper("ZEN")` → `true` |
| `islower(s)` | All lowercase | `string.islower("zen")` → `true` |
| `istitle(s)` | Title case | `string.istitle("Zen Lang")` → `true` |
| `isspace(s)` | All whitespace | `string.isspace("   ")` → `true` |
| `isdigit(s)` | All digits | `string.isdigit("123")` → `true` |
| `isalnum(s)` | Alphanumeric | `string.isalnum("abc123")` → `true` |
| `isalpha(s)` | All letters | `string.isalpha("abc")` → `true` |

```zen
import string

print(string.isupper("HELLO"))  # true
print(string.isdigit("123"))    # true
print(string.isalnum("abc123")) # true
print(string.isspace("   "))    # true
```

## 17.6 Prefix/suffix

| Function | Description | Example |
|----------|-------------|---------|
| `starts_with(s, prefix)` | Boolean | `string.starts_with("zen", "ze")` → `true` |
| `ends_with(s, suffix)` | Boolean | `string.ends_with("zen", "en")` → `true` |

```zen
import string

print(string.starts_with("https://", "https"))  # true
print(string.ends_with("file.z", ".z"))         # true
```

## 17.7 Replacement and translation

| Function | Description | Example |
|----------|-------------|---------|
| `replace(s, old, new)` | Replace all | `string.replace("aaa", "a", "b")` → `"bbb"` |
| `count(s, sub)` | Count occurrences | `string.count("banana", "a")` → `3` |
| `find(s, sub)` | Index or -1 | `string.find("zen", "e")` → `1` |

```zen
import string

print(string.replace("hello world", "world", "zen"))  # hello zen
print(string.count("banana", "a"))                    # 3
print(string.find("zen 3.0", "3"))                    # 4
```

## 17.8 Quick reference

```zen
import string

string.upper("zen")           # ZEN
string.lower("ZEN")           # zen
string.title("zen lang")      # Zen Lang
string.capitalize("zen")      # Zen
string.trim("  x  ")          # x
string.split("a,b,c", ",")    # ["a", "b", "c"]
string.join(", ", ["a","b"])  # a, b
string.ljust("x", 5, "-")     # x----
string.zfill("42", 5)         # 00042
string.isdigit("123")         # true
string.replace("aaa", "a", "b") # bbb
```

## 17.9 Exercises

1. Write `snake_case(name)` that converts "Hello World" to "hello_world".
2. Write `wrap_text(text, width)` that wraps text at `width` characters.
3. Write `is_palindrome(s)` using `string.lower` and slicing.
4. Write `mask_email(email)` that hides the username part.
5. Write `columnate(rows, width)` that prints columns aligned.
