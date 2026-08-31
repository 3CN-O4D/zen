# shutil — High-level file operations

The `shutil` module provides high-level operations for files and directories, such as copying, moving, and recursive deletion. It is available globally as `shutil`.

```zen
# 1. Copying a file
shutil.copy("source.txt", "dest.txt")

# 2. Moving a directory
shutil.move("old_dir", "new_dir")

# 3. Recursive delete
shutil.rmtree("temp_files")
```

## Functions

| Function | Description |
|----------|-------------|
| `copy(src, dst)` | Copies a file from `src` to `dst`. |
| `copy2(src, dst)` | Like `copy`, but also preserves file metadata (mtime, etc.). |
| `move(src, dst)` | Moves a file or directory. |
| `rmtree(path)` | Recursively deletes a directory tree. |
| `copytree(src, dst)` | Recursively copies a directory tree. |
| `which(cmd)` | Returns the absolute path to an executable if it's in the system PATH. |
| `disk_usage(path)` | Returns a dict with `total`, `used`, and `free` bytes for the disk at path. |

## Examples

### Checking if a program exists
```zen
var git_path = shutil.which("git")
if git_path {
    print("Git found at: ${git_path}")
} else {
    print("Git is not installed.")
}
```

### Checking disk space
```zen
var usage = shutil.disk_usage("/")
print("Total disk space: ${usage.total / 1024 / 1024 / 1024} GB")
```

## See Also
- [fs](fs.md) — Standard filesystem module.
- [pathlib](pathlib.md) — Path manipulation.
