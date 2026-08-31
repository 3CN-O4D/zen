# glob — File pattern matching

The `glob` module provides a simple way to find files and directories whose names match a specific pattern. It is available globally as `glob`.

```zen
# Find all .z files in the current directory
var scripts = glob.glob("*.z")
print(scripts)

# Find all files recursively
var all = glob.glob("**/*")
```

## Functions

| Function | Description |
|----------|-------------|
| `glob(pattern)` | Returns a list of paths matching a Unix-style glob pattern. |

## Patterns

- `*`: Matches zero or more characters.
- `?`: Matches a single character.
- `**`: Matches zero or more directories (recursive match).
- `[...]`: Matches a range or set of characters (e.g., `[0-9]`).

## Examples

### Processing all CSV files
```zen
var files = glob.glob("data/*.csv")
for f in files {
    print("Processing ${f}...")
    var data = csv.read(f)
    # ...
}
```

## See Also
- [fs](fs.md) — The `fs.glob()` function is an alias for this.
- [pathlib](pathlib.md) — Provides `pathlib.glob()`.
