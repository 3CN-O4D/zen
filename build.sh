#!/usr/bin/env bash
# Zen multi-platform build/install script
# Automatically detects the OS + architecture (including Android/Termux),
# installs any missing build requirements, compiles the optimized binary,
# and installs it to the correct prefix/bin.
#
# Usage:
#   ./build.sh [--prefix <dir>] [install|build|package|deb|clean]
#
#   --prefix <dir>   Install prefix (default: /usr/local, or $PREFIX in Termux)
#   install          Build and install (default action)
#   build            Build only
#   package          Build and create a .tar.gz package
#   deb              Build and create a .deb package (needs dpkg-deb)
#   clean            Remove build artifacts
#
#   You may also pass:  ./build.sh --prefix=$PREFIX
set -euo pipefail

# Resolve the repository root (parent of this script), not a nested zen/ dir.
cd "$(dirname "$0")"
ROOT="$(pwd)"

VERSION="${VERSION:-$(grep '^version' Cargo.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')}"
MANIFEST="$ROOT/Cargo.toml"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'

log()  { echo -e "${GREEN}==> ${NC}$*"; }
warn() { echo -e "${YELLOW}==> ${NC}$*"; }
err()  { echo -e "${RED}==> ${NC}$*" >&2; }
info() { echo -e "${BLUE}    ${NC}$*"; }

# ─── Platform detection ────────────────────────────────────────────────────
detect_os() {
    local os
    os="$(uname -s)"
    case "$os" in
        Linux*)
            if [ -n "${ZEN_IS_TERMUX:-}" ] && [ "$ZEN_IS_TERMUX" = "1" ]; then
                echo "termux"
            elif [ -f /data/data/com.termux/files/usr/etc/termux_version ] 2>/dev/null || \
                 { [ -n "${PREFIX:-}" ] && [ "$(uname -o 2>/dev/null)" = "Android" ]; }; then
                echo "termux"
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

detect_arch() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64)      echo "x86_64" ;;
        aarch64|arm64)     echo "aarch64" ;;
        armv7l|armhf)      echo "armv7" ;;
        armv6l)            echo "armv6" ;;
        riscv64)           echo "riscv64" ;;
        i386|i486|i586|i686) echo "x86" ;;
        *)                 echo "$arch" ;;
    esac
}

# ─── Install missing build requirements ────────────────────────────────────
install_reqs() {
    local os="$1"

    # Rust toolchain is always required. We do not auto-install rustup (that
    # would require network + interactive shell reload), but give clear steps.
    if ! command -v cargo >/dev/null 2>&1 || ! command -v rustc >/dev/null 2>&1; then
        err "Rust toolchain (cargo/rustc) not found."
        case "$os" in
            termux)
                echo ""
                echo "  Install Rust in Termux with:"
                echo "    pkg install rust"
                echo ""
                ;;
            *)
                echo ""
                echo "  Install Rust with rustup:"
                echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
                echo "    source ~/.cargo/env"
                echo ""
                ;;
        esac
        exit 1
    fi
    info "Rust: $(rustc --version)"

    case "$os" in
        termux)
            # Termux: ensure the native build toolchain + libs are present.
            # Probe for a representative binary each package actually provides
            # (e.g. binutils ships `ld`, not a `binutils` command) so we don't
            # re-run pkg install on every invocation when the tools are present.
            local termux_pkgs='rust:rustc clang:clang binutils:ld make:make pkg-config:pkg-config'
            local missing=()
            for entry in $termux_pkgs; do
                local pkg_name="${entry%%:*}" probe="${entry##*:}"
                command -v "$probe" >/dev/null 2>&1 || missing+=("$pkg_name")
            done
            if [ "${#missing[@]}" -gt 0 ]; then
                log "Installing missing Termux packages: ${missing[*]}"
                pkg install -y "${missing[@]}"
            fi
            ;;
        linux)
            # Non-Termux Linux: just ensure a C compiler + pkg-config exist.
            if ! command -v cc >/dev/null 2>&1 && ! command -v gcc >/dev/null 2>&1 && ! command -v clang >/dev/null 2>&1; then
                warn "No C compiler found. Some crates (native-tls) need one."
                warn "Install one, e.g. on Debian/Ubuntu:  sudo apt-get install build-essential"
            fi
            ;;
        macos)
            if ! command -v cc >/dev/null 2>&1 && ! command -v clang >/dev/null 2>&1; then
                warn "Xcode command-line tools not found. Run:  xcode-select --install"
            fi
            ;;
    esac
}

# ─── Resolve the Rust target triple ───────────────────────────────────────
resolve_target() {
    local os="$1" arch="$2"
    case "$os" in
        termux)
            case "$arch" in
                aarch64) echo "aarch64-linux-android" ;;
                armv7)   echo "armv7-linux-androideabi" ;;
                x86_64)  echo "x86_64-linux-android" ;;
                x86)     echo "i686-linux-android" ;;
                riscv64) echo "riscv64-linux-android" ;;
                *)       echo "${arch}-linux-android" ;;
            esac
            ;;
        linux|windows-wsl) echo "${arch}-unknown-linux-gnu" ;;
        macos)
            [ "$arch" = "aarch64" ] && arch="aarch64"
            case "$arch" in
                aarch64) echo "aarch64-apple-darwin" ;;
                x86_64)  echo "x86_64-apple-darwin" ;;
                *)       echo "${arch}-apple-darwin" ;;
            esac
            ;;
        freebsd) echo "${arch}-unknown-freebsd" ;;
        *)       echo "${arch}-unknown-linux-gnu" ;;
    esac
}

# ─── Build ──────────────────────────────────────────────────────────────────
do_build() {
    local os="$1" arch="$2"
    local rust_target
    rust_target="$(resolve_target "$os" "$arch")"

    local host_target
    host_target="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')"

    log "Building zen v${VERSION} for ${os}/${arch} (${rust_target})"

    if [ "$rust_target" = "$host_target" ]; then
        info "Building natively for $host_target"
        cargo build --release --manifest-path "$MANIFEST"
        echo "$ROOT/target/release/zen"
    else
        # Cross-build: ensure the target is installed, then build.
        if ! rustup target list --installed 2>/dev/null | grep -qx "$rust_target"; then
            log "Adding Rust target: $rust_target"
            rustup target add "$rust_target"
        fi
        if [ "$os" = "termux" ] && [ "$rust_target" != "$host_target" ]; then
            # On Termux we build natively unless a cross target was requested.
            warn "Termux detected but target $rust_target != host $host_target; building natively."
            cargo build --release --manifest-path "$MANIFEST"
            echo "$ROOT/target/release/zen"
        else
            cargo build --release --target "$rust_target" --manifest-path "$MANIFEST"
            echo "$ROOT/target/${rust_target}/release/zen"
        fi
    fi
}

# ─── Install ──────────────────────────────────────────────────────────────────
do_install() {
    local bin_path="$1" prefix="$2"
    local site="${prefix}/bin"

    mkdir -p "$site"
    log "Installing to $site/zen"
    cp "$bin_path" "$site/zen"
    chmod +x "$site/zen"

    # The standard library must travel with the binary: imports resolve it
    # via the executable's directory walk-up ($prefix/share/zen/std).
    if [ -d "$ROOT/std" ]; then
        mkdir -p "${prefix}/share/zen"
        log "Installing stdlib to ${prefix}/share/zen/std"
        cp -r "$ROOT/std" "${prefix}/share/zen/std"
    else
        warn "std/ not found in $ROOT; stdlib imports will only resolve from the working directory"
    fi

    log "Installed! Run: zen --help"
}

# ─── Package ──────────────────────────────────────────────────────────────
do_package() {
    local bin_path="$1" os="$2" arch="$3"
    local pkg_dir="zen-${VERSION}-${os}-${arch}"
    local pkg_name="${pkg_dir}.tar.gz"

    log "Creating package: $pkg_name"
    mkdir -p "$pkg_dir"
    cp "$bin_path" "$pkg_dir/zen"
    cp "$ROOT/README.md" "$pkg_dir/" 2>/dev/null || true
    printf '%s\n' '#!/usr/bin/env bash' 'set -euo pipefail' 'PREFIX="${PREFIX:-/usr/local}"' 'SRC="$(cd "$(dirname "$0")" && pwd)"' 'mkdir -p "$PREFIX/bin" "$PREFIX/share/zen"' 'cp "$SRC/zen" "$PREFIX/bin/zen"' 'chmod +x "$PREFIX/bin/zen"' 'cp -r "$SRC/std" "$PREFIX/share/zen/std"' 'echo "Installed zen to $PREFIX/bin/zen (+ stdlib). Run: zen --version"' > "$pkg_dir/install.sh"
    chmod +x "$pkg_dir/install.sh"
    # Ship the stdlib alongside the binary so imports resolve from any
    # working directory (found via the executable walk-up).
    if [ -d "$ROOT/std" ]; then
        cp -r "$ROOT/std" "$pkg_dir/std"
    fi

    tar czf "$pkg_name" "$pkg_dir"
    rm -rf "$pkg_dir"
    info "Package: $pkg_name"
}

# ─── Debian package ───────────────────────────────────────────────────────
# Builds zen_<version>_<debian-arch>.deb with FHS layout (/usr/bin/zen,
# /usr/share/zen/std, /usr/share/doc/zen). Requires dpkg-deb.
deb_arch() {
    case "$1" in
        x86_64)  echo "amd64" ;;
        aarch64) echo "arm64" ;;
        armv7)   echo "armhf" ;;
        armv6)   echo "armel" ;;
        riscv64) echo "riscv64" ;;
        x86)     echo "i386" ;;
        *)       echo "$1" ;;
    esac
}

do_deb() {
    local bin_path="$1" arch="$2"
    local debarch
    debarch="$(deb_arch "$arch")"
    local pkgdir="debroot"
    local deb="zen_${VERSION}_${debarch}.deb"

    if ! command -v dpkg-deb >/dev/null 2>&1; then
        err "dpkg-deb not found (install dpkg-dev to build .deb packages)"
        exit 1
    fi

    log "Creating Debian package: $deb"
    rm -rf "$pkgdir"
    mkdir -p "$pkgdir/DEBIAN" "$pkgdir/usr/bin"
    mkdir -p "$pkgdir/usr/share/zen" "$pkgdir/usr/share/doc/zen"
    cp "$bin_path" "$pkgdir/usr/bin/zen"
    chmod +x "$pkgdir/usr/bin/zen"
    if [ -d "$ROOT/std" ]; then
        cp -r "$ROOT/std" "$pkgdir/usr/share/zen/std"
    else
        err "std/ not found in $ROOT; refusing to build a stdlib-less .deb"
        exit 1
    fi
    [ -f "$ROOT/CHANGELOG.md" ] && cp "$ROOT/CHANGELOG.md" "$pkgdir/usr/share/doc/zen/changelog"
    [ -f "$ROOT/README.md" ] && cp "$ROOT/README.md" "$pkgdir/usr/share/doc/zen/README"
    cat > "$pkgdir/DEBIAN/control" <<EOF
Package: zen
Version: ${VERSION}
Section: devel
Priority: optional
Architecture: ${debarch}
Maintainer: 3CN-O4D <3cn-o4d@users.noreply.github.com>
Description: Zen scripting language
 A fast, native scripting language with a Python-flavoured syntax,
 plus batteries-included modules for HTTP, files, regex, JSON,
 threading, sockets and crypto.
EOF
    dpkg-deb --root-owner-group --build "$pkgdir" "$deb"
    rm -rf "$pkgdir"
    info "Debian package: $deb ($(du -h "$deb" | cut -f1))"
}

# ─── Main ────────────────────────────────────────────────────────────────
main() {
    local action="install"
    local prefix=""
    local os arch
    os="$(detect_os)"
    arch="$(detect_arch)"

    # Extra export so detect_os knows we're on Termux even if PREFIX is unset.
    if [ "$(uname -o 2>/dev/null)" = "Android" ] || [ -f /data/data/com.termux/files/usr/etc/termux_version ] 2>/dev/null; then
        export ZEN_IS_TERMUX=1
    fi

    # Parse arguments: handle --prefix=<dir>, --prefix <dir>, and actions.
    while [ $# -gt 0 ]; do
        case "$1" in
            --prefix=*) prefix="${1#--prefix=}"; shift ;;
            --prefix)   prefix="$2"; shift 2 ;;
            build|install|package|deb|clean) action="$1"; shift ;;
            -h|--help)
                echo "Usage: $0 [--prefix <dir>] [install|build|package|deb|clean]"
                echo ""
                echo "  --prefix <dir>   Install prefix (default: /usr/local, or \$PREFIX in Termux)"
                echo "  install          Build and install (default)"
                echo "  build            Build binary only"
                echo "  package          Build and package as .tar.gz
  deb              Build and package as .deb (Debian/Ubuntu)"
                echo "  clean            Remove build artifacts"
                echo ""
                echo "Detected: OS=$os ARCH=$arch"
                exit 0
                ;;
            *) err "Unknown argument: $1 (try --help)"; exit 1 ;;
        esac
    done

    echo ""
    echo "╔═══════════════════════════════════════╗"
    echo "║        Zen Build System v${VERSION}         ║"
    echo "╚═══════════════════════════════════════╝"
    echo ""
    info "OS:          $os"
    info "Architecture: $arch"
    echo ""

    if [ "$action" = "clean" ]; then
        log "Cleaning build artifacts"
        cargo clean --manifest-path "$MANIFEST"
        exit 0
    fi

    install_reqs "$os"

    # Choose the install prefix.
    if [ -z "$prefix" ]; then
        if [ "$os" = "termux" ] && [ -n "${PREFIX:-}" ]; then
            prefix="${PREFIX}"
        else
            prefix="${INSTALL_DIR:-/usr/local}"
        fi
    fi
    info "Install prefix: $prefix"

    # do_build prints status lines via log/info (stdout) then the binary path
    # last; take only the final line so $bin is just the path.
    local bin
    bin="$(do_build "$os" "$arch" | tail -1)"

    if [ ! -f "$bin" ]; then
        err "Build failed: binary not found at $bin"
        exit 1
    fi
    info "Binary size: $(du -h "$bin" | cut -f1)"

    case "$action" in
        build)    log "Build complete: $bin" ;;
        package)  do_package "$bin" "$os" "$arch" ;;
        deb)      do_deb "$bin" "$arch" ;;
        install)  do_install "$bin" "$prefix" ;;
    esac
}

main "$@"
