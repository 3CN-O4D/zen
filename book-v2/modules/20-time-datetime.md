# Chapter 20: datetime/time Module

The `datetime`/`time` module provides date and time manipulation: parsing,
formatting, arithmetic, and timestamps. Zen ships a pure-Zen implementation
with a native tier.

## 20.1 The `datetime` type

```zen
import datetime

let now = datetime.datetime.now()
print(now.year)       # 2026
print(now.month)      # 10
print(now.day)        # 26
print(now.hour)       # 14
print(now.minute)     # 30
print(now.second)     # 45
print(now.isoformat())# 2026-10-26T14:30:45
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
print(today.date().isoformat())  # 2026-10-26
```

## 20.3 Formatting: `strftime`

```zen
import datetime

let now = datetime.datetime.now()
print(now.strftime("%Y-%m-%d"))   # 2026-10-26
print(now.strftime("%H:%M:%S"))   # 14:30:45
print(now.strftime("%A, %B %d"))  # Monday, October 26
print(now.strftime("%Y-%m-%d %H:%M:%S"))  # full
```

Format codes:

| Code | Meaning | Example |
|------|---------|---------|
| `%Y` | 4-digit year | `2026` |
| `%m` | Month (01-12) | `10` |
| `%d` | Day (01-31) | `26` |
| `%H` | Hour (00-23) | `14` |
| `%M` | Minute (00-59) | `30` |
| `%S` | Second (00-59) | `45` |
| `%A` | Weekday name | `Monday` |
| `%B` | Month name | `October` |

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

> Note: `datetime` objects are foreign handles from Python. Arithmetic
> (`+`, `-`) works when the Python side supports it; otherwise use
> explicit methods.

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

let dt = datetime.datetime.now()
# Use Python interop for timestamp conversion if needed
pymport datetime as pydt
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

1. Write a function `age(birthdate)` that computes age in years.
2. Write `days_until(target_date)` that returns days remaining.
3. Use `time.time()` to measure how long a sorting operation takes.
4. Write a countdown timer that prints seconds remaining.
5. Format a datetime as "Monday, October 26, 2026 at 2:30 PM".
