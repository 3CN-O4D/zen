# calendar — dates and calendars

Calendar helpers in the spirit of Python's `calendar` (proleptic Gregorian).
Weekdays are Monday = 0 through Sunday = 6.

```zen
import calendar

print calendar.weekday(2024, 1, 1)  # 0 (Monday)
print calendar.month_str(2024, 1)
```

## Functions

| Function | Description |
|----------|-------------|
| `is_leap(year)` | Gregorian leap-year rule. |
| `days_in_month(year, month)` | Month length (29 for February in leap years). |
| `weekday(year, month, day)` | Weekday number, Monday = 0. |
| `month_range(year, month)` | `[weekday of the 1st, days in month]`. |
| `month_calendar(year, month)` | Weeks (Mon…Sun lists); `0` marks out-of-month days. |
| `month_str(year, month)` | Pretty month block (`"January 2024"` header). |
| `yeardays(year)` | 366 or 365. |

## Constants

`DAY_NAMES`, `DAY_ABBR`, `MONTH_NAMES` (1-based, index 0 is `""`),
`MONTH_ABBR`, `MONTH_DAYS` (index 0 is `0`).

## Example

```zen
import calendar

print calendar.month_str(2024, 1)
# January 2024
# Mo Tu We Th Fr Sa Su
#  1  2  3  4  5  6  7
#  8  9 10 11 12 13 14
# 15 16 17 18 19 20 21
# 22 23 24 25 26 27 28
# 29 30 31
```
