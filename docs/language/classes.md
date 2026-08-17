# Classes

Complete reference for defining and using classes in Zen — including constructors, methods, inheritance, properties, and common patterns.

## Defining a Class

### Basic class

```
class Dog {
    __init__ = function(self, name) {
        self.name = name
    }

    speak = function(self) {
        return self.name + " says woof"
    }
}
```

### Class without `__init__`

```
class Empty {
    greet = function(self) {
        return "Hello from " + self.str()
    }
}

let e = new Empty()
print e.greet()    // Hello from <instance of Empty>
```

---

## The `self` Keyword

Inside methods, `self` refers to the current instance. It is **automatically bound** when calling methods — you don't pass it:

```
class Counter {
    __init__ = function(self) {
        self.count = 0
    }

    increment = function(self) {
        self.count = self.count + 1
        return self    // enables chaining
    }

    get_count = function(self) {
        return self.count
    }
}

let c = new Counter()
c.increment()           // self is passed implicitly
c.increment()
print c.get_count()    // 2
```

### `self` is required in method definitions

```
class Bad {
    // WRONG — missing self
    greet = function() {
        return "hello"
    }
}

// CORRECT
class Good {
    greet = function(self) {
        return "hello"
    }
}
```

---

## Instantiation

### Using `new`

```
let d = new Dog("Rex")
print d.speak()    // Rex says woof
```

### Without `new` (calling the class directly)

```
let d = Dog("Rex")
print d.speak()    // Rex says woof (same result)
```

Both `new ClassName(args)` and `ClassName(args)` work identically.

---

## Instance Properties

### Setting properties

Properties can be set and read freely — no declarations needed:

```
class Person {
    __init__ = function(self, name, age) {
        self.name = name
        self.age = age
    }
}

let p = new Person("Alice", 30)
print p.name       // Alice
print p.age        // 30

// Add new properties dynamically
p.email = "alice@example.com"
print p.email      // alice@example.com

// Modify existing properties
p.age = 31
print p.age        // 31
```

### Properties vs methods

```
class Circle {
    __init__ = function(self, radius) {
        self.radius = radius
    }

    // Method (function)
    area = function(self) {
        return math.pi * self.radius ** 2
    }

    // Property-like (computed in __init__)
    circumference = function(self) {
        return 2 * math.pi * self.radius
    }
}

let c = new Circle(5)
print c.radius         // 5 (set in __init__)
print c.area()         // 78.53981633974483
print c.circumference() // 31.41592653589793
```

---

## Inheritance

### Basic inheritance with `extends`

```
class Animal {
    __init__ = function(self, name) {
        self.name = name
    }

    speak = function(self) {
        return self.name + " makes a sound"
    }
}

class Dog extends Animal {
    speak = function(self) {
        return self.name + " says woof"
    }
}

class Cat extends Animal {
    speak = function(self) {
        return self.name + " says meow"
    }
}

let d = new Dog("Rex")
let c = new Cat("Whiskers")

print d.speak()    // Rex says woof
print c.speak()    // Whiskers says meow
```

### Inherited methods

Methods not defined on the child are inherited from the parent:

```
class Animal {
    __init__ = function(self, name) {
        self.name = name
    }

    eat = function(self) {
        return self.name + " is eating"
    }

    speak = function(self) {
        return self.name + " makes a sound"
    }
}

class Dog extends Animal {
    // Only override speak — eat is inherited
    speak = function(self) {
        return self.name + " says woof"
    }
}

let d = new Dog("Rex")
print d.speak()    // Rex says woof (Dog's method)
print d.eat()      // Rex is eating (inherited from Animal)
```

### Multi-level inheritance

```
class Vehicle {
    __init__ = function(self, make) {
        self.make = make
    }

    describe = function(self) {
        return "Vehicle: " + self.make
    }
}

class Car extends Vehicle {
    __init__ = function(self, make, doors) {
        self.make = make
        self.doors = doors
    }

    describe = function(self) {
        return "Car: " + self.make + " (" + str(self.doors) + " doors)"
    }
}

class ElectricCar extends Car {
    __init__ = function(self, make, doors, battery) {
        self.make = make
        self.doors = doors
        self.battery = battery
    }

    describe = function(self) {
        return "EV: " + self.make + " " + str(self.battery) + "kWh"
    }
}

let tesla = new ElectricCar("Tesla", 4, 75)
print tesla.describe()    // EV: Tesla 75kWh
```

---

## Method Binding

Instance methods are automatically bound to the instance when accessed via `instance.method()`. This means you can pass methods as callbacks without losing `self`:

```
class Logger {
    __init__ = function(self, prefix) {
        self.prefix = prefix
    }

    log = function(self, msg) {
        print self.prefix + ": " + msg
    }
}

let logger = new Logger("LOG")

// Pass method as callback — self is preserved!
["hello", "world"].each(logger.log)
// LOG: hello
// LOG: world
```

### Without binding, this would break

```
// If methods weren't bound, this would fail:
let fn = logger.log
fn("test")    // Error: log expects self
```

---

## Class as Expression

Classes can be used inline without a name:

```
let Dog = class {
    __init__ = function(self, name) {
        self.name = name
    }

    speak = function(self) {
        return self.name + " says woof"
    }
}

let d = new Dog("Rex")
print d.speak()    // Rex says woof
```

### Anonymous class with inheritance

```
let Child = class extends ParentClass {
    // overrides
}
```

### Factory pattern with anonymous classes

```
function create_type(type_name) {
    return class {
        __init__ = function(self, value) {
            self.type = type_name
            self.value = value
        }

        describe = function(self) {
            return self.type + ": " + str(self.value)
        }
    }
}

let IntType = create_type("Integer")
let StringType = create_type("String")

print new IntType(42).describe()       // Integer: 42
print new StringType("hello").describe()  // String: hello
```

---

## Type Display

Instances display as `<instance of ClassName>`:

```
print new Dog("Rex")     // <instance of Dog>
d.str()                  // "<instance of Dog>"
```

---

## Common Patterns

### Builder pattern

```
class RequestBuilder {
    __init__ = function(self) {
        self.method = "GET"
        self.url = ""
        self.headers = {}
        self.body = null
    }

    set_method = function(self, method) {
        self.method = method
        return this
    }

    set_url = function(self, url) {
        self.url = url
        return this
    }

    set_header = function(self, key, value) {
        self.headers[key] = value
        return this
    }

    set_body = function(self, body) {
        self.body = body
        return this
    }

    build = function(self) {
        return {
            "method": self.method,
            "url": self.url,
            "headers": self.headers,
            "body": self.body
        }
    }
}

let req = new RequestBuilder()
    .set_method("POST")
    .set_url("https://api.example.com/users")
    .set_header("Content-Type", "application/json")
    .set_body('{"name": "Alice"}')
    .build()

print json.encode(req, {"pretty": true})
```

### Iterator pattern

```
class Range {
    __init__ = function(self, start, end, step = 1) {
        self.current = start
        self.end = end
        self.step = step
    }

    next = function(self) {
        if (self.step > 0 and self.current >= self.end) or
           (self.step < 0 and self.current <= self.end) {
            return null
        }
        let val = self.current
        self.current = self.current + self.step
        return val
    }
}

let r = new Range(0, 10, 2)
let val = r.next()
while val != null {
    print val
    val = r.next()
}
// 0, 2, 4, 6, 8
```

### Singleton-like pattern

```
class Config {
    __init__ = function(self) {
        self.data = {}
    }

    get = function(self, key) {
        return self.data[key]
    }

    set = function(self, key, value) {
        self.data[key] = value
    }
}

let config = new Config()
config.set("theme", "dark")
print config.get("theme")    // dark
```

---

## Pro Tips

1. **Always use `self` in method definitions.** Without it, the method won't receive the instance.
2. **Return `self` for method chaining.** Enables `obj.method1().method2()` patterns.
3. **Use `__init__` for setup.** Initialize all properties in the constructor.
4. **Use inheritance for "is-a" relationships.** Dog is an Animal, Car is a Vehicle.
5. **Anonymous classes are useful for factories.** Create types dynamically at runtime.

---

## Common Mistakes

### Forgetting `self` parameter

```
class Bad {
    greet = function() {       // MISSING SELF
        return "hello"
    }
}

let b = new Bad()
b.greet()    // Error: greet expects 1 argument

// CORRECT
class Good {
    greet = function(self) {
        return "hello"
    }
}
```

### Using `self` outside methods

```
class Foo {
    __init__ = function(self) {
        self.value = 42
    }
}

// WRONG — self is not available outside methods
print self.value

// CORRECT — access via instance
let f = new Foo()
print f.value    // 42
```

### Not initializing properties in `__init__`

```
class Bad {
    __init__ = function(self) {
        // forgot to set self.name
    }

    greet = function(self) {
        return "Hello, " + self.name    // Error: undefined
    }
}
```

---

## See Also

- [Functions](functions.md) — Methods, closures, and binding
- [Variables](variables.md) — Scope and `self`
- [Collections](collections.md) — Storing instances in lists/dicts
- [Control Flow](control-flow.md) — Using classes with if/else
