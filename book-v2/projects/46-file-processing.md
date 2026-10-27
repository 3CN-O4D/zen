# Chapter 46: File Processing Tools

This chapter shows how to process files in Zen: reading/writing text and binary, CSV, JSON, line-by-line processing, temporary files, and directory walking.

## 46.1 Reading text files

```zen
import fs

// Read entire file
let content = fs.read("data.txt")
print(content)

// Read line by line
let lines = fs.read("data.txt").split("\n")
for line in lines {
    if len(line) > 0 {
        print(line)
    }
}
```

## 46.2 Writing text files

```zen
import fs

// Write entire file (overwrites)
fs.write("output.txt", "Line 1\nLine 2\nLine 3\n")

// Append (read-modify-write)
let existing = ""
if fs.exists("log.txt") {
    existing = fs.read("log.txt")
}
fs.write("log.txt", existing + "new entry\n")
```

## 46.3 Binary files

```zen
import fs

// Read binary
let data = fs.read_binary("image.png")
print(len(data))   // byte count

// Write binary
fs.write_binary("copy.png", data)

// Copy a file
fs.copy("source.txt", "destination.txt")
```

## 46.4 CSV processing (manual)

```zen
import fs

func read_csv(path) {
    let lines = fs.read(path).split("\n")
    if len(lines) == 0 {
        return []
    }
    let header = lines[0].split(",")
    let rows = []
    for i in 1..len(lines) {
        if lines[i] == "" {
            continue
        }
        let values = lines[i].split(",")
        let row = {}
        for j in 0..len(header) {
            row[header[j]] = values[j]
        }
        rows.push(row)
    }
    return rows
}

let data = read_csv("data.csv")
for row in data {
    print(row["name"])
}
```

## 46.5 JSON files

```zen
import fs
import json

// Read JSON
let text = fs.read("config.json")
let config = json.loads(text)
print(config["theme"])

// Write JSON
let data = {"name": "Zen", "version": 3}
fs.write("config.json", json.dumps(data))
```

## 46.6 Line processing pipeline

```zen
import fs

func process_lines(path, func) {
    let lines = fs.read(path).split("\n")
    let results = []
    for line in lines {
        if len(line) > 0 {
            results.push(func(line))
        }
    }
    return results
}

let upper_lines = process_lines("data.txt", fn(line) {
    return line.upper()
})
for line in upper_lines {
    print(line)
}
```

## 46.7 Temporary files

```zen
import tempfile
import fs

let tmp = tempfile.mktemp()
fs.write(tmp, "temporary data")
let content = fs.read(tmp)
fs.remove(tmp)

// Or use a directory
let dir = tempfile.mkdtemp()
// ... use dir ...
fs.rmtree(dir)
```

## 46.8 Directory walking

```zen
import fs

func walk(dir, func) {
    for name in fs.list(dir) {
        let path = fs.join(dir, name)
        if fs.is_dir(path) {
            walk(path, func)
        } else {
            func(path)
        }
    }
}

walk(".", fn(path) {
    print(path)
})
```

## 46.9 File info

```zen
import fs

print(fs.exists("file.txt"))      # true/false
print(fs.is_file("file.txt"))     # true/false
print(fs.is_dir("dir"))           # true/false
print(fs.size("file.txt"))        // byte count
print(fs.join("a", "b", "c"))     // a/b/c
print(fs.basename("/a/b/c.txt"))  // c.txt
print(fs.dirname("/a/b/c.txt"))   // /a/b
```

## 46.10 Quick reference

```zen
import fs
import json
import tempfile

fs.read("file.txt")           # string
fs.read_binary("file.bin")    # bytes
fs.write("file.txt", "data")
fs.write_binary("file.bin", bytes)
fs.exists("file.txt")
fs.is_file("file.txt")
fs.is_dir("dir")
fs.size("file.txt")
fs.mkdirs("a/b/c")
fs.remove("file.txt")
fs.rmtree("dir")
fs.copy("src", "dst")
fs.move("old", "new")
fs.join("a", "b")
fs.basename("/a/b/c.txt")
fs.dirname("/a/b/c.txt")
fs.list(".")                  # directory entries

let tmp = tempfile.mktemp()
fs.write(tmp, "data")
fs.remove(tmp)

let dir = tempfile.mkdtemp()
fs.rmtree(dir)
```

## 46.11 Exercises

1. Write `file_size_human(path)` that returns a human-readable file size.
2. Write a script that copies all `.txt` files from one directory to another.
3. Write `grep_file(pattern, path)` that prints lines containing `pattern`.
4. Write a file tailer that reads the last `n` lines of a file.
5. Write `csv_to_json(path)` that converts a CSV file to JSON.
6. Write `count_lines(path)` that counts non-empty lines in a file.
7. Write `merge_files(paths, output)` that concatenates multiple files into one.


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


## 46.12 Full tutorial — file processing pipeline

### Goal
Read a CSV-like file, transform each row, write results to another file.

### Implementation (`process.z`)

```zen
import os
import pathlib

func read_lines(path) {
    let lines = []
    with open(path, "r") as f {
        for line in f {
            lines.push(line)
        }
    }
    return lines
}

func parse_csv_line(line) {
    return line.strip().split(",")
}

func transform(row) {
    # row: ["name", "age", "city"]
    if len(row) < 3 { return nil }
    return #{name: row[0], age: int(row[1]), city: row[2]}
}

func format_record(r) {
    return r["name"] + "," + str(r["age"]) + "," + r["city"] + "\n"
}

func process(input_path, output_path) {
    let lines = read_lines(input_path)
    let results = []
    for line in lines {
        let row = parse_csv_line(line)
        let r = transform(row)
        if r != nil { results.push(format_record(r)) }
    }
    with open(output_path, "w") as f {
        for r in results { f.write(r) }
    }
    print("processed " + str(len(results)) + " records -> " + output_path)
}

func main(argv) {
    if len(argv) < 3 {
        print("usage: process <input.csv> <output.csv>")
        return 1
    }
    process(argv[1], argv[2])
    return 0
}

if __name__ == "__main__" { sys.exit(main(sys.argv)) }
```

### Sample data (`input.csv`)

```
Ada,36,London
Alan,42,Manchester
Grace,85,New York
```

### Sample run

```bash
$ zen run process.z input.csv output.csv
processed 3 records -> output.csv
```

### Design notes
- `with` ensures the file closes even on error.
- `transform` returns `nil` on malformed rows; the pipeline skips them.
- Output written only after all rows transform successfully (simple).

## 46.13 File processing patterns

| Pattern | Use |
|---------|-----|
| Read → transform → write | ETL pipeline |
| Stream line-by-line | large files (low memory) |
| Chunked binary copy | large binary files |
| Filter + aggregate | reports/summaries |
| Parse-validate-load | data import |

## 46.14 Exercises

1. Add a header row detection (skip first line).
2. Support a delimiter flag (`,`, `\t`, `;`).
3. Filter rows where `age > 40`.
4. Sort output by age.
5. Write a summary line: total, min, max, average age.
6. Handle missing input file gracefully.
7. Write output incrementally (streaming) instead of collecting all.
8. Support `.tsv` input automatically by extension.
9. Add a `--dry-run` flag that prints without writing.
10. Process all `.csv` files in a directory.

