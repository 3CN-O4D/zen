#!/usr/bin/env bash
# Cross-compile zen for desktop Linux + Android (Termux) architectures.
# Uses `cross` (Docker-based) so aws-lc-sys / C deps build cleanly per target.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"
DIST="$ROOT/dist"
mkdir -p "$DIST"

export CARGO_TERM_COLOR=never

log() { echo "[cross] $*"; }

# cargo install cross if missing
if ! command -v cross >/dev/null 2>&1; then
    log "installing cross..."
    cargo install cross --locked
fi
if ! command -v cross >/dev/null 2>&1; then
    log "cross not on PATH; adding ~/.cargo/bin"
    export PATH="$HOME/.cargo/bin:$PATH"
fi

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

build_target() {
    local target="$1" suffix="$2"
    log "adding target: $target"
    rustup target add "$target"
    log "building $target (release)..."
    cross build --release --target "$target"
    local bin="target/$target/release/zen"
    if [ ! -f "$bin" ]; then
        log "ERROR: no binary for $target"
        return 1
    fi
    local out="$DIST/zen-${VERSION}-${suffix}"
    cp "$bin" "$out"
    chmod +x "$out"
    log "done: $out"
}

# ---- Desktop Linux ----
build_target x86_64-unknown-linux-gnu    x86_64-linux
build_target x86_64-unknown-linux-musl   x86_64-linux-musl
build_target aarch64-unknown-linux-gnu   aarch64-linux
build_target aarch64-unknown-linux-musl  aarch64-linux-musl
build_target armv7-unknown-linux-gnueabihf armv7-linux
build_target arm-unknown-linux-gnueabihf arm-linux
build_target i686-unknown-linux-gnu      i686-linux

# ---- Android / Termux ----
build_target aarch64-linux-android       aarch64-termux
build_target armv7-linux-androideabi     armv7-termux
build_target x86_64-linux-android        x86_64-termux
build_target i686-linux-android          i686-termux

log "ALL BUILDS COMPLETE"
ls -lh "$DIST"
