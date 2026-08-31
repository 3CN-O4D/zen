#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

echo "Building release binary..."
cargo build --release 2>&1 | tail -1

echo "Building .deb package..."
cargo deb --no-build 2>&1 | tail -3

echo "Package info:"
dpkg -I target/debian/*.deb

echo "File listing:"
dpkg -c target/debian/*.deb | grep -v '/\.$'

echo ""
echo "To install locally:"
echo "  sudo dpkg -i target/debian/zen_2.1.0-1_amd64.deb"
echo "  sudo apt-get -f install   # resolve any missing deps"
