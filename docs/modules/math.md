# math — Mathematical functions

The `math` module provides standard mathematical constants and functions. It is available globally as `math`.

```zen
# 1. Constants
print(math.pi) # 3.14159...
print(math.e)  # 2.71828...

# 2. Basic functions
print(math.sqrt(144)) # 12
print(math.pow(2, 8))  # 256
```

## Constants
- `math.pi`
- `math.e`
- `math.inf`
- `math.nan`

## Functions

| Function | Description |
|----------|-------------|
| `sqrt(n)` | Square root. |
| `abs(n)` | Absolute value. |
| `pow(x, y)` | Power (x to the y). |
| `floor(n)` / `ceil(n)` / `round(n)` | Rounding. |
| `trunc(n)` | Truncates decimal part. |
| `sin(n)` / `cos(n)` / `tan(n)` | Trigonometry (radians). |
| `asin(n)` / `acos(n)` / `atan(n)` / `atan2(y, x)` | Inverse trigonometry. |
| `log(n)` / `log10(n)` / `log2(n)` | Logarithms. |
| `exp(n)` | Exponential (e to the n). |
| `gcd(a, b)` / `lcm(a, b)` | Greatest common divisor / least common multiple. |
| `factorial(n)` / `comb(n, k)` / `perm(n, k)` | Combinatorics. |
| `isnan(n)` / `isfinite(n)` / `isinf(n)` | Numeric checks. |
| `min(list)` / `max(list)` | Minimum/maximum in a list. |
| `sum(list)` | Sum of a list. |

## Examples

### Converting between degrees and radians
```zen
var deg = 180
var rad = math.radians(deg) # pi
print(math.degrees(rad))    # 180
```

### Using combinatorics
```zen
# How many ways to pick 2 items from 5?
print(math.comb(5, 2)) # 10
```

## See Also
- [decimal](decimal.md) — For arbitrary-precision math.
- [statistics](statistics.md) — For statistical functions.
