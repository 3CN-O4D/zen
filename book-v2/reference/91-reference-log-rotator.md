# Chapter 91: Reference Implementation — Log Rotator

Rotates log files by size: when the active log exceeds a size threshold, it is
renamed to `log.1`, `log.1` to `log.2`, and so on (up to a max count), and a
fresh empty log is started. Demonstrates file I/O, renaming, deletion, and CLI
flags.

## 91.1 The program (`logrotate.z`)

```zen
import os
import pathlib
import sys

func file_size(path) {
    let b = pathlib.Path(path)
    if b.exists() and b.is_file() {
        return b.stat().size   # conceptual: file size in bytes
    }
    return 0
}

func rotate(log_path, max_size, max_count) {
    let size = file_size(log_path)
    print(log_path + " is " + str(size) + " bytes (limit " + str(max_size) + ")")

    if size < max_size {
        print("no rotation needed")
        return 0
    }

    # shift existing rotations: max_count .. 1
    var i = max_count
    while i >= 1 {
        let from = if i == 1 { log_path } else { log_path + "." + str(i - 1) }
        let to = log_path + "." + str(i)
        let from_p = pathlib.Path(from)
        if from_p.exists() {
            if i == max_count {
                # oldest: delete instead of shifting
                try { os.remove(from) } catch as e { print("delete failed: " + e.msg) }
            } else {
                try { os.rename(from, to) } catch as e { print("rename failed: " + e.msg) }
            }
        }
        i -= 1
    }

    # start a fresh empty log
    try {
        # close any open handles first in real use; here just truncate
        with open(log_path, "w") as f { f.write("") }
        print("rotated: new empty " + log_path)
    } catch as e {
        print("truncate failed: " + e.msg)
        return 1
    }
    return 0
}

func main(argv) {
    if len(argv) < 2 {
        print("Usage: zen run logrotate.z <logfile> [max_size_bytes] [max_count]")
        print("  Example: zen run logrotate.z app.log 1048576 5")
        return 1
    }
    let log_path = argv[1]
    let max_size = int(argv[2] ?? "1048576")   # 1 MB default
    let max_count = int(argv[3] ?? "5")
    return rotate(log_path, max_size, max_count)
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 91.2 Example session

```
$ echo "lots of log data..." > app.log
# (make app.log larger than 1 MB)
$ zen run logrotate.z app.log 1000000 5
app.log is 1500000 bytes (limit 1000000)
rotated: new empty app.log
$ ls -la
app.log          0 bytes
app.log.1  1500000 bytes   # previous content
```

Run again after app.log grows:
```
$ zen run logrotate.z app.log 1000000 5
app.log is 1200000 bytes
rotated: new empty app.log
$ ls -la
app.log.1   1200000 bytes
app.log.2   1500000 bytes
```

## 91.3 Design notes

- **Shift strategy:** `log.N` → `log.(N+1)`; the oldest (`log.max_count`) is
  deleted to bound disk usage.
- **Truncate-and-rotate:** after shifting, the active log is truncated to zero.
- **Size-only trigger:** rotation is size-based; time-based rotation (daily,
  hourly) is a natural extension.
- **No open-handle handling:** in production you'd coordinate with the writing
  process so it reopens the new log (or use `copytruncate`).

## 91.4 Exercises

1. Add time-based rotation: rotate if the file wasn't modified in N hours.
2. Add compression: gzip rotated files (via FFI or external tool).
3. Add `--dry-run` to report what would happen without changing files.
4. Add a configuration file listing multiple logs with individual limits.
5. Add a `--compress` flag to compress the rotated file before keeping it.
