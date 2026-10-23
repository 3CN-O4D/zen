# pymport test suite

These examples exercise the embedded CPython bridge. They are the runnable
counterparts to [Chapter 42](../../interop/42-pymport-python-interoperability.md).

## Quick start

From this directory:

```bash
# 1. Provide a CPython environment with the third-party dependency
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt

# 2. Point the embedded interpreter at it
export PYTHONPATH="$(find "$PWD/.venv" -type d -name site-packages | head -1)"
export ZEN_NO_PY_BOOTSTRAP=1

# 3. Run a test
ZEN=../../../target/release/zen
$ZEN run 01_json_stdlib.z
```

`ZEN` must be a build with the Python bridge:

```bash
cargo build --release --features python
```

## Tests

| Script | What it proves |
|---|---|
| `01_json_stdlib.z` | stdlib import, calling functions, dict/list round-trip |
| `02_os_math_stdlib.z` | nested attributes (`os.path.join`), string/number marshaling |
| `03_bs4_third_party.z` | third-party import, parsing a local fixture, tag extraction |
| `04_foreign_objects.z` | opaque handles, attribute access, method calls |
| `05_error_handling.z` | Python exceptions become catchable Zen errors |

## Using `zen py-sync` instead of PYTHONPATH

This directory has a `requirements.txt`, so:

```bash
../../../target/release/zen py-sync
../../../target/release/zen run 03_bs4_third_party.z
```

`py-sync` creates `.zen_venv/` here, installs `requirements.txt`, and `zen run`
prepends that environment automatically. `ZEN_NO_PY_BOOTSTRAP=1` disables this
behavior (used in CI where the environment is already prepared).

## Expected output

Each script prints deterministic results and exits `0`. `03_bs4_third_party.z`
reads `fixture.html` from the current directory, so run it from here.
