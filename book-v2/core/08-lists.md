# Chapter 8: Lists and Arrays

Lists are Zen's ordered, mutable, heterogeneous sequences. They support
indexing, slicing, appending, insertion, removal, sorting, and more. This
chapter covers lists in depth.

## 8.1 List literals

```zen
let empty = []
let nums = [1, 2, 3, 4, 5]
let mixed = [1, "two", 3.0, true, nil]
let nested = [[1, 2], [3, 4], [5, 6]]
```

Lists can contain any type, including other lists and dicts.

## 8.2 Indexing and slicing

```zen
let nums = [10, 20, 30, 40, 50]

print(nums[0])    # 10
print(nums[2])    # 30
print(nums[-1])   # 50 (last)
print(nums[-2])   # 40

# Slicing [start:end]
print(nums[1:4])  # [20, 30, 40]
print(nums[:3])   # [10, 20, 30]
print(nums[3:])   # [40, 50]
print(nums[-2:])  # [40, 50]
print(nums[:-2])  # [10, 20, 30]
```

## 8.3 Mutation methods

| Method | Description | Example |
|--------|-------------|---------|
| `push(x)` | Append to end | `nums.push(6)` |
| `pop()` | Remove and return last | `nums.pop()` |
| `pop(i)` | Remove and return index i | `nums.pop(0)` |
| `insert(i, x)` | Insert at index | `nums.insert(0, 0)` |
| `remove(x)` | Remove first equal to x | `nums.remove(20)` |
| `clear()` | Remove all | `nums.clear()` |
| `sort()` | Sort in place | `nums.sort()` |
| `reverse()` | Reverse in place | `nums.reverse()` |
| `copy()` | Shallow copy | `nums.copy()` |
| `index(x)` | Index of first x | `nums.index(30)` |
| `count(x)` | Count occurrences of x | `nums.count(20)` |
| `extend(list)` | Extend with list | `nums.extend([6, 7])` |
| `len(list)` | Length (builtin) | `len(nums)` |

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
```

## 8.4 List operations

### Concatenation and repetition

```zen
let a = [1, 2]
let b = [3, 4]

print(a + b)    # [1, 2, 3, 4]
print([0] * 3)  # [0, 0, 0]
```

### Membership

```zen
let nums = [1, 2, 3]
print(2 in nums)   # true
print(5 in nums)   # false
print(5 !in nums)  # true
```

### List comprehension

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

## 8.5 Common patterns

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

## 8.6 Common pitfalls

- **Lists are mutable.** Assigning `b = a` makes `b` an alias — changes to
  `a` affect `b`. Use `a.copy()` for an independent copy.
- **`pop()` with no index** removes the last element. `pop(0)` removes the
  first (slow for large lists).
- **`remove(x)` removes only the first** occurrence of `x`.
- **Slicing creates a new list.** `nums[1:3]` is a copy, not a view.
- **List comprehensions create new lists.** They don't modify the original.

## 8.7 Quick reference

```zen
let nums = [3, 1, 4, 1, 5]

len(nums)           # 5
nums[0]             # 3
nums[-1]            # 5
nums[1:3]           # [1, 4]
nums.push(9)        # [3, 1, 4, 1, 5, 9]
nums.pop()          # 9
nums.insert(0, 0)   # [0, 3, 1, 4, 1, 5]
nums.remove(4)      # [0, 3, 1, 1, 5]
nums.sort()         # [0, 1, 1, 3, 5]
nums.reverse()      # [5, 3, 1, 1, 0]
nums.clear()        # []
nums.copy()         # shallow copy
nums.index(3)       # position of 3
nums.count(1)       # 2
2 in nums           # true
[x * x for x in nums]  # list comprehension
```

## 8.8 Exercises

1. Write `remove_duplicates(lst)` that returns a list with duplicates removed
   while preserving order.
2. Write `flatten(nested)` that flattens a list of lists one level deep.
3. Write `rotate(lst, k)` that rotates a list right by `k` positions.
4. Write `interleave(a, b)` that alternates elements from two lists.
5. Write `chunk(lst, size)` that splits a list into chunks of `size`.
