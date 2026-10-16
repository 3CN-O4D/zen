//! End-to-end execution tests: compile Zen source with `zen-compiler` and run
//! the resulting bytecode on the `zen-vm` interpreter.

use zen_compiler::compile_source;
use zen_vm::{RefCountGc, Value, Vm, VmError};

fn run(src: &str) -> Value {
    let module =
        compile_source(src).unwrap_or_else(|e| panic!("compile failed: {e:?}\nsource:\n{src}"));
    let mut vm = Vm::new(Box::new(RefCountGc));
    vm.run_module(&module.main, &module.children)
        .unwrap_or_else(|e| panic!("runtime error: {e}\nsource:\n{src}"))
}

fn run_vm(src: &str) -> Vm {
    let module =
        compile_source(src).unwrap_or_else(|e| panic!("compile failed: {e:?}\nsource:\n{src}"));
    let mut vm = Vm::new(Box::new(RefCountGc));
    vm.run_module(&module.main, &module.children)
        .unwrap_or_else(|e| panic!("runtime error: {e}\nsource:\n{src}"));
    vm
}

fn int(v: Value) -> i64 {
    v.as_int()
        .unwrap_or_else(|| panic!("expected int, got {:?}", v.type_name()))
}

fn float(v: Value) -> f64 {
    v.as_float()
        .unwrap_or_else(|| panic!("expected float, got {:?}", v.type_name()))
}

fn text(v: Value) -> String {
    v.as_str()
        .map(str::to_string)
        .unwrap_or_else(|| panic!("expected string, got {:?}", v.type_name()))
}

fn ints(v: Value) -> Vec<i64> {
    v.as_list()
        .unwrap_or_else(|| panic!("expected list, got {:?}", v.type_name()))
        .iter()
        .map(|e| {
            e.as_int()
                .unwrap_or_else(|| panic!("expected int element, got {:?}", e.type_name()))
        })
        .collect()
}

fn strings(v: Value) -> Vec<String> {
    v.as_list()
        .unwrap_or_else(|| panic!("expected list, got {:?}", v.type_name()))
        .iter()
        .map(|e| {
            e.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| panic!("expected string element, got {:?}", e.type_name()))
        })
        .collect()
}

fn yes(v: Value) -> bool {
    v.as_bool()
        .unwrap_or_else(|| panic!("expected bool, got {:?}", v.type_name()))
}

#[test]
fn arithmetic_precedence() {
    assert_eq!(int(run("return 1 + 2 * 3")), 7);
}

#[test]
fn float_division() {
    assert_eq!(float(run("return 10 / 4")), 2.5);
}

#[test]
fn modulo() {
    assert_eq!(int(run("return 10 % 3")), 1);
}

#[test]
fn negation() {
    assert_eq!(int(run("return -5 + 5")), 0);
}

#[test]
fn num_to_string_concat() {
    assert_eq!(text(run("return 2 + \"x\"")), "2x");
    assert_eq!(text(run("return \"a\" + 1")), "a1");
}

#[test]
fn comparison_and_boolean_ops() {
    assert!(run("return 1 < 2 && 2 <= 2").as_bool().unwrap());
    assert!(run("return 1 > 2 || 2 > 1").as_bool().unwrap());
    assert!(run("return !(1 > 2)").as_bool().unwrap());
    assert!(!run("return 1 < 2 && 2 > 2").as_bool().unwrap());
}

#[test]
fn if_else_selects_branch() {
    let src = "
        var x = 10
        if x > 5 { return \"big\" } else { return \"small\" }
    ";
    assert_eq!(text(run(src)), "big");
}

#[test]
fn while_loop_accumulates() {
    let src = "
        var i = 0
        var total = 0
        while i < 5 {
            total = total + i
            i = i + 1
        }
        return total
    ";
    assert_eq!(int(run(src)), 10);
}

#[test]
fn function_call() {
    let src = "
        func add(a, b) { return a + b }
        return add(2, 3)
    ";
    assert_eq!(int(run(src)), 5);
}

#[test]
fn recursive_factorial() {
    let src = "
        func fact(n) {
            if n <= 1 { return 1 }
            return n * fact(n - 1)
        }
        return fact(5)
    ";
    assert_eq!(int(run(src)), 120);
}

#[test]
fn closure_captures_local() {
    let src = "
        var n = 10
        var f = func() { return n + 1 }
        return f()
    ";
    assert_eq!(int(run(src)), 11);
}

#[test]
fn string_methods_and_index() {
    let src = "
        var s = \"hello\"
        return s.upper() + s[0]
    ";
    assert_eq!(text(run(src)), "HELLOh");
    assert_eq!(int(run("return \"hello\".length()")), 5);
}

#[test]
fn list_index_and_mutation() {
    let src = "
        var l = [1, 2, 3]
        l.push(4)
        l.pop()
        return l[1] + l.length()
    ";
    assert_eq!(int(run(src)), 5);
}

#[test]
fn dict_index_field_and_insert() {
    let src = "
        var d = {a: 1, b: 2}
        d[\"c\"] = 3
        d.d = 4
        return d[\"a\"] + d.b + d.c + d.d
    ";
    assert_eq!(int(run(src)), 10);
}

#[test]
fn for_loop_over_list() {
    let src = "
        var total = 0
        for x in [1, 2, 3, 4] {
            total = total + x
        }
        return total
    ";
    assert_eq!(int(run(src)), 10);
}

#[test]
fn print_captures_output() {
    let vm = run_vm("print(\"hello\")\nprint(1 + 2)");
    assert_eq!(vm.output(), "hello\n3\n");
}

#[test]
fn classes_construct_and_dispatch_methods() {
    let src = "
        class Counter {
            func init(start) {
                self.n = start
            }
            func inc(by) {
                self.n = self.n + by
                return self.n
            }
            func get() {
                return self.n
            }
        }
        var a = new Counter(10)
        var b = new Counter(100)
        a.inc(1)
        a.inc(2)
        return a.get() == 13 && b.get() == 100 && a.inc(7) == 20
    ";
    assert!(run(src).as_bool().unwrap());
}

#[test]
fn classes_inherit_parent_methods() {
    let src = "
        class Base {
            func name() {
                return \"base\"
            }
        }
        class Child extends Base {
            func greet() {
                return \"hi \" + self.name()
            }
        }
        var c = new Child()
        return c.greet()
    ";
    assert_eq!(text(run(src)), "hi base");
}

#[test]
fn nested_functions_mutate_module_globals() {
    let src = "
        var counter = 0
        fn bump() {
            counter += 1
        }
        fn get() {
            return counter
        }
        bump()
        bump()
        bump()
        return counter == 3 && get() == 3
    ";
    assert!(run(src).as_bool().unwrap());
}

#[test]
fn module_globals_persist_across_calls() {
    let src = "
        var total = 10
        fn add(n) {
            total = total + n
            return total
        }
        add(5)
        var second = add(2)
        return total == 17 && second == 17
    ";
    assert!(run(src).as_bool().unwrap());
}

fn run_with_modules(entry: &str, modules: &[(&str, &str)]) -> Value {
    let mut vm = Vm::new(Box::new(RefCountGc));
    for (name, src) in modules {
        let m = compile_source(src).unwrap_or_else(|e| panic!("compile '{name}' failed: {e:?}"));
        vm.register_module(*name, m.into());
    }
    let entry_mod = compile_source(entry).unwrap_or_else(|e| panic!("compile entry failed: {e:?}"));
    vm.run_module(&entry_mod.main, &entry_mod.children)
        .unwrap_or_else(|e| panic!("runtime error: {e}"))
}

#[test]
fn imported_modules_capture_exports() {
    let a = "let a = 10\nfn get_a() { return a }";
    let b = "let b = 5\nfn get_b() { return b }";
    let entry = "import a\nimport b\nreturn a.a + b.b";
    assert_eq!(int(run_with_modules(entry, &[("a", a), ("b", b)])), 15);
}

#[test]
fn module_globals_are_isolated_per_module() {
    let a = "var state = 0\nfn set(n) { state = n }\nfn get() { return state }";
    let b = "var state = 100\nfn get() { return state }";
    let entry = "
        import a
        import b
        let sa = a.set
        sa(42)
        let ga = a.get
        let gb = b.get
        return ga() == 42 && gb() == 100
    ";
    assert!(run_with_modules(entry, &[("a", a), ("b", b)])
        .as_bool()
        .unwrap());
}

#[test]
fn importing_twice_shares_one_module_instance() {
    let shared = "var v = 0\nfn set(n) { v = n }\nfn get() { return v }";
    let entry = "
        import store as s1
        import store as s2
        let set = s1.set
        set(7)
        let get = s2.get
        return get()
    ";
    assert_eq!(int(run_with_modules(entry, &[("store", shared)])), 7);
}

#[test]
fn division_by_zero_is_error() {
    let module = compile_source("return 1 / 0").expect("compiles");
    let mut vm = Vm::new(Box::new(RefCountGc));
    let err = vm.run_module(&module.main, &module.children).unwrap_err();
    assert!(
        matches!(err, zen_vm::VmError::DivideByZero(_)),
        "got {err:?}"
    );
}

#[test]
fn unbounded_recursion_errors_without_panicking() {
    // Each call consumes register-window space; the VM must report a
    // StackOverflow instead of indexing past the register file.
    let module = compile_source("func f() { return f() }\nf()").expect("compiles");
    let mut vm = Vm::new(Box::new(RefCountGc));
    let err = vm.run_module(&module.main, &module.children).unwrap_err();
    assert!(
        matches!(err, zen_vm::VmError::StackOverflow(_)),
        "got {err:?}"
    );
}

#[test]
fn bare_catch_handles_a_runtime_error() {
    let src = "
        var caught = false
        try { var bad = 1 / 0 } catch { caught = true }
        return caught
    ";
    assert!(run(src).as_bool().unwrap());
}

#[test]
fn throw_is_caught_and_the_body_can_skip_the_catch() {
    let caught = run("var hit = 0\ntry { throw \"boom\" } catch { hit = 7 }\nreturn hit");
    assert_eq!(int(caught), 7);
    let skipped = run("var hit = 0\ntry { hit = 3 } catch { hit = 7 }\nreturn hit");
    assert_eq!(int(skipped), 3);
}

#[test]
fn typed_catch_matches_the_error_kind_and_binds_the_error() {
    let matched =
        run("var hit = 0\ntry { throw \"boom\" } catch UserError e { hit = 1 }\nreturn hit");
    assert_eq!(int(matched), 1);
    let unbound =
        run("var hit = 0\ntry { throw \"boom\" } catch UserError { hit = 1 }\nreturn hit");
    assert_eq!(int(unbound), 1);
    // A kind that does not match leaves the error unhandled.
    let module = compile_source(
        "var hit = 0\ntry { throw \"boom\" } catch TypeError { hit = 1 }\nreturn hit",
    )
    .expect("compiles");
    let mut vm = Vm::new(Box::new(RefCountGc));
    let err = vm.run_module(&module.main, &module.children).unwrap_err();
    assert!(
        matches!(&err, zen_vm::VmError::UserError(m) if m == "boom"),
        "got {err:?}"
    );
}

#[test]
fn an_unmatched_clause_falls_through_to_the_next_one() {
    let src = "
        var hit = \"\"
        try { throw \"z\" } catch TypeError { hit = hit + \"t\" } catch UserError { hit = hit + \"u\" }
        return hit
    ";
    assert_eq!(text(run(src)), "u");
}

#[test]
fn an_unhandled_error_keeps_propagating_outward() {
    let src = "
        func inner() {
            try { throw \"boom\" } catch TypeError { return \"wrong\" }
            return \"propagated\"
        }
        var hit = \"\"
        try { hit = inner() } catch { hit = \"outer\" }
        return hit
    ";
    assert_eq!(text(run(src)), "outer");
}

#[test]
fn finally_runs_on_the_failure_path_and_after_a_catch() {
    let on_error = run("var f = \"\"\ntry { try { throw \"x\" } finally { f = f + \"f\" } } catch { f = f + \"c\" }\nreturn f");
    assert_eq!(text(on_error), "fc");
    let on_success = run("var f = \"\"\ntry { f = f + \"b\" } finally { f = f + \"f\" }\nreturn f");
    assert_eq!(text(on_success), "bf");
    let after_catch = run("var f = \"\"\ntry { try { throw \"y\" } catch { f = f + \"c\" } finally { f = f + \"f\" } } catch { }\nreturn f");
    assert_eq!(text(after_catch), "cf");
}

#[test]
fn errors_from_a_callee_are_catchable_by_the_caller() {
    let src = "
        func deep() { return [1][5] }
        var hit = false
        try { deep() } catch IndexError { hit = true }
        return hit
    ";
    assert!(run(src).as_bool().unwrap());
}

#[test]
fn leaving_a_try_with_break_retires_its_handler() {
    let src = "
        var hit = 0
        try {
            var i = 0
            while i < 2 {
                i = i + 1
                try { break } catch { hit = hit + 100 }
            }
            throw \"leak\"
        } catch { hit = hit + 1 }
        return hit
    ";
    assert_eq!(int(run(src)), 1);
}

#[test]
fn leaving_a_try_with_continue_retires_its_handler() {
    let src = "
        var hit = 0
        var i = 0
        while i < 3 {
            i = i + 1
            try { continue } catch { hit = hit + 100 }
            hit = hit + 1
        }
        return hit
    ";
    assert_eq!(int(run(src)), 0);
}

#[test]
fn an_uncaught_error_reports_the_thrown_message() {
    let module = compile_source("throw \"boom\"").expect("compiles");
    let mut vm = Vm::new(Box::new(RefCountGc));
    let err = vm.run_module(&module.main, &module.children).unwrap_err();
    assert!(
        matches!(&err, zen_vm::VmError::UserError(m) if m == "boom"),
        "got {err:?}"
    );
}

#[test]
fn if_expression_selects_an_arm() {
    assert_eq!(
        text(run("return if 1 > 0 { \"yes\" } else { \"no\" }")),
        "yes"
    );
    assert_eq!(
        text(run("return if 1 < 0 { \"yes\" } else { \"no\" }")),
        "no"
    );
}

#[test]
fn if_expression_without_a_matching_arm_is_nil() {
    let v = run("return if 1 > 100 { \"yes\" }");
    assert!(v.is_nil(), "got {v:?}");
}

#[test]
fn if_expression_elif_chain_takes_the_first_match() {
    let src = "return if 1 > 100 { \"huge\" } elif 1 > 0 { \"big\" } else { \"small\" }";
    assert_eq!(text(run(src)), "big");
    let src = "return if 1 > 100 { \"huge\" } elif 1 > 50 { \"big\" } else { \"small\" }";
    assert_eq!(text(run(src)), "small");
}

#[test]
fn if_expression_arm_yields_its_last_expression() {
    // Several statements: the last expression is the value.
    assert_eq!(
        int(run("return if true { var a = 2; a * 3 } else { 0 }")),
        6
    );
    // A trailing non-expression makes the arm nil.
    let v = run("return if true { var b = 2 } else { 0 }");
    assert!(v.is_nil(), "got {v:?}");
    // An empty arm is nil.
    let v = run("return if true { } else { 7 }");
    assert!(v.is_nil(), "got {v:?}");
}

#[test]
fn if_expression_conditions_use_truthiness() {
    // Only nil and false are falsy in Zen: an empty string is truthy.
    assert_eq!(text(run("return if \"\" { \"t\" } else { \"f\" }")), "t");
    assert_eq!(text(run("return if [] { \"t\" } else { \"f\" }")), "t");
    assert_eq!(text(run("return if nil { \"t\" } else { \"f\" }")), "f");
    assert_eq!(text(run("return if false { \"t\" } else { \"f\" }")), "f");
    assert_eq!(text(run("return if 0 { \"t\" } else { \"f\" }")), "t");
}

#[test]
fn if_expression_arms_run_once_in_order() {
    let src = "
        var order = \"\"
        func mark(t) {
            order = order + t
            return true
        }
        var r = if mark(\"a\") { mark(\"b\") } elif mark(\"c\") { mark(\"d\") } else { mark(\"e\") }
        return order
    ";
    assert_eq!(text(run(src)), "ab");
}

#[test]
fn if_expression_nests_and_works_as_an_argument() {
    assert_eq!(
        text(run("return \"v=\" + str(if 1 > 0 { 7 } else { 8 })")),
        "v=7"
    );
    // An `if` in statement position inside an arm parses as a statement, so the
    // arm's value is nil — the oracle behaves the same way.
    let nested = run("return if true { if 1 > 2 { \"a\" } else { \"b\" } } else { \"c\" }");
    assert!(nested.is_nil(), "got {nested:?}");
    let src = "
        func classify(v) { return if v < 0 { \"neg\" } elif v == 0 { \"zero\" } else { \"pos\" } }
        return classify(-1) + \",\" + classify(0) + \",\" + classify(1)
    ";
    assert_eq!(text(run(src)), "neg,zero,pos");
}

#[test]
fn if_expression_drives_a_loop_condition() {
    let src = "
        var i = 0
        var hits = 0
        while if i < 3 { true } else { false } {
            hits = hits + (if i == 1 { 10 } else { 1 })
            i = i + 1
        }
        return hits
    ";
    assert_eq!(int(run(src)), 12);
}

#[test]
fn for_binds_each_variable_to_one_element() {
    let src = "
        var total = 0
        for a, b in [[1, 2], [3, 4]] { total = total + a * b }
        return total
    ";
    assert_eq!(int(run(src)), 14);
    let tuples = "
        var sums = \"\"
        for a, b, c in [(1, 2, 3), (4, 5, 6)] { sums = sums + str(a + b + c) }
        return sums
    ";
    // 1+2+3 == 6, then 4+5+6 == 15.
    assert_eq!(text(run(tuples)), "615");
    let chars = "var s = \"\"\nfor a, b in [\"ab\", \"cd\"] { s = s + a + b }\nreturn s";
    assert_eq!(text(run(chars)), "abcd");
}

#[test]
fn for_unpacking_ignores_extra_elements() {
    let src = "var s = \"\"\nfor a, b in [[1, 2, 3]] { s = str(a) + str(b) }\nreturn s";
    assert_eq!(text(run(src)), "12");
}

#[test]
fn for_unpacking_a_short_item_is_a_value_error() {
    let src = "
        var caught = \"\"
        for bad in [[1], [2, 3]] {
            try {
                for a, b in [bad] { caught = caught + \"b\" }
            } catch ValueError { caught = caught + \"v\" }
        }
        return caught
    ";
    // The short item binds nothing, then the loop carries on with the next one.
    assert_eq!(text(run(src)), "vb");
}

#[test]
fn for_unpacking_a_non_sequence_is_a_value_error() {
    let module = compile_source("for a, b in [5] { }").expect("compiles");
    let mut vm = Vm::new(Box::new(RefCountGc));
    let err = vm.run_module(&module.main, &module.children).unwrap_err();
    match err {
        zen_vm::VmError::ValueError(msg) => {
            assert_eq!(msg, "not enough values to unpack in for");
        }
        other => panic!("expected ValueError, got {other:?}"),
    }
}

#[test]
fn for_unpacking_an_empty_run_never_executes_the_body() {
    let src = "var seen = \"\"\nfor a, b in [] { seen = \"x\" }\nreturn seen == \"\"";
    assert!(run(src).as_bool().unwrap());
}

#[test]
fn for_over_several_variables_honours_break_and_continue() {
    let src = "
        var seen = \"\"
        for a, b in [[1, 2], [3, 4], [5, 6]] {
            if a == 3 { continue }
            if a == 5 { break }
            seen = seen + str(a) + str(b)
        }
        return seen
    ";
    assert_eq!(text(run(src)), "12");
}

#[test]
fn enumerate_and_items_feed_the_two_variable_form() {
    let src = "
        var s = \"\"
        for i, item in enumerate([\"x\", \"y\"]) { s = s + str(i) + item }
        for key, value in {a: 1, b: 2}.items() { s = s + key + str(value) }
        return s
    ";
    assert_eq!(text(run(src)), "0x1ya1b2");
}

#[test]
fn comprehension_basic_guard_and_nested_clauses() {
    assert_eq!(
        ints(run("return [x * x for x in 1 -> 5]")),
        vec![1, 4, 9, 16, 25]
    );
    assert_eq!(
        ints(run("return [x for x in 1 -> 10 if x % 2 == 0]")),
        vec![2, 4, 6, 8, 10]
    );
    assert_eq!(
        ints(run(
            "return [cell for row in [[1, 2], [3, 4]] for cell in row]"
        )),
        vec![1, 2, 3, 4]
    );
    // An inner clause reads the outer clause's variable.
    let nested = run("return [[y for y in 1 -> n] for n in 1 -> 3]");
    let outer = nested.as_list().expect("list").to_vec();
    assert_eq!(outer.len(), 3);
    assert_eq!(ints(outer[2]), vec![1, 2, 3]);
    // The outer guard runs before the inner iterable is walked.
    assert_eq!(
        ints(run(
            "return [a for a in 1 -> 4 if a % 2 == 0 for b in 1 -> 2]"
        )),
        vec![2, 2, 4, 4]
    );
}

#[test]
fn comprehension_variables_are_isolated() {
    let src = "
        var x = 99
        var out = [x for x in 1 -> 3]
        return x == 99 && out == [1, 2, 3]
    ";
    assert!(yes(run(src)));
    // An outer binding is still visible from the comprehension body.
    let reads = "
        var limit = 3
        return [n for n in 1 -> 6 if n < limit] == [1, 2]
    ";
    assert!(yes(run(reads)));
}

#[test]
fn comprehension_evaluates_iterables_once_per_clause() {
    let src = "
        var calls = 0
        func source() { calls = calls + 1
 return [1, 2] }
        var a = [v for v in source()]
        var b = [v for v in source()]
        return str(calls)
    ";
    assert_eq!(text(run(src)), "2");
}

#[test]
fn comprehension_iterates_any_iterable() {
    assert_eq!(
        strings(run("return [k for k in {a: 1, b: 2}]")),
        vec!["a", "b"]
    );
    assert_eq!(
        strings(run("return [c for c in \"hey\"]")),
        vec!["h", "e", "y"]
    );
    assert_eq!(int(run("return len([v for v in []])")), 0);
    assert_eq!(int(run("return len([v for v in 7])")), 0);
}

#[test]
fn range_is_inclusive_and_empty_when_descending() {
    assert_eq!(ints(run("return 2 -> 5")), vec![2, 3, 4, 5]);
    // The parser makes `..` and `->` the same operator, inclusive.
    assert_eq!(ints(run("return 1 .. 3")), vec![1, 2, 3]);
    assert_eq!(int(run("return len(5 -> 1)")), 0);
    assert_eq!(int(run("return len(4 -> 4)")), 1);
    assert_eq!(int(run("return len(0 -> 10)")), 11);
}

#[test]
fn range_ends_must_be_ints() {
    for src in ["return 1.5 -> 3", "return \"a\" -> 3"] {
        let module = compile_source(src).expect("compiles");
        let mut vm = Vm::new(Box::new(RefCountGc));
        let err = vm.run_module(&module.main, &module.children).unwrap_err();
        match err {
            zen_vm::VmError::TypeError(msg) => {
                assert!(msg.starts_with("expected int"), "unexpected message: {msg}");
            }
            other => panic!("expected TypeError, got {other:?}"),
        }
    }
}

#[test]
fn higher_order_methods_map_filter_reduce() {
    let src = "
        let double = (x) => x * 2
        var nums = [1, 2, 3, 4, 5]
        return nums.map(double) == [2, 4, 6, 8, 10] \
            && nums.filter((x) => x % 2 == 0) == [2, 4] \
            && nums.reduce((acc, x) => acc + x) == 15 \
            && nums.reduce((acc, x) => acc + x, 100) == 115
    ";
    assert!(yes(run(src)));
    // `each` returns nil but still runs the callback for every element.
    let each = "
        var total = 0
        var r = [1, 2, 3].each((x) => { total = total + x })
        return total == 6 && r == nil
    ";
    assert!(yes(run(each)));
}

#[test]
fn higher_order_methods_iterate_any_iterable() {
    assert_eq!(ints(run("return [1, 2].map((x) => x + 1)")), vec![2, 3]);
    assert_eq!(int(run("return [1, 2, 3, 4].reduce((a, b) => a * b)")), 24);
    assert_eq!(int(run("return [].reduce((a, b) => a + b, 7)")), 7);
    // Iteration follows `iter_get`, so a string maps over its characters.
    assert_eq!(
        strings(run("return \"ab\".map((c) => c.upper())")),
        vec!["A", "B"]
    );
    assert_eq!(
        strings(run("return {a: 1, b: 2}.map((k) => k.upper())")),
        vec!["A", "B"]
    );
}

#[test]
fn reduce_of_empty_sequence_without_an_initial_value() {
    let module = compile_source("return [].reduce((a, b) => a + b)").expect("compiles");
    let mut vm = Vm::new(Box::new(RefCountGc));
    let err = vm.run_module(&module.main, &module.children).unwrap_err();
    match err {
        zen_vm::VmError::ValueError(msg) => {
            assert_eq!(msg, "reduce of empty sequence with no initial value");
        }
        other => panic!("expected ValueError, got {other:?}"),
    }
}

#[test]
fn callback_frames_keep_their_own_registers() {
    // Each callback reuses the same window above the interrupted frame, so a
    // loop of calls must not let one call's arguments leak into the next.
    let src = "
        var out = []
        for x in [1, 2, 3] {
            var mapper = (y) => y + x
            var got = [10].map(mapper)
            out.push(got[0])
        }
        return out
    ";
    assert_eq!(ints(run(src)), vec![11, 12, 13]);
}

#[test]
fn callback_catch_handles_inside_and_outside() {
    // The callback's own `catch` handles it...
    let src = "
        var out = [1, 0, 2].map((x) => { if x == 0 { return 99 }
 x * 10 })
        return out == [10, 99, 20]
    ";
    assert!(yes(run(src)));
    // ...and an unhandled one escapes to the caller's handler.
    let esc = "
        var caught = 0
        try {
            [1, 2].map((x) => { if x == 2 { return 1 / 0 }
 x })
        } catch ZeroDivisionError { caught = caught + 1 }
        return caught == 1
    ";
    assert!(yes(run(esc)));
}

#[test]
fn instance_methods_win_over_higher_order_builtins() {
    let src = "
        class Doubler {
            func map(f) { return \"instance map\" }
        }
        return Doubler().map((x) => x)
    ";
    assert_eq!(text(run(src)), "instance map");
}

#[test]
fn all_block_bodies_auto_return_last_expression() {
    let src = "
        let arrow = (x) => { let doubled = x * 2
 doubled + 1 }
        func plain(x) { let doubled = x * 2
 doubled + 1 }
        let anon = fn(x) { x * 3 }
        class C { func m(x) { x * 4 } }
        return str(arrow(5)) + \" \" + str(plain(5)) + \" \" + str(anon(5)) + \" \" + str(C().m(5))
    ";
    assert_eq!(text(run(src)), "11 11 15 20");
    // A trailing statement that is not an expression yields nil.
    assert_eq!(
        text(run("let a = (x) => { var v = x
 var w = 2 }\nreturn str(a(1))")),
        "nil"
    );
    // `return` still wins over the trailing expression.
    assert_eq!(
        text(run("let a = (x) => { let d = x * 2
 return d + 1
 d + 99 }\nreturn str(a(5))")),
        "11"
    );
}

#[test]
fn arrow_block_trailing_expression_runs_once() {
    let src = "
        var bumps = 0
        func bump() { bumps = bumps + 1
 return bumps }
        let arrow = () => bump()
        arrow()
        arrow()
        return str(bumps)
    ";
    assert_eq!(text(run(src)), "2");
}

#[test]
fn match_expression_literal_arms_and_wildcard() {
    let src = r#"
        func status_of(s) {
            return match s {
                "loading": "spinner",
                "ready" => "check",
                _: "unknown"
            }
        }
        return status_of("loading") + "," + status_of("ready") + "," + status_of("nope")
    "#;
    assert_eq!(text(run(src)), "spinner,check,unknown");
}

#[test]
fn match_expression_yields_nil_when_nothing_matches() {
    // The oracle's `exec_match_arms` falls through to nil rather than raising.
    assert!(yes(run(
        "return match \"zzz\" { \"aaa\": 1, \"bbb\": 2 } == nil"
    )));
    assert!(yes(run("return match 9 { 1: 1, 2: 2 } == nil")));
    assert_eq!(int(run("return len(match 1 { 2: 1 })")), 0);
    assert!(yes(run("return when { false: \"no\" } == nil")));
}

#[test]
fn match_expression_binds_and_guards_in_arm_scope() {
    let src = r#"
        func pick(n) {
            return match n {
                k if k > 100: "big",
                k if k > 10: "medium",
                k: "small"
            }
        }
        return pick(500) + "," + pick(50) + "," + pick(1)
    "#;
    assert_eq!(text(run(src)), "big,medium,small");
}

#[test]
fn match_arm_bindings_are_isolated() {
    // A failed arm's binding is invisible to later arms, and an arm's binding
    // never escapes the match.
    let src = r#"
        var leaked = "outer"
        var seen = match 7 {
            leaked: { leaked = "inner" }
            _: { seen = "reached" }
        }
        return seen + "," + leaked
    "#;
    assert_eq!(text(run(src)), "nil,outer");
}

#[test]
fn match_block_body_runs_and_yields_nil() {
    let src = r#"
        var ran = ""
        var value = match 3 {
            1: { ran = "one" }
            3: { ran = "three" }
        }
        return ran + "," + str(value)
    "#;
    assert_eq!(text(run(src)), "three,nil");
}

#[test]
fn match_nests_as_an_expression() {
    let src = r#"
        var nest = match 2 {
            1: match 9 { _: "inner-one" },
            2: match 3 {
                3: "inner-two"
                _: "inner-other"
            }
        }
        return nest
    "#;
    assert_eq!(text(run(src)), "inner-two");
}

#[test]
fn match_composes_inside_larger_expressions() {
    // Regression guard: the match result and its subject must not take a
    // register an enclosing expression is still reading.
    let src = r#"
        func f(x) { return x * 100 }
        var got = f(1) + str(match 2 { 2: "two" })
        var args = str(match 1 { 1: "one" }) + str(match 2 { 2: "two" })
        return got + "|" + args
    "#;
    assert_eq!(text(run(src)), "100two|onetwo");
}

#[test]
fn match_in_a_loop_reuses_its_registers() {
    // The reserved-slot pool is released per match, so a loop of matches costs
    // a constant number of registers no matter how many iterations run.
    let src = r#"
        var out = ""
        for i in 1 -> 5 {
            out = out + match i {
                1: "a"
                2: "b"
                _: "z"
            }
        }
        return out
    "#;
    assert_eq!(text(run(src)), "abzzz");
}

#[test]
fn match_with_many_arms_stays_within_the_register_file() {
    // A module root is one frame capped at 250 registers; a long arm list used to
    // spend two per `match` and could exhaust it.
    let mut arms = String::new();
    for i in 0..80 {
        arms.push_str(&format!("{i}: \"hit-{i}\",\n"));
    }
    let src = format!("return match 79 {{ {arms} _: \"miss\" }}");
    assert_eq!(text(run(&src)), "hit-79");
}

#[test]
fn when_expression_tests_conditions_in_order() {
    let src = r#"
        func grade_of(score) {
            return when {
                score >= 90: "A",
                score >= 85 => "A-",
                score >= 80: "B+",
                _: "C"
            }
        }
        return grade_of(87) + "," + grade_of(95) + "," + grade_of(10)
    "#;
    assert_eq!(text(run(src)), "A-,A,C");
}

#[test]
fn match_statement_form_returns_from_the_function() {
    let src = r#"
        func kind(x) {
            match x {
                1: { return "one" }
                _: { return "other" }
            }
        }
        return kind(1) + "," + kind(2)
    "#;
    assert_eq!(text(run(src)), "one,other");
}

#[test]
fn nested_destructuring_does_not_clobber_the_element_register() {
    // A sub-pattern's subject is a temporary, and `declare_hidden` hands out
    // `num_locals` — the lowest live temporary. Declaring a binding while that
    // element register was still in flight made the enclosing pattern index the
    // binding instead of the sequence, so `[b, c]` indexed `b` (an int) for its
    // second element and raised "expected iterable, got int".
    let src = r#"
        func shape(x) {
            return match x {
                #([a], [b, c]): "two:" + str(a) + str(b) + str(c),
                #([a, b], [c]): "one:" + str(a) + str(b) + str(c),
                _: "no"
            }
        }
        return shape(#([1], [2, 3])) + "," +
               shape(#([1, 2], [3])) + "," +
               shape(#([1, 2], [3, 4])) + "," +
               shape(9)
    "#;
    assert_eq!(text(run(src)), "two:123,one:123,no,no");
}

#[test]
fn nested_destructuring_of_lists_binds_every_position() {
    let src = r#"
        func deep(x) {
            return match x {
                [[[a], [b]], [[c, d]]]: str(a) + str(b) + str(c) + str(d),
                _: "no"
            }
        }
        return deep([[[1], [2]], [[3, 4]]]) + "," + deep([[[1]], [[2]]])
    "#;
    assert_eq!(text(run(src)), "1234,no");
}

#[test]
fn inline_cache_monomorphic_field_access() {
    let src = "
        class Point {
            func init(x, y) {
                self.x = x
                self.y = y
            }
            func sum() {
                return self.x + self.y
            }
        }
        var p = new Point(3, 4)
        var total = 0
        for i in range(1000) {
            total = total + p.sum()
        }
        return total
    ";
    assert_eq!(int(run(src)), 7000);
}

#[test]
fn inline_cache_shape_transition_on_new_property() {
    let src = "
        class Bag {
            func init() {
                self.a = 1
            }
        }
        var b = new Bag()
        var first = b.a
        b.c = 41
        var second = b.a
        return first * 1000 + second * 10 + b.c
    ";
    assert_eq!(int(run(src)), 1051);
}

#[test]
fn inline_cache_polymorphic_field_orders() {
    let src = "
        class Node {
            func init() {
            }
        }
        var a = new Node()
        a.x = 1
        a.y = 2
        var b = new Node()
        b.y = 20
        b.x = 10
        return a.x * 100000 + a.y * 1000 + b.x * 10 + b.y
    ";
    assert_eq!(int(run(src)), 102120);
}

#[test]
fn inline_cache_method_monomorphic() {
    let src = "
        class Counter {
            func init() {
                self.n = 0
            }
            func inc() {
                self.n = self.n + 1
                return self.n
            }
        }
        var c = new Counter()
        var total = 0
        for i in range(1000) {
            total = total + c.inc()
        }
        return total
    ";
    assert_eq!(int(run(src)), 500500);
}

#[test]
fn inline_cache_method_polymorphic_returns_correct_method() {
    let src = "
        class A {
            func value() { return 10 }
        }
        class B {
            func value() { return 20 }
        }
        func call(x) {
            return x.value()
        }
        return call(A()) * 100 + call(B())
    ";
    assert_eq!(int(run(src)), 1020);
}

#[test]
fn inline_cache_method_inherited() {
    let src = "
        class Base {
            func init() {
                self.n = 5
            }
            func get() {
                return self.n
            }
        }
        class Derived extends Base {
            func init() {
                self.n = 7
            }
        }
        func call(x) {
            return x.get()
        }
        var d = new Derived()
        var total = 0
        for i in range(1000) {
            total = total + call(d)
        }
        return total
    ";
    assert_eq!(int(run(src)), 7000);
}

#[test]
fn inline_cache_method_wide_class() {
    let src = "
        class Wide {
            func m0() { return 0 }
            func m1() { return 1 }
            func m2() { return 2 }
            func m3() { return 3 }
            func m4() { return 4 }
            func m5() { return 5 }
            func target() { return 42 }
        }
        var w = new Wide()
        var total = 0
        for i in range(1000) {
            total = total + w.target()
        }
        return total
    ";
    assert_eq!(int(run(src)), 42000);
}

#[test]
fn step_budget_stops_infinite_loop() {
    let module = compile_source("var x = 0\nwhile true { x = x + 1 }").unwrap();
    let mut vm = Vm::new(Box::new(RefCountGc));
    vm.set_step_budget(Some(10_000));
    let err = vm
        .run_module(&module.main, &module.children)
        .expect_err("infinite loop should exhaust the budget");
    assert!(
        format!("{err:?}").contains("step budget"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn step_budget_does_not_trip_on_short_program() {
    let module = compile_source("var x = 1\nx = x + 1").unwrap();
    let mut vm = Vm::new(Box::new(RefCountGc));
    vm.set_step_budget(Some(1_000_000));
    vm.run_module(&module.main, &module.children)
        .expect("short program should finish within budget");
}

#[test]
fn oversized_string_repetition_errors_without_aborting() {
    let module =
        compile_source("func big() { return 1000000000 }\nvar s = \"x\"\nreturn s * big()")
            .unwrap();
    let mut vm = Vm::new(Box::new(RefCountGc));
    let err = vm.run_module(&module.main, &module.children).unwrap_err();
    assert!(matches!(err, VmError::ValueError(_)), "unexpected: {err:?}");
}

#[test]
fn oversized_zfill_errors_without_aborting() {
    let module =
        compile_source("func big() { return 1000000000 }\nvar s = \"x\"\nreturn s.zfill(big())")
            .unwrap();
    let mut vm = Vm::new(Box::new(RefCountGc));
    let err = vm.run_module(&module.main, &module.children).unwrap_err();
    assert!(matches!(err, VmError::ValueError(_)), "unexpected: {err:?}");
}

#[test]
fn huge_int_exponent_promotes_to_float() {
    let module = compile_source("func big() { return 5000000000 }\nreturn 2 ** big()").unwrap();
    let mut vm = Vm::new(Box::new(RefCountGc));
    let v = vm.run_module(&module.main, &module.children).unwrap();
    assert!(v.as_float().map(|f| f.is_infinite()).unwrap_or(false));
}
