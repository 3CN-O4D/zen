#!/usr/bin/env bash
# Zen installer.
# Detects Termux (Android) and installs the native Python deps from apt first,
# because pip cannot build the C-extension packages (psutil, lxml) there.
set -e

cd "$(dirname "$0")"

PYTHON="${PYTHON:-python3}"
PIP="${PIP:-pip}"

# ---- Termux (Android) ----------------------------------------------------
# apt-installed Python packages ship their .dist-info, so pip's resolver will
# treat psutil/lxml as already installed and skip building them from source.
if [ -n "$PREFIX" ] && [ "$(uname -o 2>/dev/null)" = "Android" ]; then
    echo "==> Termux detected. Installing system packages..."
    apt update
    apt install -y python-psutil python-lxml
fi

# ---- Python install ------------------------------------------------------
echo "==> Installing zen and its dependencies..."
ERR_LOG="$(mktemp)"
if ! "$PIP" install -e . 2>"$ERR_LOG"; then
    if grep -qi 'externally-managed\|break-system-packages' "$ERR_LOG"; then
        echo "==> Externally-managed environment. Retrying with --break-system-packages..."
        "$PIP" install --break-system-packages -e .
    else
        cat "$ERR_LOG"
        exit 1
    fi
fi
rm -f "$ERR_LOG"

echo "==> Done. Run: zen shell"
