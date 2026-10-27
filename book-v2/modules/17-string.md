# Chapter 17: string Module

The `string` module provides string utilities: case conversion, trimming, splitting, joining, padding, alignment, and character classification. These operate on Zen strings (immutable `Arc<str>`) and return new strings.

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
print(string.swapcase("HeLLo"))   # hEllO
```

### Edge cases

```zen
print(string.upper(""))           # "" (empty stays empty)
print(string.lower("123ABC"))     # "123abc" (digits unaffected)
print(string.title(""))           # ""
print(string.capitalize(""))      # ""
print(string.swapcase("123"))     # "123" (digits unaffected)
```

`title()` capitalizes the first character of each word and lowercases the rest:
```zen
print(string.title("HELLO WORLD"))  # "Hello World"
print(string.title("hello   world"))  # "Hello   World" (multiple spaces preserved)
```

## 17.2 Trimming

| Function | Description | Example |
|----------|-------------|---------|
| `trim(s)` | Whitespace both ends | `string.trim("  x  ")` → `"x"` |
| `lstrip(s)` | Whitespace left | `string.lstrip("  x")` → `"x"` |
| `rstrip(s)` | Whitespace right | `string.rstrip("x  ")` → `"x"` |

Whitespace includes space, tab, newline, carriage return, form feed.

```zen
import string

print(string.trim("  hello  "))   # hello
print(string.lstrip("  hello"))   # hello
print(string.rstrip("hello  "))   # hello
print(string.trim("\n\t hello \n"))  # hello
```

Custom character trimming is **not** supported — `trim` only removes whitespace.

```zen
print(string.trim("xxhelloxx"))   # "xxhelloxx" (x is not whitespace)
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

### split behavior

```zen
print(string.split("a,,b", ","))            # ["a", "", "b"]  (empty strings preserved)
print(string.split("", ","))                # [""]              (single empty string)
print(string.split("a,b,", ","))            # ["a", "b", ""]
print(string.split_whitespace("  a   b  c  "))  # ["a", "b", "c"]  (empties discarded)
```

`split_whitespace` with no argument splits on any whitespace and discards empty strings:
```zen
print(string.split_whitespace("  a   b  c  "))       # ["a", "b", "c"]
```

### join behavior

```zen
let tags = ["zen", "lang", "script"]
print(string.join(", ", tags))   # "zen, lang, script"
print(string.join("#", tags))    # "zen#lang#script"
print(string.join("", tags))     # "zenlangscript"
```

`join` requires all elements to be strings:
```zen
try {
    print(string.join("-", [1, 2, 3]))  # TypeError
} catch as e {
    print(e.msg)
}
```

### partition / rpartition

`partition` splits on the first occurrence of `sep`, returning `[before, sep, after]`. If `sep` is not found, returns `[s, "", ""]`.

```zen
print(string.partition("a=1=b", "="))   # ["a", "=", "1=b"]
print(string.partition("no_eq", "="))   # ["no_eq", "", ""]
```

`rpartition` splits on the **last** occurrence:
```zen
print(string.rpartition("a=1=b", "="))   # ["a=1", "=", "b"]
print(string.rpartition("no_eq", "="))   # ["no_eq", "", ""]
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

If `width` <= `len(s)`, the original string is returned unchanged:
```zen
print(string.ljust("hello", 3, "-"))   # "hello"
print(string.rjust("hello", 3, "-"))   # "hello"
print(string.center("hello", 3, "-"))  # "hello"
print(string.zfill("hello", 3))        # "hello"
```

`zfill` only pads with `0` and treats the string as potentially signed:
```zen
print(string.zfill("-42", 5))   # "-0042" (sign preserved, zeros after sign)
print(string.zfill("+42", 5))   # "+0042"
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
print(string.isupper("Hello"))  # false
print(string.islower("hello"))  # true
print(string.istitle("Zen Lang"))  # true
print(string.istitle("Zen lang"))  # false
print(string.isspace("   "))    # true
print(string.isspace(""))       # false (empty is not whitespace)
print(string.isdigit("123"))    # true
print(string.isdigit("12.3"))   # false
print(string.isalnum("abc123")) # true
print(string.isalnum("abc_123")) # false
print(string.isalpha("abc"))    # true
print(string.isalpha("abc123")) # false
```

Edge cases:
- `isupper("")` → false (empty has no cased characters)
- `islower("")` → false
- `istitle("")` → false
- `isdigit("")` → false
- `isalnum("")` → false
- `isalpha("")` → false

## 17.6 Prefix/suffix

| Function | Description | Example |
|----------|-------------|---------|
| `starts_with(s, prefix)` | Boolean | `string.starts_with("zen", "ze")` → `true` |
| `ends_with(s, suffix)` | Boolean | `string.ends_with("zen", "en")` → `true` |

```zen
import string

print(string.starts_with("https://", "https"))  # true
print(string.ends_with("file.z", ".z"))         # true
print(string.starts_with("Zen", "zen"))         # false (case-sensitive)
print(string.ends_with("file.Z", ".z"))         # false (case-sensitive)
print(string.starts_with("", ""))               # true (empty prefix matches)
print(string.ends_with("", ""))                 # true
```

## 17.7 Replacement and translation

| Function | Description | Example |
|----------|-------------|---------|
| `replace(s, old, new)` | Replace all | `string.replace("aaa", "a", "b")` → `"bbb"` |
| `count(s, sub)` | Count occurrences | `string.count("banana", "a")` → `3` |
| `find(s, sub)` | Index or -1 | `string.find("zen", "e")` | `1` |

```zen
import string

print(string.replace("hello world", "world", "zen"))  # hello zen
print(string.count("banana", "a"))                    # 3
print(string.find("zen 3.0", "3"))                    # 4
```

### replace replaces ALL occurrences

```zen
print(string.replace("aaa", "a", "b"))   # "bbb"
print(string.replace("ababab", "ab", "xy"))  # "xyxyxy"
print(string.replace("hello", "x", "y"))  # "hello" (not found, unchanged)
```

`replace` with empty `old`:
```zen
print(string.replace("abc", "", "x"))   # behavior: inserts x between every char and at ends
```

### count counts non-overlapping occurrences

```zen
print(string.count("banana", "a"))      # 3
print(string.count("aaaa", "aa"))       # 2  (non-overlapping: positions 0-1 and 2-3)
print(string.count("aaaa", "aaa"))      # 1  (non-overlapping: positions 0-2)
print(string.count("hello", "z"))       # 0
```

### find returns -1 when not found

```zen
print(string.find("zen 3.0", "3"))      # 4
print(string.find("hello", "x"))        # -1
print(string.find("banana", "an", 2))   # 3  (note: string.find in Zen does NOT take a start arg)
```

> Note: Zen's `string.find(s, sub)` takes only two arguments (no start position). To search from a position, slice first: `string.find(s[2:], "an")`.

## 17.8 Quick reference

```zen
import string

string.upper("zen")           # ZEN
string.lower("ZEN")           # zen
string.title("zen lang")      # Zen Lang
string.capitalize("zen")      # Zen
string.swapcase("HeLLo")      # hEllO
string.trim("  x  ")          # x
string.split("a,b,c", ",")    # ["a", "b", "c"]
string.join(", ", ["a","b"])  # a, b
string.ljust("x", 5, "-")     # x----
string.zfill("42", 5)         # 00042
string.isdigit("123")         # true
string.replace("aaa", "a", "b") # bbb
string.count("banana", "a")   # 3
string.find("zen", "e")       # 1
string.starts_with("zen", "ze") # true
string.ends_with("zen", "en") # true
```

## 17.9 Exercises

1. Write `snake_case(name)` that converts "Hello World" to "hello_world".
2. Write `wrap_text(text, width)` that wraps text at `width` characters.
3. Write `is_palindrome(s)` using `string.lower` and slicing.
4. Write `mask_email(email)` that hides the username part.
5. Write `columnate(rows, width)` that prints columns aligned.
6. Write `to_snake_case(text)` handling spaces and hyphens.
7. Write `strip_html_tags(text)` that removes `<...>` tags using repeated `find` and slicing.
8. Write `format_number_with_commas(n)` that inserts commas every 3 digits.


## 17.12 string module — complete reference

Constants: `digits`, `hexdigits`, `octdigits`, `ascii_letters`,
`ascii_lowercase`, `ascii_uppercase`, `punctuation`, `whitespace`, `printable`.

| Function | Purpose | Example |
|----------|---------|---------|
| `upper(s)` | uppercase | `string.upper("abc")` |
| `lower(s)` | lowercase | `string.lower("ABC")` |
| `title(s)` | title case | `string.title("hello world")` |
| `capitalize(s)` | first char upper | `string.capitalize("hello")` |
| `swapcase(s)` | flip case | `string.swapcase("AbC")` |
| `strip(s)` | trim whitespace | `string.strip("  hi  ")` |
| `lstrip(s)` / `rstrip(s)` | trim one side | `string.lstrip("  hi")` |
| `split(s, sep=None)` | split | `string.split("a,b,c", ",")` |
| `join(sep, parts)` | join | `string.join("-", ["a","b"])` |
| `replace(s, old, new)` | replace | `string.replace("aaa","a","b")` |
| `count(s, sub)` | count occurrences | `string.count("banana","an")` |
| `find(s, sub)` | first index or -1 | `string.find("hello","ll")` |
| `startswith(s, prefix)` | boolean | `string.startswith("hello","he")` |
| `endswith(s, suffix)` | boolean | `string.endswith("hello","lo")` |
| `isdigit(s)` | all digits | `string.isdigit("123")` |
| `isalpha(s)` | all letters | `string.isalpha("abc")` |
| `isalnum(s)` | letters/digits | `string.isalnum("abc1")` |
| `isspace(s)` | all whitespace | `string.isspace("   ")` |

### Usage

```zen
import string
print(string.upper("zen"))        # ZEN
print(string.digits)              # "0123456789"
print(string.join(":", ["2024","01","15"]))  # 2024:01:15
print(string.isdigit("42"))       # true
```

### Exercises

1. Write `is_palindrome(s)` using string module case helpers.
2. Implement `mask_all_but_last(s, n)` masking characters.
3. Write `extract_digits(s)` keeping only digits.
4. Implement `title_case(s)` manually and compare to `string.title`.
5. Write `validate_username(s)` (alphanumeric, 3-16 chars).
6. Implement `word_count(text)` using split.
7. Write `remove_punctuation(s)` using `string.punctuation`.
8. Implement `caesar_shift(s, k)` using `ascii_letters`.
9. Write `is_pangram(s)` checking all letters present.
10. Implement `wrap(s, width)` inserting newlines.

