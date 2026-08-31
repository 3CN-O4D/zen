# fs — Filesystem operations

The `fs` module provides a comprehensive suite of tools for interacting with the filesystem. It is available globally as `fs`.

```zen
# 1. Reading and writing files
fs.write("notes.txt", "Zen is fast.")
var content = fs.read("notes.txt")
print(content)

# 2. Directory operations
fs.mkdir("scripts")
fs.write("scripts/test.z", "print(1)")
print(fs.list("scripts")) # [test.z]

# 3. Path helpers
print(fs.join("docs", "modules", "fs.md"))
```

## File Operations

| Function | Description |
|----------|-------------|
| `read(path)` | Reads a file's content as a UTF-8 string. |
| `write(path, data)` | Writes a string to a file (overwrites). |
| `read_binary(path)` | Reads a file as a binary-safe string. |
| `write_binary(path, data)` | Writes a binary-safe string to a file. |
| `append(path, data)` | Appends data to an existing file. |
| `remove(path)` | Deletes a file. |
| `copy(src, dst)` | Copies a file from `src` to `dst`. |
| `move(src, dst)` | Moves or renames a file. |
| `exists(path)` | Returns `true` if the path exists. |
| `is_file(path)` | Returns `true` if the path is a regular file. |
| `size(path)` | Returns the file size in bytes. |
| `mtime(path)` | Returns the last modification time (timestamp). |

## Directory Operations

| Function | Description |
|----------|-------------|
| `mkdir(path)` | Creates a single directory. |
| `mkdirs(path)` | Creates a directory and all missing parents. |
| `rmdir(path)` | Removes an empty directory. |
| `rmtree(path)` | Recursively removes a directory and all its contents. |
| `list(path)` | Returns a list of filenames in the directory. |
| `is_dir(path)` | Returns `true` if the path is a directory. |
| `cwd()` | Returns the current working directory. |
| `cd(path)` | Changes the current working directory. |
| `home()` | Returns the user's home directory. |

## Path Manipulation

| Function | Description |
|----------|-------------|
| `join(parts...)` | Joins multiple path components using the system separator. |
| `basename(path)` | Returns the final component of a path. |
| `dirname(path)` | Returns the directory component of a path. |
| `glob(pattern)` | Returns a list of paths matching a glob pattern (e.g., `*.txt`). |

## Examples

### Checking if a file exists before reading
```zen
var path = "config.json"
if fs.exists(path) {
    var config = json.parse(fs.read(path))
    print("Loaded: ${config}")
} else {
    print("Error: Missing ${path}")
}
```

### Listing all .z files in a project
```zen
var files = fs.glob("**/*.z")
for f in files {
    print("Source: ${f}")
}
```

## See Also
- [pathlib](pathlib.md) — Object-oriented path manipulation.
- [shutil](shutil.md) — High-level file and directory operations.
