# Standard Library Overview

The Zen Standard Library (`std/`) is a collection of modules written in the
Zen language itself. Unlike the native modules (like `math` or `fs`), these
modules must be **imported** before use.

```zen
import logging
logging.info("Starting script...")

import sys
print(sys.platform)
```

## Available Standard Modules

| Module | Purpose |
|--------|---------|
| `argparser` | Command-line argument parsing (Python-style). |
| `logging` | Structured logging with levels and handlers. |
| `requests` | Python-like HTTP requests wrapper. |
| `sys` | System-specific parameters and functions. |

## How to use

Standard library modules reside in the `std/` directory. When you write
`import logging`, Zen looks for `logging.z` in the `std/` directory bundled
with the binary.

```zen
from argparser import ArgumentParser

var p = ArgumentParser("My Tool")
p.add_argument("--verbose", {action: "store_true"})
var args = p.parse_args()

if args.verbose {
    print("Verbose mode on")
}
```

## Difference from Native Modules

- **Native Modules** (`fs`, `http`, `re`, `json`, ...): Built directly into
  the Rust binary. Available as global variables; no `import` is required.
- **Standard Modules** (`logging`, `sys`, ...): Written in Zen. Reside in
  `std/*.z` files. Must be `import`ed.

## See also

- [argparser](argparser.md) — Parsing command-line flags and arguments.
- [logging](logging.md) — Logging to terminal or files.
- [requests](requests.md) — Simplified HTTP requests.
- [sys](sys.md) — System information and CLI arguments.
