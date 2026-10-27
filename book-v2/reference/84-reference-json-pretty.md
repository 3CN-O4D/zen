# Chapter 84: Reference Implementation — JSON Pretty Printer & Validator

Reads a JSON file (or stdin), validates it, and pretty-prints it with indentation.
Also supports minification (removing whitespace) and a `--check` mode that only
validates without output. Demonstrates `json`, file I/O, and CLI flags.

## 84.1 The program (`jsonpp.z`)

```zen
import json
import sys

func read_all(path) {
    if path == "-" or path == "" {
        # read stdin — conceptual; use your build's stdin helper
        return read_stdin()
    }
    with open(path, "r") as f { return f.read() }
}

func read_stdin() {
    # placeholder: read all of stdin line by line
    let lines = []
    # conceptual loop; replace with your build's stdin reading
    return lines.join("\n")
}

func main(argv) {
    let pretty = true
    let check_only = false
    let input = nil
    var i = 1
    while i < len(argv) {
        let a = argv[i]
        if a == "--minify" { pretty = false }
        elif a == "--check" { check_only = true }
        elif a == "--help" or a == "-h" {
            print("Usage: zen run jsonpp.z [--minify|--check] [file.json]")
            return 0
        } else {
            input = a
        }
        i += 1
    }

    let text = if input == nil { read_stdin() } else { read_all(input) }

    let parsed = nil
    try {
        parsed = json.parse(text)
    } catch as e {
        print("invalid JSON: " + e.msg)
        return 1
    }

    if check_only {
        print("valid JSON")
        return 0
    }

    let output = if pretty { json.pretty(parsed) } else { json.stringify(parsed) }
    print(output)
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 84.2 Example session

Input `data.json` (minified):
```json
{"name":"Ada","age":36,"tags":["a","b"],"active":true}
```

```
$ zen run jsonpp.z data.json
{
  "name": "Ada",
  "age": 36,
  "tags": [
    "a",
    "b"
  ],
  "active": true
}

$ zen run jsonpp.z --minify data.json
{"name":"Ada","age":36,"tags":["a","b"],"active":true}

$ zen run jsonpp.z --check data.json
valid JSON

$ zen run jsonpp.z --check bad.json
invalid JSON: ...
```

## 84.3 Design notes

- **Validation first:** `json.parse` throws on invalid JSON; the tool catches
  and reports a clean message instead of a traceback.
- **`--check` mode:** validates without printing, useful in shell scripts
  (exit code 0 = valid).
- **Stdin support:** when no file is given, reads from stdin (conceptual —
  wire up your build's stdin helper).

## 84.4 Exercises

1. Add `--sort-keys` to sort object keys in output.
2. Add `--indent N` to control indentation width.
3. Add `--extract <path>` to print only a dotted-path value (e.g., `tags.0`).
4. Add `--validate-schema <schema>` for basic type checking (conceptual).
5. Support reading from multiple files and concatenating results.
