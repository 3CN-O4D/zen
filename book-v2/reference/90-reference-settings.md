# Chapter 90: Reference Implementation — Settings Manager (INI-like Config)

A small tool that reads and writes INI-style configuration files (`[section]`
headers and `key = value` pairs). Supports `get`, `set`, and `list` operations.
Demonstrates file parsing, dict-of-dicts data structure, and CLI dispatch.

## 90.1 The program (`settings.z`)

```zen
import sys
import os
import pathlib

func parse_ini(text) {
    let sections = #{}
    let current = nil
    for line in text.splitlines() {
        let trimmed = line.strip()
        if trimmed == "" or trimmed.startsWith("#") or trimmed.startsWith(";") {
            continue
        }
        if trimmed.startsWith("[") and trimmed.endsWith("]") {
            current = trimmed[1:len(trimmed)-1]
            if !(current in sections) { sections[current] = #{} }
            continue
        }
        let idx = trimmed.find("=")
        if idx != -1 and current != nil {
            let key = trimmed[0:idx].strip()
            let value = trimmed[idx+1:].strip()
            sections[current][key] = value
        }
    }
    return sections
}

func stringify_ini(sections) {
    let out = ""
    for section in sections {
        out = out + "[" + section + "]\n"
        let dict = sections[section]
        for key in dict {
            out = out + key + " = " + dict[key] + "\n"
        }
        out = out + "\n"
    }
    return out
}

func read_all(path) {
    with open(path, "r") as f { return f.read() }
}

func write_all(path, text) {
    with open(path, "w") as f { f.write(text) }
}

func main(argv) {
    if len(argv) < 3 {
        print("Usage:")
        print("  zen run settings.z <file> list")
        print("  zen run settings.z <file> get <section> <key>")
        print("  zen run settings.z <file> set <section> <key> <value>")
        return 1
    }
    let path = argv[1]
    let cmd = argv[2]

    let sections = #{}
    let p = pathlib.Path(path)
    if p.exists() {
        try { sections = parse_ini(read_all(path)) } catch as e {}
    }

    if cmd == "list" {
        for section in sections {
            print("[" + section + "]")
            for key in sections[section] {
                print("  " + key + " = " + sections[section][key])
            }
        }
    } elif cmd == "get" {
        if len(argv) < 5 {
            print("usage: settings.z <file> get <section> <key>")
            return 1
        }
        let section = argv[3]
        let key = argv[4]
        if section in sections and key in sections[section] {
            print(sections[section][key])
        } else {
            print("not found")
            return 1
        }
    } elif cmd == "set" {
        if len(argv) < 6 {
            print("usage: settings.z <file> set <section> <key> <value>")
            return 1
        }
        let section = argv[3]
        let key = argv[4]
        let value = argv[5]
        if !(section in sections) { sections[section] = #{} }
        sections[section][key] = value
        write_all(path, stringify_ini(sections))
        print("set " + section + "/" + key + " = " + value)
    } else {
        print("unknown command: " + cmd)
        return 1
    }
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 90.2 Example session

```
$ cat > config.ini <<'EOF'
[database]
host = localhost
port = 5432

[app]
debug = true
EOF
$ zen run settings.z config.ini list
[database]
  host = localhost
  port = 5432
[app]
  debug = true

$ zen run settings.z config.ini get database host
localhost

$ zen run settings.z config.ini set database port 3306
set database/port = 3306
$ zen run settings.z config.ini get database port
3306
```

## 90.3 Design notes

- **INI format:** `[section]` headers and `key = value` pairs; `#` and `;`
  lines are comments.
- **Data structure:** a dict of dicts (`#{section => #{key => value}}`).
- **Create-if-missing:** if the file doesn't exist, `set` creates it.
- **Round-trip:** `stringify_ini` rewrites the whole file (loses comments and
  formatting — fine for simple configs, not for hand-edited ones).

## 90.4 Exercises

1. Add support for quoted values (`key = "value with spaces"`).
2. Add `--section` to list only one section.
3. Add `delete <section> <key>` to remove a key.
4. Add type hints: `get --int` / `get --bool` to parse values.
5. Preserve comments and formatting on write (harder; good challenge).
