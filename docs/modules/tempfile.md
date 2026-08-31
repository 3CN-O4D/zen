# tempfile — Temporary files and directories

The `tempfile` module provides tools for creating temporary files and folders that are automatically cleaned up by the operating system. It is available globally as `tempfile`.

```zen
# 1. Create a temporary directory
var tmpdir = tempfile.mkdtemp()
print("Working in ${tmpdir}")

# 2. Create a temporary file
var tmpfile = tempfile.mkstemp()
fs.write(tmpfile, "Temp data")
```

## Functions

| Function | Description |
|----------|-------------|
| `mkdtemp()` | Creates a temporary directory and returns its path. |
| `mkstemp()` | Creates a temporary file and returns its path. |
| `dir()` | Returns the system's default temporary directory (e.g., `/tmp`). |

## Examples

### Using a temporary directory for work
```zen
var d = tempfile.mkdtemp()
var out = pathlib.join(d, "output.log")
fs.write(out, "Log data")
# ... do work ...
shutil.rmtree(d) # Clean up manually if needed
```

## See Also
- [fs](fs.md) — For file operations.
- [shutil](shutil.md) — For `rmtree`.
