# Chapter 8: Lists and Arrays

Lists are Zen's ordered, mutable, heterogeneous sequences. They support indexing, slicing, appending, insertion, removal, sorting, comprehensions, and more. This chapter covers lists in depth, including aliasing, mutation, and performance.

## 8.1 List literals

```zen
let empty = []
let nums = [1, 2, 3, 4, 5]
let mixed = [1, "two", 3.0, true, nil]
let nested = [[1, 2], [3, 4], [5, 6]]
let single = [42]
```

Lists can contain any type, including other lists, dicts, functions, and foreign handles. An empty list `[]` is distinct from an empty dict `{}`.

## 8.2 Indexing and slicing

### Indexing

```zen
let nums = [10, 20, 30, 40, 50]

print(nums[0])    # 10
print(nums[2])    # 30
print(nums[-1])   # 50  (last)
print(nums[-2])   # 40  (second to last)
```

Indexing out of bounds raises `IndexError`:

```zen
try {
    print(nums[10])
} catch as e {
    print(e.msg)  # IndexError: index 10 out of bounds for length 5
}
```

### Slicing

`s[a:b]` returns a new list from `a` (inclusive) to `b` (exclusive). Omitting `a` starts at 0; omitting `b` goes to the end. Negative indices work. Bounds are clamped.

```zen
let nums = [10, 20, 30, 40, 50]

print(nums[1:4])   # [20, 30, 40]
print(nums[:3])    # [10, 20, 30]
print(nums[3:])    # [40, 50]
print(nums[-2:])   # [40, 50]
print(nums[:-2])   # [10, 20, 30]
print(nums[0:100]) # [10, 20, 30, 40, 50]  (clamped)
print(nums[-100:2])# [10, 20]             (clamped)
```

Slicing creates a **new list** (a copy). Mutating the slice never affects the original, and vice versa:

```zen
let a = [1, 2, 3]
let b = a[0:2]
b[0] = 99
print(a)  # [1, 2, 3]  (unchanged)
print(b)  # [99, 2]
```

## 8.3 Mutation methods

| Method | Signature | Description | Example |
|--------|-----------|-------------|---------|
| `push(x)` | `list → nil` | Append to end | `nums.push(6)` |
| `pop()` | `list → value` | Remove and return last | `nums.pop()` |
| `pop(i)` | `list → value` | Remove and return index i | `nums.pop(0)` |
| `insert(i, x)` | `list → nil` | Insert at index | `nums.insert(0, 0)` |
| `remove(x)` | `list → nil` | Remove first equal to x | `nums.remove(20)` |
| `clear()` | `list → nil` | Remove all | `nums.clear()` |
| `sort()` | `list → nil` | Sort in place | `nums.sort()` |
| `reverse()` | `list → nil` | Reverse in place | `nums.reverse()` |
| `copy()` | `list → list` | Shallow copy | `nums.copy()` |
| `index(x)` | `list → int` | Index of first x | `nums.index(30)` |
| `count(x)` | `list → int` | Count occurrences of x | `nums.count(20)` |
| `extend(list)` | `list → nil` | Extend with another list | `nums.extend([6, 7])` |
| `len(list)` | builtin | Length (not a method) | `len(nums)` |

### Examples

```zen
let nums = [3, 1, 4, 1, 5]

nums.push(9)
print(nums)   # [3, 1, 4, 1, 5, 9]

let last = nums.pop()
print(last)   # 9
print(nums)   # [3, 1, 4, 1, 5]

nums.insert(0, 0)
print(nums)   # [0, 3, 1, 4, 1, 5]

nums.remove(4)
print(nums)   # [0, 3, 1, 1, 5]

nums.sort()
print(nums)   # [0, 1, 1, 3, 5]

nums.reverse()
print(nums)   # [5, 3, 1, 1, 0]

print(nums.index(3))  # 1
print(nums.count(1))  # 2
print(len(nums))      # 5

nums.clear()
print(nums)   # []
```

### `pop` behavior

```zen
let nums = [10, 20, 30]

print(nums.pop())   # 30  (removes last)
print(nums)         # [10, 20]

print(nums.pop(0))  # 10  (removes first)
print(nums)         # [20]
```

`pop()` with no index removes the last element (O(1)). `pop(i)` removes at index i (O(n) because elements after i shift).

### `remove` removes only the first match

```zen
let nums = [1, 2, 1, 2, 1]
nums.remove(1)
print(nums)   # [2, 1, 2, 1]  (only first 1 removed)
```

If the value is not present, `remove` raises `ValueError`.

## 8.4 List operations

### Concatenation and repetition

```zen
let a = [1, 2]
let b = [3, 4]

print(a + b)    # [1, 2, 3, 4]
print([0] * 3)  # [0, 0, 0]
print(3 * [0])  # [0, 0, 0]
```

`+` between lists concatenates (creates a new list). `*` with an integer repeats. `*` is commutative for lists: `[0] * 3` and `3 * [0]` both work.

### Membership

```zen
let nums = [1, 2, 3]

print(2 in nums)   # true
print(5 in nums)   # false
print(5 !in nums)  # true
```

Membership uses `==` comparison. For lists of lists, `==` is order-insensitive for dicts but order-sensitive for lists:
```zen
print([1, 2] in [[1, 2], [3, 4]])   # true
print([2, 1] in [[1, 2], [3, 4]])   # false
```

## 8.5 List comprehension

Zen supports list comprehensions with `for` and optional `if`:

```zen
let squares = [x * x for x in 1..5]
print(squares)  # [1, 4, 9, 16, 25]

let evens = [x for x in 1..10 if x % 2 == 0]
print(evens)    # [2, 4, 6, 8, 10]

let words = ["Zen", "lang", "script"]
let lengths = [len(w) for w in words]
print(lengths)  # [3, 4, 6]
```

Syntax: `[expr for var in iterable]` or `[expr for var in iterable if condition]`.

The iterable can be a list, range, string, dict (iterates keys), or any iterable value.

### Nested comprehensions

```zen
let nested = [[1, 2], [3, 4], [5, 6]]
let flat = [x for sub in nested for x in sub]
print(flat)  # [1, 2, 3, 4, 5, 6]
```

The order of `for` clauses matches nesting order: outer `for` first, inner `for` second.

## 8.6 Common patterns

### Sum, min, max

```zen
let nums = [5, 2, 8, 1, 9]

let total = 0
for n in nums {
    total = total + n
}
print(total)  # 25

# min / max via sort
let sorted = nums.copy()
sorted.sort()
print(sorted[0])   # 1 (min)
print(sorted[-1])  # 9 (max)
```

### Filter and map

```zen
let nums = [1, 2, 3, 4, 5, 6]

# Filter
let evens = [n for n in nums if n % 2 == 0]
print(evens)  # [2, 4, 6]

# Map
let doubled = [n * 2 for n in nums]
print(doubled)  # [2, 4, 6, 8, 10, 12]

# Filter + map
let even_doubled = [n * 2 for n in nums if n % 2 == 0]
print(even_doubled)  # [4, 8, 12]
```

### Flatten nested lists

```zen
let nested = [[1, 2], [3, 4], [5, 6]]
let flat = [x for sub in nested for x in sub]
print(flat)  # [1, 2, 3, 4, 5, 6]
```

### Find first match

```zen
let words = ["apple", "banana", "cherry"]
var found = nil
for w in words {
    if w.starts_with("b") {
        found = w
        break
    }
}
print(found)  # banana
```

### Remove all matching elements

```zen
let nums = [1, 2, 3, 2, 4, 2, 5]
let filtered = [n for n in nums if n != 2]
print(filtered)  # [1, 3, 4, 5]
```

### Zip (manual)

Zen has no built-in `zip`. Implement manually:

```zen
let keys = ["a", "b", "c"]
let vals = [1, 2, 3]

let pairs = []
for i in 0..len(keys) {
    pairs.push(#(keys[i], vals[i]))
}
print(pairs)  # [#("a", 1), #("b", 2), #("c", 3)]
```

## 8.6 Sorting

```zen
let nums = [3, 1, 4, 1, 5]

nums.sort()
print(nums)   # [1, 1, 3, 4, 5]

nums.reverse()
print(nums)   # [5, 4, 3, 1, 1]
```

`sort()` sorts in place using the elements' `==`/`<` ordering. For mixed-type lists, sort behavior is undefined (may raise TypeError). `sort()` is stable.

Sorting strings:
```zen
let words = ["banana", "apple", "cherry"]
words.sort()
print(words)   # ["apple", "banana", "cherry"]
```

There is no `sort(key=...)` or `reverse=` parameter. To sort descending, sort then `reverse()`. To sort by a key, precompute keys or use a decorator-sort-undecorate pattern.

## 8.7 Common pitfalls

- **Lists are mutable.** `b = a` makes `b` an alias — changes to `a` affect `b`. Use `a.copy()` for an independent copy.
- **`pop()` with no index** removes the last element (fast). `pop(0)` removes the first (slow, O(n) shift).
- **`remove(x)` removes only the first** occurrence of `x`. Raises `ValueError` if not found.
- **Slicing creates a new list.** `nums[1:3]` is a copy, not a view.
- **List comprehensions create new lists.** They don't modify the original.
- **`sort()` and `reverse()` mutate in place** and return `nil`. Don't write `let s = nums.sort()` expecting `s` to be the sorted list — `s` will be `nil`.
- **`index(x)` raises `ValueError`** if x is not present.
- **Mixed-type lists** may not sort or compare meaningfully.

## 8.8 Quick reference

```zen
let nums = [3, 1, 4, 1, 5]

len(nums)           # 5
nums[0]             # 3
nums[-1]            # 5
nums[1:3]           # [1, 4]
nums.push(9)        # [3, 1, 4, 1, 5, 9]
nums.pop()          # 9
nums.pop(0)         # 3
nums.insert(0, 0)   # [0, 3, 1, 4, 1, 5]
nums.remove(4)      # [0, 3, 1, 1, 5]
nums.sort()         # [0, 1, 1, 3, 5]
nums.reverse()      # [5, 3, 1, 1, 0]
nums.clear()        # []
nums.copy()         # shallow copy
nums.index(3)       # position of 3
nums.count(1)       # 2
nums.extend([6, 7]) # [.., 6, 7]
2 in nums           # true
[x * x for x in nums]  # list comprehension
[x for x in nums if x > 2]  # filtered comprehension
```

## 8.9 Exercises

1. Write `remove_duplicates(lst)` that returns a list with duplicates removed while preserving order.
2. Write `flatten(nested)` that flattens a list of lists one level deep.
3. Write `rotate(lst, k)` that rotates a list right by `k` positions.
4. Write `interleave(a, b)` that alternates elements from two lists.
5. Write `chunk(lst, size)` that splits a list into chunks of `size`.
6. Write `running_total(lst)` that returns a list of running sums.
7. Write `is_sorted(lst)` that checks if a list is in non-decreasing order.
8. Write `second_largest(lst)` that returns the second-largest distinct value.
9. Write `merge_sorted(a, b)` that merges two sorted lists into one sorted list.
10. Demonstrate aliasing: create `a = [1,2,3]`, `b = a`, mutate `a`, show `b` changed; then do the same with `.copy()` and show `b` unchanged.
