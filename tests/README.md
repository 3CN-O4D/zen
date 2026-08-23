# Zen Test Suite

Run all black-box tests against the built interpreter:

    ./tests/run_tests.sh            # uses ./target/debug/zen
    ZEN_BIN=./target/release/zen ./tests/run_tests.sh

The runner builds the binary if missing, starts `tests/mock_ftp.py`
(127.0.0.1:2121, admin/s3cret99) for socket/hydra cases, and prints a
per-file summary. A case fails if it exits non-zero or its output
contains any `FAIL:` line.

Conventions for case files:

- define `check(cond, name)` (copy from an existing case) and use it
  for every expectation; it prints `FAIL: <name>` on failure
- end with the standard SUITE PASS / FAILURES footer
- never rely on files outside `tests/` or `/tmp/opencode`

Rust unit tests remain separate: `cargo test`.
