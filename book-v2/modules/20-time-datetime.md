# Chapter 20: datetime/time Module

The `datetime`/`time` module provides date and time manipulation: parsing, formatting, arithmetic, and timestamps. Zen ships a pure-Zen implementation with a native tier.

## 20.1 The `datetime` type

```zen
import datetime

let now = datetime.datetime.now()
print(now.year)       # 2026
print(now.month)      # 10
print(now.day)        # 27
print(now.hour)       # 14
print(now.minute)     # 30
print(now.second)     # 45
print(now.isoformat())# 2026-10-27T14:30:45
print(now.date())     # date object
```

## 20.2 Creating datetimes

```zen
import datetime

let epoch = datetime.datetime.fromtimestamp(0)
print(epoch.isoformat())  # 1970-01-01T00:00:00

let specific = datetime.datetime(2030, 1, 2, 3, 4, 5)
print(specific.strftime("%Y-%m-%d %H:%M:%S"))  # 2030-01-02 03:04:05

let today = datetime.datetime.now()
print(today.date().isoformat())  # 2026-10-27
```

### Constructor signatures

```zen
datetime.datetime.now()           # current local datetime
datetime.datetime.fromtimestamp(ts)  # from Unix timestamp (float)
datetime.datetime(y, m, d)        # date only, midnight
datetime.datetime(y, m, d, h, mi, s)  # full datetime
```

## 20.3 Formatting: `strftime`

```zen
import datetime

let now = datetime.datetime.now()
print(now.strftime("%Y-%m-%d"))   # 2026-10-27
print(now.strftime("%H:%M:%S"))   # 14:30:45
print(now.strftime("%A, %B %d"))  # Tuesday, October 27
print(now.strftime("%Y-%m-%d %H:%M:%S"))  # full
```

Format codes:

| Code | Meaning | Example |
|------|---------|---------|
| `%Y` | 4-digit year | `2026` |
| `%m` | Month (01-12) | `10` |
| `%d` | Day (01-31) | `27` |
| `%H` | Hour (00-23) | `14` |
| `%M` | Minute (00-59) | `30` |
| `%S` | Second (00-59) | `45` |
| `%A` | Weekday name | `Tuesday` |
| `%a` | Abbrev weekday | `Tue` |
| `%B` | Month name | `October` |
| `%b` | Abbrev month | `Oct` |
| `%y` | 2-digit year | `26` |
| `%%` | Literal `%` | `%` |

```zen
import datetime

let d = datetime.datetime(2026, 10, 27, 14, 30, 45)
print(d.strftime("%Y-%m-%d"))          # 2026-10-27
print(d.strftime("%H:%M:%S"))          # 14:30:45
print(d.strftime("%A, %B %d, %Y"))     # Tuesday, October 27, 2026
print(d.strftime("%I:%M %p"))          # 02:30 PM
print(d.strftime("%Y-%m-%dT%H:%M:%S")) # ISO-like
```

## 20.4 Datetime arithmetic

```zen
import datetime

let now = datetime.datetime.now()
let later = datetime.datetime(2030, 1, 1)

# Difference
let delta = later - now
print(delta.days)   # days between

# Add a timedelta
let one_week = datetime.timedelta(days=7)
let next_week = now + one_week
print(next_week.isoformat())
```

> Note: `datetime` objects are foreign handles from Python. Arithmetic (`+`, `-`) works when the Python side supports it; otherwise use explicit methods.

### timedelta

```zen
import datetime

let one_day = datetime.timedelta(days=1)
let one_hour = datetime.timedelta(hours=1)
let one_minute = datetime.timedelta(minutes=30)

let now = datetime.datetime.now()
print((now + one_day).isoformat())      # tomorrow
print((now - one_hour).isoformat())     # 1 hour ago
print((now + one_minute).isoformat())   # 30 min from now

let delta = datetime.timedelta(days=7, hours=3, minutes=30)
print(delta.days)   # 7
print(delta.seconds)  # seconds component (not total)
print(delta.total_seconds())  # total seconds if available
```

## 20.5 The `time` module

```zen
import time

print(time.time())        # current timestamp (float)
print(time.sleep(1))      # sleep 1 second
print(time.strftime("%H:%M:%S"))  # formatted local time

let t = time.time()
# ... do work ...
print(time.time() - t)    # elapsed seconds
```

### time functions

| Function | Description |
|----------|-------------|
| `time.time()` | Current Unix timestamp (float seconds) |
| `time.sleep(secs)` | Sleep for secs seconds |
| `time.strftime(fmt)` | Format current local time |

```zen
import time

let start = time.time()
# ... do work ...
let elapsed = time.time() - start
print("elapsed: " + str(elapsed) + "s")

time.sleep(1)   # sleep 1 second
time.sleep(0.5) # sleep 500ms
```

## 20.6 Common patterns

### Timestamp → datetime

```zen
import datetime

let ts = 1785309045.0
let dt = datetime.datetime.fromtimestamp(ts)
print(dt.isoformat())
```

### datetime → timestamp

```zen
import datetime
pymport datetime as pydt

let dt = datetime.datetime.now()
let ts = pydt.datetime.now().timestamp()
print(ts)
```

### Measure execution time

```zen
import time

let start = time.time()
# ... some work ...
let elapsed = time.time() - start
print("elapsed: " + str(elapsed) + "s")
```

### Sleep

```zen
import time

time.sleep(1)   # sleep 1 second
time.sleep(0.5) # sleep 500ms
```

### Countdown timer

```zen
import time

func countdown(seconds) {
    while seconds > 0 {
        print(str(seconds))
        time.sleep(1)
        seconds = seconds - 1
    }
    print("Done!")
}

countdown(5)
```

## 20.7 Quick reference

```zen
import datetime
import time

datetime.datetime.now()           # current datetime
datetime.datetime(2030, 1, 2, 3, 4, 5)
datetime.datetime.fromtimestamp(0)
now.strftime("%Y-%m-%d %H:%M:%S")
now.date().isoformat()
datetime.timedelta(days=7)
now + datetime.timedelta(days=7)

time.time()                       # timestamp
time.sleep(1)                     # sleep
time.strftime("%H:%M:%S")
```

## 20.8 Exercises

1. Write `age(birthdate)` that computes age in years.
2. Write `days_until(target_date)` that returns days remaining.
3. Use `time.time()` to measure how long a sorting operation takes.
4. Write a countdown timer that prints seconds remaining.
5. Format a datetime as "Monday, October 26, 2026 at 2:30 PM".
6. Write `is_weekend(dt)` that returns true for Saturday/Sunday.
7. Write `add_business_days(dt, n)` that adds n business days (skipping weekends).
