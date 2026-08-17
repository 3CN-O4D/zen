# File System Module

Complete reference for reading, writing, and manipulating files and directories in Zen.

## Quick Start

```
// Read a file
let content = fs.read("data.txt")
print content

// Write a file (overwrites)
fs.write("output.txt", "Hello, World!")

// Append to a file
fs.append("log.txt", "New log entry\n")

// Check if file exists
print fs.exists("data.txt")     // true

// List directory
let files = fs.readdir(".")
print files

// Create directory
fs.mkdir("new_folder")
```

---

## Reading Files

### `fs.read(path)` — Read entire file

```
let content = fs.read("config.txt")
print content
```

### `fs.read_lines(path)` — Read as list of lines

```
let lines = fs.read_lines("data.txt")
for line in lines {
    print line
}
```

### `fs.open(path, mode)` — Open file handle

Modes:
- `"r"` — read
- `"w"` — write (create/truncate)
- `"a"` — append
- `"rb"` — read binary
- `"wb"` — write binary

```
let f = fs.open("data.txt", "r")
let line = f.readline()
print line
f.close()
```

### Read line by line

```
let f = fs.open("huge.log", "r")
while !f.eof() {
    let line = f.readline()
    // process line
}
f.close()
```

---

## Writing Files

### `fs.write(path, content)` — Write/create file

```
fs.write("output.txt", "Hello, World!")
fs.write("data.json", json.encode(data))
```

### `fs.append(path, content)` — Append to file

```
fs.append("log.txt", "Entry at {datetime.now()}\n")
```

### Write with file handle

```
let f = fs.open("output.txt", "w")
f.write("Line 1\n")
f.write("Line 2\n")
f.close()
```

---

## File Operations

### `fs.exists(path)` — Check existence

```
if fs.exists("config.txt") {
    let config = json.load("config.txt")
} else {
    print "Config not found"
}
```

### `fs.is_file(path)` — Check if file

```
if fs.is_file("data.txt") {
    let content = fs.read("data.txt")
}
```

### `fs.is_dir(path)` — Check if directory

```
if fs.is_dir("my_folder") {
    let items = fs.readdir("my_folder")
}
```

### `fs.size(path)` — Get file size

```
let bytes = fs.size("large_file.zip")
print "File size: {bytes} bytes"
```

---

## Directory Operations

### `fs.readdir(path)` — List directory

```
let items = fs.readdir(".")
for item in items {
    print item
}
```

### `fs.mkdir(path)` — Create directory

```
fs.mkdir("new_folder")
fs.mkdir("a/b/c")    // creates nested directories
```

### `fs.rm(path)` — Remove file

```
fs.rm("temp.txt")
```

### `fs.rmdir(path)` — Remove directory

```
fs.rmdir("empty_folder")
```

### `fs.rename(old, new)` — Rename/move

```
fs.rename("old_name.txt", "new_name.txt")
fs.rename("file.txt", "archive/file.txt")
```

---

## Path Utilities

### `fs.join(parts...)` — Join path segments

```
let path = fs.join("home", "user", "docs")
print path    // home/user/docs
```

### `fs.dirname(path)` — Get directory part

```
print fs.dirname("/home/user/file.txt")    // /home/user
```

### `fs.basename(path)` — Get filename part

```
print fs.basename("/home/user/file.txt")    // file.txt
```

### `fs.ext(path)` — Get file extension

```
print fs.ext("data.json")    // .json
print fs.ext("README.md")    // .md
```

### `fs.abs(path)` — Get absolute path

```
print fs.abs("data.txt")    // /home/user/project/data.txt
```

---

## JSON Convenience

### `fs.load(path)` — Read and parse JSON

```
let data = fs.load("config.json")
print data.host
print data.port
```

### `fs.save(path, data)` — Encode and write JSON

```
let config = {host: "localhost", port: 8080}
fs.save("config.json", config)
```

---

## Module Loading

### `fs.load_module(path)` — Load a Zen module

```
let my_module = fs.load_module("utils.z")
let result = my_module.do_something()
```

### Dynamic module loading

```
let module_name = "math_utils"
let module = fs.load_module("{module_name}.z")

// Use module functions
let result = module.add(1, 2)
```

---

## Common Patterns

### Copying a file

```
function copy_file(src, dest) {
    let content = fs.read(src)
    fs.write(dest, content)
}

copy_file("source.txt", "backup.txt")
```

### Reading config with defaults

```
function load_config(path, defaults) {
    if !fs.exists(path) {
        return defaults
    }
    let content = fs.read(path)
    return json.parse(content)
}

let defaults = {host: "localhost", port: 8080}
let config = load_config("config.json", defaults)
```

### Walking a directory tree

```
function walk(dir, callback) {
    let items = fs.readdir(dir)
    for item in items {
        let path = fs.join(dir, item)
        if fs.is_dir(path) {
            walk(path, callback)
        } else {
            callback(path)
        }
    }
}

walk(".", function(path) {
    if fs.ext(path) == ".z" {
        print "Zen file: {path}"
    }
})
```

### Safe file operations

```
function safe_read(path, default) {
    try {
        return fs.read(path)
    } catch err {
        return default
    }
}

let content = safe_read("optional.txt", "No content")
```

---

## Pro Tips

1. **Use `fs.read_lines()` for large files.** Avoids loading entire file into memory.
2. **Use `fs.exists()` before reading.** Prevents errors.
3. **Use `fs.load()` for JSON files.** Combines read + parse.
4. **Use `fs.join()` for paths.** Cross-platform path construction.
5. **Always close file handles.** Use `f.close()` after done.

---

## Common Mistakes

### Not closing file handles

```
// BAD — file handle left open
let f = fs.open("data.txt", "r")
let content = f.read()
// f.close() missing!

// GOOD — always close
let f = fs.open("data.txt", "r")
let content = f.read()
f.close()
```

### Wrong mode for append

```
// BAD — overwrites file
fs.write("log.txt", "new entry\n")

// GOOD — appends to file
fs.append("log.txt", "new entry\n")
```

### Relative vs absolute paths

```
// Relative to current directory
fs.read("data.txt")

// Absolute path
fs.read("/home/user/data.txt")

// Check current directory
print fs.cwd()
```

---

## See Also

- [JSON Module](json.md) — JSON encoding/decoding
- [os Module](overview.md) — Process and environment info
- [Module Overview](overview.md) — All available modules
