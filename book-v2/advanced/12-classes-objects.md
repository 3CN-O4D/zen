# Chapter 12: Classes and Objects

Zen supports class-based object-oriented programming with fields, methods,
constructors (`init`), inheritance (`extends`), `super`, `this`, and bound
methods. This chapter covers OOP in depth.

## 12.1 Defining a class

```zen
class Person {
    var name
    var age

    func init(name, age) {
        this.name = name
        this.age = age
    }

    func greet() {
        return "Hi, I'm " + this.name
    }

    func birthday() {
        this.age = this.age + 1
    }
}
```

- `class Name { ... }` defines a class.
- `var name` / `var age` declare fields (mutable).
- `func init(...)` is the constructor, called by `new Person(...)`.
- `this` refers to the current instance.

## 12.2 Creating instances

```zen
let p = new Person("Zen", 30)
print(p.name)    # Zen
print(p.age)     # 30
print(p.greet()) # Hi, I'm Zen

p.birthday()
print(p.age)     # 31
```

## 12.3 Fields and methods

Fields are accessed with `this.field` inside methods and `instance.field`
outside. Methods are called with `instance.method()`.

```zen
class Counter {
    var count

    func init() {
        this.count = 0
    }

    func inc() {
        this.count = this.count + 1
    }

    func get() {
        return this.count
    }
}

let c = new Counter()
c.inc()
c.inc()
print(c.get())  # 2
```

## 12.4 Inheritance: `extends` and `super`

```zen
class Employee extends Person {
    var salary

    func init(name, age, salary) {
        super.init(name, age)
        this.salary = salary
    }

    func greet() {
        return super.greet() + " and I earn " + str(this.salary)
    }
}

let e = new Employee("Zen", 30, 100000)
print(e.name)       # Zen
print(e.age)        # 30
print(e.salary)     # 100000
print(e.greet())    # Hi, I'm Zen and I earn 100000
```

- `extends Parent` establishes inheritance.
- `super.init(...)` calls the parent constructor.
- `super.method()` calls the parent's overridden method.
- Child classes inherit all parent fields and methods.

## 12.5 Method overriding

```zen
class Shape {
    func area() {
        return 0
    }
}

class Circle extends Shape {
    var radius

    func init(radius) {
        this.radius = radius
    }

    func area() {
        return 3.14159 * this.radius * this.radius
    }
}

let c = new Circle(5)
print(c.area())  # 78.53975
```

## 12.6 `this` binding

`this` refers to the instance on which a method was called. When a method is
extracted as a bound method, `this` is preserved:

```zen
class Counter {
    var count

    func init() {
        this.count = 0
    }

    func inc() {
        this.count = this.count + 1
    }
}

let c = new Counter()
let inc = c.inc   # bound method
inc()
inc()
print(c.count)   # 2
```

## 12.7 Common patterns

### Data class

```zen
class Point {
    var x
    var y

    func init(x, y) {
        this.x = x
        this.y = y
    }

    func distance_to(other) {
        let dx = this.x - other.x
        let dy = this.y - other.y
        return (dx * dx + dy * dy) ** 0.5
    }

    func __str__() {
        return "Point(" + str(this.x) + ", " + str(this.y) + ")"
    }
}

let p1 = new Point(0, 0)
let p2 = new Point(3, 4)
print(p1.distance_to(p2))  # 5.0
print(p1)                  # Point(0, 0)
```

### Singleton

```zen
class Config {
    var instance = nil

    func init() {
        if Config.instance != nil {
            return Config.instance
        }
        Config.instance = this
        this.settings = {}
    }

    func get(key) {
        return this.settings[key]
    }

    func set(key, value) {
        this.settings[key] = value
    }
}

let c1 = new Config()
let c2 = new Config()
c1.set("theme", "dark")
print(c2.get("theme"))  # dark
print(c1 == c2)         # true (same instance)
```

## 12.8 Common pitfalls

- **`this` is required** to access fields and other methods inside a class.
  Bare `name` refers to a local variable, not a field.
- **`super.init` must be called** in a child constructor if the parent has
  required fields, or those fields won't be initialized.
- **Fields are per-instance.** Class-level (static) data is not directly
  supported; use a module-level variable instead.
- **Methods are bound** when extracted — `this` follows the instance, not the
  call site.

## 12.9 Quick reference

```zen
class Name extends Parent {
    var field

    func init(args) {
        super.init(args)
        this.field = value
    }

    func method(arg) {
        return this.field + arg
    }
}

let obj = new Name(args)
obj.field
obj.method(arg)
super.method()   # from within a child method
```

## 12.10 Exercises

1. Create a `BankAccount` class with `deposit`, `withdraw`, and `get_balance`
   methods. `withdraw` should reject amounts exceeding the balance.
2. Create a `Shape` base class with `area()` and `perimeter()` methods, then
   `Rectangle` and `Circle` subclasses.
3. Implement a `Stack` class using a list internally, with `push`, `pop`,
   `peek`, and `is_empty`.
4. Implement a `Queue` class with `enqueue` and `dequeue`.
5. Create an `Animal` base class with `speak()`, then `Dog` and `Cat`
   subclasses that override `speak()`.

## 12.12 Classes and objects — complete reference

Zen supports classes with single inheritance, constructors, methods, instance
fields, and `pub` visibility.

```zen
class Point {
    var x = 0.0
    var y = 0.0

    func init(x, y) {
        self.x = x
        self.y = y
    }

    func distance_to(other) {
        let dx = self.x - other.x
        let dy = self.y - other.y
        return (dx * dx + dy * dy) ** 0.5
    }

    func __str__() {
        return "Point(${self.x}, ${self.y})"
    }
}

let p = new Point(3.0, 4.0)
print(p.x, p.y)          # 3.0 4.0
print(p.distance_to(new Point(0.0, 0.0)))   # 5.0
print(p)                 # Point(3.0, 4.0)
```

### Class members

| Member | Declaration | Purpose |
|--------|-------------|---------|
| instance field | `var name = default` | per-object state |
| method | `func name(params) { ... }` | behavior on self |
| constructor | `func init(params) { ... }` | runs on `new` |
| static/class method | `pub func name(...) { ... }` | namespace member (no self) |

### `self` / `this`

Inside a method, `self` is the receiver. `this` is an accepted alias. Use
`self.field` to read/write fields and `self.method()` to call siblings.

```zen
class Counter {
    var n = 0
    func inc() { self.n += 1 }
    func value() { return self.n }
}
```

### Instantiation

`new ClassName(args)` calls `init`. Fields default to their declared values
when not provided in `init`.

```zen
let c = new Counter()
print(c.value())   # 0
c.inc()
print(c.value())   # 1
```

### Single inheritance

```zen
class Animal {
    var name = ""
    func init(name) { self.name = name }
    func speak() { return "..." }
}

class Dog extends Animal {
    func speak() { return self.name + " says woof" }
}

let d = new Dog("Rex")
print(d.speak())   # Rex says woof
```

Call the parent constructor explicitly if needed (`super` where supported;
otherwise re-initialize parent fields in `init`).

### Method overriding

Subclasses override methods by redefining them with the same name.

```zen
class Cat extends Animal {
    func speak() { return self.name + " says meow" }
}
```

### Visibility (`pub`)

`pub` members are accessible outside the module; non-`pub` are private to the
file.

```zen
class BankAccount {
    var balance = 0.0
    pub func deposit(amount) { self.balance += amount }
    func _apply_fee() { self.balance -= 1.0 }   # private
}
```

### `__str__` and representation

Define `__str__()` for a human-readable string form used by `print`/string
interpolation. If absent, a default representation is used.

### Object equality

By default, `==` on instances compares identity (same object). Override
`__eq__` for value equality if desired.

```zen
func __eq__(other) {
    return self.x == other.x and self.y == other.y
}
```

### Instance vs class state

Fields declared with `var` at class body level become instance fields (each
`new` gets its own). Class-level shared state (if supported) uses module-level
variables or `shared`.

### Common patterns

#### Value object (immutable-ish)

```zen
class Money {
    var amount = 0.0
    var currency = "USD"
    func init(amount, currency) { self.amount = amount; self.currency = currency }
    func add(other) {
        if self.currency != other.currency { throw "currency mismatch" }
        return new Money(self.amount + other.amount, self.currency)
    }
}
```

#### Service object

```zen
class Emailer {
    var host = ""
    func init(host) { self.host = host }
    func send(to, body) { print("sending to ${to} via ${self.host}") }
}
```

## 12.13 Common OOP mistakes

1. **Forgetting `self.`** on field access inside methods.
2. **Not calling parent init** when overriding (fields stay at defaults).
3. **Mutating shared mutable default** across instances (e.g. `var items = []`
   shared — be careful; each `new` should initialize its own).
4. **Relying on `==` for value equality** without overriding `__eq__`.
5. **Overusing inheritance** when composition is clearer.
6. **Exposing internal state** via public fields; prefer methods.

## 12.14 Exercises

1. Create a `Rectangle` class with `width`, `height`, `area()`, `perimeter()`.
2. Create a `BankAccount` class with `deposit`, `withdraw`, `balance`, and
   overdraft protection.
3. Build an `Animal` hierarchy: `Dog`, `Cat`, `Bird` with `speak()`.
4. Implement a `Stack` class using a list (`push`, `pop`, `peek`, `is_empty`).
5. Implement a `Queue` class (`enqueue`, `dequeue`, `is_empty`).
6. Create a `Fraction` class with `__eq__` and `__str__`.
7. Build a `Shape` base class with `area()`; subclasses `Circle`, `Rectangle`.
8. Implement a `Point` class with `__add__` vector addition if supported.
9. Write a `Logger` class with `info`, `warn`, `error` methods and a level.
10. Create a `Config` class wrapping a dict with typed getters.

