# Chapter 13: File I/O

Zen's `fs` module provides file system operations: reading, writing, creating
directories, checking existence, removing, copying, moving, and more. The `os`
module adds path manipulation. This chapter covers file I/O in depth.

## 13.1 The `fs` module

```zen
import fs

# Read a file
let content = fs.read("hello.txt")

# Write a file
fs.write("out.txt", "Hello, file!")

# Check existence
if fs.exists("data.json") {
    print("exists")
}

# Create directories
fs.mkdirs("a/b/c")

# Remove
fs.remove("temp.txt")
fs.rmtree("a/b/c")

# Check type
print(fs.is_dir("folder"))
print(fs.is_file("file.txt"))

# Path operations
print(fs.join("a", "b", "c"))   # a/b/c
print(fs.basename("/a/b/c.txt")) # c.txt
print(fs.dirname("/a/b/c.txt"))  # /a/b

# Copy and move
fs.copy("src.txt", "dst.txt")
fs.move("old.txt", "new.txt")

# File size
print(fs.size("file.txt"))   # bytes

# List directory
for name in fs.list(".") {
    print(name)
}
```

### `fs.read` vs `fs.read_binary`

```zen
# Text read (returns string)
let text = fs.read("README.md")

# Binary read (returns bytes)
let data = fs.read_binary("image.png")
```

### `fs.write` vs `fs.write_binary`

```zen
# Text write
fs.write("note.txt", "hello")

# Binary write
fs.write_binary("out.bin", bytes_data)
```

## 13.2 The `os` module

```zen
import os

print(os.getcwd())          # current working directory
print(os.getenv("HOME"))    # environment variable
print(os.getpid())          # process ID

# Path manipulation
print(os.path.join("a", "b"))          # a/b
print(os.path.basename("/a/b/c.txt"))  # c.txt
print(os.path.dirname("/a/b/c.txt"))   # /a/b
print(os.path.exists("/tmp/foo"))      # true/false
```

## 13.3 The `pathlib` module (pure-Zen)

```zen
import pathlib

let p = pathlib.Path("/usr/bin/zen")
print(p.name)        # zen
print(p.stem)        # zen
print(p.suffix)      # (empty)
print(p.parent)      # /usr/bin
print(p.exists())    # true/false
print(p.is_file())   # true/false
print(p.is_dir())    # true/false

# Read / write
let content = p.read_text()
p.write_text("new content")

# Iterate directory
for child in pathlib.Path(".").iterdir() {
    print(child.name)
}
```

## 13.4 Reading and writing text files

```zen
import fs

# Read entire file
let content = fs.read("input.txt")
print(content)

# Read line by line
let lines = fs.read("input.txt").split("\n")
for line in lines {
    print(line)
}

# Write entire file
fs.write("output.txt", "Line 1\nLine 2\nLine 3\n")

# Append (read + write)
let existing = fs.exists("log.txt") ? fs.read("log.txt") : ""
fs.write("log.txt", existing + "new entry\n")
```

## 13.5 Binary files

```zen
import fs

# Read binary
let data = fs.read_binary("image.png")
print(len(data))   # byte count

# Write binary
fs.write_binary("copy.png", data)
```

## 13.6 Temporary files

```zen
import tempfile

let tmp = tempfile.mktemp()
fs.write(tmp, "temporary data")
let content = fs.read(tmp)
fs.remove(tmp)

# Or use a context
let dir = tempfile.mkdtemp()
# ... use dir ...
fs.rmtree(dir)
```

## 13.7 The `with` statement (auto-scoping)

Zen's `with` statement provides automatic resource management:

```zen
import fs

with fs.open("data.txt", "r") as f {
    let content = f.read()
    print(content)
}
# f is automatically closed here
```

```zen
with fs.open("out.txt", "w") as f {
    f.write("hello from with")
}
```

## 13.8 Common patterns

### Copy a file

```zen
import fs

fs.copy("source.txt", "destination.txt")
```

### Move a file

```zen
fs.move("old_name.txt", "new_name.txt")
```

### Delete a file safely

```zen
if fs.exists("temp.txt") {
    fs.remove("temp.txt")
}
```

### Walk a directory tree

```zen
import fs

func walk(dir) {
    for name in fs.list(dir) {
        let path = fs.join(dir, name)
        if fs.is_dir(path) {
            walk(path)
        } else {
            print(path)
        }
    }
}
walk(".")
```

### Read CSV (simple)

```zen
import fs

let lines = fs.read("data.csv").split("\n")
let header = lines[0].split(",")
for line in lines[1:] {
    if line == "" { continue }
    let values = line.split(",")
    let row = {}
    for i in 0..len(header) {
        row[header[i]] = values[i]
    }
    print(row)
}
```

## 13.9 Common pitfalls

- **`fs.read` returns a string.** For binary data, use `fs.read_binary`.
- **`fs.write` overwrites.** It does not append. Read-modify-write to append.
- **Directories must exist before writing.** Use `fs.mkdirs` first.
- **Paths use `/`** even on Windows (Zen normalizes them).
- **`fs.exists` returns `false` for broken symlinks.**

## 13.10 Quick reference

```zen
import fs
import os
import pathlib

fs.read("file.txt")           # string
fs.read_binary("file.bin")    # bytes
fs.write("file.txt", "data")
fs.write_binary("file.bin", bytes)
fs.exists("file.txt")
fs.is_dir("folder")
fs.is_file("file.txt")
fs.mkdirs("a/b/c")
fs.remove("file.txt")
fs.rmtree("dir")
fs.size("file.txt")
fs.join("a", "b")
fs.basename("/a/b/c.txt")
fs.dirname("/a/b/c.txt")
fs.copy("src", "dst")
fs.move("old", "new")
fs.list(".")                  # directory entries

os.getcwd()
os.getenv("HOME")
os.getpid()
os.path.join("a", "b")

import pathlib
pathlib.Path("file.txt").read_text()
pathlib.Path("file.txt").write_text("data")
```

## 13.11 Exercises

1. Write a function `file_size_human(path)` that returns a human-readable
   file size (e.g. "1.5 MB").
2. Write a script that copies all `.txt` files from one directory to another.
3. Write `grep_file(pattern, path)` that prints lines containing `pattern`.
4. Write a file tailer that reads the last `n` lines of a file.
5. Use `with fs.open(...)` to read a file and count its words.
