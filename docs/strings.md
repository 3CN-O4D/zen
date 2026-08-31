# Strings

Strings are immutable sequences of Unicode characters. `+` concatenates, `*`
repeats, `${...}` interpolates, and a healthy set of methods covers common text
work.

## String literals

```zen
var a = "double quoted"
var b = 'single quoted'
var c = """triple
multi-line"""
var d = '''also triple'''
```

Differences:

| Literal | Interpolation |
|---------|---------------|
| `"..."` | **yes** — `${expr}` |
| `'...'` | no — literal `$` |
| `"""..."""` / `'''...'''` | no interpolation, real newlines |

```zen
print "${1 + 1}"        # "2"
print '${1 + 1}'        # "${1 + 1}" (literal)
```

There are **no backtick template literals**.

## Escape sequences

Standard backslash escapes are processed:

```zen
print len("\n")         # 1            (it's one newline char)
print "a\tb"            # a<TAB>b
print "say \"hi\""      # say "hi"
print "back\\slash"     # back\slash
```

## Interpolation

Use `"${expression}"` inside double quotes:

```zen
var name = "Ada"
var n = 3
print "Hello, ${name}!"
print "2 + 2 = ${2 + 2}"
print "item ${n}/${n + 1}"
```

For fancier formatting, reuse the pieces:

```zen
var price = 19.5
var msg = "Price: ${price.to_fixed(2)}"
print msg                         # Price: 19.50
```

## Indexing

```zen
var s = "hello"
print s[0]            # "h"
print s[-1]           # "o"
print s[10]           # null  (out of range)
```

No slice syntax (`s[1:3]`) — use `.slice()`:

```zen
print s.slice(1)      # "ello"
print s.slice(1, 3)   # "el"
```

Indexed assignment fails — strings are immutable.

## Length

```zen
print len("héllo")    # 5  (characters, not bytes)
print "héllo".len     # 5
print "héllo".length()  # 5
```

## Methods (verified)

| Method | Type | Example → result |
|--------|------|------------------|
| `upper()` | case | `"abc".upper()` → `"ABC"` |
| `lower()` | case | `"ABC".lower()` → `"abc"` |
| `title()` | case | `"hello world".title()` → `"Hello World"` |
| `capitalize()` | case | `"hello".capitalize()` → `"Hello"` |
| `strip()` | whitespace | `"  x  ".strip()` → `"x"` |
| `lstrip()` | leading | `"  x  ".lstrip()` → `"x  "` |
| `rstrip()` | trailing | `"  x  ".rstrip()` → `"  x"` |
| `trim_left()` / `trim_right()` | leading/trailing | `"  x".trim_left()` → `"x"` |
| `center(w)` | padding | `"x".center(5)` → `"  x  "` |
| `zfill(w)` | padding | `"42".zfill(5)` → `"00042"` |
| `split(sep)` | splitting | `"a-b".split("-")` → `["a", "b"]` |
| `splitlines()` | splitting | `"""a\nb""".splitlines()` → `["a", "b"]` |
| `join(sep)` | joining | `["a","b"].join("-")` → `"a-b"` |
| `replace(old, new[, count])` | editing | `"a1a1".replace("1","X")` → `"aXaX"` |
| `count(sub)` | search | `"banana".count("an")` → `2` |
| `find(sub)` | search | `"banana".find("na")` → `2` |
| `contains(sub)` | search | `"banana".contains("ana")` → `true` |
| `startswith(prefix)` | search | `"hello".startswith("hel")` → `true` |
| `endswith(suffix)` | search | `"hello".endswith("lo")` → `true` |
| `isdigit()` | checks | `"123".isdigit()` → `true` |
| `isalpha()` | checks | `"abc".isalpha()` → `true` |
| `isalnum()` | checks | `"ab1".isalnum()` → `true` |
| `islower()` / `isupper()` | checks | `"abc".islower()` → `true` |
| `isspace()` / `is_space()` | checks | `"   ".isspace()` → `true` |
| `is_number()` | checks | `"12.5".is_number()` → `true` |
| `repeat(n)` | repetition | `"ab".repeat(2)` → `"abab"` (same as `"ab" * 2`) |
| `slice(start[, end])` | slicing | `"hello".slice(1, 3)` → `"el"` |

Not method-callable on string values (all exist in the `string` module's
namespace instead): `swapcase`, `ljust`, `rjust`, `rfind`, `to_int`.

### split details

`split` **requires a separator** and treats it **literally** (not a regex):

```zen
print "a b c".split(" ")        # ["a", "b", "c"]
print "a,b;c".split(",")        # ["a", "b;c"]
```

```zen
"a b c".split()                 # Error: split expects a string argument
"a1b22".split("\\d+")           # ["a1b22"]  (no regex splitting!)
```

Splitting on empty gives edge empty strings:

```zen
"abc".split("")                 # ["", "a", "b", "c", ""]
```

For per-character iteration use `split("")` and skip the edge empties, or use
indexing in a loop.

## The `string` module

`string` is a dict of helpers and constants (see
[modules/string](#) for the full reference). Highlights:

```zen
print string.upper("abc")          # ABC
print string.strip("  x  ")        # x
print string.digits                # 0123456789
print string.ascii_lowercase       # abcdefghijklmnopqrstuvwxyz
print string.punctuation           # !"#$%&'()*+,-./:;<=>?@[...]
```

## Comparing & testing strings

```zen
print "a" == "a"        # true  (deep equality)
print "a" < "b"         # true  (lexicographic)
print "ell" in "hello"  # true  (substring membership)

if s.startswith("http") and s.contains("example.com") {
    print "looks like a url"
}
```

## Numbers to strings and back

```zen
print str(42)                 # "42"
print int("42")               # 42
print float("3.14")           # 3.14
print int("nope")             # Error: cannot parse int from "nope"
```

Number value methods for formatting live on the number itself:

```zen
print (3.14159).to_fixed(2)   # "3.14"
print (3.9).to_int()          # 3   (truncation)
```

## Common pitfalls

| Mistake | Reality |
|---------|---------|
| `"a b".split()` | error — a separator is required |
| `"a1".split("\\d")` expecting regex | `.split` is literal — use `re.split` |
| `s[1:3]` slices | unsupported — use `s.slice(1, 3)` |
| `` `template ${x}` `` backticks | unsupported — use `"${x}"` |
| `'${x}'` expecting interpolation | single quotes are literal |
| `s.upper = ...` / `s[0] = ...` | strings are immutable |
| `"abc".swapcase()` / `.ljust(3)` | not methods on values — use `string.swapcase(...)` |
| ``"x\n".len`` counting bytes | `len` counts characters |