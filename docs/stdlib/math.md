# math — mathematics

Pure-Zen math shadowing the native tier (`math.native.*` stays reachable).
Constants `pi`, `e`, `inf`, `nan`; note that `min`/`max`/`fsum`/`prod`/
`hypot` take up to two arguments (each a number or a list).

```zen
import math

print math.gcd(12, 18)  # 6
```

## Functions

| Function | Description |
|----------|-------------|
| `sqrt(x)` / `pow(a, b)` / `exp(x)` | Roots, powers, exponentials. |
| `floor(x)` / `ceil(x)` / `trunc(x)` / `round(x)` | Rounding. |
| `log(x, base)` / `log2(x)` / `log10(x)` | Logarithms (`base` defaults to `e`). |
| `sin(x)` / `cos(x)` / `tan(x)` / `asin(x)` / `acos(x)` / `atan(x)` / `atan2(y, x)` | Trigonometry (radians). |
| `degrees(x)` / `radians(x)` | Angle conversion. |
| `abs(x)` | Absolute value. |
| `gcd(a, b)` | Greatest common divisor. |

## Example

```zen
import math

print math.pi                # 3.141592653589793
print math.floor(2.9)        # 2
print math.gcd(48, 180)      # 12
print math.degrees(math.pi)  # 180
```
