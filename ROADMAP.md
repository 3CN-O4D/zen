# Native Zen roadmap

This is a ground-up implementation. It shares Zen source syntax, but has no
runtime dependency on the Python package. Each milestone must have executable
conformance tests before it is considered complete.

## M1 — native language core (in progress)

- [x] CLI, source files, inline evaluation, release builds
- [x] Lexer with locations, comments, literals, operators, delimiters
- [x] Variables, arithmetic, comparisons, boolean logic, strings, lists
- [x] Inclusive ascending and descending ranges
- [x] Dictionaries, member access, indexing and negative list indexes
- [x] Conditionals and loop control
- [ ] Assignment through indexes/members and immutable `const`
- [x] Named functions, parameters, return values and recursion
- [ ] Closures, function values, default parameters, lambdas and callbacks
- [ ] `try`/`catch`/`finally`, `throw`, `assert`
- [x] Classes, objects, constructors, fields, inherited methods and instance-method dispatch
- [ ] Destructuring, interpolation and comprehensions

## M2 — standard library

- [ ] Deterministic collection/string methods
- [ ] JSON, regular expressions, date/time, files, crypto and HTTP
- [ ] Module resolution and a versioned native package format
- [ ] Structured diagnostics with source excerpts and call stacks

## M3 — browser automation

- [ ] Native Chrome DevTools Protocol transport and browser launcher
- [ ] Navigation, selectors, element actions, screenshots and downloads
- [ ] HTTP-only mode
- [ ] Browser integration tests using a local fixture site

## M4 — distribution and compatibility

- [ ] Linux, macOS and Windows release artifacts
- [ ] `zen check` and `zen fmt`
- [ ] A shared syntax/behaviour conformance suite for Python Zen and Rust Zen
- [ ] Migration guide and full native user documentation

## Design rules

1. The Rust runtime never starts Python as a fallback.
2. Unsupported constructs fail with a location-aware error.
3. The language semantics are tested independently of browser support.
4. Browser code lives behind a runtime interface so `--no-browser` stays fast.
