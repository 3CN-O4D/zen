# Chapter 22: re (Regex) Module

The `re` module provides regular expression matching, searching, and
replacement. Zen's regex engine supports the common Perl-compatible syntax.

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

## 22.3 Find all matches

```zen
import re

let matches = re.findall("[0-9]+", "a1b23c456")
print(matches)  # ["1", "23", "456"]
```

`findall` returns a list of strings (or tuples of groups if the pattern has
capture groups).

## 22.4 Substitution

```zen
import re

let result = re.sub("[0-9]+", "#", "a1b23c456")
print(result)  # a#b#c#

# With count limit
let result2 = re.sub("[0-9]+", "#", "a1b23c456", 1)
print(result2) # a#b23c456
```

## 22.5 Split

```zen
import re

let parts = re.split("[,;]", "a,b;c,d")
print(parts)  # ["a", "b", "c", "d"]
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

- **Escape backslashes.** In Zen strings, `\\d` becomes the regex `\d`.
  Use raw strings if available, or double-escape: `"\\\\d"` for a literal
  backslash-d in the regex.
- **`re.match` anchors at the start** but not the end. Use `re.fullmatch` or
  add `$` for full-string matching.
- **`re.findall` with groups** returns tuples of groups, not full matches.

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
```

## 22.10 Exercises

1. Write `is_valid_email(email)` using a regex.
2. Write `extract_urls(text)` that finds all `http://` or `https://` URLs.
3. Write `mask_phone(phone)` that replaces digits with `X` except the last 4.
4. Write `camel_to_snake(name)` that converts `camelCase` to `camel_case`.
5. Write `highlight(text, word)` that wraps `word` occurrences in `**`.
