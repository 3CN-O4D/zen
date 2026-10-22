//! `zen explain` knowledge base: one entry per error code in the registry.
//!
//! Codes are banded by phase: `Z01` lexing, `Z02` parsing, `Z03`
//! compile-time (including the `check` lints), `Z04` runtime. The same table
//! backs `zen explain <code>` and the `--list` overview; a test below pins
//! that every code the toolchain can emit has exactly one entry here.

/// One error-code explanation: title plus body (cause, example, fix).
pub struct Explanation {
    pub code: &'static str,
    pub title: &'static str,
    pub body: &'static str,
}

pub static EXPLANATIONS: &[Explanation] = &[
    // ── Z01: lexer ────────────────────────────────────────────────────
    Explanation {
        code: "Z0101",
        title: "unterminated string literal",
        body: "The lexer reached the end of the line (or file) while still inside a `\"...\"` string.\n\nBad:\n    var s = \"hello\n\nGood:\n    var s = \"hello\"\n\nFor multi-line text use a triple-quoted string instead.",
    },
    Explanation {
        code: "Z0102",
        title: "unterminated triple-quoted string",
        body: "A `\"\"\"` string was opened but never closed. Triple-quoted strings end at the next `\"\"\"` on its own terms — check for a missing closing fence.\n\nBad:\n    var s = \"\"\"hello\n\nGood:\n    var s = \"\"\"hello\"\"\"",
    },
    Explanation {
        code: "Z0103",
        title: "unterminated block comment",
        body: "A `/*` comment was opened but never closed with `*/`. Everything after it is swallowed, so the error often points far from the cause — search backwards for the stray opener.\n\nBad:\n    /* note\n    var x = 1\n\nGood:\n    /* note */\n    var x = 1",
    },
    Explanation {
        code: "Z0104",
        title: "unterminated interpolation",
        body: "A `${...}` splice inside a string never found its closing `}`. The message names where the expression started.\n\nBad:\n    print \"x = ${x\"\n\nGood:\n    print \"x = ${x}\"",
    },
    Explanation {
        code: "Z0105",
        title: "raw newline inside string literal",
        body: "Single-quoted (`\"...\"`) strings cannot span lines. Either escape the newline or switch to a triple-quoted string.\n\nBad:\n    var s = \"line one\n    line two\"\n\nGood:\n    var s = \"line one\\nline two\"\n    var t = \"\"\"line one\n    line two\"\"\"",
    },
    Explanation {
        code: "Z0106",
        title: "unfinished escape sequence",
        body: "A string ends with a lone backslash, so there is nothing for it to escape. Either complete the escape (`\\n`, `\\\\`, `\\\"`) or drop the backslash.\n\nBad:\n    var s = \"abc\\\"\n\nGood:\n    var s = \"abc\\\\\"",
    },
    Explanation {
        code: "Z0107",
        title: "unknown escape sequence",
        body: "The escape after `\\` is not one Zen knows. Valid escapes are `\\n \\t \\\\ \\\" \\u{...}` (plus `\\x..` hex).\n\nBad:\n    var s = \"a\\qb\"\n\nGood:\n    var s = \"a\\\\qb\"   # literal backslash + q",
    },
    Explanation {
        code: "Z0108",
        title: "invalid hex escape",
        body: "A `\\x` escape needs exactly two hexadecimal digits.\n\nBad:\n    var s = \"\\x4\"\n    var s = \"\\xzz\"\n\nGood:\n    var s = \"\\x41\"   # \"A\"",
    },
    Explanation {
        code: "Z0109",
        title: "invalid unicode escape",
        body: "Use `\\u{...}` with 1–6 hex digits inside the braces.\n\nBad:\n    var s = \"\\u1F600\"\n    var s = \"\\u{}\"\n\nGood:\n    var s = \"\\u{1F600}\"",
    },
    Explanation {
        code: "Z0199",
        title: "invalid token",
        body: "The lexer found a character (or sequence) that starts no valid token — often a stray symbol or an unsupported operator. Check for typos and for operators Zen does not have (there is no `//` division: `//` starts a comment).\n\nBad:\n    var x = 7 // 2   # this divides nothing; `// 2` is a comment\n\nGood:\n    var x = 7 / 2",
    },
    // ── Z02: parser ───────────────────────────────────────────────────
    Explanation {
        code: "Z0201",
        title: "expected X, found Y",
        body: "The parser wanted one construct and found another — an unclosed bracket two lines up, a missing comma, or a keyword typo. Read it as \"I needed <expected> to finish what you started; instead I got <found>\".\n\nBad:\n    if x > 0 {\n        print x\n    # missing }\n\nGood: close every opener on the way down.",
    },
    Explanation {
        code: "Z0202",
        title: "unexpected end of input",
        body: "The file ends in the middle of a construct: an unclosed `{`, `(`, `[`, or string. The fix is almost always *above* the reported position — the parser only notices at EOF that something never closed.\n\nChecklist: every `{` has `}`, every `(` has `)`, every `[` has `]`.",
    },
    Explanation {
        code: "Z0203",
        title: "invalid expression",
        body: "Something sits where a value is required: a dangling operator, an empty pair of parentheses, a keyword used as a value.\n\nBad:\n    var x = 1 +\n    var y = return\n\nGood:\n    var x = 1 + 2",
    },
    Explanation {
        code: "Z0204",
        title: "custom parse error",
        body: "A context-specific syntax rule rejected this construct; the message names the rule. Read it literally — custom parse errors are raised for one reason only, unlike the generic Z0201/Z0203.",
    },
    // ── Z03: compile time ─────────────────────────────────────────────
    Explanation {
        code: "Z0301",
        title: "undefined variable",
        body: "A name is read before anything binds it. Unlike the runtime NameError (Z0404), this fires at compile time when the compiler can prove the name is unknown.\n\nBad:\n    print total   # never declared\n\nGood:\n    var total = 0\n    print total\n\nAlso check spelling, and import the module that defines it.",
    },
    Explanation {
        code: "Z0303",
        title: "cannot assign to constant",
        body: "`const` bindings are frozen at declaration. If the value must change, declare it with `var` instead.\n\nBad:\n    const limit = 10\n    limit = 20\n\nGood:\n    var limit = 10\n    limit = 20",
    },
    Explanation {
        code: "Z0304",
        title: "break outside of loop",
        body: "`break` only makes sense inside `for`/`while`. To leave a function early, use `return`.\n\nBad:\n    func f() {\n        break\n    }\n\nGood:\n    func f() {\n        return\n    }",
    },
    Explanation {
        code: "Z0305",
        title: "continue outside of loop",
        body: "`continue` only makes sense inside `for`/`while` — it means \"skip to the next iteration\", and there is no iteration here.",
    },
    Explanation {
        code: "Z0307",
        title: "too many local variables",
        body: "One function declares more locals than a frame holds. Split it into smaller helpers — deeply nested temporaries usually hide a missing abstraction anyway.",
    },
    Explanation {
        code: "Z0308",
        title: "expression too complex for one frame",
        body: "A single expression needs more registers than exist. Break it into named temporaries; the names document the intermediate values for free.\n\nBad:\n    var v = f(a) + g(b) * h(c) - i(d) / j(e)\n\nGood:\n    var t1 = f(a) + g(b)\n    var t2 = h(c) - i(d) / j(e)\n    var v = t1 * t2",
    },
    Explanation {
        code: "Z0309",
        title: "unsupported construct",
        body: "Valid syntax the compiler does not lower yet (e.g. `include`). Track the roadmap for landing order; restructure around it meanwhile.",
    },
    Explanation {
        code: "Z0310",
        title: "too many constants",
        body: "One module holds more distinct constants than the pool indexes. Move large literal tables into a data file and load them at runtime.",
    },
    Explanation {
        code: "Z0311",
        title: "closure captures too many variables",
        body: "A closure captures more upvalues than the ABI carries. Capture a single dict holding the values instead of a dozen loose names.",
    },
    Explanation {
        code: "Z0312",
        title: "function body too large",
        body: "Jump distances overflow 16 bits — the function is simply enormous. Split it into smaller helpers.",
    },
    Explanation {
        code: "Z0313",
        title: "invalid match arm pattern",
        body: "Patterns are literals, names, `_`, or destructuring shapes. Anything else (calls, arithmetic) belongs in a guard or the arm body.\n\nBad:\n    match x {\n        f(1) => ...\n    }\n\nGood:\n    match x {\n        1 => ...\n        _ => ...\n    }",
    },
    Explanation {
        code: "Z0314",
        title: "or-pattern arms bind different names",
        body: "Every alternative of an or-pattern must bind the same names, or the arm body could not name them reliably.\n\nBad:\n    (a, _) | (_, b) => a + b\n\nGood:\n    (a, _) | (_, a) => a + a",
    },
    Explanation {
        code: "Z0315",
        title: "rest pattern must come last",
        body: "A `..rest` capture swallows everything after it, so anything following could never match. Move it to the end.\n\nBad:\n    [..rest, last] => ...\n\nGood:\n    [first, ..rest] => ...",
    },
    Explanation {
        code: "Z0316",
        title: "only one rest pattern is allowed",
        body: "Two `..rest` captures would split the leftovers ambiguously. Merge them into one and slice it in the body.",
    },
    Explanation {
        code: "Z0317",
        title: "duplicate dict key",
        body: "A dict literal names one key twice; the later entry would silently shadow the earlier. Keep one, or build the dict programmatically if the duplication is intentional.\n\nBad:\n    {\"a\": 1, \"a\": 2}\n\nGood:\n    {\"a\": 2}",
    },
    Explanation {
        code: "Z0318",
        title: "invalid comprehension clause",
        body: "Comprehensions are `for x in xs` with an optional `if` guard — nothing else fits in the brackets.\n\nGood:\n    [x * 2 for x in xs if x > 0]",
    },
    Explanation {
        code: "Z0319",
        title: "super call outside of class",
        body: "Bare `super(...)` only works inside a class method, where there is a parent to delegate to.",
    },
    Explanation {
        code: "Z0320",
        title: "super method call outside of class",
        body: "`super.method(...)` only works inside a class method. Outside a class there is no parent implementation to invoke.",
    },
    Explanation {
        code: "Z0321",
        title: "destructuring is not supported here",
        body: "This position does not accept a destructuring pattern. Bind the whole value, then index into it.\n\nBad:\n    for [a, b] in pairs { ... }   # if unsupported here\n\nGood:\n    for p in pairs {\n        var a = p[0]\n        var b = p[1]\n    }",
    },
    Explanation {
        code: "Z0322",
        title: "custom compile error",
        body: "A context-specific compile rule rejected this construct; the message names the rule. Read it literally — custom errors are raised for one reason only.",
    },
    Explanation {
        code: "Z0323",
        title: "attempt to divide by zero (static)",
        body: "The `check` pass found a division (or modulo) by a literal `0` — provably wrong before the program ever runs. This is distinct from Z0406, the *runtime* divide-by-zero for values only known at runtime.\n\nBad:\n    var x = 10 / 0\n\nGood:\n    var x = 10 / d   # check d != 0 first if it can be zero",
    },
    Explanation {
        code: "Z0324",
        title: "module not found",
        body: "The linker could not resolve an `import`ed module to a source file. Check the spelling, make sure the file is on the import path (entry directory, then `std/`, then project root), and that the filename matches the module name exactly.\n\nBad:\n    import nonexistent\n\nGood:\n    import fs\n    import ./local_module",
    },
    Explanation {
        code: "Z0325",
        title: "cannot read source file",
        body: "The linker found the module path but could not read the file — usually a permissions issue or a stale path. Check the file exists and is readable.\n\nBad:\n    import /root/secret.z   # permission denied\n\nGood:\n    import ./config   # readable relative file",
    },
    // ── Z04: runtime ──────────────────────────────────────────────────
    Explanation {
        code: "Z0401",
        title: "type error",
        body: "An operation met a value of the wrong type. Check operand types with `typeof(x)` and convert explicitly instead of assuming.\n\nBad:\n    var n = \"42\" + 1   # string + int\n\nGood:\n    var n = \"42\".to_int() + 1",
    },
    Explanation {
        code: "Z0402",
        title: "index out of bounds",
        body: "The index is outside `0..len`. The message names both the index and the length; valid indices end at `len - 1`.\n\nBad:\n    xs[xs.len()]   # one past the end\n\nGood:\n    if i < xs.len() {\n        xs[i]\n    }",
    },
    Explanation {
        code: "Z0403",
        title: "missing dict key",
        body: "The key is not in the dict. Test membership first, or supply a default so absence is a value instead of an error.\n\nBad:\n    d[\"port\"]\n\nGood:\n    if \"port\" in d {\n        d[\"port\"]\n    }\n    d.get(\"port\", 8080)",
    },
    Explanation {
        code: "Z0404",
        title: "unknown name at runtime",
        body: "A name the compiler could not resolve statically (dynamic scopes, conditional definitions, `eval`-adjacent patterns) turned out to be unbound at runtime. Declare it before use, check the spelling, and make sure its module is imported.\n\nDiffers from Z0301, which fires when the compiler can *prove* the name is unknown without running.",
    },
    Explanation {
        code: "Z0405",
        title: "invalid value",
        body: "The type is right but the value is not acceptable here: out-of-range numbers, malformed formats, empty inputs. Check ranges, formats, and emptiness before the call.",
    },
    Explanation {
        code: "Z0406",
        title: "division by zero at runtime",
        body: "The denominator evaluated to zero. Unlike Z0323 (a literal `0` the checker sees statically), this value only became zero at runtime — so the guard belongs in the code.\n\nBad:\n    var q = total / n\n\nGood:\n    var q = if n != 0 { total / n } else { 0 }",
    },
    Explanation {
        code: "Z0407",
        title: "stack overflow (runaway recursion)",
        body: "A function never stopped calling itself (or two functions ping-pong forever). Check the recursion base case: every recursive path must shrink toward it.\n\nBad:\n    func f(n) { return f(n) }\n\nGood:\n    func f(n) {\n        if n <= 0 { return 0 }\n        return f(n - 1)\n    }",
    },
    Explanation {
        code: "Z0408",
        title: "wrong argument count",
        body: "The call supplies a different number of arguments than the function declares. The message names both counts — match the call against the parameter list, watching for missing or extra trailing arguments.",
    },
    Explanation {
        code: "Z0409",
        title: "bad handle",
        body: "A native resource handle (file, socket, channel) is closed, exhausted, or came from a failed open. Check the resource lifecycle: open success first, close exactly once.",
    },
    Explanation {
        code: "Z0410",
        title: "I/O shape mismatch",
        body: "A read or write produced/consumed a different shape than requested: short reads, wrong modes, size disagreements. Check sizes and open modes on both ends.",
    },
    Explanation {
        code: "Z0411",
        title: "native module error",
        body: "A native (Rust-implemented) module raised this while running: the Zen side is fine, one of its arguments is not. Check the arguments against `:help(module)` — e.g. a `pymport`ed call with a wrong-typed argument surfaces here with Python's own message attached.",
    },
    Explanation {
        code: "Z0412",
        title: "user-thrown error",
        body: "Your own `throw` value, uncaught. Either the `try`/`catch` around it is missing (or misspelled), or the throw is genuinely fatal — both are correct outcomes, the code just tells you which.",
    },
    Explanation {
        code: "Z0413",
        title: "native routine panicked",
        body: "A Rust routine hit an internal invariant — this is always a bug in Zen, never in your program. Report it with a minimal reproducer: the smallest program that still panics.",
    },
    Explanation {
        code: "Z0414",
        title: "foreign import failed (pymport)",
        body: "The Python bridge could not import the module. Three usual causes, in order: (1) no bridge is linked at all — rebuild with `--features python` (needs libpython); (2) the package is not installed — add it to requirements.txt and run `zen py-sync`; (3) a project-local `.py` file is invisible — it must sit in the entry file's directory or the project root, which `run` adds to sys.path automatically.\n\nBad:\n    pymport whatsapp   # without requirements.txt or a venv\n\nGood:\n    # requirements.txt contains the pip name, then:\n    pymport whatsapp",
    },
    Explanation {
        code: "Z0415",
        title: "foreign attribute missing",
        body: "The Python object has no such member. Attribute names are exact and version-sensitive — print `dir(obj)` equivalents or check the library's changelog for renames. Note that only callables come back as handles; data attributes arrive as plain values, which cannot then be called (that surfaces as Z0416 instead).\n\nBad:\n    math.nonexistent(1)\n\nGood:\n    math.sqrt(16.0)",
    },
    Explanation {
        code: "Z0416",
        title: "foreign call failed",
        body: "The call reached Python and Python raised: the message carries Python's own exception text, and the chained traceback (when one exists) follows as a note. Match argument counts and types against the Python signature. (Keyword arguments do not exist in Zen calls at all — everything crosses positionally.)\n\nBad:\n    math.sqrt()   # missing the operand\n\nGood:\n    math.sqrt(16.0)",
    },
    Explanation {
        code: "Z0417",
        title: "cannot cross the Python boundary",
        body: "Only nil, bool, int, float, string, bytes, and flat lists/dicts of those cross by value (tuples arrive as lists); dict keys must be strings. Everything else — instances, closures — either travels as an opaque handle or is rejected outright, never silently coerced.\n\nBad:\n    json.dumps({1: \"a\"})   # int key\n    json.dumps(my_func)    # closures cannot cross\n\nGood:\n    json.dumps({\"1\": \"a\"})",
    },
    // ── Z05: command line ───────────────────────────────────────────────
    Explanation {
        code: "Z0501",
        title: "usage error",
        body: "The command was invoked with the wrong arguments. See `zen help` for the full command list, or `zen explain --list` for every error code.\n\nBad:\n    zen run          # missing file argument\n\nGood:\n    zen run script.z\n    zen explain Z0308",
    },
    Explanation {
        code: "Z0502",
        title: "unknown command",
        body: "The subcommand is not recognized. Run `zen help` to see every available command.\n\nBad:\n    zen frobnicate\n\nGood:\n    zen run script.z\n    zen explain Z0308\n    zen repl",
    },
];

/// Canonicalize user input to a registry code: `Z0308`, `z0308`, `0308`.
/// Returns `None` for anything else (including Rust-style `E0308` — see
/// [`rust_redirect`] for that nudge).
pub fn normalize(input: &str) -> Option<String> {
    let t = input.trim().to_ascii_uppercase();
    let digits = t.strip_prefix('Z').unwrap_or(&t);
    if digits.len() == 4 && digits.bytes().all(|b| b.is_ascii_digit()) {
        return Some(format!("Z{digits}"));
    }
    None
}

/// When the user types a Rust-style code, point at the Zen twin instead of
/// failing blankly. Returns the Zen code when `input` looks like `E####`.
pub fn rust_redirect(input: &str) -> Option<String> {
    let t = input.trim().to_ascii_uppercase();
    let digits = t.strip_prefix('E')?;
    if digits.len() == 4 && digits.bytes().all(|b| b.is_ascii_digit()) {
        return Some(format!("Z{digits}"));
    }
    None
}

/// Look up a registry entry by canonical code.
pub fn lookup(code: &str) -> Option<&'static Explanation> {
    EXPLANATIONS.iter().find(|e| e.code == code)
}

/// Codes sharing the three-character band (`Z03`), for "unknown code, did
/// you mean one of these" suggestions. Empty when the band itself is unknown.
pub fn band_members(band: &str) -> Vec<&'static Explanation> {
    EXPLANATIONS
        .iter()
        .filter(|e| e.code.starts_with(band))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_has_no_duplicates_and_covers_bands() {
        let mut seen = HashSet::new();
        for e in EXPLANATIONS {
            assert!(seen.insert(e.code), "duplicate code {}", e.code);
            assert!(!e.title.is_empty() && !e.body.is_empty(), "empty entry {}", e.code);
        }
        // Lexer, parser, compiler(+check), runtime bands all represented.
        for band in ["Z01", "Z02", "Z03", "Z04"] {
            assert!(
                !band_members(band).is_empty(),
                "band {band} has no entries"
            );
        }
    }

    #[test]
    fn normalize_accepts_common_spellings() {
        assert_eq!(normalize("Z0308"), Some("Z0308".to_string()));
        assert_eq!(normalize("z0308"), Some("Z0308".to_string()));
        assert_eq!(normalize("0308"), Some("Z0308".to_string()));
        assert_eq!(normalize("  Z0401 "), Some("Z0401".to_string()));
        assert_eq!(normalize("E0308"), None);
        assert_eq!(normalize("Z308"), None);
        assert_eq!(normalize("hello"), None);
    }

    #[test]
    fn rust_spellings_redirect() {
        assert_eq!(rust_redirect("E0308"), Some("Z0308".to_string()));
        assert_eq!(rust_redirect("e0401"), Some("Z0401".to_string()));
        assert_eq!(rust_redirect("Z0308"), None);
    }
}
