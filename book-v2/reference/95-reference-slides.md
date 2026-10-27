# Chapter 95: Reference Implementation — Markdown Slides Generator

Converts a markdown file into a single HTML slideshow. Slides are separated by
a line containing `---`. Each slide becomes a full-screen section with
keyboard navigation (arrow keys) and a slide counter. Demonstrates string
splitting, HTML generation, CSS/JS embedding, and file I/O.

## 95.1 The program (`slides.z`)

```zen
import sys
import pathlib
import os

func read_all(path) {
    with open(path, "r") as f { return f.read() }
}

func write_all(path, text) {
    let p = pathlib.Path(path)
    let parent = p.parent
    if parent != nil and str(parent) != "" { os.makedirs(str(parent)) }
    with open(path, "w") as f { f.write(text) }
}

func escape(s) {
    return s.replace("&", "&amp;").replace("<", "&lt;")
         .replace(">", "&gt;").replace('"', "&quot;")
}

func heading_level(s) {
    var level = 0
    var i = 0
    while i < 6 and i < len(s) and s[i] == '#' {
        level += 1
        i += 1
    }
    if i < len(s) and s[i] == ' ' { return level }
    return 0
}

func md_to_html(text) {
    # tiny subset: headings, paragraphs, bold, lists
    let out = ""
    let lines = text.splitlines()
    var in_list = false
    for line in lines {
        let s = line.strip()
        if s == "" {
            if in_list { out = out + "</ul>\n"; in_list = false }
            continue
        }
        # heading detection: line starts with 1-6 '#' followed by space
        let level = heading_level(s)
        if level > 0 {
            if in_list { out = out + "</ul>\n"; in_list = false }
            let title = s[level:].strip()
            out = out + "<h" + str(level) + ">" + escape(title) + "</h" + str(level) + ">\n"
            continue
        }
        if s.startswith("- ") {
            if !in_list { out = out + "<ul>\n"; in_list = true }
            out = out + "  <li>" + escape(s[2:]) + "</li>\n"
            continue
        }
        if in_list { out = out + "</ul>\n"; in_list = false }
        # bold **text**
        let b = s
        while true {
            let idx = b.find("**")
            if idx == -1 { break }
            let end = b.find("**", idx + 2)
            if end == -1 { break }
            b = b[0:idx] + "<strong>" + b[idx+2:end] + "</strong>" + b[end+2:]
        }
        out = out + "<p>" + b + "</p>\n"
    }
    if in_list { out = out + "</ul>\n" }
    return out
}

func build_slides(text) {
    let raw_slides = text.split("\n---\n")
    let slides = []
    for raw in raw_slides {
        let trimmed = raw.strip()
        if trimmed == "" { continue }
        slides.push(md_to_html(trimmed))
    }
    return slides
}

let CSS = """
<style>
* { box-sizing: border-box; }
body { margin: 0; font-family: system-ui, sans-serif; background: #f5f5f5; }
.slides { position: relative; height: 100vh; overflow: hidden; }
.slide { position: absolute; top: 0; left: 0; width: 100%; height: 100%;
         display: none; flex-direction: column; justify-content: center;
         align-items: center; padding: 2rem; background: #fff; }
.slide:first-child { display: flex; }
.slide h1 { font-size: 3rem; margin-bottom: 1rem; }
.slide p { font-size: 1.25rem; line-height: 1.6; max-width: 600px;
           text-align: center; }
.slide ul { text-align: left; display: inline-block; text-align: center; }
.slide li { margin: 0.5rem 0; }
.counter { position: fixed; bottom: 1rem; right: 1rem;
           background: rgba(0,0,0,0.6); color: #fff; padding: 0.4rem 0.8rem;
           border-radius: 0.5rem; font-family: monospace; }
.hint { position: fixed; bottom: 1rem; left: 1rem; color: #666;
        font-size: 0.9rem; }
</style>
"""

let JS = """
<script>
let idx = 0;
const total = {{total}};
function show(n) {
  idx = ((n % total) + total) % total;
  document.querySelectorAll('.slide').forEach((s, i) => {
    s.style.display = (i === idx) ? 'flex' : 'none';
  });
  document.getElementById('counter').textContent = (idx + 1) + ' / ' + total;
}
document.addEventListener('keydown', (e) => {
  if (e.key === 'ArrowRight' || e.key === ' ') show(idx + 1);
  if (e.key === 'ArrowLeft') show(idx - 1);
});
show(0);
</script>
"""

func build(slides) {
    let sections = ""
    var i = 0
    while i < len(slides) {
        sections = sections + "<section class='slide'>" + slides[i] + "</section>\n"
        i += 1
    }
    let html = """<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Slides</title>
""" + CSS + """
</head>
<body>
<div class="slides">
""" + sections + """
</div>
<div class="counter" id="counter">1 / {{total}}</div>
<div class="hint">Use &larr; / &rarr; / Space to navigate</div>
""" + JS.replace("{{total}}", str(len(slides))) + """
</body>
</html>"""
    return html
}

func main(argv) {
    if len(argv) < 2 {
        print("Usage: zen run slides.z <input.md> [output.html]")
        print("  Example: zen run slides.z talk.md talk.html")
        return 1
    }
    let input = argv[1]
    let output = argv[2] ?? "slides.html"
    let text = read_all(input)
    let slides = build_slides(text)
    let html = build(slides)
    write_all(output, html)
    print("built " + str(len(slides)) + " slide(s) -> " + output)
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 95.2 Example session

Input `talk.md`:
```
# Welcome

Zen for Dummies

---

# Slide 2

- point one
- point two

---

# Thank You

Questions?
```

```
$ zen run slides.z talk.md talk.html
built 3 slide(s) -> talk.html
```

Open `talk.html` in a browser; use arrow keys / space to navigate.

## 95.3 Design notes

- **Slide separator:** a line exactly `---` splits slides.
- **Single-file output:** CSS and JS are embedded, so the HTML is portable.
- **Navigation:** arrow keys and space move forward; left arrow moves back.
- **Tiny markdown:** headings, paragraphs, bold, and bullet lists. Extend with
  code blocks, images, and links as needed.

## 95.4 Exercises

1. Add a `--title` flag to set the page title.
2. Add syntax highlighting for code blocks (conceptual).
3. Add speaker notes (hidden div, shown with `n` key).
4. Add auto-advance timing (`data-delay` attribute per slide).
5. Add PDF export hint (print CSS with `@media print`).
