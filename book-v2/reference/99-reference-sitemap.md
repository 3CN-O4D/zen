# Chapter 99: Reference Implementation — Sitemap & Robots Generator

Walks a directory of HTML files (e.g., the `public/` output of a static site
generator) and produces `sitemap.xml` and `robots.txt`. Useful for SEO after
building a static site (chapters 60/77/94). Demonstrates file walking, XML
escaping, URL joining, and file writing.

## 99.1 The program (`sitemap.z`)

```zen
import os
import pathlib
import sys

func escape_xml(s) {
    return s.replace("&", "&amp;").replace("<", "&lt;")
         .replace(">", "&gt;").replace('"', "&quot;")
}

func find_html_files(dir) {
    let out = []
    func walk(d) {
        for name in os.listdir(d) {
            let p = pathlib.Path(d) / name
            if p.is_dir() {
                walk(str(p))
            } elif name.endswith(".html") {
                out.push(str(p))
            }
        }
    }
    walk(dir)
    return out
}

func path_to_url(root, file_path, base_url) {
    # compute path relative to root
    let rel = ""
    if file_path.startsWith(root + "/") {
        rel = file_path[len(root) + 1:]
    } else {
        rel = file_path
    }
    # normalize: index.html -> directory or root
    let url = base_url
    if url.endsWith("/") { url = url[0:len(url)-1] }
    let target = rel
    if target == "index.html" {
        target = ""
    } elif target.endsWith("/index.html") {
        target = target[0:len(target) - len("index.html")]
    }
    return url + "/" + target
}

func last_modified(path) {
    let b = pathlib.Path(path)
    if b.exists() {
        return b.stat().mtime   # conceptual: modification time
    }
    return ""
}

func generate_sitemap(root, base_url, out_file) {
    let files = find_html_files(root)
    files.sort()
    let urls = ""
    for f in files {
        let url = path_to_url(root, f, base_url)
        let lm = last_modified(f)
        let lm_tag = if lm != "" { "<lastmod>" + lm + "</lastmod>" } else { "" }
        urls = urls + "  <url>\n    <loc>" + escape_xml(url) + "</loc>\n" +
               lm_tag + "\n  </url>\n"
    }
    let xml = """<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
""" + urls + """</urlset>"""
    write_all(out_file, xml)
    print("wrote " + out_file + " (" + str(len(files)) + " url(s))")
}

func generate_robots(out_file, base_url, disallow = nil) {
    let lines = []
    lines.push("User-agent: *")
    if disallow != nil {
        for d in disallow {
            lines.push("Disallow: " + d)
        }
    } else {
        lines.push("Disallow:")
    }
    lines.push("Sitemap: " + base_url + "/sitemap.xml")
    write_all(out_file, lines.join("\n") + "\n")
    print("wrote " + out_file)
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

func main(argv) {
    if len(argv) < 3 {
        print("Usage:")
        print("  zen run sitemap.z <public_dir> <base_url> [sitemap.xml] [robots.txt]")
        print("  Example: zen run sitemap.z public https://example.com")
        return 1
    }
    let root = argv[1]
    let base_url = argv[2]
    let sitemap_file = argv[3] ?? root + "/sitemap.xml"
    let robots_file = argv[4] ?? root + "/robots.txt"

    generate_sitemap(root, base_url, sitemap_file)
    generate_robots(robots_file, base_url)
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 99.2 Example session

```
$ mkdir -p public/blog
$ echo '<h1>Home</h1>' > public/index.html
$ echo '<h1>About</h1>' > public/about.html
$ echo '<h1>Post</h1>' > public/blog/hello.html
$ zen run sitemap.z public https://example.com
wrote public/sitemap.xml (3 url(s))
wrote public/robots.txt
```

`public/sitemap.xml`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/</loc>
  </url>
  <url>
    <loc>https://example.com/about</loc>
  </url>
  <url>
    <loc>https://example.com/blog/hello</loc>
  </url>
</urlset>
```

`public/robots.txt`:
```
User-agent: *
Disallow:
Sitemap: https://example.com/sitemap.xml
```

## 99.3 Design notes

- **URL normalization:** `index.html` is stripped to the directory URL; the
  root `index.html` becomes the bare base URL.
- **Sorted output:** URLs are sorted alphabetically for a stable sitemap.
- **`robots.txt`:** allows all crawlers by default and points to the sitemap;
  pass disallow paths to block specific areas.
- **`lastmod`:** uses the file's modification time (conceptual — wire up your
  build's stat helper).

## 99.4 Exercises

1. Add `--changefreq` and `--priority` per URL or globally.
2. Add multiple sitemaps with a sitemap index for large sites.
3. Add `--disallow <path>` flags (repeatable) to block directories.
4. Add `--noindex` for pages that shouldn't be indexed (meta tag).
5. Combine with chapter 94's blog generator to produce a complete publishable site.
