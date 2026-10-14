# string — string utilities

String helpers in the spirit of Python's `string` module, plus the classic
constant groups. Most entries mirror a string method as a plain function.

```zen
import string

print string.join(", ", ["a", "b"])  # a, b
```

## Functions

Case: `upper`, `lower`, `title`, `capitalize`, `swapcase`. Trimming: `strip`,
`lstrip`, `rstrip`. Splitting: `split(s, sep)`, `splitlines`. Building:
`join(sep, items)`, `replace`, `repeat`. Searching: `count`, `find`,
`rfind`, `startswith`, `endswith`, `contains`. Padding: `ljust`, `rjust`,
`center`, `zfill` (each with optional fill). Checks: `isdigit`, `isalpha`,
`isalnum`, `isspace`, `islower`, `isupper`.

## Constants

`digits`, `hexdigits`, `octdigits`, `ascii_letters`, `ascii_lowercase`,
`ascii_uppercase`, `punctuation`, `whitespace`, `printable`.

## Example

```zen
import string

print string.center("hi", 6, "-")  # --hi--
print string.zfill("42", 5)        # 00042
print string.ascii_lowercase[:5]   # abcde
```
