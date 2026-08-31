# Scripts

A complete guide to writing, running, and debugging Zen script files — including arguments, environment variables, shebangs, cross-platform patterns, and debugging techniques.

## File Extension

Zen scripts use the `.z` extension by convention:

```
myscript.z
```

You can use other extensions, but `.z` is recognized automatically by the CLI.

---

## Running Scripts

### Basic execution

```bash
zen run script.z
```

### With CLI flags

```bash
zen run script.z --headful
zen run script.z --no-headless
```

### Inline evaluation with `-e`

Execute code directly from the command line without a file:

```bash
zen -e 'print "Hello from Zen!"'
# Output: Hello from Zen!
```

Multiple `-e` flags execute in sequence:

```bash
zen -e 'let x = 10' -e 'print x * 2'
# Output: 20
```

Multi-line inline code:

```bash
zen -e '
    for i in 1 -> 5 {
        print "Step {i}"
    }
'
```

---

## Script Structure

A well-organized Zen script:

```
// ─── 1. Imports ──────────────────────────────────────
import utils

// ─── 2. Configuration ────────────────────────────────
const BASE_URL = "https://api.example.com"
const API_KEY = os.env("API_KEY") ?? "default_key"
const MAX_RETRIES = 3

// ─── 3. Helper Functions ─────────────────────────────
function fetch_data(endpoint) {
    let url = BASE_URL + endpoint
    let resp = http.get(url, headers={
        "Authorization": "Bearer " + API_KEY
    })

    if !resp.ok {
        throw "HTTP {resp.status}: {endpoint}"
    }

    return resp.json()
}

function retry(fn, max_attempts) {
    let attempt = 0
    while attempt < max_attempts {
        attempt = attempt + 1
        try {
            return fn()
        } catch err {
            print "Attempt {attempt} failed: {err}"
            if attempt >= max_attempts {
                throw err
            }
            sleep(1)
        }
    }
}

// ─── 4. Main Logic ───────────────────────────────────
function main() {
    let data = retry(() => fetch_data("/users"), MAX_RETRIES)
    print "Fetched {data.len} users"

    // Process and save
    let output = []
    for user in data {
        output.append({
            "name": user["name"],
            "email": user["email"]
        })
    }

    fs.mkdirs("output")
    json.save("output/users.json", output)
    print "Saved to output/users.json"
}

// ─── 5. Entry Point ──────────────────────────────────
main()
```

---

## Arguments and Environment Variables

### Accessing command-line arguments

```
// script.z
let args = os.args()
print "Script name: {args[0]}"
print "Arguments: {args[1:]}"
```

```bash
zen run script.z hello world
# Script name: script.z
# Arguments: [hello, world]
```

### Accessing environment variables

```
// Use os.env() for individual variables
let home = os.env("HOME")
let api_key = os.env("API_KEY") ?? "no-key-set"

print "Home: {home}"
print "API Key: {api_key}"
```

```bash
API_KEY=abc123 zen run script.z
```

### Setting environment variables

```
os.setenv("MY_VAR", "hello")
print os.env("MY_VAR")    // hello
```

### Platform detection

```
print os.name             // linux, macos, windows
print os.platform()       // linux, darwin, windows
print os.arch()           // x86_64, aarch64, etc.
print os.hostname()       // my-machine
print os.pid()            // process ID
print os.cpu_count()      // number of CPU cores
```

---

## Shebangs

Make your script executable directly from the shell using a shebang line.

### Linux / macOS

```
#!/usr/bin/env zen

print "Hello from executable script!"
print "Args: {os.args()}"
```

```bash
chmod +x script.z
./script.z
```

### How it works

The shebang `#!/usr/bin/env zen` tells the OS to use `env` to find `zen` in your `PATH`, then pass the script file to it. This is more portable than `#!/path/to/zen` because it doesn't depend on where zen is installed.

### Windows

Shebangs don't work natively on Windows. Instead:

```powershell
zen script.z
```

Or create a `.bat` wrapper:

```bat
@echo off
zen "%~dp0script.z" %*
```

---

## Including Files

Use `include` to load code from other files. The included code is executed in the current scope:

### Simple include

```
// utils.z
function greet(name) {
    return "Hello, " + name + "!"
}

function factorial(n) {
    if n <= 1 { return 1 }
    return n * factorial(n - 1)
}
```

```
// main.z
include "utils.z"

print greet("World")     // Hello, World!
print factorial(5)       // 120
```

### Include with relative paths

```
// Project structure:
// project/
// ├── main.z
// ├── lib/
// │   ├── utils.z
// │   └── math.z
// └── config/
//     └── defaults.z

// main.z
include "lib/utils.z"
include "lib/math.z"
include "config/defaults.z"
```

### Import for namespacing

For better organization, use `import` instead of `include`:

```
// lib/utils.z
function greet(name) {
    return "Hello, " + name + "!"
}

function slugify(text) {
    return text.lower().replace(" ", "-")
}
```

```
// main.z
import utils

print utils.greet("World")     // Hello, World!
print utils.slugify("Hello World")   // hello-world
```

Or import specific items:

```
from utils import greet, slugify

print greet("World")
print slugify("Hello World")
```

---

## Comments

### Single-line comments

```
// This is a comment
let x = 5  // inline comment
```

### Hash comments

```
# This is also a comment
```

### Block comments

```
/*
  This is a multi-line
  comment block.
*/
```

---

## Debugging

### Print debugging

```
function compute(x) {
    print "DEBUG: compute called with x={x}"
    let result = x * 2 + 1
    print "DEBUG: result={result}"
    return result
}

compute(5)
// DEBUG: compute called with x=5
// DEBUG: result=11
```

### Using the `.vars` shell command

In the REPL, use `.vars` to inspect all variables:

```
zen ❯ let x = 42
zen ❯ let name = "test"
zen ❯ .vars
x: 42
name: test
```

### Using `.type` to inspect values

```
zen ❯ .type [1, 2, 3]
list

zen ❯ .type {"a": 1}
dict

zen ❯ .type (x) => x * 2
function
```

### Error tracing

Zen provides detailed error messages with file, line, and column:

```
error[undefined variable: foo]
 --> script.z:5:5
  |
5 | print foo
        ^
  |
  = error: undefined variable: foo
```

### Try/catch for runtime debugging

```
function risky_operation() {
    // something that might fail
    let data = json.parse("not json")
}

try {
    risky_operation()
} catch err {
    print "ERROR: {err}"
    print "Stack: the error includes file and line info"
}
```

---

## Cross-Platform Scripts

### Path handling

Don't hardcode path separators — use `fs.join()`:

```
// WRONG — breaks on Windows
let path = "data" + "/" + "output" + "/" + "result.txt"

// CORRECT — works everywhere
let path = fs.join("data", "output", "result.txt")
```

### Platform-specific logic

```
let platform = os.name

if platform == "windows" {
    print "Running on Windows"
    // Windows-specific code
} elif platform == "macos" {
    print "Running on macOS"
    // macOS-specific code
} else {
    print "Running on Linux/other"
    // Linux-specific code
}
```

### Portable paths

```
// Get the user's home directory
let home = os.home()

// Get temp directory
let temp = fs.join(home, "temp")
fs.mkdirs(temp)
```

---

## Real-World Example: File Organizer

A script that organizes files by extension:

```
#!/usr/bin/env zen
// organize.z — Organize files in a directory by extension

function main() {
    let target_dir = os.args()[1] ?? "."

    if !fs.exists(target_dir) {
        print "Directory not found: {target_dir}"
        exit 1
    }

    if !fs.is_dir(target_dir) {
        print "Not a directory: {target_dir}"
        exit 1
    }

    let files = fs.list(target_dir)
    let organized = {}

    for filename in files {
        let full_path = fs.join(target_dir, filename)

        // Skip directories
        if fs.is_dir(full_path) {
            continue
        }

        // Get file extension
        let parts = filename.split(".")
        let ext = if parts.len > 1 { parts[-1] } else { "no_extension" }

        // Create category directory
        let dest_dir = fs.join(target_dir, ext)
        if !fs.exists(dest_dir) {
            fs.mkdirs(dest_dir)
            print "Created directory: {dest_dir}"
        }

        // Move file
        let dest_path = fs.join(dest_dir, filename)
        fs.move(full_path, dest_path)
        print "  {filename} -> {dest_dir}/"

        // Track stats
        organized[ext] = (organized[ext] ?? 0) + 1
    }

    // Print summary
    print "\n=== Summary ==="
    for ext in organized {
        print "  .{ext}: {organized[ext]} files"
    }
}

main()
```

Usage:

```bash
chmod +x organize.z
./organize.z ~/Downloads
```

---

## Pro Tips

1. **Use `zen -e` for quick tests.** No need to create a file for one-liners.
2. **Start with the REPL.** Experiment in `zen shell`, then save working code to a `.z` file.
3. **Use `include` for shared code.** Put utility functions in separate files and include them.
4. **Check `os.env()` for secrets.** Never hardcode API keys or passwords in scripts.
5. **Use `fs.join()` for paths.** Never concatenate path strings with `/`.
6. **Add a shebang for executable scripts.** `#!/usr/bin/env zen` makes scripts directly runnable.
7. **Use `try/catch` around risky operations.** Network calls, file I/O, and JSON parsing can fail.

---

## Common Mistakes

### Using `return` at the top level

`return` can only be used inside functions:

```
// WRONG
return 0

// CORRECT
function main() {
    // ...
    return 0
}
main()
```

### Forgetting that `for` requires a list

```
// WRONG — strings are not iterable with for-in
for char in "hello" { print char }

// CORRECT — use range or split
for i in 0 -> 4 { print "hello"[i] }
// or
for char in "hello".split("") { print char }
```

### Shadowing built-in functions

```
// BAD — this shadows the built-in len() function
let len = 10

// BETTER — use a different name
let length = 10
```

---

## See Also

- [Quick Start](quickstart.md) — Beginner-friendly overview
- [Shell Usage](shell.md) — Interactive REPL reference
- [Language Reference](../index.md) — Complete language documentation
- [CLI Reference](../cli.md) — All CLI commands and options
