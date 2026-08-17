# Installation

A comprehensive guide to installing Zen on every supported platform, including troubleshooting, PATH configuration, permissions, and multi-version management.

## Prerequisites

Zen is a Rust-based scripting language. You need one of the following installation methods:

| Method | Requirements |
|--------|-------------|
| Pre-built binary | Download from GitHub releases |
| Build from source | Rust toolchain (rustup) |
| pip (wrapper) | Python 3.8+, pip |

---

## Linux

### Ubuntu / Debian (`.deb`)

```bash
# Download the latest .deb from GitHub releases
wget https://github.com/3CN-O4D/zen/releases/latest/download/zen_<version>_amd64.deb
sudo dpkg -i zen_<version>_amd64.deb
sudo apt-get install -f  # fix any missing dependencies

# Verify
zen --version
```

### Fedora / RHEL (`.rpm`)

```bash
sudo dnf install zen_<version>_amd64.rpm
zen --version
```

### Arch Linux (AUR)

```bash
# Using yay
yay -S zen

# Using makepkg
git clone https://aur.archlinux.org/zen.git
cd zen
makepkg -si

zen --version
```

### Any Linux — Install from source

```bash
# Install Rust if you don't have it
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone and build
git clone https://github.com/3CN-O4D/zen.git
cd zen
cargo build --release

# The binary is at target/release/zen — copy it to your PATH
sudo cp target/release/zen /usr/local/bin/

# Verify
zen --version
```

### Any Linux — pip install

```bash
pip install zen-lang
# or from source
git clone https://github.com/3CN-O4D/zen.git
cd zen
pip install -e .
```

---

## macOS

### Homebrew (recommended)

```bash
brew tap 3CN-O4D/zen
brew install zen

zen --version
```

### Install from source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone and build
git clone https://github.com/3CN-O4D/zen.git
cd zen
cargo build --release

# Copy to PATH
cp target/release/zen /usr/local/bin/
# or for Apple Silicon
cp target/release/zen /opt/homebrew/bin/

zen --version
```

### pip install

```bash
pip install zen-lang
```

---

## Windows

### Scoop

```powershell
scoop install zen
zen --version
```

### winget

```powershell
winget install 3CNO4D.Zen
zen --version
```

### Chocolatey

```powershell
choco install zen
zen --version
```

### Install from source (Windows)

```powershell
# Install Rust via rustup-init.exe from https://rustup.rs

# Clone and build
git clone https://github.com/3CN-O4D/zen.git
cd zen
cargo build --release

# Copy to a PATH directory
copy target\release\zen.exe C:\Users\YourUser\AppData\Local\Microsoft\WindowsApps\

zen --version
```

### pip install

```powershell
pip install zen-lang
```

---

## Termux (Android)

Termux requires special handling because some C-extension dependencies must be installed from apt before pip.

```bash
# Update package repos
pkg update && pkg upgrade

# Install build dependencies from apt
pkg install python-psutil python-lxml rust binutils

# Clone and install
git clone https://github.com/3CN-O4D/zen.git
cd zen
pip install -e .

# Verify
zen --version
```

Or use the automated install script:

```bash
git clone https://github.com/3CN-O4D/zen.git
cd zen
./install.sh
```

### Common Termux issues

```bash
# If rust is not found
pkg install rust
export PATH="$HOME/.cargo/bin:$PATH"

# If pip fails with "externally managed environment"
pip install -e . --break-system-packages
```

---

## FreeBSD

```bash
# Install Rust
pkg install rust

# Clone and build
git clone https://github.com/3CN-O4D/zen.git
cd zen
cargo build --release
sudo cp target/release/zen /usr/local/bin/

zen --version
```

---

## Docker

### Minimal Dockerfile

```dockerfile
FROM rust:slim AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/zen /usr/local/bin/
ENTRYPOINT ["zen"]
```

### Build and run

```bash
docker build -t zen .
docker run -it zen shell
docker run -it zen -e 'print 2 + 2'
```

### pip-based Dockerfile

```dockerfile
FROM python:3.11-slim
RUN pip install zen-lang
ENTRYPOINT ["zen"]
```

---

## Verifying the Installation

```bash
# Check version
zen --version
# Zen v0.x.x

# Start interactive shell
zen shell

# Run inline code
zen -e 'print "Hello from Zen!"'

# Run a script
echo 'print "It works!"' > test.z
zen run test.z
```

---

## PATH Configuration

If `zen` is not found after installation, it's likely not in your `PATH`.

### Finding where zen was installed

```bash
# If installed via cargo
ls ~/.cargo/bin/zen

# If installed via pip
which zen
# or
python -c "import shutil; print(shutil.which('zen'))"

# If installed from source
ls /usr/local/bin/zen
```

### Adding to PATH

```bash
# Add to ~/.bashrc or ~/.zshrc
export PATH="$HOME/.cargo/bin:$PATH"

# Apply immediately
source ~/.bashrc
# or
source ~/.zshrc
```

### Windows PATH

```powershell
# Check current PATH
$env:PATH -split ";"

# Add permanently (requires admin)
[Environment]::SetEnvironmentVariable("PATH", $env:PATH + ";C:\path\to\zen", "User")
```

---

## Permissions

### "Permission denied" on Linux/macOS

```bash
# Make zen executable
chmod +x /usr/local/bin/zen

# Or if installed to a user directory
chmod +x ~/.cargo/bin/zen
```

### "Permission denied" with pip

```bash
# Option 1: User install
pip install zen-lang --user

# Option 2: Virtual environment (recommended)
python -m venv ~/zen-env
source ~/zen-env/bin/activate
pip install zen-lang

# Option 3: System-wide (requires root)
sudo pip install zen-lang --break-system-packages
```

---

## Managing Multiple Versions

### Using rustup (if built from source)

```bash
# Zen is a single binary — no multi-version management needed
# Just replace the binary when upgrading
cargo build --release
sudo cp target/release/zen /usr/local/bin/zen

# Check version
zen --version
```

### Using separate directories

```bash
mkdir -p ~/zen-versions
# Download different versions to different directories
~/zen-versions/v0.1.0/zen --version
~/zen-versions/v0.2.0/zen --version

# Use a symlink for the active version
ln -sf ~/zen-versions/v0.2.0/zen ~/bin/zen
```

---

## Uninstalling

### Binary installation

```bash
# Remove the binary
sudo rm /usr/local/bin/zen
# or
rm ~/.cargo/bin/zen
```

### pip installation

```bash
pip uninstall zen-lang
```

### Source installation

```bash
cd zen
cargo uninstall zen  # if installed via cargo install
rm -rf /usr/local/bin/zen  # if manually copied
```

### Scoop (Windows)

```powershell
scoop uninstall zen
```

### Cleaning up

```bash
# Remove any config or cache files
rm -rf ~/.config/zen
rm -rf ~/.zen
rm -f ~/.zen_history
```

---

## Common Installation Errors

### "cargo: command not found"

You need to install Rust first:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### "error: linker 'cc' not found"

Install build tools:

```bash
# Ubuntu/Debian
sudo apt-get install build-essential

# Fedora
sudo dnf groupinstall "Development Tools"

# macOS
xcode-select --install
```

### "pip: externally managed environment" (PEP 668)

```bash
# Option 1: Use a virtual environment
python -m venv ~/zen-env && source ~/zen-env/bin/activate
pip install zen-lang

# Option 2: Override (use with caution)
pip install zen-lang --break-system-packages
```

### "zen: command not found" after install

```bash
# Check if it's installed
find / -name "zen" -type f 2>/dev/null

# Add the directory to PATH
export PATH="/path/to/zen:$PATH"

# Make it permanent
echo 'export PATH="/path/to/zen:$PATH"' >> ~/.bashrc
```

---

## See Also

- [Quick Start](quickstart.md) — Get started with your first Zen script
- [Shell Usage](shell.md) — Interactive REPL reference
- [CLI Reference](../cli.md) — All CLI commands and options
- [Troubleshooting](../troubleshooting.md) — Common errors and solutions
