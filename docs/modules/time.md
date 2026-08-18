# Time Module (`time`)

Time utilities.

```zen
time.now()                    // epoch seconds (float)
time.unix()                   // alias for now()
time.utc()                    // UTC timestamp

time.sleep(2)                 // pause 2 seconds

time.format(time.now(), "%Y-%m-%d %H:%M:%S")
time.parse("2024-01-15", "%Y-%m-%d")

time.year()                   // current year
time.month()                  // 1..12
time.day()                    // day of month
time.hour()                   // hour (0-23)
time.minute()                 // minute
time.second()                 // second
time.weekday()                // 0=Monday .. 6=Sunday
```
