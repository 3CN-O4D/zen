# Chapter 94: Reference Implementation — Static Blog Engine

A small static blog generator: reads Markdown-ish posts from `posts/`, renders
each to HTML with a shared layout, and builds an `index.html` listing all posts
sorted by date (from front-matter). Demonstrates file walking, front-matter
parsing, templating, sorting by value, and HTML generation.

## 94.1 The program (`blog.z`)

```zen
import os
import pathlib
import sys
import re

let POSTS_DIR = "posts"
let LAYOUT_FILE = "layout.html"
let OUT_DIR = "public"

let FM_RE = r"^---\n(.*?)\n---\n"   # front-matter between --- fences

func parse_front_matter(text) {
    let meta = #{}
    let body = text
    let m = re.match(FM_RE, text)
    if m != nil {
        let block = m[1]
        body = text[m[0].length:]
        for line in block.split("\n") {
            let idx = line.find(":")
            if idx != -1 {
                let key = line[0:idx].strip()
                let val = line[idx + 1:].strip()
                if (val.startswith('"') and val.endswith('"')) or \
                   (val.startswith("'") and val.endswith("'")) {
                    val = val[1:len(val) - 1]
                }
                meta[key] = val
            }
        }
    }
    return #(meta, body)
}

func md_to_html(text) {
    # very small markdown subset: headings, paragraphs, lists, bold
    let out = ""
    let lines = text.splitlines()
    var in_list = false
    for line in lines {
        let s = line.strip()
        if s == "" {
            if in_list { out = out + "</ul>\n"; in_list = false }
            continue
        }
        let h = re.match(r"^(#{1,6})\s+(.*)", s)
        if h != nil {
            if in_list { out = out + "</ul>\n"; in_list = false }
            let level = len(h[1])
            out = out + "<h" + str(level) + ">" + escape(h[2]) + "</h" + str(level) + ">\n"
            continue
        }
        if s.startswith("- ") {
            if !in_list { out = out + "<ul>\n"; in_list = true }
            out = out + "  <li>" + escape(s[2:]) + "</li>\n"
            continue
        }
        if in_list { out = out + "</ul>\n"; in_list = false }
        out = out + "<p>" + escape(s) + "</p>\n"
    }
    if in_list { out = out + "</ul>\n" }
    return out
}

func escape(s) {
    return s.replace("&", "&amp;").replace("<", "&lt;")
         .replace(">", "&gt;").replace('"', "&quot;")
}

func load_layout() {
    let p = pathlib.Path(LAYOUT_FILE)
    if p.exists() { return read_all(str(p)) }
    return """<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>{{title}}</title></head>
<body>{{body}}</body></html>"""
}

func read_all(path) {
    with open(path, "r") as f { return f.read() }
}

func write_all(path, text) {
    let p = pathlib.Path(path)
    let parent = p.parent
    if parent != nil and str(parent) != "" { os.makedirs(str(parent)) }
    with open(path, "w") as f { f.write(text) }
}

func build() {
    let layout = load_layout()
    let posts_dir = pathlib.Path(POSTS_DIR)
    if !posts_dir.exists() {
        print("posts dir '" + POSTS_DIR + "' not found")
        return 0
    }
    os.makedirs(OUT_DIR)

    let files = []
    for name in os.listdir(POSTS_DIR) {
        if name.endswith(".md") { files.push(POSTS_DIR + "/" + name) }
    }

    let posts = []
    for f in files {
        let text = read_all(f)
        let (meta, body_md) = parse_front_matter(text)
        let body_html = md_to_html(body_md)
        let page = layout.replace("{{title}}", meta["title"] ?? "Untitled")
                    .replace("{{body}}", body_html)
        let slug = f[len(POSTS_DIR) + 1:]
        slug = slug[0:len(slug) - 3] + ".html"
        let out_path = OUT_DIR + "/" + slug
        write_all(out_path, page)
        posts.push(#{title: meta["title"] ?? "Untitled", date: meta["date"] ?? "",
                     url: slug, file: f})
        print("rendered " + f + " -> " + out_path)
    }

    # sort posts by date descending
    posts.sort(fn(a, b) {
        if a["date"] > b["date"] { return -1 }
        elif a["date"] < b["date"] { return 1 }
        return 0
    })

    # build index
    let index_items = ""
    for p in posts {
        index_items = index_items + "<li><a href='" + p["url"] + "'>" +
                      escape(p["title"]) + "</a> (" + p["date"] + ")</li>\n"
    }
    let index_html = "<h1>Blog</h1><ul>" + index_items + "</ul>"
    let index_page = layout.replace("{{title}}", "Blog")
                    .replace("{{body}}", index_html)
    write_all(OUT_DIR + "/index.html", index_page)
    print("built " + str(len(posts)) + " post(s) + index")
    return 0
}

if __name__ == "__main__" {
    sys.exit(build())
}
```

## 94.2 Example project layout

```
my-blog/
├── blog.z
├── layout.html
└── posts/
    ├── 2024-01-15-hello.md
    └── 2024-01-20-about.md
```

`posts/2024-01-20-about.md`:
```
---
title: About This Blog
date: 2024-01-20
---

# About

Welcome to my Zen-powered blog.

- post one
- post two
```

## 94.3 Expected output

```
$ zen run blog.z
rendered posts/2024-01-15-hello.md -> public/2024-01-15-hello.html
rendered posts/2024-01-20-about.md -> public/2024-01-20-about.html
built 2 post(s) + index
```

## 94.4 Design notes

- **Front-matter:** parsed from `---` fenced blocks at the top of each post.
- **Sorting:** posts sorted by `date` descending for the index.
- **Layout:** shared `layout.html` with `{{title}}`/`{{body}}` placeholders.
- **Slug:** derived from the filename (`.md` → `.html`).

## 94.4 Exercises

1. Add RSS feed generation (`rss.xml` from posts).
2. Add pagination to the index.
3. Add tags/categories from front-matter with tag pages.
4. Add `--watch` mode to rebuild on file changes.
5. Add a `--clean` flag that wipes `public/` before building.
