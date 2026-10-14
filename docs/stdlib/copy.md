# copy — shallow and deep copies

Copying in the spirit of Python's `copy`. Lists and dicts duplicate;
primitives (numbers, strings, bools, null) are immutable and returned as-is.
Cyclic structures are not supported by `deepcopy`.

```zen
import copy

let orig = [1, [2]]
let dp = copy.deepcopy(orig)
print dp == orig  # true
```

## Functions

| Function | Description |
|----------|-------------|
| `shallow(x)` | Duplicates one level of a list/dict (nested values stay shared). Named `shallow` because a same-named member would shadow the module on import. |
| `deepcopy(x)` | Duplicates lists/dicts recursively. |
