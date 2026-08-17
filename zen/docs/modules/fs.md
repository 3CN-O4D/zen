# Zen Filesystem Module (`fs`)

The `fs` module is Zen's primary interface for interacting with the local filesystem. It provides functions for reading files, writing data, manipulating directories, and querying path metadata. All functions are available as globals — no import statement is required.

Every function follows a consistent pattern: accept string paths, return clear results, and raise errors on failure. Paths can be absolute or relative to the current working directory.

---

## Table of Contents

- [Reading Files](#reading-files)
- [Writing Files](#writing-files)
- [Appending to Files](#appending-to-files)
- [Binary Read/Write](#binary-readwrite)
- [File Metadata](#file-metadata)
- [Directory Operations](#directory-operations)
- [File Operations](#file-operations)
- [Path Utilities](#path-utilities)
- [Glob Pattern Matching](#glob-pattern-matching)
- [Common Patterns](#common-patterns)

---

## Reading Files

### `fs.read(path)` → `string`

Reads an entire file and returns its contents as a string. The file must exist or an error is raised.

```zen
// Read a configuration file
config := fs.read("config.json")
print(config)
```

```
// Expected output (depends on file contents):
{"port": 8080, "debug": true, "name": "zen-app"}
```

```zen
// Read a script or source file
source := fs.read("main.zen")
print(source)
```

```
// Expected output:
fn greet(name: string) {
    print("Hello, {name}!")
}

greet("World")
```

```zen
// Reading a non-existent file raises an error
fs.read("missing.txt")
// Error: file not found: missing.txt
```

**Key behavior:**
- Returns the full file content as a single string.
- The file must exist; if it doesn't, an error is raised (not `null`).
- Works best for text files. For binary data, use `fs.read_binary()`.

---

### `fs.exists(path)` → `boolean`

Returns `true` if the path exists (file, directory, or anything else), `false` otherwise.

```zen
// Check if a file exists before reading it
if fs.exists("config.json") {
    config := fs.read("config.json")
    print("Loaded config: {config}")
} else {
    print("No config file found, using defaults.")
}
```

```
// Expected output (if config.json exists):
Loaded config: {"port": 8080, "debug": true, "name": "zen-app"}
```

```zen
// Check a directory
print(fs.exists("/tmp"))          // true
print(fs.exists("/nonexistent"))  // false
```

```
// Expected output:
true
false
```

---

### `fs.is_file(path)` / `fs.isFile(path)` → `boolean`

Returns `true` if the path exists and is a regular file (not a directory or symlink).

```zen
print(fs.is_file("main.zen"))     // true  (it's a file)
print(fs.is_file("src/"))         // false (it's a directory)
print(fs.is_file("/tmp"))         // false (it's a directory)
```

```
// Expected output:
true
false
false
```

```zen
// Guard against reading a directory as a file
path := "data.txt"
if fs.is_file(path) {
    content := fs.read(path)
    print(content)
} else {
    print("'{path}' is not a file.")
}
```

```
// Expected output:
This is the contents of data.txt.
```

Both camelCase (`fs.isFile`) and snake_case (`fs.is_file`) are supported. They are identical.

---

### `fs.is_dir(path)` / `fs.isDirectory(path)` → `boolean`

Returns `true` if the path exists and is a directory.

```zen
print(fs.is_dir("src/"))       // true
print(fs.is_dir("main.zen"))   // false
print(fs.is_dir("/tmp"))       // true
```

```
// Expected output:
true
false
true
```

---

### `fs.size(path)` → `integer`

Returns the size of the file in bytes.

```zen
// Check file size before reading
path := "large_log.txt"
bytes := fs.size(path)
print("File is {bytes} bytes")

if bytes > 1048576 {  // 1 MB
    print("That's a big file — consider filtering first.")
}
```

```
// Expected output:
File is 204847 bytes
```

```zen
// Small file
print(fs.size("README.md"))
```

```
// Expected output:
1024
```

---

### `fs.mtime(path)` → `integer`

Returns the last modification time of the file as a Unix timestamp (seconds since epoch).

```zen
// Get the modification time of a file
mod_time := fs.mtime("main.zen")
print("Last modified: {mod_time}")

// Convert to a more readable form is left to your own logic,
// but the raw timestamp is useful for comparisons.
now := time.now()
age := now - mod_time
print("File is {age} seconds old.")
```

```
// Expected output:
Last modified: 1692153600
File is 86400 seconds old.
```

---

## Writing Files

### `fs.write(path, data)` → `void`

Writes `data` (a string) to the file at `path`. If the file already exists, it is overwritten. If it doesn't exist, it is created. Parent directories are created automatically if they don't exist.

```zen
// Write a simple file
fs.write("hello.txt", "Hello, World!")
print(fs.read("hello.txt"))
```

```
// Expected output:
Hello, World!
```

```zen
// Overwrite an existing file
fs.write("hello.txt", "New content replaces old.")
print(fs.read("hello.txt"))
```

```
// Expected output:
New content replaces old.
```

```zen
// Creates parent directories automatically
fs.write("output/logs/run.log", "Log entry 1\nLog entry 2")
print(fs.read("output/logs/run.log"))
```

```
// Expected output:
Log entry 1
Log entry 2
```

**Key behavior:**
- Overwrites existing files entirely (no append — use `fs.append()` for that).
- Creates parent directories if they don't exist.
- Always works with strings. For binary data, use `fs.write_binary()`.

---

## Appending to Files

### `fs.append(path, data)` → `void`

Appends `data` to the end of the file at `path`. If the file doesn't exist, it is created.

```zen
// Build a log file entry by entry
fs.write("app.log", "")  // start fresh
fs.append("app.log", "[INFO] Application started\n")
fs.append("app.log", "[INFO] Listening on port 8080\n")
fs.append("app.log", "[WARN] High memory usage\n")
fs.append("app.log", "[INFO] Request handled: GET /api/status\n")

print(fs.read("app.log"))
```

```
// Expected output:
[INFO] Application started
[INFO] Listening on port 8080
[WARN] High memory usage
[INFO] Request handled: GET /api/status
```

```zen
// Append to an existing file without overwriting
existing := fs.read("notes.txt")
print("Before: {existing}")

fs.append("notes.txt", "\nMore notes appended.\n")
updated := fs.read("notes.txt")
print("After: {updated}")
```

```
// Expected output:
Before: Original line 1.
Original line 2.
After: Original line 1.
Original line 2.

More notes appended.
```

---

## Binary Read/Write

### `fs.read_binary(path)` / `fs.readBinary(path)` → `string`

Reads a binary file and returns its contents as a hex-encoded string. This is useful for inspecting binary formats, comparing files byte-by-byte, or processing non-text data.

```zen
// Read a binary file as hex
hex_data := fs.read_binary("image.png")
print(hex_data[:40])  // print just the first 20 bytes
```

```
// Expected output:
89504e47 0d0a1a0a 0000000d 49484452
```

```zen
// Compare the headers of two files
header_a := fs.read_binary("file1.bin")[:8]
header_b := fs.read_binary("file2.bin")[:8]
if header_a == header_b {
    print("Both files have the same magic bytes.")
} else {
    print("Files have different headers.")
}
```

```
// Expected output:
Both files have the same magic bytes.
```

---

### `fs.write_binary(path, data)` / `fs.writeBinary(path, data)` → `void`

Writes a hex-encoded string as binary data to the file at `path`. The hex string should contain pairs of hex characters (e.g., `"48656c6c6f"` for `"Hello"`).

```zen
// Write a binary file from hex data
fs.write_binary("output.bin", "48656c6c6f20576f726c64")
print(fs.read_binary("output.bin"))
```

```
// Expected output:
48656c6c6f20576f726c64
```

```zen
// Verify round-trip: hex → binary → hex
original := "deadbeef01234567"
fs.write_binary("test.bin", original)
readback := fs.read_binary("test.bin")
print(readback == original)  // true
```

```
// Expected output:
true
```

**Hex string format:**
- Pairs of hex digits: `"0a1b2c"`.
- Case-insensitive: `"DEADBEEF"` and `"deadbeef"` are equivalent.
- Spaces between pairs are typically preserved on read but should be stripped before passing to `write_binary`.

---

## File Metadata

### `fs.size(path)` → `integer`

Returns the file size in bytes.

```zen
print(fs.size("main.zen"))
```

```
// Expected output:
4096
```

```zen
// List all files in a directory with their sizes
files := fs.list("src/")
for file in files {
    full_path := fs.join("src", file)
    if fs.is_file(full_path) {
        print("{file}: {fs.size(full_path)} bytes")
    }
}
```

```
// Expected output:
main.zen: 1024 bytes
utils.zen: 2048 bytes
config.zen: 512 bytes
```

---

### `fs.mtime(path)` → `integer`

Returns the last modification time as a Unix timestamp.

```zen
// Check how old a file is
ts := fs.mtime("build/output")
now := time.now()
age_hours := (now - ts) / 3600
print("Build output is {age_hours} hours old.")
```

```
// Expected output:
Build output is 3 hours old.
```

---

## Directory Operations

### `fs.list(path)` → `list`

Returns a list of names (strings) in the given directory. Does not include `.` or `..`.

```zen
// List a directory
entries := fs.list(".")
for entry in entries {
    print(entry)
}
```

```
// Expected output:
main.zen
src/
README.md
config.json
tests/
```

```zen
// Filter for files only
entries := fs.list("src/")
for entry in entries {
    full_path := fs.join("src", entry)
    if fs.is_file(full_path) {
        print("FILE: {entry}")
    } else {
        print("DIR:  {entry}")
    }
}
```

```
// Expected output:
FILE: main.zen
FILE: utils.zen
DIR:  helpers/
```

---

### `fs.mkdir(path)` / `fs.mkdirs(path)` → `void`

Creates a directory at `path`. `fs.mkdirs` (or `fs.mkdir` with nested option) creates all intermediate directories as needed.

```zen
// Create a single directory
fs.mkdir("output")
print(fs.is_dir("output"))  // true
```

```
// Expected output:
true
```

```zen
// Create nested directories (like mkdir -p)
fs.mkdirs("build/release/bin")
print(fs.is_dir("build/release/bin"))  // true
```

```
// Expected output:
true
```

---

### `fs.rmdir(path)` → `void`

Removes an **empty** directory. If the directory contains files or subdirectories, an error is raised.

```zen
// Create and remove an empty directory
fs.mkdir("temp_dir")
print(fs.exists("temp_dir"))   // true

fs.rmdir("temp_dir")
print(fs.exists("temp_dir"))   // false
```

```
// Expected output:
true
false
```

```zen
// Trying to remove a non-empty directory
fs.mkdir("nonempty")
fs.write("nonempty/file.txt", "data")
fs.rmdir("nonempty")  // Error: directory not empty
```

---

### `fs.rmtree(path)` → `void`

Removes a directory and **all** of its contents recursively. Use with extreme caution.

```zen
// Remove an entire directory tree
fs.mkdirs("build/cache/temp")
fs.write("build/cache/temp/data.bin", "binary stuff")
fs.write("build/cache/meta.json", "{}")

fs.rmtree("build/cache")
print(fs.exists("build/cache"))  // false
```

```
// Expected output:
false
```

**Warning:** There is no undo. `fs.rmtree` permanently deletes everything under the path.

---

## File Operations

### `fs.copy(src, dst)` → `void`

Copies a file from `src` to `dst`. The destination path must be a file path (not a directory). Parent directories of `dst` are created automatically.

```zen
// Copy a file
fs.write("original.txt", "Important data")
fs.copy("original.txt", "backup.txt")
print(fs.read("backup.txt"))
```

```
// Expected output:
Important data
```

```zen
// Copy to a different directory
fs.mkdirs("backups")
fs.copy("data.json", "backups/data.json.bak")
print(fs.read("backups/data.json.bak"))
```

```
// Expected output:
{"key": "value"}
```

---

### `fs.move(src, dst)` / `fs.rename(src, dst)` → `void`

Moves or renames a file. If `dst` is in a different directory, the file is moved. If it's in the same directory, the file is renamed.

```zen
// Rename a file
fs.write("old_name.txt", "content")
fs.rename("old_name.txt", "new_name.txt")
print(fs.exists("old_name.txt"))  // false
print(fs.read("new_name.txt"))    // content
```

```
// Expected output:
false
content
```

```zen
// Move a file to another directory
fs.mkdirs("archive")
fs.move("new_name.txt", "archive/new_name.txt")
print(fs.exists("new_name.txt"))         // false
print(fs.is_file("archive/new_name.txt"))  // true
```

```
// Expected output:
false
true
```

---

### `fs.remove(path)` / `fs.unlink(path)` → `void`

Deletes a single file. The path must point to a file, not a directory.

```zen
// Delete a file
fs.write("temp.txt", "temporary data")
print(fs.exists("temp.txt"))  // true

fs.remove("temp.txt")
print(fs.exists("temp.txt"))  // false
```

```
// Expected output:
true
false
```

```zen
// Trying to remove a directory raises an error
fs.mkdir("a_dir")
fs.remove("a_dir")  // Error: use fs.rmdir or fs.rmtree for directories
```

---

## Path Utilities

### `fs.join(parts...)` → `string`

Joins path components into a single path string, handling separators automatically.

```zen
// Join path components
print(fs.join("usr", "local", "bin"))
```

```
// Expected output:
usr/local/bin
```

```zen
// Handles leading/trailing slashes
print(fs.join("/", "home", "user", "file.txt"))
print(fs.join("base/", "/nested/", "/deep"))
```

```
// Expected output:
/home/user/file.txt
base/nested/deep
```

```zen
// Build dynamic paths
user := "admin"
action := "export"
path := fs.join("data", user, "{action}.csv")
print(path)
```

```
// Expected output:
data/admin/export.csv
```

---

### `fs.basename(path)` → `string`

Extracts the filename (final component) from a path.

```zen
print(fs.basename("/home/user/file.txt"))  // file.txt
print(fs.basename("src/main.zen"))         // main.zen
print(fs.basename("archive.tar.gz"))       // archive.tar.gz
print(fs.basename("/just/a/dir/"))         // dir
print(fs.basename("no_slash"))             // no_slash
```

```
// Expected output:
file.txt
main.zen
archive.tar.gz
dir
no_slash
```

---

### `fs.dirname(path)` → `string`

Extracts the directory portion of a path (everything before the final component).

```zen
print(fs.dirname("/home/user/file.txt"))  // /home/user
print(fs.dirname("src/main.zen"))         // src
print(fs.dirname("file.txt"))             // .
print(fs.dirname("/a/b/c/"))             // /a/b/c
```

```
// Expected output:
/home/user
src
.
/a/b/c
```

---

### `fs.cwd()` → `string`

Returns the current working directory.

```zen
print(fs.cwd())
```

```
// Expected output:
/home/user/projects/my-app
```

---

### `fs.cd(path)` → `void`

Changes the current working directory. All subsequent relative path operations use this as the base.

```zen
// Change directory
print(fs.cwd())       // /home/user
fs.cd("/tmp")
print(fs.cwd())       // /tmp

// Relative paths now resolve from /tmp
fs.write("relative.txt", "written in /tmp")
print(fs.read("/tmp/relative.txt"))
```

```
// Expected output:
/home/user
/tmp
written in /tmp
```

**Note:** `fs.cd` changes the working directory for the entire process. Use it intentionally.

---

## Glob Pattern Matching

### `fs.glob(pattern)` → `list`

Returns a list of file paths matching the given glob pattern. Supports standard glob syntax.

```zen
// Find all .zen files
zen_files := fs.glob("*.zen")
for f in zen_files {
    print(f)
}
```

```
// Expected output:
main.zen
utils.zen
```

```zen
// Find all files recursively
all_z := fs.glob("**/*.zen")
for f in all_z {
    print(f)
}
```

```
// Expected output:
main.zen
src/utils.zen
src/helpers/format.zen
tests/main_test.zen
```

```zen
// Find files by extension
configs := fs.glob("**/*.json")
for c in configs {
    print(c)
}
```

```
// Expected output:
config.json
package.json
tests/fixtures/test_data.json
```

```zen
// Complex patterns
results := fs.glob("**/test_*.zen")
for r in results {
    print(r)
}
```

```
// Expected output:
tests/test_parser.zen
tests/test_runner.zen
src/helpers/test_utils.zen
```

**Glob syntax:**
- `*` — matches any sequence of characters (except `/`)
- `**` — matches any sequence of characters including `/` (recursive)
- `?` — matches a single character
- `[abc]` — matches any one of the enclosed characters
- `[a-z]` — matches any character in the range

---

## Common Patterns

### Reading and Parsing a Config File

```zen
// Load a config file with error handling
config_path := "app.json"

if !fs.exists(config_path) {
    print("No config file found. Using defaults.")
    // write a default config
    fs.write(config_path, '{"port": 3000, "debug": false}')
}

raw := fs.read(config_path)
config := json.parse(raw)
print("Port: {config.port}")
```

```
// Expected output (first run, creates file):
No config file found. Using defaults.
Port: 3000
```

```
// Expected output (subsequent runs):
Port: 3000
```

---

### Writing a Log File

```zen
// Simple log writer
fn log(level: string, message: string) {
    timestamp := time.now()
    entry := "[{timestamp}] [{level}] {message}\n"
    fs.append("app.log", entry)
}

log("INFO", "Server started on port 8080")
log("INFO", "Connected to database")
log("WARN", "Slow query detected (1.2s)")
log("ERROR", "Connection refused to cache server")

print(fs.read("app.log"))
```

```
// Expected output:
[1692153600] [INFO] Server started on port 8080
[1692153600] [INFO] Connected to database
[1692153600] [WARN] Slow query detected (1.2s)
[1692153600] [ERROR] Connection refused to cache server
```

---

### Walking a Directory Tree

```zen
// Recursively list all files with indentation by depth
fn walk(path: string, depth: int) {
    entries := fs.list(path)
    for entry in entries {
        full_path := fs.join(path, entry)
        indent := ""
        for i in 0..depth {
            indent = indent + "  "
        }
        if fs.is_dir(full_path) {
            print("{indent}{entry}/")
            walk(full_path, depth + 1)
        } else {
            print("{indent}{entry}  ({fs.size(full_path)} bytes)")
        }
    }
}

walk(".", 0)
```

```
// Expected output:
main.zen  (1024 bytes)
src/
  utils.zen  (2048 bytes)
  helpers/
    format.zen  (512 bytes)
tests/
  main_test.zen  (1536 bytes)
config.json  (256 bytes)
README.md  (1024 bytes)
```

---

### Collecting All Files of a Type

```zen
// Find all markdown files and report their sizes
md_files := fs.glob("**/*.md")
total := 0

for file in md_files {
    size := fs.size(file)
    total = total + size
    print("{file}: {size} bytes")
}

print("Total: {total} bytes across {len(md_files)} files")
```

```
// Expected output:
README.md: 1024 bytes
docs/guide.md: 3072 bytes
docs/api.md: 5120 bytes
CONTRIBUTING.md: 768 bytes
Total: 9984 bytes across 4 files
```

---

### Batch Renaming Files

```zen
// Rename all .txt files to .md
txt_files := fs.glob("*.txt")
for file in txt_files {
    // Extract just the filename without extension
    name := fs.basename(file)
    dir := fs.dirname(file)

    // Simple extension swap (manual string work)
    base := name[0:len(name)-4]  // strip ".txt"
    new_path := fs.join(dir, "{base}.md")

    fs.move(file, new_path)
    print("Renamed: {file} -> {new_path}")
}
```

```
// Expected output:
Renamed: notes.txt -> notes.md
Renamed: readme.txt -> readme.md
Renamed: todo.txt -> todo.md
```

---

### Backing Up Files Before Overwriting

```zen
// Safely update a file by backing up the original first
fn safe_write(path: string, data: string) {
    if fs.exists(path) {
        backup := "{path}.bak"
        fs.copy(path, backup)
        print("Backed up: {path} -> {backup}")
    }
    fs.write(path, data)
    print("Wrote: {path}")
}

safe_write("config.json", '{"version": 2, "setting": true}')
```

```
// Expected output (if config.json existed):
Backed up: config.json -> config.json.bak
Wrote: config.json
```

```
// Expected output (if config.json did not exist):
Wrote: config.json
```

---

### Checking File Freshness

```zen
// Determine if a cached file needs regeneration
cache_path := "cache/data.json"
source_path := "raw/data.csv"

if !fs.exists(cache_path) {
    print("Cache miss — generating data.json")
    // generate cache...
} else {
    source_mtime := fs.mtime(source_path)
    cache_mtime := fs.mtime(cache_path)

    if source_mtime > cache_mtime {
        print("Source is newer — regenerating cache")
    } else {
        print("Cache is up to date")
    }
}
```

```
// Expected output:
Cache is up to date
```

---

### Temporary Files with Cleanup

```zen
// Create a temp file, use it, then clean up
temp_path := fs.join(fs.cwd(), "temp_work.txt")
fs.write(temp_path, "intermediate processing data")

// ... do work with temp file ...

// Clean up
fs.remove(temp_path)
print(fs.exists(temp_path))  // false
```

```
// Expected output:
false
```

---

### Comparing Two Files

```zen
// Check if two files have identical contents
fn files_equal(a: string, b: string) -> bool {
    if !fs.exists(a) || !fs.exists(b) {
        return false
    }
    if fs.size(a) != fs.size(b) {
        return false
    }
    return fs.read(a) == fs.read(b)
}

print(files_equal("file_a.txt", "file_b.txt"))
```

```
// Expected output:
true
```

---

### Writing Structured Data

```zen
// Write a JSON file
data := {"name": "zen-app", "version": "1.0.0", "deps": ["json", "fs"]}
json_str := json.stringify(data, indent=2)
fs.write("package.json", json_str)
print(fs.read("package.json"))
```

```
// Expected output:
{
  "name": "zen-app",
  "version": "1.0.0",
  "deps": [
    "json",
    "fs"
  ]
}
```

---

### Reading CSV Data

```zen
// Read a CSV file and process rows
raw := fs.read("sales.csv")
lines := raw.split("\n")

for line in lines[1:] {  // skip header
    parts := line.split(",")
    if len(parts) >= 3 {
        product := parts[0]
        qty := parts[1]
        price := parts[2]
        print("{product}: {qty} units at ${price}")
    }
}
```

```
// Expected output (if CSV contains):
Widget: 10 units at $9.99
Gadget: 5 units at $24.50
Doohickey: 20 units at $3.25
```

---

## Error Handling

All `fs` functions raise errors when operations fail. Common error scenarios:

| Scenario | Error |
|---|---|
| File doesn't exist (read) | `file not found: <path>` |
| Permission denied | `permission denied: <path>` |
| Directory not empty (rmdir) | `directory not empty: <path>` |
| Is a directory (file op) | `is a directory: <path>` |
| Is a file (dir op) | `not a directory: <path>` |
| Disk full (write) | `no space left on device` |

Use `fs.exists()` as a guard before operations that may fail:

```zen
path := "maybe_exists.txt"
if fs.exists(path) {
    content := fs.read(path)
    // safe to use content
} else {
    print("File not found, skipping.")
}
```

---

## Quick Reference

| Function | Description |
|---|---|
| `fs.read(path)` | Read file to string |
| `fs.read_binary(path)` | Read file as hex string |
| `fs.write(path, data)` | Write string to file |
| `fs.append(path, data)` | Append string to file |
| `fs.write_binary(path, data)` | Write hex string as binary |
| `fs.exists(path)` | Check if path exists |
| `fs.is_file(path)` | Check if path is a file |
| `fs.is_dir(path)` | Check if path is a directory |
| `fs.size(path)` | Get file size in bytes |
| `fs.mtime(path)` | Get last modification time |
| `fs.list(path)` | List directory contents |
| `fs.mkdir(path)` | Create a directory |
| `fs.mkdirs(path)` | Create directories recursively |
| `fs.rmdir(path)` | Remove empty directory |
| `fs.rmtree(path)` | Remove directory and contents |
| `fs.copy(src, dst)` | Copy a file |
| `fs.move(src, dst)` | Move/rename a file |
| `fs.remove(path)` | Delete a file |
| `fs.glob(pattern)` | Find files matching pattern |
| `fs.join(parts...)` | Join path components |
| `fs.basename(path)` | Extract filename from path |
| `fs.dirname(path)` | Extract directory from path |
| `fs.cwd()` | Get current working directory |
| `fs.cd(path)` | Change current working directory |
