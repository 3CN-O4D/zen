# Regular Expressions (re)

## Functions

```
re.matches("^\\d+$", "123")           // true (full match)
re.search("(\\d+)", "abc 123 def")    // ZenRegexMatch
re.findall("\\d+", "a1 b22 c3")       // ["1", "22", "3"]
re.split("\\s+", "a b   c")           // ["a", "b", "c"]
re.sub("\\d+", "X", "abc 123 def")    // "abc X def"
```

## ZenRegexMatch

```
let m = re.search("(\\w+)@(\\w+)", "user@example.com")
m.match          // "user@example.com"
m.start          // 0
m.end            // 17
m.group(0)       // "user@example.com"
m.group(1)       // "user"
m.groups()       // ["user", "example"]
```
