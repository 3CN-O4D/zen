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
