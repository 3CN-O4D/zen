# Chapter 18: collections Module

The `collections` module provides advanced data structures beyond the built-in list and dict: `deque`, `Counter`, `OrderedDict`, `namedtuple`, and `defaultdict`-style helpers.

## 18.1 deque — double-ended queue

```zen
import collections

let d = collections.deque([1, 2, 3])
d.append(4)           # right append
d.appendleft(0)       # left append
print(d.pop())        # 4 (right pop)
print(d.popleft())    # 0 (left pop)
print(d)              # [1, 2, 3]
```

`deque` supports O(1) appends and pops from both ends, making it ideal for queues and sliding windows.

### deque methods

| Method | Description |
|--------|-------------|
| `append(x)` | Add x to the right end |
| `appendleft(x)` | Add x to the left end |
| `pop()` | Remove and return rightmost element |
| `popleft()` | Remove and return leftmost element |
| `len(d)` | Number of elements (builtin) |

```zen
import collections

let d = collections.deque()
d.append(1)
d.append(2)
d.appendleft(0)
print(d)          # [0, 1, 2]
print(d.pop())    # 2
print(d.popleft()) # 0
print(d)          # [1]
```

`pop()` and `popleft()` raise `IndexError` on an empty deque.

```zen
let d = collections.deque()
try {
    d.pop()
} catch as e {
    print(e.msg)  # IndexError: pop from empty deque
}
```

### Use cases

**Queue (FIFO):**
```zen
import collections

let q = collections.deque()
q.append("task1")
q.append("task2")
print(q.popleft())  # "task1"
print(q.popleft())  # "task2"
```

**Stack (LIFO) using right end:**
```zen
let s = collections.deque()
s.append("a")
s.append("b")
print(s.pop())  # "b"
print(s.pop())  # "a"
```

**Sliding window:**
```zen
import collections

func sliding_window_max(nums, k) {
    let dq = collections.deque()
    let result = []
    for i in 0..len(nums) {
        # Remove indices out of window
        if len(dq) > 0 and dq[0] < i - k + 1 {
            dq.popleft()
        }
        # Remove smaller values from right
        while len(dq) > 0 and nums[dq[len(dq)-1]] < nums[i] {
            dq.pop()
        }
        dq.append(i)
        if i >= k - 1 {
            result.push(nums[dq[0]])
        }
    }
    return result
}

print(sliding_window_max([1, 3, -1, -3, 5, 3, 6, 7], 3))
# [3, 3, 5, 5, 6, 7]
```

## 18.2 Counter — count hashable objects

```zen
import collections

let c = collections.Counter(["zen", "lang", "zen", "script", "lang", "zen"])
print(c["zen"])   # 3
print(c["lang"])  # 2
print(c.most_common(2))  # [["zen", 3], ["lang", 2]]
print(list(c.elements()))  # all elements with repetition
```

Counter is useful for frequency counting, voting, and histograms.

### Counter methods

| Method | Description |
|--------|-------------|
| `c[key]` | Count of key (0 if absent) |
| `most_common(n)` | Top n [key, count] pairs |
| `elements()` | Iterator over elements repeating by count |
| `len(c)` | Number of distinct keys |

```zen
import collections

let c = collections.Counter(["a", "b", "a", "c", "b", "a"])
print(c["a"])       # 3
print(c["z"])       # 0 (missing key returns 0, not KeyError)
print(c.most_common(2))  # [["a", 3], ["b", 2]]
print(c.most_common())  # all, sorted by count desc
print(len(c))         # 3 (distinct keys)

# elements()
for e in c.elements() {
    print(e)   # a, a, a, b, b, c (order arbitrary)
}
```

Counter returns 0 for missing keys (unlike dict which raises KeyError):
```zen
let c = collections.Counter(["a"])
print(c["missing"])  # 0
```

### Common patterns

**Word frequency:**
```zen
import collections

let text = "the quick brown fox jumps over the lazy dog"
let words = text.split(" ")
let c = collections.Counter(words)
print(c.most_common(3))  # [["the", 2], ...]
```

**Voting:**
```zen
let votes = ["alice", "bob", "alice", "carol", "alice", "bob"]
let c = collections.Counter(votes)
print(c.most_common(1))  # [["alice", 3]]
print(c.most_common(1)[0][0])  # "alice" (winner)
```

## 18.3 OrderedDict — dict that remembers order

Zen dicts already preserve insertion order, so `OrderedDict` is mostly for API compatibility:

```zen
import collections

let od = collections.OrderedDict()
od["a"] = 1
od["b"] = 2
od["c"] = 3
print(od.keys())  # ["a", "b", "c"]
```

### OrderedDict methods

Same as dict: `keys()`, `values()`, `items()`, `get()`, `has()`, `pop()`, `update()`, `len()`.

```zen
import collections

let od = collections.OrderedDict()
od["first"] = 1
od["second"] = 2
od["third"] = 3

for k in od.keys() {
    print(k)  # first, second, third (insertion order)
}
```

## 18.4 namedtuple — tuple with named fields

```zen
import collections

let Point = collections.namedtuple("Point", ["x", "y"])
let p = Point(3, 4)
print(p.x)   # 3
print(p.y)   # 4
print(p[0])  # 3 (tuple access)
print(p[1])  # 4
print(len(p)) # 2
```

Namedtuples are lightweight, immutable records.

### namedtuple behavior

```zen
import collections

let Person = collections.namedtuple("Person", ["name", "age"])
let p = Person("Zen", 30)

print(p.name)   # "Zen"
print(p.age)    # 30
print(p[0])     # "Zen"  (tuple index access)
print(p[1])     # 30
print(len(p))   # 2

# Namedtuples are immutable
try {
    p.age = 31
} catch as e {
    print(e.msg)  # TypeError or similar (immutable)
}
```

```zen
import collections

let Point = collections.namedtuple("Point", ["x", "y", "z"])
let p = Point(1, 2, 3)
print(p.x, p.y, p.z)  # 1 2 3
print(p[0], p[1], p[2])  # 1 2 3
```

## 18.5 defaultdict — dict with default factory

```zen
import collections

let d = collections.defaultdict(0)  # default int → 0
d["a"] = d["a"] + 1
print(d["a"])  # 1
print(d["b"])  # 0 (default, auto-created)
```

### defaultdict behavior

```zen
import collections

let d = collections.defaultdict(0)
d["a"] = 1
print(d["a"])  # 1
print(d["b"])  # 0  (auto-created on access)
print(d["c"])  # 0
print(d)       # {"a": 1, "b": 0, "c": 0}  (accessing missing keys creates them!)
```

> Warning: accessing a missing key with `d["key"]` **auto-creates** it with the default value. Use `has()` to check without creating:
```zen
import collections

let d = collections.defaultdict(0)
print(d.has("missing"))  # false (doesn't create)
print(d)                 # {}  (still empty)
d["missing"]             # creates it with 0
print(d.has("missing"))  # true
```

The default factory is called with no arguments:
```zen
import collections

let d = collections.defaultdict(list)  # default → []
d["a"].push(1)
print(d["a"])  # [1]
print(d["b"])  # []  (auto-created empty list)
```

```zen
let d = collections.defaultdict(lambda: 0)  # if lambda supported
# or use a constant
let d = collections.defaultdict(0)
```

## 18.6 Quick reference

```zen
import collections

let d = collections.deque([1, 2, 3])
d.append(4)
d.appendleft(0)
d.pop()
d.popleft()

let c = collections.Counter(["a", "b", "a"])
c["a"]   # 2
c.most_common(1)  # [["a", 2]]

let od = collections.OrderedDict()
od["a"] = 1

let Point = collections.namedtuple("Point", ["x", "y"])
let p = Point(1, 2)
p.x  # 1

let dd = collections.defaultdict(0)
dd["missing"]   # 0
```

## 18.7 Exercises

1. Use `collections.deque` to implement a sliding-window average.
2. Use `collections.Counter` to find the most common word in a text.
3. Use `collections.namedtuple` to represent a `Person` with `name` and `age`.
4. Use `collections.defaultdict(list)` to group items by category.
5. Compare `collections.deque` vs list for a queue (push/pop both ends).
6. Write `top_k_words(text, k)` using `collections.Counter`.
7. Write `group_by(items, key_func)` using `collections.defaultdict(list)`.
8. Write a `RingBuffer` class using `collections.deque` with a fixed max size.
