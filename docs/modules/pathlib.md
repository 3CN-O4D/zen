# pathlib — Path manipulation

The `pathlib` module provides an object-oriented and functional way to handle filesystem paths. It is available globally as `pathlib`.

```zen
# 1. Joining paths
var p = pathlib.join("src", "main.z")
print(p)  # src/main.z

# 2. Extracting parts
print(pathlib.name("docs/index.md")) # index.md
print(pathlib.stem("docs/index.md")) # index
print(pathlib.suffix("docs/index.md")) # .md
```

## Functions

| Function | Description |
|----------|-------------|
| `join(parts...)` | Joins path components. |
| `name(path)` | Returns the filename with suffix. |
| `stem(path)` | Returns the filename without suffix. |
| `suffix(path)` | Returns the file extension (e.g., `.md`). |
| `suffixes(path)` | Returns a list of extensions (e.g., `[.tar, .gz]`). |
| `parent(path)` | Returns the directory containing the file/folder. |
| `absolute(path)` | Returns an absolute path string. |
| `resolve(path)` | Resolves symlinks and `..` to return a canonical path. |
| `is_absolute(path)` | Returns `true` if the path is absolute. |

## Filesystem Interaction

| Function | Description |
|----------|-------------|
| `exists(path)` | Checks if the path exists. |
| `is_file(path)` | Checks if the path is a file. |
| `is_dir(path)` | Checks if the path is a directory. |
| `glob(pattern)` | Returns a list of matching paths. |
| `read_text(path)` | Shortcut for `fs.read()`. |
| `write_text(path, s)` | Shortcut for `fs.write()`. |
| `mkdir(path)` | Creates the directory. |
| `rmdir(path)` | Removes the directory. |
| `unlink(path)` | Deletes a file. |
| `rename(old, new)` | Renames the path. |

## Examples

### Building a backup path
```zen
var original = "data.csv"
var backup = pathlib.join("backups", pathlib.stem(original) + "_backup" + pathlib.suffix(original))
print(backup) # backups/data_backup.csv
```

## See Also
- [fs](fs.md) — Standard filesystem module.
- [shutil](shutil.md) — High-level file operations.
