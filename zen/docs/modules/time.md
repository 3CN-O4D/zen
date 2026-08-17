# Time Module (`time`)

Current time, formatting, sleeping.

```zen
time.now()                    // epoch seconds (float)
time.unix()                   // alias for now()
time.utc()                    // UTC timestamp

time.sleep(2)                 // pause 2 seconds

time.format(time.now(), "%Y-%m-%d %H:%M:%S")
// "2026-08-17 09:45:00"

time.parse("2024-01-15", "%Y-%m-%d")

time.year()                   // current year (or pass timestamp)
time.month()                  // 1..12

time.weekday()                // 0=Monday .. 6=Sunday
```
