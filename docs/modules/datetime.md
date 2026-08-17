# Date & Time (datetime)

## Current Time

```
datetime.now()              // ISO string
datetime.utcnow()           // UTC ISO string
datetime.today()            // date string
datetime.unix()             // unix timestamp
```

## Parsing & Formatting

```
datetime.from_unix(ts)      // ISO string from timestamp
datetime.parse("2024-01-01", "%Y-%m-%d")  // parse date
datetime.format(dt, "%Y")   // format date
```

## Components

```
datetime.year()             // current year
datetime.month()            // current month (1-12)
datetime.day()              // current day
datetime.hour()             // current hour
datetime.minute()           // current minute
datetime.second()           // current second
datetime.weekday()          // 0=Monday, 6=Sunday
```

## Constants

```
datetime.MONDAY    // 0
datetime.TUESDAY   // 1
datetime.WEDNESDAY // 2
datetime.THURSDAY  // 3
datetime.FRIDAY    // 4
datetime.SATURDAY  // 5
datetime.SUNDAY    // 6
```
