# Classes in Zen

Classes define objects with properties and methods.

```zen
class Person {
    function init(name) {
        self.name = name
    }
    function greet() {
        return "Hello, I am " + self.name
    }
}

let p = new Person("Ada")
print p.greet()
```

Inheritance:
```zen
class Student extends Person {
    function init(name, grade) {
        super.init(name)
        self.grade = grade
    }
}
```

Properties are set via `self.property`. The `init` method is called when using `new`.
