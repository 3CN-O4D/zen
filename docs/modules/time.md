# time — Time and timestamps

The `time` module provides functions for working with Unix timestamps and formatting dates. It is available globally as `time`.

```zen
# 1. Get current Unix timestamp (seconds)
var t = time.now()
print(t)  # e.g., 1724248800

# 2. Format a timestamp
print(time.format(t, "%Y-%m-%d %H:%M:%S"))

# 3. Pause execution
time.sleep(1.5)  # Pause for 1.5 seconds
```

## Functions

| Function | Description |
|----------|-------------|
| `now()` | Returns the current Unix timestamp (float). |
| `unix()` | Alias for `now()`. |
| `utc()` | Returns the current UTC timestamp. |
| `format(t, fmt?)` | Formats a timestamp `t` using a strftime string. |
| `parse(str, fmt)` | Parses a date string into a timestamp. |
| `year(t)` / `month(t)` / `day(t)` | Extracts components from a timestamp. |
| `hour(t)` / `minute(t)` / `second(t)` | Extracts time components. |
| `weekday(t)` | Returns the day of the week (0-6, Monday is 0). |
| `sleep(secs)` | Pauses execution for N seconds (float). |
| `wait(ms)` | Pauses execution for N milliseconds. |

## Examples

### Measuring execution time
```zen
var start = time.now()
# ... do work ...
var end = time.now()
print("Elapsed: ${end - start} seconds")
```

### Working with dates
```zen
var t = time.now()
print("Year: ${time.year(t)}")
print("Month: ${time.month(t)}")
```

## See Also
- [datetime](datetime.md) — Higher-level date objects.
- [builtins](../builtins.md) — For global `sleep()` and `wait()`.
