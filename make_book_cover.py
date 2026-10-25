#!/usr/bin/env python3
"""Craft the Zen for Dummies book cover (front, spine, back) at print resolution.

Uses the official Zen brand: enso circle with blue gradient, white Z, green
accent dot, off-white paper background. 6x9in trade paperback @ 300 DPI with
0.125in bleed on all sides.
"""
from PIL import Image, ImageDraw, ImageFont, ImageOps, ImageFilter
import io, os

# ── brand constants ─────────────────────────────────────────────────────────
NAVY   = (30, 58, 138)      # #1e3a8a
BLUE   = (59, 130, 246)     # #3b82f6
SKY    = (96, 165, 250)     # #60a5fa
GREEN  = (16, 185, 129)     # #10b981
INK    = (15, 23, 42)       # #0f172a near-black
SLATE  = (94, 114, 164)     # #5e72a4 muted
PAPER  = (248, 249, 244)    # #f8f9f4 off-white
CREAM  = (252, 250, 240)    # warmer off-white
GOLD   = (212, 175, 55)     # accent

DPI = 300
IN  = DPI
W, H = int(6 * IN), int(9 * IN)            # 1800 x 2700 trim
BLEED = int(0.125 * IN)                     # 38 px
FW, FH = W + 2 * BLEED, H + 2 * BLEED       # full bleed canvas

SPINE_W = int(0.45 * IN)                    # ~135px spine (approx page count)

FONT_BOLD = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
FONT_REG  = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
FONT_ITAL = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Oblique.ttf"
FONT_MONO = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"

def load_logo(size):
    img = Image.open("assets/logo-terminal.png").convert("RGBA")
    return img.resize((size, size), Image.LANCZOS)

def gradient_rect(size, colors, direction="vertical"):
    w, h = size
    img = Image.new("RGB", (w, h), 0)
    draw = ImageDraw.Draw(img)
    if direction == "vertical":
        steps = h
        for i in range(steps):
            t = i / max(steps - 1, 1)
            c = tuple(
                int(colors[0][j] + (colors[1][j] - colors[0][j]) * t)
                for j in range(3)
            )
            draw.line([(0, i), (w, i)], fill=c)
    elif direction == "horizontal":
        steps = w
        for i in range(steps):
            t = i / max(steps - 1, 1)
            c = tuple(
                int(colors[0][j] + (colors[1][j] - colors[0][j]) * t)
                for j in range(3)
            )
            draw.line([(i, 0), (i, h)], fill=c)
    return img

def text(draw, xy, txt, font, fill, anchor="la", align="left"):
    draw.text(xy, txt, font=font, fill=fill, anchor=anchor, align=align)

def measure(txt, font):
    dummy = ImageDraw.Draw(Image.new("RGB", (1, 1)))
    return dummy.textlength(txt, font=font)

def main():
    os.makedirs("assets", exist_ok=True)

    # =======================================================================
    # FRONT COVER
    # =======================================================================
    front = Image.new("RGB", (FW, FH), PAPER)
    d = ImageDraw.Draw(front)

    # Deep blue field (top ~55%)
    field_h = int(FH * 0.56)
    field = gradient_rect((FW, field_h), [NAVY, BLUE], "vertical")
    front.paste(field, (0, 0))

    # Soften the horizon with a feathered band
    band = Image.new("L", (FW, int(FH * 0.06)), 0)
    bd = ImageDraw.Draw(band)
    for i in range(band.height):
        t = i / band.height
        alpha = int(255 * (1 - t))
        bd.line([(0, i), (FW, i)], fill=alpha)
    mask = band.convert("L")
    paper_strip = Image.new("RGB", (FW, band.height), PAPER)
    front.paste(paper_strip, (0, field_h - band.height // 2), mask)

    # Bottom paper area
    d.rectangle([0, field_h, FW, FH], fill=PAPER)

    # ── Enso logo (large, centered-upper) ──────────────────────────────────
    logo_size = int(FW * 0.52)
    logo = load_logo(logo_size)
    # Place logo so it straddles the blue/paper boundary
    lx = (FW - logo_size) // 2
    ly = int(FH * 0.22)
    front.paste(logo, (lx, ly), logo)

    # ── Title ──────────────────────────────────────────────────────────────
    f_title = ImageFont.truetype(FONT_BOLD, int(IN * 0.95))   # ~285pt
    f_sub   = ImageFont.truetype(FONT_ITAL, int(IN * 0.30))   # ~90pt
    f_small = ImageFont.truetype(FONT_REG, int(IN * 0.16))    # ~48pt

    title = "Zen for Dummies"
    tw = measure(title, f_title)
    tx = (FW - tw) // 2
    ty = int(FH * 0.66)

    # Title with subtle ink drop shadow
    d.text((tx + 6, ty + 6), title, font=f_title, fill=(20, 30, 50), anchor="la")
    d.text((tx, ty), title, font=f_title, fill=PAPER, anchor="la")

    # Subtitle
    sub = "The definitive guide to the Zen programming language"
    sw = measure(sub, f_sub)
    d.text(((FW - sw) // 2, ty + int(IN * 1.15)), sub, font=f_sub,
           fill=(230, 235, 245), anchor="la")

    # Series rule
    rule_y = ty + int(IN * 1.75)
    d.line([(FW * 0.30, rule_y), (FW * 0.70, rule_y)], fill=GOLD, width=3)

    # ── Author / imprint ───────────────────────────────────────────────────
    d.text((FW // 2, int(FH * 0.905)), "Zen Language Project", font=f_small,
           fill=NAVY, anchor="mm")
    d.text((FW // 2, int(FH * 0.945)), "Zen 3.0  ·  ISBN 978-0-00000-000-0", font=f_small,
           fill=SLATE, anchor="mm")

    front.save("assets/book-cover-front.png", dpi=(DPI, DPI))
    print("front saved", front.size)

    # =======================================================================
    # BACK COVER
    # =======================================================================
    back = Image.new("RGB", (FW, FH), PAPER)
    bd = ImageDraw.Draw(back)

    # Top paper area with a thin blue header band
    band_h = int(FH * 0.07)
    bd.rectangle([0, 0, FW, band_h], fill=NAVY)
    bd.rectangle([0, band_h, FW, band_h + 6], fill=GOLD)

    # Small logo top-left
    sm_logo = load_logo(int(IN * 1.2))
    back.paste(sm_logo, (BLEED, int(band_h * 0.5 - sm_logo.height // 2) + band_h), sm_logo)

    # ── Blurb ──────────────────────────────────────────────────────────────
    f_blurb = ImageFont.truetype(FONT_REG, int(IN * 0.175))
    f_blurb_b = ImageFont.truetype(FONT_BOLD, int(IN * 0.175))
    f_head  = ImageFont.truetype(FONT_BOLD, int(IN * 0.22))

    mx = BLEED + int(IN * 0.35)
    mw = FW - 2 * (BLEED + int(IN * 0.35))

    head = "About this book"
    hw = measure(head, f_head)
    bd.text((mx, int(FH * 0.15)), head, font=f_head, fill=NAVY, anchor="la")

    blurb = (
        "Zen 3.0 is a fast, native scripting language with a Python-flavoured "
        "syntax, a Cranelift JIT, embedded-Python interop (pymport), and "
        "Rust-grade coded diagnostics. It ships as a single static binary with "
        "a batteries-included standard library.\n\n"
        "This book walks you through every part of the language — from variables "
        "and control flow to classes, concurrency, sockets, HTTP, crypto, and "
        "FFI — with runnable examples you can execute yourself. Dedicated "
        "chapters cover pymport (Python interoperability), web scraping and "
        "automation (bs4, MechanicalSoup, Selenium, raw-socket CDP), and the "
        "complete crypto module.\n\n"
        "Every code sample in this book is tested and downloadable from the "
        "companion test suite in book-v2/tests/. Clone the repo, run setup.sh, "
        "and run the examples. No author paths, no hidden setup."
    )

    # Wrap blurb text
    import textwrap
    wrapper = textwrap.TextWrapper(width=78)
    lines = []
    for para in blurb.split("\n"):
        lines.extend(wrapper.wrap(para) if para else [""])

    y = int(FH * 0.15) + int(IN * 0.55)
    for i, line in enumerate(lines):
        fnt = f_blurb_b if (i == 0 and line.isupper()) else f_blurb
        bd.text((mx, y), line, font=fnt, fill=INK, anchor="la")
        y += int(IN * 0.215)

    # ── Security features strip ────────────────────────────────────────────
    sy = y + int(IN * 0.35)
    bd.text((mx, sy), "Security features", font=f_head, fill=NAVY, anchor="la")
    sy += int(IN * 0.4)
    feats = [
        "constant-time compare  ·  secure zeroize",
        "TLS with certificate pinning  ·  FIPS-ish policy",
        "safe mode (--safe)  ·  FFI capability gates",
    ]
    for feat in feats:
        bd.text((mx, sy), "✦  " + feat, font=f_blurb, fill=SLATE, anchor="la")
        sy += int(IN * 0.26)

    # ── Bottom band ────────────────────────────────────────────────────────
    by = FH - int(FH * 0.14)
    bd.rectangle([0, by, FW, FH], fill=NAVY)
    bd.rectangle([0, by, FW, by + 6], fill=GOLD)

    # White logo bottom-right corner of bottom band
    bw_logo = load_logo(int(IN * 0.75))
    # Make white version
    white_logo = Image.new("RGBA", bw_logo.size, (255, 255, 255, 255))
    alpha = bw_logo.split()[3]
    white_logo.putalpha(alpha)
    back.paste(white_logo, (FW - BLEED - bw_logo.width, by + (int(FH * 0.14) - bw_logo.height) // 2), white_logo)

    # Zen 3.0 footer
    bd.text((FW // 2, FH - int(IN * 0.28)), "Zen 3.0  ·  native scripting language",
            font=f_small, fill=(200, 210, 225), anchor="mm")
    bd.text((FW // 2, FH - int(IN * 0.18)), "https://github.com/3CN-O4D/zen",
            font=f_small, fill=(150, 170, 200), anchor="mm")

    back.save("assets/book-cover-back.png", dpi=(DPI, DPI))
    print("back saved", back.size)

    # =======================================================================
    # FULL WRAP (front + spine + back)
    # =======================================================================
    total_w = FW + SPINE_W + FW
    wrap = Image.new("RGB", (total_w, FH), PAPER)
    wd = ImageDraw.Draw(wrap)

    # Back panel
    wrap.paste(back, (0, 0))
    # Spine panel
    wrap.paste(front, (FW + SPINE_W, 0))

    # Spine: navy with vertical title
    spine = Image.new("RGB", (SPINE_W, FH), NAVY)
    sd = ImageDraw.Draw(spine)
    s_title = "Zen for Dummies"
    f_spine = ImageFont.truetype(FONT_BOLD, int(IN * 0.42))
    # Rotate title 90° CCW
    tmp = Image.new("RGBA", (int(IN * 0.5), SPINE_W), (0, 0, 0, 0))
    td = ImageDraw.Draw(tmp)
    tw = measure(s_title, f_spine)
    td.text(((tmp.width - tw) // 2, SPINE_W // 2), s_title, font=f_spine,
            fill=PAPER, anchor="mm")
    tmp = tmp.rotate(90, expand=True)
    sx = (SPINE_W - tmp.width) // 2
    sy = (FH - tmp.height) // 2
    spine.paste(tmp, (sx, sy), tmp)

    # Thin gold lines on spine
    sd.line([(SPINE_W * 0.18, int(FH * 0.42)), (SPINE_W * 0.82, int(FH * 0.42))],
            fill=GOLD, width=2)
    sd.line([(SPINE_W * 0.18, int(FH * 0.58)), (SPINE_W * 0.82, int(FH * 0.58))],
            fill=GOLD, width=2)

    wrap.paste(spine, (FW, 0))

    wrap.save("assets/book-cover-full.png", dpi=(DPI, DPI))
    print("full wrap saved", wrap.size)

    # Also make a web-friendly downscale
    front.convert("RGB").resize((600, 900), Image.LANCZOS).save(
        "assets/book-cover-front-web.jpg", quality=85)
    back.convert("RGB").resize((600, 900), Image.LANCZOS).save(
        "assets/book-cover-back-web.jpg", quality=85)

if __name__ == "__main__":
    main()
