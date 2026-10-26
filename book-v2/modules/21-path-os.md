# Chapter 21: pathlib & os Module

The `pathlib` module provides an object-oriented filesystem path API, and the
`os` module provides low-level OS operations: environment variables, process
info, and path manipulation.

## 21.1 pathlib.Path

```zen
import pathlib

let p = pathlib.Path("/usr/bin/zen")

print(p.name)        # zen
print(p.stem)        # zen
print(p.suffix)      # (empty for no extension)
print(p.parent)      # /usr/bin
print(p.parts)       # ["", "usr", "bin", "zen"]
print(p.exists())    # true/false
print(p.is_file())   # true/false
print(p.is_dir())    # true/false

# Read / write text
let content = p.read_text()
p.write_text("new content")

# Iterate directory
for child in pathlib.Path(".").iterdir() {
    print(child.name)
}

# Join paths
let p2 = pathlib.Path("/usr").joinpath("bin", "zen")
print(p2)  # /usr/bin/zen
```

### Path operations

```zen
import pathlib

let p = pathlib.Path("docs/guide.z")

print(p.with_name("index.z"))   # docs/index.z
print(p.with_suffix(".md"))     # docs/guide.md
print(p.absolute())             # /cwd/docs/guide.z
print(p.relative_to("docs"))    # guide.z
```

## 21.2 The `os` module

```zen
import os

# Process info
print(os.getpid())          # process ID
print(os.getppid())         # parent process ID
print(os.getuid())          # user ID
print(os.geteuid())         # effective user ID
print(os.getgid())          # group ID
print(os.getegid())         # effective group ID

# Environment
print(os.getenv("HOME"))    # /home/user
print(os.getenv("PATH", "/usr/bin"))  # with default

# Working directory
print(os.getcwd())          # /current/dir
```

## 21.3 Path manipulation: os.path

```zen
import os

print(os.path.join("a", "b", "c"))        # a/b/c
print(os.path.basename("/a/b/c.txt"))     # c.txt
print(os.path.dirname("/a/b/c.txt"))      # /a/b
print(os.path.exists("/tmp/foo"))         # true/false
print(os.path.isfile("/tmp/foo.txt"))     # true/false
print(os.path.isdir("/tmp"))              # true/false
print(os.path.abspath("relative.z"))      # /abs/relative.z
print(os.path.realpath("/a/b/c.txt"))     # resolved path
```

## 21.4 Common patterns

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

### Check if path exists before writing

```zen
import os
import fs

let path = "output/result.txt"
if !os.path.exists("output") {
    fs.mkdirs("output")
}
fs.write(path, "result")
```

### Read environment with default

```zen
import os

let port = os.getenv("PORT", "8080")
let host = os.getenv("HOST", "localhost")
let debug = os.getenv("DEBUG", "false") == "true"

print("Listening on " + host + ":" + port)
```

### Expand user home

```zen
import os

let home = os.getenv("HOME")
let config = os.path.join(home, ".config", "zen", "config.z")
print(config)
```

## 21.5 Quick reference

```zen
import pathlib
import os

pathlib.Path("/usr/bin/zen")
pathlib.Path("file.txt").read_text()
pathlib.Path("file.txt").write_text("data")
pathlib.Path(".").iterdir()

os.getcwd()
os.getenv("HOME", "/default")
os.getpid()
os.path.join("a", "b")
os.path.basename("/a/b/c.txt")
os.path.dirname("/a/b/c.txt")
os.path.exists(path)
os.path.isfile(path)
os.path.isdir(path)
```

## 21.9 Exercises

1. Write a function `find_files(dir, extension)` that returns all files with
   a given extension under `dir` (recursive).
2. Write `backup_file(path)` that copies `path` to `path.bak`.
3. Write `ensure_dir(path)` that creates parent directories if missing.
4. Write a script that prints all environment variables containing "ZEN".
5. Write `human_path(path)` that replaces `$HOME` with `~`.
