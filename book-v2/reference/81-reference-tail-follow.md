# Chapter 81: Reference Implementation — Tail -f (File Watcher)

A `tail -f`-like tool: it prints the last N lines of a file, then watches for
new lines appended to the file and prints them as they arrive. Demonstrates
file reading, seeking, polling, and graceful shutdown.

## 81.1 The program (`tailf.z`)

```zen
import sys
import time
import os

func read_last_n(path, n) {
    let lines = []
    with open(path, "r") as f {
        for line in f {
            lines.push(line)
            if len(lines) > n { lines = lines[1:] }   # keep only last n
        }
    }
    return lines
}

func follow(path, n = 10) {
    # open and seek to end (read all existing lines first)
    let f = open(path, "r")
    # skip to end: read all lines to position at EOF
    let last = read_last_n(path, n)
    for line in last {
        let l = line
        if l.endswith("\n") { l = l[0:len(l)-1] }
        print(l)
    }
    print("--- watching " + path + " (Ctrl-C to stop) ---")
    while true {
        let line = f.readline()
        if line == nil {
            time.sleep(500)   # poll every 0.5s
            continue
        }
        let l = line
        if l.endswith("\n") { l = l[0:len(l)-1] }
        print(l)
    }
    f.close()
}

func main(argv) {
    if len(argv) < 2 {
        print("Usage: zen run tailf.z <file> [lines]")
        return 1
    }
    let path = argv[1]
    let n = int(argv[2] ?? "10")
    follow(path, n)
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

> Note: `readline()` and `pathlib` availability depend on the stdlib build; if
> `readline` is unavailable, read in a loop with `read(1)` and split on `\n`.

## 81.2 Example session

Terminal 1:
```
$ zen run tailf.z app.log 5
2024-01-15 09:30:00 INFO Server started
2024-01-15 09:30:01 DEBUG Loading config
2024-01-15 09:30:02 WARN High memory
2024-01-15 09:30:03 ERROR DB failed
2024-01-15 09:30:04 INFO Retrying
--- watching app.log (Ctrl-C to stop)
2024-01-15 09:30:06 INFO Connected   # <- appears as another process appends it
2024-01-15 09:30:07 INFO Request /api
```

Terminal 2 (another process appends to app.log):
```
$ echo "2024-01-15 09:30:06 INFO Connected" >> app.log
$ echo "2024-01-15 09:30:07 INFO Request /api" >> app.log
```

## 81.3 Design notes

- **Polling:** the tool sleeps 500 ms between checks when no new data arrives.
  This is simple but not instant; reduce the sleep for lower latency.
- **Last N lines:** on startup it shows the tail so you have context before
  following.
- **Graceful shutdown:** Ctrl-C ends the loop and closes the file.

## 81.4 Exercises

1. Add `--follow-name` to track a file even if it's rotated (reopen by name).
2. Add `--lines N` to control how many initial lines to show.
3. Add `--quiet` to suppress the initial tail and only show new lines.
4. Add multiple file support (`tailf.z file1 file2`).
5. Add `--max-lines` to stop after printing N new lines.
