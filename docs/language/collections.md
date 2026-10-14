# Lists & Dicts

Zen's two workhorse collections live on their own pages:

- [Lists](../lists.md) — literals, indexing, slices, `push`/`pop`, iteration.
- [Dicts](../dicts.md) — literals, lookup, `has`/`get`/`keys`, nesting.

```zen
var xs = [3, 1, 2]
print xs.sort()   # [1, 2, 3] (a new list; xs is unchanged)

var user = {"name": "Ada", "tags": ["admin"]}
print user.get("missing", "n/a")  # n/a
```
