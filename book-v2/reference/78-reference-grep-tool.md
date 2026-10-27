# Chapter 78: Reference Implementation — Grep Tool (Pattern Search)

A `grep`-like tool that searches files for a regex pattern and prints matching
lines with line numbers. Supports multiple files, case-insensitive mode, and
invert-match. Demonstrates file reading, regex, and CLI flag parsing.

## 78.1 The program (`grep.z`)

```zen
import re
import sys
import os

func grep_file(path, pattern, case_insensitive, invert, show_line_numbers) {
    let flags = if case_insensitive { "i" } else { "" }
    let rx = re.compile(pattern)   # conceptual; flags handling per build
    let f = open(path, "r")
    let lineno = 0
    let matched_any = false
    for line in f {
        lineno += 1
        let line_text = line
        # strip trailing newline for display
        if line_text.endswith("\n") { line_text = line_text[0:len(line_text)-1] }
        let m = rx.search(line_text)
        let matched = m != nil
        if invert { matched = !matched }
        if matched {
            matched_any = true
            let prefix = if show_line_numbers { str(lineno) + ":" } else { "" }
            print(path + ":" + prefix + line_text)
        }
    }
    f.close()
    return matched_any
}

func main(argv) {
    let case_insensitive = false
    let invert = false
    let show_line_numbers = true
    let files = []
    var i = 1
    while i < len(argv) {
        let a = argv[i]
        if a == "-i" { case_insensitive = true }
        elif a == "-v" { invert = true }
        elif a == "-n" { show_line_numbers = true }
        elif a == "-N" { show_line_numbers = false }
        else { files.push(a) }
        i += 1
    }
    if len(files) < 2 {
        print("Usage: zen run grep.z [-i] [-v] [-n|-N] <pattern> <file> [<file> ...]")
        return 1
    }
    let pattern = files[0]
    let targets = files[1:]
    let any = false
    for fp in targets {
        if !os.path.exists(fp) {
            print("grep: " + fp + ": no such file")
            continue
        }
        if grep_file(fp, pattern, case_insensitive, invert, show_line_numbers) {
            any = true
        }
    }
    return if any { 0 } else { 1 }
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 78.2 Example session

Input `app.log`:
```
2024-01-15 09:30:00 INFO Server started
2024-01-15 09:30:02 ERROR Database connection failed
2024-01-15 09:30:03 INFO Request /health
2024-01-15 09:30:05 ERROR Database connection failed
```

```
$ zen run grep.z ERROR app.log
app.log:2:2024-01-15 09:30:02 ERROR Database connection failed
app.log:4:2024-01-15 09:30:05 ERROR Database connection failed

$ zen run grep.z -i started app.log
app.log:1:2024-01-15 09:30:00 INFO Server started

$ zen run grep.z -v INFO app.log
app.log:2:2024-01-15 09:30:02 ERROR Database connection failed
app.log:4:2024-01-15 09:30:05 ERROR Database connection failed
```

## 78.3 Design notes

- **`-i`** case-insensitive, **`-v`** invert-match, **`-n`**/`-N` toggle line
  numbers.
- **Multiple files:** each matching line is prefixed with the filename (like
  GNU grep when multiple files are given).
- **Exit code:** 0 if any match found, 1 if none (useful in shell scripts).
- **Error handling:** missing files are reported but don't abort the run.

## 78.4 Exercises

1. Add `--color` to highlight the matching portion of each line.
2. Add `-c` to print only the count of matching lines per file.
3. Add `-l` to print only filenames that contain a match.
4. Add recursive directory search (`-r`).
5. Add context lines (`-C 2` to show 2 lines before/after each match).
