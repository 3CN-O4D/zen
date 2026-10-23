# Book test suite

Runnable examples for *Zen for Dummies*. Everything here is self-contained:
no paths from the author's machine, no hidden setup. Clone the repo, run one
script, or run the whole suite.

```bash
git clone https://github.com/3CN-O4D/zen.git
cd zen/book-v2/tests
./setup.sh                              # builds zen + creates .venv
./run-all.sh                            # hermetic pymport tests
./run-all.sh --network                  # + live TikTok web automation
```

## Layout

| Path | Contents |
|---|---|
| `pymport/` | 5 hermetic tests for the embedded CPython bridge |
| `web-automation/` | 4 tested TikTok scrapers: bs4, MechanicalSoup, Selenium, CDP |
| `setup.sh` | Builds Zen with `--features python`, creates `.venv`, installs packages |
| `run-all.sh` | Runs the suite and validates downloaded MP4 artifacts |

## Requirements

- **Zen built with the Python bridge**: `cargo build --release --features python`
- Python 3.9+ with `venv`
- `chromium` for the Selenium and CDP tests
- `chromedriver` matching Chromium for the Selenium test
- Network access for `--network`

`setup.sh` does the Zen build and Python installs. Chromium is **not** installed
by the script; use your package manager.

## Running a single test

```bash
cd book-v2/tests
export PYTHONPATH="$(.venv/bin/python -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"
export ZEN_NO_PY_BOOTSTRAP=1
ZEN=../../target/release/zen

cd pymport
$ZEN run 01_json_stdlib.z
```

Each script prints deterministic output and exits non-zero on failure.

## The tests

### pymport (no network)

| Script | Covers |
|---|---|
| `pymport/01_json_stdlib.z` | stdlib import, calls, dict/list round-trip |
| `pymport/02_os_math_stdlib.z` | nested modules (`os.path.join`), scalar marshaling |
| `pymport/03_bs4_third_party.z` | third-party import, parsing `fixture.html` |
| `pymport/04_foreign_objects.z` | opaque handles, attributes, methods, builtins |
| `pymport/05_error_handling.z` | Python exceptions as catchable Zen errors |

### web automation (network)

| Script | Technique |
|---|---|
| `web-automation/bs4_tiktok.z` | native `requests` + bs4 + `urllib.request.urlretrieve` |
| `web-automation/ms_tiktok.z` | `StatefulBrowser.open` + `browser.page` + `builtins.open("wb")` |
| `web-automation/selenium_tiktok.z` | headless Chromium, rehydration JSON, embed MP4 route |
| `web-automation/cdp_tiktok.z` | raw TCP + RFC 6455 WebSocket + CDP, masked frames via `sys.write_bytes` |

The network tests download the same public video
(`@scout2015/video/6718335390845095173`) and write MP4s into
`web-automation/out/`. `run-all.sh` checks the `ftyp` box before reporting
success.

TikTok changes its markup and applies WAF challenges per IP. The examples use
the `/embed/v2/<id>` route because it is stable for plain HTTP; Selenium opens
the full video page. If a test suddenly fails with a challenge page or a
renamed JSON key, that is external drift, not a Zen failure. See
[Chapter 44](../projects/44-web-automation.md) for the full walkthrough.

## Using `zen py-sync` instead

Each test directory has a `requirements.txt`:

```bash
cd book-v2/tests/pymport
../../../target/release/zen py-sync
../../../target/release/zen run 03_bs4_third_party.z
```

`py-sync` creates `.zen_venv/` beside the requirements file and `zen run`
prepends it to `PYTHONPATH`. The suite uses `ZEN_NO_PY_BOOTSTRAP=1` because it
manages its own `.venv`, which is faster for repeated runs.

## Related chapters

- [Chapter 42 — pymport](../../interop/42-pymport-python-interoperability.md)
- [Chapter 44 — Web Scraping & Automation](../projects/44-web-automation.md)
