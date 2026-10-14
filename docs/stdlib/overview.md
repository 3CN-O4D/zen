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
| `base64` | Base64 and URL-safe codecs. |
| `binascii` | Hex and Base64 binary/text conversions. |
| `bisect` | Binary search and sorted insertion. |
| `bytes` | UTF-8 byte helpers. |
| `calendar` | Leap years, weekdays, month calendars. |
| `collections` | Counters, chaining, flattening. |
| `copy` | Shallow (`shallow`) and deep (`deepcopy`) copies. |
| `ffmpeg` | Media probing and conversion (needs the `ffmpeg` binary). |
| `fractions` | Exact rational arithmetic. |
| `functools` | Reduce, partials, memoizing decorators. |
| `glob` | Shell-style path matching. |
| `hashlib` | MD5/SHA/blake2 hashing and PBKDF2. |
| `heapq` | Binary heaps and top-n selection. |
| `ini` | INI/config files with defaults and typed getters. |
| `itertools` | Combinatorics and iterator tools. |
| `json` | JSON parsing and output (shadows the native tier). |
| `logging` | Structured logging with levels and handlers. |
| `math` | Trigonometry, logs, rounding, gcd (shadows the native tier). |
| `nmap` | Port/service discovery. |
| `operator` | First-class operator functions. |
| `pathlib` | Object-oriented paths. |
| `pprint` | Pretty-printing (`pformat`/`pp`). |
| `random` | Seeded randomness and sampling. |
| `requests` | Python-like HTTP requests wrapper. |
| `shutil` | File copying, moving, deletion, lookup. |
| `soup` | BeautifulSoup-like HTML parsing and queries. |
| `statistics` | Means, medians, variance, regression. |
| `string` | String helpers and constants. |
| `sys` | System-specific parameters and functions. |
| `tempfile` | Temporary files and directories. |
| `textwrap` | Wrapping, filling, dedenting, shortening. |
| `uuid` | UUID generation (v1/v3/v4/v5). |

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
- [base64](base64.md) — Base64 codecs.
- [bisect](bisect.md) — Binary search.
- [calendar](calendar.md) — Calendars.
- [collections](collections.md) — Container helpers.
- [copy](copy.md) — Shallow and deep copies.
- [fractions](fractions.md) — Exact rationals.
- [functools](functools.md) — Higher-order functions.
- [glob](glob.md) — Path globbing.
- [hashlib](hashlib.md) — Hashing.
- [heapq](heapq.md) — Binary heaps.
- [ini](ini.md) — INI/config files.
- [itertools](itertools.md) — Iterator tools.
- [json](json.md) — JSON parsing and output.
- [logging](logging.md) — Logging to terminal or files.
- [math](math.md) — Mathematics.
- [operator](operator.md) — Operator functions.
- [pathlib](pathlib.md) — Object-oriented paths.
- [pprint](pprint.md) — Pretty-printing.
- [random](random.md) — Randomness.
- [requests](requests.md) — Simplified HTTP requests.
- [shutil](shutil.md) — File operations.
- [soup](soup.md) — HTML parsing.
- [statistics](statistics.md) — Descriptive statistics.
- [string](string.md) — String utilities.
- [sys](sys.md) — System information and CLI arguments.
- [tempfile](tempfile.md) — Temporary files.
- [textwrap](textwrap.md) — Text wrapping.
- [uuid](uuid.md) — Unique identifiers.
