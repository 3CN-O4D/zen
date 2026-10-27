# Chapter 60: Reference Implementation — Static Site Generator

A complete, working static site generator: it reads Markdown-ish text files from
a `content/` directory, renders them into HTML with a template, and writes the
result to `public/`. It demonstrates file walking, string templating, JSON
front-matter parsing, and CLI argument handling.

## 60.1 The program (`ssg.z`)

```zen
import os
import pathlib
import json
import sys
import re

# ── Configuration ──────────────────────────────────────────────────────────
let CONTENT_DIR = "content"
let TEMPLATE_FILE = "template.html"
let PUBLIC_DIR = "public"

# ── Front-matter parsing ───────────────────────────────────────────────────
# Files begin with an optional YAML-ish block between --- fences:
# ---
# title: Hello World
# date: 2024-01-15
# ---

func parse_front_matter(text) {
    let meta = #{}
    let body = text
    if text.startswith("---\n") {
        let end = text.find("\n---\n", 4)
        if end != -1 {
            let block = text[4:end]
            body = text[end + 5:]
            for line in block.split("\n") {
                let idx = line.find(":")
                if idx != -1 {
                    let key = line[0:idx].strip()
                    let val = line[idx + 1:].strip()
                    # strip surrounding quotes
                    if (val.startswith('"') and val.endswith('"')) or \
                       (val.startswith("'") and val.endswith("'")) {
                        val = val[1:len(val) - 1]
                    }
                    meta[key] = val
                }
            }
        }
    }
    return #(meta, body)
}

# ── Markdown-ish rendering (subset) ────────────────────────────────────────
func md_to_html(text) {
    let out = ""
    let lines = text.splitlines()
    var in_list = false
    for line in lines {
        let stripped = line.strip()
        if stripped == "" {
            if in_list { out = out + "</ul>\n"; in_list = false }
            out = out + "\n"
            continue
        }
        # Headings
        let m = re.match(r"^(#{1,6})\s+(.*)", stripped)
        if m != nil {
            if in_list { out = out + "</ul>\n"; in_list = false }
            let level = len(m[1])
            let title = m[2]
            out = out + "<h" + str(level) + ">" + escape_html(title) + "</h" + str(level) + ">\n"
            continue
        }
        # List items
        if stripped.startswith("- ") or stripped.startswith("* ") {
            if !in_list { out = out + "<ul>\n"; in_list = true }
            out = out + "  <li>" + escape_html(stripped[2:]) + "</li>\n"
            continue
        }
        if in_list { out = out + "</ul>\n"; in_list = false }
        # Paragraph
        out = out + "<p>" + escape_html(stripped) + "</p>\n"
    }
    if in_list { out = out + "</ul>\n" }
    return out
}

func escape_html(s) {
    let r = s
    r = r.replace("&", "&amp;")
    r = r.replace("<", "&lt;")
    r = r.replace(">", "&gt;")
    r = r.replace("\"", "&quot;")
    return r
}

# ── Template ───────────────────────────────────────────────────────────────
func load_template() {
    let p = pathlib.Path(TEMPLATE_FILE)
    if p.exists() {
        return read_all(str(p))
    }
    # Built-in default template
    return """<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>{{title}}</title>
<style>
body { font-family: system-ui, sans-serif; max-width: 640px; margin: 2rem auto;
       padding: 0 1rem; line-height: 1.6; color: #333; }
h1, h2, h3 { border-bottom: 1px solid #ddd; padding-bottom: 0.5rem; }
code { background: #f4f4f4; padding: 0 0.2rem; }
a { color: #0366d6; }
</style>
</head>
<body>
{{body}}
</body>
</html>
"""
}

func render_template(tmpl, meta, body) {
    let r = tmpl.replace("{{title}}", meta["title"] ?? "Untitled")
    r = r.replace("{{body}}", body)
    return r
}

# ── File helpers ───────────────────────────────────────────────────────────
func read_all(path) {
    with open(path, "r") as f { return f.read() }
}

func write_all(path, text) {
    let p = pathlib.Path(path)
    # ensure parent dir exists
    let parent = p.parent
    if parent != nil and str(parent) != "" {
        os.makedirs(str(parent))
    }
    with open(path, "w") as f { f.write(text) }
}

func copy_static(src, dst) {
    # crude binary copy: read all bytes and write
    with open(src, "rb") as fin {
        with open(dst, "wb") as fout {
            while true {
                let chunk = fin.read(8192)
                if chunk == nil or len(chunk) == 0 { break }
                fout.write(chunk)
            }
        }
    }
}

# ── Main build ─────────────────────────────────────────────────────────────
func build() {
    let template = load_template()
    let content = pathlib.Path(CONTENT_DIR)
    if !content.exists() {
        print("content dir '" + CONTENT_DIR + "' not found; nothing to build")
        return 0
    }

    let public = pathlib.Path(PUBLIC_DIR)
    os.makedirs(PUBLIC_DIR)

    let files = find_files(CONTENT_DIR, ".md")
    var count = 0
    for src in files {
        let text = read_all(src)
        let (meta, body_md) = parse_front_matter(text)
        let body_html = md_to_html(body_md)
        let html = render_template(template, meta, body_html)

        # output path: public/<relative>.html
        let rel = src[len(CONTENT_DIR) + 1:]
        let out_rel = rel[0:len(rel) - 3] + ".html"   # strip .md, add .html
        let out = pathlib.Path(PUBLIC_DIR) / out_rel
        write_all(str(out), html)
        print("rendered " + src + " -> " + str(out))
        count += 1
    }

    # copy static assets if present
    let assets = pathlib.Path("static")
    if assets.exists() {
        for f in find_files("static", "") {
            let dst = pathlib.Path(PUBLIC_DIR) / f[len("static") + 1:]
            copy_static(f, str(dst))
        }
    }

    print("built " + str(count) + " page(s) into " + PUBLIC_DIR + "/")
    return 0
}

func find_files(dir, suffix) {
    let out = []
    func walk(d) {
        for name in os.listdir(d) {
            let p = pathlib.Path(d) / name
            if p.is_dir() {
                walk(str(p))
            } elif suffix == "" or name.endswith(suffix) {
                out.push(str(p))
            }
        }
    }
    walk(dir)
    return out
}

func main(argv) {
    if len(argv) > 1 and (argv[1] == "--help" or argv[1] == "-h") {
        print("Usage: zen run ssg.z [content_dir] [public_dir]")
        print("  content_dir  default: content")
        print("  public_dir   default: public")
        return 0
    }
    if len(argv) > 1 { CONTENT_DIR = argv[1] }   # note: config is const-like; reassignment shown for demo
    if len(argv) > 2 { PUBLIC_DIR = argv[2] }
    return build()
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 60.2 Example project layout

```
my-site/
├── ssg.z
├── template.html          # optional; built-in default if absent
├── static/
│   └── style.css
└── content/
    ├── about.md
    └── posts/
        └── hello.md
```

`content/posts/hello.md`:
```
---
title: Hello, Zen
date: 2024-01-15
---

# Welcome

This site was generated by the Zen static site generator.

- item one
- item two
```

## 60.3 Expected output

```
$ zen run ssg.z
rendered content/about.md -> public/about.html
rendered content/posts/hello.md -> public/posts/hello.html
built 2 page(s) into public/
```

`public/posts/hello.html` (excerpt):
```html
<h1>Welcome</h1>
<p>This site was generated by the Zen static site generator.</p>
<ul>
  <li>item one</li>
  <li>item two</li>
</ul>
```

## 60.4 Design notes

- Front-matter is parsed manually (no YAML dependency) to keep the example
  dependency-free.
- `find_files` walks recursively; `os.makedirs` creates output subdirectories.
- `escape_html` prevents XSS from user content.
- The default template is embedded so the program works with zero config files.

## 60.5 Extensions to try

- Add an RSS feed generator (`rss.xml` from post dates/titles).
- Add a `serve` mode that runs a local HTTP server over `public/`.
- Support `{{date}}` and other front-matter keys in the template.
- Add incremental builds (skip files whose output is newer than source).
- Add a `--clean` flag that wipes `public/` before building.
