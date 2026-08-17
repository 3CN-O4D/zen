# Operators

## Arithmetic

```
+  -  *  /  %  **
```

`+` also concatenates strings:

```
"Hello, " + "World!"   // "Hello, World!"
```

## Compound Assignment

```
x += 5       // x = x + 5
x -= 3       // x = x - 3
x *= 2       // x = x * 2
x /= 4       // x = x / 4
x %= 10      // x = x % 10
```

## Postfix Increment / Decrement

```
x++          // x = x + 1
x--          // x = x - 1
```

## Comparison

```
==  !=  <  >  <=  >=
```

## Strict Equality

```
===  !==
```

Strict equality checks both value and type:

```
1 == "1"     // true (loose)
1 === "1"    // false (strict)
1 === 1      // true
```

## Chained Comparisons

```
1 < 5 < 10       // true: (1 < 5) and (5 < 10)
0 <= x < 100     // true if x is between 0 and 99
10 > 5 > 2       // true
a == b == c      // true if all three are equal
```

## Identity

`is` checks strict identity (same object); `is not` checks non-identity:

```
null is null       // true
[] is []           // false (different list objects)
1 is not "1"       // true
null is not null   // false
```

## Membership

```
"world" in "hello world"      // true (substring)
42 in [1, 2, 42]              // true (list contains)
"x" in {"x": 1}               // true (dict key exists)
"z" not in {"x": 1}           // true
```

## Logical

```
and  or  not
```

Aliases:

```
&&  ||  !     // JS-style aliases
```

Short-circuit evaluation:

```
true or print("not reached")   // true (print never runs)
false and print("not reached") // false (print never runs)
```

## Nullish Coalescing

Returns the right side only when the left is `null`:

```
null ?? "default"        // "default"
"hello" ?? "default"     // "hello"
0 ?? "default"           // 0 (not null, so kept)
"" ?? "default"          // "" (not null, so kept)
```

## Ternary Conditional

Inline if/else as an expression:

```
let grade = "pass" if score >= 50 else "fail"
let label = "high" if x > 100 else "low" if x > 50 else "zero"
let max = a if a > b else b
```

## Range (`->`, `..`, `to`)

Create inclusive numeric ranges:

```
1 -> 5           // [1, 2, 3, 4, 5]
1 .. 10          // [1, 2, ..., 10]
1 to 10          // same
5 -> -5          // [5, 4, 3, 2, 1, 0, -1, -2, -3, -4, -5]
```

Direction is auto-detected: when start > end, the range descends.

With step:

```
1 -> 10 by 2     // [1, 3, 5, 7, 9]
1 .. 10 @ 3      // [1, 4, 7, 10]
10 -> 1 by -1    // [10, 9, ..., 1]
```

## Spread (`...`)

Unpacks iterables inside list and dict literals:

```
let a = [1, 2, 3]
let b = [...a, 4, 5]          // [1, 2, 3, 4, 5]

let d1 = {"x": 1}
let d2 = {...d1, "y": 2}      // {"x": 1, "y": 2}
let merged = {...d1, ...d2}
```

## Safe Navigation (`?.`)

Access properties or call methods on values that might be `null`:

```
let name = user?.name          // null if user is null
let result = obj?.method()     // null if obj is null
```

## typeof Operator

Get the type name as a string:

```
typeof 42         // "int"
typeof "hello"    // "string"
typeof [1, 2, 3]  // "list"
typeof {a: 1}     // "dict"
typeof null       // "null"
typeof true       // "bool"
typeof 3.14       // "float"
```

## Bitwise Operators

```
& | ^ ~ << >>
```

```
5 & 3        // 1 (bitwise AND)
5 | 3        // 7 (bitwise OR)
5 ^ 3        // 6 (bitwise XOR)
~5           // -6 (bitwise NOT)
5 << 1       // 10 (left shift)
5 >> 1       // 2 (right shift)
```

## Operator Precedence (highest to lowest)

```
**               exponentiation
-  !  not  typeof ~  unary
*  /  %          multiplication
+  -             addition
->  ..  to      range (right-associative)
== != < > <= >=  comparison
is  is not      identity
in  not in       membership
&                bitwise AND
^                bitwise XOR
|                bitwise OR
<< >>            bit shift
and  &&         logical and
or  ||          logical or
??               nullish coalescing
if else          ternary (right-associative)
= += -= *= /= %= ??=  assignment
```
