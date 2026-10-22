# 11 — FFI, handles, and the native module ABI

> **Status:** Draft for review. The rulings a reviewer should attack first are §4.1 (a handle is a
> 24-byte heap object, not an integer in a `0b111` tag — the first draft of this document had it
> wrong, and §4.1 says how), §4.3 (the handle container — the blueprint says `DashMap`, this
> document does not), §4.5 (the identity pool leaks 24 bytes per handle until M9), §7.4 (replacing
> `libffi` with a hand-rolled signature dispatcher instead of `libloading`), and §9.2.1 (rolling
> back the `Vm`'s structural state when a native panics, which is what makes `catch_unwind` at the
> native boundary — §9.2, a ruling that reverses nothing in [`08-vm.md`](08-vm.md) §4.1 but is easy
> to mistake for a reversal — actually sound).
> **Milestone:** M7 (FFI, handles, `sys`/`os`/`fs`/`time`) for §2–§6, §8 and §10. The user-facing
> `dlopen` escape hatch of §7 and everything in §11 that touches the network are M10.

---

## Contents

1. [Purpose](#1-purpose)
2. [The native function ABI](#2-the-native-function-abi)
3. [Module registration](#3-module-registration)
4. [The handle table](#4-the-handle-table)
5. [The stale-handle problem](#5-the-stale-handle-problem)
6. `typeof(handle) == "handle"`
7. [`dlopen` and the user FFI escape hatch](#7-dlopen-and-the-user-ffi-escape-hatch)
8. [The `sys` module plan](#8-the-sys-module-plan)
9. [Safety rules](#9-safety-rules)
10. [Test plan](#10-test-plan)
11. [Milestone sequencing](#11-milestone-sequencing)
12. [Rulings a reviewer should double-check](#12-rulings-a-reviewer-should-double-check)

> **Consumes:** the `NativeFn` ABI and `NativeEntry` layout from [`08-vm.md`](08-vm.md) §8; the
> `Value` representation from [`04-value-and-types.md`](04-value-and-types.md) and
> [ADR-0001](decisions/ADR-0001-tag-budget.md); the no-panic contract from
> [`00-blueprint-review.md`](00-blueprint-review.md) §5.
> **Governs:** [`12-concurrency.md`](12-concurrency.md) §3 (a handle is one of the values whose
> thread-safety the affinity tag decides) and [`13-networking.md`](13-networking.md) §3–§5 (every
> socket is a handle).

---

## 1. Purpose

This document answers one question in both directions.

**Outbound: how does a Rust function become a Zen function?** Everything in Zen 2.x that is not
written in Zen is a Rust function behind a `Value::NativeFunction(String)` name lookup — a string
compared per call, resolved through a `match` with hundreds of arms (`src/runtime.rs:18202` and
its neighbours). That is not an ABI, it is a lookup table with a linear scan. §2 defines the real
ABI (`fn(&mut Vm, &[Value]) -> VmResult<Value>`, already frozen in
[`08-vm.md`](08-vm.md) §8.1), §2.4 defines the argument helpers that make it impossible to write a
native that copies a string by accident, and §3 defines how a crate says "this is my module".

**Inbound: how does Zen code touch the OS without raw pointers in the language?** The blueprint
gets this right and it is the one idea in the FFI chapter that survives review unchanged
([`00-blueprint-review.md`](00-blueprint-review.md) §1, K7): the language has no pointer type, so a
file descriptor, a TCP connection, an mmap region, a sqlite connection and a `dlopen`ed library are
all a small integer naming a slot in a process-global table. §4 designs that table, §5 designs the
part that actually matters — what happens when the slot number is reused — and §6 makes `typeof`
tell the truth about it.

**The load-bearing claim of this document** is §5: *a handle to a closed resource is a catchable
`NativeError`, never a dangling pointer, never a wrong-but-plausible answer, and never undefined
behaviour.* Everything else in §4 is a performance argument; §5 is a safety argument and it is the
one that decides the design.

**What this document does not do.** It does not decide the threading model
([`12-concurrency.md`](12-concurrency.md)), the networking layers
([`13-networking.md`](13-networking.md)), or the `Value` layout
([`04-value-and-types.md`](04-value-and-types.md)). It consumes all three.

### 1.1 The three hard problems, stated up front

| # | Problem | Why it is hard | Where |
|---|---|---|---|
| H1 | A native borrows its arguments; the OS wants to keep them | The C ABI has no lifetimes, so a callee may retain the pointer after return | §2.4 |
| H2 | The OS recycles file descriptors, and so do we recycle slots | Without a generation counter, a stale handle silently names a *different live resource* | §5 |
| H3 | `libffi` will call a function with whatever signature you told it to | There is no way to check; a wrong signature corrupts the stack | §7.4 |

H1 has a known answer (copy at exactly one well-defined place, and nowhere else). H2 has a known
answer (generation counters, ~15 lines, converts UB into a typed error). H3 does not have a good
answer in the existing tree, and §7.4 proposes one that narrows the feature rather than widening it.

---

## 2. The native function ABI

### 2.1 The signature, as real Rust

This is not a proposal; it is the interface already specified in
[`08-vm.md`](08-vm.md) §8.1 and restated here because every other section depends on it.

```rust
// zen-vm/src/native.rs

/// A native that only reads its arguments. The slice points **directly into
/// `vm.registers`**, at the callee's window base. No allocation, no copy.
pub type NativeFn = fn(&mut Vm, &[Value]) -> VmResult<Value>;

/// A native that needs to mutate its arguments in place, or grow a slice
/// (`*args`, `**kwargs`, `sort`). Writes into `vm.native_args`.
pub type NativeFnV2 = fn(&mut Vm, &mut [Value]) -> VmResult<Value>;

/// Every native returns this. `Err` is the only failure path; there is no
/// sentinel, no side channel, and no sentinel string.
pub type VmResult<T> = Result<T, VmErr>;
```

`NativeEntry` grows one field. It is 24 bytes today ([`08-vm.md`](08-vm.md) §8.1); it becomes 28
bytes with the blocking class, and this document needs that field for
[`12-concurrency.md`](12-concurrency.md) §7 and §10.3:

```rust
#[derive(Clone, Copy)]
#[repr(C)]
pub struct NativeEntry {
    /// Arity check the VM performs before the call. `Arity::Variadic` skips it.
    pub arity: Arity,
    /// The v1 or v2 function, discriminated by `is_v2`.
    pub f: NativeFn,
    /// Zen-visible name, for `NameError: print is not a function`.
    pub name_id: u32,
    pub is_v2: bool,
    /// NEW in M7. Can this native park the calling OS thread? Used by
    /// `spawn_async` (§7 of 12-concurrency.md) and by the deadlock watchdog
    /// (§10.3 there). Defaults to `MayBlock`.
    pub blocking: BlockingClass,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BlockingClass {
    /// Pure computation. Cannot block. e.g. `len`, `math.floor`, `json.encode`.
    NeverBlocks = 0,
    /// May block for an unbounded time on an external resource. e.g. `read`,
    /// `accept`, `http.get`. The default, because a wrong `MayBlock` is only
    /// a missed optimisation.
    MayBlock = 1,
    /// Blocks until the resource ends, e.g. `recv` with no timeout. The
    /// watchdog is not required to flag it if the script called
    /// `sys.set_timeout` first.
    BlocksForever = 2,
}
```

**Why `MayBlock` is the default and not `NeverBlocks`.** The two failure modes are asymmetric. A
native that claims `NeverBlocks` but blocks is a *liveness* bug: the deadlock watchdog in
[`12-concurrency.md`](12-concurrency.md) §10.3 reports a false stall, and `spawn_async` holds an OS
thread it did not need to. A native that claims `MayBlock` but never blocks is a *performance* bug
on a cold path. Optimise for the worse failure. The compiler audits `NeverBlocks` in debug builds
under `zen test --native-audit`, which runs each such native once under a 50 ms watchdog and
reclassifies any that trips it — opt-in, because it triples the suite's wall time.

### 2.2 The argument-count check

The check happens in the VM, before the call, and it is a `#[cold]` error arm. The VM does it, not
the native, because doing it in the native means several hundred functions each re-implementing
the same comparison.

```rust
// zen-vm/src/exec.rs
#[cold]
fn native_arity_mismatch(&mut self, e: &NativeEntry, got: usize) -> VmErr {
    let name = self.intern.resolve(e.name_id);
    let want = match e.arity {
        Arity::Exact(n)  => format!("{n} argument{}", plural(n)),
        Arity::Min(n)    => format!("at least {n} argument{}", plural(n)),
        Arity::Max(n)    => format!("at most {n} argument{}", plural(n)),
        Arity::Range(a, b) => format!("{a}..{b} arguments"),
        Arity::Variadic  => "any number of arguments".into(),
    };
    VmErr::raise(ErrorKind::TypeError,
                 format!("{name} expects {want}, got {got}"),
                 self.current_span())
}
```

| `Arity` | Zen call form | Examples | Check |
|---|---|---|---|
| `Exact(n)` | fixed | `sys.close(fd)`, `math.floor(x)` | `args.len() == n` |
| `Min(n)` | trailing optional | `sys.open(path, flags)` — 2 or 3 | `args.len() >= n` |
| `Max(n)` | leading optional | `dict.get(d, k)` | `args.len() <= n` |
| `Range(a, b)` | positional + `*rest` | `http.get(url, headers)` | `a <= len <= b` |
| `Variadic` | `*args` / `**kwargs` | `print`, `min`, `max`, `error` | none; the v2 ABI stages into `vm.native_args` |

The error names the function and both counts. `TypeError`, not `NativeError`: an arity mismatch is
a type error in every language that has one, and it is the error a script author recognises.

### 2.3 The error path

There is exactly one error path and it is a `Result`, because
[`08-vm.md`](08-vm.md) §4.1 rejected the Zen 2.x side channel (`Vm::thrown_values` plus the
`INTERNAL_THROW_SENTINEL` string, `src/runtime.rs:168-172`) for exactly the reason a native
boundary is the worst possible place to have one.

```rust
/// Every `Err` a native can produce. Constructed with `?` or with the
/// `raise` helpers; never from a bare `String`.
#[derive(Debug, Clone)]
pub struct VmErr {
    pub kind: ErrorKind,
    pub msg: String,
    /// Source span, for `bt`. A native that has no span of its own (there is
    /// no Zen source for `read(2)`) leaves this `None` and the VM substitutes
    /// the span of the call site.
    pub span: Option<Span>,
}

pub enum ErrorKind {
    TypeError, IndexError, KeyError, NameError,
    DivideByZero, StackOverflow, OutOfMemory,
    /// Anything the host did: `read` returned `EBADF`, `dlopen` returned NULL,
    /// a handle was stale (§5), a lock timed out.
    NativeError,
}
```

Three rules, each of which is a bug class in the old tree that is not being carried forward:

1. **A native returns `Err`; it never mutates VM error state and returns `Ok`.** There is no
   `vm.pending_error` a native can set and forget. A native that genuinely swallows an error must
   return `Ok` with a value that says so — that is what a `(ok, value)` tuple return is for, and
   it is the only legitimate use of one.
2. **A native does not `format!` on the happy path.** The happy path must not allocate a `String`
   it discards. All the `format!`s live behind `#[cold]` helpers.
3. **A native's `Err` message is built before the unwinder runs.** It is the only allocation on
   the error path, which is why a native that captures `errno` must do so *first* (§7.6).

### 2.4 Zero-copy strings and bytes, and the places a copy is unavoidable

`String` is `Arc<str>` and `Bytes` is `Arc<[u8]>` ([`08-vm.md`](08-vm.md) §8.2, ADR-0002). The
`&[Value]` slice handed to a v1 native points into the register file, and each `Value` in it holds
an `Arc`. Borrowing a `&str` out of an `Arc<str>` is therefore **a pointer read, not a copy** — the
`Arc` keeps the allocation alive for the length of the borrow, and the borrow ends when the native
returns. The same is true of `&[u8]`.

```rust
// zen-vm/src/arg.rs — the ONLY sanctioned way for a native to read an argument.

/// Borrow argument `i` as a `&str`. Zero-copy.
/// # Errors
/// `TypeError: <fn>: argument <i> is <actual>, expected string`
#[inline]
pub fn arg_str<'a>(vm: &'a Vm, args: &'a [Value], i: usize, what: &str) -> VmResult<&'a str> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s),
        Some(other) => Err(type_error(what, i, "string", other.type_name(), vm)),
        None => Err(missing_arg(what, i, vm)),
    }
}

/// Borrow argument `i` as bytes. Accepts a `bytes` (zero-copy) and a
/// `string` (also zero-copy — `Arc<str>` is UTF-8 by construction, so
/// `s.as_bytes()` is free). A `list` of ints is a **copy**, and is the only
/// accepting conversion that costs anything.
#[inline]
pub fn arg_bytes<'a>(vm: &'a Vm, args: &'a [Value], i: usize, what: &str) -> VmResult<Cow<'a, [u8]>> {
    match args.get(i) {
        Some(Value::Bytes(b)) => Ok(Cow::Borrowed(b)),
        Some(Value::Str(s))   => Ok(Cow::Borrowed(s.as_bytes())),
        Some(Value::List(l))  => Ok(Cow::Owned(list_to_bytes(l, what, vm)?)),
        other => Err(/* TypeError naming the actual type */),
    }
}
```

The `Cow` in `arg_bytes` is not decoration: it is the honest return type, because the
list-of-ints case genuinely must allocate. A signature returning `&[u8]` would either lie or panic.

**A worked example, the one that matters.** `sys.write` is a v1 native taking a Zen `string`:

```rust
// zen-std-sys/src/write.rs
pub fn sys_write(vm: &mut Vm, args: &[Value]) -> VmResult<Value> {
    let h = arg_handle(vm, args, 0, "write", ResKind::Fd)?;
    // Zero-copy: a borrow of the Arc<str> payload. No allocation.
    let data = arg_str(vm, args, 1, "write")?;
    // `write(2)` takes a `const void*` and the kernel copies before it
    // returns, so the borrow is valid for the whole call and no Zen-side
    // copy is needed.
    let n = unsafe { libc::write(h.raw(), data.as_ptr().cast(), data.len()) };
    if n < 0 {
        return Err(vm.errno_error("write", vm.take_errno()));
    }
    Ok(Value::from_int(n as i64))
}
```

**Where a copy is genuinely unavoidable.** This is the part that gets hand-waved in most FFI
documentation, so it is enumerated:

| # | Situation | Why a copy is forced | Cost | Who pays it |
|---|---|---|---|---|
| C1 | A native must retain the value past the call | The borrow ends at return, and the `Arc` may be the last one | **O(1)** — `Arc::clone` is an atomic increment, *not* a copy | Nobody, in practice. This is the whole reason ADR-0002 chose `Arc<str>` over an inline SSO string |
| C2 | A native **mutates** a `string`/`bytes` argument in place | `Arc<str>` is immutable; `Arc::make_mut` clones whenever `rc > 1`, which is *always*, because the register file holds one reference and the argument list another | **O(n)**, one allocation | The caller, and the module's docs must say "this copies". There are exactly four such natives in M7 and all four are documented as copying |
| C3 | Passing a string to C that may **retain** the pointer (`setenv`, `strdup`, `asprintf`) | The C callee has no lifetime contract; the pointer must stay valid after return | **O(n)** | The native owns a `CString` in a `CallData` guard for the call's duration. Zen 2.x does this correctly: `CallData { cstrings, buffers }` at `src/ffi.rs:108-111` |
| C4 | A string containing an interior NUL | `CString::new` fails; the C ABI cannot express a NUL-terminated string with an interior NUL | **The call is rejected** | `TypeError: sys.open: path contains a NUL byte`. Not a silent truncation. Zen 2.x gets this right: `src/ffi.rs:162-164` |
| C5 | A `list` of ints passed where `&[u8]` is wanted | The list is a vector of tagged `Value`s with a stride of 8, not a byte array | **O(n)** | `arg_bytes` returns `Cow::Owned` |
| C6 | Returning a string that C allocated (`strdup`, `getenv`) | Ownership transferred to us; we must free it and copy it into an `Arc<str>` | **O(n)** | Unavoidable. `read_cstr` at `src/ffi.rs:53-70` does this and handles the null-pointer case by returning `""`, which is a decision: keep it, document it |

**The ruling:** a native that does not need to keep a value does not copy it; a native that mutates
one copies it and says so in its docs; a native that hands a pointer to C may retain, and copies
and guards. There is no fifth case, and there is no case where the copy is hidden from the caller.

### 2.5 Re-entrancy: the one rule a native must not break

From [`08-vm.md`](08-vm.md) §8.3: a native holding `&mut Vm` across a Zen call is an aliasing
problem, because the Zen call pushes frames and can move the register window the native is
borrowing. The rule, restated as something a reviewer can check:

> A native **must not** hold a `&Value`, a `&[Value]`, or any pointer derived from `vm.registers`
> across a call to `vm.call_zen(...)`, `vm.call_native(...)`, or any v2 native that can re-enter.

```rust
// Vm::call_zen asserts !self.in_native_reentry
debug_assert!(!self.in_native_reentry,
    "a native re-entered the VM while holding a register borrow");
```

Violations are not memory-unsafe in M4–M8 (the register file is a fixed array and is only ever
*logically* moved, per [`08-vm.md`](08-vm.md) §2.2), but they become memory-unsafe the moment the
M9 collector lands, because a re-entrant native means the root map has a window in which a
borrowed `Value` is not a root. Test 12 in §10 asserts the flag fires.

---

## 3. Module registration

### 3.1 `zen_register_module`

The blueprint's spelling is right; it needs a signature and a return value.

```rust
// zen-vm/src/module.rs

/// A Zen module's function table, built by the owning crate.
pub struct Module {
    /// Dotted, e.g. `net.http`. Lowercase ASCII `[a-z0-9_.]`. A registered
    /// name is stable forever; renaming one is a breaking change.
    pub name: &'static str,
    /// Zen-visible members, in **documentation order**. A module is a dict
    /// with deterministic iteration order, because `dir(net.http)` and
    /// tab-completion must not shuffle between runs.
    pub funcs: &'static [(&'static str, NativeEntry)],
    /// Re-entrant initialiser, called at first `import`. May register nested
    /// modules (`net.http` also makes `net` reachable). Must be idempotent.
    pub init: Option<fn(&mut Vm) -> VmResult<()>>,
}

/// The registration entry point a crate calls. Exactly one per module.
/// Returns 0 on success, or a negative code on a rejected registration
/// (bad name characters, duplicate name, `.`/`/` namespace confusion).
///
/// # Safety
/// `name` must be a NUL-terminated UTF-8 string and `funcs` must be a valid
/// pointer to a `#[repr(C)] NativeEntry` array of the length it claims.
/// Both are read once and copied into owned storage; neither is retained.
#[no_mangle]
pub extern "C" fn zen_register_module(name: *const c_char, funcs: *const NativeEntry) -> i32;
```

The `#[no_mangle]` is not premature: M8's `zenn` toolchain must be able to load a *compiled* Zen
plugin (a `.cdylib`) that exports natives, and the ELF/Mach-O contract is an exported symbol. The
symbol exists from M7 with an empty registration path so the ABI is frozen before anything depends
on it — the same reasoning as [`08-vm.md`](08-vm.md) §9's `zen_jit_compile`.

`name: *const c_char` is the one raw pointer in the design, and it is the right one: a
NUL-terminated string from a `dlopen`ed plugin, read once into an owned `String`, never stored.

### 3.2 The Rust-side `register()` pattern

Every native module crate follows one shape:

```rust
// crates/zen-std-net/Cargo.toml
// [lib]
// name = "zen_std_net"
// crate-type = ["rlib", "cdylib"]
// [features]
// default = []
// ffi = ["dep:libloading"]      // §7.5

// crates/zen-std-net/src/http.rs
use zen_vm::{Arity, BlockingClass, Module, NativeEntry, NativeFn, Vm, VmResult, Value};

pub fn register(vm: &mut Vm) -> VmResult<()> {
    // `static` function items, never closures: `NativeEntry` holds a bare
    // `fn` pointer, so it is `Copy` and the table is `'static` and lives in
    // `.rodata` with zero relocations. A closure needs an environment, and an
    // environment cannot be `'static`.
    static GET: NativeFn = net_http_get;
    static POST: NativeFn = net_http_post;
    static STREAM: NativeFn = net_http_stream;

    let table: &[(&str, NativeEntry)] = &[
        ("get",    NativeEntry::new(Arity::Min(1),    GET,    "http.get",    BlockingClass::MayBlock)),
        ("post",   NativeEntry::new(Arity::Min(1),    POST,   "http.post",   BlockingClass::MayBlock)),
        ("stream", NativeEntry::new(Arity::Exact(2),  STREAM, "http.stream", BlockingClass::MayBlock)),
    ];
    zen_vm::module::install(vm, Module { name: "net.http", funcs: table, init: None })
}

fn net_http_get(vm: &mut Vm, args: &[Value]) -> VmResult<Value> { /* … */ }
```

Three things about that shape are load-bearing:

- **`static` function items, never closures** (above).
- **`init` is optional and idempotent.** A module with no state (`net.http`) passes `None` and
  `import` is free. A module with state (`sys`, because of the errno slot in §7.6) uses it, and
  `install` calls it under an "already installed" check.
- **The feature flag lives in `Cargo.toml`, not in a runtime check** (§7.5).

### 3.3 How an `import` resolves

Zen 2.x resolves `import json as j` by trying the native module table, then the filesystem, with
no caching discipline that survives a loop (`tests/cases/10_import_forms.z:44-49` re-imports inside
a `while` five times and asserts the cache holds). M7 formalises the order. **The native table is
checked first**, so a native module can never be shadowed by a stray file on disk — the opposite
order is a path-injection vulnerability in any language that lets a script write files.

| Step | Lookup key | If found | If not |
|---|---|---|---|
| 1 | `native_modules[name]` — a `HashMap<&'static str, ModuleId>` in the `Vm` | Bind the module dict. **No filesystem access.** | → 2 |
| 2 | prefix scan: does any registered name start with `name + "."`? | Bind a **namespace dict** of submodules, so `net.http` is reachable as `net.http` after `import net` | → 3 |
| 3 | each `VM_SEARCH_PATH` entry: `<dir>/<name>.z` | Parse, cache in `vm.module_cache` | → 4 |
| 4 | each entry: `<dir>/<name>/__init__.z` | Same | → 5 |
| 5 | each entry: `<dir>/<name>.zeno` (compiled bytecode, M8) | `validate()` then load ([`08-vm.md`](08-vm.md) §4.2) | → 6 |
| 6 | `ZEN_STD_PATH/std/`, same five shapes | Same | → error |

```rust
pub enum ImportError {
    NotFound { name: String, searched: Vec<PathBuf> },
    /// A `.z` file that resolved to a package directory with no `__init__.z`.
    EmptyPackage { dir: PathBuf },
    /// A `.zeno` file that failed `validate()`. Carries the offending opcode.
    BadBytecode { path: PathBuf, detail: String },
    /// `import ffi` without `--allow-ffi`. §7.3.
    FeatureDisabled { module: &'static str, flag: &'static str },
}
```

| Zen form | Resolves via | Caching | Notes |
|---|---|---|---|
| `import net.http` | Step 1, native | `OnceCell` per `Vm`, keyed by name | Idempotent; a re-import inside a loop is a hash lookup |
| `import net.http as h` | Step 1, native + alias binding | Alias is a global binding | Binds a **name**, not a copy; `h` and `net.http` are the same dict object |
| `from net.http import get` | Step 1, then one member lookup | Same module cache | A missing member is `NameError: net.http has no member 'gett'`, listing near-misses |
| `from net.http import *` | Step 1, all public members | Same | Excludes `_`-prefixed names. A lint, not a hard error — the checker that would host it is not a document in this series and is not scheduled by any milestone in it, so the rule lives here: star-import is legal, `_`-prefixed names stay private, and `zen check --pedantic` reports every star-import it sees |
| `import pkg.sub.mod` | Steps 3/4 per segment | Same | Dotted packages; `tests/cases/10_import_forms.z:51-56` is the conformance test |
| `import ffi` | Step 1 **plus the gate check** | `Err(FeatureDisabled)` without `--allow-ffi` | §7.3 |
| `import sqlite` | Step 1, native | `init` runs once | `init` opens the handle table if it is not already open |

**Naming ruling.** Dotted native names use `.`; filesystem modules use `/` on disk. A native
module named `net/http` is rejected at registration with a negative return code — the namespaces
are not interchangeable, and pretending otherwise makes `import net.http` ambiguous on a machine
that happens to have a `net/http.z` file.

### 3.4 The module dictionary

A module binds as a Zen `dict` whose values are `func` values wrapping a `(NativeId, Arity)`. This
is what makes `dir()` and attribute access work, and it is why `Value::NativeFunction(String)` has
to go: a `String` name means a hash lookup, a `match`, and a per-call comparison on the hottest
path in the runtime.

```rust
/// A `func` value. `proto_id` is 0 for a native.
pub struct Closure {
    pub proto: ProtoId,          // 0 for natives
    pub native: NativeId,        // index into `vm.natives`; usize::MAX for a Zen closure
    pub upvalues: Arc<[Value]>,  // empty for a native
    pub name_id: NameId,         // for `bt`
}
```

`NativeId` is a `u32` index, so a native call site is `vm.natives[id]` — one bounds-checked load
with the bound checked at load time, exactly like a constant-pool index
([`08-vm.md`](08-vm.md) §4.3). The Zen 2.x string comparison is the difference between a `match`
with hundreds of arms and a `match` with two.

### 3.5 `CallData`: keeping C-owned arguments alive for exactly one call

A native that passes a Zen `string` to C keeps the borrow for the call. A native that hands C a
pointer it *invented* must own the memory for the duration. §2.4's `Cow` covers the first case;
this covers the second, and the Zen 2.x implementation is already correct and is carried over
essentially unchanged:

```rust
// zen-ffi/src/user_ffi.rs
/// Keeps every allocation the marshaller invented alive for one call.
struct CallData {
    cstrings: Vec<CString>,
    buffers: Vec<Vec<u8>>,
}
// Zen 2.x: `src/ffi.rs:108-111`. The `Box<u64>` / `Box<f64>` arenas that
// pinned each scalar's address for `libffi::middle::Arg` are removed along
// with libffi — §7.4.
```

---
## 4. The handle table

### 4.1 What a handle is

**Ruling: a handle is a heap object with `ObjHeader.kind == OBJ_HANDLE`, and it carries a
48-bit identity token.** This is not a stylistic preference; it is forced by
[ADR-0001](decisions/ADR-0001-tag-budget.md) together with
[ADR-0004](decisions/ADR-0004-honest-types.md), and the first draft of this document got it wrong in
a way worth writing down.

**The tag budget has no room for a handle tag.** The canonical assignment, which
[`09-memory.md`](09-memory.md) §2.4 establishes as the one that wins over
[`08-vm.md`](08-vm.md) §8.2's illustration, spends **all eight** of the three-bit tags:

| Tag | Type | Tag | Type |
|---|---|---|---|
| `0b000` | `nil` | `0b100` | `float` (raw `f64` bits) |
| `0b001` | `false` | `0b101` | `string` (`*const u8`, thin) |
| `0b010` | `true` | `0b110` | `bytes` (`*const u8`, thin) |
| `0b011` | `int` (i48) | `0b111` | the other nine types, discriminated by `ObjHeader.kind` |

There is no ninth tag, and `0b110` is not spare: 08's "reserved, M9 GC mark bits live here" comment
is the error 09 corrects, and the GC's mark bits live in `ObjHeader.marks` (which is why
[`09-memory.md`](09-memory.md) sizes the header at 16 bytes). So a handle must be one of the nine
types sharing `0b111`, and the only mechanism the design provides for telling them apart is
`ObjHeader.kind`. That is precisely what
[`00-blueprint-review.md`](00-blueprint-review.md) §A1 says: *"`dict`/`list`/`set`/`range`/`range`/
`handle`/`instance` share machine tag `0b111` and are told apart by `ObjHeader.kind`, a `u8` at
offset 0 of every allocation."*

**The bug the first draft had.** It read "handles remain small integers" (ADR-0004) and wrote

```rust
// WRONG. Do not implement this.
fn from_handle_token(token: u32) -> Value {
    (token as u64) | (TAG_HEAP << 48)      // payload is an integer, not a pointer
}
```

which produces a value whose payload is a number, tagged as a heap object. Every `is_instance`,
every `dict` field load, every GC mark, and the JIT's own descriptor test in
[`10-jit.md`](10-jit.md) §*(`tag == 0b111 && header.kind == INSTANCE`)* all read `*(payload)` as an
`ObjHeader`. On this value that is a dereference of the integer `0x1000_0003` — a segfault on the
first heap operation, in a language whose one hard rule is that a script may not be able to crash
the process. It is also precisely the memory-safety hole ADR-0004 was written to close, reached by
the other door: `typeof` would have been honest while the *representation* was a lie.

**The shape that is correct.** The integer survives, one level down, where nothing can mistake it
for a pointer:

```
  The `Value` a Zen script holds — 8 bytes, the only thing the script can copy
  63                            48 47                                                0
  +--------------------------------+--------------------------------------------------+
  |          tag = 0b111          |        *const ObjHeader  (fits in 48 bits)      |
  +--------------------------------+--------------------------------------------------+

  The object it points at — 24 bytes, allocated by `alloc(OBJ_HANDLE, 8)`
  +0        1        4        8                                                          16      24
  +--------+--------+--------+-----------------------------------------------------------+--------+
  | kind=  | shape  |  rc    |  marks/flags  │                token: u32                | pad    |
  | HANDLE |  = 0  |        |               │   (index: 20 bits ‖ generation: 28 bits)  |        |
  +--------+--------+--------+-----------------------------------------------------------+--------+
   ObjHeader (16 B, ADR-0001 / 09 §2)                        payload (8 B)
```

```rust
// zen-vm/src/handles.rs

/// Payload of an `OBJ_HANDLE` allocation. Deliberately one `u32` + one
/// `Affinity`-shaped field, so the whole object is 24 bytes and lands in a
/// single 32-byte allocator bucket.
#[repr(C)]
pub struct HandleObj {
    pub header: ObjHeader,     // kind = ObjKind::Handle, 16 B
    pub token: u32,            // index: 20 bits ‖ generation: 28 bits
    /// Set at `close` and never cleared. This is what makes `bool(handle)`
    /// a one-load test, and what lets a stale handle produce an *error*
    /// instead of a segfault (§5).
    pub closed: bool,
}

impl Value {
    /// # Safety
    /// `obj` must point at a live `HandleObj` allocation made by
    /// `zen_alloc(ObjKind::Handle, 8)` and not yet freed.
    #[inline]
    pub unsafe fn from_handle(obj: NonNull<ObjHeader>) -> Value {
        // The same 48-bit check ADR-0001 requires of every pointer payload,
        // and the debug assertion that catches a 5-level-paging regression
        // in a test rather than in production.
        debug_assert!(obj.as_ptr() as u64 >> 48 == 0);
        Value::from_bits(0b111u64 << 48 | (obj.as_ptr() as u64 & MASK_48))
    }

    /// `true` for a handle, `false` for every other `0b111` type. One tag
    /// compare plus one byte load, from the same cache line as the payload
    /// pointer — the `is_dict()` cost ADR-0001 measures as negligible.
    #[inline]
    pub fn is_handle(self) -> bool {
        self.tag() == 0b111 && unsafe { self.header().kind } == ObjKind::Handle
    }
}
```

**What this costs, stated honestly.** 24 bytes and one allocation per live handle, and a pointer
chase on every `sys.read(fd, …)`. Against a `read(2)` at 300–500 ns, a chase to a 24-byte object
that was allocated at the same time as the fd and is therefore in cache is not measurable; the
allocation is one bump in the arena, not a `malloc`. The two properties it buys are worth more than
the bytes:

1. **`typeof` is a pure function of the representation** (ADR-0004's requirement, and the reason the
   test at [`tests/cases/15_sqlite.z:20`](../../tests/cases/15_sqlite.z) —
   `check(typeof(h) == "int", ...)` — is rewritten rather than preserved).
2. **A stale handle is a live object, so it can be *asked* what happened.** The `closed` byte turns
   §5's failure mode from undefined behaviour into a message. The generation counter is still
   needed, and §5.2 says why: the object keeps the token alive after `close`, so without the
   generation the token would silently match a *new* resource that inherited the index.

**What "handles remain small integers" means here.** ADR-0004's sentence is about the
*language-level* value — a Zen script treats a handle as an opaque small integer, prints it as
`#3#1`, compares it, and cannot forge one (§7.3) — and it explicitly rejects making it an opaque
struct. `HandleObj` is not an opaque struct: it is one 4-byte field, the token, plus two bytes of
bookkeeping the script cannot observe. The alternative reading — that the `u64` in the script's
hand must itself be the token — is the one §4.1's wrong snippet implements, and it is unsound.
**A reviewer who wants the token to live directly in the `Value` has to argue for a ninth machine
tag first**, which means amending ADR-0001 and moving `bytes` or re-deriving the 48-bit payload
limit; that is a bigger change than it looks, and §12 records it as the one design that would have
to be re-opened.

- **index** — a slot number. 1 048 576 live resources max, which is at the default Linux fd limit
  (`/proc/sys/fs/file-max` defaults to 1 048 576) and above anything a script will reach. Exceeding
  it is a `NativeError`, not an allocation failure.
- **generation** — bumped every time the slot is freed. §5.2.
- **0 is reserved** and means "nil handle"; no allocation ever produces it.


### 4.2 Three candidate containers, evaluated honestly

The blueprint specifies `DashMap<u32, Box<dyn Any + Send>>`. Zen 2.x uses
`static LIBRARIES: OnceLock<Mutex<HashMap<u64, SendLib>>>` (`src/ffi.rs:26-29`) with a global
`AtomicU64` id allocator (`src/ffi.rs:31`) — a single global mutex, ids that are never reused, and
no generation counter. That last point is a live bug: `ffi.close(h)` removes the id and nothing
stops a later `ffi.load` from reissuing the same number to a *different* library, at which point
`ffi.call(h, "strlen", …)` silently calls the new library. `tests/cases/19_sys_ffi.z` never
exercises the reuse, so it passes.

| Criterion | `DashMap<u32, Box<dyn Any + Send>>` | Sharded `parking_lot::RwLock<Slab<Slot>>` | Lock-free `AtomicPtr` free list |
|---|---|---|---|
| Lookup (open → `Slot`) | ~20 ns, sharded by `ahash` on the key | ~15 ns, `Vec` index, no hashing | ~8 ns, one load + one acquire |
| Insert / free-list pop | **not supported** — a separate `Mutex<Vec<u32>>` and a second lock must be bolted on | **free** — `Slab`'s free list is built in and preserves the entry header, which is where the generation lives | Treiber stack; needs epoch reclamation for the *value* |
| Memory per entry | `HashMap` overhead ≈ 33 B (control byte + key + fat pointer + `Box` indirection) plus a heap node | `Slot` is `{ gen: u32, kind: ResKind, value: Box<dyn Any + Send> }` = 24 B inline, exact, no per-entry allocation | 16 B node + the value, plus a hazard-pointer table |
| Type safety on use | `downcast_ref::<T>()` is a vtable call plus a type-id compare, and **`Any` cannot express "this must be a socket"** — §4.4 | same `Box`, same limitation | same |
| Contention | 4×ncpu shards; the hottest shard is 1/(4·ncpu) of the table under one lock | 16 fixed shards, `index & 15`; tunable | none |
| Poisoning | `parking_lot` internally, so no poisoning | `parking_lot` — no poisoning | no lock |
| Deterministic iteration | **no** — `iter()` is a per-shard snapshot and is not a consistent view | **yes** — a `Vec`, so `handles.debug()` walks `0..len` | no |
| `Drop` on close | `remove(&k)` returns the `Box`, so the destructor runs *outside* the shard lock | identical: `slab.remove(idx)` returns `Slot`, the `Box` drops by the caller | the value sits behind a `crossbeam_epoch::Atomic`, so dropping requires `defer_destroy` and the fd closes at an **unbounded later time** |
| Leaked-program behaviour | unbounded unless a free list is added | bounded by 1 048 576 | bounded |
| Extra crates | `dashmap` 6 | `parking_lot` 0.12.5 (already in `zen/Cargo.toml:38`) + `slab` 0.4 | `crossbeam-epoch` 0.9 + `crossbeam-utils` 0.8 |

**The honest reading of this table: on lookup latency, all three are irrelevant.** The 15 ns
between the sharded slab and the atomic free list is 4 % of the 300–500 ns a `read(2)` costs on the
same operation, and an immeasurable fraction of a `bench_core` run. Anyone choosing on the latency
column is optimising a number nobody will ever see. The columns that *decide* are free-list
support, deterministic iteration, and `Drop` timing.

### 4.3 Ruling: sharded `parking_lot::RwLock<Slab<Slot>>`, 16 shards, `dashmap` retired

```rust
// zen-vm/src/handles.rs
#![deny(unsafe_op_in_unsafe_fn)]

use parking_lot::RwLock;
use slab::Slab;
use std::any::Any;

const SHARDS: usize = 16;          // fixed, NOT num_cpus — see below
const SHARD_MASK: u32 = (SHARDS as u32) - 1;
pub const MAX_LIVE: u32 = 1 << 20; // index is 20 bits
pub const INVALID_HANDLE: u32 = 0;

/// Discriminant stored **next to** the value, so a wrong-resource use is a
/// precise error rather than a failed downcast.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum ResKind {
    Fd = 1,          // OwnedFd
    File = 2,        // std::fs::File
    Dir = 3,         // std::fs::Dir
    Socket = 4,      // socket2::Socket
    TlsSocket = 5,   // tokio_rustls::client::TlsStream<TcpStream>
    DnsResolver = 6, // Arc<hickory_resolver::TokioResolver>
    Sqlite = 7,      // rusqlite::Connection
    Process = 8,     // std::process::Child
    Library = 9,     // libloading::Library — a dlopen'd handle
    FnPtr = 10,      // a symbol address, owned but not dereferenceable
    Memory = 11,     // an mmap region
    Timer = 12,      // the M10 monotonic-timer slab
}

struct Slot {
    /// Bumped on every free. §5.
    gen: u32,
    kind: ResKind,
    /// `Box<dyn Any + Send>` because the resource set is open: a plugin can
    /// register its own kind. `Send` because the *token* is a `u32` and
    /// therefore travels to any thread; the *thread-safety of the resource*
    /// is ResKind's problem, documented per kind.
    value: Box<dyn Any + Send>,
}

/// The token natives actually work with. A `Handle` is never what a Zen
/// script holds — §4.1: the script holds a `Value` pointing at a
/// `HandleObj`, and `Value::as_handle()` reads this token back out. Keeping
/// the two types distinct is what stops a native from treating a token as a
/// pointer, which is the bug the first draft of §4.1 made.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle(u32);

impl Handle {
    pub const NIL: Handle = Handle(0);

    #[inline]
    pub fn new(index: u32, gen: u32) -> Handle {
        debug_assert!(index < (1 << 20), "handle index is 20 bits");
        Handle((gen << 20) | index)
    }

    #[inline]
    pub fn split(self) -> (u32, u32) {
        (self.0 & 0xF_FFFF, self.0 >> 20)
    }
}

impl std::fmt::Display for Handle {
    /// `3#1` — slot 3, generation 1. This is the exact text §5.3's error
    /// message interpolates and the exact text a user greps for.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (i, g) = self.split();
        write!(f, "{i}#{g}")
    }
}

pub struct HandleTable {
    shards: [RwLock<Slab<Slot>>; SHARDS],
    /// Live count per shard, for `sys.handle_count()` and the leak test.
    live: [std::sync::atomic::AtomicU32; SHARDS],
    /// §4.5: every `HandleObj` ever created, so a closed handle stays
    /// dereferenceable and can report *why* it is closed. These are arena
    /// allocations with a manual refcount (08 §8.2), not `Box`es, because
    /// they share the allocator with every other `0b111` value.
    objs: Mutex<Vec<NonNull<ObjHeader>>>,
    /// Soft cap on `objs.len()`. Pre-GC this is the only bound on the
    /// identity pool; §4.5 explains why it is not optional.
    obj_count: std::sync::atomic::AtomicU32,
}
```

Allocation uses the hinted shard first (a thread's own resources land in the shard it hashed to),
falls back to a forward scan of at most two shards, and only then takes a global
`Mutex<Vec<(u8, u32)>>` free list drained in batches of 64. That slow path runs once per 64
inserts. It is documented in the module header because *"why is there a free list in two places"*
is the first question a reviewer will ask, and the answer is that the batched path is an
optimisation, not a second source of truth: the free list is refilled from the slabs and the slabs
remain authoritative.

**The `Drop` is the point.**

```rust
impl HandleTable {
    /// Reserve a slot and mint the `Value` a script will hold.
    ///
    /// Two allocations, on purpose: the `Slot` (which is freed at `close`)
    /// and the `HandleObj` (which is not — §4.5). The slot is the resource;
    /// the object is the identity. Conflating them is what makes a stale
    /// handle either a segfault or an alias, and §5.2 explains which.
    pub fn open(&self, kind: ResKind, value: Box<dyn Any + Send>) -> VmResult<Value> {
        let obj = self.new_identity()?;
        let index = /* slab insert, §4.3's shard + free-list rules */;
        let gen = self.shard(index).read().get(index as usize).unwrap().gen;
        let h = Handle::new(index, gen);
        unsafe {
            (*obj.as_ptr()).as_mut().cast::<HandleObj>()
                .token = h.0;
        }
        self.shard(index).write().get_mut(index as usize).unwrap()
            .value = value;
        Ok(unsafe { Value::from_handle(obj) })
    }

    fn new_identity(&self) -> VmResult<NonNull<ObjHeader>> {
        // 24 bytes × 4 MiB = 96 MB of identity objects, which is the cap
        // before the OOM-adjacent behaviour of §4.5 sets in.
        const MAX_IDENTITIES: u32 = 1 << 22;
        let n = self.obj_count.fetch_add(1, Relaxed);
        if n >= MAX_IDENTITIES {
            self.obj_count.fetch_sub(1, Relaxed);
            return Err(VmErr::raise(ErrorKind::NativeError,
                format!("handle identity pool exhausted ({MAX_IDENTITIES} handles created); \
                         this program opens handles in a loop faster than it closes them"),
                Span::none()));
        }
        // 16 B header + 8 B payload, one bump. `rc` starts at 1: the
        // table's own reference, the one that makes a closed handle
        // still dereferenceable.
        let p = unsafe { zen_alloc(ObjKind::Handle, 8) }?;
        self.objs.lock().push(p);
        Ok(p)
    }

    /// Free a slot. Returns the payload; the caller drops it **outside the
    /// lock**, so a slow destructor (a `sqlite3_close` that flushes a WAL)
    /// does not block every handle in that 1/16 of the table.
    ///
    /// The generation is bumped HERE, under the write lock, and nowhere
    /// else. That is the whole of §5. The `HandleObj` is *not* freed here.
    pub fn free(&self, h: Handle) -> VmResult<Slot> {
        let (index, gen) = h.split();
        let mut g = self.shard(index).write();
        let slot = g.get_mut(index as usize).ok_or_else(|| stale(self, h))?;
        if slot.gen != gen { return Err(stale(self, h)); }        // §5
        if matches!(slot.value.downcast_ref::<Closed>(), Some(_)) {
            return Err(already_closed(h));                      // double close
        }
        let taken = std::mem::replace(
            slot.value.downcast_mut::<Closed>().expect("checked above"),
            Box::new(Closed),
        );
        slot.gen = slot.gen.wrapping_add(1);                    // the integer moves on
        // Mark the identity so `bool(h)` and `stale` can say *closed*
        // instead of dereferencing a freed object. The object itself is
        // kept: §4.5.
        self.mark_closed(h);
        Ok(taken)                                                // caller drops: this is the fd close
    }

    /// `bool(handle)` — ADR-0004's ergonomic liveness test, and it is a
    /// single byte load. It cannot be racy: the `closed` byte is written
    /// under the shard's write lock and read after acquiring it, or the
    /// slot is gone and the answer is `false` either way.
    #[inline]
    pub fn is_open(&self, h: Handle) -> bool {
        let (index, gen) = h.split();
        self.shard(index).read()
            .get(index as usize)
            .is_some_and(|s| s.gen == gen
                && !matches!(s.value.downcast_ref::<Closed>(), Some(_)))
    }
}
```

### 4.5 The identity pool: the one thing §5 forces, and the first real need for the M9 GC

`free` releases the **resource** and keeps the **identity**. That asymmetry is not an oversight; it
is the whole reason a stale handle can be reported instead of dereferenced, and it has a cost that
has to be named rather than discovered:

| | Freed at `close`? | Freed at `Vm` drop? | Freed by the M9 collector? |
|---|---|---|---|
| `Slot` (holds the `Box<dyn Any>`: fd, socket, connection) | **yes** | yes (whatever is still open) | yes, via finalizers (§5.4) |
| `HandleObj` (holds the token and `closed`) | **no** | yes | **yes** — this is the first thing the collector reclaims |

So in M7–M8, a script that opens and closes a handle in a loop grows `objs` by 24 bytes per
iteration, forever, because "is any `Value` still pointing at this object?" is **undecidable
without a collector** — that question *is* the collector. Three consequences, all of them
deliberate:

1. **`MAX_IDENTITIES = 1 << 22` is a hard error, not a warning.** At 24 bytes that is 96 MB. Past
   it, `open` returns a `NativeError` naming the pool, rather than letting a `while` loop over
   `net.connect` take the process down. The alternative — an OOM, which
   [`00-blueprint-review.md`](00-blueprint-review.md) §5 makes the one fatal error in the
   language — is worse than a script that has to be written to close what it opens.
2. **The leak is measured, not assumed.** Test 20 in §10 asserts that a script doing 100 000
   open/close cycles ends with `sys.handle_count() == 0` and `sys.identity_count() == 100 000`, so
   the regression is a *test failure with two numbers in it* rather than a slow RSS climb someone
   notices in production.
3. **This is the concrete argument for shipping the M9 collector**, and it is the first one in the
   plan that is not about performance. Before the collector, `sys.handle_count()` is the only number
   that means anything; after it, `identity_count()` becomes a real statistic because the pool
   drains. A reviewer who thinks the GC can be deferred should look at this table and say so
   explicitly.

**Why the identity is not refcounted instead.** `ObjHeader.rc` is right there, and using it would
free the object as soon as the last `Value` goes away — which reintroduces §5.1's exact failure, in
a new costume: the token is freed, the index is reissued, and a stale handle that *survived* (held
in a list, say, by a reference the program legitimately still has) now points at a reissued
identity. The generation counter cannot save it either, because the object holding the generation is
the thing that was freed. A refcount tracks the *script's* references; the safety property needs
tracking of the *table's* history, and only the table knows that.


The `Drop` that closes the fd is the *resource's own* `Drop`, and it is not special-cased:

| `ResKind` | Stored as | `Drop` does |
|---|---|---|
| `Fd` | `std::os::fd::OwnedFd` | `close(2)`, exactly once, guaranteed by `OwnedFd`'s own `Drop` |
| `File` | `std::fs::File` | same, via the inner `OwnedFd` |
| `Dir` | `std::fs::Dir` | the inner `readdir` iterator's `Drop` |
| `Socket` | `socket2::Socket` | `close(2)`; **no `shutdown`** — a half-closed socket is a protocol state, and silently doing it would change observable behaviour |
| `TlsSocket` | `tokio_rustls::client::TlsStream<TcpStream>` | TLS `close_notify`, then the socket |
| `Sqlite` | `rusqlite::Connection` | `sqlite3_close_v2`, which finalises statements and releases the WAL |
| `Library` | `libloading::Library` | `dlclose(2)`. The `Library` is the *only* thing keeping the mapping alive, so the `Box` must be dropped, never `mem::forget`ed |
| `Memory` | `MmapRegion { ptr: NonNull<u8>, len: usize, prot: i32 }` | `munmap(2)`. A plain newtype over `libc::mmap`'s result, because a raw pointer cannot be passed to C without an `as_ptr()` a reviewer has to check |
| `Process` | `std::process::Child` | does **not** `kill()`; a handle going out of scope reaps a zombie, it does not kill a running process. Killing is `os.kill(handle, sig)` |
| `Timer` | `TimerRegion` | the M10 monotonic-timer slab; no OS resource |
| `DnsResolver` | `Arc<hickory_resolver::TokioResolver>` | drops the connection pool |
| `FnPtr` | `FnPtrSlot { addr: *mut c_void, owner: LibraryId }` with no `Drop` | nothing. Holding a `FnPtr` **does not** keep its `Library` alive — but it is *linked* to it, so `dlclose` can invalidate every `FnPtr` derived from that library. §7.1.1 |

`OwnedFd` is the load-bearing choice for `Fd`: it is `Send + Sync`, its `Drop` calls `close(2)`
exactly once, and it makes the double-close question unanswerable at the type level for fds the
runtime creates. The *handle* can be closed twice — that is a `NativeError` from §5 — but the fd
cannot.

**Handles are not garbage-collected in M7–M8, and that is a stated leak, not an oversight.** There
are no finalizers until the M9 collector has finalizer support, and
[`12-concurrency.md`](12-concurrency.md) does not add them. A script that opens 10 000 sockets and
drops every handle leaks 10 000 fds until the `Vm` drops; a script that *closes* them leaks 10 000
24-byte identity objects (§4.5), which is the same class of leak in a different unit. Both are
stated, measured (the leak test in [`17-testing-strategy.md`](17-testing-strategy.md) covers "a
script that opens and closes in a loop has flat RSS" for the fd case, and §10 test 20 covers the
identity case with two asserted numbers), and documented in the language manual. It is also exactly
how CPython behaves with raw fds, and a user who wants determinism calls `close`.

**What the collector has to do, stated as a requirement rather than a hope.** M9 must be able to
reclaim a `HandleObj` whose `Slot` is gone, which means two things the §M9 design has to promise:
finalizer ordering (an identity may be collected only after the last `Value` pointing at it is
unreachable, which is the collector's own job, and must run *after* the slot's payload `Drop` so a
`close_notify` is not interrupted) and identity-table compaction, so `objs` does not become a
`Vec` that grows without bound even after the identities inside it are collected. If M9 cannot do
both, `MAX_IDENTITIES` (§4.5) stays, and the honest label for the language is "handles are not
reclaimed until the program exits".

**Ruling summary, one paragraph.** Sharded `parking_lot::RwLock<Slab<Slot>>`, 16 fixed shards,
batched free list. `dashmap` is removed from `zen/Cargo.toml:39` because the one thing it provides
that `Slab` does not — hash-based lookup — is not needed (the key is an integer index), and the
one thing `Slab` provides that `DashMap` does not — an O(1) free list that preserves the
generation in place — is the entire safety mechanism of §5. A lock-free `AtomicPtr` free list is
rejected because its `Drop` timing is `crossbeam_epoch`-deferred, so `close()` would return before
the fd is actually closed, and a script closing 1000 descriptors in a loop would hit `EMFILE`. A
correct, bounded, ~400-line table beats a 200-line table that occasionally loses file descriptors,
and this is not a hot path.

### 4.4 The `ResKind` tag, and the error it buys

`Box<dyn Any + Send>` alone cannot distinguish a socket from a file, because both are an `i32` on
the wire. Storing `ResKind` in the slot lets the *native* check the kind before the downcast:

```rust
pub fn arg_handle(vm: &Vm, args: &[Value], i: usize, what: &str,
                  want: ResKind) -> VmResult<Handle> {
    let h = match args.get(i) {
        Some(v) if v.is_handle() => v.as_handle(),
        other => return Err(type_error(what, i, want.name(), value_name(other), vm)),
    };
    let slot = vm.handles.peek(h).ok_or_else(|| stale(&vm.handles, h))?;
    if slot.kind != want {
        return Err(VmErr::raise(ErrorKind::TypeError,
            format!("{what}: handle {} is a {}, not a {}",
                    h, slot.kind.name(), want.name()),
            vm.current_span()));
    }
    Ok(h)
}
```

The error text is the product. `socket.read(h)` on a file handle says
`socket.read: handle 41 is a file, not a socket`. The Zen 2.x alternative
(`Value::NativeFunction` + a `HashMap<u64, Box<dyn Any>>`) produces either a successful downcast
into the wrong resource or a bare "no such socket". Test 3 pins the message.

---

## 5. The stale-handle problem

### 5.1 The failure, concretely

```zen
let h = sqlite.open("/tmp/a.db")
sqlite.execute(h, "CREATE TABLE t (x)")     // ok
sqlite.close(h)

// … one million other opens and closes happen …

let h2 = sqlite.open("/tmp/b.db")           // very plausibly slot 3
sqlite.execute(h, "SELECT * FROM t")        // ← h == h2. No error. Wrong answer.
```

Without a generation counter, a stale handle is **not** a dangling pointer — it is worse, because
it does not crash. It names a *live* resource that happens to be a different one, and the script
gets a confidently wrong answer. This is the exact failure mode
[`00-blueprint-review.md`](00-blueprint-review.md) §5 exists to prevent: a wrong answer is
indistinguishable from a right one, and nothing in the traceback says otherwise.

With a generation counter, slot 3's *token* is `0x1_0000_0003` (index 3, gen 1); after the free it
is `0x2_0000_0003` (index 3, gen 2). A stale `Value` still points at a perfectly valid
`HandleObj` — §4.1, the identity is never freed — and that object still holds `0x1_0000_0003`. The
table lookup finds slot 3, sees `gen == 2 ≠ 1`, and returns
`handle 3#1 is closed (slot now at generation 2)`. The script gets a **catchable `NativeError`
naming both generations**, and `tests/cases/19_sys_ffi.z` never has to run to a segfault to prove
the bug is gone.

**Why §4.1's "never free the identity" does not make the generation redundant — the ABA case.**
A refcounted identity would be freed the moment the last `Value` dropped, and the danger would be
gone with it, so it is fair to ask why both mechanisms exist. The answer is the script that keeps a
stale handle *somewhere the runtime cannot see*:

```zen
let saved = [sqlite.open("/tmp/a.db")]   // a list holds the only reference
sqlite.close(saved[0])                   // slot freed; index 3 goes back on the free list
// … one million other opens and closes; slot 3 is now a different, live connection …
sqlite.execute(saved[0], "SELECT 1")     // the identity is STILL alive, because `saved` holds it
```

Here the identity object is alive, its `closed` byte is set, and the index has been reissued. A
`closed`-byte-only design reports "closed" here, which is right. But the object does not know *which*
resource used to live at index 3, and neither does the script's error message. The generation is
what lets the table say **"this was slot 3 at generation 1, and slot 3 is now at generation 2"** —
so the diagnosis distinguishes "you closed it" from "you closed it and the slot has been reused
since", which are different bugs with different fixes. The `closed` byte is what makes the answer
*safe*; the generation is what makes it *useful*.

**The cost of the whole arrangement, in one line:** a stale handle is a normal live object with a
dead token, which is 24 bytes per handle ever created and unbounded until the M9 collector (§4.5).
That trade is deliberate — a crash-free, well-diagnosed error is worth more than 24 bytes that the
collector will reclaim — but it is the trade a reviewer should be asked to approve.

### 5.2 The generation-counter design

The generation is bumped in exactly one place — `HandleTable::free`, §4.3 — under the shard's
write lock, and it is written *before* the payload's destructor runs. The ordering matters, and it
is why the free path has the shape it does:

```
  free(h):
    lock shard
      if slot.gen != h.gen          → Stale          (already freed, gen moved on)
      if slot.value is Closed       → AlreadyClosed  (double close)
      take payload out, put Closed in its place      ← payload is now exclusively ours
      slot.gen = slot.gen.wrapping_add(1)            ← the integer moves on
    unlock shard
    drop(payload)                                    ← fd closes, here, outside the lock
```

Two orderings that are **not** allowed, and why:

| Bad ordering | Failure |
|---|---|
| Bump the generation *after* dropping the payload | A `Drop` that itself allocates a handle (a `sqlite3_close` that flushes and runs a finalizer) would do it with the old generation still live, and a re-entrant lookup would see a live slot that is being torn down |
| Hold the shard write lock across `drop(payload)` | A `sqlite3_close` on a 2 GB database blocks every handle in that 1/16 of the table for the duration of an fsync. That is a self-inflicted denial of service, and a script can trigger it deliberately |

**Generation wraparound, and why the split is 20/28 rather than 24/24.** `gen` is 28 bits and wraps
after 268 435 456 reuses of the *same index*. On wrap, one token in 268 million becomes aliasable
again — the stale `HandleObj` and the new one both read `0x0_0000_0003` — and the §5.1 failure
returns in its original form. The exact exposure: a script must hold a stale handle across 2²⁸
allocations of that same slot index. A 24-bit generation (16 777 216) is reachable by a program that
allocates 16 million handles over its lifetime — a fuzz test, a connection-pool test — and would be
reachable on a Sunday. A 20-bit index caps *live* handles at 1 048 576, which is exactly the default
Linux fd limit, so the index cap is not a real cap on the thing it caps.

**Ruling: index 20 bits, generation 28 bits.** All 32 bits of the token are spent, which is only
possible because the token now lives in its own `u32` field rather than sharing an i48 payload with
four other things — §4.1's correction is what makes this split free. (`ResKind` and `ObjKind` live
in the slot and the header respectively, so neither competes for token bits either way.) A 20/28
split is also the only one where the *wrapping* failure is unreachable in practice: a program would
have to both hold a stale handle and drive one specific index through 2²⁸ reuses, and
`MAX_IDENTITIES` (§4.5) means it can only try 4 194 304 times per process lifetime anyway.

### 5.3 The exact error

```rust
/// A handle whose (index, generation) does not name a live slot, or names a
/// slot that has since been reissued.
///
/// Takes `&HandleTable` because the *live* generation is a property of the
/// table, not of the handle: there is no global to read it from, and a
/// global would be a lie the moment there is more than one `Vm` (which
/// §2.4's re-entrancy rule permits).
#[cold]
pub fn stale(t: &HandleTable, h: Handle) -> VmErr {
    let (index, gen) = h.split();
    let now = t.shard(index).read().get(index as usize).map(|s| s.gen);
    let detail = match now {
        Some(n) if n != gen => format!("closed (slot now at generation {n})"),
        Some(_)           => "already closed".to_string(),
        // The slot is gone entirely, which only happens if the index was
        // removed rather than freed. Reported distinctly because it means
        // the table itself is inconsistent — a bug worth a distinct string.
        None              => "closed (slot released)".to_string(),
    };
    VmErr::raise(ErrorKind::NativeError, format!("handle {h} is {detail}"), Span::none())
}
```

Two details here are load-bearing and easy to get wrong. The live generation is read **from the
shard, under its read lock**, not from a `static` — a `static AtomicU32` would be a global mutable
across `Vm`s, and the message would be able to name a generation that belongs to another thread's
table. And the three `detail` strings are deliberately different words, because they mean different
bugs: a moved-on generation is ordinary use-after-close, an unchanged generation is a double close,
and a released slot is a table bug that a reviewer wants to see in a log.

Rendered, for a script that does `sqlite.execute(h, "…")` after `sqlite.close(h)`:

```
NativeError: sqlite.execute: handle 3#1 is closed (slot now at generation 4)
  at case19.z:41
```

Three properties, all requirements:

1. **`ErrorKind::NativeError`, catchable by `try`/`catch`.** It is a Zen error, so
   [`00-blueprint-review.md`](00-blueprint-review.md) §5.1 is satisfied: no input aborts the
   process.
2. **The message names the index, the generation the script held, and the generation now live.**
   A user who sees `handle 3#1` can grep their program for the place that produced a handle with
   generation 1 — and there is exactly one way to get generation 1 of slot 3, the first allocation
   of that slot.
3. **It is `#[cold]` and the live-generation load is inside the helper.** No cost on any path that
   does not produce a stale handle.

**Double close is a different error**, because it is a different bug:

```
NativeError: sqlite.close: handle 3#1 is already closed
```

Tests 2, 4 and 5 in §10 pin all three messages.

### 5.4 The `Drop` on teardown

The table is a field of `Vm`. `Vm::drop` drains every shard and drops every live `Slot`, so:

- a script that exits with 10 000 open fds closes them, deterministically, at exit;
- a `sqlite` connection is finalised, so a WAL is checkpointed rather than left for recovery;
- a `dlopen`ed library is `dlclose`d, which matters because a suite that loads `libsqlite3` 400
  times otherwise leaks 400 mappings.

Draining is `for s in 0..SHARDS { self.shards[s].get_mut().iter_mut().for_each(drop) }`, with no
lock taken (a `&mut self` means nobody else can hold it). A payload whose `Drop` panics while
another unwind is in flight is a double-panic abort, which is why §9.3 requires native-module
`Drop` impls to be infallible. Test 19 asserts every kind drops exactly once.

### 5.5 What is explicitly *not* in the design

| Rejected | Why |
|---|---|
| `Box::into_raw` the resource and use the pointer as the handle | The integer in a Zen script is then a raw address. A closed resource leaves a dangling integer, a reallocated one aliases (§5.1), and the JIT's `typeof` guard is written against a pointer-shaped payload ([ADR-0004](decisions/ADR-0004-honest-types.md)). The old `ffi.malloc` at `src/ffi.rs:443-451` does exactly this and is the design being replaced |
| Weak references / resurrection | There is no GC to weakly reference, and a resurrectable handle defeats the generation counter |
| Per-`Vm` handle tables | Two `Vm`s (two Zen threads) must be able to name the same fd, or a socket opened on thread A is unusable on thread B. One process-global table, sharded |
| A monotonically increasing id with no reuse | A script that opens and closes 10⁷ files exhausts memory. Zen 2.x's `AtomicU64` id at `src/ffi.rs:31` is this design |
| `Rc`/`Arc` refcount as the generation, or as the liveness answer | A refcount tracks the *script's* references; the safety property needs the *table's* history, and only the table knows it. Freeing the identity when the count hits zero deletes the generation counter, which §5.1 shows is the only thing standing between a stale handle and a wrong answer. The identity is also not an owner: `HandleObj` holds a token and a flag, never the fd, so dropping the last script reference to a handle never closes anything |

---

## 6. `typeof(handle) == "handle"`

**Ruling, from [ADR-0004](decisions/ADR-0004-honest-types.md): `handle` is a language type and
`typeof` returns the string `"handle"`. There is no compatibility exception.**

```zen
let h = sqlite.open(":memory:")
print(typeof(h))            // handle
print(typeof(h) == "int")   // false
print(bool(h))              // true  — live
sqlite.close(h)
print(bool(h))              // false — closed
```

### 6.1 Why, restated for the FFI audience

The ADR's argument is about the JIT. The FFI-specific version of the same argument is about the
type dispatch every native performs first:

```rust
// The one place `ResKind` is reached, and it is a tag compare plus a byte
// load — see §4.1: a handle is a `0b111` value with `kind == OBJ_HANDLE`,
// so this arm already exists in every heap dispatch in the VM.
match value.tag() {
    0b111 => match value.obj_kind() {             // ← ObjKind, at ObjHeader+0
        OBJ_HANDLE => { /* … */ }
        // …
    }
    _ => unreachable!("not a heap value"),
}
```

If `typeof` reports `"int"` for a handle, a script can write `h + 1`, `h * 2`, `d[h]`,
`sorted([3, 1, h])`, and every one of those must be handled by a runtime check at best. With
`typeof(h) == "handle"` the compiler emits *different bytecode* for `h + 1` — an immediate
`TypeError` with a constant message, zero branches on the hot path. The honest type is also the
faster type, once the type is actually used.

### 6.2 `bool(handle)` is the liveness test, and it is the only one

| Expression | Result | Why |
|---|---|---|
| `bool(h)` | `true` if the slot is live and the generation matches, `false` otherwise | The only supported liveness test, because it is the only one that cannot race: the lookup and the generation check happen inside one shard read-lock |
| `h == 0` | **`TypeError: handle and int cannot be compared`** | `h == 0` was the Zen 2.x idiom for "nil handle", and it silently means nothing to a user. `bool(h)` says what they mean |
| `h == h2` | `true` iff the same (index, gen) | Two handles to the same file are two different slots, because a handle names a *resource the runtime opened*, not an open file description. `sys.dup` gives you a second slot; this is documented. The comparison is on the **token** inside the `HandleObj`, which is why it is two `u32` compares and not a pointer compare |
| `int(h)` | `TypeError` | A handle is not a number. `sys.fd(h)` returns the raw `int` for the `ResKind::Fd` kind only |
| `str(h)` | `"handle(3#1)"` | Diagnostic only, and specified and tested so error messages that interpolate a handle are stable |

`bool(handle)` does not need a new opcode in M7. `OP_TRUTHY` is written to delegate for
`tag == 0b111 && kind == OBJ_HANDLE`, which is one compare against a constant in an arm that
already exists. It only needs to become a distinct opcode when the JIT wants to inline it, which is
M11.

`bool(h)` costs a shard **read** lock, not just a byte load, because `closed` is written under the
shard's write lock and the check has to be ordered against it. That is ~15 ns, and it is the price
of the guarantee: there is no interleaving of `close(h)` and `bool(h)` that reports a closed handle
as live. A lock-free `AtomicBool` per identity would be faster and would be wrong, because the
generation check and the `closed` check have to agree atomically with the slot's existence — three
words in three places, and only the lock makes them one fact.

### 6.3 The legacy test that changes

`tests/cases/15_sqlite.z:20` asserts `typeof(h) == "int"`. Per ADR-0004 the **test changes**:

```zen
# tests/cases/15_sqlite.z, M7
let h = sqlite.open(":memory:")
check(typeof(h) == "handle", "sqlite.open returns a handle")
check(bool(h), "a fresh handle is live")
sqlite.close(h)
check(not bool(h), "a closed handle is falsy")
check(typeof(h) == "handle", "typeof is stable after close")
```

`tests/cases/19_sys_ffi.z:49` (`check(fd > 0, "open creat")`) is different: `sys.open` returns an
`Fd` handle, and `fd > 0` is a `TypeError` under the new rules. The rewritten test uses `bool(fd)`
and `sys.fd(fd)`. §8.4 has the full rewrite plan.

### 6.4 Handles in error messages and `bt`

A handle `Display`s as `handle(3#1)`. Every error message that interpolates one uses that form, so
a traceback is unambiguous about *which generation* of *which slot* was used. This is free and it
is the single most useful debugging feature in this document.

---

## 7. `dlopen` and the user FFI escape hatch

### 7.1 What Zen 2.x has, and what is kept

`src/ffi.rs` is 583 lines and exposes 17 natives. The API is genuinely good — a typed dict for
non-default scalar types, hex for buffers, a `ret` descriptor for the return type, and manual
`CString`/`Vec` pinning for the call duration. All of that is kept.

| Zen 2.x native | Line | M7 name | Verdict | Change |
|---|---|---|---|---|
| `ffi.load(path)` | `src/ffi.rs:354` | `ffi.dlopen(path)` | **KEEP**, alias `load` | Returns a `handle` (`ResKind::Library`), not an `int` |
| `ffi.close(h)` | `src/ffi.rs:366` | `ffi.dlclose(h)` | **KEEP**, alias `close` | Renamed because `close` beside `sys.close(fd)` is a trap; the alias keeps `19_sys_ffi.z:132-133` working |
| `ffi.symbol(h, name)` | `src/ffi.rs:382` | `ffi.symbol` | **KEEP** | Returns a `handle` of a new `ResKind::FnPtr`, not a raw address. `ffi.call_at` becomes unnecessary and is **DEPRECATED** |
| `ffi.call(h, name, args, ret)` | `src/ffi.rs:398` | `ffi.call` | **KEEP** | Typed dict args (`{"type": …, "value": …}`) kept verbatim |
| `ffi.call_at(addr, …)` | `src/ffi.rs:427` | — | **DROP** | With `symbol` returning a handle, `ffi.call(sym, args, ret)` covers it. The raw-address form is the one place a Zen integer is a machine-code pointer; deleting it removes the feature's worst footgun |
| `ffi.malloc(n)` | `src/ffi.rs:443` | `ffi.alloc(n)` | **KEEP as `alloc`, DROP as `malloc`** | Returns a `handle` (`ResKind::Memory`), not an address. Alias `malloc` retained. A Zen integer that is a `malloc`'d address is exactly what K7 forbids |
| `ffi.realloc(p, n)` | `src/ffi.rs:453` | `ffi.resize(h, n)` | **KEEP**, alias `realloc` | Takes a handle |
| `ffi.free(p)` | `src/ffi.rs:464` | `ffi.free(h)` | **KEEP** | Double free is a `NativeError` via the generation check |
| `ffi.read(p, n)` | `src/ffi.rs:470` | `ffi.read(h, n) -> bytes` | **KEEP, changed** | Returns `bytes`, not a hex `string`. The hex convention is Zen 2.x's workaround for having no `bytes`; `00-blueprint-review.md` §3 keeps `bytes` as a first-class type precisely so binary protocols are natural |
| `ffi.write(p, hex)` | `src/ffi.rs:477` | `ffi.write(h, b)` | **KEEP, changed** | Takes `bytes`. `ffi.write(h, "hex")` still accepted with a deprecation warning, routed through `hex_decode` |
| `ffi.str(p)` | `src/ffi.rs:490` | `ffi.cstr(h) -> string` | **KEEP**, alias `str` | Renamed; `str` shadows the `string` type name inside a module namespace and is a readability trap |
| `ffi.set(p, b, n)` | `src/ffi.rs:495` | `ffi.fill(h, byte, n)` | **KEEP**, alias `set` | — |
| `ffi.copy(d, s, n)` | `src/ffi.rs:503` | `ffi.copy(d, s, n)` | **KEEP** | Handles on both sides; overlapping *and differing* ranges raise rather than silently `memmove` |
| `ffi.errno()` | `src/ffi.rs:511` | `ffi.errno()` | **KEEP, changed** | §7.6 |
| `ffi.strerror(e)` | `src/ffi.rs:523` | `ffi.strerror` | **KEEP** | — |
| `ffi.sizeof(t)` | `src/ffi.rs:537` | `ffi.sizeof` | **KEEP, table fixed** | The old table maps `"float"` to **8** (`src/ffi.rs:81`); C `float` is 4. This is a live bug |
| `ffi.alignof(t)` | `src/ffi.rs:545` | `ffi.alignof` | **KEEP, table fixed** | Same bug, same table |

The `sizeof("float") == 8` bug is worth calling out: it is the kind of thing a hand-written type
table gets wrong, and it is caught the moment a script uses `ffi.alloc` for a C `float*` and reads
garbage. Test 8 pins the corrected table.

### 7.1.1 The `Library` ↔ `FnPtr` link, without which test 14 cannot exist

A first draft of this section made a promise it could not keep. Test 14 asserts that

```zen
let lib  = ffi.dlopen("libc.so.6")
let sym  = ffi.symbol(lib, "abs")
ffi.dlclose(lib)
ffi.call(sym, [-1], "int")        // must raise, must not jump into munmap'd memory
```

raises `NativeError: symbol handle is closed`. But `HandleTable` §4.3 has no relationship between a
`Library` slot and the `FnPtr` slots derived from it: `free(library)` drops the `Library`, the
mapping goes away, and every `FnPtr` is still "live" as far as the table is concerned. The test
would jump into unmapped memory. **A table that cannot express "this symbol outlived its library"
cannot make a stale-handle guarantee about symbols at all**, so the design has to add the link
rather than drop the test.

**Ruling: every `FnPtr` slot records the index of the `Library` it came from, and `dlclose`
cascades.** The cascade is the whole point — it must happen *before* the `Library` is dropped, and
under the same lock ordering, or there is a window in which a symbol is callable and its code is
not mapped.

```rust
/// What a `ResKind::FnPtr` slot actually holds. `owner` is the *index* of
/// the `Library` slot, not a pointer to it: a `Library` slot can move
/// (a `Slab` insert shifts nothing, but a `Vec`-backed debug view can
/// reallocate) and a stale `&Library` would be a use-after-free in the
/// safety machinery itself.
struct FnPtrSlot {
    addr: *mut c_void,
    owner: u32,                 // index of the owning `Library` slot
    owner_gen: u32,             // its generation: a *reused* index is not our library
}

/// Called from `free` **before** the payload is dropped, holding the
/// owning shard's read lock. Fails closed: if the owning library cannot be
/// positively identified, the symbol is retired anyway.
fn retire_symbols_of(&self, owner: Handle) -> usize {
    let (index, gen) = owner.split();
    let mut n = 0;
    for shard in &self.shards {
        let mut g = shard.write();
        for slot in g.iter_mut() {
            if let Some(f) = slot.value.downcast_mut::<FnPtrSlot>() {
                if f.owner == index && f.owner_gen == gen {
                    f.addr = std::ptr::null_mut();      // retired, not dropped
                    n += 1;
                }
            }
        }
    }
    n
}
```

`ffi.call` on a `FnPtr` whose `addr` is null is a `NativeError` naming the library, not a jump:

```
NativeError: ffi.call: symbol from library 7#2 was invalidated by ffi.dlclose
  at case19.z:138
```

**Why retire rather than free.** Freeing the `FnPtr` slot would make the script's `sym` handle
*stale*, which works but produces the §5.1-generation message — technically correct, diagnostically
useless, because the user did nothing wrong and the message would not say who invalidated it.
Retiring leaves the slot live with a null address, so the error names the *cause*. It also means
`bool(sym)` is still `true` after the `dlclose`, which is a deliberate and documented lie: the slot
exists, the code does not. The alternative — a distinct `ResKind::RetiredSymbol` — buys that
honesty for one more enum variant and one more error string, and this document chooses the cheaper
one, with the trade written down here rather than left for a user to discover.

**The cost:** `dlclose` is O(live handles), not O(1). For a script that opens 10 000 libraries and
each exports 200 symbols, that is 2 000 000 slot visits under sixteen write locks, and the script
will feel it. It is bounded, it happens once per `dlclose`, and `dlclose` is a debugging tool
behind `--allow-ffi` — a trade worth taking. A `HashMap<u32, SmallVec<u32>>` of owner → symbols
would make it O(symbols of that library) at the cost of a second structure to keep consistent with
a table that can be reallocated; if profiling ever shows it matters, that is the change, and the
test that would catch a mistake in it is test 14.

### 7.2 What is unsafe about it, stated precisely

`ffi.call(h, "strlen", ["hello world"], "int")` tells the runtime: build a call frame with one
`char*` argument, and call this address as `long (*)(char*)`. **Nothing checks that the callee's
real signature matches.** Three concrete hazards:

| Hazard | Example | Consequence |
|---|---|---|
| **Wrong arity** | `ffi.call(h, "write", [1], "int")` — one arg to a three-arg function | The callee reads garbage for `buf` and `count`. On x86-64 SysV the second and third integer arguments are in `rsi`/`rdx`; whatever is there gets dereferenced. Fault at best, corruption at worst |
| **Wrong argument width** | `ffi.call(h, "printf", [{"type":"int","value":1}], "void")` where the callee is `printf(const char*, ...)` | The callee treats the integer as a pointer. Immediate fault, usually |
| **Variadic mismatch** | `ffi.call(h, "printf", ["%d\n", 5], "int")` where the CIF is not variadic | The x86-64 SysV variadic ABI requires `%al` to hold the number of vector registers used. `libffi` handles this; a hand-rolled dispatcher must too, or it is a different bug class |

The Zen 2.x code is *careful*: it pins each scalar in a `Box` so the address is stable
(`src/ffi.rs:298-315`), and that is correct. The problem is not the marshalling code. The problem
is that **a declared signature is a lie the script tells the machine, and the machine has no way
to check it.**

This is not a defect in Zen 2.x. It is a defect in the *concept*, present in every FFI, and it is
why the feature is gated.

### 7.3 The gate

```rust
// zen-vm/src/limits.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// `import ffi` without this flag is `FeatureDisabled`.
    pub allow_ffi: bool,
    /// `net.raw` / `net.link` without this flag is `FeatureDisabled`.
    pub allow_raw_sockets: bool,
    /// More than this many live OS threads is a `NativeError`.
    pub max_threads: u32,
    /// More than this many bytes of `mmap` in total is a `NativeError`.
    pub max_mapped_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { allow_ffi: false, allow_raw_sockets: false,
                 max_threads: 4096, max_mapped_bytes: 1 << 34 }
    }
}
```

| Flag | Enables | Without it |
|---|---|---|
| `--allow-ffi` | `import ffi` | `NativeError: the ffi module is disabled; pass --allow-ffi to enable it` |
| `--allow-raw-sockets` | `net.raw`, `net.link`, `sys.socket`/`connect`/`send`/`recv` | `NativeError: raw sockets need CAP_NET_RAW; grant it and pass --allow-raw-sockets` |

**The gate is checked in three places, not one**, and that is the part worth arguing about:

1. **At `import`.** The obvious one. `tests/cases/ffi_disabled.z` asserts the error.
2. **At `zen build`.** A compiled `.zeno` artifact records the limits it was built under in its
   `FileHeader.flags`. Building with `--allow-ffi` writes the bit; a run *without* `--allow-ffi`
   still refuses, because the run-time gate is the only one that matters. Building with the flag
   and shipping the artifact is therefore not a way to smuggle FFI onto someone else's machine.
3. **At every FFI native, not only at import.** A plugin (M8) can register `net.http` in a process
   where `ffi` was never imported. `if !vm.limits.allow_ffi { return Err(gated("dlopen")); }` is
   the first line of all 17 natives. It is one predictable branch on a path that is about to call
   `dlopen`.

**The gate is a flag, not a policy engine.** There is no "trusted script" concept and no allowlist
of libraries, because a script that can `dlopen` can already do everything a native can. A flag is
honest: *this program can call arbitrary C code* is the entire fact, and it is a fact the user
chooses.

### 7.4 The porting cost from `libffi` to `libloading`, and what actually replaces it

**The honest answer to the question as asked: `libloading` does not replace `libffi`, and anyone
who says it does has not written the marshaller.** `libloading` 0.8 is a safe wrapper over
`dlopen`/`dlsym`/`dlclose` and nothing else. Zen 2.x already uses it — `use libloading::Library`
at `src/ffi.rs:14` — and it sits *beside* `libffi`, not instead of it, because the calling
convention is libffi's job.

So the real choice is one of three:

| Option | Effort | What you get | Verdict |
|---|---|---|---|
| **A. Keep `libffi` 3.2** | ~0 | The full C ABI: 400 arguments, variadics, structs by value, long double, every platform glibc/musl/macOS/Windows build it | Rejected. It is a C dependency — the old `Cargo.toml` lists `libffi8` in the deb `depends` — and it is a source build on musl and on MSVC. A scripting language that needs a C library in order to call a C library is a distribution problem |
| **B. Hand-rolled x86-64 SysV + aarch64 AAPCS dispatcher** | ~3 days | A `match` on (arg types, ret type) producing a typed `extern "C"` fn pointer, plus both classification algorithms | **CHOSEN** |
| **C. Drop arbitrary-signature calling, keep `dlopen`+`symbol` and require a fixed declared signature** | ~1 day | Basically B with a smaller table | Superseded by B |

**Ruling: B.** A hand-rolled signature dispatcher, arch-limited, with a hard error outside the
supported set. The reasoning:

1. **Zen 3.0's FFI-supported platforms are Linux x86-64, Linux aarch64, macOS x86-64 and macOS
   arm64.** Four targets, two ABIs, both frozen for 25 and 15 years respectively. This is the one
   part of the stack where a table beats a general library.
2. **The unsupported set is a feature, not a limitation.** A general FFI lets a script express a
   call that is wrong in a way nothing can detect. A narrow one makes the wrong call
   *unrepresentable* — the same move as "no pointer type in the language".
3. **The supported set covers every real use.** `strlen`, `abs`, `getenv`, `write`, `ceil`, `pow`
   — every symbol in `tests/cases/19_sys_ffi.z:91-106` — is at most four scalar arguments with an
   integer or `double` return.

```rust
// zen-ffi/src/dispatch.rs
#![deny(unsafe_op_in_unsafe_fn, clippy::missing_safety_doc)]

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CType { I8, U8, I16, U16, I32, U32, I64, U64, F32, F64, Ptr, Void }

/// The largest argument count we marshal. SysV puts arguments 7+ on the
/// stack; we do not, so a script asking for 8 gets an error rather than a
/// call whose 7th argument is stack garbage.
pub const MAX_ARGS: usize = 6;

pub struct Sig { pub args: ArgSet, pub ret: RetKind }

/// # Safety
/// `addr` must be the address of a function whose real signature is exactly
/// `sig`. We cannot check this. It is the caller's obligation, which is why
/// `ffi.call` sits behind `--allow-ffi`.
unsafe fn call_at(addr: *mut c_void, sig: &Sig) -> VmResult<Ret> {
    dispatch_matrix::dispatch(addr, sig)
}
```

The sharp edge of a hand-rolled dispatcher is that the match arms must be over the **exact arity**,
not `<= MAX_ARGS`, or the callee reads uninitialised registers. Sixteen arity cases times ten
return cases is 160 arms, generated by a macro from the same table the dispatcher validates
against, so the table and the arms cannot drift:

```rust
// zen-ffi/src/matrix.rs — generated; do not hand-edit.
macro_rules! arms {
    ($(($n:expr, $Ret:ident, $asm:ident) => { /* … */ }),* $(,)?) => { … }
}
```

Everything outside the matrix is an error, not a guess:

```
NativeError: ffi.call: unsupported signature (8 args, void return with sret)
  at case19.z:93
  supported: 0..=6 arguments drawn from {i8,u8,i16,u16,i32,u32,i64,u64,f32,f64,ptr},
             returning one of {i8..i64, u8..u64, f32, f64, void, ptr}
```

On an unsupported architecture the module reports itself unavailable rather than pretending:

```rust
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn call_at(_: *mut c_void, _: &Sig) -> VmResult<Ret> {
    Err(VmErr::raise(ErrorKind::NativeError,
        "ffi.call is not available on this architecture (supported: x86_64, aarch64)".into(),
        Span::none()))
}
```

`ffi.dlopen`, `ffi.dlclose`, `ffi.symbol`, `ffi.sizeof`, `ffi.alignof`, `ffi.strerror` and
`ffi.errno` are pure Rust plus `libc` and are available on every target.

**Porting cost, quantified.** Deleted: `use libffi::middle::…` (`src/ffi.rs:13`), the wrong half of
the `size_of`/`align_of` tables, the `Type`/`Cif`/`CodePtr` plumbing, and the `libffi` line in
`Cargo.toml`. Kept: `CallData` (with the `Box<u64>` arenas removed, because the arms transmute to a
fixed parameter list and do not need stable scalar addresses), the typed-dict argument parser
(`src/ffi.rs:150-217` is good and is ported essentially unchanged), the return descriptor parser
(`src/ffi.rs:228-287`), and `read_cstr`. New: `dispatch.rs` (~250 lines including the macro),
`matrix.rs` (~120 lines, generated), and one `#[cfg]` guard. `libffi` leaves `Cargo.toml`;
`libloading` 0.8 stays. The `libffi8` debian dependency disappears and the binary links one fewer
shared library.

**The cost of the narrowing, stated honestly:** a script that calls a 7-argument C function, a
function taking a struct by value, or a variadic function with a `float` argument cannot. Under
`--allow-ffi`, a user who needs that has three options: a `zen-ffi` plugin registering a
purpose-built native, a hand-written `extern "C"` shim compiled into a plugin, or a PR to widen
the table. All three are better than a silent wrong call.

### 7.5 The feature flag, in `Cargo.toml`

```toml
# zen-vm/Cargo.toml
[features]
default = []
# Every FFI native. Off in every build that is not `zen build --with-ffi`,
# and off at run time unless `--allow-ffi` is passed (§7.3).
ffi = ["dep:libloading"]
```

`cargo build` with no features produces a binary where `import ffi` fails with a message that says
the *binary* lacks the feature, distinct from the flag:

```
NativeError: the ffi module was not compiled into this binary (build with --features ffi)
```

The distinction matters for packaging: a distro shipping the FFI build says so in the package
description, exactly as the old `Cargo.toml`'s `depends = [ … "libffi8" ]` did implicitly.

### 7.6 `ffi.errno()` is wrong in a threaded program, and must change

`src/ffi.rs:511-521` reads `*libc::__errno_location()` at call time. `errno` is **thread-local**,
so this is *approximately* right single-threaded and wrong everywhere else:

- In a multithreaded program, `ffi.errno()` reads the calling thread's `errno`, which any
  intervening native may have set. The test at `tests/cases/19_sys_ffi.z:33-34` — `try { sys.close(
  123456789) } catch { }` then `sys.errno() != 0` — passes only because nothing runs in between. It
  is a test that will pass for a week and then fail when `spawn` lands.
- Even single-threaded, the value is destroyed by the *next* libc call, and `sys.errno()` is
  itself a libc call in the general case.

**Ruling.** `errno` is captured **at the point of failure** and stored per-`Vm` (and therefore
per-thread, since each Zen thread owns its `Vm`).

```rust
// zen-vm/src/vm.rs
/// Last errno captured by a failing native, on THIS thread's Vm.
errno_slot: std::cell::Cell<i32>,
```

```rust
let rc = unsafe { libc::close(fd.raw()) };
if rc < 0 {
    // Read errno IMMEDIATELY — before building the message, because
    // `format!` allocates and an allocator may touch errno on some platforms.
    let e = vm.take_errno();
    return Err(vm.errno_error("close", e));
}
```

`sys.errno()` then returns `vm.errno_slot`, not `__errno_location()`. It means "the errno of the
most recent failed `sys` call on this thread", which is useful, well-defined, and testable, and
the existing test keeps passing for the right reason. `errno_error` stores the value *and* puts it
in the message, so `sys.errno()` after a catch is redundant — which is the point: the error message
already has it. Test 20 is the per-thread regression test.

---

## 8. The `sys` module plan

### 8.1 What exists, and what is wrong with it

`src/sys.rs` is 60,319 bytes and registers **94 natives**. It is a thin, mostly-correct wrapper
over `libc` with a uniform `syserr(what)` formatter (`src/sys.rs:53-58`) producing
`sys.open: No such file or directory (errno 2)`, which is exactly the right shape. The module is
the best of the Zen 2.x natives; the work here is mostly *narrowing* and *portability*, not
invention.

Four defects in the current shape that the port must fix:

| # | Defect | Evidence | Fix |
|---|---|---|---|
| S1 | **Raw addresses in the language.** `mmap`, `munmap`, `mprotect`, `mremap`, `mlock`, `munlock` and all eight `peek*`/`poke*` take and return `usize as f64` | `src/sys.rs:629-645`: `sys_mmap` returns `Value::Number(ptr as usize as f64)` | `mmap` returns a `ResKind::Memory` **handle**; `peek`/`poke`/`munmap` take one. §8.3 |
| S2 | **An address as an `f64` loses precision above 2⁵³.** Linux with 5-level paging has a 56-bit user space. Any address ≥ 2⁵³ is silently rounded, and a rounded pointer is a wild pointer. Worse, an address in a `handle` cannot even be represented: i48 tops out at 2⁴⁷, so a 48-bit address is already out of range | `src/sys.rs:640` | Fixed by S1. A handle carries an *index*, never an address, which is the only reason this works at all |
| S3 | **`errno` read lazily** | `src/sys.rs:1291-1294` | §7.6 |
| S4 | **`read_str` returns a `string` from a byte-count read**, so binary data becomes lossy UTF-8 | `tests/cases/19_sys_ffi.z:42` asserts `sys.read_str(p[0], 8) == "hi"` | `sys.read(fd, n) -> bytes`; `sys.read_text(fd, n) -> string`, raising on invalid UTF-8. The test changes to `read_text` for the text case and gains a `bytes` case |

### 8.2 The `sys` surface, function by function

Portability columns: **L** = Linux/glibc only · **P** = POSIX (Linux, macOS, the BSDs) ·
**P\*** = POSIX with a macOS deviation. The 19 functions `tests/cases/19_sys_ffi.z` exercises are
marked ★.

| Zen name | C call | Returns | Port. | Porting plan |
|---|---|---|---|---|
| ★ `sys.platform()` | — | `string` | — | **NEW.** One of `"linux"`, `"macos"`, `"windows"`, `"bsd"`. The first entry in the M7 `sys` surface; nothing else in the module should need a `cfg` |
| ★ `sys.getpid()` | `getpid()` | `int` | P | Trivial |
| ★ `sys.getppid()` | `getppid()` | `int` | P | — |
| ★ `sys.getuid()` | `getuid()` | `int` | P | — |
| ★ `sys.geteuid()` | `geteuid()` | `int` | P | — |
| ★ `sys.getgid()` | `getgid()` | `int` | P | — |
| ★ `sys.getegid()` | `getegid()` | `int` | P | — |
| ★ `sys.gethostname()` | `gethostname(buf, 256)` | `string` | P | `libc::gethostname` with a `MaybeUninit<[u8; 256]>`. `19_sys_ffi.z:24` asserts only `len > 0`, so truncation needs a documented policy: **raise** `NativeError: gethostname: name is longer than 255 bytes` rather than silently truncate |
| ★ `sys.uname()` | `uname(&utsname)` | `dict` | P | `libc::uname` writes fixed-width NUL-padded arrays; every field must go through `CStr::from_bytes_until_nul`, not `to_string_lossy` on the whole struct. Dict shape pinned by `19_sys_ffi.z:23`: `{sysname, nodename, release, version, machine}` |
| ★ `sys.times()` | `times(&tms)` | `dict` | P | `tms` is `clock_t`. The dict carries `{user, system, children_user, children_system}` in **raw ticks** plus `clock_t`, because `SC_CLK_TCK` varies by platform and returning seconds without saying which would be a lie |
| ★ `sys.sysconf(name)` | `sysconf(int)` | `int` | P | `SC_CLK_TCK`, `SC_PAGESIZE`, `SC_NPROCESSORS_ONLN`, `SC_PHYS_PAGES`. `-1` is a legitimate "indeterminate" answer per POSIX, so it is **returned, not raised** |
| ★ `sys.clock_gettime(clk)` | `clock_gettime(clk, &ts)` | `int` (**nanoseconds**) | P | A single `int` of nanoseconds, which is what `19_sys_ffi.z:28-29` asserts (`> 0`). Nanoseconds in an i48 covers ±4.5 years, enough for a monotonic clock |
| `sys.clock_getres(clk)` | `clock_getres` | `int` | P | Nanoseconds. Needed to interpret a coarse clock |
| ★ `sys.nanosleep(secs)` | `nanosleep(&ts, &rem)` | `nil` | P | Zen 2.x takes **seconds as a `float`** (`sys.nanosleep(0.001)` at `19_sys_ffi.z:30`). Keep it; convert to `tv_sec`/`tv_nsec` and surface the remaining time on `EINTR` |
| `sys.sleep(secs)` | `thread::sleep` | `nil` | P | `BlockingClass::MayBlock`. Exists so a script does not reach for `nanosleep` |
| ★ `sys.errno()` | — | `int` | P | §7.6: the captured slot, not `__errno_location()` |
| ★ `sys.strerror(e)` | `strerror` | `string` | P\* | `src/sys.rs:37-49` does this correctly, but the returned buffer is per-thread and may be overwritten by the next call, so the result must be copied into an owned `String` — which the current code already does |
| ★ `sys.close(fd)` | `close(2)` | `nil` | P | Takes a `ResKind::Fd` **handle**. `Drop` also calls this, so a double close through the handle is a generation error, not an `EBADF` |
| ★ `sys.pipe()` | `pipe2(fds, O_CLOEXEC)` | `[handle, handle]` | P | **`O_CLOEXEC` is mandatory** — M10's `spawn` and `subprocess` both `exec`, and a leaked write end in a child is a hang that is very hard to diagnose. `pipe2` is Linux; macOS has it since 10.10, so this is P in practice |
| ★ `sys.open(path, flags, mode)` | `open(2)` | `handle` (`Fd`) | P | Mode defaults to `0o666` when omitted. Never omits `O_CLOEXEC` |
| ★ `sys.read(fd, n)` | `read(2)` | `bytes` | P | **Changed** from `read_str` (S4). A short read is normal and is **not** an error: a read returning fewer than `n` bytes returns those bytes. Only a negative result raises |
| ★ `sys.read_text(fd, n)` | `read(2)` | `string` | P | Raises `NativeError: read_text: invalid UTF-8 in response` on a decode failure. The `19_sys_ffi.z:42,52` calls become this |
| ★ `sys.write(fd, s_or_b)` | `write(2)` | `int` (count) | P | Accepts `string` **or** `bytes`, both zero-copy (§2.4). A short write is returned and **not** retried: looping here would hang forever on a full pipe, and the caller must check, exactly as POSIX requires |
| `sys.pwrite(fd, off, b)` | `pwrite(2)` | `int` | P | — |
| ★ `sys.lseek(fd, off, whence)` | `lseek(2)` | `int` | P | Delegates straight to `lseek(2)` on the slot's fd, so the offset lives in the kernel's open file description and is therefore **shared with every `dup` of that description**, exactly as POSIX specifies. This document's first draft stored the offset in the slot and sold independence as an improvement; it is not an improvement, it is a lie about the OS: an independent position requires `openat(2)` of the same file, and the slot does not always have a path. A handle-based API has no business reinterpreting `dup`. Documented, and test 17 asserts the `19_sys_ffi.z:51-53` cases still pass |
| ★ `sys.fstat(fd)` | `fstat(2)` | `dict` | P | Shape pinned by `19_sys_ffi.z:54-55`: `{dev, ino, mode, nlink, uid, gid, rdev, size, atime, mtime, ctime, blksize, blocks}`. Times as `float` seconds since epoch |
| ★ `sys.stat(path)` | `stat(2)` | `dict` | P | Same shape, so `19_sys_ffi.z:58` (`st2.size == 9`) is unchanged |
| `sys.lstat(path)` | `lstat(2)` | `dict` | P | — |
| ★ `sys.access(path, mode)` | `access(2)` | `bool` | P | `access(2)` uses **real** uid/gid, a known footgun for setuid programs. `faccessat(AT_EACCESS)` is the correct one where available; fall back to `access` on macOS with a documented caveat |
| ★ `sys.unlink(path)` | `unlink(2)` | `bool` | P | Returns `true`; failure raises |
| `sys.rmdir(path)` | `rmdir(2)` | `bool` | P | — |
| `sys.rename(a, b)` | `rename(2)` | `bool` | P | — |
| `sys.chmod(path, mode)` | `chmod(2)` | `bool` | P | — |
| `sys.symlink` / `link` / `readlink` | as named | `bool`/`string` | P | — |
| ★ `sys.fcntl(fd, cmd)` | `fcntl(fd, cmd)` | `int` | P\* | **Varargs.** The right implementation is a per-command argument union; the wrong one is a hardcoded `(i32, i32)`. The `F_*` constants in the old tree are all 4-byte values, so one `c_int` covers the supported set — and the doc must say **only the integer-returning commands are supported**. `F_GETFL`, `F_SETFL`, `F_GETFD`, `F_SETFD`, `F_DUPFD_CLOEXEC` |
| ★ `sys.dup(fd)` | `fcntl(fd, F_DUPFD_CLOEXEC, 0)` | `handle` | P | `F_DUPFD_CLOEXEC` is Linux 2.6.24+; on macOS use `fcntl(fd, F_DUPFD, 0)` then set `FD_CLOEXEC`. **Shares the file position** with the original, per POSIX and per the `lseek` row above. The new handle gets its own slot and its own identity, so §6's `h == h2` is `false` while `sys.fstat` agrees on `dev`/`ino` — two handles, one open file description |
| ★ `sys.dup2(old, new)` | `dup2(2)` | `handle` | P | **DROPPED.** The Zen 2.x test asserts `sys.dup2(dupfd, 55) == 55` (`19_sys_ffi.z:67`), which is a raw-fd API: its whole purpose is to place a *specific fd number*, which the handle ruling forbids. `sys.dup` is enough. The test loses two lines |
| ★ `sys.mmap(len, prot, flags)` | `mmap(2)` | `handle` (`Memory`) | P\* | **Returns a handle** (S1). `MAP_ANONYMOUS` on Linux, `MAP_ANON` on macOS — export both names as the same value per target, so a script does not have to know the platform. `MAP_FAILED` is a `NativeError` |
| `sys.munmap(h, len)` | `munmap(2)` | `bool` | P | Takes the handle. `Drop` also does this, so `munmap` and dropping are interchangeable and a double `munmap` is a generation error |
| `sys.mprotect(h, len, prot)` | `mprotect(2)` | `bool` | P | — |
| `sys.mremap(h, …)` | `mremap(2)` | `bool` | L | `#[cfg(target_os = "linux")]`, and on other targets the native **exists and raises** `NativeError: mremap is Linux-only`. A `cfg`-removed symbol gives a script `NameError`; a message is better |
| `sys.msync(h, …)` | `msync(2)` | `bool` | P | — |
| `sys.mlock` / `munlock` | as named | `bool` | P | Subject to `RLIMIT_MEMLOCK`; the error must name the limit, not just "resource temporarily unavailable" |
| ★ `sys.poke8/16/32/64(h, off, v)` | `write_unaligned` | `nil` | P | **Signature changes** from `(addr, v)` to `(handle, offset, v)` — §8.3 |
| ★ `sys.peek8/16/32/64(h, off)` | `read_unaligned` | `int` | P | — |
| ★ `sys.mem_write(h, off, b)` | `copy_nonoverlapping` | `int` | P | Takes **`bytes`**, not a hex string. `19_sys_ffi.z:82-83` changes accordingly |
| ★ `sys.mem_read(h, off, n)` | slice copy | `bytes` | P | — |
| ★ `sys.syscall(n, …)` | `syscall(2)` | `int` | L | **See §8.4.** `libc::syscall` is varargs and Linux-specific. macOS's `syscall(3)` uses entirely different numbering *and* different argument marshalling; Windows has none |
| `sys.getrlimit` / `setrlimit` | as named | `dict`/`nil` | P\* | `struct rlimit` is `u64` on Linux and `int64` on macOS. Use `libc::rlimit`'s own field types, never a hand-written struct |
| `sys.umask(m)` | `umask(2)` | `int` | P | — |
| `sys.socket` / `bind` / `listen` / `accept` / `connect` / `send` / `recv` / `sendto` / `recvfrom` / `setsockopt` / `getsockopt` / `getsockname` / `getpeername` / `shutdown` / `socketpair` | as named | handle / `int` | P | **KEEP, all `ResKind::Socket` handles.** These are the 16 natives that make `tests/cases/20_sockets.z` work, and that test is 60 lines of hand-packed `sockaddr` hex — the strongest possible argument for a `net.tcp` module ([`13-networking.md`](13-networking.md) §5). The `sys` versions stay, behind `--allow-raw-sockets` for the send/recv half, because a raw `sys.connect` bypasses every check `net.tcp` makes |
| ★ `sys.poll(fds, timeout_ms)` | `poll(2)` | `[dict{fd, revents}]` | P | Shape pinned by `20_sockets.z:24-25`. `sys.pipe` + `sys.poll` is a portable event loop with no threads. Keep it, and cite it as the fallback when `spawn_async` is unavailable ([`12-concurrency.md`](12-concurrency.md) §7) |
| `sys.ioctl(fd, req)` | `ioctl(2)` | `int` | L | Varargs, request-number encoding differs completely per platform, and most interesting `ioctl`s return a pointer. **DROPPED.** It cannot be made safe and it cannot be made portable, and every use case has a better answer: `sys.fcntl` for the `F_*` family, a native for terminal size, a native for interface flags |
| `sys.kill(pid, sig)` | `kill(2)` | `bool` | P | — |
| `sys.fork` | `fork(2)` | `int` | P | **DROPPED.** A `fork()` in a process with open fds, a background JIT thread and a handle table copies a world the child must not touch; the child gets a duplicate of every `Sqlite` and `Socket` handle whose `Drop` will now `close(2)` a descriptor the parent also owns. `os.spawn` over `std::process::Command` is the answer |
| `sys.exec`, `sys.waitpid`, `sys.wexitstatus`, `sys.wifexited`, `sys.wifsignaled`, `sys.wifstopped`, `sys.wtermsig` | as named | `int`/`bool` | P | **DROPPED**, replaced by `os.spawn(path, args)` and `os.wait(h)` over `std::process::Child`. A hand-rolled `waitpid` is a `WNOHANG` polling loop waiting to happen; `Child::wait` handles the pidfd/`waitid` machinery correctly |
| `sys.fdatasync` / `fsync` / `ftruncate` / `truncate` | as named | `bool` | P | — |
| `sys.mkfifo` / `sys.mknod` | as named | `bool` | P\* | `mknod` for a character device needs privileges; `mkfifo` does not. Both kept, neither blocks |
| `sys.__errno_location` | — | — | — | **DROPPED**, private. Replaced by the errno slot of §7.6 |

**The summary: 94 → 68 natives.** 22 dropped, 4 renamed, 1 fundamentally changed (`errno`), 1 added
(`sys.platform`).

### 8.3 The `mmap`/pointer-arithmetic ruling

`tests/cases/19_sys_ffi.z:72-84` is the compatibility target, and it is why this ruling deserves a
paragraph:

```zen
let mem = sys.mmap(4096, PROT_READ + PROT_WRITE, MAP_ANONYMOUS + MAP_PRIVATE, -1, 0)
sys.poke64(mem, 1234)              // (addr, value)
check(sys.peek64(mem) == 1234)
sys.poke16(mem + 2, 0xABCD)        // ← ADDRESS ARITHMETIC IN THE LANGUAGE
check(sys.peek16(mem + 2) == 0xABCD)
sys.mem_write(mem, "68656c6c6f")   // ← hex string
check(sys.mem_read(mem, 5) == "68656c6c6f")
```

`mem + 2` is the fingerprint of a language that exposes addresses. Under the handle ruling:

```zen
let mem = sys.mmap(4096, sys.PROT_READ + sys.PROT_WRITE,
                   sys.MAP_ANONYMOUS + sys.MAP_PRIVATE)
check(typeof(mem) == "handle", "mmap returns a handle")
sys.poke64(mem, 0, 1234)           // (handle, offset, value)
check(sys.peek64(mem, 0) == 1234, "poke/peek64")
sys.poke32(mem, 0, 0x11223344)
check(sys.peek32(mem, 0) == 0x11223344, "poke/peek32")
sys.poke16(mem, 2, 0xABCD)         // ← the offset, not `mem + 2`
check(sys.peek16(mem, 2) == 0xABCD, "poke/peek16 at offset")
sys.mem_write(mem, 0, bytes.from_hex("68656c6c6f"))
check(sys.mem_read(mem, 0, 5) == bytes.from_hex("68656c6c6f"), "mem_read/mem_write")
check(sys.munmap(mem, 4096), "munmap")
```

What this buys, concretely:

- `mem + 2` was an `f64` addition on a value that is a machine address, silently truncated to i48
  on the way into `sys.poke16`. Under a handle the offset is an `int` and `mem + 2` is a
  `TypeError`.
- A `Memory` handle carries `len` and `prot`, so `poke64` bounds-checks the offset against the
  mapping length and raises `IndexError: poke64: offset 4090 + 8 exceeds mapping of 4096` instead
  of writing past the region. **This converts a silent memory-corruption primitive into a
  catchable error**, which is the entire point of
  [`00-blueprint-review.md`](00-blueprint-review.md) §5.
- A `Memory` handle is `Send` — the mapping is process-wide, not thread-local — and is on the §3
  affinity list in [`12-concurrency.md`](12-concurrency.md) as "readable from any thread; two
  threads poking the same mapping is a race the *script* owns, because an `mmap` region is
  explicitly shared mutable memory by construction".

### 8.4 What must change in `tests/cases/19_sys_ffi.z`

The file is 138 lines and is the compatibility target. Four things in it are x86-64-Linux-specific
and must go.

**T1 — `libc.so.6` and `libm.so.6` are glibc sonames** (lines 91, 101). On macOS the equivalents
are `libSystem.dylib`, which contains both libc *and* libm — `ceil`, `floor` and `pow` are all in
it — and there is no separate `libm`. On musl, `libc.so`, not `libc.so.6`.

The fix is a one-line helper at the top of the test, which is the pattern every cross-platform test
in the suite should use:

```zen
# Cross-platform shared-library name. Not a hack: the test's *subject* is
# "dlopen a system library and call a known function", and the name of the
# system library is platform data, not Zen behaviour.
func system_lib() {
    if sys.platform() == "macos" { return "libSystem.dylib" }
    if sys.platform() == "linux" { return "libc.so.6" }   // glibc
    return "libc.so"                                       // musl, BSD
}
let h = ffi.dlopen(system_lib())
```

**T2 — `sys.syscall(39)` is `getpid` on x86-64 Linux only** (line 88). The syscall table is
architecture-specific *and* the test hardcodes the number.

| Arch | `getpid` syscall number | Note |
|---|---|---|
| x86-64 Linux | 39 | What the test assumes |
| aarch64 Linux | 172 | The x32 ABI has its own table again |
| i686 Linux | 20 | |
| macOS x86-64 / arm64 | 20 | BSD numbering, entirely different scheme; `syscall(3)` is a *function*, not a raw syscall |
| Windows | — | Does not exist |

Fix: a platform-keyed constant table, and the raw-`syscall` assertion is skipped where there is no
table, **visibly**:

```zen
if sys.has_raw_syscall() {
    check(sys.syscall(sys.SYS.getpid) == pid1, "raw syscall getpid matches sys.getpid")
} else {
    print("SKIP: raw syscall (not available on " + sys.platform() + ")")
}
```

**T3 — every `fd`-as-int becomes a handle** (lines 49, 65, 66, 67, 72-84).
`check(fd > 0, "open creat")` becomes
`check(typeof(fd) == "handle" and bool(fd), "open creat")`, and `sys.dup2(dupfd, 55) == 55`
(line 67) is deleted with the native.

**T4 — `read_str` becomes `read_text`, hex strings become `bytes`** (lines 42, 52, 82, 83).

**Net: 138 lines → about 155**, and the file gains a `bytes` assertion and a `bool(closed_handle)`
assertion. The compatibility target is the *behaviour*, not the byte count.

### 8.5 The `sys` module split

94 natives in one module is a namespace nobody can navigate, and the portability matrix is the
real structure. **Ruling: three registered modules, one underlying implementation.**

| Module | Contents | Gate |
|---|---|---|
| `sys` | The portable POSIX core: platform, process identity, time, errno, file descriptors, `fstat`/`stat`, `pipe`, `dup`, `mmap` + `peek`/`poke`, `poll` | none |
| `sys.posix` | The Unix-only and unportable remainder: `syscall`, `getrlimit`/`setrlimit`, `umask`, `mkfifo`, `mknod`, and the 16 socket natives | `--allow-raw-sockets` for the socket half |
| `sys.linux` | `mremap`, `eventfd`, `epoll_create`/`epoll_ctl`/`epoll_wait`, `inotify_*`, `memfd_create`, `prctl`, `gettid` | `cfg(target_os = "linux")`; **exists on every platform and raises on the wrong one**, so a script gets a message, not a `NameError` |

The constants split the same way: `sys.O_RDWR`, `sys.PROT_READ`, `sys.SEEK_SET`, `sys.MAP_PRIVATE`
are in `sys`; `sys.MAP_ANONYMOUS` is in `sys` (aliased to `MAP_ANON` on macOS) because a script
that mmaps should not have to know the platform; `sys.SYS` is in `sys.linux`.

---

## 9. Safety rules

### 9.1 The complete `unsafe` allow-list

[`08-vm.md`](08-vm.md) §3.5 establishes a `build.rs` grep that fails the build on any `unsafe`
outside `value.rs` and `gc.rs`, and lists five functions. **M7 extends that list, per crate**, and
the extension is the whole of the FFI `unsafe` surface:

| Crate | File | `unsafe` lines | What it is for | Who reviews it |
|---|---|---|---|---|
| `zen-vm` | `src/value.rs` | 5 | NaN-box punning, `Value::from_ptr`'s 48-bit assert, the one `get_unchecked` | ADR-0001 owner |
| `zen-vm` | `src/gc.rs` | 0 in M4–M8 | M9 only | M9 author |
| `zen-vm` | `src/handles.rs` | 0 | The handle table is safe Rust | — |
| `zen-std-sys` | `src/sys.rs` | ~95 | One `unsafe { libc::… }` per native, and nothing else | OS reviewer |
| `zen-std-sys` | `src/errno.rs` | 1 | `__errno_location` / `__error` | — |
| `zen-ffi` | `src/ffi.rs` | 3 | `Library::new` (it is `unsafe` for good reasons), `Symbol::try_as_raw_ptr`, the transmute to `usize` | FFI owner |
| `zen-ffi` | `src/dispatch.rs` + `src/matrix.rs` | 1 macro body | The typed `transmute` and the 160 generated arms | FFI owner |
| `zen-ffi` | `src/mem.rs` | 4 | `malloc`/`realloc`/`free`/`memcpy` | FFI owner |
| `zen-std-net` | `src/raw.rs` | 2 | `libc::mmap` for the `Memory` handle, `libc::getifaddrs` | Network owner |

**Total new `unsafe`: ~105 lines, of which 95 are one-expression `libc` calls.** The rule that
keeps this auditable:

> **One `unsafe` block per native, containing exactly the FFI call.** No `unsafe` block may span
> more than one statement, and no `unsafe` block may contain an `if` that guards a pointer
> dereference — validation happens in safe Rust *before* the block, and the compiler can see it.

```rust
// The only shape permitted in zen-std-sys/src/sys.rs.
pub fn sys_read(vm: &mut Vm, args: &[Value]) -> VmResult<Value> {
    let h = arg_handle(vm, args, 0, "read", ResKind::Fd)?;                 // safe: validates
    let n = arg_usize(vm, args, 1, "read", 0, MAX_READ)?;                   // safe: range-checks
    let fd = vm.handles.peek_fd(h).ok_or_else(|| stale(&vm.handles, h))?;     // safe
    let mut buf = [0u8; 4096];                                             // safe
    let want = n.min(buf.len());
    let rc = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), want) };      // ← the only unsafe
    if rc < 0 { return Err(vm.errno_error("read", vm.take_errno())); }
    Ok(Value::bytes(&buf[..rc as usize]))                                   // safe
}
```

Enforced by a second `build.rs` rule, in `zen-std-sys`:

```rust
// Fails the build if an `unsafe { … }` block in this crate contains more
// than one statement. Crude, unarguable, and it is the property that matters.
const MAX_STATEMENTS_IN_UNSAFE: usize = 1;
```

**M10 adds two more files** when the network lands — `zen-std-net/src/tls.rs` (a `rustls` config
builder, which is safe Rust; the `unsafe` is inside `aws-lc-rs`/`ring`, not ours) and nothing else.
The count does not grow with features.

### 9.2 `catch_unwind` at the native boundary

[`08-vm.md`](08-vm.md) §4.1 ruling 5 says "Zen exceptions do not use Rust unwinding", and §13
ruling 5 restates it: the alternative is rejected because it makes "did this unwind through a
`Drop` that can panic?" a permanent concern.

**This does not contradict that ruling, and the distinction is the point:**

> Rust unwinding is **not** Zen's error mechanism. Zen's error mechanism is `Err(VmErr)` returned
> from `run`, plus the handler cursor stack. `catch_unwind` at the native boundary is a **net under
> a component we do not control** — the FFI marshaller, `libc` on a platform nobody has tested,
> `rusqlite`'s C code, a `regex` internal. It catches a panic that would otherwise be a process
> abort and converts it to a `NativeError`. It is not part of how a Zen `try`/`catch` works.

```rust
// zen-vm/src/native.rs
/// Call a native, converting an escaped Rust panic into a `NativeError`.
///
/// # Safety (of the AssertUnwindSafe)
/// The `AssertUnwindSafe` is sound here because, on unwind, the only state
/// that survives is `vm`'s **register file and handler stack**, and a panic
/// in a native has by definition not left them inconsistent: the native is a
/// leaf frame, it pushed no `Frame`, and `vm.frames.len()` on exit equals its
/// value on entry. Anything the native mutated in `vm` before panicking — a
/// cached constant, a lazily-initialised module flag — is a *cache* by
/// contract and is documented as rebuildable. The `Drop` hazard is real and
/// is handled by §9.3's rule that native `Drop` impls are infallible.
#[inline]
pub fn call_native_catching(vm: &mut Vm, f: NativeFn, args: &[Value]) -> VmResult<Value> {
    match std::panic::catch_unwind(AssertUnwindSafe(|| f(vm, args))) {
        Ok(r) => r,
        Err(payload) => Err(VmErr::raise(ErrorKind::NativeError,
            format!("internal error in native: {}", panic_message(&payload)),
            Span::none())),
    }
}
```

| Requirement | Why | Status |
|---|---|---|
| `panic = "unwind"` in the release profile | `catch_unwind` is a no-op under `panic = "abort"` | **`zen/Cargo.toml:57` already sets `panic = "unwind"`**, commented *"NOT abort — see docs/01-principles.md §4"*. M7 inherits it. This is the concrete reason that comment exists |
| `AssertUnwindSafe` argued in a `// # Safety` block | The lint does not cover this, but the code must be argued anyway | Above |
| A panic hook that prints to **stderr**, not stdout | The hook runs on the panicking thread and stdout is the program's output | [`08-vm.md`](08-vm.md) §4.7 already specifies this |
| Exit code 101 for an escaped panic, distinct from 1 for a Zen error | So a test can tell "the script errored" from "the VM has a bug" | [`08-vm.md`](08-vm.md) §4.7 |
| **Cost** | `catch_unwind` with no panic is **zero runtime cost** — a landing pad, not a call. It does inhibit one optimisation: the frame cannot be a leaf | **[T]** `bench_native_call` shows ≤ 3 % regression, and the gate is *no measurable regression* on `bench_core` |

### 9.2.1 What may be true of `vm` after a native panics — the part the `# Safety` block cannot assert

The `AssertUnwindSafe` argument above has a hole, and it is worth naming rather than papering over.
"It mutated only caches" is an *aspiration about every future native*, not an invariant the type
system or the call site checks. A native that has already done

```rust
pub fn register_kind(vm: &mut Vm, args: &[Value]) -> VmResult<Value> {
    vm.call_depth += 1;                    // fine
    vm.frames.truncate(vm.frames.len() - 3);   // not fine
    panic!("bug in register_kind");
}
```

leaves a `Vm` whose frame stack is three entries short of where the unwinder expects, and
`catch_unwind` hands that back to the dispatch loop as if nothing happened. The `NativeError` is
correct; the *state* is not. A later unwind, a `bt`, or a `max_depth` check then misbehaves, and
the symptom appears in code that has nothing to do with the native that failed.

**Ruling: the native boundary is transactional over the structural part of `Vm`, and the
transaction is rolled back on unwind.** The cost is four words on the stack and four `debug_assert`s
on the exit path — nothing on a call that does not panic — and the benefit is that "a native panic
leaves the VM in a defined state" becomes a checked property instead of a review convention.

```rust
// zen-vm/src/native.rs

/// The part of `Vm` that the unwinder depends on and that a native is
/// therefore not allowed to change destructively. Everything *not* in here
/// is scratch: module-init flags, arenas, the handle table, the native's own
/// allocations. Those are either rebuilt or harmless, and §9.3's contract
/// says a native that panics has not published anything.
#[derive(Clone, Copy)]
struct Checkpoint {
    frames: usize,
    handlers: usize,        // the handler cursor stack
    call_depth: u32,
    const_pool_len: u32,    // a native must not grow the pool mid-call
}

impl Vm {
    #[inline]
    fn checkpoint(&self) -> Checkpoint {
        Checkpoint { frames: self.frames.len(), handlers: self.handlers.len(),
                     call_depth: self.call_depth, const_pool_len: self.pool.len() as u32 }
    }

    /// Truncate, never rebuild. A native may legitimately *grow* a stack
    /// (§2.5's re-entrancy rule); on the failure path the excess is simply
    /// removed, because there is no correct way to resume what the native
    /// was doing.
    #[cold]
    fn rollback(&mut self, c: Checkpoint) {
        debug_assert!(self.pool.len() as u32 == c.const_pool_len,
            "a native grew the constant pool and then panicked: the pool is not \
             transactionally rolled back, so the rollback is incomplete. \
             See docs/11-ffi-handles.md 9.2.1 — this is a native bug, and the \
             assertion is the contract, not a heuristic");
        self.frames.truncate(c.frames);
        self.handlers.truncate(c.handlers);
        self.call_depth = c.call_depth;
    }
}

#[inline]
pub fn call_native_catching(vm: &mut Vm, f: NativeFn, args: &[Value]) -> VmResult<Value> {
    let cp = vm.checkpoint();
    match std::panic::catch_unwind(AssertUnwindSafe(|| f(vm, args))) {
        Ok(r) => r,
        Err(payload) => {
            vm.rollback(cp);      // the line the first draft was missing
            Err(VmErr::raise(ErrorKind::NativeError,
                format!("internal error in native: {}", panic_message(&payload)),
                Span::none()))
        }
    }
}
```

| What the native did | What `rollback` does | Correct? |
|---|---|---|
| nothing structural | no-op | yes — the common case, and the `#[cold]` path is not entered |
| grew the frame stack for a re-entrant call (§2.5) | the extra frames are truncated | yes: a re-entrant call that panicked has no correct resumption point, and truncating restores the caller's frame exactly |
| pushed a handler for a `try` and then panicked | handlers truncated to the checkpoint | yes: the handler belonged to the failed native, and leaving it would make the *next* Zen error jump into a dead native's cleanup |
| grew the constant pool and then panicked | `debug_assert!` fires; in release the pool keeps the extra constants | **partially** — the constants are unreachable garbage, not corruption, because the pool is append-only and nothing indexes it positionally. Stated as a known gap rather than papered over |

**The gap, honestly.** The last row is a real one. A `debug_assert!` catches it in CI, a release
build leaks a few `Value`s per panicking native, and a script that panics inside natives in a tight
loop could grow the pool without bound. It is accepted because (a) reaching it requires a native
that both mutates the pool and panics, which §9.3's no-panic contract already forbids — so it is a
*bug in a native*, never a bug in a script; and (b) the alternative, a transactional pool, means a
native cannot intern a string at all, which is a real feature to give up. A reviewer who wants this
closed should ask for a per-call pool snapshot, and should also say what that costs §2.2's arity
check on the hot path.

**What this means for §2.5's re-entrancy flag.** A re-entrant native that panics leaves
`in_native_reentry` set, because the flag is cleared by the guard's `Drop` — which does run during
unwind. That is correct: the outer native is still inside its own call, and a second re-entrant
call from the error path must not be allowed. The next `debug_assert!(!vm.in_native_reentry)` is
therefore *not* a second failure; it is the same one, reported where it becomes visible. This is
the only place in the document where a `catch_unwind` interacts with a `Drop` that runs *during* the
unwind, and it is safe only because that `Drop` is infallible and idempotent (§9.3).

**What is NOT wrapped:** the VM dispatch loop, the compiler, the parser, the `Drop` of the handle
table. Those are *ours*; a panic there is a bug we want to abort on in CI, not paper over. Only
the boundary is wrapped.

**And the honest caveat:** `catch_unwind` does **not** make the FFI safe. A wrong signature in the
dispatcher (§7.4) is undefined behaviour at the machine level — it corrupts registers or the stack
— and no Rust facility catches that. `catch_unwind` catches a *cooperative* failure. The signature
table in `matrix.rs` is the actual safety mechanism, and `--allow-ffi` is the honest label on it.

### 9.3 "Natives must not panic" — the contract

A native that panics is caught by §9.2 and becomes a `NativeError`, which is a *recoverable* error
for a bug that should have been a compile-time rejection. The point of the contract is to make the
panic a test failure, not to make it graceful.

| Panic site | Why it is a bug | What to write instead |
|---|---|---|
| `.unwrap()` on a slot lookup | The generation check in §5.2 already proved it is `Some`; if it is not, the table is corrupt | `ok_or_else(stale)?`; and if the proof is genuinely total, `debug_assert!` plus a `#[cold]` fallback |
| `.unwrap()` on an `OsString` conversion | A path with an interior NUL | `TypeError: sys.open: path contains a NUL byte`, matching the existing check at `src/ffi.rs:162` |
| `as usize` on a negative length | `sys.poke64(h, -1, v)` | `arg_usize(vm, args, i, what, MIN, MAX)` range-checks; `IndexError: poke64: offset must be >= 0` |
| i48 overflow in an index computation | [`08-vm.md`](08-vm.md) §4.3 is explicit that register math cannot overflow; handle math is the same class | `checked_add` → `NativeError: handle index overflow` |
| `Vec::with_capacity(n)` with a script-supplied `n` | A 2⁶⁴ allocation is an abort, not a panic — the OOM carve-out in §4.6 | `try_reserve` → a clean `OutOfMemory`-flavoured message |
| `RefCell::borrow_mut()` double-borrow | A native that re-enters | The `in_native_reentry` flag of §2.5: a `debug_assert`, and a `NativeError` in release |
| A `Drop` that panics | **Unwinding out of a `Drop` during another unwind aborts the process.** §9.2's `catch_unwind` is *above* the drop and cannot help | **Natives' `Drop` impls are infallible.** `rusqlite::Connection::close` returns a `Result` and is explicitly discarded; `Library`'s drop cannot fail; `OwnedFd`'s cannot. A `Drop` that needs to report cleanup is a design error — see `Process` in §4.3, which deliberately does not `kill()` |
| `slice::from_raw_parts` with a `len` from C | C returned a short buffer and the script asked for more | Bounds-check against the returned length first, and raise `NativeError: ffi.call: returned 3 bytes, "len" said 4096` — the path at `src/ffi.rs:337-345` trusts `len` completely |
| `NonNull::new(ptr).unwrap()` after a C call | `mmap` returned `MAP_FAILED` | `NonNull::new` is an `Option`; the `MAP_FAILED` check is `#[cold]` |

**The CI assertion.** `clippy::panic`, `clippy::unwrap_used`, `clippy::expect_used` and
`clippy::indexing_slicing` are already denied crate-wide in `zen-vm`
([`08-vm.md`](08-vm.md) §3.5). **M7 extends the same four lints to `zen-std-sys` and `zen-ffi`**,
with exactly one exception: `matrix.rs` gets a `#[allow(clippy::too_many_lines)]`, which is not a
safety lint. No `unwrap` exemption anywhere.

### 9.4 The `blocking` class, again, because it is a safety property

§2.1's `BlockingClass` is not a profiling hint. Three consumers:

1. **`spawn_async` at M10** refuses a `NeverBlocks` native inside a task (it is pointless) and a
   `MayBlock` one is documented as holding that task's thread
   ([`12-concurrency.md`](12-concurrency.md) §7).
2. **The deadlock watchdog** distinguishes "this thread is in a native" from "this thread is
   computing" ([`12-concurrency.md`](12-concurrency.md) §10.3), so a stall report names the native.
3. **The `never_blocks` audit**: `zen test --native-audit` runs each `NeverBlocks` native once
   under a 50 ms watchdog and reclassifies any that trips it. Opt-in, nightly in CI, because it
   triples the suite's wall time.

---

## 10. Test plan

23 entries. Three are CI build gates rather than runnable tests, which is stated so nobody counts
them as coverage. `M7` = lands in the FFI milestone; the file column is the new tree's
`tests/cases/`, with the ported `19_sys_ffi.z` as the compatibility target.

| # | Test | File | What it proves | Gate |
|---|---|---|---|---|
| 1 | `ffi_native_arity_exact` | `ffi_abi.z` | `sys.close()` with 0 args raises `TypeError: sys.close expects 1 argument, got 0`, and the VM did **not** call the native | must not crash |
| 2 | `ffi_double_close` | `ffi_handles.z` | `sys.close(h)` twice: the second raises `NativeError: sys.close: handle 3#1 is already closed`, and the fd is closed exactly once — checked by opening 5000 fds after 5000 double-closes and getting 5000 distinct numbers | exit 0, exact message |
| 3 | `ffi_wrong_resource_kind` | `ffi_handles.z` | `socket.read(file_handle)` raises `TypeError: socket.read: handle 41 is a file, not a socket` — the `ResKind` message of §4.4, not a bare `TypeError` | exact message |
| 4 | `ffi_stale_handle` ★ | `ffi_handles.z` | `h = open(); close(h);` then force 100 000 more open/close cycles so slot reuse is certain, then `sys.read(h, 1)` raises `NativeError: sys.read: handle 3#1 is closed (slot now at generation 100001)`. **The generation in the message is asserted**, which is what makes this a test rather than a smoke check | exact message |
| 5 | `ffi_stale_handle_does_not_alias` | `ffi_handles.z` | The critical negative: after the reuse cycle, `sys.fstat(stale)` **raises** and does not equal `sys.fstat(fresh)`. A test that only checked "it did not crash" would pass on the old design | must raise |
| 6 | `ffi_generation_layout` | unit (`ffi_handles.rs`) | `Handle::split`/`join` field widths (20/28), `INVALID_HANDLE == 0`, and that `gen` is masked to 28 bits on join — the one place a bit-shift bug would be silent | unit |
| 7 | `ffi_mmap_is_a_handle` | `ffi_handles.z` | `typeof(sys.mmap(…)) == "handle"`; `sys.poke64(mem, 4090, 1)` raises `IndexError: poke64: offset 4090 + 8 exceeds mapping of 4096` rather than corrupting the page; `mem + 2` is a `TypeError` | must raise |
| 8 | `ffi_sizeof_table` | `ffi_abi.z` | The full `sizeof`/`alignof` table including the **corrected** `"float" == 4` that the old tree gets wrong at `src/ffi.rs:81` | exact values |
| 9 | `ffi_dispatch_matrix` | unit (`ffi_dispatch.rs`) | For every `(arity, ret)` pair in `matrix.rs`: the generated arm's `fn` type has exactly that parameter count and the right ABI class, and `size_of::<FnType>() == size_of::<RetAbi>()`. Catches a macro that generated the wrong arm | 160 cases |
| 10 | `ffi_dispatch_rejects_unsupported` | `ffi_abi.z` | An 8-argument call raises `NativeError: ffi.call: unsupported signature (8 args, …)` and the function is **not** called — verified with a test-only `extern "C" fn bump()` in the test binary that increments a counter | must not call |
| 11 | `ffi_native_panic_becomes_error` ★ | `ffi_panic.z` | A test-only native `zen_test::panic_now` that does `panic!("boom")`. The script catches `NativeError: internal error in native: boom` and **keeps executing**: a second native call, a `bt`, a `try`/`catch` and a `raise` all behave normally afterwards, which is §9.2.1's rollback rather than luck. The process does not abort, exit code 0, stderr carries the panic-hook line. A companion native `zen_test::panic_after_push_frame` grows the frame stack and *then* panics, and the test asserts `vm.frames.len()` is back at the checkpoint — the row in §9.2.1's table that is easy to get wrong | exit 0, frames restored |
| 12 | `ffi_reentrancy_flag` | `ffi_abi.z` | A test-only native `zen_test::reenter` that calls back into a Zen function which calls another native. The `in_native_reentry` `debug_assert` fires in a debug build; in release the script completes with the same answer | debug build must trip the assert |
| 13 | `ffi_dlopen_cycle` ★ | `ffi_dlopen.z` | 500 `dlopen`/`dlclose` cycles of the system libc, then an RSS check: the process's mapped-object count returns to its starting value. Proves `Library`'s `Drop` runs and is not `mem::forget`ed | RSS delta < 1 MB |
| 14 | `ffi_dlclose_closes_use` * | `ffi_dlopen.z` | `lib = dlopen(libc); sym = symbol(lib, "abs"); dlclose(lib); ffi.call(sym, [-1], "int")` raises `NativeError: ffi.call: symbol from library 7#2 was invalidated by ffi.dlclose` — **without** crashing. Unmapping live code would turn this into a jump into `munmap`ed memory, which no unwinder can catch, so §7.1.1's `Library` -> `FnPtr` cascade is the only thing standing between the two. Asserts the exact message, `bool(sym) == true` still (the deliberate §7.1.1 lie), and that a *second* library reusing index 7 does not retire an unrelated symbol — the `owner_gen` field's only job | exact message, must not crash |
| 15 | `ffi_typed_dict_args` | `ffi_dlopen.z` | The `19_sys_ffi.z:103-109` cases ported verbatim: `{"type":"double","value":1.2}` → `ceil` → `2`; `{"type":"int","value":-999}` → `abs` → `999`; `write(1, {"buf": …}, 2)` → `2`. The typed-dict API is unchanged from `src/ffi.rs:150-217` | exit 0 |
| 16 | `ffi_gate_without_flag` | `ffi_disabled.z` | `import ffi` without `--allow-ffi` raises `FeatureDisabled`; **and** a plugin-registered `ffi.dlopen` call is also refused, proving the gate is at the native and not only at the import | both raise |
| 17 | `sys_posix_surface` ★ | `19_sys_ffi.z` | The ported old file: 60+ assertions covering pid/uid/uname/times/sysconf/clock_gettime/nanosleep/errno/pipe/open/write/read/lseek/fstat/stat/access/unlink/fcntl/dup/mmap/peek/poke/syscall. `syscall(39)` → `syscall(sys.SYS.getpid)`; `libc.so.6` → `system_lib()`; `read_str` → `read_text`; hex → `bytes`; `dup2` deleted | exit 0 |
| 18 | `sys_cross_platform` ★ | `19_sys_ffi.z` | The same file on `linux-gnu` (x86-64 and aarch64), `macos` (x86-64 and arm64) and `linux-musl` in CI. `sys.has_raw_syscall()` gates the one arch-specific assertion and prints `SKIP:` when unavailable. **`MAP_ANONYMOUS` and `MAP_ANON` both resolve on both platforms** | green on 5 targets |
| 19 | `ffi_drop_table_all_kinds` | unit (`ffi_handles.rs`) | Insert one `Slot` of every `ResKind` in §4.3, drop the `Vm`, assert every resource's `Drop` ran exactly once (a `DropCounter` in the test's `Drop` impls), **and** that `objs` — the identity pool of §4.5 — is empty afterwards. Proves §5.4 and the "freed at `Vm` drop" column of §4.5's table | 12 drops, 0 double-drops, 0 identities |
| 20 | `errno_is_per_thread` | `ffi_errno.z` | Thread A does a failing `sys.open`; thread B does a failing `sys.open`; each then asserts `sys.errno()` matches *its own* error and not the other's. **Fails on the Zen 2.x `__errno_location()` design** and is the regression test for §7.6 | both correct |
| 21 | `ffi_identity_pool_accounting` * | `ffi_handles.z` | §4.5's leak, as two numbers instead of a slow RSS climb. A script opens and closes a handle 100 000 times, then asserts `sys.handle_count() == 0` (nothing leaked as an **fd**) and `sys.identity_count() == 100 000` (the identities are all still there, exactly as specified). The second number is the regression detector: if a future change starts freeing identities early, this test says so, and if it never frees them, §4.5's `MAX_IDENTITIES` error is what a user eventually sees | both exact counts |
| 22 | `native_no_panic_audit` | CI | `cargo clippy -D warnings` on `zen-std-sys` and `zen-ffi` with the four denied lints of §9.3, plus the `build.rs` one-statement rule. A `panic!`/`unwrap`/`expect`/unchecked index anywhere in those crates **fails the build** | build gate |
| 23 | `no_unsafe_outside_allowlist` | CI | The §9.1 `build.rs` grep across all five crates. 105 lines expected; a count outside `[100, 130]` fails, so a deletion *and* an addition are both visible | build gate |

**Suite shape.** 20 Zen-level and unit tests, 3 CI build gates. Only test 18 needs a CI matrix, and
it needs one because the old file hardcodes a glibc soname and a syscall number — a portability bug
that would otherwise have shipped and then failed on a contributor's laptop.

---

## 11. Milestone sequencing

The blueprint's build order ([`00-blueprint-review.md`](00-blueprint-review.md) §A6) puts
"FFI, handles, `sys`/`os`/`fs`/`time`" at **M7** and "Concurrency + 7-layer net" at **M10**. That
split is right, and the reason is not "FFI is easier".

### 11.1 M7 — FFI, handles, `sys`/`os`/`fs`/`time`

**Why these four and nothing else at M7.** They have no cross-thread and no async surface, so they
can be written against a single-threaded `Vm` and the §2 ABI with no interaction with the M10
concurrency model. They are also the modules that let the *rest* of the language be tested at all:
`fs` is how the test suite creates fixtures, `time` is how the benchmarks measure, and `sys` is how
`19_sys_ffi.z` becomes a portable conformance test instead of a Linux-only one.

| Deliverable | Contents | Depends on |
|---|---|---|
| M7.1 | `zen-vm::native` — `NativeFn`/`NativeFnV2`, `NativeEntry` with `BlockingClass`, `arg_str`/`arg_bytes`/`arg_handle`/`arg_int`, the arity check, `call_native_catching` | [`08-vm.md`](08-vm.md) §8 |
| M7.2 | `zen-vm::module` — `Module`, `zen_register_module`, `install`, the 6-step `import` resolution, the `NativeId`-keyed `func` value, the migration off `Value::NativeFunction(String)` | §3 |
| M7.3 | `zen-vm::handles` — the sharded slab, generations, `ResKind`, the teardown drain, `sys.handle_count()` | §4, §5 |
| M7.4 | `zen-std-sys` — 68 natives across `sys`/`sys.posix`/`sys.linux`, the errno slot, the constants tables | §8 |
| M7.5 | `zen-std-os` + `zen-std-fs` + `zen-std-time` — the friendly layers over `sys`. `fs` gets `ResKind::File` and `ResKind::Dir` handles, because `open`/`close`/`read`/`write` on a *path* is the ergonomic layer and `sys` is the fd layer | M7.4 |
| M7.6 | `zen-ffi` — the 17 natives, the hand-rolled dispatcher, the `--allow-ffi` gate, the `matrix.rs` unit test | M7.1, M7.3 |
| M7.7 | Port `19_sys_ffi.z`; delete `sys.dup2`, `sys.ioctl`, `sys.fork`, `sys.exec`, `sys.waitpid` and the five `sys_w*` predicates | §8.2 |
| M7.8 | Port `15_sqlite.z`'s `typeof` assertion to `"handle"`; port `20_sockets.z` | M7.3 |

**M7 exit criterion:** `19_sys_ffi.z` passes on all five CI targets, the 22 entries in §10 are
green or are build gates, and `git grep -c unsafe` across the five crates returns the §9.1 counts.

**`sqlite` is M7-adjacent and deliberately small.** `tests/cases/15_sqlite.z` is one file, and
`rusqlite` 0.40 with the `bundled` feature compiles SQLite from C — which the M7 "no C dependencies"
argument would otherwise forbid. **Ruling: `rusqlite` with `bundled` is the single permitted C
dependency**, justified by the fact that shipping SQLite is precisely what the `bundled` feature
exists for. It is 2 MB of C that vendors cleanly, versus `libffi`, which is a C dependency *for a
feature most scripts never use*. The `unsafe` inside `rusqlite` is its own; ours is one line.

### 11.2 M10 — the parts of this document that need concurrency

| Item | Milestone | Why not M7 |
|---|---|---|
| `ResKind::Socket` / `TlsSocket` / `DnsResolver` | **M10** | They are `tokio` types. A socket is `Send + Sync`, and its thread-safety is part of the concurrency model ([`12-concurrency.md`](12-concurrency.md) §3) |
| The 16 raw socket natives | **M10** | Same, plus the `--allow-raw-sockets` gate's interaction with `net.raw` ([`13-networking.md`](13-networking.md) §3) |
| `sys.poll` | **M7** | A blocking `poll(2)`, single-threaded, and the fallback event loop |
| `ResKind::Process` / `os.spawn` | **M7** | `std::process::Command` is synchronous; the *async* form is M13 |
| `ResKind::Timer` | **M10** | The monotonic-timer slab is a concurrency structure |
| `BlockingClass` on `NativeEntry` | **M7** (declared) / **M10** (consumed) | It must exist in M7 so M7's natives are classified; `spawn_async` and the watchdog are its M10 consumers |
| `zen build` recording `Limits` in the artifact header | **M8** | A `zenn` feature, in the same spirit as [`08-vm.md`](08-vm.md) §10.3's "the REPL is not a special mode" |
| The `dlopen` cycle test (13) and the `Memory` leak check | **M7** / **M8** | The RSS assertions need the leak harness from [`17-testing-strategy.md`](17-testing-strategy.md) |

### 11.3 What is explicitly not in either

| Deferred | To | Because |
|---|---|---|
| Handle finalizers (a handle dropped by GC closes its resource) | **M9+** | Needs finalizer support in the tracing collector ([`09-memory.md`](09-memory.md)). A handle in M7 is a plain integer, so a handle surviving its `Vm` is impossible and a handle outliving its script is a documented leak |
| A `handle` type exposed to plugins with new `ResKind`s beyond the twelve above | **M12** | `Box<dyn Any + Send>` already permits it mechanically; what is missing is the resource table in the manual, and a table nobody wrote is a table nobody reads |
| A `--asan`-equivalent in the FFI gate | **not planned** | The dispatcher's unit test (9) and the `[100, 130]` `unsafe`-count gate (22) are the actual controls. An AddressSanitizer build would be a CI matrix on top of a feature that is off by default — a poor trade |
| Calling C++ functions, or C functions that need a struct by value | **not planned** | Both are outside the matrix by construction (§7.4) |
| `ffi.call` from a `.zeno` artifact built without `--allow-ffi` | **never** | §7.3, step 2. The build-time bit is recorded; the run-time flag is the one that decides |

---

## 12. Rulings a reviewer should double-check

| # | Ruling | Where | The alternative view |
|---|---|---|---|
| 1 | **The handle table is a sharded `Slab`, not a `DashMap`**, and `dashmap` leaves the workspace | §4.3 | The blueprint says `DashMap`. It is a perfectly good structure; it just cannot give an O(1) free list that preserves the generation, which is the entire §5 safety mechanism. If a reviewer prefers `DashMap` plus a `Mutex<Vec<u32>>`, that is defensible — but the second structure must exist either way, and then the decision is between one lock and two |
| 2 | **Index 20 bits, generation 28 bits** (1 048 576 live handles, 268 M reuse generations) | §5.2 | 24/24 is the intuitive split. 20/28 is the split the wraparound analysis produces, because 24 generations is reachable by a fuzz test and 28 is not. A reviewer who thinks 24/24 is safe should say what script reaches 2²⁴ reuses of one slot |
| 3 | **A stale handle is a catchable `NativeError`, not UB** — i.e. the whole §5 design exists | §5 | The alternative is to accept aliasing on reuse, which is what Zen 2.x does (`src/ffi.rs:31`). That is a wrong-answer bug, which is worse than a crash and is exactly what §5 of the blueprint forbids |
| 4 | **A `Drop` that holds no shard lock, with the generation bumped before the drop** | §4.3, §5.2 | Bumping after the drop is simpler to write and breaks under a re-entrant `Drop`; holding the lock across the drop is simpler still and lets a script block 1/16 of the table on a deliberate `fsync` |
| 5 | **Replace `libffi` with a hand-rolled, arch-limited signature dispatcher** | §7.4 | `libffi` 3.2 works today and handles every C ABI. The case against it is a C dependency and a source build on musl; the case for the replacement is that it makes an unsupported call *representable as an error*. **This is the ruling most likely to be wrong**, and it is cheap to reverse: keeping `libffi` behind the same `--allow-ffi` gate costs one dependency |
| 6 | **`libloading` is not the replacement for `libffi`** | §7.4 | The question as usually posed assumes it is. It is a `dlopen` wrapper and nothing more; Zen 2.x already uses it beside `libffi` at `src/ffi.rs:14` |
| 7 | **`sizeof("float")` is 4, not 8** | §7.1 | This is not a judgement call; the old table at `src/ffi.rs:81` is wrong and the fix is a bug fix |
| 8 | **`h == 0` is a `TypeError`; `bool(h)` is the liveness test** | §6.2 | The alternative is allowing integer comparison on handles, which keeps the `h == 0` idiom alive. The cost of that is that `h + 1` stays legal and the JIT keeps a guarded path it does not need. ADR-0004 already forces the first half of this decision; the second half follows from it |
| 9 | **`catch_unwind` at the native boundary, deliberately and narrowly** | §9.2 | A reviewer may read this as contradicting `08-vm.md` §4.1 ruling 5. It does not: that ruling is about Zen's error *mechanism*, and this is a net under third-party code. The gate is that `catch_unwind` must never appear in the dispatch loop, the compiler, or the parser |
| 10 | **`errno` is captured at the failure point, not read on demand** | §7.6 | The Zen 2.x design is approximately right single-threaded and wrong everywhere else. This is a straight bug fix, and test 20 is the regression test |
| 11 | **Handles are not GC'd in M7–M8; a leaked handle is a leaked fd** | §4.3 | The alternative is finalizers, which need M9. A language that documents "call `close`" is not broken; a language that claims finalizers it does not have is |
| 12 | **`rusqlite` with `bundled` is the one permitted C dependency** | §11.1 | A purist reading of "no C dependencies" would drop `sqlite` and the existing test with it. The bundled build vendors cleanly and is the reason the feature exists upstream; excluding it would delete a module users have |
| 13 | **A handle is a heap object (`ObjKind::Handle`), not an integer in a `0b111` tag** | §4.1 | ADR-0004 says "handles remain small integers", and read literally that means the `u64` *is* the token. It cannot: ADR-0001 spends all eight tags, so a handle must share `0b111` with the other heap types, and a `0b111` payload that is an integer is a segfault waiting for the first field access. The token is preserved one level down, in a 24-byte object. A reviewer who wants the token in the `Value` has to argue for a ninth machine tag, which means amending ADR-0001 |
| 14 | **The identity object outlives `close`, at 24 bytes per handle ever created** | §4.5, §5.1 | Freeing the identity on `close` would reclaim that memory and is the obvious optimisation. It also deletes the `closed` byte, so a stale handle becomes a dangling pointer and §5's catchable error becomes a segfault. The cost is an unbounded pool with a 4 Mi-identity cap and an honest `NativeError` when it is hit. This is the first place the M9 collector is *required* rather than merely useful, and a reviewer who wants to defer M9 should say so here |
| 15 | **The native boundary rolls back the structural part of `Vm` on unwind** | §9.2.1 | The alternative is to argue in prose that natives only touch caches, which was the first draft's position and is not checkable. Four words and a `#[cold]` truncation turn it into an invariant. The known gap — a native that grows the constant pool and then panics — is stated with its cost rather than hidden |
| 16 | **`dlclose` retires every `FnPtr` derived from that library** | §7.1.1 | Without the link, test 14's promise is unkeepable: a symbol handle stays "live" after its code is unmapped, and the next `ffi.call` jumps into `munmap`ed memory. The alternative — making a `FnPtr` refcount its `Library` — is better and was rejected for one reason: it is invisible in the source, so `dlclose` would stop being able to invalidate anything, which is the bug. Cost: `dlclose` is O(live handles) |



