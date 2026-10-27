# Chapter 56: Cookbook — Files & Data

Complete recipes for file I/O, CSV, JSON, and data processing.

## 56.1 Recipe: Read a whole file safely

```zen
func read_all(path) {
    try {
        with open(path, "r") as f {
            return f.read()
        }
    } catch as e {
        print("cannot read " + path + ": " + e.msg)
        return nil
    }
}
let content = read_all("data.txt")
if content != nil { print(content[0:100]) }
```

## 56.2 Recipe: Write a file (create/overwrite)

```zen
func write_all(path, text) {
    with open(path, "w") as f {
        f.write(text)
    }
}
write_all("out.txt", "Hello, Zen!\n")
```

## 56.3 Recipe: Append a timestamped log line

```zen
import datetime
func append_log(path, message) {
    let ts = datetime.strftime("%Y-%m-%d %H:%M:%S", datetime.now())
    with open(path, "a") as f {
        f.write(ts + " " + message + "\n")
    }
}
append_log("app.log", "started server")
```

Expected output in `app.log`:
```
2024-01-15 09:30:00 started server
```

## 56.4 Recipe: Copy a file in chunks (binary-safe)

```zen
func copy_file(src, dst) {
    with open(src, "rb") as fin {
        with open(dst, "wb") as fout {
            while true {
                let chunk = fin.read(8192)
                if chunk == nil or len(chunk) == 0 { break }
                fout.write(chunk)
            }
        }
    }
}
copy_file("input.bin", "output.bin")
```

## 56.5 Recipe: Count lines without loading the whole file

```zen
func line_count(path) {
    var n = 0
    with open(path, "r") as f {
        for line in f { n += 1 }
    }
    return n
}
print(line_count("bigfile.txt"))   # 42
```

## 56.6 Recipe: Grep a file for a pattern

```zen
func grep_file(path, pattern) {
    with open(path, "r") as f {
        for line in f {
            if line.contains(pattern) {
                print(line.strip())
            }
        }
    }
}
grep_file("log.txt", "ERROR")
```

## 56.7 Recipe: Parse CSV into a list of dicts

```zen
func parse_csv(text) {
    let lines = text.splitlines()
    if len(lines) == 0 { return [] }
    let header = lines[0].split(",")
    let rows = []
    for i in 1..len(lines) {
        let vals = lines[i].split(",")
        let row = #{}
        for j in 0..len(header) {
            if j < len(vals) { row[header[j]] = vals[j] }
        }
        rows.push(row)
    }
    return rows
}

let data = parse_csv(read_all("data.csv"))
for row in data {
    print(row["name"], row["age"])
}
```

Given `data.csv`:
```
name,age
Ada,36
Alan,42
```

Expected output:
```
Ada 36
Alan 42
```

## 56.8 Recipe: Read, modify, write JSON config

```zen
import json
import pathlib

let p = pathlib.Path("config.json")
let cfg = json.load(str(p))
cfg["theme"] = "dark"
cfg["version"] = 2
json.save(str(p), cfg)
print("config updated")
```

## 56.9 Recipe: Find all files with a suffix recursively

```zen
import os
import pathlib
func find_files(dir, suffix) {
    let out = []
    for name in os.listdir(dir) {
        let p = pathlib.Path(dir) / name
        if p.is_dir() {
            for sub in find_files(str(p), suffix) { out.push(sub) }
        } elif name.endswith(suffix) {
            out.push(str(p))
        }
    }
    return out
}
for f in find_files(".", ".z") { print(f) }
```

Expected output (example):
```
main.z
utils.z
lib/math.z
```

## 56.10 Recipe: Compute SHA-256 of a file

```zen
import hashlib
func file_hash(path) {
    return hashlib.sha256(read_all(path))
}
print(file_hash("document.pdf"))   # a3f5...c9
```

## 56.11 Recipe: Deduplicate files by content hash

```zen
func dedupe_files(paths) {
    let by_hash = #{}
    for p in paths {
        let h = file_hash(p)
        if h in by_hash { by_hash[h].push(p) }
        else { by_hash[h] = [p] }
    }
    let dupes = []
    for h in by_hash {
        if len(by_hash[h]) > 1 { dupes.push(by_hash[h]) }
    }
    return dupes
}
```

## 56.12 Recipe: Ensure a directory exists

```zen
import os
func ensure_dir(path) {
    os.makedirs(path)
}
ensure_dir("output/subdir")
```
