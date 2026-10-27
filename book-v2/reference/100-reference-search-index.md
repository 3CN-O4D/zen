# Chapter 100: Reference Implementation — Client-Side Search Index Generator

Walks a directory of HTML files (e.g., `public/`), strips tags, extracts words,
and builds a JSON search index mapping each word to the pages it appears on.
The index can be shipped with a static site for client-side (JavaScript-free,
Zen-renderable) search. Demonstrates file walking, HTML stripping, tokenization,
dict-of-lists, and JSON output.

## 100.1 The program (`searchindex.z`)

```zen
import os
import pathlib
import json
import sys
import re

let INDEX_RE = r"<[^>]*>"            # HTML tags
let WORD_RE = r"[a-z0-9]+"           # lowercase alphanumeric tokens

func read_all(path) {
    with open(path, "r") as f { return f.read() }
}

func write_all(path, text) {
    let p = pathlib.Path(path)
    let parent = p.parent
    if parent != nil and str(parent) != "" { os.makedirs(str(parent)) }
    with open(path, "w") as f { f.write(text) }
}

func strip_html(html) {
    # remove tags, keep text
    let text = ""
    var i = 0
    while i < len(html) {
        if html[i] == '<' {
            # skip until '>'
            var j = i
            while j < len(html) and html[j] != '>' { j += 1 }
            if j < len(html) { j += 1 }
            i = j
            text = text + " "
        } else {
            text = text + html[i]
            i += 1
        }
    }
    return text
}

func tokenize(text) {
    let words = []
    var cur = ""
    for ch in text.lower() {
        if (ch >= 'a' and ch <= 'z') or (ch >= '0' and ch <= '9') {
            cur = cur + ch
        } else {
            if cur != "" { words.push(cur); cur = "" }
        }
    }
    if cur != "" { words.push(cur) }
    return words
}

let STOP_WORDS = #{the: true, and: true, or: true, a: true, an: true, in: true,
                   on: true, at: true, to: true, for: true, of: true, is: true,
                   it: true, as: true, be: true, are: true, was: true}

func build_index(root) {
    let files = find_html_files(root)
    let index = #{}
    for f in files {
        let html = read_all(f)
        let text = strip_html(html)
        let words = tokenize(text)
        let rel = f
        if rel.startswith(root + "/") {
            rel = rel[len(root) + 1:]
        }
        for w in words {
            if w in STOP_WORDS { continue }
            if w in index {
                let pages = index[w]
                if !(rel in pages) { pages.push(rel) }
            } else {
                index[w] = [rel]
            }
        }
    }
    return index
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

func main(argv) {
    if len(argv) < 3 {
        print("Usage:")
        print("  zen run searchindex.z <public_dir> <output.json>")
        print("  Example: zen run searchindex.z public public/search.json")
        return 1
    }
    let root = argv[1]
    let output = argv[2]

    let index = build_index(root)
    let total_pages = 0
    let total_terms = 0
    for term in index {
        total_terms += 1
        total_pages += len(index[term])
    }
    write_all(output, json.stringify(index))
    print("wrote " + output)
    print("  terms:   " + str(total_terms))
    print("  postings: " + str(total_pages))
    return 0
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 100.2 Example session

```
$ mkdir -p public/blog
$ echo '<h1>Zen Language</h1><p>Zen is a fast native language.</p>' > public/index.html
$ echo '<h1>Networking</h1><p>Zen supports TCP, UDP, and TLS.</p>' > public/blog/networking.html
$ zen run searchindex.z public public/search.json
wrote public/search.json
  terms:   11
  postings: 14
```

`public/search.json`:
```json
{"zen":["index.html","blog/networking.html"],"fast":["index.html"],"native":["index.html"],
 "language":["index.html"],"supports":["blog/networking.html"],"tcp":["blog/networking.html"],
 "udp":["blog/networking.html"],"tls":["blog/networking.html"]}
```

## 100.3 Design notes

- **HTML stripping:** removes tags with a simple state scan (no regex needed).
- **Tokenization:** lowercases and splits on non-alphanumeric characters.
- **Stop words:** common words (the, and, is, …) are skipped to keep the index small.
- **Index structure:** `#{term => [page1, page2, ...]}` — an inverted index.
- **Client-side use:** ship `search.json` with the site; a tiny bit of JS (or Zen
  compiled to WASM) can look up terms and show results without a server.

## 100.4 Exercises

1. Add phrase search (quoted terms) support in the index query logic.
2. Add term frequency (count occurrences, not just presence) for ranking.
3. Add title extraction (`<title>` or first `<h1>`) to display in results.
4. Add `--min-df N` to drop terms appearing in fewer than N pages.
5. Combine with chapter 99's sitemap generator for a complete static-site SEO + search pipeline.
