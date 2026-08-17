# Regex Module (`re`)

Regular expressions powered by Rust `regex` crate.

Important: `re.replace()` args are `(pattern, text, replacement)` — NOT `(text, pattern)`.

```zen
re.match("hello", "hello world")     // true
re.search("world", "hello world")     // true
re.find("\\d+", "age 42")              // ["42"]
re.split("\\s+", "a b c")              // ["a", "b", "c"]
re.replace("\\d+", "price 99", "[N/A]")  // "price [N/A]"
```
