use std::collections::BTreeMap;

use zen_compiler::compile_source;

fn dump(name: &str, src: &str) {
    match compile_source(src) {
        Ok(module) => {
            let mut ops: BTreeMap<String, usize> = BTreeMap::new();
            for inst in module.main.code.iter() {
                *ops.entry(format!("{:?}", inst.op())).or_insert(0) += 1;
            }
            println!("=== {name} main (len={}) ===", module.main.code.len());
            for (op, n) in ops {
                println!("  {op:16} x{n}");
            }
            for (ci, child) in module.children.iter().enumerate() {
                let mut cops: BTreeMap<String, usize> = BTreeMap::new();
                for inst in child.code.iter() {
                    *cops.entry(format!("{:?}", inst.op())).or_insert(0) += 1;
                }
                println!("  --- child{ci} {} (len={}) ---", child.name, child.code.len());
                for (op, n) in cops {
                    println!("  {op:16} x{n}");
                }
            }
        }
        Err(e) => println!("=== {name}: COMPILE ERROR {e:?}"),
    }
}

fn main() {
    dump("loop_sum", "var s = 0\nvar i = 0\nwhile i < 1000000 {\n    s = s + i\n    i = i + 1\n}\ns\n");
    dump("list", "var l = []\nvar i = 0\nwhile i < 200000 {\n    l.push(i)\n    i = i + 1\n}\nvar s = 0\nvar j = 0\nwhile j < 200000 {\n    s = s + l[j]\n    j = j + 1\n}\ns\n");
    dump("field", "class P {\n    func init() {\n        self.x = 1\n        self.y = 2\n    }\n}\nvar p = new P()\nvar s = 0\nvar i = 0\nwhile i < 1000000 {\n    s = s + p.x + p.y\n    i = i + 1\n}\ns\n");
    dump("method", "class Counter {\n    func init() {\n        self.n = 0\n    }\n    func inc() {\n        self.n = self.n + 1\n        return self.n\n    }\n}\nvar c = new Counter()\nvar i = 0\nwhile i < 500000 {\n    c.inc()\n    i = i + 1\n}\ni\n");
    dump("closure", "func make(x) {\n    return func() { return x }\n}\nvar s = 0\nvar i = 0\nwhile i < 200000 {\n    var f = make(i)\n    s = s + f()\n    i = i + 1\n}\ns\n");
    dump("string", "var s = \"\"\nvar i = 0\nwhile i < 5000 {\n    s = s + \"x\"\n    i = i + 1\n}\ns.length()\n");
    dump("iter", "var s = 0\nfor v in range(1000000) {\n    s = s + v\n}\ns\n");
    dump("dict", "var d = {}\nvar i = 0\nwhile i < 50000 {\n    d[i] = i\n    i = i + 1\n}\nvar s = 0\nvar j = 0\nwhile j < 50000 {\n    s = s + d[j]\n    j = j + 1\n}\ns\n");
}
