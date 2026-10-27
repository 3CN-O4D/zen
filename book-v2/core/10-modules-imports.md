# Chapter 10: Modules and Imports

Zen's module system lets you organize code into reusable files and import from both pure-Zen modules (`.z`) and native Rust modules. This chapter covers `import`, `from ... import`, module resolution, `requirements.txt`, `zen py-sync`, and the `native` tier in exhaustive detail.

## 10.1 Importing modules

### Bare import

```zen
import fs
import os
import json
import socket
import requests
import crypto
import tls
import ffi
import sys
import process
import thread
import channel
import shared
import net
import dns
import http
import urllib
import html
import lxml
import sqlite
import qr
import wa
import ffmpeg
import hydra
import raw
```

After importing, access members with dot notation:

```zen
import fs

let exists = fs.exists("hello.z")
let content = fs.read("hello.z")
```

### `from ... import`

Import specific names into the current scope:

```zen
from fs import read, write
from json import loads, dumps

print(loads('{"a": 1}'))
write("out.txt", "hello")
```

Multiple names:
```zen
from fs import read, write, exists, mkdirs
```

### `from ... import as`

Rename on import:

```zen
import requests as req
import os.path as path
import json as j

let r = req.get("https://example.com")
print(path.join("a", "b"))
print(j.loads('{"a": 1}'))
```

### Importing submodules

```zen
import urllib.parse
import urllib.request
import lxml.etree
import net.dns
import crypto.symmetric
import crypto.asymmetric
```

Submodule access uses dot notation:
```zen
let parsed = urllib.parse.urlparse("https://example.com")
let dns_addrs = net.dns.resolve("example.com")
```

## 10.2 Module resolution order

When you `import foo`, Zen searches for `foo` in this order:

### Step 1: Native (Rust) modules

Built-in modules compiled into the Zen binary. These have no `.z` counterpart and are always available:

| Module | Purpose |
|--------|---------|
| `fs` | File system operations |
| `os` | OS operations, env, process info |
| `time` | Time operations |
| `json` | JSON (also has `std/json.z` pure tier) |
| `socket` | TCP/UDP/raw sockets |
| `ffi` | Foreign function interface (C libraries) |
| `sys` | Low-level syscalls |
| `process` | Process spawn/control |
| `thread` | Threading |
| `channel` | Message-passing channels |
| `shared` | Atomic shared values |
| `net` | TCP/UDP helpers |
| `net.dns` | DNS resolution |
| `requests` | HTTP client |
| `http` | HTTP client |
| `http.server` | HTTP static server |
| `urllib` | URL utilities |
| `urllib.parse` | URL parsing |
| `urllib.request` | URL fetching |
| `html` | HTML parsing |
| `html.parser` | HTML parser |
| `lxml` | lxml compatibility |
| `lxml.etree` | XML element tree |
| `lxml.html` | HTML parsing |
| `raw` | Raw socket/packet crafting |
| `crypto` | Cryptography (also has `std/crypto/` pure tier) |
| `tls` | TLS with certificate pinning |
| `sqlite` | SQLite database |
| `qr` | QR code generation |
| `wa` | WebAssembly |
| `ffmpeg` | FFmpeg |
| `hydra` | Hydra |
| `raw` | Raw sockets |

If `foo` is a native module, it is used directly and the search stops.

### Step 2: Pure-Zen `.z` modules

Files in `std/` (the standard library) or in your project directory:

```
std/argparser.z    std/base64.z    std/binascii.z   std/bytes.z
std/collections.z  std/crunch.z    std/crypto/      std/ffmpeg.z
std/functools.z    std/glob.z      std/hashlib.z    std/itertools.z
std/json.z         std/logging.z   std/math.z       std/nmap.z
std/operator.z     std/pathlib.z   std/random.z     std/shutil.z
std/statistics.z   std/string.z    std/struct.z     std/tempfile.z
std/uuid.z
```

### Step 3: Project-relative modules

Files next to your entry script or in subdirectories relative to the entry script's directory.

### Step 4: `ZEN_MODULE_PATH`

Directories in the `ZEN_MODULE_PATH` environment variable (colon-separated on Linux/macOS, semicolon-separated on Windows).

```bash
export ZEN_MODULE_PATH="/home/user/mymodules:/opt/zen/modules"
zen run app.z
```

### Complete resolution order

```
1. Native (Rust) module
2. Pure-Zen .z module (std/ or project dir)
3. Project-relative subdirectory
4. ZEN_MODULE_PATH directories
```

If none match, `Z0324: module 'foo' not found` is raised.

### Subdirectory imports

```zen
import utils.helper
from utils.helper import format_date
```

Zen looks for `utils/helper.z` relative to the entry script's directory and the search path. Nested subdirectories work to any depth:
```zen
import a.b.c.d   # looks for a/b/c/d.z
```

## 10.3 Module tiers: pure-Zen vs native

Modules that ship both a pure-Zen `.z` implementation and a native Rust implementation expose the native tier under `<name>.native`:

```zen
import math

print(math.pi)           # pure-Zen tier
print(math.native.pi)    # native Rust tier
```

### Forcing tiers

**Force native tier** (for all modules that have one):
```bash
ZEN_NATIVE_ONLY=math zen run app.z
```
Multiple modules: `ZEN_NATIVE_ONLY=math,json,socket`

**Force pure-Zen tier:**
Set `ZEN_PURE = true` at the top of a `.z` module:
```zen
// std/math.z
ZEN_PURE = true
```
This makes the pure-Zen tier win over the native tier for that module. The native members remain reachable as `math.native.*`.

### Tier selection logic

The linker decides per-module:

| Condition | Result |
|-----------|--------|
| `ZEN_NATIVE_ONLY` names module | Native tier (no `.z` lookup) |
| `.z` exists AND `ZEN_PURE = true` | Pure-Zen tier, native registered as `.native` |
| `.z` exists, no `ZEN_PURE` | Pure-Zen tier wins; native registered as `.native` |
| No `.z`, native exists | Native tier directly |
| Neither exists | `Z0324: module not found` |

### Example: comparing tiers

```zen
import math

let pure_pi = math.pi
let native_pi = math.native.pi

print(pure_pi == native_pi)  # true (same value)
```

```bash
# Compare behavior under each tier
zen run app.z                    # pure-Zen math tier
ZEN_NATIVE_ONLY=math zen run app.z  # native math tier
```

## 10.4 `requirements.txt` and `zen py-sync`

Zen can scan your `.z` files for `pymport` statements and ensure a Python virtual environment with the needed packages.

### Create `requirements.txt`

Put it in your project root (or nearest to the entry). Map modules to pip package names where they differ:

```text
beautifulsoup4
mechanicalsoup
selenium
requests
lxml
```

> A module name is not always the pip name (e.g., `bs4` comes from `beautifulsoup4`). The scanner reports what was found; Zen does **not** guess package names.

### Run sync

```bash
zen py-sync
```

### Behavior

1. **Scan**: recursively parses entry + transitive `.z` imports to find all `pymport` statements.
2. **Plan**: determines state (`Clean`, `NoRequirements`, `MissingVenv`, `MissingPackages`).
3. **Ensure**: creates `.zen_venv` in project root, runs `pip install -r requirements.txt` in the foreground, writes a stamp (requirements length + hash).
4. **Point**: `zen run` prepends `.zen_venv/lib/python*/site-packages` to `PYTHONPATH`.

### Control flags

| Flag | Meaning |
|------|---------|
| `ZEN_NO_PY_BOOTSTRAP=1` | Skip bootstrap entirely (for tests/hermetic runs). Runtime will still fail if packages are missing. |
| (none) | Normal bootstrap; if `requirements.txt` is missing and `pymport` is used, Zen reports an error (it will not auto-install). |

### Example end-to-end

```bash
cat > requirements.txt << 'EOF'
paramiko
EOF
zen py-sync
zen run my_script.z  # uses .zen_venv
```

## 10.5 `pymport` — Python interoperability

`pymport` imports Python modules into Zen. Covered in depth in [Chapter 42](../interop/42-pymport-python-interoperability.md). Quick summary:

```zen
pymport json
pymport os
pymport requests
pymport bs4
pymport selenium.webdriver as webdriver
pymport urllib.request
pymport builtins
```

Calls are **positional-only** (no keyword arguments):

```zen
pymport json
let data = json.loads('{"a": 1}')
print(data["a"])

pymport os
print(os.getcwd())
print(os.path.join("a", "b"))
```

Foreign objects (e.g. `datetime.datetime`) are opaque handles — attributes and methods work, scalar results marshal back to Zen values:

```zen
pymport datetime
let now = datetime.datetime.now()
print(now.year)
print(now.isoformat())
print(builtins.len([1, 2, 3]))
```

Python exceptions become catchable Zen errors:

```zen
try {
    json.loads("{bad}")
} catch as e {
    print("caught: " + e.msg)
}
```

## 10.6 Writing your own modules

Create `mymod.z`:

```zen
// mymod.z
let VERSION = "1.0.0"

func add(a, b) {
    return a + b
}

func greet(name) {
    return "Hello, " + name
}
```

Import it:

```zen
import mymod

print(mymod.VERSION)   # 1.0.0
print(mymod.add(2, 3)) # 5
print(mymod.greet("Zen"))  # Hello, Zen
```

Or import specific names:

```zen
from mymod import add, greet
print(add(2, 3))
print(greet("Zen"))
```

### Module execution on import

When a module is imported, its top-level code runs once. The module's exports become its dictionary. Subsequent imports of the same module return the cached module (no re-execution):

```zen
// counter.z
let count = 0
func inc() { count = count + 1; return count }
```

```zen
import counter
import counter   // does NOT re-run; count is still 0
print(counter.inc())  # 1
print(counter.inc())  # 2
```

## 10.7 Module search path

Zen searches these directories for modules:

1. **The directory of the entry script.**
2. **`std/`** (the standard library).
3. **Project-relative subdirectories** (relative to entry script).
4. **Directories in `ZEN_MODULE_PATH`** (colon-separated on Linux/macOS, semicolon-separated on Windows).

```bash
export ZEN_MODULE_PATH="/home/user/mymodules:/opt/zen/modules"
zen run app.z
```

### Example: custom module path

```bash
# Create a module in a custom directory
mkdir -p /home/user/mymodules
echo 'func hello() { return "hi from custom" }' > /home/user/mymodules/custom.z

ZEN_MODULE_PATH="/home/user/mymodules" zen run -e 'import custom; print(custom.hello())'
# hi from custom
```

## 10.8 Circular imports

Zen handles circular imports by returning partially-initialized modules. If module A imports B and B imports A, B will see A's bindings defined so far. Avoid circular imports where possible.

```zen
// a.z
import b
let a_val = 1
func a_func() { return b.b_val }

// b.z
import a
let b_val = 2
func b_func() { return a.a_val }  // a.a_val may be nil if a hasn't finished
```

## 10.9 `include` directive

Zen has an `include` keyword (token `KwInclude`) that inlines another file's contents at the point of the directive. This is different from `import` — it's textual inclusion, not module scoping.

```zen
include "utils.z"
```

Use `include` for code generation or when you want to literally splice file contents. Prefer `import` for normal modularity.

## 10.10 Common pitfalls

- **`import` is not hoisted.** Import at the top of the file before use.
- **Module names must be valid identifiers.** `import foo.bar` resolves `foo/bar.z`.
- **`pymport` is positional-only.** No keyword arguments.
- **`requirements.txt` maps module names to pip names.** `bs4` → `beautifulsoup4`.
- **Native modules have no `.z` source.** `import fs` uses the Rust tier directly.
- **Circular imports** return partially-initialized modules; avoid them.
- **`ZEN_MODULE_PATH`** must be set before running `zen`, not inside the script.

## 10.10 Quick reference

```zen
import fs
import os.path as path
from json import loads, dumps
pymport requests
import utils.helper

fs.exists("file.z")
path.join("a", "b")
loads('{"a": 1}')
requests.get("https://example.com")

zen py-sync           # install Python deps
ZEN_NO_PY_BOOTSTRAP=1 # skip bootstrap
math.native.pi        # native tier
ZEN_NATIVE_ONLY=math  # force native tier
```

## 10.11 Exercises

1. Create a `utils.z` module with `format_date`, `slugify`, and `truncate` functions. Import and use them from `main.z`.
2. Create `requirements.txt` with `beautifulsoup4`, run `zen py-sync`, and write a script that imports `bs4` and parses HTML.
3. Write a script that uses both the pure-Zen `math` tier and `math.native` and compares results.
4. Set `ZEN_MODULE_PATH` to a custom directory, place a module there, and import it.
5. Use `zen check` and `zen lint` on a project with intentional errors and unused variables.
6. Create two modules that import each other (circular) and observe the behavior.
7. Use `include` to splice a file's contents and compare with `import`.

## 10.12 Modules and imports — complete reference

Zen's module system maps `import a.b.c` to `a/b/c.z` or `a/b/c/mod.z`.
The `mod.z` file is the canonical package entry point; `main.z` is deprecated.

### Import forms

| Form | Meaning |
|------|---------|
| `import module` | bind module namespace |
| `import module as alias` | shorten the name |
| `import mod.sub.mod as alias` | nested module |
| `from module import name, name as alias` | bind specific members |
| `from module import *` | import all public members |

### Visibility

`pub` prefix on `func`, `var`, `const`, `class` exports the symbol.
Non-`pub` is private to the file.

```zen
# utils.z
pub func add(a, b) { return a + b }   # exported
func helper() { return 42 }            # private

# main.z
import utils
print(utils.add(2, 3))   # 5
# utils.helper()  # error: not exported
```

### Module resolution order

1. Standard library modules (e.g. `math`, `json`, `socket`)
2. Local files relative to the importing file / project root
3. `ZEN_PATH` entries (colon-separated)
4. Site-packages / installed modules

### Circular imports

Circular imports can cause partially-initialized modules. Design modules as
DAGs; put shared definitions in a third module imported by both.

```zen
# a.z imports b.z; b.z imports a.z  -> avoid
```

### Reloading and caching

Modules are typically loaded once and cached. Re-importing returns the cached
module. To pick up changes, restart the process.

### `ZEN_PATH`

Colon-separated list of directories searched for user modules:

```bash
export ZEN_PATH="$HOME/zen/lib:/opt/zen/modules"
zen run app.z
```

### `__name__`

A module's `__name__` (if exposed) is `"__main__"` when run directly and the
module path when imported. Use it for guard clauses:

```zen
# entry.z
func main() { print("running directly") }
if __name__ == "__main__" { main() }
```

### Packages

A directory with `mod.z` is a package:

```
mypkg/
    mod.z        # package entry point
    sub/
        mod.z    # mypkg.sub
```

Import: `import mypkg`, `import mypkg.sub`.

### stdlib module list (selected)

- math, random, string, collections, json, datetime/time
- pathlib, os, re, base64, binascii, hashlib, crypto
- socket, http, requests, dns, url
- threading, channels, mutexes
- ffi, sys, pymport

See Appendix D for the full index.

## 10.13 Writing your own modules

```zen
# geometry.z
pub const PI = 3.141592653589793

pub func circle_area(r) {
    return PI * r * r
}

pub func circle_circumference(r) {
    return 2 * PI * r
}

func _internal() { return "not exported" }
```

```zen
import geometry
print(geometry.circle_area(5))   # 78.5398...
```

### Module docstrings / documentation

Place a comment at the top of each `.z` file describing purpose, exports, and
usage. Some tooling may extract leading comments as docs.

## 10.14 Import performance

- Import once at the top of the file.
- Avoid imports inside hot loops.
- Local (relative) imports resolve faster than deep stdlib lookups.

## 10.15 Common import mistakes

1. **Missing `pub`** means the symbol is unavailable to importers.
2. **Wrong path:** `import foo.bar` looks for `foo/bar.z` or `foo/bar/mod.z`.
3. **Name shadowing:** naming a local file `math.z` shadows stdlib `math`.
4. **Circular imports** lead to partially initialized modules.
5. **Assuming re-import re-runs code.** Modules are cached.
6. **`from module import *`** brings only `pub` members; be explicit.

## 10.16 Exercises

1. Create a `utils.z` with `pub func` helpers and import it in `main.z`.
2. Build a `geometry` package with `mod.z` and `sub/area.z`.
3. Demonstrate `import module as alias` shortening a long name.
4. Use `from module import name` to bind `math.sin` locally.
5. Show that two files importing the same module share state.
6. Write a module with a `pub const` and read it from another file.
7. Demonstrate `if __name__ == "__main__"` guard behavior.
8. Set `ZEN_PATH` and import a module from a non-default directory.
9. Create a name-shadowing file `json.z` and observe stdlib being hidden.
10. Write a module that exports both functions and constants; document each.

