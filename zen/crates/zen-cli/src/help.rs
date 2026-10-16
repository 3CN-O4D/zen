//! Python-style `:help(topic)` for the REPL: modules, types, keywords, and
//! session values.
//!
//! Module member lists come from the live [`ModuleUnit`]s (like Python
//! introspecting live objects), so they can never rot; the one-line module
//! descriptions, keyword docs, and type/method references are curated here.

use std::collections::HashMap;

use zen_vm::native::native_modules;
use zen_vm::Value;

/// One-line descriptions for every native module, keyed by import name.
fn module_descriptions() -> HashMap<&'static str, &'static str> {
    [
        ("fs", "file-system reads, writes, and path utilities"),
        ("os", "environment variables, working directory, and process control"),
        ("time", "wall clock, monotonic clock, and sleep"),
        ("json", "JSON parsing and serialization"),
        ("socket", "TCP/UDP sockets, bind/listen/accept/connect"),
        ("ffi", "calling native shared-library functions"),
        ("channel", "message-passing channels between threads"),
        ("thread", "spawning and joining OS threads"),
        ("shared", "shared-memory values across threads"),
        ("process", "spawning and managing subprocesses"),
        ("net", "networking tiers: raw sockets through HTTP"),
        ("net.dns", "DNS resolution and address lookup"),
        ("requests", "HTTP client with sessions, cookies, and auth"),
        ("http", "HTTP client verbs (get/post/put/...)"),
        ("http.server", "static file server; `zen -m http.server [port]`"),
        ("urllib", "URL handling aggregator (parse + request)"),
        ("urllib.parse", "URL quoting, splitting, joining, query strings"),
        ("urllib.request", "opening URLs and retrieving resources"),
        ("html", "HTML escaping and lightweight parsing"),
        ("html.parser", "tokenizing HTMLParser with feed/close"),
        ("lxml", "XML/HTML tree aggregator"),
        ("lxml.etree", "element trees, XPath subset, serialization"),
        ("lxml.html", "tolerant HTML tree parsing"),
        ("raw", "raw packet sockets and low-level frames"),
        ("crypto", "hashing, HMAC, ciphers, and key derivation"),
        ("sqlite", "embedded SQLite databases"),
        ("qr", "QR code generation"),
        ("wa", "WhatsApp client (connect, send, poll)"),
        ("ffmpeg", "audio/video probing and transcoding"),
        ("hydra", "online credential tester for line-oriented protocols (ftp)"),
        ("sys", "raw syscalls and low-level OS access"),
        ("math", "trigonometry, rounding, powers, and constants"),
        ("base64", "base-64 encoding and decoding"),
        ("binascii", "binary/hex/ascii conversions"),
        ("collections", "deque, counter, and container helpers"),
        ("string", "string constants and template helpers"),
        ("tempfile", "temporary files and directories"),
        ("random", "pseudo-random numbers and selection"),
        ("itertools", "iterator combinators (chain, cycle, product, ...)"),
        ("re", "regular expressions: match, search, split, sub"),
    ]
    .into_iter()
    .collect()
}

/// Rich help for an importable module: description plus live members.
/// Returns `None` when `name` is not a known module.
pub fn module_help(name: &str) -> Option<String> {
    let modules = native_modules();
    let unit = modules.get(name)?;
    let descs = module_descriptions();
    let mut out = String::new();
    out.push_str(&format!("module {name}\n"));
    if let Some(d) = descs.get(name) {
        out.push_str(&format!("    {d}\n"));
    }
    let mut funcs: Vec<&str> = unit.exports.iter().map(|(n, _)| n.as_str()).collect();
    funcs.sort_unstable();
    if !funcs.is_empty() {
        out.push_str("\nfunctions:\n");
        for f in funcs {
            out.push_str(&format!("    {f}(...)\n"));
        }
    }
    if !unit.const_globals.is_empty() {
        let mut consts: Vec<(u16, String)> = unit
            .const_globals
            .iter()
            .map(|(slot, v)| (*slot, summarize_value(v)))
            .collect();
        consts.sort_by(|a, b| a.0.cmp(&b.0));
        out.push_str("\nconstants:\n");
        // Slot numbers are internal; show values positionally is noise, so
        // list distinct value summaries only.
        let mut seen = std::collections::HashSet::new();
        for (_, summary) in consts {
            if seen.insert(summary.clone()) {
                out.push_str(&format!("    {summary}\n"));
            }
        }
    }
    out.push_str(&format!("\nimport with:  import {name}\n"));
    Some(out)
}

/// Short display of a constant value for the constants list.
fn summarize_value(v: &Value) -> String {
    if v.is_nil() {
        "nil".to_string()
    } else if let Some(b) = v.as_bool() {
        b.to_string()
    } else if let Some(i) = v.as_int() {
        i.to_string()
    } else if let Some(f) = v.as_float() {
        f.to_string()
    } else if let Some(s) = v.as_str() {
        format!("\"{s}\"")
    } else {
        format!("<{}>", v.type_name())
    }
}

/// All importable module names, for suggestions and bare `:help`.
pub fn module_names() -> Vec<String> {
    let mut names: Vec<String> = native_modules().keys().cloned().collect();
    names.sort();
    names
}

/// Close matches for an unknown topic (prefix matches, up to 5).
pub fn suggest(topic: &str) -> Vec<String> {
    let mut hits: Vec<String> = module_names()
        .into_iter()
        .filter(|n| n.starts_with(topic) || n.contains(topic))
        .take(5)
        .collect();
    hits.sort();
    hits
}

// ── keywords ────────────────────────────────────────────────────────────

fn keyword_docs() -> HashMap<&'static str, &'static str> {
    [
        ("func", "define a function: `func add(a, b) { return a + b }`"),
        ("var", "declare a mutable variable: `var total = 0`"),
        ("const", "declare an immutable binding: `const LIMIT = 10`"),
        ("if", "conditional: `if x > 0 { ... } else { ... }` (`elif` chains allowed)"),
        ("else", "fallback branch of `if`; also `elif cond { ... }` between them"),
        ("for", "iterate: `for x in xs { ... }`, `for k, v in dict { ... }`"),
        ("while", "loop while true: `while n > 0 { n -= 1 }` (`break`/`continue` work)"),
        ("return", "leave a function with a value: `return a + b`"),
        ("break", "exit the innermost loop immediately"),
        ("continue", "skip to the next loop iteration"),
        ("import", "use a module: `import math`, `import pkg.sub as s`"),
        ("from", "selective import: `from math import sqrt, pi`"),
        ("class", "define a class: `class Point { func init(self, x) { ... } }`"),
        ("new", "instantiate: `var p = new Point(1, 2)`"),
        ("try", "guard errors: `try { risky() } catch e { print(e) }` (+ `finally`)"),
        ("throw", "raise an error: `throw \"boom\"` (caught by `try`/`catch`)"),
        ("match", "pattern match: `match x { 1 => \"one\", _ => \"other\" }`"),
        ("lambda", "anonymous function: `var f = lambda x: x * 2`"),
        ("in", "membership / iteration: `if k in d`, `for x in xs`"),
        ("is", "identity comparison: `if x is nil`"),
        ("nil", "the absence of a value (like null/None)"),
        ("true", "boolean truth"),
        ("false", "boolean falsity"),
        ("and", "logical conjunction (short-circuits)"),
        ("or", "logical disjunction (short-circuits)"),
        ("not", "logical negation: `if not done { ... }`"),
        ("print", "write values: `print(\"total:\", total)`"),
        ("typeof", "runtime type name: `typeof(42)` is `\"int\"`"),
    ]
    .into_iter()
    .collect()
}

/// Help for a language keyword. Returns `None` when unknown.
pub fn keyword_help(name: &str) -> Option<String> {
    keyword_docs()
        .get(name)
        .map(|d| format!("keyword {name}\n    {d}\n"))
}

// ── types ───────────────────────────────────────────────────────────────

struct TypeDoc {
    blurb: &'static str,
    methods: &'static [&'static str],
}

fn type_docs() -> HashMap<&'static str, TypeDoc> {
    [
        ("int", TypeDoc {
            blurb: "64-bit signed integers: `var n = 42`. Arithmetic `+ - * / %`, comparisons, bitwise ops.",
            methods: &[],
        }),
        ("float", TypeDoc {
            blurb: "64-bit floats: `var pi = 3.14`. Whole values print as `3.0`.",
            methods: &[],
        }),
        ("string", TypeDoc {
            blurb: "UTF-8 text: `\"hi\"`, interpolation `\"x = ${x}\"`.",
            methods: &["length()/len()", "upper()", "lower()", "trim()/strip()", "lstrip()", "rstrip()", "title()", "capitalize()", "contains(s)", "starts_with(s)/startswith(s)", "ends_with(s)/endswith(s)", "find(s)/index(s)", "count(s)", "replace(a, b)", "split(sep)", "zfill(w)", "substring(i, j)/slice(i, j)", "repeat(n)", "to_int()", "to_float()", "to_num()/parse_num()", "ord()", "isalpha()/isdigit()/isalnum()/isspace()/islower()/isupper()"],
        }),
        ("bool", TypeDoc {
            blurb: "`true` / `false`. Combine with `and` / `or` / `not`.",
            methods: &[],
        }),
        ("list", TypeDoc {
            blurb: "ordered values: `[1, 2, 3]`. Index from 0; negative indices count from the end.",
            methods: &["length()/len()", "push(x)/append(x)", "pop()", "contains(x)", "sort()", "reverse()", "extend(xs)", "index(x)", "join(sep)"],
        }),
        ("dict", TypeDoc {
            blurb: "string-keyed maps: `{\"a\": 1}` or `{a: 1}`. Access with `d[\"k\"]`.",
            methods: &["length()/len()", "keys()", "values()", "items()", "contains(k)/has(k)", "get(k, default)"],
        }),
        ("set", TypeDoc {
            blurb: "unique values with set operations.",
            methods: &["length()/len()", "contains(x)", "add(x)/insert(x)", "remove(x)"],
        }),
        ("bytes", TypeDoc {
            blurb: "raw byte strings for binary I/O and native calls.",
            methods: &[],
        }),
        ("nil", TypeDoc {
            blurb: "the absence of a value. Test with `x is nil` / `x is not nil`.",
            methods: &[],
        }),
    ]
    .into_iter()
    .collect()
}

/// Help for a type name (`str` is accepted as an alias of `string`).
pub fn type_help(name: &str) -> Option<String> {
    let key = if name == "str" { "string" } else { name };
    let docs = type_docs();
    let doc = docs.get(key)?;
    let mut out = format!("type {key}\n    {}\n", doc.blurb);
    if !doc.methods.is_empty() {
        out.push_str("\nmethods (call as `value.method(...)`):\n");
        for m in doc.methods {
            out.push_str(&format!("    {m}\n"));
        }
    }
    Some(out)
}
