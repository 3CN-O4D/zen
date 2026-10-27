# Chapter 88: Reference Implementation — File Deduplicator

Finds duplicate files in a directory tree by computing SHA-256 hashes and
grouping files with identical content. Reports groups of duplicates and can
optionally delete all but the first copy. Demonstrates file walking, hashing,
dict grouping, and optional destructive action.

## 88.1 The program (`dedup.z`)

```zen
import os
import pathlib
import hashlib
import sys

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

func walk_files(dir) {
    let out = []
    func walk(d) {
        for name in os.listdir(d) {
            let p = pathlib.Path(d) / name
            if p.is_dir() {
                walk(str(p))
            } else {
                out.push(str(p))
            }
        }
    }
    walk(dir)
    return out
}

func find_duplicates(dir, delete = false) {
    let files = walk_files(dir)
    let by_hash = #{}
    for f in files {
        let h = file_hash(f)
        if h == nil { continue }
        if h in by_hash { by_hash[h].push(f) }
        else { by_hash[h] = [f] }
    }

    let dup_groups = []
    for h in by_hash {
        if len(by_hash[h]) > 1 {
            dup_groups.push(by_hash[h])
        }
    }

    # sort groups by size descending
    dup_groups.sort(fn(a, b) { return len(b) - len(a) })

    print("scanned " + str(len(files)) + " file(s)")
    print("found " + str(len(dup_groups)) + " duplicate group(s)")
    print("========================")

    var total_saved = 0
    for group in dup_groups {
        let first = group[0]
        print("group (" + str(len(group)) + " copies):")
        print("  keep: " + first)
        var i = 1
        while i < len(group) {
            let dup = group[i]
            print("  dup:  " + dup)
            if delete {
                try {
                    os.remove(dup)
                    print("    (deleted)")
                } catch as e {
                    print("    (delete failed: " + e.msg + ")")
                }
            }
            i += 1
        }
    }
    return dup_groups
}

func main(argv) {
    if len(argv) < 2 {
        print("Usage: zen run dedup.z <dir> [--delete]")
        print("  Example: zen run dedup.z ./downloads")
        print("  With --delete: removes all but the first copy")
        return 1
    }
    let dir = argv[1]
    let delete = len(argv) >= 3 and argv[2] == "--delete"
    find_duplicates(dir, delete)
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 88.2 Example session

```
$ zen run dedup.z ./photos
scanned 142 file(s)
found 3 duplicate group(s)
========================
group (3 copies):
  keep: ./photos/vacation/img1.jpg
  dup:  ./photos/backup/img1.jpg
  dup:  ./photos/archive/img1.jpg
group (2 copies):
  keep: ./photos/vacation/img2.jpg
  dup:  ./photos/backup/img2.jpg
```

Run with `--delete` to remove duplicates (keeps the first copy of each group).

## 88.3 Design notes

- **Content-addressed:** files are grouped by SHA-256 hash, so duplicates are
  detected by content, not by name.
- **Groups sorted by size:** the largest duplicate groups are shown first.
- **`--delete` is opt-in:** destructive action requires an explicit flag.
- **Errors ignored:** unreadable files are skipped silently (hash returns nil).

## 88.4 Exercises

1. Add `--min-size N` to skip files smaller than N bytes.
2. Add `--dry-run` to report what would be deleted without deleting.
3. Add `--hash md5|sha1|sha256` to choose the hash algorithm.
4. Export results as JSON (`--json`).
5. Add a `--exclude "*.tmp"` pattern filter.
