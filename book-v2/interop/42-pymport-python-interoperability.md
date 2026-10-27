# Chapter 42: pymport - Python Interoperability

`pymport` imports Python modules into Zen, letting you call Python functions, access Python objects, and use the entire Python ecosystem from Zen. This chapter covers setup, syntax, `zen py-sync`, environment management, limits, examples, and troubleshooting.

## 42.1 Why `pymport` (and not just `import`)?

Zen's `import` loads Zen modules (`.z` files or native Rust modules). `pymport` loads **CPython** modules — the entire Python ecosystem (PyPI, numpy, pandas, requests, beautifulsoup4, etc.).

```zen
import json          // Zen's json module (Zen or native Rust)
pymport json         // CPython's json module
```

## 42.2 Syntax and Binding

`pymport` calls are **positional-only** — no keyword arguments.

```zen
pymport json
pymport os
pymport requests
pymport bs4            // beautifulsoup4
pymport selenium.webdriver as webdriver
pymport urllib.request
pymport builtins
```

### Calling functions

```zen
pymport json
let data = json.loads('{"name": "Zen", "version": 3}')
print(data["name"])     # Zen

pymport os
print(os.getcwd())
print(os.path.join("a", "b"))

pymport datetime
let now = datetime.datetime.now()
print(now.year)
print(now.isoformat())
```

### Attribute access and method calls

Foreign objects (e.g. `datetime.datetime`) are opaque handles — attributes and methods work:

```zen
pymport datetime
let now = datetime.datetime.now()
print(now.year)           // attribute access
print(now.isoformat())    // method call
let delta = datetime.timedelta(days=7)
let later = now + delta   // operator overloading (if supported)
```

Scalar results marshal back to Zen values (int, float, string, bool, list, dict).

## 42.3 Building Zen with Python Support

`pymport` requires Zen built with the Python feature:

```bash
cargo build --release --features python
```

This requires `libpython` at build and run time. Without it, `pymport` reports:

```
pymport needs a linked Python bridge (this build has none). Rebuild with --features python
```

## 42.4 Environment Setup (venv + PYTHONPATH)

You can point CPython to any Python environment via `PYTHONPATH`.

```bash
cd book-v2/tests
./setup.sh          # builds zen --features python, creates .venv, installs requirements

export PYTHONPATH="$(book-v2/tests/.venv/bin/python -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"
export ZEN_NO_PY_BOOTSTRAP=1
./target/release/zen run script.z
```

`ZEN_NO_PY_BOOTSTRAP=1` tells Zen to use that environment as-is instead of creating `.zen_venv`.

### Create your own venv

```bash
python3 -m venv .venv
.venv/bin/pip install beautifulsoup4 mechanicalsoup selenium lxml requests
export PYTHONPATH="$(.venv/bin/python -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"
```

### Check installed packages

```bash
book-v2/tests/.venv/bin/pip list
```

## 42.5 Dependency Sync with `zen py-sync` and `requirements.txt`

Zen can scan your `.z` files for `pymport` statements and ensure a venv with dependencies.

### Create `requirements.txt`

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
./target/release/zen py-sync
```

### Behavior

1. **Scan**: recursively parses entry + transitive `.z` imports to find all `pymport` statements.
2. **Plan**: determines state (`Clean`, `NoRequirements`, `MissingVenv`, `MissingPackages`).
3. **Ensure**: creates `.zen_venv` in project root, runs `pip install -r requirements.txt` in the foreground, writes a stamp (requirements length + hash).
4. **Point**: `zen run` prepends `.zen_venv/lib/python*/site-packages` to `PYTHONPATH`.

### Control flags

| Flag | Meaning |
|------|---------|
| `ZEN_NO_PY_BOOTSTRAP=1` | Skip bootstrap entirely (for tests/hermetic runs). |
| (none) | If `requirements.txt` is missing and `pymport` is used, Zen reports an error. |

### Example end-to-end

```bash
cat > requirements.txt << 'EOF'
paramiko
EOF
./target/release/zen py-sync
./target/release/zen run my_script.z  # uses .zen_venv
```

## 42.6 Using Python from Zen

### Importing modules

```zen
pymport json        // stdlib
pymport os          // stdlib
pymport requests    // third-party (if installed)
pymport bs4         // beautifulsoup4
pymport selenium.webdriver as webdriver
pymport urllib.request
pymport builtins
```

### Calling functions and accessing attributes

Foreign calls are **positional-only** in v1. Use `getattr`-style access via dot notation; `call_attr` fuses resolve+dispatch for methods.

```zen
pymport json
let data = json.loads('{"a": 1}')
print(data["a"])

pymport os
print(os.getcwd())
print(os.path.join("a", "b"))

pymport datetime
let now = datetime.datetime.now()
print(now.year)
print(now.isoformat())
```

### Working with Python objects and binary data

Foreign objects are opaque handles:

```zen
pymport datetime
pymport builtins

let epoch = datetime.datetime.fromtimestamp(0)
print(epoch.year)
print(epoch.isoformat())
print(epoch.date().isoformat())

let later = datetime.datetime(2030, 1, 2, 3, 4, 5)
print(later.strftime("%Y-%m-%d %H:%M:%S"))

print(builtins.len([10, 20, 30]))
print(builtins.sum([10, 20, 30]))
print(builtins.sorted([3, 1, 2]))
```

### Binary data

Python `bytes` are foreign handles. Write them through Python's own file object:

```zen
import fs
pymport mechanicalsoup
pymport builtins

let f = builtins.open("out.bin", "wb")
f.write(some_bytes)
f.close()
```

## 42.7 Error handling

Python exceptions become catchable Zen errors:

```zen
import json as zenjson
pymport json

let bad = false
try {
    json.loads("{not json}")
} catch as e {
    bad = true
    print("caught json error")
}

if !bad {
    print("expected json.loads to fail")
    exit(1)
}

// Missing modules fail at the pymport statement and are catchable too
let missing = false
try {
    pymport this_module_does_not_exist
} catch as e {
    missing = true
    print("caught import error")
}

if !missing {
    print("expected import to fail")
    exit(1)
}

print("recovered: " + str(zenjson.parse("{\"ok\": true}")["ok"]))
```

## 42.8 Limits and Non-Goals (v1)

- **No keyword arguments** in `pymport` calls.
- **No passing Zen closures into Python** (rejected by marshal).
- **No per-handle finalizers**: foreign handles remain until bridge teardown.
- **Tuples → lists**, `big int` → `f64` in some cases, non-string dict keys rejected.
- **Functions containing `PyImport` not JIT-lowered** (stay interpreted).
- **No `zen.toml [py-deps]`** yet (use `requirements.txt`).
- **Network installs** only via `zen py-sync`/preamble; tests avoid silent installs.
- **Handles are opaque** across isolates in general.

## 42.9 Working Examples (tested)

All scripts live in [`book-v2/tests/web-automation/`](../tests/web-automation/). Five smaller, hermetic pymport tests live in [`book-v2/tests/pymport/`](../tests/pymport/).

### Example A: `bs4` + `requests` scrape + download

See [`tests/web-automation/bs4_tiktok.z`](../tests/web-automation/bs4_tiktok.z) — downloads a TikTok embed MP4 via `urllib.request.urlretrieve`, parsed with BeautifulSoup from `__FRONTITY_CONNECT_STATE__`.

### Example B: `mechanicalsoup` stateful scrape + download

See [`tests/web-automation/ms_tiktok.z`](../tests/web-automation/ms_tiktok.z). Uses `StatefulBrowser`, accesses `browser.page`, reuses `browser.session.get()` for CDN, writes bytes via `builtins.open(...,"wb").write()`.

### Example C: Selenium automation + download

See [`tests/web-automation/selenium_tiktok.z`](../tests/web-automation/selenium_tiktok.z). Launches headless Chromium, polls for `__UNIVERSAL_DATA_FOR_REHYDRATION__`, extracts stats, fetches the embed route with plain HTTP for an unsigned MP4 URL, downloads via `urllib.request.urlretrieve`.

### Example D: CDP over raw TCP/WebSocket (no Selenium)

See [`tests/web-automation/cdp_tiktok.z`](../tests/web-automation/cdp_tiktok.z). Launches Chromium with `pymport subprocess`, discovers targets via HTTP `/json/list`, upgrades to WebSocket, sends masked CDP frames via `sys.write_bytes(socket.fileno(sock), hex)`, parses unmasked server frames, runs `Page.navigate` + `Runtime.evaluate` to extract TikTok metadata. Cleans up with `browser.terminate()` in a `finally` block.

Run all of them:

```bash
cd book-v2/tests
./run-all.sh --network
```

## 42.10 Troubleshooting

| Issue | Fix |
|-------|-----|
| `Z0414` / "unknown native" / needs `--features python` | Rebuild with `cargo build --release --features python` and ensure libpython is available. |
| `ModuleNotFoundError` for third-party package | Install into venv, set `PYTHONPATH`, or run `zen py-sync` with `requirements.txt`. |
| Selenium can't find chromedriver | Install `chromedriver` matching Chromium or put it on `PATH`. Use `--no-sandbox` in containers. |
| TikTok WAF blocks plain requests | Use `/embed/v2/<id>`, or Selenium/real browser. See Examples A/C. |
| Binary download corrupted | Don't rely on string-based `fs.write_binary`; use `urllib.request.urlretrieve` or `builtins.open("wb").write()`. |
| WebSocket hang in raw sockets | Read until `Content-Length` for HTTP, handle EAGAIN on keep-alive, mask client frames, parse length (126/127). |
| `pymport` call with kwargs fails | Use positional args only in v1. |
| Bootstrap skipped unexpectedly | Check `ZEN_NO_PY_BOOTSTRAP` or that `requirements.txt` exists near entry. |

## 42.11 References

- Docs: `docs/20-pymport.md` (design/spec)
- VM bridge: `crates/zen-vm/src/pybridge.rs`, `crates/zen-vm/src/pyembed.rs`
- CLI bootstrap: `zen-cli/src/pydeps.rs`
- Tests: `crates/zen-cli/tests/pymport_e2e.rs`, `crates/zen-vm/tests/pyembed.rs`, `crates/zen-jit/tests/jit_pymport.rs`
- Scripts: `book-v2/tests/web-automation/*.z`, `book-v2/tests/pymport/*.z`

## 42.12 Quick reference

```zen
pymport json
pymport os
pymport requests
pymport bs4
pymport selenium.webdriver as webdriver
pymport urllib.request
pymport builtins

// Positional calls only
json.loads('{"a": 1}')
os.getcwd()
os.path.join("a", "b")
datetime.datetime.now()
builtins.len([1, 2, 3])

// Error handling
try {
    pymport nonexistent_module
} catch as e {
    print("caught import error")
}

// zen py-sync setup
// ./target/release/zen py-sync
// ./target/release/zen run script.z
```

## 42.13 Exercises

1. Write a script that imports `json`, `os`, and `datetime` via `pymport` and prints the current date.
2. Use `pymport bs4` to parse an HTML file and extract all links.
3. Use `pymport selenium.webdriver` to open a headless browser and print the page title.
4. Write `py_len(x)` that calls `builtins.len(x)` on any value.
5. Demonstrate that `pymport` of a missing module is catchable with `try`/`catch`.
