# tempfile — temporary files

Temporary files and directories in the spirit of Python's `tempfile`.

```zen
import tempfile

print tempfile.dir()  # /tmp
```

## Functions

| Function | Description |
|----------|-------------|
| `dir()` | Temporary directory path. |
| `mkdtemp(prefix)` | Creates a uniquely-named temp directory, returns its path. |
| `mkstemp(prefix)` | Creates a uniquely-named temp file, returns its path. |

## Example

```zen
import tempfile
import fs

let d = tempfile.mkdtemp("demo")
print d
fs.write(d + "/note.txt", "hello")
```
