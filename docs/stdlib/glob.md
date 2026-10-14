# glob — path globbing

Shell-style path matching in the spirit of Python's `glob`.

```zen
import glob

print glob.glob("std/*.z")
```

## Functions

| Function | Description |
|----------|-------------|
| `glob(pathname, recursive)` | Matching paths (`*`, `?`, `[...]`; `**` with `recursive=true`). |
| `iglob(pathname, recursive)` | Same matches (eager list, like `glob`). |
| `escape(pathname)` | Escapes magic characters in a literal path. |
| `has_magic(s)` | Whether a pattern contains magic characters. |

## Example

```zen
import glob

print glob.has_magic("std/*.z")  # true
print glob.escape("[abc]")       # escaped literal
for p in glob.glob("tests/cases/3*.z") {
    print p
}
```
