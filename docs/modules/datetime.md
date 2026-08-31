# datetime — Date and time objects

The `datetime` module provides high-level functions for date/time manipulation. It is available globally as `datetime`.

```zen
# 1. Current timestamp (epoch seconds)
var t = datetime.now()
print(t)

# 2. Components
print(datetime.year(t))
print(datetime.weekday(t)) # 0 for Monday

# 3. Arithmetic
var tomorrow = datetime.add_days(t, 1)
```

## Functions

| Function | Description |
|----------|-------------|
| `now()` / `today()` | Returns current local timestamp (float). |
| `utcnow()` | Returns current UTC timestamp. |
| `year(t)` / `month(t)` / `day(t)` | Extracts components from a timestamp. |
| `hour(t)` / `minute(t)` / `second(t)` | Extracts time components. |
| `weekday(t)` | Returns day index (0=Monday, 6=Sunday). |
| `format(t, fmt?)` | Formats a timestamp into a string. |
| `parse(str, fmt)` | Parses a string into a timestamp. |
| `from_unix(n)` | Converts a numeric Unix timestamp to a datetime value. |
| `add_days(t, n)` | Returns a new timestamp `n` days later. |

## Constants
- `datetime.MONDAY` (0)
- `datetime.TUESDAY` (1)
- `datetime.WEDNESDAY` (2)
- `datetime.THURSDAY` (3)
- `datetime.FRIDAY` (4)
- `datetime.SATURDAY` (5)
- `datetime.SUNDAY` (6)

## Examples

### Formatting a date
```zen
var t = datetime.now()
print(datetime.format(t, "%A, %B %d, %Y")) 
# e.g., Monday, August 31, 2026
```

### Checking for the weekend
```zen
var day = datetime.weekday(datetime.now())
if day == datetime.SATURDAY || day == datetime.SUNDAY {
    print("It is the weekend!")
}
```

## See Also
- [time](time.md) — Lower-level Unix timestamp functions.
