# Chapter 14: Memory and References

Zen uses manual-ish memory management with explicit reference counting
(`retain`/`release`) rather than a garbage collector. This chapter explains
how values are stored, how references work, and how to avoid leaks.

## 14.1 Value representation

Zen values are 64-bit NaN-boxed tagged unions. There are 16 language types:

- `nil`, `bool`, `int`, `float`, `string`, `bytes`
- `list`, `dict`, `set`, `tuple`
- `func`, `class`, `instance`, `cell`
- `error`, `channel`, `mutex`, `handle`
- `foreign` (Python interop)

Small values (`nil`, `bool`, `int`, `float`) are stored inline. Larger values
(`string`, `list`, `dict`, etc.) are heap objects with a manual refcount.

## 14.2 Reference counting

Every heap object carries a reference count in its header:

- `retain(obj)` — increment the refcount.
- `release(obj)` — decrement the refcount; free when it reaches zero.

Zen's VM inserts `retain`/`release` automatically at assignment, function
calls, and scope exit. You generally don't call them directly unless you're
writing native code.

## 14.3 Ownership and aliasing

Assignment copies the reference, not the value:

```zen
let a = [1, 2, 3]
let b = a          # b aliases the same list
b.push(4)
print(a)           # [1, 2, 3, 4] (a sees the change!)
```

For an independent copy:

```zen
let b = a.copy()
b.push(4)
print(a)           # [1, 2, 3] (unchanged)
print(b)           # [1, 2, 3, 4]
```

Dicts behave the same way:

```zen
let a = {"x": 1}
let b = a
b["y"] = 2
print(a)           # {"x": 1, "y": 2}
```

Strings and numbers are immutable, so aliasing them is safe:

```zen
let a = "hello"
let b = a
# can't modify strings anyway
```

## 14.4 Closures and captured variables

Functions that capture variables from their enclosing scope create shared
`cell` objects:

```zen
func make_counter() {
    var count = 0
    func inc() {
        count = count + 1
        return count
    }
    return inc
}

let c1 = make_counter()
let c2 = make_counter()
print(c1())  # 1
print(c1())  # 2
print(c2())  # 1 (separate capture)
```

Each `make_counter()` call creates a new `count` cell. `c1` and `c2`
capture different cells.

## 14.5 Cycles and leaks

Reference counting cannot collect reference cycles. If object A references B
and B references A, neither will ever be freed:

```zen
# Potential cycle (conceptual)
class Node {
    var next
}
let a = new Node()
let b = new Node()
a.next = b
b.next = a   # cycle: a -> b -> a
```

To avoid cycles:

- Break the cycle explicitly: `a.next = nil` when done.
- Use weak references where supported.
- Prefer tree-like (parent-owns-child) structures over graph-like ones.

## 14.6 Handles and resources

Some native resources (files, sockets, channels, mutexes) use handles —
integer IDs into a resource table. When the Zen value is dropped, the handle
should be closed explicitly:

```zen
import fs

let fd = fs.open("data.txt", "r")
let content = fd.read()
fd.close()   # explicit close
```

Or use `with` for automatic cleanup:

```zen
import fs

with fs.open("data.txt", "r") as f {
    let content = f.read()
}
# f is closed automatically
```

## 14.7 `with` statement

`with` guarantees cleanup when exiting a block, whether normally or via
error:

```zen
with fs.open("out.txt", "w") as f {
    f.write("data")
    throw "oops"   # f still closes
}
```

## 14.8 Step budget (for untrusted code)

Zen supports an instruction budget to guarantee termination:

```zen
// From Rust embedding:
vm.set_step_budget(Some(1000000))
```

If the budget is exhausted, the run aborts with a `NativeError`. This lets
you run untrusted Zen scripts safely.

## 14.9 Common pitfalls

- **Aliasing is the default.** `b = a` shares the object. Use `.copy()`.
- **Cycles leak.** Break reference cycles when done with graph structures.
- **Handles must be closed.** Files, sockets, and channels don't close
  themselves unless you use `with`.
- **Strings are immutable.** Building strings in a loop with `+` creates a
  new rope each time — still O(1) per append due to rope structure, but be
  aware.

## 14.10 Quick reference

```zen
let a = [1, 2, 3]
let b = a              # alias (same object)
let c = a.copy()       # independent copy

a.push(4)              # b sees it, c doesn't

let s = "hello"        # immutable, safe to alias
let t = s

with fs.open("f.txt", "r") as f {
    let data = f.read()   # auto-close
}

vm.set_step_budget(Some(1000000))  # Rust embedding only
```

## 14.11 Exercises

1. Create two lists that alias each other, modify one, and observe the other.
2. Use `.copy()` to make an independent list and confirm the original is
   unchanged.
3. Write a function that takes a list and returns a new list with the same
   elements (without aliasing).
4. Use `with fs.open(...)` to write to a file, then throw an error inside
   the block — confirm the file is still closed.
5. Create a simple linked list class and manually break the cycle when
   disposing of it.

## 14.12 Values, references, and ownership — complete reference

Zen is a garbage-collected / reference-counted language. Values such as ints,
floats, bools, and nil are small and copied by value. Lists, dicts, sets,
strings, bytes, instances, and functions are heap objects referenced by
handles.

### Copying semantics

```zen
let a = [1, 2, 3]
let b = a          # b refers to the SAME list (no copy)
b.push(4)
print(a)           # [1, 2, 3, 4]  (a changed too!)
print(a is b)      # true
```

To copy a list, use `list(a)` or slice `a[:]` (shallow copy):

```zen
let c = list(a)
c.push(5)
print(a)           # [1, 2, 3, 4]  (unchanged)
print(a is c)      # false
```

Note: shallow copy shares nested mutable objects.

### Strings are immutable

```zen
let s = "hello"
let t = s          # same string handle
t = "world"        # rebinding t, not mutating s
print(s)           # "hello"
```

Because strings are immutable, sharing is safe.

### `is` vs `==`

- `==` : structural/value equality
- `is` : identity (same handle/object)

```zen
let x = [1, 2]
let y = [1, 2]
let z = x
print(x == y)   # true
print(x is y)   # false
print(x is z)   # true
```

### Reference counting and lifetime

Heap objects are freed when no references remain. Cycles (e.g. two objects
referencing each other) may leak unless the GC collects cycles. Avoid reference
cycles in long-running programs, or break them explicitly.

```zen
class Node {
    var value = 0
    var parent = nil
    var child = nil
}

let a = new Node()
let b = new Node()
a.child = b
b.parent = a    # cycle: a -> b -> a
```

To break: set `a.child = nil` or `b.parent = nil` when done.

### `shared` — thread-shared mutable state

`shared name = expr` creates an `Arc<Mutex<_>>`-like handle safe to share
across threads. Access requires locking (often automatic via methods).

```zen
shared counter = 0
# threads can safely read/write counter
```

See Chapter 35–38 for concurrency details.

### `let`, `var`, `const`

- `let` : immutable binding (rebind not allowed)
- `var` : mutable binding
- `const` : compile-time/module constant

```zen
let x = 1          # can't reassign x
var y = 2          # y = 3 OK
const MAX = 100    # fixed
```

`let` on a heap object prevents rebinding the variable but NOT mutating the
object:

```zen
let lst = [1, 2]
lst.push(3)   # OK — mutating the object, not rebinding
# lst = [4]   # error — can't rebind let
```

### Move semantics (conceptual)

Some operations "move" ownership (invalidating the source). In Zen 3.0 most
heap values are copied-by-reference, so no explicit move is needed unless a
function consumes the argument.

### Memory leaks to avoid

1. **Reference cycles** — break them when tearing down graphs.
2. **Global caches growing without bound** — use size-limited caches/eviction.
3. **Event listeners/callbacks not removed** — they hold references.
4. **Closures capturing large objects** — the closure keeps the object alive.
5. **Open files/sockets not closed** — use `with` / `finally`.

### Inspecting memory

Some builds expose `sys.getsizeof` or similar; otherwise reason about
references. Use `typeof` to check if a value is a container.

```zen
print(typeof([1,2,3]))   # "list"
print(typeof(42))        # "int"
```

### Performance tips

- Prefer `let` for bindings that don't change (signals intent, may help
  optimizer).
- Pass large containers by reference (they're handles — cheap to pass).
- Avoid copying large lists/dicts in hot loops.
- Reuse buffers instead of allocating repeatedly.
- Null out references to large structures when done (helps the collector).

## 14.13 Exercises

1. Demonstrate that `b = a` shares a list, and `list(a)` copies it.
2. Show that nested lists are shared after a shallow copy.
3. Write `deep_copy` for lists and dicts (manual recursion).
4. Create a reference cycle between two objects and break it.
5. Show `let` prevents rebinding but not mutation.
6. Compare `==` and `is` for two equal lists and two aliases.
7. Write a function that accepts a list and does NOT mutate the caller's list.
8. Demonstrate `shared` usage across two threads (concurrency chapter).
9. Write `clear_all(lst)` emptying a list in place vs rebinding.
10. Explain why `"a" is "a"` may be true (string interning) but
    `[] is []` is false.

