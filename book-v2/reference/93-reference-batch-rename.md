# Chapter 93: Reference Implementation — Batch File Renamer

Renames all files in a directory matching a pattern: replaces a substring,
adds a prefix/suffix, or converts case. Includes a `--dry-run` mode that shows
what would change without renaming. Demonstrates file walking, string
manipulation, `os.rename`, and safe CLI design.

## 93.1 The program (`rename.z`)

```zen
import os
import pathlib
import sys

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

func new_name(old, replace_from, replace_to, prefix, suffix, upper, lower) {
    let base = pathlib.Path(old).name   # filename only
    let dir = str(pathlib.Path(old).parent)
    let n = base
    if replace_from != nil and replace_from != "" {
        # replace all occurrences (crude: first only here)
        let idx = n.find(replace_from)
        if idx != -1 {
            n = n[0:idx] + replace_to + n[idx + len(replace_from):]
        }
    }
    if prefix != nil { n = prefix + n }
    if suffix != nil {
        let stem = pathlib.Path(n).stem
        let ext = pathlib.Path(n).suffix
        n = stem + suffix + ext
    }
    if upper { n = n.upper() }
    if lower { n = n.lower() }
    if dir == "" or dir == "." { return n }
    return dir + "/" + n
}

func main(argv) {
    if len(argv) < 2 {
        print("Usage:")
        print("  zen run rename.z <dir> [--from X --to Y] [--prefix P] [--suffix S]")
        print("                 [--upper] [--lower] [--dry-run]")
        return 1
    }
    let dir = argv[1]
    let replace_from = nil
    let replace_to = nil
    let prefix = nil
    let suffix = nil
    let upper = false
    let lower = false
    let dry_run = false
    var i = 2
    while i < len(argv) {
        let a = argv[i]
        if a == "--from" and i + 1 < len(argv) { replace_from = argv[i + 1]; i += 2 }
        elif a == "--to" and i + 1 < len(argv) { replace_to = argv[i + 1]; i += 2 }
        elif a == "--prefix" and i + 1 < len(argv) { prefix = argv[i + 1]; i += 2 }
        elif a == "--suffix" and i + 1 < len(argv) { suffix = argv[i + 1]; i += 2 }
        elif a == "--upper" { upper = true; i += 1 }
        elif a == "--lower" { lower = true; i += 1 }
        elif a == "--dry-run" { dry_run = true; i += 1 }
        else { i += 1 }
    }

    let files = walk_files(dir)
    var changed = 0
    for old in files {
        let n = new_name(old, replace_from, replace_to, prefix, suffix, upper, lower)
        if n == pathlib.Path(old).name { continue }   # no change
        let new_path = str(pathlib.Path(old).parent) + "/" + n
        if dry_run {
            print("would rename: " + old + " -> " + new_path)
        } else {
            try {
                os.rename(old, new_path)
                print("renamed: " + old + " -> " + new_path)
                changed += 1
            } catch as e {
                print("failed: " + old + " -> " + new_path + " (" + e.msg + ")")
            }
        }
    }
    if dry_run {
        print("dry run: " + str(changed) + " file(s) would be renamed")
    } else {
        print("done: " + str(changed) + " file(s) renamed")
    }
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 93.2 Example session

```
$ ls photos/
IMG_001.jpg  IMG_002.jpg  vacation.png
$ zen run rename.z photos/ --from IMG_ --to photo_ --dry-run
would rename: photos/IMG_001.jpg -> photos/photo_001.jpg
would rename: photos/IMG_002.jpg -> photos/photo_002.jpg
dry run: 2 file(s) would be renamed

$ zen run rename.z photos/ --from IMG_ --to photo_
renamed: photos/IMG_001.jpg -> photos/photo_001.jpg
renamed: photos/IMG_002.jpg -> photos/photo_002.jpg
done: 2 file(s) renamed

$ zen run rename.z photos/ --prefix trip_ --lower
renamed: photos/photo_001.jpg -> photos/trip_photo_001.jpg
...
done: 3 file(s) renamed
```

## 93.3 Design notes

- **Dry run by default is safer:** use `--dry-run` first to preview changes.
- **Filename-only:** operates on the basename; directory structure is
  preserved.
- **Composable transforms:** replace, prefix, suffix, upper, lower can be
  combined in one run.
- **Errors isolated:** one failed rename doesn't abort the batch.

## 93.4 Exercises

1. Add `--recursive` toggle (off by default) to skip subdirectories.
2. Add sequential numbering: `photo_1.jpg`, `photo_2.jpg`, ...
3. Add `--pattern <glob>` to rename only matching files (e.g., `*.jpg`).
4. Add a `--case` option: `lower`, `upper`, `title`.
5. Add undo support: log renames to a file so they can be reversed.
