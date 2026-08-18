# Datetime Module (`datetime`)

Date/time utilities with day constants.

```zen
datetime.now()               // current timestamp
datetime.utcnow()            // UTC timestamp
datetime.today()             // today's date
datetime.unix()              // alias for now()
datetime.from_unix(1234567890)

datetime.format(ts, "%A, %B %d, %Y")
datetime.parse("2024-01-15", "%Y-%m-%d")

datetime.year()
datetime.month()
datetime.day()
datetime.hour()
datetime.minute()
datetime.second()
datetime.weekday()

datetime.add_days(ts, 7)     // timestamp + 7 days

// Constants
datetime.MONDAY              // 0.0
datetime.TUESDAY             // 1.0
datetime.WEDNESDAY           // 2.0
datetime.THURSDAY            // 3.0
datetime.FRIDAY              // 4.0
datetime.SATURDAY            // 5.0
datetime.SUNDAY              // 6.0
```
