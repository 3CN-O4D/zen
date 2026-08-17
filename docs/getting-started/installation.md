# Installation

## Prerequisites

- **Python 3.8+**
- **pip** (Python package manager)
- **Chrome or Chromium** browser

## Install from Source

```bash
git clone https://github.com/3CN-O4D/zen.git
cd zen
pip install -e .
```

## Verify Installation

```bash
zen --version
# Zen v0.1.0
```

## Platform-Specific Notes

### Linux

DrissionPage uses your system's Chrome or Chromium. Ensure at least one is installed:

```bash
# Ubuntu/Debian
sudo apt install chromium-browser

# Fedora
sudo dnf install chromium

# Arch
sudo pacman -S chromium
```

### macOS

```bash
brew install --cask chromium
```

### Windows

Download and install [Google Chrome](https://www.google.com/chrome/) or [Chromium](https://www.chromium.org/).

### Termux (Android)

Termux's pip cannot build the C-extension dependencies (`psutil`, `lxml`), so install them from the apt repos first:

```bash
pkg update
pkg install python-psutil python-lxml
pip install -e .
```

> `python-psutil` is needed by DrissionPage (browser mode), `python-lxml` by
> soup mode (`--soup`). The apt packages ship their `.dist-info`, so pip treats
> them as already installed and skips building them from source.

Or use the install script, which does this automatically:

```bash
git clone https://github.com/3CN-O4D/zen.git
cd zen
./install.sh
```

### Headless Servers

On headless servers (no display), Zen runs in headless mode by default. For sites that detect headless browsers:

```bash
zen shell --headful  # requires X11 forwarding or VNC
```

## Docker

```dockerfile
FROM python:3.11-slim

# Install Chrome
RUN apt-get update && apt-get install -y \
    chromium \
    chromium-driver \
    && rm -rf /var/lib/apt/lists/*

# Install Zen
COPY . /app
WORKDIR /app
RUN pip install -e .

CMD ["zen", "shell"]
```

## Troubleshooting

### "DrissionPage not found"

```bash
pip install DrissionPage
```

### "Chromium not found"

Zen uses your system Chrome/Chromium. Install Chrome or use `--connect` to attach to a running browser.

### Sandbox Errors on Linux

```bash
# Run with sandbox disabled (for Docker/CI):
export CHROME_FLAGS="--no-sandbox"
```

### Permission Errors

If you get permission errors installing globally:

```bash
pip install -e . --user
# or
pip install -e . --break-system-packages
```
