# Chapter 42: pymport - Python Interoperability

`pymport` lets you load CPython modules directly from Zen. Unlike `import` (which loads Zen bytecode), `pymport` uses an embedded CPython interpreter through the PyO3 bridge to call Python functions, access attributes, and work with Python objects as **foreign handles**.

This chapter covers: syntax, how `pymport` differs from `import`, building Zen with Python support, environment setup, dependency management with `zen py-sync` and `requirements.txt`, common patterns, limitations, and working examples (including Selenium/BeautifulSoup/MechanicalSoup and binary downloads).

## 42.1 Why `pymport` (and not just `import`)?

Zen keeps foreign modules on a separate keyword for correctness and tooling:

- **Parser/compiler**: `pymport` is its own statement and emits opcode `PyImport` (discriminant 81), not a flag on `Import`. Native imports stay fast and unambiguous.
- **JIT**: Functions containing `PyImport` stay **interpreted** (foreign boundaries are not lowered in v1).
- **Tooling**: A real parser scan can find all foreign dependencies for `zen py-sync` and editor integrations.
- **Determinism**: No runtime guessing of module kind via file extensions or `sys.path` probing on every resolve.

```zen
import fast_math        # Zen bytecode
pymport requests        # CPython via embedded interpreter
pymport selenium.webdriver as webdriver  # dotted path, aliased
```

## 42.2 Syntax and Binding

- `pymport dotted.path [as alias]` — one module per statement, no comma lists.
- **Binding name**: if `as alias` is given, use `alias`. Else use the **single final name** for `pymport name`. For `pymport a.b`, bind the **root package `a`** (Python-consistent with how imports behave).
- `a.pymport` is parsed as member access (lexer treats `pymport` after `.` as an identifier).
- The destination receives a **foreign handle** (`typeof(x) == "foreign"`).

## 42.3 Building Zen with Python Support

`pymport` requires an embedded CPython. Default builds are hermetic (no Python dependency). Build with the `python` feature:

```bash
cd /home/bard/Desktop/zen
cargo build --release --features python
```

Verify:

```bash
./target/release/zen run - << 'EOF'
pymport json
let data = json.loads("{\"hello\": \"world\", \"n\": 42}")
print(data["hello"])
print(data["n"])
EOF
```

If you see `Z0414` / "unknown native" or a message about `--features python`/libpython, rebuild with `--features python`.

The bridge auto-initializes on first GIL use (no cost if you don't `pymport`).

## 42.4 Environment Setup (venv + PYTHONPATH)

You can point CPython to any Python environment via `PYTHONPATH`. The book ships a ready-made one in `book-v2/tests`:

```bash
cd book-v2/tests
./setup.sh          # builds zen --features python, creates .venv, installs requirements
```

Then, from the repository root, point Zen at that environment:

```bash
export PYTHONPATH="$(book-v2/tests/.venv/bin/python -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"
export ZEN_NO_PY_BOOTSTRAP=1
./target/release/zen run book-v2/tests/pymport/01_json_stdlib.z
```

`ZEN_NO_PY_BOOTSTRAP=1` tells Zen to use that environment as-is instead of creating `.zen_venv`.

### Create your own venv instead

```bash
python3 -m venv .venv
.venv/bin/pip install beautifulsoup4 mechanicalsoup selenium lxml requests
```

Point `PYTHONPATH` at its `site-packages`. The path varies with the Python version (`python3.11`, `python3.12`, `python3.13`), so ask Python rather than hard-coding it:

```bash
export PYTHONPATH="$(.venv/bin/python -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"
```

### Check installed packages

```bash
book-v2/tests/.venv/bin/pip list
```

## 42.5 Dependency Sync with `zen py-sync` and `requirements.txt`

Zen can discover `pymport` usage and ensure a venv with dependencies.

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
./target/release/zen py-sync
```

Behavior:
1. **Scan**: recursively parses entry + transitive `.z` imports to find all `pymport` statements.
2. **Plan**: determines state (`Clean`, `NoRequirements`, `MissingVenv`, `MissingPackages`).
3. **Ensure**: creates `.zen_venv` in project root, runs `pip install -r requirements.txt` in the **foreground**, writes a stamp (requirements length + hash).
4. **Point**: `zen run` prepends `.zen_venv/lib/python*/site-packages` to `PYTHONPATH`. Relative requirements paths resolve against project root.

Control flags:
- `ZEN_NO_PY_BOOTSTRAP=1` — skip bootstrap entirely (for tests/hermetic runs). The runtime will still fail if packages are missing.
- If `requirements.txt` is missing and `pymport` is used, Zen reports (it will not auto-install).

Example end-to-end (as tested):
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
pymport json        # stdlib
pymport os          # stdlib
pymport requests    # third-party (if installed)
pymport bs4         # beautifulsoup4
pymport selenium.webdriver as webdriver
pymport urllib.request
pymport builtins
```

### Calling functions and accessing attributes

Foreign calls are **positional-only** in v1. Use `getattr`-style access via dot notation; `call_attr` fuses resolve+dispatch for methods.

```zen
pymport math
print(math.sqrt(16.0))
```

```zen
pymport json
let d = json.loads("{\"a\": 1}")
print(d["a"])  # dict/list/indexing works on marshaled values where possible
```

### Selenium example (positional only)

```zen
pymport selenium.webdriver as webdriver
pymport selenium.webdriver.chrome.options as options

let opts = options.Options()
opts.add_argument("--headless=new")
opts.add_argument("--no-sandbox")
opts.add_argument("--disable-dev-shm-usage")
let driver = webdriver.Chrome(opts)  # positional, not keyword
driver.get("https://example.com")
print(driver.title)
driver.quit()
```

### Working with Python objects and binary data

- Python `bytes` marshal to Zen `bytes`/handles as appropriate; Python `str`/`int`/`float`/`bool`/`None`/`list`/`dict` marshal where shapes are supported.
- Tuples become lists at the boundary. Big ints beyond `i64` may become `f64` (precision caveat). Non-string dict keys are rejected.
- For binary downloads, prefer `urllib.request.urlretrieve(url, path)` or `builtins.open(path,"wb").write(py_bytes_handle)`. Native `fs.write_binary` takes a string and has string/UTF-8 constraints—treat it as text-oriented.

### Error handling

Foreign errors surface as `NativeError`/`VmError` from the bridge with Python exception messages captured.

```zen
try {
    pymport nonexistent_module
} catch as e {
    print(e)
}
```

## 42.7 Working Examples (tested)

All scripts below live in [`book-v2/tests/web-automation/`](../tests/web-automation/). Five smaller, hermetic pymport tests live in [`book-v2/tests/pymport/`](../tests/pymport/).

### Example A: `bs4` + `requests` scrape + download

See [`tests/web-automation/bs4_tiktok.z`](../tests/web-automation/bs4_tiktok.z) (downloads a TikTok embed MP4 via `urllib.request.urlretrieve`, parsed with BeautifulSoup from `__FRONTITY_CONNECT_STATE__`).

### Example B: `mechanicalsoup` stateful scrape + download

See [`tests/web-automation/ms_tiktok.z`](../tests/web-automation/ms_tiktok.z). Uses `StatefulBrowser`, accesses `browser.page`, reuses `browser.session.get()` for CDN, writes bytes via `builtins.open(...,"wb").write()`.

### Example C: Selenium automation + download

See [`tests/web-automation/selenium_tiktok.z`](../tests/web-automation/selenium_tiktok.z). Launches headless Chromium, polls for `__UNIVERSAL_DATA_FOR_REHYDRATION__`, extracts stats, fetches the embed route with plain HTTP for an unsigned MP4 URL, downloads via `urllib.request.urlretrieve`.

Run all of them (including Example D):
```bash
cd book-v2/tests
./run-all.sh --network
```

Or run the web examples individually:
```bash
cd book-v2/tests/web-automation
export PYTHONPATH="$(../.venv/bin/python -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"
export ZEN_NO_PY_BOOTSTRAP=1
ZEN=../../../../target/release/zen
$ZEN run bs4_tiktok.z
$ZEN run ms_tiktok.z
timeout 300 $ZEN run selenium_tiktok.z
file out/tiktok_*.mp4
```

All produced verified `ISO Media, MP4 Base Media v1` files (~3.22 MB).

### Example D: CDP over raw TCP/WebSocket (no Selenium)

See [`tests/web-automation/cdp_tiktok.z`](../tests/web-automation/cdp_tiktok.z). Launches Chromium with `pymport subprocess`, discovers targets via HTTP `/json/list` (handle `Content-Length` and possible EAGAIN), upgrades to WebSocket, sends masked CDP frames via `sys.write_bytes(socket.fileno(sock), hex)`, parses unmasked server frames, runs `Page.navigate` + `Runtime.evaluate` to extract TikTok metadata. Cleans up with `browser.terminate()` in a `finally` block.

This demonstrates low-level socket control and binary-safe frame writing in Zen.

## 42.8 Limits and Non-Goals (v1)

- **No keyword arguments** in `pymport` calls.
- **No passing Zen closures into Python** (rejected by marshal).
- **No per-handle finalizers**: foreign handles remain until bridge teardown; `release` exists on the bridge but Zen GC cannot call it per-handle.
- **Tuples → lists**, `big int` → `f64` in some cases, non-string dict keys rejected.
- **Functions containing `PyImport` not JIT-lowered** (stay interpreted).
- **No `zen.toml [py-deps]`** yet (use `requirements.txt`).
- **Network installs** only via `zen py-sync`/preamble; tests avoid silent installs.
- **Handles are opaque** across isolates in general (same caveat as native handles).

## 42.9 Troubleshooting

| Issue | Fix |
|---|---|
| `Z0414` / "unknown native" / needs `--features python` | Rebuild with `cargo build --release --features python` and ensure libpython is available. |
| `ModuleNotFoundError` for third-party package | Install into venv, set `PYTHONPATH`, or run `zen py-sync` with `requirements.txt`. |
| Selenium can't find chromedriver | Install `chromedriver` matching Chromium or put it on `PATH`. Use `--no-sandbox` in containers. |
| TikTok WAF blocks plain requests | Use `/embed/v2/<id>`, or Selenium/real browser. See Examples A/C. |
| Binary download corrupted | Don't rely on string-based `fs.write_binary`; use `urllib.request.urlretrieve` or `builtins.open("wb").write()`. |
| WebSocket hang in raw sockets | Read until `Content-Length` for HTTP, handle EAGAIN on keep-alive, mask client frames, parse length (126/127). |
| `pymport` call with kwargs fails | Use positional args only in v1. |
| Bootstrap skipped unexpectedly | Check `ZEN_NO_PY_BOOTSTRAP` or that `requirements.txt` exists near entry. |

## 42.10 References

- Docs: `docs/20-pymport.md` (design/spec)
- VM bridge: `crates/zen-vm/src/pybridge.rs`, `crates/zen-vm/src/pyembed.rs`
- CLI bootstrap: `zen-cli/src/pydeps.rs`
- Tests: `crates/zen-cli/tests/pymport_e2e.rs`, `crates/zen-vm/tests/pyembed.rs`, `crates/zen-jit/tests/jit_pymport.rs`
- Scripts: `book-v2/tests/web-automation/*.z`, `book-v2/tests/pymport/*.z`
