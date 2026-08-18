#!/usr/bin/env bash
# Zen multi-arch build script
# Detects OS + architecture, builds optimized binary, installs to /usr/local/bin
# Supports: Linux, macOS, Android (Termux), FreeBSD
# Architectures: x86_64, aarch64, armv7l, armv6l, riscv64
set -euo pipefail

VERSION="${VERSION:-$(grep 'version' zen/Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')}"
BUILD_DIR="zen/target/release"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'

log()  { echo -e "${GREEN}==> ${NC}$*"; }
warn() { echo -e "${YELLOW}==> ${NC}$*"; }
err()  { echo -e "${RED}==> ${NC}$*" >&2; }
info() { echo -e "${BLUE}    ${NC}$*"; }

# ─── Detect OS ───────────────────────────────────────────────────────────
detect_os() {
    local os
    os="$(uname -s)"
    case "$os" in
        Linux*)
            if [ -f /data/data/com.termux/files/usr/etc/termux_version ] 2>/dev/null || \
               [ -n "${PREFIX:-}" ] && [ "$(uname -o 2>/dev/null)" = "Android" ]; then
                echo "android"
            elif grep -qi microsoft /proc/version 2>/dev/null; then
                echo "windows-wsl"
            else
                echo "linux"
            fi
            ;;
        Darwin*)  echo "macos" ;;
        FreeBSD*) echo "freebsd" ;;
        *)        echo "unknown" ;;
    esac
}

# ─── Detect architecture ─────────────────────────────────────────────────
detect_arch() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64)   echo "x86_64" ;;
        aarch64|arm64)   echo "aarch64" ;;
        armv7l|armhf)    echo "armv7" ;;
        armv6l)          echo "armv6" ;;
        riscv64)         echo "riscv64" ;;
        i386|i486|i586|i686) echo "x86" ;;
        *)               echo "$arch" ;;
    esac
}

# ─── Detect C cross-compiler ─────────────────────────────────────────────
find_cc() {
    local arch="$1" os="$2"
    local cc=""

    # Try cross-compilers first
    case "$arch" in
        aarch64)
            for try in aarch64-linux-gnu-gcc aarch64-linux-android-clang \
                       aarch64-unknown-linux-gnu-gcc clang; do
                command -v "$try" >/dev/null 2>&1 && { cc="$try"; break; }
            done
            ;;
        armv7|armv6)
            for try in arm-linux-gnueabihf-gcc arm-linux-gnueabi-gcc \
                       armv7-linux-androideabi-clang clang; do
                command -v "$try" >/dev/null 2>&1 && { cc="$try"; break; }
            done
            ;;
        *)
            for try in gcc cc clang musl-gcc; do
                command -v "$try" >/dev/null 2>&1 && { cc="$try"; break; }
            done
            ;;
    esac

    echo "$cc"
}

# ─── Check for Rust ──────────────────────────────────────────────────────
check_rust() {
    if ! command -v rustc >/dev/null 2>&1; then
        err "Rust is not installed."
        echo ""
        echo "Install Rust:"
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        echo ""
        echo "Then add to PATH:"
        echo "  source ~/.cargo/env"
        echo ""
        exit 1
    fi

    local rustc_ver
    rustc_ver="$(rustc --version | sed 's/rustc \([0-9.]*\).*/\1/')"
    info "Rust version: $rustc_ver"
}

# ─── Add Rust target if needed ───────────────────────────────────────────
add_target() {
    local target="$1"
    local current_targets
    current_targets="$(rustup target list --installed 2>/dev/null || true)"

    if ! echo "$current_targets" | grep -q "$target"; then
        log "Adding Rust target: $target"
        rustup target add "$target"
    fi
}

# ─── Build ────────────────────────────────────────────────────────────────
do_build() {
    local target_os="$1" target_arch="$2"
    local rust_target=""
    local cc_var=""
    local cross=false

    # Build Rust target triple
    case "$target_arch" in
        x86_64)   rust_target="x86_64-" ;;
        aarch64)  rust_target="aarch64-" ;;
        armv7)    rust_target="armv7-" ;;
        armv6)    rust_target="armv6-" ;;
        riscv64)  rust_target="riscv64gc-" ;;
        *)        rust_target="$target_arch-" ;;
    esac

    case "$target_os" in
        linux)      rust_target="${rust_target}unknown-linux-gnu" ;;
        android)    rust_target="${rust_target}linux-android" ;;
        macos)      rust_target="${rust_target}apple-darwin" ;;
        freebsd)    rust_target="${rust_target}unknown-freebsd" ;;
        windows-wsl)
            rust_target="${rust_target}unknown-linux-gnu"
            warn "Building native Linux binary inside WSL"
            ;;
        *)          rust_target="${rust_target}unknown-linux-gnu" ;;
    esac

    # Check if we need cross-compilation
    local host_target
    host_target="$(rustc -vV 2>/dev/null | grep host | sed 's/host: //')"
    if [ "$rust_target" != "$host_target" ]; then
        cross=true
        log "Cross-compiling: $host_target -> $rust_target"
        add_target "$rust_target"

        # Set cross-compilation linker
        local cc
        cc="$(find_cc "$target_arch" "$target_os")"
        if [ -n "$cc" ]; then
            cc_var="CC_${rust_target//-/_}=cc_${rust_target}=${cc}"
            # Cargo uses underscored target names for env vars
            local cargo_target
            cargo_target="$(echo "$rust_target" | tr '-' '_')"
            export "CC_${cargo_target}=${cc}"
            info "Using C compiler: $cc"
        fi
    else
        info "Building natively for $host_target"
    fi

    log "Building zen v${VERSION} for ${target_os}/${target_arch} (${rust_target})"

    if [ "$cross" = true ]; then
        cargo build --release --target "$rust_target" --manifest-path zen/Cargo.toml
    else
        cargo build --release --manifest-path zen/Cargo.toml
    fi

    # Find the built binary
    local bin_path
    if [ "$cross" = true ]; then
        bin_path="zen/target/${rust_target}/release/zen"
    else
        bin_path="zen/target/release/zen"
    fi

    if [ ! -f "$bin_path" ]; then
        err "Build failed: binary not found at $bin_path"
        exit 1
    fi

    local size
    size="$(du -h "$bin_path" | cut -f1)"
    info "Binary size: $size"

    echo "$bin_path"
}

# ─── Install ──────────────────────────────────────────────────────────────
do_install() {
    local bin_path="$1"

    if [ -d "$INSTALL_DIR" ] && [ -w "$INSTALL_DIR" ] 2>/dev/null; then
        log "Installing to $INSTALL_DIR/zen"
        cp "$bin_path" "$INSTALL_DIR/zen"
        chmod +x "$INSTALL_DIR/zen"
    elif command -v sudo >/dev/null 2>&1; then
        log "Installing to $INSTALL_DIR/zen (sudo)"
        sudo cp "$bin_path" "$INSTALL_DIR/zen"
        sudo chmod +x "$INSTALL_DIR/zen"
    else
        # Fallback: install to ~/.local/bin
        local user_bin="$HOME/.local/bin"
        mkdir -p "$user_bin"
        log "Installing to $user_bin/zen"
        cp "$bin_path" "$user_bin/zen"
        chmod +x "$user_bin/zen"

        # Check if in PATH
        case ":$PATH:" in
            *":$user_bin:"*) ;;
            *)
                warn "$user_bin is not in your PATH."
                warn "Add this to your shell profile:"
                warn "  export PATH=\"\$HOME/.local/bin:\$PATH\""
                ;;
        esac
    fi

    log "Installed! Run: zen --help"
}

# ─── Package ──────────────────────────────────────────────────────────────
do_package() {
    local bin_path="$1" target_os="$2" target_arch="$3"
    local pkg_dir="zen-${VERSION}-${target_os}-${target_arch}"
    local pkg_name="${pkg_dir}.tar.gz"

    log "Creating package: $pkg_name"
    mkdir -p "$pkg_dir"
    cp "$bin_path" "$pkg_dir/zen"
    cp install.sh "$pkg_dir/" 2>/dev/null || true
    cp README.md "$pkg_dir/" 2>/dev/null || true

    tar czf "$pkg_name" "$pkg_dir"
    rm -rf "$pkg_dir"
    info "Package: $pkg_name"
}

# ─── Main ──────────────────────────────────────────────────────────────────
main() {
    local action="${1:-build}"
    local target_os target_arch

    target_os="$(detect_os)"
    target_arch="$(detect_arch)"

    echo ""
    echo "╔══════════════════════════════════════╗"
    echo "║       Zen Build System v${VERSION}       ║"
    echo "╚══════════════════════════════════════╝"
    echo ""

    info "OS:          $target_os"
    info "Architecture: $target_arch"
    echo ""

    case "$action" in
        build)
            check_rust
            local bin
            bin="$(do_build "$target_os" "$target_arch")"
            log "Build complete: $bin"
            ;;
        install)
            check_rust
            local bin
            bin="$(do_build "$target_os" "$target_arch")"
            do_install "$bin"
            ;;
        package)
            check_rust
            local bin
            bin="$(do_build "$target_os" "$target_arch")"
            do_package "$bin" "$target_os" "$target_arch"
            ;;
        clean)
            log "Cleaning build artifacts"
            cargo clean --manifest-path zen/Cargo.toml
            ;;
        *)
            echo "Usage: $0 [build|install|package|clean]"
            echo ""
            echo "  build     Build binary for current platform"
            echo "  install   Build and install to $INSTALL_DIR"
            echo "  package   Build and create .tar.gz package"
            echo "  clean     Remove build artifacts"
            echo ""
            echo "Env vars:"
            echo "  VERSION     Override version string"
            echo "  INSTALL_DIR Override install path (default: /usr/local/bin)"
            exit 1
            ;;
    esac
}

main "$@"
