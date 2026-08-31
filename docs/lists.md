# Lists

Lists are ordered, heterogeneous collections. They are **immutable values**
from the caller's point of view: almost every operation returns a **new**
list rather than modifying the one you call it on.

```zen
var nums = [1, 2, 3]
var more = nums.push(4)     # [1, 2, 3, 4]
print nums                  # [1, 2, 3]   <-- unchanged!
```

> **Mental model:** `list.push(x)` returns a *new list*. If you want the
> "changed" value, bind it: `nums = nums.push(4)`. The global `push(l, x)`
> behaves the same.

## Building lists

```zen
var empty = []
var nums  = [1, 2, 3]
var mixed = [1, "two", true, null, [5], { a: 1 }]
```

Spread (`...`) flattens another list in:

```zen
var a = [1, 2]
var b = [0, ...a, 3]        # [0, 1, 2, 3]
```

Trailing commas are allowed:

```zen
var c = [1, 2, 3,]
```

## Access & iteration

```zen
print nums[0]            # 1
print nums[-1]           # 3   (negative index = from the end)
print nums[10]           # null (out of range is null, not an error)

numbers[i] = 42          # indexed assignment works
print len(nums)          # 3   (len() global)
print nums.len           # 3   (property)
print nums.length()      # 3   (method)
```

Iteration requires a list (`for` over dicts/strings is an error):

```zen
for x in nums { print x }
for i in 0 .. len(nums) { print nums[i] }
for a, b in [[1, 2], [3, 4]] { print a + b }    # multi-variable unpacking
```

No slices (`l[1:3]`) — use `.slice()`:

```zen
num.slice(1)      # from index 1 to end
nums.slice(1, 3)  # indexes 1..2  (end exclusive)
```

## The full method table

All methods are verified against the current runtime.

| Method | Signature | Returns |
|--------|-----------|---------|
| `push` / `append` / `add` | `(item)` | new list with item at the end |
| `unshift` | `(item)` | new list with item at the front |
| `pop` | `()` | the last element (removed), or `null` |
| `shift` | `()` | the first element (removed), or `null` |
| `insert` | `(index, item)` | new list with the item inserted |
| `clear` | `()` | new empty list |
| `join` | `(sep?)` | string of elements joined by `sep` |
| `contains` | `(item)` | `bool` |
| `includes` / `indexOf` / `index_of` | `(item)` | index of first match, or `-1` |
| `first` | `()` | first element or `null` |
| `last` | `()` | last element or `null` |
| `length` | `()` | element count (property form `.len`) |
| `reverse` | `()` | new list, reversed |
| `sort` / `sorted` | `()` | new list sorted as strings |
| `skip` / `drop` | `(n)` | new list without the first `n` |
| `take` | `(n)` | new list of the first `n` |
| `slice` | `(start[, end])` | sub-list |
| `splice` | `(start[, deleteCount, ...items])` | the **removed** items |
| `concat` | `(...listsOrItems)` | concatenated list |
| `flat` / `flatten` | `()` | one level of flattening |
| `compact` | `()` | drops falsy elements |
| `unique` / `uniq` | `()` | deduplicated |
| `chunk` | `(n)` | list of sub-lists of size `n` |
| `zip` | `(otherList)` | list of `[a, b]` pairs |
| `sum` | `()` | sum of numeric elements |
| `copy` | `()` | shallow copy |
| `fill` | `(value)` | new list of `value` repeated `length` times |
| `shuffle` | `()` | random permutation |
| `sample` | `()` | a random element or `null` |
| `map` | `(fn)` | new mapped list |
| `filter` | `(fn)` | elements for which fn is truthy |
| `each` | `(fn)` | `null` (side effects) |
| `reduce` | `(fn[, init])` | accumulated value |
| `find` | `(fn)` | first element passing fn, or `null` |
| `some` | `(fn)` | `bool` — any element passes |
| `every` | `(fn)` | `bool` — all elements pass |

Not available on values: `count()` (`l.count` property works, the call does
not), `min()`/`max()` (use the global `min(l)`/`max(l)`), `remove(item)` (use
`filter`/`splice`), `get(index)` (use `l[i]`).

## The functional-style methods with callbacks

```zen
var nums = [1, 2, 3, 4]

print nums.map((x) => x * 10)              # [10, 20, 30, 40]
print nums.filter((x) => x % 2 == 0)       # [2, 4]
print nums.reduce((a, b) => a + b)         # 10
print nums.reduce((a, b) => a + b, 100)    # 110
print nums.find((x) => x > 2)              # 3
print nums.some((x) => x > 3)              # true
print nums.every((x) => x > 0)             # true

var total = 0
nums.each((x) => { total = total + x })
print total                                # 10
```

Any function value works — `fn`, `lambda`, or arrow:

```zen
fn even(x) { return x % 2 == 0 }
print nums.filter(even)                    # [2, 4]
```

## Functional mutation recipes

Zen's lists are value-like. Translate in-place habits like this:

```zen
# Python:  lst.append(4)
lst = lst.push(4)

# Python:  lst.insert(1, 99)
lst = lst.insert(1, 99)

# Python:  lst.sort()
lst = lst.sorted()

# Python:  lst.clear()
lst = lst.clear()          # or simply: lst = []
```

The global helpers `push(l, x)`, `pop(l)`, `keys(d)`, `values(d)`, `items(d)`,
`has(d, k)`, `slice(l, a, b)` exist too but the method forms are clearer.

## Ranges make lists

```zen
var up      = 0 .. 5        # [0, 1, 2, 3, 4]   (exclusive)
var incl    = 1 -> 5        # [1, 2, 3, 4, 5]   (inclusive)
var down    = 5 .. 1        # [5, 4, 3, 2]      (auto-descends)
var spaced  = range(0, 10, 2)   # [0, 2, 4, 6, 8]
var negstep = range(10, 0, -2)  # [10, 8, 6, 4, 2]
```

`enumerate` pairs each element with its index:

```zen
for pair in enumerate(["a", "b"]) {
    print pair          # [0, a] then [1, b]
}
```

## List comprehensions

Single-`for` comprehensions with an optional filter:

```zen
var squares = [x * x for x in 0 .. 5]
print squares                    # [0, 1, 4, 9, 16]

var evens   = [x for x in 0 .. 10 if x % 2 == 0]
print evens                      # [0, 2, 4, 6, 8]
```

Nested `for` clauses are not supported.

## Nested lists

```zen
var grid = [[1, 2], [3, 4]]
print grid[0][1]                 # 2

var flat = grid.flat()           # [1, 2, 3, 4]  (one level only)
var deep = [1, [2, [3]]].flat()  # [1, 2, [3]]
```

## Common pitfalls

| Mistake | Reality |
|---------|---------|
| `lst.append(4)` then reading `lst` | unchanged — bind the result (`lst = lst.push(4)`) |
| `l.count()` | not a method — use `l.len` |
| `l.min()` | not a method — use global `min(l)` |
| `l[1:3]` | slices unsupported — use `l.slice(1, 3)` |
| `l.remove(item)` | not a method — use `l.filter((x) => x != item)` |
| `for ch in "abc"` | `for` needs a list — use `"abc".split("")` |
| `[1, 2] * 3` | error — repetition only works on strings |
| `l[10]` out of range | returns `null`, not an error |