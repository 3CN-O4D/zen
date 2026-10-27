# Chapter 79: Reference Implementation — Minimal Test Runner

A small test framework: register test functions with names, run them all, and
report pass/fail counts with a summary. Tests are Zen functions that throw on
failure. Demonstrates functions as values, `try`/`catch`, and CLI reporting.

## 79.1 The program (`testrunner.z`)

```zen
import sys

var tests = #{}
var results = #{}
let passed = 0
let failed = 0

func test(name, fn) {
    tests[name] = fn
}

func assert(condition, message) {
    if !condition {
        throw message ?? "assertion failed"
    }
}

func assert_eq(actual, expected, message) {
    if actual != expected {
        throw (message ?? "expected " + str(expected) + ", got " + str(actual))
    }
}

func assert_true(x, message) { assert(x == true, message) }
func assert_false(x, message) { assert(x == false, message) }

func run_test(name, fn) {
    try {
        fn()
        passed += 1
        print("PASS " + name)
    } catch as e {
        failed += 1
        print("FAIL " + name + " — " + e.msg)
    }
}

func run_all() {
    let names = []
    for n in tests { names.push(n) }
    names.sort()
    print("running " + str(len(names)) + " test(s)")
    print("========================")
    for n in names {
        run_test(n, tests[n])
    }
    print("========================")
    print("passed: " + str(passed))
    print("failed: " + str(failed))
    return if failed == 0 { 0 } else { 1 }
}

# ── Example tests (replace with your own) ───────────────────────────────────
test("addition", fn() {
    assert_eq(2 + 2, 4)
    assert_eq(10 - 3, 7)
})

test("string ops", fn() {
    assert_eq("hello".upper(), "HELLO")
    assert_eq(len("zen"), 3)
    assert("world".contains("or"))
})

test("list ops", fn() {
    let xs = [1, 2, 3]
    assert_eq(len(xs), 3)
    assert(3 in xs)
    assert(!(9 in xs))
})

test("dict ops", fn() {
    let d = #{a: 1, b: 2}
    assert_eq(d["a"], 1)
    assert("c" not in d)
})

test("always fails (demo)", fn() {
    assert_eq(1, 2, "this one intentionally fails")
})

# ── Main ───────────────────────────────────────────────────────────────────
func main(argv) {
    let code = run_all()
    return code
}

if __name__ == "__main__" {
    sys.exit(main(sys.argv))
}
```

## 79.2 Example session

```
$ zen run testrunner.z
running 5 test(s)
========================
PASS addition
PASS string ops
PASS list ops
PASS dict ops
FAIL always fails (demo) — expected 2, got 1
========================
passed: 4
failed: 1
```

Exit code is 1 when any test fails (suitable for CI).

## 79.3 Design notes

- **Tests are functions:** `test(name, fn)` stores a function value; `run_all`
  iterates and invokes each.
- **`assert`/`assert_eq`:** throw on failure; `run_test` catches and records.
- **Sorted output:** test names are sorted for deterministic ordering.
- **Extendable:** add `assert_lt`, `assert_gt`, `assert_approx`, `assert_raises`
  as needed.

## 79.4 Exercises

1. Add `assert_raises(fn, error_code)` — passes if `fn` throws the given code.
2. Add `assert_approx(actual, expected, epsilon)` for float comparison.
3. Add `--verbose` to print each test's runtime.
4. Add test grouping / suites (run only a subset by name prefix).
5. Write an HTML or JUnit-style XML report instead of plain text.
