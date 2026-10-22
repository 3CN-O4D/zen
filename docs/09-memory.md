# 09 — Memory: objects, allocation, and the tracing-GC roadmap

> **Status:** Draft for review. The rulings in §2.1 (the header is a prefix, not an `Arc`),
> §2.4 (ADR-0001's tags beat 08-vm.md §8.2's illustrative ones), §3.4 (`Value` is `Copy` and
> therefore leaks by default), §4.6 (`Arc::make_mut` versus a manual `rc`), §5.1 (string `+` is
> quadratic until M11) and §6.4 (a cargo feature is the selector, because a runtime selector
> would cost a `Value` bit) are the ones to attack first.
> **Milestone:** M2 (`Value` + NaN-box) for §2–§4, §8–§10. **M9** for §6.4 and §7. The collector
> in §7 is specified now so that M4 is not designed in a way that forecloses it, and so that the
> M9 decision is a build rather than a re-design.
> **Recorded as:** [`decisions/ADR-0002-memory-model.md`](decisions/ADR-0002-memory-model.md).
> **Tag budget:** [`decisions/ADR-0001-tag-budget.md`](decisions/ADR-0001-tag-budget.md) is
> canonical and this document does not change it. §2.4 records where
> [`08-vm.md`](08-vm.md) §8.2's illustrative `ValueRepr` disagrees with it.

### How to read the numbers in this document

| Marker | Meaning |
|---|---|
| **[M]** | **Measured**, by a command named in the same sentence. Reproducible today. |
| **[T]** | **Target.** A gate checked in CI; failing it is a release blocker. Not yet achieved. |
| **[E]** | **Estimate**, reasoned from first principles or from a comparable system (CPython 3.13, V8, LuaJIT, Rust's own `String`). Used to size a structure. |

**There are zero [M] performance numbers in this document.** The new tree has no allocator yet,
so every performance statement about allocation is a **[T]** or an **[E]**. The numbers that
*are* facts are the layout constants of §2.1, which are `const`-asserted, and the Zen 2.x line
references in §4.1, which are readable in the old tree today.

---

## Contents

1. [Purpose and the current ruling](#1-purpose-and-the-current-ruling)
2. [The heap object layout](#2-the-heap-object-layout)
3. [Every heap type's Rust representation](#3-every-heap-types-rust-representation)
4. [Copy-on-write semantics in detail](#4-copy-on-write-semantics-in-detail)
5. [String concatenation and the quadratic build loop](#5-string-concatenation-and-the-quadratic-build-loop)
6. [The Gc trait](#6-the-gc-trait)
7. [The Milestone-9 tracing collector](#7-the-milestone-9-tracing-collector)
8. [Memory safety rules and the unsafe allow-list](#8-memory-safety-rules-and-the-unsafe-allow-list)
9. [Test plan](#9-test-plan)
10. [Metrics](#10-metrics)
11. [Rulings a reviewer should double-check](#11-rulings-a-reviewer-should-double-check)

---

## 1. Purpose and the current ruling

### 1.1 What is decided

[`00-blueprint-review.md`](00-blueprint-review.md) §A2 rejected the blueprint's bump arenas and
mark-and-sweep collector for three independent reasons, and
[`decisions/ADR-0002-memory-model.md`](decisions/ADR-0002-memory-model.md) recorded the replacement:

> **Phase 1 (M2–M8): reference counting with copy-on-write containers. Phase 2 (M9): a two-space
> copying collector behind a `Gc` trait, implemented only if the M4/M5 profile shows refcount
> traffic on the critical path. A complete root map is a precondition, not a follow-up.**

This document is the design of both phases. It has three jobs.

1. **Specify the object layout and the ownership discipline** for the `Arc` + CoW collector that
   M2 actually ships, in enough detail that the `unsafe` surface is a countable list rather than
   a reviewer's memory.
2. **Specify the tracing collector completely enough that M9 is a build, not a re-design.**
   Every decision §7 needs is made there, including the ones a collector author would otherwise
   defer and then discover were load-bearing.
3. **Name the costs.** §5 identifies a case — `s + "x"` in a loop — where this ruling is *knowingly*
   worse than CPython, states it as a likely gate failure rather than discovering it in CI, and
   gives the fix design.

### 1.2 What is not decided here

- **Whether to build the collector at all.** That is the M9 gate, and it opens only on profile
  evidence. This document does not pre-authorise M9; it makes M9 *possible* without touching M4.
- **The exact nursery size.** §7.4 gives a starting value and the tuning knob, not a constant.
- **Whether `string` changes representation.** §3 ships `Arc<str>`; §5.5 says what must change to
  make the M11 in-place append possible, and that the change is gated.

### 1.3 How this plugs into the `Vm`

The collector is reached through a free function in `value.rs`, not through a field, and the
dispatch loop does not change shape:

```rust
// zen-vm/src/value.rs

/// The collector for this thread. In M2–M8 this returns a `&'static
/// RefCountGc` and every call is monomorphised and inlined; in M9 it returns
/// a `&'static TracingGc` and the allocation-failure path can run a
/// collection. Same signature, same call site, different type (§6.4).
///
/// This is a function and not a `Vm` field on purpose: `Vm` is `!Send` and
/// per-thread (08-vm.md §2.5), and a thread-local would put a TLS access on
/// the allocation path. A `static` of a zero-sized collector is a single
/// address, and `TracingGc`'s state is in a separate `static Mutex`-free
/// structure that is documented in §7.6.
#[inline(always)]
pub fn gc() -> &'static ActiveGc { &ACTIVE }
```

The other `Vm` fields that §7.2 classifies as roots — `registers`, `frames`, `handlers`, `protos`,
`globals`, `modules`, `strings`, `names`, `closures`, `exc_payload`, `spill`, `native_args` —
already exist in [`08-vm.md`](08-vm.md) §2.1 and are not modified by this document. That is the
test of "the design plugs in": **§7.2 is a table of fields that already exist**, and §8.3 makes a
new field without a table row a build failure.

---

## 2. The heap object layout

### 2.1 The header, and its exact offsets

Every Zen heap allocation begins with a 16-byte header. This is [`08-vm.md`](08-vm.md) §8.2's
`ObjHeader`, with its three spare padding bytes named so that the collector has somewhere to live.

```rust
// zen-vm/src/value.rs

/// 16 bytes, at offset 0 of every allocation whose `Value` tag is `0b111`
/// (heap), and at offset 0 of the payload of a `string`/`bytes` if §5.5's
/// amendment is adopted.
///
/// `align(8)` is a deliberate refinement of 08-vm.md §8.2's `#[repr(C)]`.
/// With `repr(C)` alone this struct's alignment is 4, so a payload member at
/// offset 16 is 8-aligned only if the *allocator* happened to return an
/// 8-aligned block. `align(8)` makes payload alignment a property of the
/// type rather than a property of libc. It costs 0 bytes: every payload
/// field we have needs 8-byte alignment anyway.
#[repr(C, align(8))]
pub struct ObjHeader {
    /// offset 0. Discriminates the nine language types that share `Value` tag
    /// `0b111` (ADR-0001: 8 machine tags, 16 language types).
    pub kind: u8,

    /// offset 1. M9 only; always 0 in M2–M8. The GC's per-object bits.
    /// See §7.3 for the assignment. Occupies what was padding.
    pub marks: u8,

    /// offset 2..4. Object flags. Bit assignments below.
    pub flags: u16,

    /// offset 4..8. Shape id for shaped objects; `0` = `Unshaped`
    /// (08-vm.md §7.1). Free for other kinds.
    pub shape: u32,

    /// offset 8..12. Strong reference count. See §4.6 — this is a **manual**
    /// count, not `Arc`'s, and that is a deviation from ADR-0002's literal
    /// text that §4.6 asks a reviewer to accept.
    pub rc: u32,

    /// offset 12..16. Total size of the allocation in bytes, header included.
    /// For `--mem-limit` (08-vm.md §4.6) and the M9 collector's copy loop.
    pub size: u32,
}

const _: () = {
    assert!(std::mem::size_of::<ObjHeader>() == 16);
    assert!(std::mem::align_of::<ObjHeader>() == 8);
    // Byte offsets, asserted at compile time so a field reorder cannot pass
    // review. These are the numbers §7.5's forwarding-pointer rule and the
    // JIT's constant-folded field offsets depend on.
    assert!(std::mem::offset_of!(ObjHeader, kind)  == 0);
    assert!(std::mem::offset_of!(ObjHeader, marks) == 1);
    assert!(std::mem::offset_of!(ObjHeader, flags) == 2);
    assert!(std::mem::offset_of!(ObjHeader, shape) == 4);
    assert!(std::mem::offset_of!(ObjHeader, rc)    == 8);
    assert!(std::mem::offset_of!(ObjHeader, size)  == 12);
};
```

The offset table, which is the artifact other documents cite:

| Offset | Size | Field | Written by | Read by |
|---|---|---|---|---|
| `+0` | 1 | `kind` | allocation | every `typeof` / `is_*` on a tag-`0b111` value |
| `+1` | 1 | `marks` | M9 collector only | M9 collector only |
| `+2` | 2 | `flags` | allocation; `CLOSE` mutation | `ITER_DONE`, `CHANNEL_RECV`, finaliser scan |
| `+4` | 4 | `shape` | `SET_FIELD` transition | the inline-cache guard (08-vm.md §6.1) |
| `+8` | 4 | `rc` | `retain` / `release` / `cow_mut` | `cow_mut` (the CoW test) |
| `+12` | 4 | `size` | allocation | `--mem-limit`, the M9 copy loop |
| `+16` | … | payload | kind-specific | kind-specific |

**Three bytes of GC state cost zero bytes.** `kind: u8` followed by `shape: u32` leaves three
padding bytes in 08-vm.md's layout, and `marks` and `flags` occupy two of them. This is the
concrete sense in which that document's comment — "the GC's word must not require reallocating
every live object" — is true: an M9 upgrade adds no allocation, no copy, and no size change to
any live object, and the third padding byte stays in reserve for a `range` or a generational mark.

### 2.2 The flags byte

```rust
pub mod flags {
    /// Bit 0. A channel or mutex is closed. Checked by the `ITER_DONE` /
    /// `CHANNEL_RECV` arms, which is why it is a flag and not a `Value` field:
    /// it must be a single bit test, not a second pointer chase.
    pub const CLOSED:        u16 = 1 << 0;
    /// Bit 1. An `error` object's traceback has been captured. Set once, at
    /// the `THROW` site (08-vm.md §5.4 rule 4), read by `zen traceback`.
    pub const HAS_TRACEBACK: u16 = 1 << 1;
    /// Bit 2. A `channel` has at least one blocked sender. A hint for the
    /// M10 scheduler; never load-bearing for correctness.
    pub const HAS_SENDERS:   u16 = 1 << 2;
    /// Bit 3. An `instance` is frozen. Rejects `SET_FIELD` with a catchable
    /// error rather than a shape transition.
    pub const FROZEN:        u16 = 1 << 3;
    /// Bits 4..16 reserved, must be 0. `debug_assert!` in the constructor.
}
```

### 2.3 Why the header is separate from the value tag

This is not belt-and-braces. The tag and the header answer different questions and live in
different places.

| | `Value` tag | `ObjHeader` |
|---|---|---|
| Lives in | the `Value` — a register, a stack slot, a `Vec` element | the heap object |
| Question it answers | *which dispatch arm?* | *what is this object's layout, and who owns it?* |
| Cost to read | 0 — it is in the word we already loaded | 1 load, same cache line as the first payload slot |
| Capacity | 3 bits, 8 values | `u8` + `u16` + `u32`, effectively unbounded |
| Copied when a `Value` is copied | yes | **no** |

Three consequences follow, and each is load-bearing.

1. **Ownership is per-object, not per-binding.** `d2 = d` copies a `Value` — one 8-byte `u64`
   move, and *no* change to `rc`. A GC mark bit in the tag would be cleared by that copy, which is
   precisely the bug class §7's collector is built to avoid. This is why the mark bit is a byte in
   the object and never a bit in the `Value`.
2. **The type test is two-level and the fast level is free.** `is_string()` is a tag compare with
   **zero loads**. `is_dict()` is a tag compare plus one load of `kind`, and that load hits the
   same cache line as the dictionary's first slot — 08-vm.md §8.2's closing observation. A
   single 4-bit tag scheme would make `is_string()` free too, but it would have to spend a tag on
   each of the nine remaining heap types, and ADR-0001's growth argument ("8 spare tags for v2")
   would be spent before the language shipped.
3. **The collector can find a header without trusting the tag.** A semispace collector must decide
   how many bytes to copy and which words are interior `Value`s. Tag `0b111` is the only tag that
   can name a variable-layout object, so "is this a Zen heap object?" is answerable without
   consulting the header — which matters in §7.5, where the collector's own forwarding words sit
   in a register that no longer holds a valid tag.

### 2.4 The `Value` tag assignment, and a discrepancy to resolve

Per [ADR-0001](decisions/ADR-0001-tag-budget.md), **canonical**, the assignment is:

| Tag | Type | Payload | Has an `ObjHeader`? | Where `rc` lives | Where `len` lives |
|---|---|---|---|---|---|
| `0b000` | `nil` | — | no | — | — |
| `0b001` | `false` | — | no | — | — |
| `0b010` | `true` | — | no | — | — |
| `0b011` | `int` | i48, sign-extended; **overflow promotes to `float`**, it does not wrap | no | — | — |
| `0b100` | `float` | raw `f64` bits | no | — | — |
| `0b101` | `string` | `*const u8`, thin, to the first byte | **no** — bare `Arc<str>` | `ptr − 24` | `ptr − 8` |
| `0b110` | `bytes` | `*const u8`, thin, to the first byte | **no** — bare `Arc<[u8]>` | `ptr − 24` | `ptr − 8` |
| `0b111` | the other nine types | `*const ObjHeader` | **yes**, 16 B at offset 0 | `+8` | `header.size` |

The `int` row is the one with consequences for everything else in the project, and it is stated
here because it is a property of the *payload* rather than of the allocator:
[ADR-0001](decisions/ADR-0001-tag-budget.md) says "`Int` is i48, range ±140,737,488,355,327.
Overflow promotes to `float` rather than wrapping", and [`08-vm.md`](08-vm.md) §4.3 gives the
mechanism ("`wrapping_add` on the masked payload, then an explicit overflow test that promotes to
`float`"). **An `int` operation can therefore change a value's type**, which is why `Value` is not
"8 bytes plus a discriminant that never moves" and why [`10-jit.md`](10-jit.md) §6.2 is a real
section rather than a footnote.

> **Discrepancy for a reviewer to resolve, recorded rather than papered over.** The illustrative
> `ValueRepr` enum in [`08-vm.md`](08-vm.md) §8.2 assigns these tags differently: it puts `F64` at
> `0b000`, `Int` at `0b001`, `Str` at `0b011`, `Obj` at `0b100`, `Bytes` at `0b101` and
> `Reserved` at `0b110`. That contradicts [ADR-0001](decisions/ADR-0001-tag-budget.md) on six of
> the eight tags, and it spends tag `0b110` on a "reserved, M9 GC mark bits live here" slot that
> ADR-0001 assigns to `bytes`. It is **also internally inconsistent with itself**: `08-vm.md`
> §8.2 describes tag `0b100` as "heap object — discriminated by `ObjHeader.kind`" *and* tag
> `0b111` as "any heap object", which is two tags for one category and means the enum has nine
> variants on eight tags. That is the strongest evidence that the enum is illustrative: a
> definition would not have that defect.
>
> **Ruling adopted here: ADR-0001 wins.** It is the decision record; the blueprint's amendment A1
> defines it; and `08-vm.md` presents its enum as an illustration ("The struct form is only for the
> `Debug` impl and the `unsafe` accessors"), not as the definition. Three consequences:
> (a) **the GC's mark bits live in `ObjHeader.marks` and never in the `Value` tag**, because there
> is no tag to spend — tag `0b110` is `bytes`; (b) `08-vm.md` §8.2's `ValueRepr` tag comments
> should be corrected the next time that document is edited, and the duplicate heap-object tag
> removed; and (c) if `04-value-and-types.md` is written
> before this is settled, **that document should carry the corrected table** and this section should
> link to it rather than duplicate it.

### 2.5 Allocation

```rust
// zen-vm/src/value.rs

/// Build a heap object: one allocation, `ObjHeader` at offset 0, payload
/// after it. `payload_size` excludes the header.
///
/// # Safety
/// The caller must initialise a payload of exactly `payload_size` bytes and
/// of alignment `<= 8` before the returned `Value` becomes reachable.
#[inline]
pub unsafe fn alloc(kind: Kind, payload_size: usize) -> Result<NonNull<ObjHeader>, ZenError> {
    // Every heap object is at least 16 (header) + 8. The 8-byte floor is not
    // cosmetic: §7.5's forwarding pointer is written into the first payload
    // word, so a zero-length payload would have nowhere to put it.
    let size = 16 + payload_size.max(8);
    let p = gc().alloc(kind, size)?;
    let h = p.as_ptr();
    (*h).kind  = kind as u8;
    (*h).marks = 0;
    (*h).flags = 0;
    (*h).shape = 0;            // Unshaped until the first SET_FIELD
    (*h).rc    = 1;            // the caller's reference
    (*h).size  = size as u32;
    debug_assert!(check_invariants(p));
    Ok(p)
}
```

A fixed-size payload is a normal Rust struct with `ObjHeader` as field 0, so its constructor calls
`alloc` and writes the fields after it; Rust's own allocator does the rest. A variable-length
payload is built with `Layout::extend`, which puts `n * size_of::<Value>()` bytes at exactly `+16`:

```rust
/// `tuple` has no sized Rust representation, so its payload is a slice.
pub fn new_tuple(items: &[Value]) -> Value {
    let n = items.len();
    let total = Layout::new::<ObjHeader>()
        .extend(Layout::array::<Value>(n).expect("n is bounded by 2^32 elements"))
        .0
        .pad_to_align()
        .size();

    let p = unsafe { alloc(Kind::Tuple, total - 16) }.expect("08-vm.md §4.6: OOM is fatal");
    unsafe {
        let h = p.as_ptr();
        // The payload begins at +16 and holds exactly `n` `Value`s, and
        // they are *moved*, not cloned: the caller gives us ownership.
        let slots = h.cast::<u8>().add(16) as *mut Value;
        std::ptr::copy_nonoverlapping(items.as_ptr(), slots, n);
        Value::from_header(p)
    }
}
```

Every `unsafe` in this section is listed in §8.1. There are four lines, and each is `Layout`, a raw
pointer write into memory we just allocated, or a `from_raw`-equivalent.

---

## 3. Every heap type's Rust representation

### 3.1 The table

`P` is the `Value` payload (48 bits, ADR-0001). "Prefixed" means `ObjHeader` is field 0 of a
sized Rust struct in a single allocation.

| Zen type | Rust type | `Value` payload `P` | Header | Allocation | Clone cost | Drop path |
|---|---|---|---|---|---|---|
| `string` | `Arc<str>` | `*const u8` → first byte | no | `Arc` | 1 atomic inc | `Arc::from_raw` |
| `bytes` | `Arc<[u8]>` | `*const u8` → first byte | no | `Arc` | 1 atomic inc | `Arc::from_raw` |
| `list` | `ListObj { hdr, inner: Vec<Value> }` | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | `release` each element, then free |
| `dict` | `DictObj { hdr, inner: IndexMap<Value, Value> }` | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | `release` keys *and* values, then free |
| `set` | `SetObj { hdr, inner: IndexSet<Value> }` | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | `release` each element, then free |
| `tuple` | header + `[Value; n]` | `*const ObjHeader` | prefix | 1 | 1 atomic inc | `release` each element, then free |
| `func` | `ClosureObj { hdr, proto: u32, upvalues: Box<[Arc<Upvalue>]> }` | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | drops the upvalue `Arc`s |
| `instance` | `InstObj { hdr, slots: Box<[Value]> }` | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | `release` each slot |
| `error` | `ErrObj { hdr, kind: u16, msg: Arc<str>, tb: Option<Arc<Vec<FrameInfo>>>, val: Value }` | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | releases `val`; `msg`/`tb` self-drop |
| `channel` | `ChanObj { hdr, inner: Mutex<ChanState> }` | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | the queue's `Value`s, then the mutex |
| `mutex` | `MutexObj { hdr, inner: Mutex<Value> }` | `*const ObjHeader` | prefixed | 1 | 1 atomic inc | `release` the inner `Value` |
| `handle` | an index into the process-global handle table | `u32` in the payload | **no header** | none | 1 table inc | 1 table dec |

**`handle` is the odd one out, and it is the good kind of odd.** Per
[ADR-0004](decisions/ADR-0004-honest-types.md), a `handle` is an index into a process-global
table, not a pointer. It has a distinct `typeof()` result and **no refcount, no header, and no
`unsafe` at all** — the count lives in the table as an ordinary `AtomicU32`. It is the cheapest
heap-adjacent type in the language, and it costs the JIT a single `cmp`.

**`string` and `bytes` are the other odd ones, and they are the reason ADR-0001 gives them their
own tags.** They have no `ObjHeader`, so `is_string()` is a pure tag compare with zero loads,
while `is_dict()` costs a load. Their length and refcount live *behind* a thin pointer, which is
the one genuinely surprising thing about this layout and gets its own subsection.

### 3.2 The `Arc::into_raw` / `from_raw` discipline

Three operations, and the discipline is one sentence:

> **Every `Arc` pointer that leaves an owned `Arc` is accounted for by exactly one `from_raw` on
> the path to zero; and every `Value` that is *stored* rather than consumed is `retain`ed first.**

```rust
// ── Arc-backed: string and bytes ───────────────────────────────────────────

/// `Arc<str>` → `Value`. `Arc::into_raw` on an `Arc<str>` yields a *fat*
/// pointer; we discard the length half, which is safe precisely because
/// §3.3 can recover it from the allocation. That recoverability is the whole
/// reason `string` is `Arc<str>` and not `Arc<String>`.
#[inline]
pub fn str_to_value(s: Arc<str>) -> Value {
    let fat = Arc::into_raw(s) as *const str;
    let p = fat as *const u8;
    debug_assert_eq!(unsafe { str_len(p) }, fat.len());
    Value::from_bits((p as u64 & PAYLOAD_MASK) | (TAG_STRING << TAG_SHIFT))
}

/// # Safety
/// `p` must be a pointer produced by `Arc::into_raw` for an `Arc<str>`, and
/// the caller must own exactly one reference to give back.
#[inline]
pub unsafe fn str_from_raw(p: *const u8) -> Arc<str> {
    let len = str_len(p);
    // SAFETY: the layout of `ArcInner<str>` is
    //   [strong: usize][weak: usize][len: usize][bytes ...]
    // and `p` points at `bytes`, so the fat pointer `(p, len)` is exactly the
    // one `Arc::into_raw` produced. `Arc::from_raw` on a fat pointer is the
    // documented inverse, not an assumption.
    Arc::from_raw(std::ptr::slice_from_raw_parts(p, len) as *const str)
}

/// The one operation for a bare `Arc<str>` that cannot be safe: reading the
/// length from *behind* a thin pointer.
///
/// # Safety
/// `p` must be live, from `Arc::into_raw`, and the allocation at least 24 B.
#[inline]
pub unsafe fn str_len(p: *const u8) -> usize {
    // SAFETY: `len` sits at offset 16 of `ArcInner<str>` and `p` is the
    // `bytes` field at offset 24, so `len` is at `p - 8`.
    std::ptr::read(p.cast::<usize>().sub(1))
}

// ── Header-backed: the other nine ──────────────────────────────────────────

/// The header's `rc`, as an `AtomicU32`.
///
/// # Safety
/// `p` must point at a live `ObjHeader`.
#[inline]
pub unsafe fn rc_ptr(p: NonNull<ObjHeader>) -> &'static AtomicU32 {
    // SAFETY: `AtomicU32` has the same in-memory representation as `u32`
    // (guaranteed by std), so the cast is layout-preserving. The `'static`
    // lifetime is a lie in the borrow-checker's sense and the truth in
    // Miri's: the referent's real lifetime is the object's, and every caller
    // holds a reference that keeps it alive. This is the standard `Arc`
    // internal-pointer pattern.
    &*(p.as_ptr().cast::<u8>().add(8) as *const AtomicU32)
}

/// Increment the strong count. The `Arc`-equivalent operation, on the manual
/// `rc` in the header.
///
/// # Safety
/// `v` must be a live heap `Value`, and the caller must own a reference to
/// give — that is, either it constructed `v` or it called `retain` before
/// storing it. A `retain` without an owned reference is a leak; an owned
/// reference without a `retain` is a use-after-free. Both are bugs, and test 4
/// finds both.
#[inline]
pub unsafe fn retain(v: Value) {
    match tag_of(v.to_bits()) {
        TAG_HEAP             => (*rc_ptr(header_of(v))).fetch_add(1, Ordering::Relaxed),
        TAG_STRING | TAG_BYTES => Arc::increment_strong_count(v.ptr::<u8>()),
        _ => {}                        // nil, bool, int, float: nothing to own
    }
}
```

`retain` is a *safe-looking* operation that the borrow checker cannot place, which is why it is
on the §8.1 list. The invariant it protects:

> A `Value` is a bit copy. `Value::clone` is a `u64` copy and does **not** touch `rc`. Any code
> that stores a `Value` into a `Vec`, a struct field, a table, or a native module's retained state
> must call `retain` at the store, and that container's `Drop` must release it.

Every container whose `Drop` releases its contents therefore has a hand-written `Drop`, and every
one of those is on the §8.1 list. There are eight and they are each two lines.

### 3.3 The `unsafe` audit list, made explicit

This is the answer to "where does the `unsafe` live". It is the complete list for M2–M8.
[`08-vm.md`](08-vm.md) §3.5 makes the same promise for the VM crate; this is the memory half.

| # | Item | Where | Why it cannot be safe | What makes it correct |
|---|---|---|---|---|
| 1 | `Value` ↔ `u64` bit punning | `value.rs` | NaN-boxing is a transmute by definition | `to_bits`/`from_bits` are total; `debug_assert!` on the 48-bit range per ADR-0001 |
| 2 | `from_header` / `header_of` | `value.rs` | Rebuilding a `Value` from a raw pointer | `debug_assert!(addr >> 48 == 0)` (ADR-0001) |
| 3 | `rc_ptr` — `u32` as `AtomicU32` | `value.rs` | a shared field under `repr(Rust)` | `AtomicU32` and `u32` have identical layout, guaranteed by std |
| 4 | `str_len` / `bytes_len` | `value.rs` | reading the length *behind* a thin pointer | Relies on `ArcInner<str>`'s layout; **test 9 asserts it empirically, in CI, on every platform** |
| 5 | `release` — the whole drop path | `value.rs` | dereferencing a raw object and recursing | only called on a `Value` that owns a reference; `debug_assert!(rc > 0)` |
| 6 | `new_tuple` and the `Layout::extend` payloads | `value.rs` | writing through a raw pointer into fresh memory | size and alignment computed from `Layout`; no arithmetic on live objects |
| 7 | `alloc`'s six initialisation stores | `value.rs` | writes through a raw pointer | memory is exclusively ours between `alloc` and the return |
| 8 | `cow_mut`'s detach path | `value.rs` | destructively replacing a shared payload | guarded on `rc == 1`; `debug_assert!` on the compare |
| 9 | the eight hand-written `Drop` impls | `value.rs` | each calls `release` | each mirrors a constructor |
| 10 | `registers.get_unchecked` | `vm.rs` | **not memory's** — [`08-vm.md`](08-vm.md) §3.2 | load-time validation |
| 11 | `marks` and forwarding writes | `gc.rs` | **M9 only; the file is empty and `unsafe`-free in M2–M8** | §7.3's bit discipline |

**M2–M8 therefore has nine, all in one file.** If `git grep -n unsafe zen-alloc/` returns more
than those nine, something has gone wrong, and that is a CI assertion (§8.3).

The eight `Drop` impls, because "eight hand-written `Drop`s" is exactly the sentence a reviewer
should refuse to accept without seeing:

```rust
// SAFETY on every arm: the container was built by its constructor, which
// retained every element it stored. We own exactly `len()` references.

impl Drop for ListObj   { fn drop(&mut self) { for v in self.inner.drain(..) { unsafe { release(v) } } } }
impl Drop for DictObj   { fn drop(&mut self) {
    // Keys are `Value`s too. `#{[1]: 2}` has a heap key, and omitting this
    // line leaks it. It was omitted once in Zen 2.x.
    for (k, v) in self.inner.drain(..) { unsafe { release(k); release(v) } } } }
impl Drop for SetObj    { fn drop(&mut self) { for v in self.inner.drain(..) { unsafe { release(v) } } } }
impl Drop for TupleObj  { fn drop(&mut self) {
    for i in 0..self.len() { unsafe { release(self.slot(i)) } } } }
impl Drop for InstObj   { fn drop(&mut self) { for v in self.slots.iter() { unsafe { release(*v) } } } }
impl Drop for ErrObj    { fn drop(&mut self) { unsafe { release(self.val) } } }  // msg and tb are Arc: self-dropping
impl Drop for ChanObj   { /* ChanState's queue holds Values; its own Drop releases them */ }
impl Drop for MutexObj  { fn drop(&mut self) {
    // `get_mut` on a poisoned mutex returns `Err`; a Zen error is a `Value`,
    // never a panic, so the mutex cannot be poisoned (08-vm.md §5.4). This
    // is therefore an infallible unwrap in disguise, and the `debug_assert`
    // is the assertion that it stays one.
    let inner = self.inner.get_mut().expect("a Zen panic cannot poison a mutex");
    unsafe { release(*inner) } } }
```

`ClosureObj` is the ninth container and needs no `Drop`: its `upvalues` are `Arc<Upvalue>`, and
dropping the `Box<[Arc<Upvalue>]>` is the whole story.

### 3.4 `Value` is `Copy`, and therefore leaks by default

**This is the most important rule in the document and it is the one a reviewer should check
first.**

`Value` is 8 bytes of NaN-box. It *must* be `Copy` — the register file is `[Value; 256]`, the
inline-cache lookup does `let e = self.ic[site]`, and none of that works if reads clone. `Copy`
means a `Value` has **no destructor**. Therefore:

> **Writing a heap pointer into a register does not release the value that was there.**

A bare `self.registers[a] = v;` **leaks** whatever `self.registers[a]` held. The VM must go
through a store:

```rust
impl Vm {
    /// The ONLY way to write a register. Every dispatch arm in 08-vm.md §3
    /// that has a `dst` operand uses this.
    ///
    /// `#[inline(always)]`; the fast path is a compare of the old tag
    /// against two immediates and a store. No load, no branch on the
    /// common case.
    #[inline(always)]
    pub fn store(&mut self, a: u16, v: Value) {
        let old = *self.registers.get_unchecked(a as usize);
        let needs_release = matches!(tag_of(old.to_bits()), TAG_HEAP | TAG_STRING | TAG_BYTES);
        *self.registers.get_unchecked_mut(a as usize) = v;
        if needs_release {
            // SAFETY: `old` sat in a register, so this `Vm` owned exactly one
            // reference to it, and we are giving that reference up.
            unsafe { release(old) }
        }
    }
}
```

| Rule | Applies to | Enforcement |
|---|---|---|
| Never write `registers[a] = v` directly | the register file | a `build.rs` grep for `registers\[[^]]*\]\s*=` in `vm.rs` (§8.3) — crude, unarguable, and a `clippy` lint does not exist for this |
| Every `push(v)` into a `Vec<Value>` is preceded by a `retain(v)` | all container code | a `Drop` on every such `Vec`; a missed `retain` is a **double-free**, and test 4 finds it |
| A `Value` leaving a native call is either freshly built or `retain`ed | the v1/v2 ABI ([`08-vm.md`](08-vm.md) §8.1) | the `NativeFn` doc comment says which, and the `zen-natives` review checklist checks it |
| In-place slot writes retain-then-release, never just assign | `SET_FIELD`, `IndexMap` mutation | a `store_slot` helper, so the two calls cannot be separated by review |

The last row is why `SET_FIELD` cannot be a three-line arm. It is
`cow_mut` (which may have replaced the whole buffer), then
`let old = slot[i]; *slot[i] = retain_new; release(old);`. Getting the order wrong is a
use-after-free; getting it right costs one load and two atomics on a path that
[`08-vm.md`](08-vm.md) §12.3 says is the single biggest benchmark win in the language.

### 3.5 `func` and the non-capturing-closure cache

[`08-vm.md`](08-vm.md) §3.4's `CLOSURE` row already says a non-capturing function literal is **one
allocation ever**, cached in `vm.closures[proto_idx]`. That cache is a `Vec<Option<Closure>>` and
it is a **root set** (§7.2, row 9) and a place where an `Option` holding a heap value is
overwritten:

```rust
/// Called from the `CLOSURE` arm. The returned `Value` is **owned by the
/// caller**, which is why the `Some` arm retains.
fn cached_closure(&mut self, proto: u32) -> Value {
    match &self.closures[proto as usize] {
        Some(c) => {
            let v = c.to_value();
            unsafe { retain(v) };      // the cache still holds one; we now hold two
            v
        }
        None => {
            let obj = ClosureObj::new_non_capturing(proto);
            let v = obj.to_value();
            self.closures[proto as usize] = Some(obj);   // moves; the slot owns it
            v                                              // the arm owns it too
        }
    }
}
```

---

## 4. Copy-on-write semantics in detail

### 4.1 What Zen 2.x already proved, and where

The old tree documents the property in a comment, and that comment is the reason this ruling is
low-risk:

```rust
// zen/src/runtime.rs:186-190 (Zen 2.2.0, still present in the old tree)
    // Reference-counted with copy-on-write semantics: cloning a Value::List
    // or Value::Dict is O(1); mutations go through Arc::make_mut which clones
    // only when the container is shared.
pub enum Value {
    // …
    List(Arc<Vec<Value>>),
    Dict(Arc<indexmap::IndexMap<String, Value>>),
```

The property this document depends on, and which Zen 2.x has been running for years:

> **An assignment between two Zen bindings is O(1) and allocates nothing, regardless of the
> container's size.**

`d2 = d` for a 100 000-entry dict is one 8-byte move and one atomic increment. This is what makes
passing containers to functions, storing them in lists, and returning them from builtins free, and
every list-shaped row in [`08-vm.md`](08-vm.md) §12.3 assumes it.

### 4.2 `cow_mut`, which is `Arc::make_mut` with `rc == 1`

[ADR-0002](decisions/ADR-0002-memory-model.md) says mutation "goes through `Arc::make_mut`". Under
§4.6's manual `rc` the std function is not applicable, so its semantics are reproduced exactly:

```rust
// zen-vm/src/value.rs

/// Copy-on-write access.
///
/// Returns a mutable reference to the payload **only if this `Value` holds
/// the last reference**. Otherwise it detaches first: allocate a fresh
/// object, copy the payload into it, give up this reference to the old one,
/// and return a reference to the new.
///
/// This is `Arc::make_mut` with `rc == 1` substituted for
/// `strong_count(..) == 1`. Same test, same clone condition, same O(1)-clone
/// property, so ADR-0002's decision is unchanged; only the spelling differs
/// (§4.6 asks a reviewer to amend the ADR's notation).
///
/// # Safety
/// The caller must hold exactly one reference to `v`, and must not use `v`
/// again without re-reading it — `cow_mut` may have replaced the object.
/// That is why the return type carries no lifetime tied to `v`.
#[inline]
pub unsafe fn cow_mut<T: Payload>(v: Value) -> Result<*mut T, CowError> {
    let p = header_of(v);
    if (*rc_ptr(p)).load(Ordering::Relaxed) == 1 {
        debug_assert!((*p.as_ptr()).marks & marks::FORWARDED == 0);
        return Ok(payload_ptr::<T>(p));            // unique: mutate in place
    }
    // Shared: detach. `clone_payload` is `Vec::clone` / `IndexMap::clone` /
    // `to_vec` — the same work `Arc::make_mut` would do.
    let new = T::clone_payload(payload_ref::<T>(p))?;
    // Give up our reference to the old object *before* installing the new
    // one, so a failure in between cannot leave a register holding a
    // dangling pointer.
    release(v);
    install(v, new)
}
```

Two details in that code are the whole design:

- **`rc == 1` is the test** — not `rc == 0`, and not "no other thread". A `Value` in the register
  file and the same `Value` in a caller's window are two references, so `rc == 2` and a write
  through the callee detaches. That is the correct, conservative answer: the callee cannot know
  whether the caller's register still holds it, and assuming otherwise is the aliasing bug.
- **The old reference is released before the new one is installed.** Doing it the other way leaves
  a window in which a panic — and [`08-vm.md`](08-vm.md) §4's no-panic rule means
  `clone_payload` cannot panic, since a `Vec` clone of ≤ 2³² elements can only fail by aborting
  (§4.6 of that document) — or an `Err` return would leave the register dangling.

### 4.3 The cost table

`n` is the container's element count, `b` a string's byte length, `k` its capacity.

| Operation | Cost | Allocates? | Atomic RMW? | Notes |
|---|---|---|---|---|
| `d2 = d` (any container) | 1 atomic increment | no | 1 | The property of §4.1. Touches the object's own cache line |
| `f(d)` — pass as an argument | 1 increment in the `CALL` arm | no | 1 | [`08-vm.md`](08-vm.md) §3.4 `MOVE`s into the callee window |
| `return d` | 1 increment | no | 1 | same reason |
| `d.a`, monomorphic and shaped | 1 load + 1 compare | no | 0 | IC hit; [`08-vm.md`](08-vm.md) §6.1 |
| `d[k]`, `k` constant | 1 load + 1 compare | no | 0 | |
| `d[k]`, `k` dynamic | hash + probe | no | 0 | ICs do not help; [`08-vm.md`](08-vm.md) §6.2 forbids a site here |
| `d.a = v`, `rc == 1`, key exists | slot store + 1 release + 1 retain | no | 2 | **The common case.** No shape transition |
| `d.b = v`, new key, `rc == 1` | slot store + a shape transition + possibly a slot-array grow | maybe | 2 | The transition is one push to `vm.shapes`, amortised across every object that reaches it ([`08-vm.md`](08-vm.md) §7.5) |
| `d.a = v`, `rc > 1` | **full clone of `n`** + 1 release + 1 retain | **yes, n** | 3 | The cost CoW exists to avoid |
| `d2 = d; d2.a = 1` | clone of `n`, then a store | **yes, n** | 3 | The canonical aliasing case |
| `list.append(x)`, `rc == 1` | possible `Vec` grow, amortised O(1) | maybe | 2 | |
| `list.append(x)`, `rc > 1` | clone `n`, then append | **yes, n** | 3 | |
| `s = t` | 1 atomic increment | no | 1 | `Arc<str>`; no copy of `b` bytes |
| `s + t` | alloc `b₁+b₂` + 2 `memcpy` | **yes, O(b)** | 3 | §5. The known quadratic case |
| `tuple` read | 1 load | no | 0 | |
| `x = handle` | 1 table increment | no | 1 | No header, no payload (§3.1) |

**The row that decides whether a real program is fast** is `d2 = d; d2.a = 1`. In real code,
aliasing a container and then writing to *one* of the two is common enough that cloning a whole
container for a single field write is routinely the wrong trade. Two mitigations exist and only
one is used:

1. **Escape analysis in the compiler** ([`07-bytecode.md`](07-bytecode.md) §11). If a parameter is
   never captured by a closure, never stored, and never returned, the compiler knows the callee
   holds the only reference and can emit a "unique" variant of the mutating opcode. Deferred past
   M4; the `zen-perf` counters exist to make it measurable first.
2. **A `unique`-hinted mutating opcode.** `COW_SET_FIELD` whose fast path is the `rc == 1` test
   and whose clone is `#[cold]`, so the failure costs a predicted-not-taken branch. This is
   [`08-vm.md`](08-vm.md) §6.3's `#[cold]` miss pattern applied to ownership rather than to shapes,
   and it should land in **M5**, alongside the ICs, because by then the shape-transition half of
   the same arm exists.

### 4.4 CoW and shapes: the sharp edge, restated

[`08-vm.md`](08-vm.md) §7.5 has the full argument. The one-line version for this document:

> `d = #{a: 1}; d2 = d; d.b = 2` — `cow_mut` sees `rc == 2`, clones, and the clone gets a **new**
> `ShapeId` for `{a, b}` while `d` keeps `{a}`. Both stay monomorphic. The immutability of shapes
> ([`08-vm.md`](08-vm.md) §7.1) is what makes this safe: sharing a shape is fine, because a shape
> describes a layout, and layouts only ever diverge by *creating* a new shape.

The corollary a reviewer should check: **`cow_mut` must not be able to hand out a `&mut` to a
payload whose `shape` is then written in place.** The `rc == 1` path returns the *existing* object
and its existing `shape`, and a `SET_FIELD` on an existing key does not change the shape at all.
The clone path returns a brand-new object whose `shape` is `0` (`Unshaped`), and its next
`SET_FIELD` creates the `{a,b}` shape. Both are correct. The bug this forbids is `cow_mut`
handing out a `&mut` and the caller then writing `header.shape = new_shape` unconditionally.

### 4.5 Drop order

[`00-blueprint-review.md`](00-blueprint-review.md) §5 defines stability as "no use-after-free in
release or debug", and drop order is where that is won or lost. Two rules:

> **A container releases its children *before* its own allocation is freed**, and children are
> released in a defined order — insertion order for the ordered containers, so a `Drop` impl is
> deterministic and therefore testable.

`drain(..)` gives that for free and is why every `Drop` in §3.3 uses it. The two cases needing more
than a `drain`:

- **`DictObj` releases keys before values**, so a key and a value that are the *same* object
  (`d[1] = d` is impossible, but `d["a"] = l; l.append(d)` is not) cannot have its second
  reference released before the first.
- **Cycles leak.** A list containing itself is a reference cycle, and a reference-counted collector
  cannot collect one. This is [ADR-0002](decisions/ADR-0002-memory-model.md)'s accepted
  consequence: "Reference cycles leak. Accepted: cycles are rare in scripts, and the JIT work in
  M11 is worth more than cycle collection." It goes in the language manual, and §9's test 14
  asserts that a cycle leaks *predictably* — RSS grows by about one cycle's worth and then stops.
  An unbounded leak is a bug; a bounded one is a language property.

  The M1–M5 freeze audit confirmed this is the *only* leak class present: LeakSanitizer reports the
  interpreter leaking the scope↔closure cycle for any program that defines a function (a plain
  assignment or class does not), and the VM leaking the analogous closure/module cycle. Every other
  allocation returns to baseline. No cycle collection is added before M9; the `--mem-limit` aggregate
  bound (§11.5 test 33, `08-vm.md` §4.6) is likewise M9 work.

### 4.6 `Arc::make_mut` versus a manual `rc` — an ADR-0002 amendment is required

**This is a real textual conflict between two canonical documents and it is recorded rather than
resolved silently.**

- [ADR-0002](decisions/ADR-0002-memory-model.md) says the payload is `List(Arc<Vec<Value>>)`,
  `Dict(Arc<IndexMap<Value, Value>>)` and that mutation "goes through `Arc::make_mut`".
- [`08-vm.md`](08-vm.md) §8.2 says `ObjRef` is "a tagged `NonNull<ObjHeader>` with a **manual
  refcount**, not an `Arc<ObjHeader>`", and gives the header a `shape: u32` and a `size: u32`.

These cannot both hold. `Arc<Vec<Value>>` points at a `Vec`, and a `Vec` has nowhere to put a
`shape` and a `size`. Any design that keeps M5's shapes and M9's collector needs a per-object
header; a per-object header means the object is no longer `Vec<Value>` but
`struct { header: ObjHeader, inner: Vec<Value> }`, at which point `Arc::make_mut` no longer
applies.

| Option | `Value` representation | `make_mut` | Shapes (M5) | GC (M9) | ADR-0002 text |
|---|---|---|---|---|---|
| **A. `Arc<ListObj>`** | `Arc<{hdr, Vec}>` | std `Arc::make_mut`, verbatim | works | works — a semispace collector can move an `ArcInner` allocation whole, because the refcount cell is at a fixed negative offset that moves with it | **literal** |
| **B. manual `rc` in the header** | raw `*const ObjHeader` | reimplemented as §4.2's `cow_mut` | works | works, and the mark bits live in a header that exists anyway | amended |

**Ruling: B, with a one-paragraph ADR-0002 amendment that changes the notation and not the
decision.** The reasons, in order of weight:

1. ADR-0002's *decision* — refcounting, clone-on-write, gc-gated-on-evidence — is untouched. Only
   the mechanism changes, and §4.2's `cow_mut` has the same test (`rc == 1`) and the same clone
   condition as `Arc::make_mut`. A reviewer who objects is objecting to spelling.
2. B gives the collector a uniform **"`+0` is the header, `+16` is the payload"** rule across all
   nine heap types and across both collectors. Under A the header would sit *after* `ArcInner`'s
   two refcount words, so it is at `+16` from the raw pointer and the payload at `+32` — a rule
   with two cases is a rule that gets a case wrong.
3. The forwarding pointer in §7.5 needs a payload word regardless, so a uniform rule is what
   makes the copy loop a single function rather than two.
4. **The cost of B is nine lines and zero allocations.** `cow_mut` and `Arc::make_mut` compile to
   the same `load; cmp; jne clone`, and B's `Drop` impls are strictly simpler because there is no
   `Arc` to reason about.

The action item is a short ADR-0002 revision, and it is on §11's list. **It should be written
before M2 lands**, because changing the representation afterwards is a `.zeno` format change.

---

## 5. String concatenation and the quadratic build loop

### 5.1 The cost, stated plainly

This is the one place where the CoW ruling is *worse* than the alternative, and it is called out
here rather than discovered in CI.

```zen
s = ""
i = 0
while i < 200_000 {
    s = s + "x"
    i = i + 1
}
```

| Scheme | Allocations | Bytes allocated | Bytes copied | Complexity |
|---|---|---|---|---|
| **M2–M8 as specified** (`Arc<str>`, `s + t` allocates) | 200 000 | 1 + 2 + … + 200 000 = **20 000 100 000** ≈ 20.0 GB | the same ≈ 20.0 GB | **O(n²) in both time and allocation** |
| Python 3.12 and earlier (`str` refcount 1, in-place resize amortised) | ~700 | ≈ 0.5 GB | ≈ 0.4 GB | O(n) amortised |
| Python 3.13 (`str` has `_gc_issafe` unused capacity; `+=` in a loop is over-allocated) | ~700 | ≈ 0.5 GB | ≈ 0.4 GB | O(n) amortised |
| Rust `String` (`push_str`) | ~18 | ≈ 0.6 GB | ≈ 0.4 GB | O(n) amortised |
| A rope / `StringBuilder` (what §5.5 proposes) | O(log n) | O(n) | O(n) | O(n log n) |

**20 GB of `memcpy` for a 200 000-iteration loop that should take 20 MB.** At an [E] of 8 GB/s for
`memcpy` and 4 GB/s for allocation-plus-copy, that loop is **[E] 7.5–10 seconds**. The same loop
under Python 3.13's scheme is [E] 0.15 s, and under a rope [E] 0.3 s. **Zen would be 25–50× slower
than CPython at string building**, and that is a language-fatal, not a microbenchmark, gap: string
building is what a script *is*.

I did not estimate 20 GB to make a point. `1 + 2 + … + n = n(n+1)/2`, `n = 200_000`, and every
intermediate string is materialised. The only way the estimate is wrong is if `s + "x"` is *not*
implemented as "allocate `b₁+b₂` and copy both", which is exactly what §3.1 specifies.

### 5.2 Why `Arc<str>` is still the right M2–M8 choice

Because the loop above is a **possible** pattern, not the **common** one, and because a rope is a
large, separate change.

| Pattern | Share in real scripts [E] | Under M2–M8 | Under §5.5 |
|---|---|---|---|
| `s = a + b`, one-shot | high | O(b) | O(b) |
| `s += a`, 2–20 iterations, accumulator | **common** | O(k²b), k ≤ 20 → 400·b worst case, typically ≈ 100·b | O(kb) |
| `s += a`, 1000+ iterations | **rare** | O(n²) — the failure above | O(n log n) |
| `lines.join("\n")`, `split("\n")` | common | O(n) | O(n) |
| f-strings, `format!` | common | O(n) | O(n) |
| `"a" * 10_000` | rare | O(n), one alloc | O(n) |

A 20-iteration `+=` accumulator is the pattern that matters, and at 100·b it is a constant factor,
not a complexity class. **The [T] gate is therefore at 20+ iterations, not at 2**, and it is
`linear_in_accumulator` — 200 000 iterations must not be 200× the time of 20 000. That gate fails
today, deliberately and visibly, and §5.5 is the fix.

### 5.3 What is cheap to do now, inside M2–M8

Four changes, all in the *builtin* and none in the `Value` representation, that recover most of
the common cases without touching ADR-0002:

1. **Constant folding of literal `+`.** `"a" + "b"` is folded in `zen-compiler` at parse time. A
   loop like `s = s + "x"` is *not* folded, but `s = "" + "x"` in a loop body is a candidate for
   an identity-fold in M4's optimiser. Small, safe, and helps a real class of generated code.
2. **`BUILD_STRING n` for `n > 2` operands.** Right-associativity means `"a" + b + c` is
   `("a" + b) + c` → two allocations and three copies. A dedicated opcode
   (`07-bytecode.md` §4's register-only opcodes are exactly where it belongs) sums all lengths
   first, allocates once, copies once. **This is a strict improvement with no representation
   change and should land in M4.**
3. **`zen.join(list)` and `zen.split(s, sep)`.** They already produce one result allocation
   rather than a chain, and they are the idiomatic answer a user reaches for. Make them fast
   rather than clever: `join` does one `size` pass, one `reserve_exact`, and one `push_str` per
   element with no intermediate allocations.
4. **A `textio`-style builder in `zen-std`.** `StringBuilder` with a `Vec<u8>` and an amortising
   `reserve` is ~40 lines of safe Rust and completely sidesteps the problem for anyone who cares
   about it. This is what Rust itself recommends, and it is the honest answer to "why is `+` slow"
   until §5.5 lands.

### 5.4 The fast path we do *not* take, and why

The tempting fix is: *"if `rc == 1` and the string has spare capacity, append in place."* It is
rejected, for three reasons that are each independently sufficient.

1. **`Arc<str>` has no capacity.** `str` is `(ptr, len)`. There is no `cap` field to grow into;
   growing means allocating a new allocation. The capacity lives in a `String` or a `Vec<u8>`,
   which is precisely the amendment §5.5 proposes and precisely what
   [ADR-0002](decisions/ADR-0002-memory-model.md) does not say today.
2. **The unique-reference test is not sufficient even with a capacity.** `s = t` gives `rc == 2`.
   Appending to `s` in place is then *wrong*, not merely slow: it mutates `t`. The
   `rc == 1` fast path is not reachable for a string that is aliased, and in the loop above the
   accumulator is aliased with the *interpreter's own copy* the moment `s + "x"` is evaluated,
   because the `+` operand `s` is moved into the call frame.
3. **`Arc` does not give you `reserve`.** The std `Arc` API has no `reserve` on `str`; there is
   nothing to grow. The whole idea requires owning the buffer rather than sharing it, which
   means a `String` or a `Vec<u8>` behind the pointer — the same amendment.

So the honest statement is: **the in-place-append fast path is not a missing optimization inside
this design, it is a different representation**, and §5.5 specifies it.

### 5.5 The M11 amendment: `StringObj { len, cap, bytes }`

Gated behind the same feature as [`10-jit.md`](10-jit.md), because they are the same change: **the
representation of `string` changes to something the JIT can build and mutate directly.** Neither is
worth doing alone.

```rust
// zen-vm/src/value.rs — compiled only under #[cfg(feature = "jit")]
// REQUIRES: a new ADR amending ADR-0002, plus a .zeno format version bump.

/// `string` becomes a prefixed object: a header, a length, a capacity, and a
/// byte buffer allocated *separately* so that the object itself never moves
/// and so that a `const string` can share the header with a runtime-growable
/// body.
///
/// The body is allocated with 16 spare bytes, so the first in-place append
/// needs no `realloc` at all; after that `realloc` grows geometrically like
/// every `String` in existence.
#[repr(C)]
pub struct StringObj {
    pub hdr: ObjHeader,          // +0, 16 bytes
    pub len: u32,                // +16
    pub cap: u32,                // +20
    /// Offset of the body. A fixed byte array with slack, then a
    /// `realloc`-grown heap tail. The JIT reads `len` at a fixed offset
    /// (what §5 of 10-jit.md relies on); the interpreter calls
    /// `as_str()`, which is a `slice::from_raw_parts` over `[body, body+len)`.
    pub body: [u8; STRING_INLINE],
}
```

What it buys, and what it costs:

| | `Arc<str>` (M2–M8) | `StringObj` (M11, gated) |
|---|---|---|
| `s + t`, unique | alloc + 2 copies | `reserve` + 2 copies, no alloc if slack |
| `s = s + "x"` in a loop | O(n²) | **O(n) amortised** |
| `s == t` | `len` + `memcmp`, both O(1) to reach | same, one more load |
| `s` in a dict key | hash O(b) | same |
| Clone cost | 1 atomic inc | 1 atomic inc |
| **Mutation while shared** | impossible (immutable) | **possible → needs `cow_mut`, so a shared `StringObj` in a `+=` loop still clones** |
| JIT `strlen` on `s` | must load `len` at `ptr-8` | **one `mov` at a fixed offset — the reason this matters** |
| JIT `s[i]` | bounds check against `ptr-8` | `cmp` against `+16`, a `cmp` against a `dword` in the same cache line as the header |
| Cost | — | a `.zeno` format bump, an ADR amendment, a `String` conversion in the FFI, and one more type in the collector's switch |

**The [T] gate for the whole §5 programme:** `zen-bench string_build` — a 200 000-iteration
accumulator, a 20-iteration accumulator, a 100 000-element `join`, and a
`"x".repeat(200_000)`. Target: the 200 000-iteration loop within **3×** of Python 3.13 on the same
machine ([E] ≤ 0.5 s), and **linear** in the iteration count with a doubling constant between 1.5×
and 3.0× when the iteration count is quadrupled. `Arc<str>` fails the first by [E] 25–50×.

---

## 6. The `Gc` trait

### 6.1 The signature

```rust
// zen-alloc/src/lib.rs

/// A collector. Two implementations exist across the project's life:
/// `RefCountGc` (M2–M8, `#[derive(Default)]`, zero-sized, zero-cost) and
/// `TracingGc` (M9, §7). `zen-vm` calls only through this trait, and §6.4
/// explains why calling through it costs nothing in M2–M8.
pub trait Gc: 'static {
    /// Allocate `size` bytes with `align`, return a pointer to the **start
    /// of the block including `ObjHeader`** (the caller writes the header;
    /// the collector does not interpret it, because `zen-alloc` does not
    /// depend on `zen-vm`).
    ///
    /// `Err` on `--mem-limit` exhaustion. For `TracingGc` this is *also*
    /// where a collection happens: it allocates, fails, collects, retries
    /// once, then returns `Err`. A second failure is fatal (§4.6 of
    /// 08-vm.md).
    fn alloc(&self, kind: u8, size: usize, align: usize) -> Result<NonNull<u8>, GcError>;

    /// Report one object's live size, for `--mem-limit` accounting and the
    /// `gc_stats` builtin.
    fn live_bytes(&self) -> usize;
}
```

Two things about that trait are deliberate.

- **It takes `size` and `align`, not a `Layout`.** `zen-alloc` depending on `Layout` is fine, but
  passing primitives keeps the crate's public surface to four items and makes the trait object a
  trivially-predicted call. The `alloc` in §2.5 is the only caller in M2–M8.
- **It returns `Err`, and the VM turns that into a fatal error, not a panic.**
  [`08-vm.md`](08-vm.md) §4's "no panics" rule is satisfied by never panicking on OOM: the
  allocation site returns `Err`, `alloc` in §2.5 propagates, `exec` reports the error, and
  `main` exits 134 like a real OOM. The one `.expect()` in §2.5's `new_tuple` is a **misreading**
  and §11 flags it: `new_tuple` must return `Result` too, or must document that the tuple is
  bounded by the operand stack so the allocation cannot fail. This is a genuine finding, not a
  formality.

### 6.2 The `Value` and `Gc` relationship

`Gc` is a **trait over the allocation strategy, not over `Value`**. A `Value` is a bit pattern and
knows nothing about collectors; `zen-alloc` does not know about NaN-boxing. The dependency graph:

```
zen-alloc   depends on:  std
zen-value   depends on:  std            (Value, Kind, bits; no allocation)
zen-vm      depends on:  zen-alloc, zen-value
```

This is the [blueprint](00-blueprint-review.md) §4.3 layering, and it is what lets §7's collector
be written without touching `Value`. The M9 collector needs to know one thing about `Value`: which
`Value`s can hold a pointer, and where the tag table is. That knowledge lives in **`zen-vm`** as a
callback, not in `zen-alloc`:

```rust
// zen-alloc/src/lib.rs
/// What the collector needs to know about a word it found in a live
/// object. Supplied by `zen-vm`, which owns the `Value` definition.
pub trait Trace: 'static {
    /// A machine word. The collector's job is to turn a set of these into
    /// the set of objects that must survive.
    ///
    /// Three verdicts, not two, and the third is what makes a semispace
    /// collector work without a read barrier: **interior** pointers point
    /// *into* a payload and must be adjusted when the object moves, but
    /// are not themselves roots.
    fn trace_word(&self, w: usize) -> Word;
    /// The finalizer, if any, for an object about to die.
    fn finalizer(&self, kind: u8) -> Option<FinalizerId>;
}

pub enum Word { NotPointer, Object(usize), Interior(usize) }
```

`Interior` is not hypothetical. `ObjHeader` pointers, `Arc<str>` bodies, and §7.5's forwarding
pointers are all interior. A collector that treats every non-null word as a root either
segfaults or silently leaks, and the Miri test in §9 test 6 exists to find that.

### 6.3 Why `RefCountGc` is a zero-sized type

```rust
// zen-alloc/src/lib.rs

/// M2–M8's collector. ZST: no state, no atomic, no `static mut`.
///
/// `live_bytes` is the sum of every `ObjHeader.size` written since process
/// start minus every freed one, maintained by... nothing. It is
/// `atomic_load_relaxed(&LIVE_BYTES)`, a single `static AtomicUsize`
/// incremented in `alloc` and decremented in `free`. That is the *only*
/// atomic in the M2–M8 allocation path, and it is a `Relaxed` store on a
/// cache line that is already being written for the `rc` bump — so it is
/// free in practice. It is `#[cfg(feature = "gc-stats")]` in release
/// builds; always on in debug.
#[derive(Debug, Default)]
pub struct RefCountGc;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

impl Gc for RefCountGc {
    #[inline(always)]
    fn alloc(&self, _kind: u8, size: usize, align: usize) -> Result<NonNull<u8>, GcError> {
        if size.checked_add(LIVE_BYTES.load(Relaxed)).is_none() || exceeds_mem_limit(size) {
            return Err(GcError::OutOfMemory { requested: size });
        }
        // SAFETY: `align` is a power of two and >= `align_of::<u8>()`; the
        // size is at least 16 (§2.5), so the layout is valid.
        let l = Layout::from_size_align(size, align).map_err(|_| GcError::BadLayout)?;
        let p = unsafe { std::alloc::alloc(l) };
        if p.is_null() { return Err(GcError::OutOfMemory { requested: size }); }
        LIVE_BYTES.fetch_add(size, Relaxed);
        Ok(NonNull::new_unchecked(p))
    }

    #[inline]
    fn live_bytes(&self) -> usize { LIVE_BYTES.load(Relaxed) }
}
```

A ZST collector is not a placeholder. It is the *reason* the trait is a trait: the M9 collector has
a large `struct` behind the same `&'static ActiveGc` reference, and because `ActiveGc` is a
`dyn Gc` behind a fat pointer, the *M2* call site is one indirection that LTO devirtualises. §6.4
is the argument that this is actually free.

### 6.4 The selector is a cargo feature, and the trait object is not a cost

Two questions a reviewer will ask, answered with measurements to take rather than hand-waving.

**"A `dyn Gc` call is an indirect call. Isn't that a pointer chase per allocation?"** In M2–M8,
`ActiveGc` is a `&'static` to a ZST behind a `dyn Gc`, so the vtable pointer is a constant and
LTO — which this project has from day 0 ([`00-blueprint-review.md`](00-blueprint-review.md) §7
requires it for the dispatch loop) — collapses the call to the concrete `RefCountGc::alloc`
body. The proof is not an argument but a test: [`09-memory.md` §9 test 15](09-memory.md) asserts
that the compiled `zen-vm` binary contains **no** `call *(%r*)`-style indirect call in
`Interpreter::alloc_value` and that the symbol `RefCountGc::alloc` is inlined into it. If LTO
fails to devirtualise, the fallback is a `#[cfg]`-selected concrete type alias, and §11 records
that as the acceptable alternative.

**"Why is the selector a cargo feature and not a runtime `gc_selector` in `Vm`?"**

```toml
# zen-alloc/Cargo.toml
[features]
default  = []
# M9. Off by default. The M9 gate in ADR-0002 is *evidence-driven*, and
# evidence is gathered from a build that has no collector in it — a
# build with a collector in it has different allocation addresses, a
# different cache footprint, and a different profile.
gc        = []
gc-stats  = ["gc"]        # per-object counters, for the §10 metrics

# zen-vm/Cargo.toml
[features]
gc        = ["zen-alloc/gc", "zen-vm/use_tracing_gc"]
```

The reasons, and they are all about the M9 gate being *honest*:

1. **The profile that decides M9 must be of M2–M8.** If `gc` were on, the numbers would not be
   the numbers the gate reads. That is the whole point of the gate.
2. **A runtime selector needs two heap implementations in one binary, so every `Value` is
   ambiguous** — is this `Arc<str>`'s refcount, or a nursery slot? The type would need a runtime
   tag, i.e. 1 bit in the 3-bit tag, which [ADR-0001](decisions/ADR-0001-tag-budget.md) does not
   have. **A runtime selector therefore costs a `Value` bit.** That is a hard argument and it is
   the real reason, not a preference for compile-time.
3. **A compile-time selector is testable in CI**: M2–M8's `cargo test -p zen-vm` runs the
   reference-count suite, and `cargo test --features gc` runs §7.11's collector suite against the
   same language tests. One test suite, two collectors, no runtime flag.

The cost: a `.zeno` binary built with `--features gc` is **not** loadable by a binary built
without it, and §10's `gc_stats` output differs. Both are recorded in the manual's "M9 preview"
section, and neither is a `.zeno` format change, because the *program* is unchanged — only the
runtime is.

---

## 7. The Milestone-9 tracing collector

Nothing in this section is built in M2–M8. It is specified now for one reason, stated in
[ADR-0002](decisions/ADR-0002-memory-model.md): **"the collector implementation is only about 400
lines" is only true if the design is nailed down now.** §7.1 through §7.10 are the nail.

### 7.1 The design, in one paragraph

**Cheney, two-space, semispace, stop-the-world-per-thread.** The nursery is two buffers of
`nursery_bytes / 2` each. Allocations bump a pointer in the active (from) semispace. When the bump
pointer reaches the end, we **flip**: the to-space becomes the new from-space (it must be empty, so
it is either the fresh malloc or the old from-space after `memset`-free reuse), and a collection
runs: mark from the roots, compute the new live set, evacuate survivors by copying each to the new
space and installing a **forwarding pointer** in the old object's first word, then sweep by moving
the bump pointer to the end of the survivors. No free list, no mark bits to clear (the `marks` byte
is never set — see §7.3), no per-object bookkeeping, and the whole thing is a single pass over
memory that is already hot in cache.

Rejected alternatives, each with its reason:

| Alternative | Why rejected |
|---|---|
| Mark-and-sweep (blueprint's original) | The original reason: a free list is a pointer chase per allocation, and fragmentation is unbounded without compaction. §A2 of the blueprint. |
| Generational (e.g. V8's, or a copying minor with a major pass) | Doubles the number of roots to enumerate (remembered sets) and the number of collectors to test. M9's gate is *"refcount traffic on the critical path"*, not *"the language needs a generational collector"*. Adding generations to a spec written before the profile exists is speculation. **Revisit only if §7.10's post-M9 profile says the single-gen collector cannot keep up.** |
| Card marking / generational write barriers, per-thread heaps | §7.6. A per-thread heap means an object can outlive its thread, and M10's scheduler moves work between threads. |
| A concurrent/incremental collector | Needs a tri-colour invariant, write barriers, and a safepoint protocol on *every* mutating opcode. The safepoint machinery is already required for the JIT ([`08-vm.md`](08-vm.md) §7.6) but reusing it for a concurrent collector is a second, much larger project. |
| Mark-compact | Same cost as mark-sweep for our purposes, and compaction *hurts* the ICs ([`08-vm.md`](08-vm.md) §6.1) because shape-sharing and slot addresses are better when objects do not move. Copying also moves, but only nursery objects, which is bounded. |
| Reference counting *plus* tracing (a hybrid) | The M9 premise is that refcounting is too expensive. A hybrid keeps the refcount traffic. If the M9 profile still shows `release` dominating, the honest conclusion is that the language needs a different ownership model (§7.10), not a second mechanism. |

### 7.2 The complete root map

**This is the precondition, and it is a table of fields that already exist in
[`08-vm.md`](08-vm.md) §2.1.** A new root-holding `Vm` field with no row here is a build failure
(§8.3). A row here with a `Vec` whose elements can be heap `Value`s is a row that needs a
`Trace` impl, not a `memcpy`.

| # | Root | Where | Type | How it is scanned | When a new `Vm` field needs a row |
|---|---|---|---|---|---|
| 1 | Register file | `vm.registers` | `[Value; 256]` | word-by-word | any new `Value` register |
| 2 | **Frame windows** | `vm.frames[i].window` | inline `[Value; WINDOW]` | word-by-word, per active frame | any local storage |
| 3 | Frame windows (inactive) | `vm.frames` capacity beyond `len` | the same array, `len` .. `cap` | **`Vec::as_ptr..+capacity` — see §7.7, this is the row people get wrong** | — |
| 4 | Active-proto constant pools | `vm.protos[p].consts` | `Vec<Value>` per proto | word-by-word, **all protos, not just active ones** | a module-level constant |
| 5 | **Proto upvalue templates** | `vm.protos[p].upvalues` | `Vec<Arc<Upvalue>>` | the `Arc` cell, not the contents | a proto-level value |
| 6 | Globals | `vm.globals` | `Vec<Value>` | word-by-word | a module-level mutable |
| 7 | Closure cache | `vm.closures` | `Vec<Option<Closure>>` | each `Some`'s payload | a cached object |
| 8 | String interner | `vm.strings` | `HashMap<Arc<str>, u32>` | the interner's own `Arc`s | an interned string |
| 9 | Name interner | `vm.names` | `HashMap<Arc<str>, u32>` | same | an interned name |
| 10 | Module handles | `vm.modules` | `HashMap<Arc<str>, ModuleId>` | the `Arc` keys | a loaded module |
| 11 | **Handle table** | `global`, `Vec<HandleEntry>` | `{ value: Value, strong: u32, weak: u32 }` | word-by-word | a handle |
| 12 | Error payload | `vm.exc_payload` | `Value` | one word | a pending exception |
| 13 | **Shadow / spill slots** | `vm.spill` | `Vec<Value>` | word-by-word **including `len`..`capacity`** | any spill slot |
| 14 | Native arguments | `vm.native_args` | `Vec<Value>` | word-by-word | a v1/v2 ABI call frame |
| 15 | **Native frames (Rust stack)** | `vm.native_stack` | a `Vec<NativeFrame>` pushed by the FFI shim | word-by-word | a native call |
| 16 | **Blocked native frames** | inside `ChanObj` / `MutexObj` | `Value`s parked by M10 | reachable from rows 1–10, **not a new root** | — |
| 17 | **Finalizer queue** | `global`, `Vec<FinalizerEntry>` | `{ obj: usize, id: u16 }` | the `obj` is a **root** while queued | any finalizer |
| 18 | **Active safepoint frames** | the JIT's frame table | `Vec<JitFrame>` | the frames' roots are the interpreter's registers, **plus** the JIT's own spill slots | a compiled frame |
| 19 | Temporaries in the allocator | `to_space` during a collection | — | none; the collection is single-threaded and the stack is a root | — |

Four rows deserve their own note, because each is a bug someone would ship.

- **Row 3.** A frame window is a fixed-size array inside a `Vec`. `len` is the call depth; the
  *capacity* is the allocation. A popped frame does not clear its slots, and `Vec::pop` does not
  shrink. **Those words still hold `Value`s, they can still point at nursery objects, and they are
  not reachable from row 2.** A collector that scans `..frames[i].window[..WINDOW]` will read
  stale pointers into a semispace that has already been reused, and the symptom is a segfault in
  the *next* collection, far from the cause. The fix is one line — scan the whole capacity — and
  the cost is scanning dead words, which is bounded by the maximum depth ever reached.
- **Row 5.** `Arc<Upvalue>` is one pointer to a refcount cell. The `Value` *inside* the upvalue is
  at a fixed offset after it. Tracing the cell reaches the value; tracing the value alone would not
  reach the cell. The `Upvalue` is 8 words, so the trace function is a `switch` on `UpvalueState`
  (closed: one root; open: the register slot index is a root through the register file, not
  through memory).
- **Row 11.** A `handle` is an index, not a pointer, so the *handle table* is a root and the
  handles themselves are not. A handle with `strong == 0` and `weak == 0` is dead and is skipped;
  a handle that is only weakly held does **not** keep its object alive, which means the object can
  die while the handle still exists, and using it raises the handle's dead error rather than
  segfaulting. That is [ADR-0004](decisions/ADR-0004-honest-types.md) working, and it must be
  tested (§9 test 12).
- **Row 18.** A JIT frame's roots are *mostly* the interpreter's, because the JIT calls
  `jit_safepoint` with the same `&mut Vm`. The exception is the JIT's own spill slots, which are
  C-allocated and unknown to the interpreter. This is the one place the two halves of the project
  must agree on a data structure, and [`10-jit.md`](10-jit.md) §7 specifies the frame table. **The
  memory collector and the JIT must be enabled together**, which the M11 feature flag does
  naturally.

### 7.3 Why `marks` is never set

`ObjHeader.marks` occupies a byte that is otherwise padding, and it is tempting to use it for a
Cheney mark bit. **Do not.** Two-space semispace does not need one:

- The **evacuate** pass visits exactly the live objects, and an object is live iff it is reached.
  There is no "have I marked this" question, because the copy loop *is* the mark.
- The forwarding pointer (§7.5) is the de-duplication mechanism, and it lives in the object's own
  first payload word — which is a word the collector is about to overwrite anyway.

So `marks` is reserved, and the byte is spent on the **finalizer mark** (§7.9) and the
**mutator-ownership mark** used by the pin test. Using a byte for a purpose that the algorithm does
not need is how a design acquires a clear bit that is set on a fast path and never cleared.

What `marks` is used for, then:

| Bit | Name | Set by | Meaning |
|---|---|---|---|
| `1 << 0` | `PINNED` | allocation, for a `ffi` object | does not move; §7.8 |
| `1 << 1` | `FINALIZER_QUEUED` | the finalizer thread | a finalizer is pending; do not queue twice |
| `1 << 2` | `FINALIZER_DONE` | the finalizer thread | the finalizer has run; free at the next collection |
| `1 << 3` | `NATIVE_OWNED` | the FFI shim | a native library holds the raw pointer; do not move or free |
| `1 << 4` | `YOUNG` | reserved for a future generational pass | always 0 in M9 |
| `bits 5..8` | — | reserved, `debug_assert!` 0 | a future `range` or an interior-marker |

### 7.4 Nursery size, and the sizing argument

| Parameter | Value | Basis |
|---|---|---|
| `NURSERY_BYTES` | **8 MiB total, 4 MiB per semispace** | CPython 3.13 uses 1 MiB per semispace; V8 uses a 1–16 MiB *semi*space for a 32 MiB young gen. A script's live working set at any allocation is small, so survival is rare and the copy is cheap. |
| Initial heap growth | `nursery × 2` per in-use semispace on each growth, capped at 64 MiB per semispace | exponential so a steady-state program stops growing, bounded so a single huge allocation does not reserve address space |
| Trigger | **bump pointer reaches the end of from-space** | the cheapest possible test, one compare per allocation |
| On trigger | allocate the to-semispace, run a collection, retry the failing allocation **once** | — |
| On second failure | `OutOfMemory` → fatal exit 134 | consistent with [`08-vm.md`](08-vm.md) §4.6 |
| Collection policy | **whenever the survivor set would not fit** — that is, if evacuation fills the to-semispace, that is a genuine OOM, not a "grow" | a copying collector's to-space is a hard bound on the live set; growing instead of failing is a different design (a growing semispace is fine too — see below) |

**The rule adopted: to-space overflow *grows* the semispace rather than failing**, up to the
64 MiB cap, and above the cap it is a fatal OOM. A live set larger than 4 MiB in a script is
possible (a 500 000-element list of 8-byte `Value`s is 4 MiB by itself), and failing there would
make the collector *less* capable than the reference counter it replaces. Growing costs nothing in
the common case because the common case never overflows.

**[E] collection cost.** Survival rate for a nursery-allocated script object is low; CPython's is
under 1% for typical object churn, and Zen's is plausibly higher because containers are large
(8 KiB lists are worth copying more than 8 KiB of ints). At an [E] 2% survival rate, a 4 MiB
semispace copies 80 KiB per collection, and a 100 000-allocation script does 2 collections. Copy
rate is [E] 3 GB/s (a `memcpy` per object plus the evacuation loop's write barrier), so that is
[E] 27 µs of the script's total. Negligible. The **fatal** case is a program that pins its whole
live set in the nursery, which is §7.8.

**[T] gate:** `zen-bench gc_micro` — a 10-million-iteration integer loop allocates nothing and must
collect zero times; a 1-million-iteration list-append loop must collect O(1) times and its total
copy volume must be under 5% of its allocation volume; a script with a 32 MiB live set must
complete with under 100 collections and no OOM.

### 7.5 The collection algorithm, line by line

```rust
// zen-alloc/src/trace.rs  — M9 only, `#[cfg(feature = "gc")]`

/// Evacuate a single object. Returns the object's *new* address.
#[inline]
fn evacuate(&self, old: NonNull<u8>, trace: &dyn Trace) -> NonNull<u8> {
    let size = read_u32(old, OFF_SIZE) as usize;

    // 1. Forwarding pointer: the first word of the payload. Cheap because
    //    for a fixed-size object the payload is a Rust struct whose first
    //    field we will overwrite anyway; for a variable-size object the
    //    payload is at least one `Value` (8 bytes) by §2.5.
    let fwd_slot = old.as_ptr().add(16);
    let existing = read_usize(fwd_slot);
    if existing & FORWARDED != 0 {
        // Already evacuated. Note this is checked on *every* edge, and
        // `FORWARDED` is 1 so the test is a bit test on a value that was
        // just written — the branch predicts.
        return NonNull::new_unchecked((existing & !FORWARDED) as *mut u8);
    }

    // 2. Copy the whole object, header included. 16-byte-aligned `memcpy`
    //    via `copy_nonoverlapping`, which LLVM lowers to two `movups` for
    //    small sizes.
    let new = self.bump(size)?;
    unsafe { std::ptr::copy_nonoverlapping(old.as_ptr(), new.as_ptr(), size) };

    // 3. Write the forwarding pointer into the OLD object. The old object is
    //    never read again except through this word, which is why the sweep
    //    can ignore it entirely.
    unsafe {
        write_usize(fwd_slot, (new.as_ptr() as usize) | FORWARDED);
    }

    // 4. Trace the *new* object's interior words, breadth-first, by pushing
    //    them onto the gray stack. The header is copied verbatim except
    //    `marks`, which §7.3 leaves at 0.
    let kind = unsafe { *new.as_ptr() };
    trace.push_children(new, kind, trace);
    new
}
```

The gray stack is a `Vec<NonNull<u8>>` that doubles as the mark list — no separate mark array,
because the forwarding pointer *is* the mark. Its size is a real concern and the design treats it
as one:

- A 32 MiB live set of 16-byte-header-plus-8-byte-`Value` objects is ~1.4 M objects, so the gray
  stack needs 11 MiB. That is too much for an array on the side.
- **Ruling: the gray stack is a `Vec` with a `#[cold]` growth path, and it is allocated from the
  old semispace if it fits, else from `malloc`.** Sizing: reserve `min(live_estimate / 64, 1 MiB)`
  elements, where `live_estimate` is the previous collection's survivor count. A 1 MiB gray stack
  is 128 K entries, which is [E] 40× the object count of a 4 MiB semispace at 2% survival, so
  overflow is rare; if it does overflow, the `Vec` grows (a `realloc`, an [E] 50 µs) and
  `zen-bench gc_micro` measures it.

The three lines that reviewers should interrogate:

1. **`FORWARDED` is bit 0 of the first payload word, and a live object might have an integer
   there.** A `tuple` of one `int` has its payload at `+16` and that `Value`'s low bit is
   arbitrary. **Solution: the forwarding word is only ever *read* as a forwarding pointer if the
   object is in the *old* semispace, and the collector knows that** — the old semispace is
   identified by its base and extent, which the collector holds. An object in the *new* semispace
   is never asked whether it is forwarded. This is the reason the design is two-space and not
   mark-sweep-in-a-half-space. It is a real invariant and §9 test 7 asserts it with a tuple of
   `int`s whose low bit is 1.
2. **The evacuation loop must not run out of to-space mid-collection** (§7.4 grows it, and the
   `bump` returning `Err` is the one `Err` path in the collector that is *not* OOM: it means "no
   semispaces left to grow to", which is a genuine OOM).
3. **Interior pointers must be fixed up.** If any root or interior field is an `Interior` word
   (§6.2), evacuation must rewrite it. For Zen's own types the interior-pointer cases are the
   `Arc<str>` body pointer and the `ObjHeader` field of a Rust struct — but the *`Value`* stored
   in a slot is always a header pointer (`Interior(0)`), and the `Arc<str>` body pointer is never
   stored in a `Value`, only inside an `Arc` that the collector does not scan. **So M9's fixup set
   is empty for Zen's own types**, and this is a simplification worth stating: the collector never
   has to update a `Value`. It is `Interior` only for the `native_owned` objects of §7.8, which are
   pinned and therefore never evacuated.

The main loop:

```rust
fn collect(&mut self, trace: &dyn Trace) -> Result<(), GcError> {
    self.flip();                                  // §7.4
    let mut gray: Vec<NonNull<u8>> = Vec::new();

    // Phase 1: evacuate from the roots. This IS the mark phase; there is no
    // separate marking, and `marks` is never set (§7.3).
    for r in trace.roots() {                      // §7.2, 18 rows
        match trace.trace_word(r) {
            Word::NotPointer    => {}
            Word::Object(o)     => gray.push(self.evacuate(o, trace)),
            Word::Interior(o)   => gray.push(self.evacuate(o, trace)), // pinned → no-op
        }
    }

    // Phase 2: transitive closure, breadth-first, iterative (never recursive
    // — a 10-deep linked list must not overflow the Rust stack).
    while let Some(o) = gray.pop() {
        let kind = unsafe { *o.as_ptr() };
        trace.push_children(o, kind, trace);      // pushes evac'ed children
    }

    // Phase 3: sweep. The survivors are the new from-space's bump pointer;
    // the old semispace is entirely garbage and is handed back. There is no
    // free list and nothing to clear — including `marks`.
    self.bump_ptr = self.to_used;
    self.live_bytes = self.to_used - self.to_base;
    Ok(())
}
```

**Sweep is one assignment.** That is the entire appeal of semispace collection, and it is the
reason it is chosen over mark-compact despite moving objects: the copy is the expensive part and
both pay it, but semispace's *sweep* is free and mark-compact's is a second pass with a
sliding/threading step.

### 7.6 Threading: one shared heap, STW per thread, and why not per-thread heaps

**Ruling: a single process-wide heap. Collection is stop-the-world *for the thread that triggers
it*, and every other thread must be at a safepoint before the collection may begin.** The
justification is in three parts.

1. **M10's scheduler is a work-stealing pool, and objects must be shareable.** Row 16 of §7.2
   (a blocked native frame parked in a `channel`) means a `Value` created on thread 3 can be
   referenced by a frame on thread 1. With per-thread heaps, that requires a cross-thread
   promotion protocol: you cannot know at allocation time whether an object will escape, and
   promoting afterwards is a full collection's worth of work plus write barriers. A shared heap
   makes the question disappear.
2. **A per-thread heap plus a shared old generation is the design that does not need write
   barriers** (minor collections only touch one thread's nursery; objects are promoted by
   copying on first survival, and only *then* need the old generation to be multi-threaded). That
   design is better than both alternatives, and it is **not** being chosen in M9, because it needs
   generational promotion logic and a per-thread root enumeration, and the M9 gate does not ask
   for it. §7.1's table says: revisit only on post-M9 profile evidence.
3. **The safepoint machinery is already being built for the JIT.** [`08-vm.md`](08-vm.md) §7.6
   has `SAFEPOINT` in the bytecode, and [`10-jit.md`](10-jit.md) §7 has the frame table. The
   protocol is therefore already a hard requirement of the project, and the collector is a second
   consumer of a mechanism that exists. **This is the single strongest argument for building the
   collector at M9 rather than M10**: the JIT needs safepoints, so the trigger mechanism is not
   optional extra work.

The protocol, and what makes it hard:

```text
Thread A (the collector trigger)               Thread B (every other thread)
─────────────────────────────────────           ───────────────────────────────────
1. publish `COLLECTING` in a global AtomicUsize with Release
2. set `thread.state = STW_REQUESTED`
3. loop: signal each thread's safepoint flag
                                                W: at the top of dispatch, or in
                                                   jit_safepoint, observe the flag
                                                W: set `thread.state = AT_SAFEPOINT`
                                                W: push its root list (registers,
                                                   native frames, spill)
4. spin-wait for every thread AT_SAFEPOINT
   with a bounded timeout → if a thread does not
   arrive in 1 s, `panic!("thread {} did not reach a safepoint")`
                                                ... a Zen process cannot panic
                                                ... (§08-vm.md §4): this is a Rust
                                                ... panic in trusted host code, and
                                                ... it aborts with a clear message
5. run §7.5's collect() with the union of all root lists
6. publish `IDLE`, store Release
                                                W: resume
```

**The 1-second timeout is a hard rule, not a heuristic.** A thread stuck in a long native call
must not deadlock the whole process; the alternative is a *non-cooperative* point, which does not
exist for a thread inside a C library. Therefore:

- Any FFI call into a native library that can block for an unbounded time **must not** be in
  `AT_SAFEPOINT` state, and therefore cannot have its frame roots in the collector's root list.
  Instead the FFI shim **retains** every `Value` it holds across the call (§7.7) and the
  `Value`s it holds are pinned as reachable via the frame's `Value` copy. This is the one place
  where "collect roots" and "the GC moves objects" genuinely conflict, and the resolution is that
  **M9's collector does not move pinned objects** (§7.8), so a pinned frame's `Value`s stay valid
  whether or not they are in the root list. **That is the safety argument for pinning, and it is
  the reason the collector is STW rather than concurrent**: STW lets a blocking native frame be
  *absent* from the root list without any risk, because it is not scanned and it does not move.

### 7.7 How `Arc` survives a tracing collector

This is the question a reviewer will ask first, and the answer is a constraint, not a mechanism.

> **`zen-alloc` may not depend on `std::sync::Arc` semantics, and `Arc`'s refcount cell may not
> move between semispaces while another thread might be reading it.**

Two options, and the ruling is the first:

**Option A (adopted): the collector understands `Arc` allocations, and pinned objects are not
moved.** `Arc<str>`/`Arc<[u8]>` are allocated in the **old generation**, not the nursery, and the
old generation is non-moving. A nursery-allocated `ListObj` is evacuated, and its payload is
`Vec<Value>`, which is a separate allocation that also lives in the old generation. So **the only
pointers into the nursery are `Value`s' header pointers**, and the only interior pointers are into
non-moving allocations. Pinned/native objects are never evacuated. Everything follows.

**Option B (rejected): treat `Arc` as opaque and root every `Arc` allocation forever.** Leaks
every string and every error message, which is most of the program. Not viable.

**Option C (rejected): replace `Arc` with a GC-managed handle and keep a weak count only.** That is
a different ownership model — effectively M10's plan — and it contradicts ADR-0002's ruling that
M9 changes the *collector*, not the *type representation*.

**Option A's rule, stated for the reviewer: a `Value` is either nursery (movable, traced) or
pinned (immovable, rooted); there is no third case; and an `Arc`'s two refcount words are the only
non-`Value` interior pointers in the system, and they never move.** The `Arc` refcount decrement in
`release` therefore needs no read barrier and no fixup, because the cell is in a non-moving
allocation.

**What this does to `retain`/`release`:** nothing observable, and that is the point. M9's
collection cost is *in addition to* the refcount traffic, which is the honest reason the M9 gate
exists. The language could remove refcounting entirely in M10 (tracing is enough to keep objects
alive; the *sharing* semantics of
[ADR-0003](decisions/ADR-0003-concurrency-semantics.md) come from the collector, not the
refcount), but that changes `Value`'s semantics and is a separate ADR.

### 7.8 Pinning, and the four `unsafe` ways to get it wrong

| Object | Why it cannot move | How |
|---|---|---|
| A `Value` held in a blocked native frame (§7.2 row 16) | the FFI shim's `&mut Vm` is not available to the collector while the native call is in progress | mark the object `PINNED` at the moment the shim retains it |
| An `ffi` object whose raw pointer was handed to C | C holds a pointer it will dereference later, with no way to be told about a move | `NATIVE_OWNED`; `ffi` is declared in the FFI section, and a non-`movable` `ffi` is a documented restriction |
| An object passed to a `#[repr(C)]` struct by pointer | same | same |
| A nursery object that a **finalizer** references | the finalizer queue is a root (row 17) and the queue holds `usize` pointers | the finalizer's target is pinned while the finalizer is pending; §7.9 |

Pinned objects live in a **non-moving old generation** and are allocated out of the nursery. So
pinning is not "mark it and hope": the allocation path has a branch — `if pinned { old_gen_alloc() }
else { bump() }` — and the old generation is a normal malloc'd region with a free list. **The old
generation is the one place a free list appears in this design, and it appears only because pinning
requires immovability.** That is a fair price, and it is [§7.1's](#71-the-design-in-one-paragraph)
mark-sweep rejection stated precisely: the blueprint rejected mark-and-sweep *for the whole heap*;
this design uses a bump allocator for the nursery, which is the part that matters for the
allocation fast path.

The four ways to get it wrong, and the mitigation for each:

1. **Unpinning while a native frame still holds the pointer.** Mitigation: `FINALIZER_DONE` and
   `NATIVE_OWNED` are only cleared by the owning shim, and both are `debug_assert!`ed against
   double-clear.
2. **A `ffi` object allocated in the nursery before the shim pinned it.** The object may already
   have moved once. Mitigation: `ffi` objects are allocated with `old_gen_alloc` **at construction**,
   never bumped. The FFI `alloc` is therefore a different function and has a `#[cfg]`-independent
   identity so a review grep can find it.
3. **A `Value` promoted to pinned while a raw pointer to its payload is in a register.** Mitigation:
   pinning happens only at FFI boundaries, where the shim re-derives every pointer from the `Value`
   it just pinned, and `08-vm.md` §8.1's v1/v2 ABI hands the native side a `*const u8` derived at
   that moment. The JIT must not cache a `Value`-derived pointer across a call either, and
   [`10-jit.md`](10-jit.md) §7.2 makes that a rule.
4. **A pinned object that is itself a container holding movable children.** A pinned `list` is fine:
   its *elements* are `Value`s, and the collector reaches them through the pinned list as a root
   and evacuates them normally. Mitigation: `PINNED` propagates to nothing. This is a point a
   reviewer may get wrong in the other direction, so it is a test (§9 test 8).

### 7.9 Finalizers

**Ruling: M9 has no finalizers, and their absence is a documented limitation, not a gap to be
closed in the same milestone.** The reasons:

- A finalizer needs `__del__` semantics, and Zen has no `__del__`. Adding it in the same
  milestone as a collector means the collector is correct *and* observable, and the two failure
  modes (running a finalizer on a dead object; not running it) are both user-visible.
- Finalizers require either a thread (a "finalizer thread") or interleaving them into the mutator,
  and the second is unsound without a write barrier.

What M9 ships instead, and it is a real answer:

- **`ffi` destructors are explicit.** A native resource is released by a `close` builtin, which
  the manual documents as "not called on an abnormal exit; use `defer`". The `defer` statement in
  [`07-bytecode.md`](07-bytecode.md) §6 is the *only* cleanup mechanism in M9, and it is
  stack-scoped and therefore needs no collector support at all.
- The finalizer machinery is *designed* — row 17, the two `marks` bits, the `FinalizerId` in
  `Trace::finalizer` — so that M10 can add it without changing the collector's structure. §7.3's
  bit table is the design; the implementation is M10.
- A finalizer test in §9 asserts the **negative**: an object with a pending finalizer target is
  *not* collected early and *not* collected twice. That is the property M10 needs, and it is
  testable with the bits set manually.

### 7.10 How this could still be the wrong answer

Honest failure modes, each with the measurement that would detect it and the response:

| Failure mode | How it would show up | Response |
|---|---|---|
| **The M9 gate fires and the collector does not help** | `zen-perf` still shows `release`/`retain` on the critical path after the gate | The gate's premise is that refcount *traffic* (atomics + cache-line ping-pong) is the cost, not memory. If the collector removes the traffic, the cost was elsewhere. **Conclusion: the ownership model is wrong, not the collector.** M10 then considers a tracing-only model (no refcount), which is a bigger ADR. |
| The nursery is the wrong size | `zen-bench gc_micro` shows either 100× the collections a same-workload CPython run does (too small) or a 32 MiB RSS for a 100 KB script (too big) | Retune; it is a constant and a `--gc-nursery` flag behind `gc-stats` |
| Marking is too slow because of the `DictObj` hash-table internals | `gc_micro` with dict-heavy allocation shows evacuation dominating | `Trace::push_children` for `DictObj` becomes an iteration over the table's entries — it already is; the cost is that a dict's *capacity* exceeds its *length* and the collector must scan the empty slots to find the entries. A `IndexMap` with tombstones makes this worse. Mitigation: an entry-count-based iteration is already O(len); if a profile says otherwise, the dict gets a dense entry vector (an M5 design change, not a collector change) |
| The single generation cannot keep a 500 MB live set | A large-`list` benchmark OOMs at 64 MiB per semispace | The cap is a constant; a large-object path (objects > 1/4 semispace are allocated in the old generation directly) is the standard fix and is ~30 lines |
| Pinning is too common | RSS grows because every `ffi` value is old-generation | That is correct behaviour; the fix is documentation, not code |
| The STW timeout fires spuriously | A `panic!` under a long native call | The `ffi`/blocking-call path must not be in `AT_SAFEPOINT`; §7.6 |

**And the case where the whole section is wasted: the gate does not fire.** Then M9 does not happen,
§7 is a design document nobody implements, and the project spent nothing on it except this file.
That is the intended risk profile, and it is why the whole section is behind one `#[cfg]`.

---

## 8. Memory safety rules and the unsafe allow-list

### 8.1 The complete `unsafe` list for M2–M8

Nine items, all in `zen-value` / `zen-vm/src/value.rs`, listed in §3.3. The CI assertion is §8.3.

### 8.2 Rust's own invariants, and where they are relied upon

Three places where correctness depends on a `std` guarantee rather than on our code. Each has a
test.

| Relied-upon guarantee | Used in | Test |
|---|---|---|
| `AtomicU32` and `u32` have identical size, alignment, and in-memory representation | `rc_ptr` (§3.2) | 9 |
| `ArcInner<str>` places `len` immediately before the bytes, and `Arc::into_raw` on `Arc<str>` returns a fat pointer whose data half is that address | `str_len` (§3.2) | 9 |
| `Arc::from_raw` on the reconstructed fat pointer is the documented inverse of `Arc::into_raw` | `str_from_raw` (§3.2) | 9, 10 |
| `Layout::extend` places the array at exactly the offset the padding computation yields | `new_tuple` (§2.5) | 3 |

None of these are "probably fine". `ArcInner` is `#[repr(C)]` in std, its layout is documented as
unspecified-but-stable, and the test is what turns an assumption into a checked fact on every
platform the project ships on. **If test 9 ever fails, §3's `string` representation is wrong and
the fix is a `#[repr(C)]`-based private struct, not a `.unwrap()`** — std's offset is a *layout*
detail, and copying it into our own type is the only way to be immune to a std change.

### 8.3 What CI enforces

| Check | Command | Fails on |
|---|---|---|
| `unsafe` count | `git grep -c unsafe -- zen-value/src/value.rs` ≠ 9 | a new `unsafe` block without an entry in §3.3 |
| Direct register writes | `grep -nE 'registers\[[^]]+\]\s*=[^=]' zen-vm/src/vm.rs` | a leak (§3.4) |
| `const` layout assertions | `cargo test -p zen-value layout` | an `ObjHeader` field reorder |
| `no_panic` | `cargo build -p zen-vm --profile release` + the Miri smoke test | a panic on the dispatch path |
| Miri on the memory core | `cargo +nightly miri test -p zen-value` | any UB in the nine `unsafe` items |
| Collector behind a feature | `cargo tree -p zen-vm --no-default-features` must not contain a `TracingGc` symbol | a stray collector in an M2–M8 build |

Miri's role here is specific and worth stating: the nine `unsafe` items are exactly the code Miri
is good at checking (aliasing, provenance, use-after-free) and exactly the code a unit test with
`assert_eq!` is bad at. **§9's Miri job is not a smoke test, it is the primary correctness argument
for §3.2.** It is also why `docs/09` is 1 700 lines: nine `unsafe` items with no Miri coverage
would be nine ways to have a `Value` that is not a `Value`.

---

## 9. Test plan

Nineteen tests. Each names the failure it is for, because "test the CoW" is not a test.

| # | Test | Fails if | How |
|---|---|---|---|
| 1 | `layout` | `ObjHeader` is not 16 bytes / 8-aligned / the asserted offsets shift | the `const` block in §2.1, run as a test |
| 2 | `value_roundtrip` | any `Value` bit pattern is not preserved, or an i48 round-trip changes value | exhaustive-ish table of `(kind, payload)` |
| 3 | `tuple_layout` | `Layout::extend` puts the payload anywhere but `+16`, or `len` is wrong | allocate `new_tuple(&[1,2,3])`, read slots at `+16/+24/+32` |
| 4 | `refcount_lifetime` | a `retain` is missing (leak) or doubled (double-free) | build/drop 10⁶ `list`s under Miri; `LIVE_BYTES` must return to its baseline |
| 5 | `list_edges` | a `Vec` capacity, a `pop`, or an `iter().rev()` leaks | drop at every length 0..=64; `LIVE_BYTES` back to baseline each time |
| 6 | `dict_key_leak` | `DictObj::drop` releases values but not keys | `d["a"] = "b"; drop(d)`; both refcounts must be back to 1 |
| 7 | `gc_forwarding_lowbit` | the `FORWARDED` bit test reads a live `int` payload | a nursery `tuple` of `int`s whose low bit is 1; the collector must not treat it as forwarded |
| 8 | `gc_pinned_container` | a pinned `list` is not evacuated, or its children are not | a `ffi` list with a `list` element; assert the child's address changed and the parent's did not |
| 9 | `arc_layout` | `str_len` / `bytes_len` is wrong, or `Arc<u32>`-style layout assumptions break | `assert_eq!(unsafe { str_len(p) }, s.len())` for 0..=64 byte strings on every supported target, in CI, and `Arc::strong_count` agrees with our `rc` after N retains |
| 10 | `arc_roundtrip` | `from_raw` is not the inverse of `into_raw` | `str_from_raw(str_to_value(Arc::from("x")))` equals `"x"`; `Arc::strong_count` never goes negative under Miri |
| 11 | `cow_mut_unique` | `cow_mut` clones when `rc == 1`, or mutates in place when `rc > 1` | assert pointer identity in both directions |
| 12 | `handle_weak` | a weakly-held handle's object dies with the handle, or survives it | `h = zen.intern(d); del d; gc()` (gc build) — `h` must raise, not segfault |
| 13 | `cycle_leak_is_bounded` | a cycle leaks *unboundedly* rather than by a fixed amount | a self-referential list, one million `append`s, RSS must grow by one cycle's worth and stop |
| 14 | `interner_roots` | the string interner (root row 8) is missed, so interned strings are collected | intern a string, drop all `Value`s, collect, compare interned pointers — must be identical |
| 15 | `devirt` | LTO failed to devirtualise `gc().alloc` | `nm`/`objdump` the release binary: no indirect call in `alloc_value`, `RefCountGc::alloc` inlined |
| 16 | `string_build_gate` | §5.5's gate: a 200 000-iteration accumulator is quadratic | time the loop at 20 000 / 40 000 / 80 000 / 160 000 / 200 000 iterations; the ratio must be linear within 3.0× per doubling. **Fails by design until §5.5; it is the visible, tracked failure that justifies the M11 amendment** |
| 17 | `stress_shapes_cow` | CoW and shape transitions interact wrongly | 10⁶ iterations of `d2 = d; d2[k] = v` over 8 keys; every object must stay monomorphic (shape count == 8) and the dict must be correct |
| 18 | `miri_memory_core` | any UB in the nine `unsafe` items | `cargo +nightly miri test -p zen-value` in CI, on every PR |
| 19 | `gc_stw_timeout` | a thread that never reaches a safepoint hangs forever instead of aborting | spawn a thread blocked in a native call, trigger a collection, assert the 1 s timeout aborts with the documented message |

Tests 7, 8, 12, 14, 19 are `#[cfg(feature = "gc")]` and only run under `--features gc`. Tests
1–6, 9–11, 13, 15–18 run in both configurations, which is the point of §6.4's single test suite.

---

## 10. Metrics

| Metric | Where | Kind | Target |
|---|---|---|---|
| `gc.alloc_ns` | `alloc_value`, `RDTSC` deltas, 4096-entry histogram | [T] | p99 < 25 ns in M2–M8. The path is: tag compare, one `bump`, six stores to a fresh cache line, done. A `malloc` instead of a bump is [E] 40 ns and would be visible. |
| `gc.copies` | `retain`/`release` counts, `#[cfg(feature = "gc-stats")]` | [T] | **the M9 gate**: `copies / total_allocs` on `zen-bench fib` and `zen-bench json_parse` |
| `gc.refcount_contention` | `fetch_add` CAS-failure counts per cacheline | [T] | < 1% on a 4-thread `zen-bench` |
| `gc.live_bytes` | `LIVE_BYTES` | [M] in debug | exact; `zen.mem_stats()` builtin |
| `gc.nursery_bytes` | gc build only | [T] | 4 MiB per semispace |
| `gc.collections` | counter, gc build only | [T] | 0 for a program under 10⁶ allocations ([E]: §7.4) |
| `gc.survival_rate` | survivors / evacuated | [T] | < 5% ([E] from CPython's < 1%) |
| `gc.gray_stack_peak` | `Vec` capacity high-water | [T] | ≤ 1 MiB (§7.5) |
| `gc.stall_us` | wall time inside `collect()` | [T] | p99 < 100 µs ([E]: 80 KiB copied at 3 GB/s = 27 µs) |
| `gc.stw_timeout` | counter | — | must be 0; any non-zero is a bug (§7.6) |
| `gc.str_build_ns` | `zen-bench string_build` | [T] | §5.5's gate: 200 000 iterations within 3× of CPython 3.13 |
| `mem.live_bytes` | `gc().live_bytes()` | [M] | monotonic non-decreasing under M2–M8; a decrease is a bug (an over-release) |

The M9 gate itself, stated as the rule that opens the milestone:

> **Open M9 if and only if `gc.copies / total_allocs` exceeds 5% on `zen-bench fib` or
> `zen-bench json_parse` in a release build with `gc-stats`, measured on the reference machine.**

5% is chosen so it is *hard to hit accidentally*: a program that is already sharing its containers
sensibly — the property ADR-0002 is buying — will not cross it, while a program that repeatedly
passes and mutates a large container will clear it by an order of magnitude. §9 test 16 and
`gc.copies` are the two numbers a reviewer should watch between M4 and M9.

---

## 11. Rulings a reviewer should double-check

Ordered by "how wrong could this be, and how late would we find out".

| # | Ruling | Where | The reviewer question | Cost if wrong |
|---|---|---|---|---|
| 1 | **`ObjHeader.rc` is a manual `u32`, not `Arc`'s** | §3.2, §4.6 | ADR-0002 says `Arc::make_mut`; 08-vm.md says manual refcount. Are we allowed to amend an ADR to keep the *decision* and change the notation? | Low technically, high procedurally: it is a `.zeno` format change if done after M2 |
| 2 | **ADR-0001's tags beat 08-vm.md §8.2's illustrative tags** | §2.4 | Is the blueprint's illustrative `ValueRepr` authoritative? If yes, six tags change and the GC mark bit has nowhere to live | Low: it is one table |
| 3 | **String `+` is O(n²) until M11, knowingly** | §5 | Is accepting a 25–50× CPython gap in string building acceptable for M2–M8? The alternative is a rope in M2 | High if the user disagrees: it is a visible language defect with a known fix that is a large change |
| 4 | **`new_tuple`'s `.expect()` is unsound under the no-panic rule** | §2.5, §6.1 | Should `alloc`-taking constructors return `Result`, or is the tuple bound by the operand stack enough to prove infallibility? | Medium: a panic path that 08-vm.md §4 forbids |
| 5 | **The `Gc` selector is a cargo feature, because a runtime selector costs a `Value` bit** | §6.4 | Is spending a tag on "which collector" worse than spending one on a growth feature? | Low, and the argument is a constraint (3 bits), not a preference |
| 6 | **One shared heap, STW, 1-second timeout that aborts** | §7.6 | Is aborting the process the right response to a stuck thread, or should it continue without that thread's roots (unsound) or spin forever (worse)? | Medium: a real deadlock risk if a native call is long and uncooperative |
| 7 | **The M9 collector is single-generation, not generational** | §7.1 | Does a script workload survive one generation? | Medium: a re-design, though the layout does not change |
| 8 | **Pinned objects live in a malloc'd old generation with a free list** | §7.8 | Is a free list in the design acceptable, given §7.1 rejected mark-and-sweep? The answer is "only in the pinned old generation"; is that convincing? | Low: it is scoped |
| 9 | **M9 has no finalizers; `defer` and `close` are the mechanism** | §7.9 | Is a language without `__del__` shippable, or does the collector milestone need it? | Medium: user-visible, but deferrable |
| 10 | **The full M9 collector costs ~400 lines plus §7.2's root map** | §7 | Is the estimate honest? The root map is 19 rows and each needs a `Trace` impl and a test | High if low: M9 slips by a milestone |
| 11 | **Cycles leak, knowingly** | §4.5 | Is a documented bounded leak acceptable in a scripting language, or does it need `weakref`/`cycle` handling before release? | Low-medium: user-visible, documented, testable |
| 12 | **The nine-`unsafe` list is complete and stays complete** | §3.3, §8.3 | Would a reviewer accept a grep-based `unsafe` count as a CI gate? | Low: it is a tripwire, not a proof; Miri is the proof |

**Cross-document action items this document creates:**

1. **Amend ADR-0002** to say `cow_mut` with a manual `rc` in place of `Arc::make_mut`, and to
   list `ObjHeader`-prefixed payloads instead of `Arc<Vec<Value>>`. *Before M2.*
2. **Correct the tag comments in 08-vm.md §8.2** to ADR-0001's map, and state that GC marks live in
   `ObjHeader.marks`. *Before M2, since the tags are baked into the `Value` and the JIT.*
3. **Carry the corrected tag table into 04-value-and-types.md** when it is written, and have §2.4
   link to it.
4. **Add `BUILD_STRING n`** to [`07-bytecode.md`](07-bytecode.md) §4 for M4. *The only change
   requested to another document by §5.3.*
5. **Agree the JIT frame table with [`10-jit.md`](10-jit.md) §7** before either M11 or M9 lands:
   root row 18 depends on it.
6. **`docs/01-principles.md` and `docs/04-value-and-types.md` do not exist yet.** This document
   links to them as relative paths; whoever writes them should link back here for §2.4 and §3.1,
   and should treat §2.4's table as the canonical one until it says otherwise.

---

*Related: [`00-blueprint-review.md`](00-blueprint-review.md) §A2 (memory) and §A6 (refcount/CoW) ·
[`08-vm.md`](08-vm.md) §2 (the `Vm` that §7.2 roots), §4 (no panics, OOM), §6 (ICs), §7 (shapes),
§8.2 (`ValueRepr`) · [`decisions/ADR-0001-tag-budget.md`](decisions/ADR-0001-tag-budget.md) ·
[`decisions/ADR-0002-memory-model.md`](decisions/ADR-0002-memory-model.md) ·
[`decisions/ADR-0003-concurrency-semantics.md`](decisions/ADR-0003-concurrency-semantics.md)
(deep-copy-on-send, which is what makes §7.7's sharing rules non-negotiable) ·
[`decisions/ADR-0004-honest-types.md`](decisions/ADR-0004-honest-types.md) ·
[`07-bytecode.md`](07-bytecode.md) §11 (escape analysis, §4.3's follow-up) ·
[`10-jit.md`](10-jit.md) (the other M11-gated representation change, §5.5)*
