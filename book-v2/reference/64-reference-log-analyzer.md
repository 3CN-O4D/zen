# Chapter 64: Reference Implementation — Log Analyzer

A log file analyzer that reads a text log (one entry per line), parses timestamp
and level fields, counts entries by level, finds error lines, and writes a
summary report. It demonstrates file reading line-by-line, string parsing,
dict counting, regex, sorting, and report generation.

## 64.1 The program (`analyze.z`)

```zen
import os
import re
import sys
import pathlib

# Sample log line format:
# 2024-01-15 09:30:00 INFO Server started on port 8080
# 2024-01-15 09:30:05 WARN High memory usage: 82%
# 2024-01-15 09:30:10 ERROR Database connection failed

let LOG_LINE_RE = r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\s+(INFO|WARN|ERROR|DEBUG)\s+(.*)$"

func parse_line(line) {
    let m = re.match(LOG_LINE_RE, line)
    if m == nil { return nil }
    return #{
        timestamp: m[1],
        level: m[2],
        message: m[3],
        raw: line,
    }
}

func analyze(path) {
    let counts = #{INFO: 0, WARN: 0, ERROR: 0, DEBUG: 0}
    let errors = []
    let total = 0
    let first_ts = nil
    let last_ts = nil

    with open(path, "r") as f {
        for line in f {
            let entry = parse_line(line)
            if entry == nil { continue }
            total += 1
            let level = entry["level"]
            counts[level] = (counts[level] ?? 0) + 1
            if level == "ERROR" { errors.push(entry) }
            if first_ts == nil { first_ts = entry["timestamp"] }
            last_ts = entry["timestamp"]
        }
    }

    print("Log analysis: " + path)
    print("  total entries: " + str(total))
    print("  INFO:   " + str(counts["INFO"] ?? 0))
    print("  WARN:   " + str(counts["WARN"] ?? 0))
    print("  ERROR:  " + str(counts["ERROR"] ?? 0))
    print("  DEBUG:  " + str(counts["DEBUG"] ?? 0))
    if first_ts != nil {
        print("  first:  " + first_ts)
        print("  last:   " + last_ts)
    }
    print("  errors: " + str(len(errors)))
    for e in errors {
        print("    [" + e["timestamp"] + "] " + e["message"])
    }

    # write summary report
    let report = pathlib.Path(path).stem + ".report.txt"
    let out = []
    out.push("Log analysis: " + path)
    out.push("  total entries: " + str(total))
    out.push("  INFO:   " + str(counts["INFO"] ?? 0))
    out.push("  WARN:   " + str(counts["WARN"] ?? 0))
    out.push("  ERROR:  " + str(counts["ERROR"] ?? 0))
    out.push("  DEBUG:  " + str(counts["DEBUG"] ?? 0))
    out.push("  errors: " + str(len(errors)))
    for e in errors {
        out.push("    [" + e["timestamp"] + "] " + e["message"])
    }
    write_all(report, out.join("\n") + "\n")
    print("report written to " + report)
}

func write_all(path, text) {
    with open(path, "w") as f { f.write(text) }
}

func main(argv) {
    if len(argv) < 2 {
        print("Usage: zen run analyze.z <logfile>")
        return 1
    }
    analyze(argv[1])
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 64.2 Example input (`app.log`)

```
2024-01-15 09:30:00 INFO Server started on port 8080
2024-01-15 09:30:01 DEBUG Loading config from config.yaml
2024-01-15 09:30:02 WARN High memory usage: 82%
2024-01-15 09:30:03 ERROR Database connection failed
2024-01-15 09:30:04 INFO Request GET /health from 10.0.0.5
2024-01-15 09:30:05 ERROR Database connection failed
2024-01-15 09:30:06 INFO Request GET /api/users from 10.0.0.9
```

## 64.3 Expected output

```
$ zen run analyze.z app.log
Log analysis: app.log
  total entries: 7
  INFO:   3
  WARN:   1
  ERROR:  2
  DEBUG:  1
  first:  2024-01-15 09:30:00
  last:   2024-01-15 09:30:06
  errors: 2
    [2024-01-15 09:30:03] Database connection failed
    [2024-01-15 09:30:05] Database connection failed
report written to app.report.txt
```

## 64.4 Design notes

- The regex captures timestamp, level, and message in one pass.
- Lines that don't match are silently skipped (tolerant of malformed entries).
- Errors are collected and printed in full, plus written to a `.report.txt`
  sidecar file.
- File is read line-by-line (low memory) rather than loaded whole.

## 64.5 Exercises

1. Add hourly aggregation: count entries per hour.
2. Add a `--level ERROR` filter to only analyze a specific level.
3. Add top-N frequent error messages (dedupe and sort by count).
4. Add JSON output mode (`--json`) for programmatic consumption.
5. Watch mode: re-analyze when the file changes (poll mtime).
