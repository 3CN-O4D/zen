use zen_compiler::compile_source;
use zen_vm::{RefCountGc, Vm};

fn source(which: &str) -> &'static str {
    match which {
        "loop" => {
            r#"
var s = 0
var i = 0
while i < 1000000 {
    s = s + i
    i = i + 1
}
s
"#
        }
        "loop_local" => {
            r#"
func run() {
    var s = 0
    var i = 0
    while i < 1000000 {
        s = s + i
        i = i + 1
    }
    return s
}
run()
"#
        }
        "recursion" => {
            r#"
func fib(n) {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}
fib(30)
"#
        }
        "field" => {
            r#"
class P {
    func init() {
        self.x = 1
        self.y = 2
    }
}
var p = new P()
var s = 0
var i = 0
while i < 1000000 {
    s = s + p.x + p.y
    i = i + 1
}
s
"#
        }
        "method" => {
            r#"
class Counter {
    func init() { self.n = 0 }
    func inc() {
        self.n = self.n + 1
        return self.n
    }
}
var c = new Counter()
var i = 0
while i < 500000 {
    c.inc()
    i = i + 1
}
i
"#
        }
        "closure" => {
            r#"
func make(x) {
    return func() { return x }
}
var s = 0
var i = 0
while i < 200000 {
    var f = make(i)
    s = s + f()
    i = i + 1
}
s
"#
        }
        "iterate" => {
            r#"
var s = 0
for v in range(1000000) {
    s = s + v
}
s
"#
        }
        _ => panic!("unknown program {which}"),
    }
}

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "loop".into());
    let reps: usize = std::env::var("REPS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let src = source(&which);
    let module = compile_source(src).expect("compile");
    for _ in 0..reps {
        let mut vm = Vm::new(Box::new(RefCountGc));
        vm.run_module(&module.main, &module.children).expect("run");
    }
}
