#!/usr/bin/env bash
# Build "Zen for Dummies" PDF from the book-v2 markdown chapters.
set -euo pipefail

cd /home/bard/Desktop/zen

OUT="book-v2/Zen-for-Dummies.pdf"
TMP="$(mktemp -d)"
BOOK="$TMP/book.md"

# title page
cat book-v2/00-INTRODUCTION.md > "$BOOK"
printf '\n\n\\newpage\n\n' >> "$BOOK"

# all chapters in TOC order
CHAPTERS=(
  book-v2/fundamentals/01-getting-started.md
  book-v2/fundamentals/02-data-types.md
  book-v2/fundamentals/03-operators.md
  book-v2/fundamentals/04-control-flow.md
  book-v2/fundamentals/05-functions.md
  book-v2/fundamentals/06-lambdas-closures.md
  book-v2/core/07-strings.md
  book-v2/core/08-lists.md
  book-v2/core/09-dictionaries.md
  book-v2/core/10-modules-imports.md
  book-v2/core/11-error-handling.md
  book-v2/advanced/12-classes-objects.md
  book-v2/advanced/13-file-io.md
  book-v2/advanced/14-memory-references.md
  book-v2/modules/15-math.md
  book-v2/modules/16-random.md
  book-v2/modules/17-string.md
  book-v2/modules/18-collections.md
  book-v2/modules/19-json.md
  book-v2/modules/20-time-datetime.md
  book-v2/modules/21-path-os.md
  book-v2/modules/22-regex.md
  book-v2/modules/23-base64-binascii.md
  book-v2/modules/24-hashlib.md
  book-v2/modules/25-crypto-complete.md
  book-v2/networking/26-networking-fundamentals.md
  book-v2/networking/27-socket-module.md
  book-v2/networking/28-http-protocol.md
  book-v2/networking/29-requests-module.md
  book-v2/networking/30-http-server.md
  book-v2/networking/31-https-ssl-certs.md
  book-v2/networking/32-dns.md
  book-v2/networking/33-url.md
  book-v2/networking/34-raw-sockets.md
  book-v2/networking/68-networking-deep-dive.md
  book-v2/concurrency/35-threading-fundamentals.md
  book-v2/concurrency/36-channels.md
  book-v2/concurrency/37-mutexes-locks.md
  book-v2/concurrency/38-thread-safety-patterns.md
  book-v2/ffi/39-ffi-fundamentals.md
  book-v2/ffi/40-ffi-practice.md
  book-v2/system/41-sys-module.md
  book-v2/interop/42-pymport-python-interoperability.md
  book-v2/advanced/50-vm-runtime-internals.md
  book-v2/advanced/51-type-system-in-depth.md
  book-v2/advanced/52-bytecode-reference.md
  book-v2/advanced/53-garbage-collection.md
  book-v2/advanced/54-jit-compiler.md
  book-v2/cookbook/55-cookbook-strings-text.md
  book-v2/cookbook/56-cookbook-files-data.md
  book-v2/cookbook/57-cookbook-networking-web.md
  book-v2/cookbook/58-cookbook-concurrency-system.md
  book-v2/cookbook/59-cookbook-crypto-security.md
  book-v2/reference/60-reference-static-site-generator.md
  book-v2/reference/61-reference-chat-server.md
  book-v2/reference/62-reference-keyvalue-store.md
  book-v2/reference/63-reference-port-scanner.md
  book-v2/reference/64-reference-log-analyzer.md
  book-v2/reference/65-reference-http-monitor.md
  book-v2/reference/66-reference-file-encryptor.md
  book-v2/reference/67-reference-link-checker.md
  book-v2/reference/69-reference-task-queue.md
  book-v2/reference/70-reference-password-tool.md
  book-v2/reference/71-reference-http-kv-server.md
  book-v2/reference/72-reference-file-backup.md
  book-v2/reference/73-reference-csv-json.md
  book-v2/reference/74-reference-scheduler.md
  book-v2/reference/75-reference-dns-tool.md
  book-v2/reference/76-reference-chat-client.md
  book-v2/reference/77-reference-static-file-server.md
  book-v2/reference/78-reference-grep-tool.md
  book-v2/reference/79-reference-test-runner.md
  book-v2/reference/80-reference-url-shortener.md
  book-v2/reference/81-reference-tail-follow.md
  book-v2/reference/82-reference-http-load-tester.md
  book-v2/reference/83-reference-host-monitor.md
  book-v2/reference/84-reference-json-pretty.md
  book-v2/reference/85-reference-base64-tool.md
  book-v2/reference/86-reference-word-frequency.md
  book-v2/reference/87-reference-env-template.md
  book-v2/reference/88-reference-file-dedup.md
  book-v2/reference/89-reference-mock-api.md
  book-v2/reference/90-reference-settings.md
  book-v2/reference/91-reference-log-rotator.md
  book-v2/reference/92-reference-http-cache.md
  book-v2/reference/93-reference-batch-rename.md
  book-v2/reference/94-reference-static-blog.md
  book-v2/reference/95-reference-slides.md
  book-v2/reference/96-reference-health-dashboard.md
  book-v2/reference/97-reference-resume.md
  book-v2/reference/98-reference-personal-dashboard.md
  book-v2/reference/99-reference-sitemap.md
  book-v2/reference/100-reference-search-index.md
  book-v2/reference/101-reference-oneliners.md
  book-v2/appendix/k-where-to-go-next.md
  book-v2/projects/43-cli-tools.md
  book-v2/projects/44-web-automation.md
  book-v2/projects/45-rest-api-client.md
  book-v2/projects/46-file-processing.md
  book-v2/projects/47-security-tools.md
  book-v2/projects/48-capstone.md
  book-v2/appendix/a-builtins.md
  book-v2/appendix/b-error-codes.md
  book-v2/appendix/c-cli-reference.md
  book-v2/appendix/d-stdlib-index.md
  book-v2/appendix/e-quick-reference.md
  book-v2/appendix/f-complete-reference.md
  book-v2/appendix/g-exercise-solutions.md
  book-v2/appendix/h-troubleshooting.md
  book-v2/appendix/i-platform-differences.md
  book-v2/appendix/j-stdlib-api-reference.md
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
  2>&1 | tail -3

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

echo "PDF written to: $OUT"
pdfinfo "$OUT" | grep Pages
rm -rf "$TMP"
