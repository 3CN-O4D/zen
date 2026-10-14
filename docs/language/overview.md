# Language Overview

Zen is a small, Python-flavoured scripting language: dynamic types, significant
newlines, `//` and `#` comments, first-class functions, classes with implicit
`self`, lists, dicts, strings with interpolation, and exceptions. Programs run
from files (`zen run app.z`), a REPL (`zen`), or one-liners (`zen -e`).

```zen
# hello.z
var name = "Ada"
print "Welcome, ${name}!"

func fib(n) {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}
print fib(20)
```

## Design notes

- **Three scopes, one rule**: locals shadow globals; inside methods, bare
  field names resolve to the instance (implicit `self`), so `name` in a
  method means the field. Locals still shadow fields.
- **Values, not references, across calls**: function and method arguments
  arrive by value — mutating a parameter never touches the caller's copy.
  Mutate through the return value (`let h = heapq.heappush(h, x)`) or through
  object fields and method receivers, which stay shared.
- **Errors are values until thrown**: `try` / `catch` handles them;
  uncaught errors abort with a message and a help hint.
- **Modules two ways**: native modules (`fs`, `http`, `re`, `json`, …) are
  globals — no import needed. Standard-library modules (`std/*.z`) must be
  imported (`import soup`), and their top-level `let` functions stay inside
  the module namespace.

## Next steps

- [Types](types.md) — the value model.
- [Variables](variables.md) — `let` / `var` / `const`.
- [Grammar](../grammar.md) — the full syntax reference.
