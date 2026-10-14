# soup — HTML parsing

BeautifulSoup-like HTML parsing written in pure Zen. `parse(html)` builds a
tree of `Node`s; query it with `find` / `find_all` / `select`, extract text
with `get_text`, and re-indent markup with `prettify`.

```zen
import soup

let doc = soup.parse("<div class=\"post\"><p>Hello <b>World</b>!</p></div>")
print doc.find("p", null, null).get_text("")
# Hello World!
```

## Functions

| Function | Description |
|----------|-------------|
| `parse(html)` | Parses HTML into a `#doc` root `Node`. Lenient: missing end tags auto-close, void elements (`br`, `img`, …) never take children, malformed input degrades to text. |
| `escape(s)` | Escapes `&`, `<`, `>`, `"` as entities. |
| `unescape(s)` | Decodes named (`&amp;`), decimal (`&#65;`) and hex (`&#x42;`) entities; unknown entities are kept verbatim. |
| `strip_tags(html)` | Returns the text content with all markup removed. |

## Node methods

| Method | Description |
|--------|-------------|
| `find(tag, cls, id)` | First matching descendant (`null` matches anything). |
| `find_all(tag, cls, id)` | All matching descendants, in document order. |
| `find_by_id(id)` | Shorthand for `find(null, null, id)`. |
| `select(css)` | CSS subset: `tag`, `.class`, `#id`, `tag.class`, `tag#id`, and descendant chains (`div p`). |
| `get_text(sep)` | Concatenated text of the subtree, joined with `sep`. |
| `stripped_text()` | `get_text("")` with surrounding whitespace removed. |
| `attr(name, dflt)` | Attribute value, or `dflt` when absent. |
| `has_class(c)` | Whether the `class` attribute contains `c`. |
| `children()` | Child element nodes (text nodes excluded). |
| `prettify(depth)` | Re-indented markup string. |
| `is_tag()` / `is_text()` | Node-kind predicates (`#text` nodes hold raw text). |

`Node` fields (`tag`, `attrs`, `kids`, `text`, `parent`) are readable
directly. Attribute values are strings, except bare boolean attributes
(`hidden`, `disabled`), which read as `true`.

## Examples

```zen
import soup

let doc = soup.parse("<ul><li class=\"a\">one</li><li>two</li></ul>")

# CSS queries
print doc.select("li.a")[0].get_text("")   # one
print len(doc.select("li"))                # 2

# Attributes and tree navigation
let first = doc.find("li", null, null)
print first.parent.tag                     # ul
print first.attr("class", "none")          # a

# Entities and cleanup
print soup.unescape("fish &amp; chips")    # fish & chips
print soup.strip_tags("<p>Hi</p> <b>yo</b>")  # Hi yo
```
