# Lists in Zen

Ordered collections with `[]` syntax.

```zen
[1, 2, 3].len()              // 3 (method) or .length (property)
[1, 2, 3].push(4)            // append
[1, 2, 3].pop()              // remove and return last
[1, 2, 3].shift()            // remove first
[1, 2, 3].unshift(0)         // insert at front
[1, 2, 3].contains(2)        // true
[1, 2].join(",")             // "1,2"
[0, ...[1, 2], 3]            // spread: [0, 1, 2, 3]
```
