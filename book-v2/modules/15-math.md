# Chapter 15: math Module

The `math` module provides mathematical functions and constants. It ships as
a pure-Zen implementation (`std/math.z`) with a native Rust tier reachable
via `math.native`. Set `ZEN_PURE = true` to prefer the pure tier; set
`ZEN_NATIVE_ONLY=math` to force the native tier.

## 15.1 Constants

```zen
import math

print(math.pi)     # 3.141592653589793
print(math.e)      # 2.718281828459045
print(math.tau)    # 6.283185307179586
```

## 15.2 Rounding

| Function | Description | Example | Result |
|----------|-------------|---------|--------|
| `floor(x)` | Largest int ≤ x | `math.floor(3.9)` | `3` |
| `ceil(x)` | Smallest int ≥ x | `math.ceil(3.1)` | `4` |
| `trunc(x)` | Truncate toward zero | `math.trunc(-3.9)` | `-3` |
| `round(x)` | Round to nearest | `math.round(3.5)` | `4` |

```zen
import math

print(math.floor(3.9))   # 3
print(math.ceil(3.1))    # 4
print(math.trunc(-3.9))  # -3
print(math.round(3.5))   # 4
print(math.round(2.5))   # 2 (banker's rounding)
```

## 15.3 Powers and logarithms

| Function | Description | Example |
|----------|-------------|---------|
| `pow(x, y)` | x raised to y | `math.pow(2, 3)` → `8` |
| `sqrt(x)` | Square root | `math.sqrt(16)` → `4` |
| `exp(x)` | e raised to x | `math.exp(1)` → `2.718...` |
| `log(x)` | Natural log | `math.log(math.e)` → `1` |
| `log10(x)` | Base-10 log | `math.log10(100)` → `2` |
| `log2(x)` | Base-2 log | `math.log2(8)` → `3` |

```zen
import math

print(math.pow(2, 10))   # 1024
print(math.sqrt(81))     # 9
print(math.exp(0))       # 1
print(math.log(1))       # 0
print(math.log10(1000))  # 3
print(math.log2(1024))   # 10
```

## 15.4 Trigonometry

All angles in radians:

| Function | Description |
|----------|-------------|
| `sin(x)` | Sine |
| `cos(x)` | Cosine |
| `tan(x)` | Tangent |
| `asin(x)` | Arcsine |
| `acos(x)` | Arccosine |
| `atan(x)` | Arctangent |
| `atan2(y, x)` | Arctangent of y/x (correct quadrant) |
| `degrees(x)` | Radians → degrees |
| `radians(x)` | Degrees → radians |

```zen
import math

print(math.sin(math.pi / 2))   # 1
print(math.cos(0))             # 1
print(math.tan(math.pi / 4))   # 1
print(math.degrees(math.pi))   # 180
print(math.radians(180))       # 3.14159...
```

## 15.5 Hyperbolic functions

| Function | Description |
|----------|-------------|
| `sinh(x)` | Hyperbolic sine |
| `cosh(x)` | Hyperbolic cosine |
| `tanh(x)` | Hyperbolic tangent |
| `asinh(x)` | Inverse hyperbolic sine |
| `acosh(x)` | Inverse hyperbolic cosine |
| `atanh(x)` | Inverse hyperbolic tangent |

## 15.6 Other functions

| Function | Description | Example |
|----------|-------------|---------|
| `abs(x)` | Absolute value | `math.abs(-5)` → `5` |
| `fsum(items)` | Accurate float sum | `math.fsum([0.1, 0.2, 0.3])` |
| `prod(items)` | Product of items | `math.prod([1, 2, 3, 4])` → `24` |
| `min(a, b)` | Minimum of two | `math.min(3, 7)` → `3` |
| `max(a, b)` | Maximum of two | `math.max(3, 7)` → `7` |
| `hypot(x, y)` | Euclidean distance | `math.hypot(3, 4)` → `5` |
| `gcd(a, b)` | Greatest common divisor | `math.gcd(48, 18)` → `6` |
| `lcm(a, b)` | Least common multiple | `math.lcm(12, 8)` → `24` |
| `isclose(a, b)` | Approximate equality | `math.isclose(0.1+0.2, 0.3)` |

> Note: In the pure-Zen tier, `min`/`max`/`fsum`/`prod`/`hypot` accept up to
> two arguments (each may be a number or a list). Calls like
> `math.min(1, 2, 3)` are unsupported in the pure tier.

```zen
import math

print(math.fsum([0.1, 0.2, 0.3]))  # 0.6
print(math.prod([1, 2, 3, 4]))     # 24
print(math.min(3, 7))              # 3
print(math.max(3, 7))              # 7
print(math.hypot(3, 4))            # 5
print(math.gcd(48, 18))            # 6
print(math.lcm(12, 8))             # 24
print(math.isclose(0.1 + 0.2, 0.3)) # true
```

## 15.7 Using the native tier

```zen
import math

# Pure-Zen tier
print(math.pi)

# Native Rust tier
print(math.native.pi)
```

Force native tier:

```bash
ZEN_NATIVE_ONLY=math zen run app.z
```

## 15.8 Quick reference

```zen
import math

math.pi          # 3.14159...
math.e           # 2.71828...
math.tau         # 6.28318...

math.floor(3.9)  # 3
math.ceil(3.1)   # 4
math.sqrt(16)    # 4
math.pow(2, 10)  # 1024
math.sin(math.pi / 2)  # 1
math.gcd(48, 18) # 6
math.hypot(3, 4) # 5
```

## 15.9 Exercises

1. Write a function `distance(x1, y1, x2, y2)` using `math.hypot`.
2. Write a function `celsius_to_fahrenheit(c)` and `fahrenheit_to_celsius(f)`.
3. Use `math.gcd` to simplify a fraction `a/b` to lowest terms.
4. Compute the hypotenuse of a right triangle given legs `a` and `b`.
5. Use `math.log10` to compute the number of digits in an integer `n`.
