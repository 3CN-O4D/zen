# Chapter 55: Cookbook — Strings & Text

Complete, copy-pasteable recipes for common string and text tasks. Each recipe
includes the code and the expected output.

## 55.1 Recipe: Case-insensitive string comparison

```zen
func ieq(a, b) {
    return a.lower() == b.lower()
}
print(ieq("Zen", "ZEN"))   # true
print(ieq("hello", "HELLO"))   # true
print(ieq("abc", "abd"))   # false
```

Expected output:
```
true
true
false
```

## 55.2 Recipe: Strip HTML tags (naive regex)

```zen
import re
func strip_tags(html) {
    return re.sub(r"<[^>]+>", "", html)
}
print(strip_tags("<b>Hello</b> <i>Zen</i>!"))   # Hello Zen!
```

Expected output:
```
Hello Zen!
```

## 55.3 Recipe: Mask all but last 4 characters

```zen
func mask_all_but_last(s, n = 4) {
    if len(s) <= n { return s }
    return "*" * (len(s) - n) + s[len(s) - n:]
}
print(mask_all_but_last("1234567890"))   # ******7890
print(mask_all_but_last("Zen"))          # Zen
```

Expected output:
```
******7890
Zen
```

## 55.4 Recipe: Slugify a title

```zen
import string
func slugify(title) {
    let t = title.lower().strip()
    let out = ""
    for ch in t {
        if ch.isalnum() or ch == " " or ch == "-" {
            out = out + ch
        }
    }
    return out.replace(" ", "-")
}
print(slugify("Zen for Dummies: The Guide!"))   # zen-for-dummies-the-guide
```

Expected output:
```
zen-for-dummies-the-guide
```

## 55.5 Recipe: Extract acronym from sentence

```zen
func abbreviate(sentence) {
    let out = ""
    for w in sentence.split(" ") {
        if w != "" { out = out + w[0].upper() }
    }
    return out
}
print(abbreviate("Zen for Dummies"))   # ZFD
```

Expected output:
```
ZFD
```

## 55.6 Recipe: Validate a simple email

```zen
import re
func is_email(s) {
    return re.match(r"^[^@\s]+@[^@\s]+\.[^@\s]+$", s) != nil
}
print(is_email("ada@example.com"))   # true
print(is_email("not-an-email"))      # false
```

Expected output:
```
true
false
```

## 55.7 Recipe: Word frequency count

```zen
func word_frequencies(text) {
    let freqs = #{}
    for w in text.lower().split(" ") {
        let key = w.strip(".,!?")
        if key != "" {
            freqs[key] = (freqs[key] ?? 0) + 1
        }
    }
    return freqs
}
print(word_frequencies("a b a c b a"))   # #{a: 3, b: 2, c: 1}
```

Expected output:
```
#{a: 3, b: 2, c: 1}
```

## 55.8 Recipe: Wrap text at a width

```zen
func wrap_text(text, width) {
    let words = text.split(" ")
    let lines = []
    var line = ""
    for w in words {
        let new_len = len(line) + (1 if line != "" else 0) + len(w)
        if new_len > width {
            lines.push(line)
            line = w
        } else {
            line = line == "" ? w : line + " " + w
        }
    }
    if line != "" { lines.push(line) }
    return lines.join("\n")
}
print(wrap_text("Zen is a fast native scripting language", 12))
```

Expected output:
```
Zen is a
fast native
scripting
language
```

## 55.9 Recipe: Caesar cipher (letters only)

```zen
func caesar(text, shift) {
    let out = ""
    for ch in text {
        let o = ch.ord()
        if o >= 65 and o <= 90 {
            out = out + chr(((o - 65 + shift) % 26) + 65)
        } elif o >= 97 and o <= 122 {
            out = out + chr(((o - 97 + shift) % 26) + 97)
        } else {
            out = out + ch
        }
    }
    return out
}
print(caesar("Hello, Zen!", 3))   # Khoor, Chq!
print(caesar("Khoor, Chq!", -3))   # Hello, Zen!
```

Expected output:
```
Khoor, Chq!
Hello, Zen!
```

## 55.10 Recipe: Validate password strength

```zen
func is_strong_password(pw) {
    if len(pw) < 8 { return false }
    var has_digit = false; var has_upper = false; var has_lower = false
    var has_symbol = false
    for ch in pw {
        let o = ch.ord()
        if o >= 48 and o <= 57 { has_digit = true }
        elif o >= 65 and o <= 90 { has_upper = true }
        elif o >= 97 and o <= 122 { has_lower = true }
        else { has_symbol = true }
    }
    return has_digit and has_upper and has_lower and has_symbol
}
print(is_strong_password("Abcdef12!"))   # true
print(is_strong_password("weak"))        # false
```

Expected output:
```
true
false
```
