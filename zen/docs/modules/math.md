# Math Module (`math`)

Standard mathematical functions. No import needed.

Constants: `math.pi`, `math.e`, `math.inf`, `math.nan`.

```zen
math.floor(3.7)              // 3
math.ceil(3.2)               // 4
math.sqrt(16)                // 4.0
math.abs(-5)                 // 5
math.pow(2, 10)              // 1024
math.round(3.5)              // 4
math.min(1, 2, 3)            // 1
math.max(10, 20)             // 20

// Trigonometry
math.sin(math.pi/2)          // 1.0
math.cos(0)                  // 1.0
math.tan(0)                  // 0.0
math.degrees(math.pi)        // 180
math.radians(180)            // 3.14159...

// Logarithms
math.log(100)                // 4.605... (natural log)
math.log10(100)              // 2.0
math.log2(8)                 // 3.0
math.exp(1)                  // 2.718...

// Advanced
math.gcd(48, 18)             // 6
math.lcm(4, 6)               // 12
math.factorial(5)            // 120
math.isnan(math.nan)         // true
math.isinf(math.inf)         // true
math.isfinite(42)            // true
math.fsum([0.1, 0.2])        // 0.30000000000000004 (accurate sum)
```
