# Chapter 7: Strings Deep Dive

Strings in Zen are immutable sequences of characters. They support indexing,
slicing, concatenation, repetition, and a rich set of methods. This chapter
covers everything you need to know about strings.

## 7.1 String literals

```zen
let s1 = "hello"
let s2 = 'world'
let s3 = "it's a nice day"
let s4 = "say \"hello\""
let s5 = "line1\nline2\tline3"
```

Single and double quotes are interchangeable. Escape sequences: `\n`, `\t`,
`\\`, `\'`, `\"`.

## 7.2 Indexing and slicing

```zen
let s = "Zen"

print(s[0])   # Z
print(s[1])   # e
print(s[2])   # n
print(s[-1])  # n (last character)
print(s[-2])  # e
```

Slicing with `[start:end]`:

```zen
let s = "Zen 3.0"

print(s[0:3])   # Zen
print(s[4:6])   # 3.
print(s[:3])    # Zen (from start)
print(s[4:])    # 3.0 (to end)
print(s[-2:])   # .0 (last two)
print(s[:-2])   # Zen 3 (all but last two)
```

Slice step is not supported — use `[start:end]` only.

## 7.3 Concatenation and repetition

```zen
# Concatenation with +
print("Zen " + "3.0")    # Zen 3.0

# Concatenation auto-stringifies non-strings
print("count: " + 42)    # count: 42
print("flag: " + true)   # flag: true
print(nil + "end")       # nilend

# Repetition with *
print("x" * 5)           # xxxxx
print("ab " * 3)         # ab ab ab 
```

## 7.4 String methods

| Method | Description | Example | Result |
|--------|-------------|---------|--------|
| `upper()` | Uppercase | `"zen".upper()` | `ZEN` |
| `lower()` | Lowercase | `"ZEN".lower()` | `zen` |
| `title()` | Title case | `"zen language".title()` | `Zen Language` |
| `capitalize()` | Capitalize first | `"zen".capitalize()` | `Zen` |
| `trim()` / `strip()` | Whitespace both ends | `"  zen  ".trim()` | `zen` |
| `lstrip()` | Whitespace left | `"  zen".lstrip()` | `zen` |
| `rstrip()` | Whitespace right | `"zen  ".rstrip()` | `zen` |
| `replace(old, new)` | Replace all | `"aaa".replace("a", "b")` | `bbb` |
| `split(sep?)` | Split into list | `"a,b,c".split(",")` | `["a", "b", "c"]` |
| `split_whitespace()` | Split on whitespace | `"a b  c".split_whitespace()` | `["a", "b", "c"]` |
| `join(list)` | Join list with string | `", ".join(["a","b"])` | `a, b` |
| `starts_with(prefix)` | Boolean | `"zen".starts_with("ze")` | `true` |
| `ends_with(suffix)` | Boolean | `"zen".ends_with("en")` | `true` |
| `contains(sub)` | Boolean | `"zen".contains("e")` | `true` |
| `find(sub)` | Index or -1 | `"zen".find("e")` | `1` |
| `find(sub, start)` | Index from start | `"zzzen".find("e", 2)` | `4` |
| `count(sub)` | Count occurrences | `"aaa".count("a")` | `3` |
| `len(s)` | Length (builtin) | `len("zen")` | `3` |
| `ord(ch)` | Character code | `ord("A")` | `65` |
| `chr(code)` | Character from code | `chr(65)` | `A` |
| `int(s)` | Parse integer | `int("42")` | `42` |
| `float(s)` | Parse float | `float("3.14")` | `3.14` |
| `str(x)` | Stringify | `str(42)` | `"42"` |

### Examples

```zen
let s = "  Hello World  "
print(s.trim())           # Hello World
print(s.upper())          #   HELLO WORLD  
print(s.trim().upper())   # HELLO WORLD

let path = "/usr/bin/zen"
print(path.split("/"))    # ["", "usr", "bin", "zen"]

let tags = ["zen", "lang", "script"]
print(", ".join(tags))    # zen, lang, script

print("2024-01-15".starts_with("2024"))  # true
print("file.z".ends_with(".z"))          # true
print("zen".find("n"))                   # 2
print("banana".count("a"))               # 3
print("zen".replace("z", "Z"))           # Zen
```

## 7.5 String formatting

Zen does not have f-strings. Use concatenation and `str()`:

```zen
let name = "Zen"
let version = 3.0

print(name + " version " + str(version))
# Zen version 3.0

let items = 5
print("You have " + str(items) + " items")
# You have 5 items
```

For aligned or padded output, use string repetition and slicing:

```zen
let name = "Zen"
let padded = name + " " * (10 - len(name))
print("[" + padded + "]")
# [Zen      ]
```

## 7.6 Character codes

```zen
print(ord("A"))   # 65
print(ord("z"))   # 122
print(chr(65))    # A
print(chr(122))   # z
```

## 7.7 Immutability

Strings are immutable — you cannot modify them in place. Operations that
"change" a string return a new string:

```zen
let s = "hello"
# s[0] = "H"   # TypeError: strings are immutable
let s2 = "H" + s[1:]
print(s2)  # Hello
print(s)   # hello (unchanged)
```

## 7.8 Common pitfalls

- **Strings are immutable.** Use concatenation to build new strings.
- **`+` auto-stringifies.** `"count: " + 42` works, but `42 + "count"` does
  not (number + string is a type error).
- **`len(s)` is a builtin**, not a method: use `len(s)`, not `s.len()`.
- **`split()` with no argument** splits on whitespace, not commas.
- **Slicing is exclusive of `end`:** `s[0:3]` gives indices 0, 1, 2.

## 7.9 Quick reference

```zen
let s = "Zen 3.0"

len(s)              # 7
s[0]                # Z
s[-1]               # 0
s[0:3]              # Zen
s + "!"             # Zen 3.0!
"-" * 3             # ---
s.upper()           # ZEN 3.0
s.trim()            # Zen 3.0
s.split(" ")        # ["Zen", "3.0"]
", ".join(["a","b"])# a, b
s.contains("3")     # true
s.find("3")         # 4
ord("Z")            # 90
chr(90)             # Z
int("42")           # 42
str(42)             # "42"
```

## 7.10 Exercises

1. Write a function `is_palindrome(s)` that returns `true` if `s` reads the
   same forwards and backwards (case-insensitive).
2. Write a function `acronym(words)` that takes a phrase and returns its
   acronym (first letter of each word, uppercase).
3. Write a function `mask_credit_card(number)` that shows only the last 4
   digits, replacing the rest with `*`.
4. Use `split` and `join` to convert a comma-separated string into a
   pipe-separated string.
5. Write a function `repeat_pattern(pattern, count)` that builds a string by
   repeating `pattern` `count` times separated by `-`.
