#!/usr/bin/env bash
# Zen language build script
# Auto-detects OS, architecture, and builds the zen binary.
#
# Usage:
#   ./scripts/build.sh                # Build for current system
#   ./scripts/build.sh --release      # Build optimized release (default)
#   ./scripts/build.sh --debug        # Build debug binary
#   ./scripts/build.sh --target <triple>  # Cross-compile for specific target
#
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"

# ── Colors ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

log()  { echo -e "${BLUE}==>${NC} $*"; }
ok()   { echo -e "${GREEN}==> OK${NC} $*"; }
warn() { echo -e "${YELLOW}==> WARN${NC} $*"; }
err()  { echo -e "${RED}==> ERROR${NC} $*" >&2; }

# ── Detect environment ───────────────────────────────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"
PROFILE="release"

# Android / Termux detection
IS_TERMUX=0
if [ -n "${PREFIX:-}" ] && [ "$(uname -o 2>/dev/null)" = "Android" ]; then
    IS_TERMUX=1
fi

# Parse arguments
TARGET=""
while [ $# -gt 0 ]; do
    case "$1" in
        --debug)   PROFILE="debug"; shift ;;
        --release) PROFILE="release"; shift ;;
        --target)  TARGET="$2"; shift 2 ;;
        -h|--help)
            echo "Usage: $0 [--release|--debug] [--target <triple>]"
            echo ""
            echo "Options:"
            echo "  --release    Build optimized release binary (default)"
            echo "  --debug      Build debug binary"
            echo "  --target     Cross-compile for a specific target triple"
            echo ""
            echo "Detected: OS=$OS ARCH=$ARCH TERMUX=$IS_TERMUX"
            exit 0
            ;;
        *) err "Unknown option: $1"; exit 1 ;;
    esac
done

# ── Ensure Rust toolchain ────────────────────────────────────────────────────
if ! command -v cargo >/dev/null 2>&1; then
    err "Rust toolchain not found."
    echo ""
    echo "  Install Rust via rustup:"
    echo ""
    echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo ""
    echo "  Then restart your shell or run:"
    echo ""
    echo "    source ~/.cargo/env"
    echo ""
    exit 1
fi

log "Detected: ${BOLD}${OS} ${ARCH}${NC}"
[ "$IS_TERMUX" -eq 1 ] && log "Termux (Android) environment detected"

# ── Resolve target triple ────────────────────────────────────────────────────
if [ -z "$TARGET" ]; then
    case "$OS" in
        Linux)
            if [ "$IS_TERMUX" -eq 1 ]; then
                case "$ARCH" in
                    aarch64)  TARGET="aarch64-linux-android" ;;
                    armv7l)   TARGET="armv7-linux-androideabi" ;;
                    x86_64)   TARGET="x86_64-linux-android" ;;
                    i686)     TARGET="i686-linux-android" ;;
                    *)        TARGET="${ARCH}-linux-android" ;;
                esac
            else
                case "$ARCH" in
                    x86_64)    TARGET="x86_64-unknown-linux-gnu" ;;
                    aarch64)   TARGET="aarch64-unknown-linux-gnu" ;;
                    armv7l)    TARGET="armv7-unknown-linux-gnueabihf" ;;
                    armv6l)    TARGET="arm-unknown-linux-gnueabihf" ;;
                    i686|i386) TARGET="i686-unknown-linux-gnu" ;;
                    *)         TARGET="${ARCH}-unknown-linux-gnu" ;;
                esac
            fi
            ;;
        Darwin)
            case "$ARCH" in
                arm64)  TARGET="aarch64-apple-darwin" ;;
                x86_64) TARGET="x86_64-apple-darwin" ;;
                *)      TARGET="${ARCH}-apple-darwin" ;;
            esac
            ;;
        MINGW*|MSYS*|CYGWIN*)
            case "$ARCH" in
                x86_64) TARGET="x86_64-pc-windows-msvc" ;;
                i686)   TARGET="i686-pc-windows-msvc" ;;
                *)      TARGET="${ARCH}-pc-windows-msvc" ;;
            esac
            ;;
        FreeBSD)
            case "$ARCH" in
                x86_64)  TARGET="x86_64-unknown-freebsd" ;;
                aarch64) TARGET="aarch64-unknown-freebsd" ;;
                *)       TARGET="${ARCH}-unknown-freebsd" ;;
            esac
            ;;
        *)
            err "Unsupported OS: $OS"
            exit 1
            ;;
    esac
fi

log "Target: ${BOLD}${TARGET}${NC}"
log "Profile: ${BOLD}${PROFILE}${NC}"

# ── Ensure target is installed ───────────────────────────────────────────────
CURRENT_HOST=$(rustc -vV | grep '^host:' | awk '{print $2}')
if [ "$TARGET" != "$CURRENT_HOST" ]; then
    log "Adding target ${TARGET}..."
    rustup target add "$TARGET" 2>/dev/null || true
fi

# ── Build ────────────────────────────────────────────────────────────────────
log "Building zen (${PROFILE})..."

CARGO_FLAGS=("--target" "$TARGET")
[ "$PROFILE" = "release" ] && CARGO_FLAGS+=("--release")

# For Termux: if building natively on device, we can skip cross
if [ "$IS_TERMUX" -eq 1 ] && [ "$TARGET" = "$(rustc -vV | grep '^host:' | awk '{print $2}')" ]; then
    CARGO_FLAGS=()
    [ "$PROFILE" = "release" ] && CARGO_FLAGS+=("--release")
fi

cargo build "${CARGO_FLAGS[@]}" 2>&1

# ── Locate binary ────────────────────────────────────────────────────────────
if [ "$PROFILE" = "release" ]; then
    BIN="target/${TARGET}/release/zen"
else
    BIN="target/${TARGET}/debug/zen"
fi

# If native build (target == host), binary is at target/{profile}/zen
if [ ! -f "$BIN" ] && [ "$TARGET" = "$CURRENT_HOST" ]; then
    BIN="target/${PROFILE}/zen"
fi

if [ ! -f "$BIN" ]; then
    # Try without target dir
    BIN="target/zen"
fi

if [ ! -f "$BIN" ]; then
    err "Build succeeded but binary not found at expected location."
    echo "  Looked in:"
    echo "    target/${TARGET}/${PROFILE}/zen"
    echo "    target/${PROFILE}/zen"
    echo "    target/zen"
    exit 1
fi

# ── Output ───────────────────────────────────────────────────────────────────
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
DIST_DIR="dist"
mkdir -p "$DIST_DIR"

# Determine extension
EXT=""
case "$OS" in
    MINGW*|MSYS*|CYGWIN*) EXT=".exe" ;;
esac

OUTPUT="${DIST_DIR}/zen-${VERSION}-${TARGET}${EXT}"
cp "$BIN" "$OUTPUT"
chmod +x "$OUTPUT"

SIZE=$(du -h "$OUTPUT" | cut -f1)
ok "Built: ${BOLD}${OUTPUT}${NC} (${SIZE})"
echo ""
echo "  To install system-wide:  ./scripts/install.sh"
echo "  To set up editors:       ./scripts/setup-editors.sh"
