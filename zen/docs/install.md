# Installation Guide

Zen is now powered by a native Rust engine. This completely replaces the previous Python implementation, removing the Python interpreter dependency and eliminating complex C-extension compilation errors on Termux and other platforms.

Pre-compiled native binaries are available with zero external dependencies.

---

## 1. Quick One-Line Installer

Run the automated script to detect your platform, download the correct binary, and install it to `/usr/local/bin`:

```bash
curl -fsSL https://raw.githubusercontent.com/ecnord/zen/main/install.sh | bash
```

For **Termux** (Android), this script will automatically place the executable under `$PREFIX/bin`:

```bash
curl -fsSL https://raw.githubusercontent.com/ecnord/zen/main/install.sh | termux=true bash
```

---

## 2. Pre-Compiled Binaries (Manual Download)

Download the binary matching your platform from the [Releases Page](./release.md) or GitHub Releases:

### Desktop Linux
* **Linux x86_64 (Dynamic):** `zen-x86_64-linux` (standard desktops/servers)
* **Linux x86_64 (Static Musl):** `zen-x86_64-linux-musl` (for Alpine, container environments, and maximum portability)
* **Linux ARM64:** `zen-aarch64-linux` (Raspberry Pi, ARM servers)
* **Linux ARMv7:** `zen-armv7-linux`

### Termux (Android)
No Python, pip, or gcc compilers are needed on Android anymore!
* **Termux ARM64 (aarch64):** `zen-aarch64-termux` (modern 64-bit phones)
* **Termux ARMv7 (armv7):** `zen-armv7-termux` (older 32-bit phones)
* **Termux x86_64:** `zen-x86_64-termux` (emulators)

### To Install Manually:
1. Download the file.
2. Mark it executable: `chmod +x zen-platform-arch`
3. Move it to your path: `mv zen-platform-arch /usr/local/bin/zen` (or `$PREFIX/bin/zen` on Termux)

---

## 3. From Source (Compile Locally)

If you have Rust installed (1.75+), you can build the executable directly:

```bash
# Clone the repository
git clone https://github.com/ecnord/zen
cd zen/zen-rust

# Build an optimized release executable
cargo build --release

# Run verification tests
cargo test

# The compiled binary will be located at:
# ./target/release/zen
```

Move the compiled binary to your bin path:
```bash
sudo cp target/release/zen /usr/local/bin/
```

---

## Requirements

* **Symmetric Execution:** Zero external dependencies (no Python, Node.js, OpenSSL, or C-runtime required).
* **Browser Automation (Optional):** To use browser automation commands, you must have Chrome or Chromium installed on your system. Zen connects to Chrome via its native Chrome DevTools Protocol (CDP) port.
