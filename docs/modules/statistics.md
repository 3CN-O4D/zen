# statistics — Statistical functions

The `statistics` module provides basic functions for mathematical statistics of numeric data. It is available globally as `statistics`.

```zen
var data = [1, 2, 2, 3, 3, 3, 4, 4, 5]

print(statistics.mean(data))      # 3
print(statistics.median(data))    # 3
print(statistics.mode(data))      # 3
print(statistics.stdev(data))     # 1.224...
print(statistics.variance(data))  # 1.5
```

## Functions

| Function | Description |
|----------|-------------|
| `mean(data)` | Arithmetic mean ("average") of data. |
| `median(data)` | Middle value of data. |
| `mode(data)` | Most common data point. |
| `sum(data)` | Sum of all elements in the list. |
| `min(data)` | Minimum value in the list. |
| `max(data)` | Maximum value in the list. |
| `stdev(data)` | Sample standard deviation. |
| `variance(data)` | Sample variance. |

## Examples

### Calculating simple averages
```zen
var grades = [85, 90, 78, 92, 88]
var avg = statistics.mean(grades)
print("Class average: ${avg}")
```

### Finding the spread of data
```zen
var temps = [20, 22, 19, 21, 35] # 35 is an outlier
print("Variance: ${statistics.variance(temps)}")
```

## See Also
- [math](math.md) — Basic mathematical functions.
