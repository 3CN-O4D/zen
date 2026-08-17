# Datetime Module (`datetime`)

Date/time utilities. Alias for time with convenience methods.

```zen
datetime.now()               // current timestamp
datetime.utcnow()            // UTC timestamp
datetime.today()             // today's date

// Formatting
let ts = time.now()
datetime.format(ts, "%A, %B %d, %Y")
// "Monday, August 17, 2026"

// Components (with optional timestamp)
datetime.year()
datetime.month()
datetime.day()
datetime.hour()
datetime.minute()
datetime.second()

// Add/subtract
datetime.add_days(ts, 7)     // timestamp + 7 days

// Weekday constants
datetime.MONDAY              // 0.0
datetime.SUNDAY              // 6.0
