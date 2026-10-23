#!/usr/bin/env bash
# Prepare the Python environment used by the pymport examples.
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$DIR/../.." && pwd)"
ZEN="$ROOT/target/release/zen"
VENV="$DIR/.venv"

echo "==> book-v2 test setup"
echo "    repo: $ROOT"

if [ ! -x "$ZEN" ]; then
    echo "==> building zen (release, python feature)"
    (cd "$ROOT" && cargo build --release --features python)
fi

if [ ! -d "$VENV" ]; then
    echo "==> creating venv at $VENV"
    python3 -m venv "$VENV"
fi

echo "==> installing test requirements"
"$VENV/bin/pip" install --quiet --upgrade pip
"$VENV/bin/pip" install --quiet -r "$DIR/requirements.txt"

SITE="$("$VENV/bin/python" -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"

cat <<EOF

==> done

Use these scripts with:

  export PYTHONPATH="$SITE"
  export ZEN_NO_PY_BOOTSTRAP=1
  ZEN="$ZEN"

Then run from the directory of the test you want:

  cd "$DIR/pymport"
  "\$ZEN" run 01_json_stdlib.z

Or run everything (pymport only by default):

  "$DIR/run-all.sh"
  "$DIR/run-all.sh" --network    # also hits TikTok (needs network + chromium)

EOF
