# fractions — exact rationals

Exact rational arithmetic in the spirit of Python's `fractions`.
`Fraction(n, d)` builds a normalized fraction (positive denominator, divided
by the gcd). All arithmetic returns new fractions.

```zen
import fractions

let h = fractions.Fraction(1, 2)
print h.add(fractions.Fraction(1, 3)).show()  # 5/6
```

## Fraction methods

| Method | Description |
|--------|-------------|
| `show()` | `"n/d"` (or `"n"` for whole values). |
| `to_float()` | Floating-point value. |
| `eq(o)` | Exact equality. |
| `add(o)` / `sub(o)` / `mul(o)` / `div(o)` | Arithmetic, normalized. |
| `neg()` / `inv()` | Negation and reciprocal (`inv` of zero throws). |

## Example

```zen
import fractions

let a = fractions.Fraction(2, 4)   # 1/2
let b = fractions.Fraction(1, 3)
print a.add(b).show()  # 5/6
print a.mul(b).show()  # 1/6
print a.eq(b)          # false
```
