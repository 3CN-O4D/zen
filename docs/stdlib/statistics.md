# statistics — descriptive statistics

Statistics in the spirit of Python's `statistics` (plus `gmean`/`hmean`
aliases).

```zen
import statistics

print statistics.mean([1, 2, 3, 4])  # 2.5
```

## Functions

| Function | Description |
|----------|-------------|
| `sum` / `min` / `max` | Basic reductions. |
| `mean` / `fmean(data, weights)` / `geometric_mean` / `harmonic_mean(data, weights)` | Averages (`gmean`/`hmean` aliases included). |
| `median` / `median_low` / `median_high` / `median_grouped(data, interval)` | Middles. |
| `mode` / `multimode` | Most common value(s). |
| `pvariance(data, mu)` / `variance(data, xbar)` | Population / sample variance. |
| `pstdev(data, mu)` / `stdev(data, xbar)` | Population / sample deviation. |
| `quantiles(data, n, method)` | Cut points (default quartiles). |
| `correlation(x, y)` / `linear_regression(x, y, proportional)` | Relationships. |

## Example

```zen
import statistics

print statistics.median([3, 1, 2])  # 2
print statistics.stdev([1, 2, 3, 4, 5])
print statistics.mode([1, 2, 2, 3])  # 2
```
