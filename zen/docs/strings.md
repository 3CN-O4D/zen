# Strings in Zen

String literals use double `"` or single `'` quotes.

```zen
"hello".len()              // 5
"hello".length()            // same (property alias)
"hello".upper()             // "HELLO"
"hello".trim()              // trim whitespace
"hello".strip()             // alias for trim()
"hello".split(",")          // list
"hello".contains("ell")     // true
"hello".replace("l", "x")   // "hexlo"
"hello".slice(1, 3)         // "el"
```
