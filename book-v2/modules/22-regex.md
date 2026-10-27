# Chapter 22: re (Regex) Module

The `re` module provides regular expression matching, searching, and replacement. Zen's regex engine supports the common Perl-compatible syntax.

## 22.1 Compiling patterns

```zen
import re

let pattern = re.compile("hello")
let m = pattern.search("hello world")
print(m.group())  # hello
```

If you only need one match, use the module-level functions:

```zen
import re

let m = re.search("world", "hello world")
print(m.group())  # world

let m2 = re.match("hello", "hello world")
print(m2.group()) # hello

let m3 = re.fullmatch("hello", "hello")
print(m3.group()) # hello
```

### compile(pattern)

Compiles a regex pattern for reuse. Returns a regex object with `search`, `match`, `fullmatch`, `findall`, `sub` methods.

```zen
import re

let email_re = re.compile("^[\\w.-]+@[\\w.-]+\\.\\w+$")

for addr in addresses {
    if email_re.match(addr) {
        print("valid: " + addr)
    }
}
```

### search(pattern, text)

Searches for the first match anywhere in the text.

```zen
import re

let m = re.search("world", "hello world")
print(m.group())  # world
```

### match(pattern, text)

Matches at the **start** of the string only (anchored at beginning, not end).

```zen
import re

let m = re.match("hello", "hello world")
print(m.group())  # hello

let m2 = re.match("world", "hello world")
print(m2)         # nil (not at start)
```

### fullmatch(pattern, text)

Matches the **entire** string.

```zen
import re

let m = re.fullmatch("hello", "hello")
print(m.group())  # hello

let m2 = re.fullmatch("hello", "hello world")
print(m2)         # nil (extra characters)
```

## 22.2 Match objects

A successful match returns a match object with:

- `group()` — the full match
- `group(i)` — capture group `i`
- `groups()` — all capture groups
- `start()` / `end()` — match positions
- `span()` — `[start, end]`

```zen
import re

let m = re.search("([a-z]+) ([0-9]+)", "items 42")
print(m.group())      # items 42
print(m.group(1))     # items
print(m.group(2))     # 42
print(m.groups())     # ["items", "42"]
print(m.start(1))     # 7
print(m.end(1))       # 12
print(m.span(1))      # [7, 12]
```

### group(0) is the full match

```zen
import re

let m = re.search("([a-z]+) ([0-9]+)", "items 42")
print(m.group(0))   # items 42 (same as m.group())
print(m.group(1))   # items
print(m.group(2))   # 42
```

### groups() returns all capture groups

```zen
import re

let m = re.search("([a-z]+) ([0-9]+)", "items 42")
print(m.groups())   # ["items", "42"]

let m2 = re.search("([0-9]+)", "abc")
print(m2.groups())  # ["42"]
```

### start/end/span

```zen
import re

let m = re.search("([a-z]+) ([0-9]+)", "items 42")
print(m.start(0))   # 0
print(m.end(0))     # 9
print(m.span(0))    # [0, 9]
print(m.start(1))   # 7
print(m.end(1))     # 10
print(m.start(2))   # 11
print(m.end(2))     # 13
```

## 22.3 Find all matches

```zen
import re

let matches = re.findall("[0-9]+", "a1b23c456")
print(matches)  # ["1", "23", "456"]
```

`findall` returns a list of strings. If the pattern has capture groups, returns a list of tuples (or lists) of groups:

```zen
import re

let matches = re.findall("([a-z]+)([0-9]+)", "a1b23")
print(matches)  # [["a", "1"], ["b", "23"]]
```

## 22.4 Substitution

```zen
import re

let result = re.sub("[0-9]+", "#", "a1b23c456")
print(result)  # a#b#c#

# With count limit
let result2 = re.sub("[0-9]+", "#", "a1b23c456", 1)
print(result2) # a#b23c456
```

`re.sub(pattern, repl, text, count?)` replaces all (or first `count`) occurrences.

```zen
import re

print(re.sub("[aeiou]", "*", "hello world"))  # h*ll* w*rld
print(re.sub(" +", " ", "a   b    c"))        # "a b c"
print(re.sub("[0-9]+", "N", "a1b23c", 1))     # "aNb23c" (only first)
```

## 22.5 Split

```zen
import re

let parts = re.split("[,;]", "a,b;c,d")
print(parts)  # ["a", "b", "c", "d"]
```

`re.split(pattern, text)` splits on the regex pattern. Capture groups in the pattern are included in the result.

```zen
import re

print(re.split("[,;]", "a,b;c,d"))      # ["a", "b", "c", "d"]
print(re.split("([,;])", "a,b;c,d"))    # ["a", ",", "b", ";", "c", "d"]
```

## 22.6 Common patterns

| Pattern | Meaning |
|---------|---------|
| `.` | Any character except newline |
| `\d` | Digit |
| `\D` | Non-digit |
| `\w` | Word character (alnum + underscore) |
| `\W` | Non-word character |
| `\s` | Whitespace |
| `\S` | Non-whitespace |
| `^` | Start of string |
| `$` | End of string |
| `*` | Zero or more |
| `+` | One or more |
| `?` | Zero or one |
| `{n}` | Exactly n |
| `{n,}` | n or more |
| `{n,m}` | Between n and m |
| `\|` | Alternation (OR) |
| `(...)` | Capture group |
| `(?:...)` | Non-capturing group |
| `[...]` | Character class |
| `[^...]` | Negated character class |

### Examples

```zen
import re

# Validate email
let email = re.match("^[\\w.-]+@[\\w.-]+\\.\\w+$", "user@example.com")
print(email != nil)  # true

# Extract hashtags
let tags = re.findall("#\\w+", "hello #zen #lang #script")
print(tags)  # ["#zen", "#lang", "#script"]

# Replace whitespace with single space
let cleaned = re.sub("\\s+", "hello   world", " ")
print(cleaned) # hello world

# Validate phone number
let phone = re.match("^\\d{3}-\\d{3}-\\d{4}$", "555-123-4567")
print(phone != nil)  # true
```

### Escape sequences in Zen strings

In Zen strings, `\\d` becomes the regex `\d`. Use double backslashes:
```zen
import re

let pattern = "\\d{3}-\\d{3}-\\d{4}"  # matches "555-123-4567"
let m = re.match(pattern, "555-123-4567")
print(m != nil)  # true
```

## 22.7 Compiling for reuse

If you use the same pattern many times, compile it once:

```zen
import re

let email_re = re.compile("^[\\w.-]+@[\\w.-]+\\.\\w+$")

for addr in addresses {
    if email_re.match(addr) {
        print("valid: " + addr)
    }
}
```

## 22.8 Common pitfalls

- **Escape backslashes.** In Zen strings, `\\d` becomes the regex `\d`. Use raw strings if available, or double-escape: `"\\\\d"` for a literal backslash-d in the regex.
- **`re.match` anchors at the start** but not the end. Use `re.fullmatch` or add `$` for full-string matching.
- **`re.findall` with groups** returns tuples of groups, not full matches.
- **`re.sub` replaces ALL occurrences** by default; pass `count` to limit.

## 22.9 Quick reference

```zen
import re

re.match(pattern, text)      # match at start
re.search(pattern, text)     # search anywhere
re.fullmatch(pattern, text)  # match entire string
re.findall(pattern, text)    # all matches
re.sub(pattern, repl, text)  # replace
re.split(pattern, text)      # split on pattern
re.compile(pattern)          # compile for reuse

m.group()        # full match
m.group(1)       # capture group 1
m.groups()       # all groups
m.start(1) / m.end(1)  # positions
m.span(1)         # [start, end]
```

## 22.10 Exercises

1. Write `is_valid_email(email)` using a regex.
2. Write `extract_urls(text)` that finds all `http://` or `https://` URLs.
3. Write `mask_phone(phone)` that replaces digits with `X` except the last 4.
4. Write `camel_to_snake(name)` that converts `camelCase` to `camel_case`.
5. Write `highlight(text, word)` that wraps `word` occurrences in `**`.
6. Write `extract_hashtags(text)` that finds all `#word` patterns.
7. Write `validate_hex_color(s)` that checks for `#RRGGBB` format.
