#!/usr/bin/env bash
# Build "Zen for Dummies" PDF from the book-v2 markdown chapters.
# Includes the crafted front/back cover (assets/book-cover-*.png).
set -euo pipefail

cd /home/bard/Desktop/zen

OUT="book-v2/Zen-for-Dummies.pdf"
TMP="$(mktemp -d)"
BOOK="$TMP/book.md"

# ── title page (from introduction) ─────────────────────────────────────────
cat book-v2/00-INTRODUCTION.md > "$BOOK"
printf '\n\n\\newpage\n\n' >> "$BOOK"

# ── chapters in TOC order (only files that exist on disk) ──────────────────
CHAPTERS=(
  book-v2/fundamentals/02-data-types.md
  book-v2/fundamentals/03-operators.md
  book-v2/fundamentals/05-functions.md
  book-v2/fundamentals/06-lambdas-closures.md
  book-v2/core/11-error-handling.md
  book-v2/modules/25-crypto-complete.md
  book-v2/networking/26-networking-fundamentals.md
  book-v2/networking/27-socket-module.md
  book-v2/networking/28-http-protocol.md
  book-v2/networking/31-https-ssl-certs.md
  book-v2/concurrency/35-threading-fundamentals.md
  book-v2/concurrency/36-channels.md
  book-v2/interop/42-pymport-python-interoperability.md
  book-v2/projects/44-web-automation.md
  book-v2/projects/45-rest-api-client.md
  book-v2/projects/46-file-processing.md
  book-v2/projects/47-security-tools.md
  book-v2/projects/48-capstone.md
)

for ch in "${CHAPTERS[@]}"; do
  if [ -f "$ch" ]; then
    printf '\n\n\\newpage\n\n' >> "$BOOK"
    cat "$ch" >> "$BOOK"
    printf '\n' >> "$BOOK"
  else
    echo "skip (missing): $ch" >&2
  fi
done

# ── build body PDF ─────────────────────────────────────────────────────────
BODY="$TMP/body.pdf"
pandoc "$BOOK" \
  -o "$BODY" \
  --pdf-engine=lualatex \
  --toc \
  --toc-depth=2 \
  --number-sections \
  --variable=geometry:margin=1in \
  --variable=fontsize=11pt \
  --variable=mainfont="DejaVu Serif" \
  --variable=monofont="DejaVu Sans Mono" \
  --variable=sansfont="DejaVu Sans" \
  --variable=colorlinks=true \
  --variable=linkcolor=blue \
  --variable=urlcolor=blue \
  --variable=toccolor=blue \
  --highlight-style=tango \
  --from markdown \
  2>&1 | tail -5

# ── merge cover + body ─────────────────────────────────────────────────────
pdftk A=assets/book-cover-front.png B="$BODY" cat A B output "$OUT" 2>/dev/null || {
  # pdftk not available: use pypdf to merge (cover as first page via img2pdf)
  python3 - "$BODY" "$OUT" <<'PY'
import sys, io
from PIL import Image
from pypdf import PdfReader, PdfWriter

cover = Image.open("assets/book-cover-front.png").convert("RGB")
buf = io.BytesIO()
cover.save(buf, format="PDF")
cover_pdf = PdfReader(buf)
body = PdfReader(sys.argv[1])
writer = PdfWriter()
for p in cover_pdf.pages:
    writer.add_page(p)
for p in body.pages:
    writer.add_page(p)
with open(sys.argv[2], "wb") as f:
    writer.write(f)
PY
}

echo
echo "PDF written to: $OUT"
ls -la "$OUT"
rm -rf "$TMP"
