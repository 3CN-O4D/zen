# Chapter 43: CLI Tools

Zen is excellent for building command-line applications. This chapter shows how to parse arguments, read input, write output, and build useful CLI tools.

## 43.1 Argument parsing: `argparser`

```zen
import argparser

let parser = argparser.ArgumentParser("mycli")
parser.add_argument("input", help="input file")
parser.add_argument("--output", "-o", help="output file")
parser.add_argument("--verbose", "-v", action="store_true", help="verbose mode")

let args = parser.parse_args()

print(args.input)
print(args.output)
print(args.verbose)
```

### ArgumentParser methods

| Method | Description |
|--------|-------------|
| `ArgumentParser(prog?)` | Create a parser |
| `add_argument(name, opts?)` | Add an argument |
| `parse_args()` | Parse `sys.argv` |

### Argument options

| Option | Description |
|--------|-------------|
| `help` | Help text |
| `action` | `"store_true"`, `"store_false"`, `"count"` |
| `default` | Default value |
| `type` | `"int"`, `"float"`, `"str"` |
| `choices` | Allowed values |
| `required` | Boolean |

### Example

```zen
import argparser

let parser = argparser.ArgumentParser("wc")
parser.add_argument("file", help="file to count")
parser.add_argument("--lines", "-l", action="store_true", help="count lines")
parser.add_argument("--words", "-w", action="store_true", help="count words")
parser.add_argument("--bytes", "-c", action="store_true", help="count bytes")

let args = parser.parse_args()

let content = fs.read(args.file)
let lines = len(content.split("\n"))
let words = len(content.split())
let bytes = len(content)

if args.lines {
    print(lines)
}
if args.words {
    print(words)
}
if args.bytes {
    print(bytes)
}
```

## 43.2 Reading stdin

```zen
import fs

// Read all stdin
let input = fs.read("/dev/stdin")
print(input)

// Or line by line
let lines = input.split("\n")
for line in lines {
    print(line)
}
```

## 43.3 Writing output

```zen
import fs

// stdout
print("hello")

// stderr
fs.write("/dev/stderr", "error message\n")

// To a file
fs.write("output.txt", "result")
```

## 43.4 Exit codes

```zen
import sys

// Success
exit(0)

// Failure
exit(1)

// Specific error
exit(2)
```

`exit(code)` terminates the process with the given code.

## 43.5 Complete example: a `cat` clone

```zen
import fs
import sys
import argparser

let parser = argparser.ArgumentParser("zcat")
parser.add_argument("files", nargs="*")
let args = parser.parse_args()

let files = args.files
if len(files) == 0 {
    // Read stdin
    print(fs.read("/dev/stdin"))
} else {
    for f in files {
        try {
            print(fs.read(f))
        } catch as e {
            fs.write("/dev/stderr", "zcat: " + f + ": " + e.msg + "\n")
            exit(1)
        }
    }
}
```

## 43.6 Complete example: a `grep` clone

```zen
import fs
import argparser
import re

let parser = argparser.ArgumentParser("zgrep")
parser.add_argument("pattern", help="search pattern")
parser.add_argument("file", help="file to search")
parser.add_argument("-i", action="store_true", help="ignore case")
let args = parser.parse_args()

let pattern = args.pattern
let flags = args.i ? "i" : ""

let content = fs.read(args.file)
let lines = content.split("\n")

for (i, line) in enumerate(lines) {
    if re.search(pattern, line, flags) != nil {
        print(str(i + 1) + ": " + line)
    }
}
```

## 43.7 Complete example: a JSON pretty-printer

```zen
import fs
import json
import argparser

let parser = argparser.ArgumentParser("zjson")
parser.add_argument("file", help="JSON file")
parser.add_argument("--indent", "-i", type="int", default=2, help="indent spaces")
let args = parser.parse_args()

let text = fs.read(args.file)
let data = json.loads(text)

// Zen's json.dumps doesn't support indent in this version
// Output compact JSON
print(json.dumps(data))
```

## 43.8 Making scripts executable

```zen
#!/usr/bin/env zen
print("Hello from a script!")
```

```bash
chmod +x mycli.z
./mycli.z
```

## 43.9 Common pitfalls

- **`argparser` reads from `sys.argv`.** Don't call `parse_args` twice.
- **`nargs="*"`** collects remaining positional args into a list.
- **`exit(code)` terminates immediately** — use `try`/`finally` for cleanup before exiting if needed.
- **`fs.read("/dev/stdin")`** reads all stdin at once; for large input, read in chunks.

## 43.10 Quick reference

```zen
import argparser
import fs
import sys
import exit

let parser = argparser.ArgumentParser("prog")
parser.add_argument("pos", help="positional")
parser.add_argument("--opt", "-o", help="optional")
parser.add_argument("--flag", "-f", action="store_true")
let args = parser.parse_args()

print(args.pos)
print(args.opt)
print(args.flag)

fs.write("/dev/stderr", "error\n")
exit(0)
```

## 43.11 Exercises

1. Write a `wc` clone that counts lines, words, and bytes.
2. Write a `head` clone that prints the first N lines.
3. Write a `cp` clone that copies one file to another.
4. Write a `find` clone that lists files matching a pattern.
5. Write a CLI config tool that reads/writes a JSON config file.


## Extended Project Ideas and Implementation Notes

### Design checklist
- Requirements and scope
- Error handling
- Logging
- Configuration
- Testing
- Documentation
- Security considerations

### Exercises
1. Extend the project with new features
2. Add tests
3. Improve error messages
4. Add help/usage
5. Make it configurable
6. Package for distribution


## 43.12 Full tutorial — building a CLI todo manager

### Requirements
- Add, list, mark done, delete tasks
- Store tasks in a JSON file
- Accept subcommands: `add`, `list`, `done`, `delete`
- Print usage when no args or `--help`

### Implementation (`todo.z`)

```zen
import sys
import json
import os
import pathlib

let TASKS_FILE = pathlib.Path("tasks.json")

func load_tasks() {
    if !TASKS_FILE.exists() { return [] }
    try {
        return json.load(str(TASKS_FILE))
    } catch as e {
        print("warn: could not load tasks: " + e.msg)
        return []
    }
}

func save_tasks(tasks) {
    json.save(str(TASKS_FILE), tasks)
}

func cmd_add(args) {
    if len(args) < 1 { print("usage: todo add <task>"); return }
    let tasks = load_tasks()
    let task = #{id: len(tasks) + 1, title: args[0], done: false}
    tasks.push(task)
    save_tasks(tasks)
    print("added: " + args[0])
}

func cmd_list(_) {
    let tasks = load_tasks()
    if len(tasks) == 0 { print("no tasks"); return }
    for t in tasks {
        let mark = if t["done"] { "[x]" } else { "[ ]" }
        print("${mark} ${t["id"]}. ${t["title"]}")
    }
}

func cmd_done(args) {
    if len(args) < 1 { print("usage: todo done <id>"); return }
    let id = int(args[0])
    let tasks = load_tasks()
    var found = false
    for t in tasks {
        if t["id"] == id { t["done"] = true; found = true }
    }
    if found { save_tasks(tasks); print("marked ${id} done") }
    else { print("no task with id ${id}") }
}

func cmd_delete(args) {
    if len(args) < 1 { print("usage: todo delete <id>"); return }
    let id = int(args[0])
    let tasks = load_tasks()
    let filtered = []
    for t in tasks { if t["id"] != id { filtered.push(t) } }
    save_tasks(filtered)
    print("deleted ${id}")
}

func main(argv) {
    if len(argv) < 2 {
        print("Todo Manager")
        print("usage: todo <add|list|done|delete> [args]")
        return 0
    }
    let cmd = argv[1]
    let rest = []
    for i in 2..len(argv) { rest.push(argv[i]) }
    if cmd == "add" { cmd_add(rest) }
    elif cmd == "list" { cmd_list(rest) }
    elif cmd == "done" { cmd_done(rest) }
    elif cmd == "delete" { cmd_delete(rest) }
    elif cmd == "--help" || cmd == "help" {
        print("usage: todo <add|list|done|delete> [args]")
    } else {
        print("unknown command: " + cmd)
    }
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

### Sample session

```bash
$ zen run todo.z add "Learn Zen"
added: Learn Zen
$ zen run todo.z add "Build a CLI"
added: Build a CLI
$ zen run todo.z list
[ ] 1. Learn Zen
[ ] 2. Build a CLI
$ zen run todo.z done 1
marked 1 done
$ zen run todo.z list
[x] 1. Learn Zen
[ ] 2. Build a CLI
$ zen run todo.z delete 2
deleted 2
```

### Design notes
- Tasks are dicts in a list, serialized to JSON.
- IDs are simple (len+1); not robust to deletion gaps but fine for a demo.
- `save_tasks` rewrites the whole file each time (simple, safe).
- Errors are caught so a corrupt JSON file doesn't crash the app.

## 43.13 CLI design checklist

1. **Parse args** from `sys.argv` (first element is the program).
2. **Dispatch** on subcommand.
3. **Validate** argument count and types before use.
4. **Print usage** on `--help`, missing args, or unknown command.
5. **Return exit codes:** 0 success, non-zero on error.
6. **Persist** state to a file (JSON/text) if stateful.
7. **Handle errors** gracefully (missing file, bad input).
8. **Document** each subcommand.

## 43.14 Exercises

1. Add a `priority` field to tasks and sort `list` by priority.
2. Add `clear` command to remove all tasks.
3. Add `--file` option to choose a different storage path.
4. Add `edit <id> <new title>` command.
5. Validate that `done`/`delete` IDs exist before operating.
6. Add timestamps (`created_at`, `completed_at`) to tasks.
7. Print overdue tasks (conceptually, with due dates).
8. Write `todo count` showing number of pending/done.
9. Add tab-completion hints (usage text only).
10. Package the CLI with `zen build todo.z -o todo`.

