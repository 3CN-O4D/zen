#!/usr/bin/env bash
# Run the four web-automation examples. Requires network + chromium.
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTS="$(cd "$DIR/.." && pwd)"
ZEN="$(cd "$TESTS/../.." && pwd)/target/release/zen"

if [ ! -x "$ZEN" ]; then
    echo "error: $ZEN not found; run ../setup.sh" >&2
    exit 2
fi
if [ ! -d "$TESTS/.venv" ]; then
    echo "==> .venv missing; running setup.sh"
    "$TESTS/setup.sh" >/dev/null
fi

export PYTHONPATH="$("$TESTS/.venv/bin/python" -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"
export ZEN_NO_PY_BOOTSTRAP=1

rm -f "$DIR"/out/tiktok_bs4.mp4 "$DIR"/out/tiktok_ms.mp4 "$DIR"/out/tiktok_selenium.mp4

fail=0
run() {
    echo
    echo "==> $1"
    if ( cd "$DIR" && timeout "$2" "$ZEN" run "$1" ); then
        return 0
    fi
    fail=1
}

run bs4_tiktok.z 120
run ms_tiktok.z 180
if command -v chromium >/dev/null 2>&1 && command -v chromedriver >/dev/null 2>&1; then
    run selenium_tiktok.z 300
else
    echo "skipping selenium_tiktok.z: chromium and/or chromedriver not on PATH"
fi
if command -v chromium >/dev/null 2>&1; then
    run cdp_tiktok.z 180
else
    echo "skipping cdp_tiktok.z: chromium not on PATH"
fi

echo
echo "==> artifacts"
for f in out/*.mp4; do
    [ -e "$f" ] || continue
    if [ "$(dd if="$f" bs=1 skip=4 count=4 2>/dev/null)" = "ftyp" ]; then
        echo "ok  $f ($(wc -c < "$f") bytes)"
    else
        echo "BAD $f"
        fail=1
    fi
done

exit "$fail"
