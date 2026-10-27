# Chapter 7: Strings Deep Dive

Strings in Zen are immutable sequences of Unicode characters. They are stored as `Arc<str>` (reference-counted, immutable), which makes copying a string cheap (pointer copy) but means every "modification" produces a new string. This chapter is the definitive reference for string literals, indexing, slicing, methods, formatting, immutability, and performance.

## 7.1 String literals

### Single and double quotes

Double and single quotes are interchangeable:

```zen
let a = "hello"
let b = 'world'
```

Escape sequences work in both:

| Escape | Meaning |
|--------|---------|
| `\n` | newline |
| `\t` | tab |
| `\\` | backslash |
| `\'` | single quote |
| `\"` | double quote |
| `\0` | null byte |
| `\xHH` | hex byte (e.g. `\x41` = `A`) |
| `\u{HHHH}` | Unicode scalar (e.g. `\u{1F600}` = 😀) |

```zen
let s1 = "it's a nice day"   # single quotes inside double
let s2 = 'say "hello"'      # double quotes inside single
let s3 = "line1\nline2\tline3"
let s4 = "byte: \x41"       # "byte: A"
let s5 = "emoji: \u{1F600}" # "emoji: 😀"
```

### Triple-quoted raw strings

Triple-quoted strings span multiple lines and do NOT process escape sequences:

```zen
let poem = """Zen is fast,
native, and safe.
No escaping needed here."""
```

```zen
let path = """C:\Users\zen\docs"""   # backslashes are literal
```

### Immutability

Strings are immutable. Indexing `s[i]` reads a character; you cannot assign to `s[i]`. Every "modification" returns a new string:

```zen
let s = "hello"
# s[0] = "H"   # TypeError: strings are immutable

let s2 = "H" + s[1:]   # build a new string
print(s2)   # Hello
print(s)    # hello  (unchanged)
```

This matters for performance: building a string in a loop with `+` creates a new rope each iteration. Zen uses rope concatenation (O(1) per append at the rope level), but the final flatten is O(n). For heavy string building, collect parts in a list and `join` once:

```zen
# Slower: repeated rope concatenation
let result = ""
for i in 1..1000 {
    result = result + "x"
}

# Faster: collect then join
let parts = []
for i in 1..1000 {
    parts.push("x")
}
let result = "".join(parts)
```

## 7.2 Indexing and slicing

### Indexing

`s[i]` returns the character at position `i` as a one-character string. Negative indices count from the end.

```zen
let s = "Zen"

print(s[0])   # "Z"
print(s[1])   # "e"
print(s[2])   # "n"
print(s[-1])  # "n"  (last)
print(s[-2])  # "e"  (second to last)
```

Indexing out of bounds raises `IndexError`:

```zen
try {
    print(s[10])
} catch as e {
    print(e.msg)  # IndexError: index 10 out of bounds for length 3
}
```

### Slicing

`s[a:b]` returns a substring from `a` (inclusive) to `b` (exclusive). Omitting `a` starts at 0; omitting `b` goes to the end. Negative indices work.

```zen
let s = "Zen 3.0"

print(s[0:3])   # "Zen"
print(s[4:6])   # "3."
print(s[:3])    # "Zen"   (from start)
print(s[4:])    # "3.0"   (to end)
print(s[-2:])   # ".0"    (last two)
print(s[:-2])   # "Zen 3" (all but last two)
print(s[::])    # "Zen 3.0" (full copy)
```

Slice bounds are clamped: `s[0:100]` on a 3-char string returns the whole string; `s[-100:2]` returns from the start.

```zen
let s = "Zen"
print(s[0:100])   # "Zen"
print(s[-100:2])  # "Ze"
```

Slicing creates a **new string** (a copy of the substring, or a rope node referencing the original). Mutating the original never affects a slice, and vice versa, because strings are immutable.

## 7.3 Concatenation and repetition

### Concatenation: `+`

When either side of `+` is a string, the other side is stringified via `str()`:

```zen
print("Zen " + "3.0")    # "Zen 3.0"
print("count: " + 42)    # "count: 42"
print("flag: " + true)   # "flag: true"
print(nil + "end")       # "nilend"
print(42 + "count")      # TypeError! (number + string, string not on left)
print(str(42) + "count") # "42count"  (correct)
```

`+` between two strings concatenates them. `+` between a string and anything else stringifies the non-string side via `str()`.

### Repetition: `*`

`*` repeats a string. Either operand may be the string:

```zen
print("x" * 5)      # "xxxxx"
print("ab " * 3)    # "ab ab ab "
print(3 * "ab")     # "ababab"   (left operand is int, still works)
```

Repeating by 0 or a negative number yields `""`:
```zen
print("abc" * 0)    # ""
print("abc" * -1)   # ""
```

## 7.4 String methods

Zen strings have the following methods. All return new strings (or lists/ints/bools) — none mutate the receiver.

| Method | Signature | Description | Example | Result |
|--------|-----------|-------------|---------|--------|
| `upper()` | `str → str` | Uppercase | `"zen".upper()` | `"ZEN"` |
| `lower()` | `str → str` | Lowercase | `"ZEN".lower()` | `"zen"` |
| `title()` | `str → str` | Title case | `"zen language".title()` | `"Zen Language"` |
| `capitalize()` | `str → str` | Capitalize first char | `"zen".capitalize()` | `"Zen"` |
| `swapcase()` | `str → str` | Invert case | `"Zen".swapcase()` | `"zEN"` |
| `trim()` / `strip()` | `str → str` | Whitespace both ends | `"  zen  ".trim()` | `"zen"` |
| `lstrip()` | `str → str` | Whitespace left | `"  zen".lstrip()` | `"zen"` |
| `rstrip()` | `str → str` | Whitespace right | `"zen  ".rstrip()` | `"zen"` |
| `replace(old, new)` | `str → str` | Replace all occurrences | `"aaa".replace("a", "b")` | `"bbb"` |
| `split(sep?)` | `str → list` | Split into list | `"a,b,c".split(",")` | `["a", "b", "c"]` |
| `split_whitespace()` | `str → list` | Split on whitespace | `"a b  c".split_whitespace()` | `["a", "b", "c"]` |
| `join(list)` | `list → str` | Join list with string | `", ".join(["a","b"])` | `"a, b"` |
| `starts_with(prefix)` | `str → bool` | Boolean | `"zen".starts_with("ze")` | `true` |
| `ends_with(suffix)` | `str → bool` | Boolean | `"zen".ends_with("en")` | `true` |
| `contains(sub)` | `str → bool` | Boolean | `"zen".contains("e")` | `true` |
| `find(sub)` | `str → int` | Index or -1 | `"zen".find("e")` | `1` |
| `find(sub, start)` | `str → int` | Index from start | `"zzzen".find("e", 2)` | `4` |
| `count(sub)` | `str → int` | Count occurrences | `"aaa".count("a")` | `3` |
| `len(s)` | builtin | Length (not a method) | `len("zen")` | `3` |
| `ord(ch)` | builtin | Character code | `ord("A")` | `65` |
| `chr(code)` | builtin | Character from code | `chr(65)` | `"A"` |

### Case conversion examples

```zen
import string  # or use methods directly

let s = "Hello World"

print(s.upper())        # "HELLO WORLD"
print(s.lower())        # "hello world"
print(s.title())        # "Hello World"
print(s.capitalize())   # "Hello world"
print(s.swapcase())     # "hELLO wORLD"
```

### Trimming examples

```zen
let s = "  hello  "

print(s.trim())    # "hello"
print(s.lstrip())  # "hello  "
print(s.rstrip())  # "  hello"

# Custom character trimming is NOT supported — trim only removes whitespace
```

### Split examples

```zen
print("a,b,c".split(","))           # ["a", "b", "c"]
print("a b  c".split_whitespace())  # ["a", "b", "c"]
print("a,,b".split(","))            # ["a", "", "b"]  (empty strings preserved)
print("".split(","))                # [""]              (single empty string)
```

`split()` with no argument splits on any whitespace and discards empty strings:
```zen
print("  a   b  c  ".split())       # ["a", "b", "c"]
```

### Join examples

```zen
let tags = ["zen", "lang", "script"]
print(", ".join(tags))   # "zen, lang, script"
print("#".join(tags))    # "zen#lang#script"
print("".join(tags))     # "zenlangscript"
```

`join` requires all elements to be strings:
```zen
try {
    print("-".join([1, 2, 3]))  # TypeError
} catch as e {
    print(e.msg)
}
```

### find / count examples

```zen
let s = "banana"

print(s.find("an"))      # 1   (first occurrence)
print(s.find("an", 2))   # 3   (start at index 2)
print(s.find("zz"))      # -1  (not found)
print(s.count("a"))      # 3
print(s.count("ana"))    # 2   (overlapping: positions 0 and 2)
```

`find` returns -1 when not found (unlike `index` in some languages which raises).

### starts_with / ends_with / contains

```zen
let s = "https://example.com/page"

print(s.starts_with("https"))   # true
print(s.ends_with(".com"))      # false
print(s.ends_with("/page"))     # true
print(s.contains("example"))    # true
print(s.contains("example.org")) # false
```

## 7.5 String formatting

Zen does **not** have f-strings or printf-style formatting built into the language. Use concatenation and `str()`:

```zen
let name = "Zen"
let version = 3.0

print(name + " version " + str(version))
# "Zen version 3.0"

let items = 5
print("You have " + str(items) + " items")
# "You have 5 items"
```

### Padded output

For aligned output, use string repetition and slicing:

```zen
let name = "Zen"
let padded = name + " " * (10 - len(name))
print("[" + padded + "]")
# "[Zen      ]"
```

```zen
func pad_right(s, width) {
    if len(s) >= width { return s }
    return s + " " * (width - len(s))
}

func pad_left(s, width) {
    if len(s) >= width { return s }
    return " " * (width - len(s)) + s
}

print("[" + pad_right("Zen", 10) + "]")   # [Zen      ]
print("[" + pad_left("Zen", 10) + "]")    # [      Zen]
```

### Number formatting

`str()` on floats uses a default representation. There is no built-in `format(2.5, ".2f")`. For fixed decimals, implement manually:

```zen
func format2(x) {
    let scaled = (x * 100).to_int()   # if to_int exists
    // ... manual decimal insertion
}
```

> Note: Zen's float-to-string conversion uses the default Rust `f64` display, which may produce varying precision. For financial/formatted output, implement your own formatter.

## 7.6 Character codes

```zen
print(ord("A"))   # 65
print(ord("z"))   # 122
print(ord("😀"))  # 128512  (Unicode scalar value)
print(chr(65))    # "A"
print(chr(122))   # "z"
print(chr(128512)) # "😀"
```

`ord()` takes a one-character string and returns its Unicode scalar value (u32). `chr()` takes a u32 and returns the corresponding one-character string. `chr()` with an invalid code point may panic or produce a replacement character.

```zen
print(ord("A") == 65)     # true
print(chr(ord("z")) == "z")  # true
```

## 7.7 Immutability and performance

### Immutability

Strings are `Arc<str>` — reference-counted and immutable. You cannot modify a string in place:

```zen
let s = "hello"
# s[0] = "H"   # TypeError: strings are immutable
let s2 = "H" + s[1:]   # new string
```

This means:
- Copying a string is cheap (pointer copy, refcount bump).
- Concatenation allocates.
- Repeated concatenation in a loop allocates each iteration.

### Rope concatenation

Zen uses rope concatenation for `+` on strings. `a + b` creates a rope node referencing both sides without copying. The rope is flattened only when needed (e.g., when passed to `print`, `len`, or indexing). This makes `a + b + c + d` efficient (O(n) total for the final flatten, not O(n²)).

However:
```zen
let result = ""
for i in 1..10000 {
    result = result + "x"   # O(n²) rope nodes, then O(n) flatten
}
```

Prefer collecting in a list and joining:
```zen
let parts = []
for i in 1..10000 {
    parts.push("x")
}
let result = "".join(parts)   # O(n) total
```

## 7.7 Common pitfalls

- **Strings are immutable.** Use concatenation to build new strings; `s[i] = "X"` is a TypeError.
- **`+` auto-stringifies only when a string is present.** `"count: " + 42` works; `42 + "count"` is a TypeError. Use `str(42) + "count"`.
- **`len(s)` is a builtin**, not a method: `len(s)`, not `s.len()`.
- **`split()` with no argument** splits on whitespace and discards empty strings. `split(",")` preserves empty strings.
- **`join` requires all elements to be strings.** `[1, 2, 3]` must be stringified first: `"-".join([str(x) for x in [1,2,3]])`.
- **`find` returns -1** when not found (does not raise).
- **Slicing out of bounds clamps** rather than raising (except direct indexing `s[i]` which raises on out of bounds).
- **Triple-quoted strings do NOT process escapes** — `\n` stays as literal backslash-n.

## 7.9 Quick reference

```zen
let s = "Zen 3.0"

len(s)              # 7
s[0]                # "Z"
s[-1]               # "0"
s[0:3]              # "Zen"
s + "!"             # "Zen 3.0!"
"-" * 3             # "---"
s.upper()           # "ZEN 3.0"
s.lower()           # "zen 3.0"
s.title()           # "Zen 3.0"
s.capitalize()      # "Zen 3.0"
s.trim()            # "Zen 3.0"
s.split(" ")        # ["Zen", "3.0"]
", ".join(["a","b"])# "a, b"
s.contains("3")     # true
s.find("3")         # 4
s.count("3")        # 1
s.starts_with("Ze") # true
s.ends_with(".0")   # true
s.replace("3", "4") # "Zen 4.0"
ord("Z")            # 90
chr(90)             # "Z"
int("42")           # 42
str(42)             # "42"
ord("😀")           # 128512
chr(128512)         # "😀"
```

## 7.10 Exercises

1. Write `is_palindrome(s)` that returns true if `s` reads the same forwards and backwards (case-insensitive). Use `lower()` and slicing.
2. Write `acronym(words)` that takes a phrase and returns its acronym (first letter of each word, uppercase). Use `split_whitespace()` and `upper()`.
3. Write `mask_credit_card(number)` that shows only the last 4 digits, replacing the rest with `*`.
4. Use `split` and `join` to convert a comma-separated string into a pipe-separated string: `"a,b,c"` → `"a|b|c"`.
5. Write `repeat_pattern(pattern, count)` that builds a string by repeating `pattern` `count` times separated by `-`.
6. Write `word_count(text)` that returns the number of words (split on whitespace).
7. Write `is_pangram(text)` that checks if text contains every letter a-z at least once (case-insensitive).
8. Write `camel_to_snake(name)` that converts `camelCase` to `camel_case` by inserting `_` before uppercase letters and lowercasing.
9. Benchmark: build a 100,000-character string with `+` in a loop vs `join`, and compare.
10. Write `strip_quotes(s)` that removes matching surrounding quotes (`"` or `'`) if present.

## 7.12 String encoding and code points vs bytes

Zen strings are sequences of Unicode characters (code points). Indexing and
slicing operate on *characters*, not raw bytes, unless you explicitly work with
`bytes`.

```zen
let s = "Zen"           # 3 code points, 3 bytes
print(len(s))           # 3

let s2 = "Zën"          # ë is one code point, 2 bytes in UTF-8
print(len(s2))          # 3 (characters, not bytes)
```

Convert between strings and bytes explicitly:

```zen
let b = s2.encode("utf-8")   # bytes
let s3 = b.decode("utf-8")   # back to string
print(s2 == s3)              # true
```

### Byte-oriented operations (when you need them)

Work on `bytes` if you need raw offsets, binary protocols, or fixed-width
encodings:

```zen
let b = bytes([72, 101, 108, 108, 111])  # "Hello" in ASCII
print(len(b))   # 5
```

## 7.13 Indexing, slicing, and negative indices

Strings support `s[i]` and `s[start:end]` (end-exclusive, like lists).
Negative indices count from the end.

```zen
let s = "Zen 3.0"
print(s[0])     # "Z"
print(s[-1])    # "0"
print(s[0:3])   # "Zen"
print(s[4:])    # "3.0"
print(s[:3])    # "Zen"
print(s[-3:])   # "3.0"
print(s[0:100]) # "Zen 3.0"  (out-of-range end clamps)
```

Slicing produces a *new* string; strings are immutable.

## 7.14 String immutability

You cannot assign to `s[i]`. Any "modification" creates a new string.

```zen
let s = "hello"
# s[0] = "H"   # error: strings are immutable
let s2 = "H" + s[1:]
print(s2)   # "Hello"
print(s)    # "hello"  (unchanged)
```

## 7.15 String methods — complete reference

Common methods. If a method is missing from your stdlib build, implement it
manually; the behavior shown is the canonical semantics.

| Method | Purpose | Example | Result |
|--------|---------|---------|--------|
| `upper()` | to uppercase | `"abc".upper()` | `"ABC"` |
| `lower()` | to lowercase | `"ABC".lower()` | `"abc"` |
| `title()` | title case | `"hello world".title()` | `"Hello World"` |
| `capitalize()` | first char upper | `"hello".capitalize()` | `"Hello"` |
| `swapcase()` | flip case | `"AbC".swapcase()` | `"aBc"` |
| `strip()` | trim whitespace | `"  hi  ".strip()` | `"hi"` |
| `lstrip()` | trim left | `"  hi".lstrip()` | `"hi"` |
| `rstrip()` | trim right | `"hi  ".rstrip()` | `"hi"` |
| `split(sep)` | split on separator | `"a,b,c".split(",")` | `["a","b","c"]` |
| `splitlines()` | split on newlines | `"a\nb\nc".splitlines()` | `["a","b","c"]` |
| `join(iter)` | join with separator | `"-".join(["a","b"])` | `"a-b"` |
| `replace(old, new)` | replace occurrences | `"aaa".replace("a","b")` | `"bbb"` |
| `count(sub)` | count occurrences | `"banana".count("an")` | `2` |
| `find(sub)` | first index or -1 | `"hello".find("ll")` | `2` |
| `rfind(sub)` | last index or -1 | `"hello".rfind("l")` | `3` |
| `startswith(prefix)` | boolean | `"hello".startswith("he")` | `true` |
| `endswith(suffix)` | boolean | `"hello".endswith("lo")` | `true` |
| `contains(sub)` | boolean | `"hello".contains("ell")` | `true` |
| `ljust(width)` | pad left | `"hi".ljust(5)` | `"hi   "` |
| `rjust(width)` | pad right | `"hi".rjust(5)` | `"   hi"` |
| `center(width)` | center pad | `"hi".center(5)` | `" hi "` |
| `zfill(width)` | zero pad | `"42".zfill(5)` | `"00042"` |
| `isdigit()` | all digits | `"123".isdigit()` | `true` |
| `isalpha()` | all letters | `"abc".isalpha()` | `true` |
| `isalnum()` | letters/digits | `"abc1".isalnum()` | `true` |
| `isspace()` | all whitespace | `"   ".isspace()` | `true` |
| `islower()` | all lowercase | `"abc".islower()` | `true` |
| `isupper()` | all uppercase | `"ABC".isupper()` | `true` |

```zen
print("hello world".replace("world", "Zen"))   # hello Zen
print("-".join(["2024", "01", "15"]))          # 2024-01-15
print("  spaced  ".strip())                    # spaced
print("PYTHON".lower())                        # python
print("hello".count("l"))                      # 2
```

## 7.16 Formatting and interpolation

Double-quoted and triple-quoted strings support `${expr}` interpolation:

```zen
let name = "Ada"
let age = 36
print("Name: ${name}, Age: ${age}")   # Name: Ada, Age: 36
print("${name} is ${age + 1} next year")   # Ada is 37 next year
```

Escapes work inside interpolation strings: `\n \t \\ \" \' \xNN \u{NNNN}`.
Single-quoted and backtick strings do not interpolate:

```zen
print('Cost: ${price}')   # literal: Cost: ${price}
print(`Cost: ${price}`)   # literal, raw
```

Inside `${...}`, the expression is parsed normally; escapes in the surrounding
string are not processed inside the interpolation.

For older-style formatting without interpolation, concatenate or use `str()`:

```zen
print("Value: " + str(42))   # Value: 42
```

## 7.17 String comparison and ordering

Strings compare lexicographically by code point. Uppercase sorts before
lowercase in ASCII/Unicode order.

```zen
print("apple" < "banana")   # true
print("Z" < "a")            # true  (uppercase before lowercase)
print("a" == "A")           # false
print("hello" == "hello")   # true
```

For case-insensitive comparison, normalize:

```zen
func ieq(a, b) { return a.lower() == b.lower() }
print(ieq("Zen", "ZEN"))   # true
```

## 7.18 Common string pitfalls

1. **Assuming `len` is byte length.** It counts characters/code points.
2. **Indexing multi-byte characters.** Indexing is by character, so `s[1]` of
   `"Zën"` is `ë`, not a raw byte.
3. **Strings are immutable.** `s[i] = x` is an error; build a new string.
4. **Case folding for Unicode.** `lower()`/`upper()` handle ASCII well; full
   Unicode case folding may need explicit handling.
5. **Trailing newline in `splitlines`.** A string ending in `\n` may or may not
   produce a trailing empty element depending on the build.
6. **`find` returns -1 when not found**, not `nil` — check for `-1`.
7. **Interpolation inside `${...}` does not process surrounding escapes.**

## 7.19 Exercises

1. Write `is_palindrome(s)` ignoring case and spaces.
2. Implement `count_vowels(s)` returning a dict of vowel counts.
3. Write `mask_card(number)` showing only the last 4 digits.
4. Implement `slugify(text)` → lowercase, spaces to hyphens, strip.
5. Write `format_bytes(n)` producing human-readable sizes (KB, MB).
6. Implement `wrap_text(text, width)` inserting newlines.
7. Write `extract_emails(text)` using `find`/slicing manually.
8. Implement `caesar_cipher(text, shift)` for letters only.
9. Write `validate_password(pw)` checking length, digit, upper, lower.
10. Implement `abbreviate(sentence)` taking first letters of words.

