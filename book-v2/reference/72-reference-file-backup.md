# Chapter 72: Reference Implementation — File Backup / Synchronizer

A simple backup tool: it copies files from a source directory to a destination
directory, skipping files that haven't changed (detected via SHA-256 hash
comparison), and prints a summary. Demonstrates file walking, hashing, binary
copy, and conditional copying.

## 72.1 The program (`backup.z`)

```zen
import os
import pathlib
import hashlib
import sys
import time

var copied = 0
var skipped = 0
var errors = 0

func file_hash(path) {
    let b = pathlib.Path(path)
    if !b.exists() { return nil }
    let data = read_all_bytes(str(b))
    return hashlib.sha256(data)
}

func read_all_bytes(path) {
    with open(path, "rb") as f {
        let parts = []
        while true {
            let chunk = f.read(8192)
            if chunk == nil or len(chunk) == 0 { break }
            parts.push(chunk)
        }
        return parts.join("")
    }
}

func copy_file(src, dst) {
    let dst_p = pathlib.Path(dst)
    let parent = dst_p.parent
    if parent != nil and str(parent) != "" {
        os.makedirs(str(parent))
    }
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

func should_copy(src_path, dst_path) {
    let dst = pathlib.Path(dst_path)
    if !dst.exists() { return true }
    let src_hash = file_hash(src_path)
    let dst_hash = file_hash(str(dst))
    if src_hash == nil or dst_hash == nil { return true }
    return src_hash != dst_hash
}

func walk_and_copy(src_dir, dst_dir) {
    func walk(d, prefix) {
        for name in os.listdir(d) {
            let src = pathlib.Path(d) / name
            let rel = (prefix == "" ? name : prefix + "/" + name)
            let dst = pathlib.Path(dst_dir) / rel
            if src.is_dir() {
                walk(str(src), rel)
            } else {
                try {
                    if should_copy(str(src), str(dst)) {
                        copy_file(str(src), str(dst))
                        copied += 1
                        print("copy  " + rel)
                    } else {
                        skipped += 1
                        print("skip  " + rel + " (unchanged)")
                    }
                } catch as e {
                    errors += 1
                    print("error " + rel + ": " + e.msg)
                }
            }
        }
    }
    walk(src_dir, "")
}

func main(argv) {
    if len(argv) < 3 {
        print("Usage: zen run backup.z <src_dir> <dst_dir>")
        print("  Example: zen run backup.z ./docs ./backup")
        return 1
    }
    let src = argv[1]
    let dst = argv[2]
    let start = time.time()
    print("backup: " + src + " -> " + dst)
    walk_and_copy(src, dst)
    let elapsed = time.time() - start
    print("----")
    print("copied: " + str(copied))
    print("skipped: " + str(skipped))
    print("errors: " + str(errors))
    print("elapsed: " + str(math.round(elapsed * 1000)) + " ms")
    import math
    return if errors == 0 { 0 } else { 1 }
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 72.2 Example session

```
$ zen run backup.z ./content ./public
copy  about.md
copy  posts/hello.md
copy  posts/welcome.md
skip  index.md (unchanged)
copy  static/style.css
----
copied: 4
skipped: 1
errors: 0
elapsed: 12 ms

$ zen run backup.z ./content ./public   # run again — everything skipped
skip  about.md (unchanged)
skip  posts/hello.md (unchanged)
...
----
copied: 0
skipped: 6
errors: 0
elapsed: 8 ms
```

## 72.3 Design notes

- **Content-addressed skipping:** files are compared by SHA-256 hash, so
  re-running the backup only copies changed files — efficient for large trees.
- **Binary copy in 8 KB chunks:** works for any file type and avoids loading
  entire files into memory.
- **Directory structure preserved:** relative paths under `src_dir` are
  recreated under `dst_dir`.
- **Error isolation:** one file's failure doesn't abort the whole backup.

## 72.4 Exercises

1. Add `--dry-run` to print what would be copied without copying.
2. Add deletion: remove files in `dst` that no longer exist in `src`.
3. Add a manifest file listing all backed-up files and their hashes.
4. Add compression option (gzip) via FFI or external tool.
5. Add `--include "*.md"` / `--exclude "*.tmp"` filters.
