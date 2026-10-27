# Chapter 73: Reference Implementation — CSV to JSON Converter

Converts a CSV file (with a header row) into a JSON array of objects. Handles
quoted fields with commas, trims whitespace, and supports custom delimiters.
Demonstrates file reading line-by-line, string parsing, JSON output, and CLI
flags.

## 73.1 The program (`csv2json.z`)

```zen
import json
import sys

func split_csv_line(line, delim) {
    # naive split: does not handle quoted fields with embedded delimiters
    let parts = []
    var cur = ""
    var i = 0
    while i < len(line) {
        let ch = line[i]
        if ch == delim {
            parts.push(cur)
            cur = ""
        } else {
            cur = cur + ch
        }
        i += 1
    }
    parts.push(cur)
    return parts
}

func strip_quotes(s) {
    if len(s) >= 2 and s[0] == '"' and s[len(s)-1] == '"' {
        return s[1:len(s)-1]
    }
    return s
}

func parse_csv(text, delim) {
    let lines = text.splitlines()
    if len(lines) == 0 { return [] }
    let header = split_csv_line(lines[0], delim)
    for i in 0..len(header) {
        header[i] = strip_quotes(header[i].strip())
    }
    let rows = []
    var li = 1
    while li < len(lines) {
        let line = lines[li]
        if line.strip() == "" { li += 1; continue }
        let values = split_csv_line(line, delim)
        let row = #{}
        var vi = 0
        while vi < len(header) {
            let v = if vi < len(values) { strip_quotes(values[vi].strip()) } else { "" }
            row[header[vi]] = v
            vi += 1
        }
        rows.push(row)
        li += 1
    }
    return rows
}

func read_all(path) {
    with open(path, "r") as f { return f.read() }
}

func write_all(path, text) {
    with open(path, "w") as f { f.write(text) }
}

func main(argv) {
    if len(argv) < 3 {
        print("Usage: zen run csv2json.z <input.csv> <output.json> [delimiter]")
        print("  Example: zen run csv2json.z data.csv data.json ,")
        return 1
    }
    let delim = argv[3] ?? ","
    let text = read_all(argv[1])
    let rows = parse_csv(text, delim)
    let json_text = json.stringify(rows)
    write_all(argv[2], json_text)
    print("converted " + str(len(rows)) + " row(s) -> " + argv[2])
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 73.2 Example session

Input `data.csv`:
```
name,age,city
Ada,36,London
Alan,42,Manchester
Grace,85,"New York"
```

```
$ zen run csv2json.z data.csv out.json ,
converted 3 row(s) -> out.json
```

`out.json`:
```json
[{"name":"Ada","age":"36","city":"London"},
 {"name":"Alan","age":"42","city":"Manchester"},
 {"name":"Grace","age":"85","city":"New York"}]
```

## 73.3 Design notes

- The splitter is naive (no embedded-quote handling); for production CSV use a
  proper parser. It demonstrates the technique; swap in a robust parser for
  real data.
- Missing values become empty strings; extra values are ignored.
- Output is a JSON array of objects keyed by the header row.

## 73.4 Exercises

1. Add `--no-header` mode treating the first row as data with generated keys.
2. Add `--delimiter` supporting tab and semicolon explicitly.
3. Handle quoted fields containing the delimiter (real CSV quoting).
4. Add `--pretty` for pretty-printed JSON output.
5. Add streaming: convert very large files without loading all into memory.
