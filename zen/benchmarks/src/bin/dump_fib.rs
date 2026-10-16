use zen_compiler::compile_source;

fn dump(name: &str, src: &str) {
    let module = compile_source(src).expect("compile");
    println!("=== {name} main: locals={} params={} max_reg={} code_len={}",
        module.main.num_locals, module.main.params.len(), module.main.max_reg, module.main.code.len());
    for (i, inst) in module.main.code.iter().enumerate() {
        println!("  m {i:>4}  {:<16} a={:<3} b={:<3} c={:<3}", format!("{:?}", inst.op()), inst.a(), inst.b(), inst.c());
    }
    for (ci, p) in module.children.iter().enumerate() {
        println!("=== {name} child {ci} '{}': params={:?} locals={} max_reg={} code_len={}",
            p.name, p.params, p.num_locals, p.max_reg, p.code.len());
        for (i, inst) in p.code.iter().enumerate() {
            println!("  c{ci} {i:>4}  {:<16} a={:<3} b={:<3} c={:<3}", format!("{:?}", inst.op()), inst.a(), inst.b(), inst.c());
        }
        for (k, v) in p.constants.iter().enumerate() {
            println!("  c{ci} const[{k}] = {:?}", v);
        }
    }
}

fn main() {
    let fib = r#"
func fib(n) {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}
fib(30)
"#;
    dump("fib", fib);
}
