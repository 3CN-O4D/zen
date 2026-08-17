# Classes

## Defining a Class

```
class Dog {
  __init__ = function(self, name) {
    self.name = name
  }
  speak = function(self) {
    print self.name + " says woof"
  }
}
```

## `self` Keyword

Inside methods, `self` refers to the current instance. It is automatically bound when calling methods on an instance — you don't pass it explicitly:

```
d = new Dog("Rex")
d.speak()     // Implicitly passes `d` as self
```

## Instantiation

Use `new ClassName(args)` to create an instance:

```
d = new Dog("Rex")
d.speak()     // "Rex says woof"
```

You can also call the class directly as a function:

```
d = Dog("Rex")
d.speak()     // same result
```

## Instance Properties

Set and read instance properties freely:

```
d.name = "Max"
print d.name    // "Max"
```

## Inheritance

Use `extends` to inherit from a parent class:

```
class Animal {
  __init__ = function(self, name) {
    self.name = name
  }
  speak = function(self) {
    print self.name + " makes a sound"
  }
}

class Dog extends Animal {
  speak = function(self) {
    print self.name + " says woof"
  }
}

d = new Dog("Rex")
d.speak()      // "Rex says woof" (Dog's method)
```

Methods not defined on the child are inherited from the parent:

```
a = new Animal("Generic")
a.speak()      // "Generic makes a sound"
```

## Method Binding

All instance methods are automatically bound to the instance when accessed via `instance.method()`. This means you can pass methods as callbacks without losing the `self` reference:

```
class Printer {
  __init__ = function(self, prefix) {
    self.prefix = prefix
  }
  print_msg = function(self, msg) {
    print self.prefix + ": " + msg
  }
}

p = new Printer("LOG")
["hello", "world"].each(p.print_msg)
// LOG: hello
// LOG: world
```

## No `__init__` Classes

A class without `__init__` can still be instantiated (constructor does nothing):

```
class Empty {
  greet = function(self) {
    print "Hello from " + self.str()
  }
}
e = new Empty()
e.greet()
```

## Class as Expression

Classes can be used inline without a name:

```
let Dog = class {
  __init__ = function(self, name) {
    self.name = name
  }
  speak = function(self) {
    print self.name + " says woof"
  }
}

let d = new Dog("Rex")
d.speak()    // "Rex says woof"
```

Anonymous classes are useful for factories, mixins, or one-off objects. They can also inherit:

```
let Child = class extends ParentClass {
  // overrides
}
```

## Type Display

Instances display as `<instance of ClassName>`:

```
print new Dog("Rex")     // <instance of Dog>
d.str()                  // "<instance of Dog>"
```
