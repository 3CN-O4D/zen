# Decimal Module (`decimal`)

High-precision decimal arithmetic.

```zen
let d = decimal.Decimal("0.1")
let ctx = decimal.getcontext()
decimal.setcontext(ctx)
decimal.localcontext()
```

Rounding constants:
`ROUND_HALF_UP`, `ROUND_HALF_EVEN`, `ROUND_DOWN`, `ROUND_UP`,
`ROUND_CEILING`, `ROUND_FLOOR`, `ROUND_HALF_DOWN`, `ROUND_05UP`
