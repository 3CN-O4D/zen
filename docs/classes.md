# Classes

Zen supports single-inheritance classes with constructors, fields, methods,
and `super`. Classes are declared with `class`, constructed with a call to the
class name (or `new`), and used like any other value.

## Declaring a class

A class body holds `var` field declarations (optional defaults) and `func`
methods. There is **no `static`**, no class-level `func` outside instances,
and no visibility modifiers.

```zen
class Counter {
    var count = 0

    func init(n) {
        this.count = n
    }

    func bump() {
        this.count = this.count + 1
        return this.count
    }
}

var c = Counter(5)
print(c.bump())     # 6
print(c.bump())     # 7
print(c.count)      # 7
```

## The constructor: `func init`

A class is instantiated by calling the class name. Inside the instance,
`this` refers to the object. The body of the first method named `init` runs as
the constructor:

```zen
class Point {
    var x = 0
    var y = 0
    func init(x, y) {
        this.x = x
        this.y = y
    }
}
var p = Point(1, 2)
print(p.x, p.y)     # 1 2
```

`new` also works and is equivalent:

```zen
var p = new Point(1, 2)
```

If a class has no `init`, it is still constructible with no arguments:

```zen
class Empty {}
var e = Empty()     # object (default init)
print(e)            # Empty()
```

## Fields & methods

Fields are declared with `var` at the top of the class body; methods with
`func`. Access both through an instance with `.`:

```zen
class Account {
    var balance = 0
    func init(opening) { this.balance = opening }
    func deposit(a) { this.balance = this.balance + a }
    func show() { return "balance=${this.balance}" }
}

var a = Account(100)
a.deposit(25)
print(a.show())     # balance=125
print(a.balance)    # 125
```

## Inheritance

`class Derived extends Base` inherits methods and fields. Use `super` to call
the parent constructor/method:

```zen
class Animal {
    var name
    func init(name) { this.name = name }
    func speak() { return "${this.name} makes a sound" }
}

class Dog extends Animal {
    func init(name) { super.init(name) }
    func speak() { return "${super.speak()} — woof!" }
}

var d = Dog("Rex")
print(d.speak())    # Rex makes a sound — woof!
```

`super.method(...)` explicitly invokes the parent implementation; unqualified
method calls dispatch through the full inheritance chain:

```zen
class A { func hi() { return "A" } }
class B extends A { func hi() { return "B" + super.hi() } }
print(B().hi())     # BA
```

## `func init` must be spelled exactly

Only the first `func init` is the constructor. A bare `init(...)` in the class
body is **not** recognized:

```zen
class Bad {
    init(x) { this.x = x }   # Error: expected func (init isn't magic by itself)
}
```

## Functions are just values

A class is a first-class value. Store it, pass it, build instances from it:

```zen
var Factory = Counter
var f = Factory(1)
print(f.bump())     # 2

fn make(kind) { return kind("x") }
var o = make(Animal) ...   # pass the class itself
```

## Missing features

- **No `static` fields/methods** — a class name is only a constructor value;
  there are no `Class.method()` calls with state.
- **No visibility modifiers** (`private`/`public`).
- **No property getters/setters** — `get`/`set` keywords are unrelated.
- **No abstract/interface/interfaces**.
- **No multiple inheritance** — `extends` takes one class.

## Errors & classes

Classes can participate in the error system through `errors.define` and
`extends errors.Error`; see [errors.md](errors.md). The usual pattern is to
throw a dict with a matching `type` key:

```zen
import errors
errors.define("DBError")
class DBError extends errors.Error {}

try {
    throw { type: "DBError", message: "connection refused" }
} catch DBError as e {
    print("typed:", e)          # typed: connection refused
} catch as e {
    print("caught:", e)
}
```

## Common pitfalls

| Mistake | Reality |
|---------|---------|
| `init(x) { }` instead of `func init(x) { }` | constructor not recognized — error |
| `class A { static f() {} }` | `static` unsupported |
| `ClassStaticMember()` on the class | only instances have methods |
| `var` field declared after methods | keep fields at the top of the body |
| multiple `extends` | single inheritance only |