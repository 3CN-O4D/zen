# decimal — Arbitrary-precision arithmetic

The `decimal` module provides support for decimal floating-point arithmetic with configurable precision and rounding. It is available globally as `decimal`.

> **Note:** In the current native runtime, the `Decimal` type returns a dictionary representation. Standard arithmetic operators (`+`, `-`, etc.) may not work directly on these objects yet; use them primarily for formatting or precision-sensitive storage.

```zen
# Create a Decimal
var price = decimal.Decimal("19.99")
print(price) # {value: 19.99, ...}

# Check rounding modes
print(decimal.ROUND_HALF_UP)
```

## Functions & Constants

| Name | Type | Description |
|------|------|-------------|
| `Decimal(value)` | Function | Creates a Decimal representation from a string or number. |
| `getcontext()` | Function | Returns the current decimal context (precision, rounding). |
| `setcontext(ctx)` | Function | Sets the global decimal context. |
| `localcontext()` | Function | A helper for temporary context changes. |
| `ROUND_*` | Constant | Various rounding modes (HALF_UP, FLOOR, CEILING, etc.). |

## Context Management

You can control how decimal math is handled by getting or setting the context.

```zen
var ctx = decimal.getcontext()
ctx.prec = 28  # Set precision to 28 digits
decimal.setcontext(ctx)
```

## Available Rounding Modes
- `decimal.ROUND_FLOOR`
- `decimal.ROUND_CEILING`
- `decimal.ROUND_HALF_UP`
- `decimal.ROUND_HALF_DOWN`
- `decimal.ROUND_HALF_EVEN`
- `decimal.ROUND_UP`
- `decimal.ROUND_DOWN`
- `decimal.ROUND_05UP`

## See Also
- [math](math.md) — Standard floating-point math.
