# Chapter 18: collections Module

The `collections` module provides advanced data structures beyond the
built-in list and dict: `deque`, `Counter`, `OrderedDict`, `namedtuple`, and
`defaultdict`-style helpers.

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

`deque` supports O(1) appends and pops from both ends, making it ideal for
queues and sliding windows.

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

## 18.3 OrderedDict — dict that remembers order

Zen dicts already preserve insertion order, so `OrderedDict` is mostly for
API compatibility:

```zen
import collections

let od = collections.OrderedDict()
od["a"] = 1
od["b"] = 2
od["c"] = 3
print(od.keys())  # ["a", "b", "c"]
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

## 18.5 defaultdict — dict with default factory

```zen
import collections

let d = collections.defaultdict(0)  # default int → 0
d["a"] = d["a"] + 1
print(d["a"])  # 1
print(d["b"])  # 0 (default, auto-created)
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
