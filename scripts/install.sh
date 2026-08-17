#!/usr/bin/env bash
# Zen language install script
# Builds zen for the current system and installs the binary to a system path.
#
# Usage:
#   ./scripts/install.sh              # Build + install to ~/.local/bin
#   ./scripts/install.sh --prefix /usr/local   # Install to /usr/local/bin
#   ./scripts/install.sh --from dist/zen-*     # Skip build, install pre-built binary
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

# ── Parse arguments ──────────────────────────────────────────────────────────
PREFIX="${HOME}/.local"
FROM_BINARY=""

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)   PREFIX="$2"; shift 2 ;;
        --from)     FROM_BINARY="$2"; shift 2 ;;
        --system)   PREFIX="/usr/local"; shift ;;
        -h|--help)
            echo "Usage: $0 [--prefix <dir>] [--from <binary>] [--system]"
            echo ""
            echo "Options:"
            echo "  --prefix <dir>   Install prefix (default: ~/.local)"
            echo "  --from <binary>  Install a pre-built binary instead of building"
            echo "  --system         Install to /usr/local/bin (may need sudo)"
            exit 0
            ;;
        *) err "Unknown option: $1"; exit 1 ;;
    esac
done

BIN_DIR="${PREFIX}/bin"
mkdir -p "$BIN_DIR"

# ── Build or use pre-built ──────────────────────────────────────────────────
if [ -n "$FROM_BINARY" ]; then
    if [ ! -f "$FROM_BINARY" ]; then
        err "Binary not found: $FROM_BINARY"
        exit 1
    fi
    BINARY="$FROM_BINARY"
    log "Using pre-built binary: $BINARY"
else
    # Check if a dist binary already exists for current arch
    ARCH="$(uname -m)"
    OS="$(uname -s)"
    IS_TERMUX=0
    [ -n "${PREFIX:-}" ] && [ "$(uname -o 2>/dev/null)" = "Android" ] && IS_TERMUX=1

    # Try to find existing dist binary
    EXISTING=""
    case "$OS" in
        Linux)
            if [ "$IS_TERMUX" -eq 1 ]; then
                EXISTING=$(ls dist/zen-*-termux 2>/dev/null | head -1)
            else
                case "$ARCH" in
                    x86_64)    EXISTING=$(ls dist/zen-*-x86_64-linux 2>/dev/null | head -1) ;;
                    aarch64)   EXISTING=$(ls dist/zen-*-aarch64-linux 2>/dev/null | head -1) ;;
                    armv7l)    EXISTING=$(ls dist/zen-*-armv7-linux 2>/dev/null | head -1) ;;
                esac
            fi
            ;;
        Darwin)
            EXISTING=$(ls dist/zen-*-apple-darwin 2>/dev/null | head -1)
            ;;
    esac

    if [ -n "$EXISTING" ]; then
        log "Found pre-built binary: $EXISTING"
        BINARY="$EXISTING"
    else
        log "No pre-built binary found. Building from source..."
        bash scripts/build.sh --release
        # Find the built binary
        BINARY=$(ls -t dist/zen-* 2>/dev/null | head -1)
        if [ -z "$BINARY" ]; then
            err "Build failed. Binary not found in dist/."
            exit 1
        fi
    fi
fi

# ── Install ──────────────────────────────────────────────────────────────────
INSTALL_PATH="${BIN_DIR}/zen"
log "Installing to: ${BOLD}${INSTALL_PATH}${NC}"

cp "$BINARY" "$INSTALL_PATH"
chmod +x "$INSTALL_PATH"

# ── Verify ──────────────────────────────────────────────────────────────────
if command -v zen >/dev/null 2>&1; then
    VERSION=$(zen --version 2>/dev/null || echo "unknown")
    ok "Installed: ${BOLD}${VERSION}${NC}"
else
    warn "Installed to ${INSTALL_PATH}"
    echo ""
    echo "  Add to your PATH if not already present:"
    echo ""
    echo "    # For bash:"
    echo "    echo 'export PATH=\"${BIN_DIR}:\$PATH\"' >> ~/.bashrc"
    echo ""
    echo "    # For zsh:"
    echo "    echo 'export PATH=\"${BIN_DIR}:\$PATH\"' >> ~/.zshrc"
    echo ""
    echo "    # For fish:"
    echo "    fish_add_path ${BIN_DIR}"
    echo ""
    echo "  Then restart your shell or run:"
    echo ""
    echo "    source ~/.bashrc   # or source ~/.zshrc"
fi

echo ""
log "Next steps:"
echo "  1. Set up editor support:   ./scripts/setup-editors.sh"
echo "  2. Verify installation:     zen --version"
echo "  3. Start the REPL:          zen repl"
echo "  4. Run a script:            zen examples/hello.z"
