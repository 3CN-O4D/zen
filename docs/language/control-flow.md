# Control Flow

## If / Elif / Else

```
let score = 85

if score >= 90 {
    print "A"
} elif score >= 80 {
    print "B"
} elif score >= 70 {
    print "C"
} else {
    print "D"
}
```

`else if` also works (synonymous with `elif`).

## Switch / Case

Multi-branch selection based on value equality:

```
switch command {
    case "start" {
        start_server()
    }
    case "stop" {
        stop_server()
    }
    case "restart" {
        restart_server()
    }
    default {
        print "Unknown command"
    }
}
```

The first matching case wins. `default` is optional.

## With Statement

Temporarily extend scope — useful for isolating variables:

```
with load_config() as cfg {
    print cfg["host"]
    print cfg["port"]
}
// cfg is not accessible here
```

The expression value is bound to the given name in a new child scope that is discarded after the block.

## While Loops

```
let x = 3
while x > 0 {
    print x
    x = x - 1
}
// 3, 2, 1
```

## For / In Loops

Iterate over lists:

```
for item in [1, 2, 3] {
    print item
}
// 1, 2, 3
```

Iterate over element lists:

```
for link in attrs("a", "href") {
    print link
}
```

With ranges:

```
for i in 1 -> 5 {
    print i
}
// 1, 2, 3, 4, 5
```

## Break & Continue

```
let i = 0
while true {
    i = i + 1
    if i == 2 { continue }
    print i
    if i >= 3 { break }
}
// 1, 3
```

## Try / Catch

```
try {
    click ".maybe-missing"
} catch {
    print "Element not found"
    print error    // built-in error variable
}
```

With named error:

```
try {
    risky_operation()
} catch err {
    print "Caught: " + err
}
```

With finally:

```
try {
    open_file()
} catch err {
    print "Error: " + err
} finally {
    print "This always runs"
}
```

## Throw / Raise

Explicitly raise an exception:

```
throw "Something went wrong"
raise "Invalid input"
```

With custom error objects:

```
throw {code: 404, message: "Not found"}
```

## Assert

Debug assertions that raise on failure:

```
let x = 10
assert x > 0
assert x > 0, "x must be positive"
```

## Infinite Loops

```
while true {
    // runs forever
    if should_stop { break }
}
```
