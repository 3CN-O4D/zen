#!/usr/bin/env bash
# Run the book's test suite. By default only hermetic pymport tests run.
# Pass --network to also run the live TikTok web-automation examples.
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$DIR/../.." && pwd)"
ZEN="$ROOT/target/release/zen"
NETWORK=0
[ "${1:-}" = "--network" ] && NETWORK=1

if [ ! -x "$ZEN" ]; then
    echo "error: $ZEN not found; run ./setup.sh first" >&2
    exit 2
fi
if [ ! -d "$DIR/.venv" ]; then
    echo "==> .venv missing; running setup.sh"
    "$DIR/setup.sh" >/dev/null
fi

SITE="$("$DIR/.venv/bin/python" -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"
export PYTHONPATH="$SITE"
export ZEN_NO_PY_BOOTSTRAP=1

PASS=0
FAIL=0
FAILED=()

run_one() { # dir, script, timeout
    local dir="$1" script="$2" limit="$3"
    echo
    echo "==> $dir/$script"
    if ( cd "$DIR/$dir" && timeout "$limit" "$ZEN" run "$script" ); then
        PASS=$((PASS + 1))
    else
        FAIL=$((FAIL + 1))
        FAILED+=("$dir/$script")
    fi
}

check_mp4() { # path
    if [ -f "$1" ] && [ "$(dd if="$1" bs=1 skip=4 count=4 2>/dev/null)" = "ftyp" ]; then
        echo "    ok: $1 ($(wc -c < "$1") bytes, MP4 ftyp)"
        return 0
    fi
    echo "    FAIL: $1 missing or not an MP4"
    return 1
}

echo "=== pymport (hermetic) ==="
for t in 01_json_stdlib.z 02_os_math_stdlib.z 03_bs4_third_party.z \
         04_foreign_objects.z 05_error_handling.z; do
    run_one pymport "$t" 60
done

echo
echo "=== book chapters (hermetic, no network) ==="
run_one fundamentals 03-operators.z 60
run_one core 11-error-handling.z 60

if [ "$NETWORK" = "1" ]; then
    echo
    echo "=== web automation (live network) ==="
    rm -f "$DIR/web-automation/out/tiktok_bs4.mp4" \
          "$DIR/web-automation/out/tiktok_ms.mp4" \
          "$DIR/web-automation/out/tiktok_selenium.mp4"

    run_one web-automation bs4_tiktok.z 120
    check_mp4 "$DIR/web-automation/out/tiktok_bs4.mp4" || {
        FAIL=$((FAIL + 1)); FAILED+=("web-automation/bs4_tiktok.z (artifact)"); }

    run_one web-automation ms_tiktok.z 180
    check_mp4 "$DIR/web-automation/out/tiktok_ms.mp4" || {
        FAIL=$((FAIL + 1)); FAILED+=("web-automation/ms_tiktok.z (artifact)"); }

    if command -v chromium >/dev/null 2>&1 && command -v chromedriver >/dev/null 2>&1; then
        run_one web-automation selenium_tiktok.z 300
        check_mp4 "$DIR/web-automation/out/tiktok_selenium.mp4" || {
            FAIL=$((FAIL + 1)); FAILED+=("web-automation/selenium_tiktok.z (artifact)"); }
    else
        echo "    skipping selenium: chromium and/or chromedriver not on PATH"
    fi

    if command -v chromium >/dev/null 2>&1; then
        run_one web-automation cdp_tiktok.z 180
    else
        echo "    skipping cdp: chromium not on PATH"
    fi
fi

echo
echo "=== summary ==="
echo "passed: $PASS  failed: $FAIL"
if [ "$FAIL" -ne 0 ]; then
    printf 'failed: %s\n' "${FAILED[@]}"
    exit 1
fi
