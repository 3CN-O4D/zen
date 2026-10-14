# textwrap — wrapping and filling text

Text wrapping in the spirit of Python's `textwrap`.

```zen
import textwrap

print textwrap.fill("aa bb cc dd ee", 8)
# aa bb cc
# dd ee
```

## Functions

| Function | Description |
|----------|-------------|
| `wrap(text, width)` | Wraps to a list of lines (default width 70). Overlong words are broken into width-sized chunks. |
| `fill(text, width)` | Like `wrap`, joined with newlines. |
| `dedent(text)` | Removes common leading whitespace from every line; blank lines become empty. |
| `indent(text, prefix)` | Adds `prefix` (default four spaces) to every non-blank line. |
| `shorten(text, width)` | Collapses to one line of at most `width` columns, ending with `...` when cut. |

## Example

```zen
import textwrap

let poem = "  Roses are red,\n    Violets are blue.\n"
print textwrap.dedent(poem)
# Roses are red,
# Violets are blue.
print textwrap.shorten("aa bb cc dd ee", 8)  # aa bb...
```
