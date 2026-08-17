# Control Flow in Zen

## Conditionals (`if` / `elif` / `else`)

```zen
if x > 0 {
    print "positive"
} elif x < 0 {
    print "negative"
} else {
    print "zero"
}
```

## Loops (`while` / `for`)

```zen
while i < 10 {
    i = i + 1
    if i == 5 { continue }
    if i == 7 { break }
    print i
}

for item in ["a", "b", "c"] {
    print item
}
```

`break` exits the loop. `continue` skips to the next iteration.
