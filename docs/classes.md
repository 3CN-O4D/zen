# Classes in Zen

## Basic Class

```zen
class Person {
    function init(name) {
        self.name = name
    }
    function greet() {
        return "Hello, " + self.name
    }
}
let p = new Person("Ada")
print p.greet()   // "Hello, Ada"
```

## Inheritance

```zen
class Student extends Person {
    function init(name, grade) {
        super.init(name)
        self.grade = grade
    }
}
let s = new Student("Grace", "A")
```

## Properties
Set via `self.property` in methods. No explicit declaration needed.
