# Regular Expressions Module

Complete reference for pattern matching, searching, replacing, and splitting strings with regular expressions in Zen.

## Quick Start

```
// Check if string matches pattern (full match)
print re.matches("^\\d+$", "123")          // true

// Find first match
let m = re.search("(\\d+)", "abc 123 def")
print m.match          // 123

// Find all matches
let all = re.findall("\\d+", "a1 b22 c3")
print all              // [1, 22, 3]

// Replace matches
let replaced = re.sub("\\d+", "X", "abc 123 def")
print replaced         // abc X def

// Split by pattern
let parts = re.split("\\s+", "a b   c")
print parts            // [a, b, c]
```

---

## Functions

### `re.matches(pattern, string)` / `re.match(pattern, string)`

Checks if the **entire string** matches the pattern. Returns `true` or `false`.

```
print re.matches("^\\d+$", "123")       // true (all digits)
print re.matches("^\\d+$", "123abc")    // false (has letters)
print re.matches("^[a-z]+$", "hello")   // true (all lowercase)
print re.matches("^[A-Z]", "Hello")     // true (starts with uppercase)
```

### `re.search(pattern, string)`

Finds the **first match** anywhere in the string. Returns a match object or null.

```
let m = re.search("(\\w+)@(\\w+)", "user@example.com")
print m.match         // user@example.com (full match)
print m.group(1)      // user
print m.group(2)      // example
print m.start         // 0
print m.end           // 17
```

### `re.findall(pattern, string)` / `re.find(pattern, string)`

Finds **all matches** and returns them as a list of strings.

```
let numbers = re.findall("\\d+", "a1 b22 c333")
print numbers         // [1, 22, 333]

let words = re.findall("[a-z]+", "Hello World 123")
print words           // [ello, orld]

let emails = re.findall("\\w+@\\w+\\.\\w+", "Contact: alice@co.com or bob@org.net")
print emails          // [alice@co.com, bob@org.net]
```

### `re.split(pattern, string)`

Splits the string by the pattern.

```
let parts = re.split("\\s+", "a b   c")
print parts            // [a, b, c]

let csv_parts = re.split(",\\s*", "a, b, c, d")
print csv_parts        // [a, b, c, d]

let lines = re.split("\\n+", "line1\n\nline2\n\n\nline3")
print lines            // [line1, line2, line3]
```

### `re.sub(pattern, replacement, string)` / `re.replace(pattern, replacement, string)`

Replaces all matches with the replacement string.

```
let result = re.sub("\\d+", "X", "abc 123 def 456")
print result           // abc X def X

let clean = re.sub("[^a-zA-Z0-9]", "_", "Hello, World! 123")
print clean            // Hello__World__123

let spaced = re.sub("([a-z])([A-Z])", "$1 $2", "helloWorld")
print spaced           // hello World
```

---

## ZenRegexMatch Object

Returned by `re.search()`. Properties and methods:

### Properties

| Property | Description |
|----------|-------------|
| `.match` | The full matched string |
| `.start` | Start index of the match |
| `.end` | End index of the match |

### Methods

| Method | Description |
|--------|-------------|
| `.group(n)` | Get capture group n (1-indexed) |
| `.groups()` | Get all capture groups as a list |

### Example

```
let m = re.search("(\\w+)@(\\w+)\\.(\\w+)", "user@example.com")

print m.match             // user@example.com
print m.start             // 0
print m.end               // 17
print m.group(0)          // user@example.com (same as .match)
print m.group(1)          // user
print m.group(2)          // example
print m.group(3)          // com
print m.groups()          // [user, example, com]
```

### When no match is found

```
let m = re.search("\\d+", "no numbers here")
print m                   // null
print m == null           // true
```

---

## Common Patterns

### Email validation

```
function is_email(s) {
    return re.matches("^[\\w.+-]+@[\\w-]+\\.[\\w.]+$", s)
}

print is_email("alice@example.com")    // true
print is_email("invalid@")             // false
print is_email("no-at-sign.com")       // false
```

### Phone number extraction

```
let text = "Call me at 555-123-4567 or 555.987.6543"
let phones = re.findall("\\d{3}[-.]\\d{3}[-.]\\d{4}", text)
print phones    // [555-123-4567, 555.987.6543]
```

### URL extraction

```
let text = "Visit https://example.com or http://test.org/path?q=1"
let urls = re.findall("https?://[^\\s]+", text)
print urls    // [https://example.com, http://test.org/path?q=1]
```

### HTML tag removal

```
let html = "<p>Hello</p><b>World</b>"
let text = re.sub("<[^>]+>", "", html)
print text    // HelloWorld
```

### Cleaning whitespace

```
let messy = "  too   many    spaces  "
let clean = re.sub("\\s+", " ", messy).strip()
print clean    // too many spaces
```

### Extracting key-value pairs

```
let config = "host=localhost port=8080 debug=true"
let pairs = re.findall("(\\w+)=(\\w+)", config)
print pairs    // [[host, localhost], [port, 8080], [debug, true]]
```

### Password strength check

```
function is_strong_password(pw) {
    if pw.len < 8 { return false }
    if !re.search("[A-Z]", pw) { return false }
    if !re.search("[a-z]", pw) { return false }
    if !re.search("[0-9]", pw) { return false }
    if !re.search("[^A-Za-z0-9]", pw) { return false }
    return true
}

print is_strong_password("Weak1!")         // false (too short)
print is_strong_password("Strong1!Pass")   // true
```

### Log parsing

```
let log = "2026-08-17 14:30:00 [INFO] Server started on port 8080"
let parts = re.search("(\\d{4}-\\d{2}-\\d{2} \\d{2}:\\d{2}:\\d{2}) \\[(\\w+)\\] (.+)", log)

print parts.group(1)    // 2026-08-17 14:30:00
print parts.group(2)    // INFO
print parts.group(3)    // Server started on port 8080
```

---

## Regex Syntax Quick Reference

| Pattern | Matches |
|---------|---------|
| `.` | Any character except newline |
| `\\d` | Digit [0-9] |
| `\\D` | Non-digit |
| `\\w` | Word character [a-zA-Z0-9_] |
| `\\W` | Non-word character |
| `\\s` | Whitespace |
| `\\S` | Non-whitespace |
| `^` | Start of string |
| `$` | End of string |
| `*` | Zero or more |
| `+` | One or more |
| `?` | Zero or one (optional) |
| `{n}` | Exactly n times |
| `{n,m}` | Between n and m times |
| `[abc]` | Character class (a, b, or c) |
| `[^abc]` | Negated class (not a, b, or c) |
| `[a-z]` | Range (a through z) |
| `(expr)` | Capture group |
| `(?:expr)` | Non-capturing group |
| `expr1\|expr2` | Alternation (or) |
| `\\b` | Word boundary |

---

## Pro Tips

1. **Use raw strings for patterns.** `re.search("\\d+", s)` — double backslashes for escape sequences.
2. **Use `re.matches()` for full-string validation.** It anchors to start/end automatically.
3. **Use `re.findall()` for extraction.** Returns all matches as a list.
4. **Use `re.sub()` for cleaning.** Replace unwanted patterns with what you want.
5. **Use `?` for non-greedy matching.** `.+?` matches as few characters as possible.

---

## Common Mistakes

### Forgetting double backslashes

```
// WRONG — \d is interpreted as literal 'd'
re.search("\d+", "123")

// CORRECT — escape the backslash
re.search("\\d+", "123")
```

### Using matches() instead of search()

```
// matches() requires full string match
re.matches("\\d+", "abc 123")     // false (doesn't match "abc")

// search() finds pattern anywhere
re.search("\\d+", "abc 123")      // matches "123"
```

### Not anchoring when needed

```
// This matches "123" anywhere
re.search("\\d+", "abc123def")    // true

// This requires the entire string to be digits
re.matches("\\d+", "abc123def")   // false
```

### Forgetting that groups are 1-indexed

```
let m = re.search("(a)(b)(c)", "abc")
print m.group(0)    // abc (full match)
print m.group(1)    // a
print m.group(2)    // b
print m.group(3)    // c
```

---

## See Also

- [Strings](../language/strings.md) — String methods and operations
- [Module Overview](overview.md) — All available modules
