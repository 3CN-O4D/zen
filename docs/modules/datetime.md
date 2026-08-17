# Date & Time Module

Complete reference for working with dates, times, formatting, parsing, and timezone operations in Zen.

## Current Time

### Get current timestamp

```
print datetime.now()          // ISO string: "2026-08-17T14:30:00"
print datetime.utcnow()       // UTC ISO string
print datetime.today()        // date only: "2026-08-17"
print datetime.unix()         // Unix timestamp (seconds since epoch)
```

### Get current components

```
print datetime.year()         // 2026
print datetime.month()        // 8
print datetime.day()          // 17
print datetime.hour()         // 14
print datetime.minute()       // 30
print datetime.second()       // 45
print datetime.weekday()      // 0 (Monday)
```

### Constants

```
print datetime.MONDAY     // 0
print datetime.TUESDAY    // 1
print datetime.WEDNESDAY  // 2
print datetime.THURSDAY   // 3
print datetime.FRIDAY     // 4
print datetime.SATURDAY   // 5
print datetime.SUNDAY     // 6
```

---

## The `time` Module

The `time` module provides the same functions as `datetime`:

```
print time.now()         // ISO string
print time.unix()        // Unix timestamp
print time.utc()         // UTC string
print time.date()        // date string
print time.year()        // current year
print time.month()       // current month
print time.day()         // current day
print time.hour()        // current hour
print time.minute()      // current minute
print time.second()      // current second
print time.weekday()     // day of week (0=Monday)
```

---

## Formatting

### Format a datetime string

```
let now = datetime.now()
print datetime.format(now, "%Y-%m-%d")          // 2026-08-17
print datetime.format(now, "%H:%M:%S")          // 14:30:00
print datetime.format(now, "%Y-%m-%d %H:%M")    // 2026-08-17 14:30
print datetime.format(now, "%B %d, %Y")         // August 17, 2026
print datetime.format(now, "%A, %B %d")          // Monday, August 17
```

### Format codes

| Code | Meaning | Example |
|------|---------|---------|
| `%Y` | Year (4 digits) | 2026 |
| `%m` | Month (01-12) | 08 |
| `%d` | Day (01-31) | 17 |
| `%H` | Hour (00-23) | 14 |
| `%I` | Hour (01-12) | 02 |
| `%M` | Minute (00-59) | 30 |
| `%S` | Second (00-59) | 45 |
| `%p` | AM/PM | PM |
| `%A` | Weekday name | Monday |
| `%a` | Weekday short | Mon |
| `%B` | Month name | August |
| `%b` | Month short | Aug |
| `%Y-%m-%d` | ISO date | 2026-08-17 |
| `%H:%M:%S` | Time | 14:30:00 |

### Custom formatting

```
let ts = datetime.now()

// Date only
let date_str = datetime.format(ts, "%Y-%m-%d")
print date_str    // 2026-08-17

// Time only
let time_str = datetime.format(ts, "%H:%M:%S")
print time_str    // 14:30:00

// Friendly format
let friendly = datetime.format(ts, "%A, %B %d, %Y at %I:%M %p")
print friendly    // Monday, August 17, 2026 at 02:30 PM
```

---

## Parsing

### Parse a datetime string

```
let dt = datetime.parse("2026-01-15", "%Y-%m-%d")
print datetime.format(dt, "%B %d, %Y")    // January 15, 2026
```

### Parse with time

```
let dt = datetime.parse("2026-01-15 14:30:00", "%Y-%m-%d %H:%M:%S")
print datetime.format(dt, "%I:%M %p")    // 02:30 PM
```

### From Unix timestamp

```
let ts = 1692153600
let dt = datetime.from_unix(ts)
print datetime.format(dt, "%Y-%m-%d %H:%M")    // 2023-08-16 00:00
```

---

## Date Arithmetic

### Adding days

```
let today = datetime.now()
let next_week = datetime.add_days(today, 7)
print datetime.format(next_week, "%Y-%m-%d")
// Date 7 days from now
```

### Difference between dates

```
let ts1 = datetime.parse("2026-01-01", "%Y-%m-%d")
let ts2 = datetime.parse("2026-12-31", "%Y-%m-%d")
// Note: these return strings, compare timestamps for arithmetic
```

### Using Unix timestamps for arithmetic

```
let now = datetime.unix()
let one_hour_ago = now - 3600
let one_day_ago = now - 86400

let age_hours = (now - one_hour_ago) / 3600
print "Event was {age_hours} hours ago"
```

---

## Common Patterns

### Timestamp logging

```
function log(level, message) {
    let ts = datetime.now()
    let entry = "[{ts}] [{level}] {message}"
    fs.append("app.log", entry + "\n")
}

log("INFO", "Server started")
log("WARN", "High memory usage")
log("ERROR", "Connection failed")
```

### Date-based file naming

```
let today = datetime.format(datetime.now(), "%Y-%m-%d")
let filename = "report-{today}.json"
json.save(filename, report_data)
print "Saved to {filename}"
```

### Checking if it's a weekday

```
let day = datetime.weekday()

if day >= datetime.MONDAY and day <= datetime.FRIDAY {
    print "It's a weekday"
} else {
    print "It's the weekend"
}
```

### Time-based scheduling

```
function is_business_hours() {
    let hour = datetime.hour()
    let day = datetime.weekday()

    let is_weekday = day >= datetime.MONDAY and day <= datetime.FRIDAY
    let is_work_time = hour >= 9 and hour < 17

    return is_weekday and is_work_time
}

if is_business_hours() {
    print "Processing requests"
} else {
    print "Outside business hours"
}
```

### Relative time strings

```
function time_ago(timestamp) {
    let now = datetime.unix()
    let diff = now - timestamp

    if diff < 60 { return "just now" }
    if diff < 3600 { return str(diff / 60) + " minutes ago" }
    if diff < 86400 { return str(diff / 3600) + " hours ago" }
    return str(diff / 86400) + " days ago"
}

print time_ago(datetime.unix() - 300)    // 5 minutes ago
print time_ago(datetime.unix() - 7200)   // 2 hours ago
```

---

## Pro Tips

1. **Use `datetime.unix()` for arithmetic.** Unix timestamps are easy to add/subtract.
2. **Use `datetime.format()` for display.** Human-readable output.
3. **Use `datetime.parse()` for input.** Convert user input to timestamps.
4. **Store timestamps as numbers.** `datetime.unix()` returns a number, easy to compare.
5. **Use `%Y-%m-%d` for sortable date strings.** Lexicographic order matches chronological order.

---

## Common Mistakes

### Confusing local and UTC time

```
// datetime.now() returns local time
print datetime.now()       // 2026-08-17T14:30:00 (local)

// datetime.utcnow() returns UTC
print datetime.utcnow()   // 2026-08-17T12:30:00 (UTC)
```

### Forgetting format codes are case-sensitive

```
// WRONG
datetime.format(ts, "%y-%m-%d")    // 2-digit year

// CORRECT — 4-digit year
datetime.format(ts, "%Y-%m-%d")
```

### Not handling timezone differences

Zen doesn't have built-in timezone support. Use UTC for consistency:

```
// Store all timestamps in UTC
let ts = datetime.utcnow()

// Convert to local only for display
let local = datetime.format(ts, "%H:%M")
```

---

## See Also

- [time Module](overview.md) — Time functions (alias for datetime)
- [os Module](overview.md) — Platform and process info
- [Module Overview](overview.md) — All available modules
