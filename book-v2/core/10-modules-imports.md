# Chapter 10: Modules and Imports

Zen's module system lets you organize code into reusable files and import
from both pure-Zen modules (`.z`) and native Rust modules. This chapter
covers `import`, `from ... import`, module resolution, `requirements.txt`,
`zen py-sync`, and the `native` tier.

## 10.1 Importing modules

```zen
import fs
import os
import json
import socket
import requests
```

After importing, access members with dot notation:

```zen
import fs

let exists = fs.exists("hello.z")
let content = fs.read("hello.z")
```

### `from ... import`

```zen
from fs import read, write
from json import loads, dumps

print(loads('{"a": 1}'))
write("out.txt", "hello")
```

### `from ... import as`

```zen
import requests as req
import os.path as path

let r = req.get("https://example.com")
print(path.join("a", "b"))
```

## 10.2 Module resolution

When you `import foo`, Zen searches for `foo` in this order:

1. **Native (Rust) modules** — built-in modules like `fs`, `os`, `socket`,
   `requests`, `json`, `crypto`, `tls`, `ffi`, `sys`, `process`, `thread`,
   `channel`, `shared`, `net`, `dns`, `http`, `urllib`, `html`, `lxml`,
   `sqlite`, `qr`, `wa`, `ffmpeg`, `hydra`, `raw`, `tls`.
2. **Pure-Zen `.z` modules** — files in `std/` (the standard library) or in
   your project directory.
3. **Project-relative modules** — files next to your entry script or in
   subdirectories.

### Standard library modules

Zen ships `std/` with modules like:

```
std/argparser.z    std/base64.z    std/binascii.z   std/bytes.z
std/collections.z  std/crunch.z    std/crypto/      std/ffmpeg.z
std/functools.z    std/glob.z      std/hashlib.z    std/itertools.z
std/json.z         std/logging.z   std/math.z       std/nmap.z
std/operator.z     std/pathlib.z   std/random.z     std/shutil.z
std/statistics.z   std/string.z    std/struct.z     std/tempfile.z
std/uuid.z
```

### Subdirectory imports

```zen
import utils.helper
from utils.helper import format_date
```

Zen looks for `utils/helper.z` relative to the entry script's directory and
the search path.

## 10.3 The `native` tier

Modules that ship both a pure-Zen `.z` tier and a native Rust tier expose the
native tier under `<name>.native`:

```zen
import math

print(math.pi)           # pure-Zen tier
print(math.native.pi)    # native Rust tier
```

Force the native tier with `ZEN_NATIVE_ONLY`:

```bash
ZEN_NATIVE_ONLY=math zen run app.z
```

Force the pure-Zen tier with `ZEN_PURE = true` at the top of a `.z` module.

## 10.4 `requirements.txt` and `zen py-sync`

Zen can scan your `.z` files for `pymport` statements and ensure a Python
virtual environment with the needed packages.

Create `requirements.txt` in your project root:

```text
beautifulsoup4
mechanicalsoup
selenium
requests
lxml
```

Run sync:

```bash
zen py-sync
```

Behavior:

1. **Scan** — recursively parses entry + transitive `.z` imports for
   `pymport` statements.
2. **Plan** — determines state (`Clean`, `NoRequirements`, `MissingVenv`,
   `MissingPackages`).
3. **Ensure** — creates `.zen_venv` in project root, runs `pip install -r
   requirements.txt` in the foreground, writes a stamp (requirements length
   + hash).
4. **Point** — `zen run` prepends `.zen_venv/lib/python*/site-packages` to
   `PYTHONPATH`.

Control flags:

- `ZEN_NO_PY_BOOTSTRAP=1` — skip bootstrap entirely (for tests/hermetic runs).
- If `requirements.txt` is missing and `pymport` is used, Zen reports an
  error (it will not auto-install).

Example end-to-end:

```bash
cat > requirements.txt << 'EOF'
paramiko
EOF
zen py-sync
zen run my_script.z  # uses .zen_venv
```

## 10.5 `pymport` — Python interoperability

`pymport` imports Python modules into Zen:

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

Foreign objects (e.g. `datetime.datetime`) are opaque handles — attributes
and methods work, scalar results marshal back to Zen values:

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

## 10.7 Module search path

Zen searches these directories for modules:

1. The directory of the entry script.
2. `std/` (the standard library).
3. Directories in `ZEN_MODULE_PATH` (colon-separated on Linux/macOS,
   semicolon-separated on Windows).

```bash
export ZEN_MODULE_PATH="/home/user/mymodules:/opt/zen/modules"
zen run app.z
```

## 10.8 Circular imports

Zen handles circular imports by returning partially-initialized modules. If
module A imports B and B imports A, B will see A's bindings defined so far.
Avoid circular imports where possible.

## 10.9 Common pitfalls

- **`import` is not hoisted.** Import at the top of the file before use.
- **Module names must be valid identifiers.** `import foo.bar` resolves
  `foo/bar.z`.
- **`pymport` is positional-only.** No keyword arguments.
- **`requirements.txt` maps module names to pip names.** `bs4` →
  `beautifulsoup4`.
- **Native modules have no `.z` source.** `import fs` uses the Rust tier
  directly.

## 10.10 Quick reference

```zen
import fs
import os.path as path
from json import loads, dumps
pymport requests

fs.exists("file.z")
path.join("a", "b")
loads('{"a": 1}')
requests.get("https://example.com")

zen py-sync           # install Python deps
ZEN_NO_PY_BOOTSTRAP=1 # skip bootstrap
math.native.pi        # native tier
```

## 10.11 Exercises

1. Create a `utils.z` module with `format_date`, `slugify`, and `truncate`
   functions. Import and use them from `main.z`.
2. Create `requirements.txt` with `beautifulsoup4`, run `zen py-sync`, and
   write a script that imports `bs4` and parses HTML.
3. Write a script that uses both the pure-Zen `math` tier and
   `math.native` and compares results.
4. Set `ZEN_MODULE_PATH` to a custom directory, place a module there, and
   import it.
5. Use `zen check` and `zen lint` on a project with intentional errors and
   unused variables.
