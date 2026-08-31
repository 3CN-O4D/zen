# Imports, includes & native calls

Zen gives you four ways to pull in code: `import`/`from ... import` for
modules and the standard library, `include`/`load` for directly executing
another file, and `native func` to call into Rust-native builtins.

## Built-in modules are already global

The Rust-native **modules are auto-registered as globals** — you can use them
without any `import`:

```zen
print(math.pi)          # 3.141592653589793   (no import needed)
print(fs.exists("."))   # true
print(json.stringify({a: 1}))
```

`import math` for these is optional and harmless. `import`/`from ... import`
matter for:

- the Zen **standard library** (`logging`, `requests`, `sys`, `argparser`),
- your **own `.z` modules**, and
- pulling specific names into scope with `from ... import`.

## import — bring in a module

For modules that aren't pre-registered globals (like the `std/` library),
`import` loads the module and binds it to a name:

```zen
import logging
logging.info("hello")
```

Multiple modules in one statement, with `as` aliases (optional):

```zen
import re, http as h
print(h.get("https://example.com"))
```

A dotted name is resolved as a submodule path (`pkg.sub` → `pkg/sub.z` or
`pkg/sub/main.z`):

```zen
import mylib.widgets       # if mylib/widgets.z exists on the module path
```

The module itself is a value — usually a dict (its members are its keys):

```zen
import json
print(json.keys())       # [parse, stringify, encode, load, save, ...]
```

## from ... import — pull names into scope

```zen
from math import pi, sqrt
print(pi)                # 3.141592653589793  (no `math.` prefix)
print(sqrt(9))           # 3
```

Aliases, commas, and star imports:

```zen
from math import sqrt as s, pi
from math import *       # every public name into scope
```

## include / load — execute another file

Both evaluate another Zen source file in the current context, so variables
and functions declared there become visible directly:

```zen
# helpers.z
var greeting = "hello"
func add_one(x) { return x + 1 }

# main.z
include "helpers.z"
print(greeting)                 # hello
print(add_one(5))               # 6
```

`include "path"` takes a file path. `load "name"` resolves a module name the
same way `import` does. Use these to share constants/utilities across the files
of a project.

## Native modules vs your own `.z` modules

- **Native modules** (`math`, `fs`, `re`, `json`, `browser`, ...) are already
  global — `import` is optional.
- **Your own modules** need `import name` (bare name) or `include "path"`.
  `import name` binds a module dict **and** exposes its top-level names as
  globals:

```zen
# helper.z
var helper = 42
func add_one(x) { return x + 1 }

# main.z  (in the same directory)
import helper
print(helper.add_one(5))   # 6    (module namespace: helper.add_one)
print(add_one(5))          # 6    (names also exposed globally)
```

`from helper import name` pulls just specific names:

```zen
from helper import add_one
print(add_one(10))         # 11
```

## native func — call Rust builtins

The runtime exposes many native functions (the language's builtin library).
`native func name(params)` declares one so you can call it like a normal
function:

```zen
native func list_modules()
print(list_modules())    # an in-memory list of every registered module
```

> `native func` requires the parenthesized declaration: `native func list_modules()`,
> not `native func sleep;` or `native sleep`.

## What's available

Two families of importable code exist:

1. **Rust-native modules** (registered at startup, globally available — see
   `help`/REPL `:help modules` / `list_modules()`): `math`, `re`, `http`,
   `json`, `csv`, `fs`, `random`, `datetime`, `time`, `errors`, `browser`,
   `base64`, `base32`, `crypto`, `cryptography`, `hashlib`, `uuid`,
   `itertools`, `collections`, `pathlib`, `shutil`, `glob`, `string`,
   `subprocess`, `struct`, `threading`, `statistics`, `decimal`, `color`,
   `os`, `socket`, `tempfile`, `binascii`, `urllib`, `ftp`, `smtp`, `pop3`,
   `imap`, `telnet`, `dns`, `ssh`, `bluetooth`, `wifi`, `crunch`, `scapy`,
   `wa`, and more.
2. **Zen standard library** in `std/` (resolved by bare name): `argparser`,
   `logging`, `requests`, `sys`.

## Module lookup rules

The resolver searches, in order:

1. Built-in/native modules (registered at startup, already global).
2. The `std/` directory bundled with Zen.
3. The current directory — `name.z` or `name/main.z` (via `import name` or
   `include`).
4. Installed package modules (via `zen pm`).

## Common pitfalls

| Mistake | Reality |
|---------|---------|
| `import os.path` when only `os` is registered | "module not found" — only modules that exist on the path import |
| `import "std/logging.z"` (literal path with `/`) | resolve by bare name instead: `import logging` |
| `import "path.z"` expecting names bound | quoted-path import doesn't bind usable globals — use `import bareName` / `include "path"` |
| `native sleep` (no `func`/parens) | syntax error — must be `native func name(args)` |
| expecting `import` to return a value | it's a statement; the bound name is a value (usually a dict or module) |
| `import *` | unsupported standalone — use `from mod import *` |