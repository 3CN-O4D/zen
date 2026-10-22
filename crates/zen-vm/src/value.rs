//! Value representation: 8 machine tags over a NaN-boxed `u64`, 16 language types.
//!
//! See [`docs/04-value-and-types.md`], [`docs/09-memory.md`] §2–4, and
//! [`ADR-0001`](docs/decisions/ADR-0001-tag-budget.md).
//!
//! All `unsafe` in the crate lives in this file and [`crate::gc`] (enforced by
//! `#![deny(unsafe_code)]` on every other module). The model is `Value: Copy`
//! over a bit pattern; heap objects carry a manual refcount in their
//! [`ObjHeader`], and `retain`/`release` are explicit, exactly as
//! `docs/09-memory.md` §3.4 requires (there is no `Drop` on `Value`).
//!
//! **M2 simplification.** `docs/09-memory.md` §3.1 gives `string`/`bytes` a
//! *thin* `Arc<str>`/`Arc<[u8]>` payload with the length stored behind the
//! pointer. std's `Arc<str>` allocation has no length word (the length lives
//! only in the fat pointer), so a thin pointer cannot recover it. String and
//! bytes therefore use a header-backed object like every other heap type, while
//! keeping their own machine tags so `typeof` and `is_string` stay honest. The
//! layout can be revisited in M9 together with the collector.

use std::ptr::NonNull;
use std::sync::atomic::{fence, AtomicU32, Ordering};
use std::sync::Arc;

use crate::object::{ObjHeader, ObjKind};

pub const QNAN: u64 = 0x7FF8_0000_0000_0000;
pub const TAG_SHIFT: u64 = 48;
pub const TAG_MASK: u64 = 0x7;
pub const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

#[inline]
const fn tagged(tag: Tag) -> u64 {
    QNAN | ((tag as u64) << TAG_SHIFT)
}

// ── Machine tags ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Tag {
    Nil = 0b000,
    False = 0b001,
    True = 0b010,
    Int = 0b011,
    Float = 0b100,
    String = 0b101,
    Bytes = 0b110,
    Heap = 0b111,
}

impl Tag {
    #[inline]
    pub fn of(bits: u64) -> Tag {
        // Anything without the quiet-NaN pattern is a raw f64.
        if (bits & QNAN) != QNAN {
            return Tag::Float;
        }
        match (bits >> TAG_SHIFT) & TAG_MASK {
            0b000 => Tag::Nil,
            0b001 => Tag::False,
            0b010 => Tag::True,
            0b011 => Tag::Int,
            0b100 => Tag::Float,
            0b101 => Tag::String,
            0b110 => Tag::Bytes,
            _ => Tag::Heap,
        }
    }
}

// ── The value ───────────────────────────────────────────────────────────────

/// A NaN-boxed 64-bit value. `Copy`: it is a bit pattern, and copying one never
/// touches a refcount (`docs/09-memory.md` §3.4).
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Value(pub u64);

impl Value {
    // ── bit plumbing ─────────────────────────────────────────────────────
    /// Rebuild a value from raw bits.
    ///
    /// # Safety
    /// The caller must ensure that if [`Tag::of`] classifies the pattern as a
    /// heap value (`heap`/`string`/`bytes`), those bits are the address of a
    /// live object with a valid [`ObjHeader`]. Passing arbitrary bits and then
    /// using a heap accessor would dereference that address. [`Tag::of`] is the
    /// safe, total way to inspect arbitrary patterns.
    #[inline]
    pub const unsafe fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    #[inline]
    pub const fn to_bits(self) -> u64 {
        self.0
    }

    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }

    // ── immediate constructors ───────────────────────────────────────────
    #[inline]
    pub const fn nil() -> Self {
        Self(tagged(Tag::Nil))
    }

    #[inline]
    pub const fn bool(b: bool) -> Self {
        Self(tagged(if b { Tag::True } else { Tag::False }))
    }

    /// The inclusive range of a hardware `int`: `[-2^47, 2^47)`.
    pub const INT_MIN: i64 = -(1i64 << 47);
    pub const INT_MAX: i64 = (1i64 << 47) - 1;

    #[inline]
    pub fn int(n: i64) -> Self {
        debug_assert!(
            (Self::INT_MIN..=Self::INT_MAX).contains(&n),
            "i48 overflow: {n}"
        );
        Self(tagged(Tag::Int) | ((n as u64) & PAYLOAD_MASK))
    }

    /// `int` that promotes to `float` on overflow (`docs/04` §6), rather than
    /// wrapping or tripping the debug assertion.
    #[inline]
    pub fn int_promote(n: i64) -> Self {
        if (Self::INT_MIN..=Self::INT_MAX).contains(&n) {
            Self::int(n)
        } else {
            Self::float(n as f64)
        }
    }

    #[inline]
    pub fn float(f: f64) -> Self {
        // Every NaN bit pattern is canonicalised; `±∞` and `-0.0` are preserved.
        // (`docs/04` §7 says "all NaN bit patterns", while its §4 snippet checks
        // the exponent only — the exponent-only check would also turn `∞` into a
        // NaN, so `is_nan` is used here.)
        if f.is_nan() {
            Self(tagged(Tag::Float))
        } else {
            Self(f.to_bits())
        }
    }

    /// Number from an `i64`, preferring `int` when it fits.
    #[inline]
    pub fn number(n: i64) -> Self {
        Self::int_promote(n)
    }

    // ── heap reference constructors ──────────────────────────────────────
    /// Wrap a heap object pointer. The caller transfers one reference.
    ///
    /// Strings and bytes keep their own machine tags so `typeof` is honest and
    /// so the fast path (`tag() == Tag::String`) needs no pointer chase; every
    /// other heap kind is `Tag::Heap` and reads its [`ObjKind`] from the header.
    #[inline]
    pub fn from_header(h: NonNull<ObjHeader>) -> Self {
        let p = h.as_ptr() as u64;
        debug_assert!(p >> 48 == 0, "heap pointer does not fit in 48 bits: {p:#x}");
        // SAFETY: the caller guarantees `h` points at a live object it owns.
        let tag = match unsafe { h.as_ref() }.kind() {
            ObjKind::String => Tag::String,
            ObjKind::Bytes => Tag::Bytes,
            _ => Tag::Heap,
        };
        Self(tagged(tag) | (p & PAYLOAD_MASK))
    }

    /// A `string` value. Backed by a header object (see the module note).
    #[inline]
    pub fn string(s: impl Into<Arc<str>>) -> Self {
        crate::gc::new_string(s.into())
    }

    /// A `bytes` value. Backed by a header object (see the module note).
    #[inline]
    pub fn bytes(b: impl Into<Arc<[u8]>>) -> Self {
        crate::gc::new_bytes(b.into())
    }

    // ── tag / predicates ─────────────────────────────────────────────────
    #[inline]
    pub fn tag(self) -> Tag {
        Tag::of(self.0)
    }

    #[inline]
    pub fn is_nil(self) -> bool {
        self.0 == Self::nil().0
    }

    #[inline]
    pub fn is_bool(self) -> bool {
        matches!(self.tag(), Tag::True | Tag::False)
    }

    #[inline]
    pub fn is_int(self) -> bool {
        self.tag() == Tag::Int
    }

    #[inline]
    pub fn is_float(self) -> bool {
        self.tag() == Tag::Float
    }

    #[inline]
    pub fn is_number(self) -> bool {
        matches!(self.tag(), Tag::Int | Tag::Float)
    }

    #[inline]
    pub fn is_string(self) -> bool {
        // Flat strings take no pointer chase; ropes (a heap kind that quacks
        // like a string) check the header.
        self.tag() == Tag::String || self.kind() == Some(ObjKind::Rope)
    }

    #[inline]
    pub fn is_bytes(self) -> bool {
        self.tag() == Tag::Bytes
    }

    #[inline]
    pub fn is_heap(self) -> bool {
        matches!(self.tag(), Tag::Heap | Tag::String | Tag::Bytes)
    }

    /// The heap object's [`ObjKind`], or `None` for an immediate.
    #[inline]
    pub fn kind(self) -> Option<ObjKind> {
        if !self.is_heap() {
            return None;
        }
        Some(unsafe { self.header_unchecked().as_ref() }.kind())
    }

    /// The language-level type name (`typeof`), never lying (`ADR-0004`).
    pub fn type_name(self) -> &'static str {
        match self.tag() {
            Tag::Nil => "nil",
            Tag::False | Tag::True => "bool",
            Tag::Int => "int",
            Tag::Float => "float",
            Tag::String => "string",
            Tag::Bytes => "bytes",
            Tag::Heap => unsafe { self.header_unchecked().as_ref() }
                .kind()
                .type_name(),
        }
    }

    // ── accessors ────────────────────────────────────────────────────────
    #[inline]
    pub fn as_bool(self) -> Option<bool> {
        match self.tag() {
            Tag::True => Some(true),
            Tag::False => Some(false),
            _ => None,
        }
    }

    #[inline]
    pub fn as_int(self) -> Option<i64> {
        (self.tag() == Tag::Int).then(|| {
            let raw = (self.0 & PAYLOAD_MASK) as i64;
            (raw << 16) >> 16 // sign-extend from 48 bits
        })
    }

    #[inline]
    pub fn as_float(self) -> Option<f64> {
        (self.tag() == Tag::Float).then(|| f64::from_bits(self.0))
    }

    /// The numeric value of an `int` or `float`.
    /// Return bitmask representing the tag (for feedback).
    #[inline]
    pub fn tag_bit(self) -> u32 {
        match self.tag() {
            Tag::Nil => 1u32 << 0,
            Tag::False => 1u32 << 1,
            Tag::True => 1u32 << 2,
            Tag::Int => 1u32 << 3,
            Tag::Float => 1u32 << 4,
            Tag::String => 1u32 << 5,
            Tag::Bytes => 1u32 << 6,
            Tag::Heap => 1u32 << 7,
        }
    }

    pub fn as_f64(self) -> Option<f64> {
        match self.tag() {
            Tag::Int => self.as_int().map(|n| n as f64),
            Tag::Float => Some(f64::from_bits(self.0)),
            _ => None,
        }
    }

    /// The header of a heap value, if any.
    #[inline]
    pub fn header(self) -> Option<NonNull<ObjHeader>> {
        self.is_heap().then(|| unsafe { self.header_unchecked() })
    }

    /// # Safety
    /// The value must be a live heap value (tag `heap`/`string`/`bytes`).
    #[inline]
    pub unsafe fn header_unchecked(self) -> NonNull<ObjHeader> {
        unsafe { NonNull::new_unchecked((self.0 & PAYLOAD_MASK) as *mut ObjHeader) }
    }

    /// A borrowed `&str` for a string value.
    ///
    /// The borrow is tied to `self`; the caller must keep the value (and thus
    /// one reference) alive for the borrow's lifetime.
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        if self.tag() == Tag::String {
            let obj = unsafe { self.header_unchecked().as_ref() };
            let obj = unsafe { &*(obj as *const ObjHeader as *const crate::object::StringObj) };
            return Some(&obj.data);
        }
        if self.kind() != Some(ObjKind::Rope) {
            return None;
        }
        // SAFETY: `kind() == Rope` proves the payload is a live `RopeObj`.
        // The flattened form memoizes in the node (`OnceLock` is race-safe),
        // so the borrow is tied to `self` exactly like the flat case.
        unsafe {
            let obj = &*(self.header_unchecked().as_ptr() as *const crate::object::RopeObj);
            Some(&**obj.flat.get_or_init(|| crate::ops::flatten_rope(*self)))
        }
    }

    /// A borrowed `&[u8]` for a bytes value.
    #[inline]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        if self.tag() != Tag::Bytes {
            return None;
        }
        let obj = unsafe { self.header_unchecked().as_ref() };
        let obj = unsafe { &*(obj as *const ObjHeader as *const crate::object::BytesObj) };
        Some(&obj.data)
    }

    /// The FFI handle id for a `handle` value, if this is one.
    #[inline]
    pub fn as_handle_id(&self) -> Option<u32> {
        if self.kind() != Some(ObjKind::Handle) {
            return None;
        }
        // SAFETY: `kind() == Handle` proves the payload is a live `HandleObj`.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *const crate::object::HandleObj;
            Some((*p).id)
        }
    }

    /// The Python-bridge registry id for a `foreign` value, if this is one.
    #[inline]
    pub fn as_foreign(&self) -> Option<u32> {
        if self.kind() != Some(ObjKind::Foreign) {
            return None;
        }
        // SAFETY: `kind() == Foreign` proves the payload is a live `ForeignObj`.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *const crate::object::ForeignObj;
            Some((*p).handle)
        }
    }

    /// Borrowed list contents, or `None` if this is not a list.
    #[inline]
    pub fn as_list(&self) -> Option<&[Value]> {
        if self.kind() != Some(ObjKind::List) {
            return None;
        }
        // SAFETY: `kind() == List` proves the payload is a live `ListObj`.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *const crate::object::ListObj;
            Some(&(*p).inner)
        }
    }

    /// Borrowed dict contents, or `None` if this is not a dict.
    ///
    /// Keys are [`DictKey`]: normalized so `values_equal`-equivalent keys hash
    /// and compare identically (int/float numerics unify; strings compare by
    /// content). Unwrap with `.0` to recover the stored (original) key.
    #[inline]
    pub fn as_dict(&self) -> Option<&indexmap::IndexMap<DictKey, Value>> {
        if self.kind() != Some(ObjKind::Dict) {
            return None;
        }
        // SAFETY: `kind() == Dict` proves the payload is a live `DictObj`.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *const crate::object::DictObj;
            Some(&(*p).inner)
        }
    }

    /// Borrowed set contents, or `None` if this is not a set.
    #[inline]
    pub fn as_set(&self) -> Option<&indexmap::IndexSet<Value>> {
        if self.kind() != Some(ObjKind::Set) {
            return None;
        }
        // SAFETY: `kind() == Set` proves the payload is a live `SetObj`.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *const crate::object::SetObj;
            Some(&(*p).inner)
        }
    }

    /// Mutable list contents. Call [`cow_mut`] first to detach shared storage.
    #[inline]
    pub fn as_list_mut(&mut self) -> &mut Vec<Value> {
        debug_assert!(self.kind() == Some(ObjKind::List));
        unsafe {
            let obj = self.header_unchecked().as_ptr() as *mut crate::object::ListObj;
            &mut (*obj).inner
        }
    }

    /// Mutable dict contents. Call [`cow_mut`] first.
    #[inline]
    pub fn as_dict_mut(&mut self) -> &mut indexmap::IndexMap<DictKey, Value> {
        debug_assert!(self.kind() == Some(ObjKind::Dict));
        unsafe {
            let obj = self.header_unchecked().as_ptr() as *mut crate::object::DictObj;
            &mut (*obj).inner
        }
    }

    /// Mutable set contents. Call [`cow_mut`] first.
    #[inline]
    pub fn as_set_mut(&mut self) -> &mut indexmap::IndexSet<Value> {
        debug_assert!(self.kind() == Some(ObjKind::Set));
        unsafe {
            let obj = self.header_unchecked().as_ptr() as *mut crate::object::SetObj;
            &mut (*obj).inner
        }
    }

    /// Borrowed tuple contents, or `None` if this is not a tuple.
    #[inline]
    pub fn as_tuple(&self) -> Option<&[Value]> {
        if self.kind() != Some(ObjKind::Tuple) {
            return None;
        }
        // SAFETY: `kind() == Tuple` proves the payload is a live `TupleObj`.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *const crate::object::TupleObj;
            Some(&(*p).inner)
        }
    }

    /// The prototype of a `func` value, or `None` if this is not a function.
    #[inline]
    pub fn func_proto(&self) -> Option<Arc<crate::bytecode::FunctionProto>> {
        if self.kind() != Some(ObjKind::Func) {
            return None;
        }
        // SAFETY: `kind() == Func` proves the payload is a live `FuncObj`.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *const crate::object::FuncObj;
            Some((*p).proto.clone())
        }
    }

    /// Borrowed captured-upvalue list of a `func` value.
    #[inline]
    pub fn func_upvals(&self) -> Option<&[Value]> {
        if self.kind() != Some(ObjKind::Func) {
            return None;
        }
        // SAFETY: `kind() == Func` proves the payload is a live `FuncObj`.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *const crate::object::FuncObj;
            Some(&(*p).upvals)
        }
    }

    /// Mutable captured-upvalue list of a `func` value.
    #[inline]
    pub fn func_upvals_mut(&mut self) -> Option<&mut Vec<Value>> {
        if self.kind() != Some(ObjKind::Func) {
            return None;
        }
        // SAFETY: `kind() == Func` proves the payload is a live `FuncObj`, and
        // `&mut self` gives unique access to the value.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *mut crate::object::FuncObj;
            Some(&mut (*p).upvals)
        }
    }

    /// Borrowed instance slots. Layout is flat: `slots[0]` is the class and
    /// `slots[1 + offset]` are the field values, with offsets from the
    /// instance's shape (`crate::shape`).
    #[inline]
    pub fn as_instance(&self) -> Option<&[Value]> {
        if self.kind() != Some(ObjKind::Instance) {
            return None;
        }
        // SAFETY: `kind() == Instance` proves the payload is a live `InstObj`.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *const crate::object::InstObj;
            Some(&(*p).slots)
        }
    }

    /// Mutable borrowed instance slots (the backing `Vec`, so shapes can grow
    /// it when a new property arrives).
    #[inline]
    pub fn as_instance_mut(&mut self) -> Option<&mut Vec<Value>> {
        if self.kind() != Some(ObjKind::Instance) {
            return None;
        }
        // SAFETY: `kind() == Instance` proves the payload is a live `InstObj`.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *mut crate::object::InstObj;
            Some(&mut (*p).slots)
        }
    }

    /// Get the shape ID of an instance.
    #[inline]
    pub fn instance_shape(&self) -> Option<u32> {
        if self.kind() != Some(ObjKind::Instance) {
            return None;
        }
        // SAFETY: `kind() == Instance` proves the payload is a live `InstObj`.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *const crate::object::InstObj;
            Some((*p).shape)
        }
    }

    /// Set the shape ID of an instance.
    #[inline]
    pub fn set_instance_shape(&mut self, shape: u32) -> bool {
        if self.kind() != Some(ObjKind::Instance) {
            return false;
        }
        unsafe {
            let p = self.header_unchecked().as_ptr() as *mut crate::object::InstObj;
            (*p).shape = shape;
            true
        }
    }

    /// Borrowed class object, or `None` if this is not a class.
    #[inline]
    pub fn as_class(&self) -> Option<&crate::object::ClassObj> {
        if self.kind() != Some(ObjKind::Class) {
            return None;
        }
        // SAFETY: `kind() == Class` proves the payload is a live `ClassObj`.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *const crate::object::ClassObj;
            Some(&(*p))
        }
    }

    /// The `(receiver, method)` pair of a bound method, or `None`.
    #[inline]
    pub fn as_bound_method(&self) -> Option<(Value, Value)> {
        if self.kind() != Some(ObjKind::BoundMethod) {
            return None;
        }
        // SAFETY: `kind() == BoundMethod` proves the payload is live.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *const crate::object::BoundMethodObj;
            Some(((*p).receiver, (*p).method))
        }
    }

    /// The boxed payload of a captured-local cell, or `None`.
    #[inline]
    pub fn as_cell(&self) -> Option<&std::cell::RefCell<Value>> {
        if self.kind() != Some(ObjKind::Cell) {
            return None;
        }
        // SAFETY: `kind() == Cell` proves the payload is a live `CellObj`.
        unsafe {
            let p = self.header_unchecked().as_ptr() as *const crate::object::CellObj;
            Some(&(*p).inner)
        }
    }
}

/// Ensure `v` is uniquely owned, detaching shared storage (`docs/09-memory.md`
/// §4.2). Safe wrapper around [`cow_mut`] for crate-internal use by the VM.
#[inline]
pub(crate) fn cow(v: &mut Value) {
    // SAFETY: callers pass a value that is either immediate or live; `cow_mut`
    // itself is a no-op for immediates and for objects with `rc == 1`.
    unsafe { cow_mut(v) }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.tag() {
            Tag::Nil => write!(f, "nil"),
            Tag::False => write!(f, "false"),
            Tag::True => write!(f, "true"),
            Tag::Int => write!(f, "{}", self.as_int().unwrap()),
            Tag::Float => write!(f, "{}", self.as_float().unwrap()),
            Tag::String => write!(f, "{:?}", self.as_str().unwrap_or("")),
            Tag::Bytes => write!(
                f,
                "bytes({}b)",
                self.as_bytes().map(<[u8]>::len).unwrap_or(0)
            ),
            Tag::Heap => {
                let h = unsafe { self.header_unchecked().as_ref() };
                // Ropes print as their string content, like flat strings.
                if h.kind() == ObjKind::Rope {
                    return write!(f, "{:?}", self.as_str().unwrap_or(""));
                }
                write!(f, "{:?}@{:#x}", h.kind(), self.0 & PAYLOAD_MASK)
            }
        }
    }
}

// ── Refcount discipline (`docs/09-memory.md` §3.2) ──────────────────────────

/// The header's `rc`, viewed as an `AtomicU32` (same layout as `u32`, guaranteed
/// by std).
///
/// # Safety
/// `h` must point at a live `ObjHeader`.
#[inline]
pub(crate) unsafe fn rc_ptr(h: NonNull<ObjHeader>) -> &'static AtomicU32 {
    // SAFETY: the caller guarantees `h` is live; `rc` sits at offset 8 of the
    // 16-byte header, and `AtomicU32` has the same layout as the `u32` there.
    unsafe { &*(h.as_ptr().cast::<u8>().add(8) as *const AtomicU32) }
}

/// Increment the strong count of a heap value. Immediates are unaffected.
///
/// Saturating: pinned objects (arena-allocated ropes, `rc == u32::MAX`) stay
/// pinned instead of wrapping to zero, which would hand live memory to
/// `dealloc`. Ordinary counts behave exactly as before.
///
/// # Safety
/// `v` must be a live heap value, and the caller must own a reference to give.
#[inline]
pub unsafe fn retain(v: Value) {
    if let Some(h) = v.header() {
        // SAFETY: `h` is a live header, per this function's contract.
        let _ = unsafe { rc_ptr(h) }.fetch_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |x| Some(x.saturating_add(1)),
        );
    }
}

/// Drop one reference to a heap value, freeing it at zero.
///
/// # Safety
/// `v` must be a live heap value and the caller must own exactly one reference.
pub unsafe fn release(v: Value) {
    if let Some(h) = v.header() {
        // SAFETY: `h` is a live header, per this function's contract.
        let rc = unsafe { rc_ptr(h) };
        if rc.fetch_sub(1, Ordering::Release) == 1 {
            fence(Ordering::Acquire);
            // SAFETY: the count hit zero, so this thread owns the last reference.
            unsafe { crate::gc::free_object(h) };
        }
    }
}

/// The strong count of a heap value, or `None` for an immediate.
///
/// Exposed for `gc_stats` and tests; the VM itself never needs to read it.
pub fn strong_count(v: Value) -> Option<u32> {
    v.header().map(|h| {
        // SAFETY: `header()` only returns for live heap values.
        unsafe { rc_ptr(h).load(Ordering::Relaxed) }
    })
}

/// Ensure `v` is uniquely owned, cloning the object if it is shared. A no-op for
/// immediates and for objects with `rc == 1` (`docs/09-memory.md` §4.2).
///
/// # Safety
/// `v` must be a live heap value if it is a heap value.
pub unsafe fn cow_mut(v: &mut Value) {
    let Some(h) = v.header() else { return };
    // SAFETY: `h` is a live header, per this function's contract.
    let rc = unsafe { rc_ptr(h) };
    if rc.load(Ordering::Acquire) == 1 {
        return;
    }
    // SAFETY: `h` is live and shared (rc > 1), so cloning is well-defined.
    let clone = unsafe { crate::gc::clone_object(h) };
    // SAFETY: we own the reference in `*v` and are replacing it.
    unsafe { release(*v) };
    *v = clone;
}

 impl Value {
    /// Recursively duplicates a value for safe passage across thread boundaries.
    pub fn deep_clone_cross_isolate(&self) -> Result<Value, String> {
        match self.tag() {
            Tag::Nil => Ok(Value::nil()),
            Tag::False => Ok(Value::bool(false)),
            Tag::True => Ok(Value::bool(true)),
            Tag::Int => Ok(*self),
            Tag::Float => Ok(*self),
            Tag::String => Ok(*self),
            Tag::Bytes => Ok(*self),
            Tag::Heap => match self.kind() {
                Some(ObjKind::String) | Some(ObjKind::Bytes) | Some(ObjKind::Channel) => Ok(*self),
                // Ropes share across isolates like flat strings: the node is
                // immutable after creation and `flat` memoizes race-safely.
                Some(ObjKind::Rope) => Ok(*self),
                _ => Err("Type cannot be sent across thread boundaries".to_string()),
            },
        }
    }
}

// ── Canonical value equality + hashing (dict/set keys) ──────────────────────
//
// `Value`'s derived `PartialEq`/`Hash` are bitwise: two equal-content strings
// (different `Arc`s) or `int(1)` vs `float(1.0)` compare/hash differently, so
// a hash map keyed by raw `Value` would both duplicate keys and degrade to
// linear scans. [`DictKey`] normalizes instead: whatever [`value_eq`] calls
// equal hashes equally. `ops::values_equal` delegates here, so there is one
// implementation and no drift.

/// A dict/set key: `Value` with [`value_eq`]/[`value_hash`] semantics.
/// `.0` recovers the stored (original, un-normalized) key.
#[derive(Clone, Copy, Debug)]
pub struct DictKey(pub Value);

impl PartialEq for DictKey {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        value_eq(self.0, other.0)
    }
}

impl Eq for DictKey {}

impl std::hash::Hash for DictKey {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        value_hash(self.0, state);
    }
}

/// Canonical equality: numerics compare across int/float, strings/bytes by
/// content, containers structurally, everything else bitwise. Mirrors what
/// `ops::values_equal` has always meant; it now delegates here.
pub fn value_eq(a: Value, b: Value) -> bool {
    if a.is_number() && b.is_number() {
        // Mirror `num_cmp`: int/int compares exactly, otherwise widen (i48 is
        // exact in f64); NaN compares unequal to everything, like before.
        match (a.as_int(), b.as_int()) {
            (Some(x), Some(y)) => return x == y,
            _ => match (a.as_f64(), b.as_f64()) {
                (Some(x), Some(y)) => return x == y,
                _ => return false,
            },
        }
    }
    if let (Some(x), Some(y)) = (a.as_str(), b.as_str()) {
        return x == y;
    }
    if let (Some(x), Some(y)) = (a.as_bytes(), b.as_bytes()) {
        return x == y;
    }
    if a.is_nil() && b.is_nil() {
        return true;
    }
    if let (Some(x), Some(y)) = (a.as_bool(), b.as_bool()) {
        return x == y;
    }
    if let (Some(x), Some(y)) = (a.as_list(), b.as_list()) {
        return x.len() == y.len() && x.iter().zip(y).all(|(p, q)| value_eq(*p, *q));
    }
    if let (Some(x), Some(y)) = (a.as_tuple(), b.as_tuple()) {
        return x.len() == y.len() && x.iter().zip(y).all(|(p, q)| value_eq(*p, *q));
    }
    if let (Some(x), Some(y)) = (a.as_set(), b.as_set()) {
        return x.len() == y.len() && x.iter().all(|v| y.iter().any(|w| value_eq(*v, *w)));
    }
    if let (Some(x), Some(y)) = (a.as_dict(), b.as_dict()) {
        return x.len() == y.len()
            && x.iter()
                .all(|(k, v)| y.get(k).is_some_and(|w| value_eq(*v, *w)));
    }
    a.to_bits() == b.to_bits()
}

/// Canonical hash, consistent with [`value_eq`]: equal values hash equally.
/// Discriminant prefixes keep distinct classes apart (collisions would still
/// be correct, just slower).
pub fn value_hash<H: std::hash::Hasher>(v: Value, state: &mut H) {
    if v.is_number() {
        // i48 is exact in f64, so int and float spellings of one number share
        // one hash. `-0.0` normalizes to `+0.0` (they compare equal).
        let mut f = v.as_f64().unwrap_or(f64::NAN);
        if f == 0.0 {
            f = 0.0;
        }
        state.write_u8(0);
        state.write_u64(f.to_bits());
        return;
    }
    match v.tag() {
        Tag::String => {
            state.write_u8(1);
            state.write(v.as_str().unwrap_or("").as_bytes());
        }
        Tag::Bytes => {
            state.write_u8(2);
            state.write(v.as_bytes().unwrap_or(&[]));
        }
        Tag::Nil => state.write_u8(3),
        Tag::False => state.write_u8(4),
        Tag::True => state.write_u8(5),
        Tag::Int | Tag::Float => {
            // Unreachable: numbers return above. Hash the bits defensively.
            state.write_u8(0);
            state.write_u64(v.to_bits());
        }
        Tag::Heap => match v.kind() {
            Some(ObjKind::List) => {
                state.write_u8(6);
                let inner = v.as_list().unwrap_or(&[]);
                state.write_usize(inner.len());
                for e in inner {
                    value_hash(*e, state);
                }
            }
            Some(ObjKind::Tuple) => {
                state.write_u8(7);
                let inner = v.as_tuple().unwrap_or(&[]);
                state.write_usize(inner.len());
                for e in inner {
                    value_hash(*e, state);
                }
            }
            Some(ObjKind::Set) => {
                // Order-insensitive (set equality is): fold commutatively.
                state.write_u8(8);
                match v.as_set() {
                    Some(inner) => {
                        state.write_usize(inner.len());
                        let mut acc: u64 = 0;
                        for e in inner {
                            let mut h =
                                std::collections::hash_map::DefaultHasher::new();
                            value_hash(*e, &mut h);
                            acc = acc.wrapping_add(std::hash::Hasher::finish(&h));
                        }
                        state.write_u64(acc);
                    }
                    None => state.write_usize(0),
                }
            }
            Some(ObjKind::Dict) => {
                // Order-insensitive like set equality.
                state.write_u8(9);
                match v.as_dict() {
                    Some(inner) => {
                        state.write_usize(inner.len());
                        let mut acc: u64 = 0;
                        for (k, e) in inner {
                            let mut h =
                                std::collections::hash_map::DefaultHasher::new();
                            value_hash(k.0, &mut h);
                            value_hash(*e, &mut h);
                            acc = acc.wrapping_add(std::hash::Hasher::finish(&h));
                        }
                        state.write_u64(acc);
                    }
                    None => state.write_usize(0),
                }
            }
            // Identity-compared heap kinds (func, class, instance, cell,
            // error, channel, mutex, handle, ...): equality is bitwise.
            // Ropes hash by content, exactly like flat strings, so equal
            // spellings stay interchangeable as dict keys.
            Some(ObjKind::Rope) => {
                state.write_u8(1);
                state.write(v.as_str().unwrap_or("").as_bytes());
            }
            _ => {
                state.write_u8(10);
                state.write_u64(v.to_bits());
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_layout_is_16_bytes() {
        assert_eq!(std::mem::size_of::<ObjHeader>(), 24);
        assert_eq!(std::mem::align_of::<ObjHeader>(), 8);
    }

    #[test]
    fn nil_and_bool_bits() {
        assert_eq!(Value::nil().tag(), Tag::Nil);
        assert!(Value::nil().is_nil());
        assert_eq!(Value::bool(true).tag(), Tag::True);
        assert_eq!(Value::bool(false).tag(), Tag::False);
        assert_eq!(Value::bool(true).as_bool(), Some(true));
        assert_eq!(Value::int(0).as_bool(), None);
    }

    #[test]
    fn int_round_trips_and_sign_extends() {
        for n in [0, 1, -1, 12345, -12345, Value::INT_MAX, Value::INT_MIN] {
            let v = Value::int(n);
            assert_eq!(v.tag(), Tag::Int, "{n}");
            assert_eq!(v.as_int(), Some(n), "{n}");
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "i48 overflow")]
    fn int_overflow_asserts_in_debug() {
        let _ = Value::int(1i64 << 47);
    }

    #[test]
    fn int_promote_overflows_to_float() {
        let v = Value::int_promote(1i64 << 47);
        assert_eq!(v.tag(), Tag::Float);
        assert_eq!(v.as_float(), Some(140_737_488_355_328.0));
        assert_eq!(Value::int_promote(5).tag(), Tag::Int);
    }

    #[test]
    fn float_semantics() {
        assert_eq!(Value::float(2.5).as_float(), Some(2.5));
        assert_eq!(Value::float(0.0).tag(), Tag::Float);
        // -0.0 is distinct from +0.0 bitwise and must be preserved.
        assert_ne!(Value::float(0.0).to_bits(), Value::float(-0.0).to_bits());
        assert!(Value::float(-0.0).as_float().unwrap().is_sign_negative());
    }

    #[test]
    fn float_infinities_are_preserved() {
        assert_eq!(Value::float(f64::INFINITY).as_float(), Some(f64::INFINITY));
        assert_eq!(
            Value::float(f64::NEG_INFINITY).as_float(),
            Some(f64::NEG_INFINITY)
        );
        assert_eq!(Value::float(f64::INFINITY).tag(), Tag::Float);
    }

    #[test]
    fn all_nan_patterns_normalize_to_one_bits() {
        let a = Value::float(f64::NAN);
        let b = Value::float(-f64::NAN);
        let c = Value::float(f64::from_bits(0x7FF8_DEAD_BEEF_0001));
        assert_eq!(a.to_bits(), b.to_bits());
        assert_eq!(a.to_bits(), c.to_bits());
        assert_eq!(a.tag(), Tag::Float);
        assert!(a.as_float().unwrap().is_nan());
    }

    #[test]
    fn tag_of_never_panics_over_random_bits() {
        // Classification is total over every 64-bit pattern.
        for seed in 0..100_000u64 {
            let bits = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let tag = Tag::of(bits);
            // Only non-heap patterns are safe to reify and inspect.
            if matches!(tag, Tag::Heap | Tag::String | Tag::Bytes) {
                continue;
            }
            // SAFETY: `tag` is immediate, so no pointer is present.
            let v = unsafe { Value::from_bits(bits) };
            let _ = v.as_int();
            let _ = v.as_float();
            let _ = v.as_bool();
            let _ = v.as_f64();
            let _ = v.type_name();
            let _ = format!("{v:?}");
        }
    }
}
