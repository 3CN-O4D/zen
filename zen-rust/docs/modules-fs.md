# File System Module (`fs`)

The `fs` module provides a comprehensive suite of utilities for manipulating files, directories, paths, and metadata. It is built natively into the interpreter, offering fast execution with no external library dependencies.

## Path & Directory Queries

### Get Current Working Directory
```zen
let current = fs.cwd()
print current
```

### Change Working Directory
```zen
fs.cd("/tmp")
print fs.cwd()  // "/tmp"
```

### List Directory Contents
Returns a list of filenames or subdirectory names within the specified directory.
```zen
let files = fs.list(".")
for file in files {
    print file
}
```

---

## File Reading & Writing

### Read Entire Text File
Reads the specified file and returns its content as a UTF-8 string.
```zen
let content = fs.read("data.txt")
print content
```

### Write Text File
Creates a new file or overwrites an existing file with the provided text content. Returns `true` on success.
```zen
fs.write("output.txt", "Hello, Zen World!")
```

### Append to Text File
Appends text to the end of a file. If the file does not exist, it is created.
```zen
fs.append("log.txt", "New event occurred\n")
```

### Read Binary File
Reads the specified file and returns its raw bytes as a base64 encoded string.
```zen
let base64_data = fs.read_binary("image.png")
# Alias: fs.readBinary
```

### Write Binary File
Decodes a base64 encoded string and writes the raw binary bytes to the specified path.
```zen
fs.write_binary("copy.png", base64_data)
# Alias: fs.writeBinary
```

---

## Metadata & Existence Checks

### Check Path Existence
Returns `true` if a file or directory exists at the specified path, otherwise `false`.
```zen
if fs.exists("config.json") {
    print "Config found!"
}
```

### Check Type (File or Directory)
```zen
if fs.is_file("config.json") {
    print "It is a file."
}
if fs.is_dir("src") {
    print "It is a directory."
}
# Aliases: fs.isFile, fs.isDir
```

### Get File Size
Returns the size of the file in bytes.
```zen
let num_bytes = fs.size("video.mp4")
print num_bytes
```

### Get Modified Time (mtime)
Returns the last modification time of the file as a UNIX timestamp (floating-point number of seconds).
```zen
let last_modified = fs.mtime("notes.md")
print last_modified
```

---

## File & Directory Operations

### Create Directory (mkdir / mkdirs)
Creates the directory. If nested subdirectories do not exist, they are recursively created.
```zen
fs.mkdir("logs/2026/08")
# Alias: fs.mkdirs
```

### Delete File
Deletes the file at the specified path.
```zen
fs.remove("temp.tmp")
```

### Delete Empty Directory
```zen
fs.rmdir("empty_folder")
```

### Delete Directory Recursively (rmtree)
Deletes a directory and all of its nested contents. Use with caution!
```zen
fs.rmtree("cache_dir")
```

### Copy File
Copies a file from the source path to the destination path.
```zen
fs.copy("original.txt", "backup.txt")
```

### Move or Rename
Moves a file/directory or renames it.
```zen
fs.move("old_name.txt", "new_name.txt")
# Alias: fs.rename
```

---

## Path Manipulations & Globbing

### Path Joining
Joins multiple path segments into a single valid path string using the platform's directory separator.
```zen
let path = fs.join("src", "modules", "core.z")
print path  // "src/modules/core.z" on Unix, "src\\modules\\core.z" on Windows
```

### Get Basename
Returns the final portion of a path.
```zen
let file = fs.basename("/var/log/nginx/access.log")
print file  // "access.log"
```

### Get Directory Name
Returns the parent directory portion of a path.
```zen
let parent = fs.dirname("/var/log/nginx/access.log")
print parent  // "/var/log/nginx"
```

### Glob File Matching
Returns a list of path names matching a specified shell pattern (e.g., `*.z`, `**/*.z`).
```zen
let scripts = fs.glob("std/*.z")
for script in scripts {
    print script
}
```
