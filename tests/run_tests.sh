#!/usr/bin/env bash
# Zen black-box test runner.
# Runs every tests/cases/*.z against the interpreter and reports results.
set -u
cd "$(dirname "$0")/.."
ROOT="$(pwd)"
ZEN_BIN="${ZEN_BIN:-$ROOT/target/debug/zen}"

if [ ! -x "$ZEN_BIN" ]; then
    echo "building $ZEN_BIN ..."
    cargo build --quiet || exit 1
fi

FTP_PID=""
FTP_STARTED=0
if nc -z 127.0.0.1 2121 2>/dev/null; then
    echo "mock ftp: already running (reusing)"
else
    setsid nohup python3 "$ROOT/tests/mock_ftp.py" > /tmp/zen_test/zen_suite_ftp.log 2>&1 < /dev/null &
    FTP_PID=$!
    FTP_STARTED=1
    # Wait for the mock server to accept connections (up to ~6s).
    for _ in $(seq 1 30); do
        if nc -z 127.0.0.1 2121 2>/dev/null; then
            break
        fi
        sleep 0.2
    done
fi
cleanup() {
    if [ "$FTP_STARTED" = "1" ] && [ -n "$FTP_PID" ]; then
        kill "$FTP_PID" 2>/dev/null
    fi
}
trap cleanup EXIT

pass=0; fail=0; failed_cases=""
for f in "$ROOT"/tests/cases/*.z; do
    name="$(basename "$f")"
    # Run from the cases dir so dotted-package fixtures (pkg/) resolve.
    out="$(cd "$ROOT/tests/cases" && "$ZEN_BIN" run "$name" 2>&1)"
    rc=$?
    nfail="$(printf '%s\n' "$out" | grep -c 'FAIL:' || true)"
    if [ "$rc" -eq 0 ] && [ "$nfail" -eq 0 ]; then
        echo "PASS  $name"
        pass=$((pass + 1))
    else
        echo "FAIL  $name  (exit=$rc, FAIL-lines=$nfail)"
        printf '%s\n' "$out" | grep 'FAIL:' | sed 's/^/      /'
        fail=$((fail + 1))
        failed_cases="$failed_cases $name"
    fi
done

echo "----------------------------------------"
echo "suites: $((pass + fail))  passed: $pass  failed: $fail"
[ "$fail" -eq 0 ]
