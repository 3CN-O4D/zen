# dict.z

```
merge({a: 1}, {b: 2})       // {a: 1, b: 2}
pick({a: 1, b: 2, c: 3}, ["a", "c"])  // {a: 1, c: 3}
omit({a: 1, b: 2}, ["a"])   // {b: 2}
invert({a: 1, b: 2})        // {1: "a", 2: "b"}
```
