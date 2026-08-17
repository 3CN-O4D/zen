# Standard Library Overview

Zen ships with standard library files in `lib/`. Use them with `include`:

```
include "lib/str.z"
include "lib/dict.z"
include "lib/browser.z"
```

Note: `list.z` and `test.z` are now loaded automatically on startup.

## Built-in Functions

Available without any `include`:

### range & interval

```
range(5)                    // [0, 1, 2, 3, 4]
range(2, 5)                 // [2, 3, 4]
range(1, 10, 2)             // [1, 3, 5, 7, 9]
interval(0, 5)              // [0, 1, 2, 3, 4]
interval(0, 10, 3)          // [0, 3, 6, 9]
```

There's also a range *operator* that produces inclusive ranges: `1 -> 5` → `[1, 2, 3, 4, 5]`. See the [Operators](../language/operators.md) section.

### Functional Built-ins

```
enumerate(list)              // [[0, "a"], [1, "b"], [2, "c"]]
enumerate(list, 1)           // [[1, "a"], [2, "b"], [3, "c"]]
zip(list1, list2)            // [[1, "x"], [2, "y"]]
map(fn, list)                // apply fn to each item
filter(fn, list)             // keep items where fn returns truthy
reduce(fn, list)             // fold list to single value
reduce(fn, list, initial)    // fold with initial value
```

Also available as list methods: `list.map(fn)`, `list.filter(fn)`, `list.reduce(fn)`.

## Standard Library Files

| File | Description |
|------|-------------|
| [list.z](list.md) | List utilities |
| [str.z](str.md) | String utilities |
| [dict.z](dict.md) | Dict utilities |
| [test.z](test.md) | Testing framework |
| [browser.z](browser.md) | Browser helpers |
