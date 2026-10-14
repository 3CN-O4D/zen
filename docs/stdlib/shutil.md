# shutil — file operations

File operations in the spirit of Python's `shutil`.

```zen
import shutil

shutil.copy("a.txt", "b.txt")
```

## Functions

| Function | Description |
|----------|-------------|
| `copy(src, dst)` / `copy2(src, dst)` / `copyfile(src, dst)` | File copies. |
| `copytree(src, dst, dirs_exist_ok)` | Recursive tree copy. |
| `move(src, dst)` | Move/rename. |
| `rmtree(path)` | Recursive delete. |
| `which(cmd, path)` | Executable lookup (`path` defaults to `PATH`). |

## Example

```zen
import shutil

print shutil.which("ls")  # /usr/bin/ls (or wherever ls lives)
```
