# 20 — `pymport`: foreign Python imports

> **Status:** Implemented (v1). The syntax, bytecode, compiler lowering, both
> runtimes, the bridge seam, the PyO3 embedding (`--features python`), the
> CLI bootstrap, and the test suites below all exist and are green. The rulings
> a reviewer should attack first are §5.2 (handles leak until bridge teardown —
> no per-handle finalizers yet), §6.3 (no cross-call IC in v1; every foreign
> call re-resolves), and §8.2 (silent `pip install` runs in the foreground,
> not the background, contradicting the blueprint's letter but not its intent).
> **Milestone:** extends M7 (FFI/handles) toward M10 (ecosystem).

---

## Contents

1. [Purpose](#1-purpose)
2. [Syntax and binding](#2-syntax-and-binding)
3. [Compilation](#3-compilation)
4. [Runtime dispatch](#4-runtime-dispatch)
5. [The bridge contract](#5-the-bridge-contract)
6. [The PyO3 embedding](#6-the-pyo3-embedding)
7. [Dependency bootstrap](#7-dependency-bootstrap)
8. [JIT interaction](#8-jit-interaction)
9. [Test plan](#9-test-plan)
10. [Limits and non-goals](#10-limits-and-non-goals)
11. [Rulings a reviewer should double-check](#11-rulings-a-reviewer-should-double-check)

> **Consumes:** the token table from [`05-lexer.md`](05-lexer.md), statement
> grammar from [`06-parser.md`](06-parser.md), the opcode table and versioning
> rule from [`07-bytecode.md`](07-bytecode.md), the `Value` model from
> [`04-value-and-types.md`](04-value-and-types.md), and handles/GC discipline
> from [`09-memory.md`](09-memory.md) §3 and [`11-ffi-handles.md`](11-ffi-handles.md).
> **Governs:** future FFI-IC work in [`10-jit.md`](10-jit.md) and any
> `requirements.txt`/venv tooling in the CLI reference.

---

## 1. Purpose

`import` loads Zen bytecode through the module linker. `pymport` loads a
CPython module through an embedded interpreter. One keyword each, decided at
parse time, so:

- the **parser** never confuses the two namespaces;
- the **compiler** emits a distinct opcode (`PyImport`, not a flag on
  `Import`);
- the **JIT** never pays a foreign check on native imports (foreign opcodes
  are simply ineligible for lowering — the function stays interpreted);
- **tooling** can see foreign dependencies with a grep over the AST
  (`zen py-sync`, editor integrations).

```zen
import fast_math        # Zen bytecode, zero foreign overhead

pymport requests        # CPython, through the bridge
pymport sys as system   # alias supported; comma lists are not
```

See the blueprint's performance argument for *why* the split exists: a
single `import` for both worlds forces dynamic probing (file extensions,
`sys.path`, venv detection) into every module resolution.

---

## 2. Syntax and binding

- `pymport dotted.path [as alias]` — one path, optional alias. No comma
  lists (one foreign boundary per statement keeps the bootstrap scanner and
  the error spans trivial).
- Binding mirrors `import`: the alias if given, the single name for
  `pymport name`, the **root** package for `pymport a.b` (Python-consistent).
- `pymport` with no module name is a parse error (`p_p3` covers it).
- The lexer treats `pymport` after `.` as an identifier (`a.pymport` lexes
  `Ident Dot Ident`), so member access never breaks.

---

## 3. Compilation

`Stmt::Pymport` lowers in `compile_pymport`, which shares
`emit_module_load` with `compile_import`: the module path rides the constant
pool as a dotted string, the destination register gets `PyImport`, and the
namespace chain for dotted paths is built from nested one-pair dicts exactly
like native imports. The **only** difference is the opcode. Consequences:

- Live-range, register, and jump behavior are identical to `import`, so no
  backend pass needs a `pymport` case.
- `import` never emits `PyImport` and vice versa (compiler tests assert both
  directions).

Bytecode: `PyImport` is discriminant 81 (`BYTECODE_VERSION` 9 → 10), operands
`a` = dest, `b` = path constant, `c` unused — same shape as `Import`.

---

## 4. Runtime dispatch

Both engines route foreign traffic through `zen_vm::pybridge`:

- **Bytecode VM** (`vm.rs`): `PyImport` resolves via the bridge and binds a
  `Foreign` handle value. `GetField`/`SetField` on foreign values call
  `getattr`/`setattr`; `Call` on a foreign callable calls `call`;
  `CallMethod` fuses into `call_attr` (one bridge round-trip). Misses and
  marshal failures surface as `NativeError` with the bridge's message.
- **Tree-walk interpreter** (`zen-interp`): `Stmt::Pymport` binds
  `Ev::Foreign(handle)`. Field, method, and direct-call evaluation each have
  a foreign arm; field assignment (`exec_assign`) routes through the bridge
  too. Differential tests assert both engines agree on values *and* errors.
- **Unlinked builds** (no `python` feature): every foreign operation fails
  with an actionable message naming `--features python` and libpython. There
  is intentionally no probing, no fallback, and no silent degeneracy.

`typeof` of a foreign value is `"foreign"` (`ObjKind::Foreign`, discriminant
16, appended per the never-renumber rule).

---

## 5. The bridge contract

`PyBridge` (object-safe, `Send + Sync`) is the only seam an embedding
implements: `import_module`, `getattr`, `setattr`, `call`, plus `release`
and an overridable fused `call_attr` (default threads `getattr` into
`call`; embeddings SHOULD override it to resolve-and-dispatch server-side).

The v1 marshaling boundary is `PyValue`: nil/bool/int/float/string/bytes
plus flat lists/dicts of those; **everything else crosses as an opaque
handle** (`PyValue::Handle`). Deliberate rejections (never silent
coercions): non-string dict keys, instances, funcs, unresolvable shapes.
Big Python ints beyond `i64` arrive as `f64` (documented precision caveat);
tuples arrive as lists (Zen draws no list/tuple distinction at the boundary).

`UnlinkedBridge` implements the trait with errors, so the seam itself —
including the default `call_attr` threading and both marshal directions —
is unit-tested with no Python present.

---

## 6. The PyO3 embedding

`zen_vm::pyembed::PythonBridge` (gated on `--features python`; default builds
stay hermetic) drives system CPython through PyO3 0.30:

- Registry `u32 → Py<PyAny>` under a mutex; ids start at 1 (0 is invalid).
- One GIL section per bridge call (`Python::attach`); `call_attr` fuses
  resolve+dispatch into a single section — the v1 fast path.
- Export is by explicit type check (`PyBool`/`PyInt`/`PyFloat`/`PyString`/
  `PyBytes`/`PyList`/`PyTuple`/`PyDict`), never trial extraction: trial
  order misclassifies (a small-int tuple extracts as bytes).
- Errors are captured to strings with the fetched error cleared, so nothing
  prints or warns on drop.
- `release` removes registry entries (manual; Zen-side `Drop` cannot reach
  the bridge, so entries otherwise live until bridge teardown — bounded by
  distinct handles).

The CLI attaches the bridge in `run_linked` when the feature is on;
initialization itself stays lazy (auto-initialize on first GIL use), so
non-`pymport` programs pay nothing.

---

## 7. Dependency bootstrap

`zen-cli/src/pydeps.rs`, wired into `zen run` and exposed as `zen py-sync`:

1. **Scan** every linked source — entry plus transitive `.z` imports — with
   the real parser (`scan_pymports`, recursive over nested blocks). A helper
   that itself pymports is covered exactly like top-level code; scanning
   only the entry file would miss it.
2. **Resolve** the project root (nearest `requirements.txt`/`zen.toml`
   upward, else the entry dir).
3. **Plan** (`plan_bootstrap`, pure filesystem reads): `Clean` (no pymports
   or fresh stamp), `NoRequirements` (report only — a module name is not a
   pip name, so Zen never guesses), `MissingVenv`, `MissingPackages`.
4. **Ensure** (`ensure_bootstrap`): create `.zen_venv`, `pip install -r`,
   write the stamp (requirements length + FNV-1a hash). Skipped entirely
   under `ZEN_NO_PY_BOOTSTRAP` (tests, hermetic runs, benchmarks).
5. **Point** the embedded interpreter at the venv (`PYTHONPATH` prepend of
   `lib/python*/site-packages`; relative requirements paths resolve against
   the project root, not the invoker's CWD).

`ensure` runs in the **foreground**: starting execution before packages
exist would fail anyway, so determinism wins over the blueprint's background
wording. Failures report and continue — the `PyImport` runtime error is the
honest signal when packages are still missing.

---

## 8. JIT interaction

`PyImport` (and any function containing it) is **ineligible for lowering**:
the function stays interpreted, which is the correct slow path for a runtime
boundary. A dedicated test pins the rejection.

The blueprint's `FfiCallIc` (inline pointer guards in Cranelift) is future
work with a precise precondition: a CPython invalidation signal for cached
callables (modules allow attribute reassignment, so an unguarded cache can
call a stale function — a wrong answer, not a missed optimization). Until
that signal exists, caching lives at most in the bridge behind explicit
version checks. The fused `call_attr` is the v1 substitute: it removes the
round-trip, which dominates the lookup.

---

## 9. Test plan

| Layer | Suite | Notes |
|-------|-------|-------|
| Lexer | `lex_tokens::keyword_pymport`, `pymport_after_dot_is_ident` | discriminant 144, member-safe |
| Parser | `parser::p_p1..p_p3` | shape, alias, errors |
| Compiler | `compiler_tests::pymport_*` (3) | emission + no cross-talk either way |
| VM (mock) | `zen-vm/tests/pymport.rs` (9) | full dispatch, no libpython |
| Interp↔VM | `zen-interp/tests/pymport.rs` (6) | differential values + errors |
| Bridge (real) | `zen-vm/tests/pyembed.rs` (13, `python` feature) | stdlib-only, no network |
| Bridge (third-party) | `pyembed.rs` paramiko tests (3, `python` feature) | `SSHConfig.from_text` parsing, `RSAKey.generate`, fingerprint bytes; skip cleanly when paramiko is absent |
| CLI | `pydeps` unit tests (11) | scan/plan/stamp/venv layout, offline |
| JIT | `jit_pymport::pyimport_stays_interpreted` | rejection pinned |
| E2E | venv + local package via `py-sync` + `run` | manual (needs pip); `double(21) == 42` |

---

## 10. Limits and non-goals

- No keyword arguments in v1 (`call_attr` is positional-only).
- No Zen closures passed into Python (marshal rejects funcs/instances).
- No per-handle finalizers (registry entries live until teardown; `release`
  is manual).
- No `zen.toml [py-deps]` yet (`requirements.txt` only).
- `tuples → lists`, big ints → `f64`, non-string dict keys rejected.
- Only the stdlib portion of any Python install is reachable without a
  venv + requirements; `pip install` runs only via `py-sync` or the `run`
  preamble (never silently in tests). Verified end-to-end with a real
  third-party package: `py-sync` installed `paramiko` into `.zen_venv` from
  a local `requirements.txt`, and `run` imported the venv's copy (5.0.0 over
  the system's 4.0.0) — `SSHConfig` parsing, `RSAKey` generation, and key
  fingerprints all round-trip.
- Cross-isolate sharing of foreign handles is untested (same caveat as
  §11 of `11-ffi-handles.md` for native handles).

---

## 11. Rulings a reviewer should double-check

1. **§5 `call_attr` default**: errors when `getattr` yields data. The real
   bridge overrides it and lets Python raise `TypeError` instead — two
   spellings of the same type error. Acceptable divergence, or unify on the
   Python message?
2. **§5.2 handle lifetime**: leak-until-teardown. Fine for v1 volumes;
   revisit with finalizers before any long-running server story.
3. **§6 registry `Mutex`**: one lock per bridge op. Contended multi-threaded
   callers will serialize here; a sharded map is the obvious follow-up.
4. **§7 foreground install**: contradicts the blueprint's background
   wording deliberately — confirm the determinism argument holds for the
   REPL/`-e` paths too (both funnel through the same preamble).
5. **§8 no guards, no IC**: confirm the team agrees an unguarded FFI cache
   is a wrong-answer risk, not just a missed optimization.
