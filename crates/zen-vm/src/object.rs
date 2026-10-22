//! Heap object header, kind discriminator, and the twelve heap payload structs.
//!
//! See [`docs/09-memory.md`] §2–4 and [`docs/04-value-and-types.md`]. Every heap
//! allocation begins with an [`ObjHeader`]; the object structs are `#[repr(C)]`
//! with the header first so a `*const ObjHeader` can be cast back to the concrete
//! type via [`ObjKind`]. `unsafe` is confined to this file, [`crate::gc`], and
//! [`crate::value`].

use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};

use crate::bytecode::FunctionProto;
use crate::value::{release, DictKey, Value};

// ── Object kinds ────────────────────────────────────────────────────────────
//
// Discriminants are frozen: `ObjHeader.kind` is a persisted byte in heap dumps
// and fixture files (ADR-0001 §"kind is stable"). Never renumber.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ObjKind {
    List = 0,
    Dict = 1,
    Set = 2,
    Tuple = 3,
    Func = 4,
    Instance = 5,
    Error = 6,
    Channel = 7,
    Mutex = 8,
    Handle = 9,
    String = 10,
    Bytes = 11,
    // Appended for the M4 OOP extension (ABI v2). Never renumber 0..=11.
    Class = 12,
    BoundMethod = 13,
    /// A boxed captured local. When an inner closure captures an enclosing
    /// local, the register is promoted to a `Cell` so every closure (and the
    /// defining frame itself) observes writes to the same storage.
    Cell = 14,
    // Appended for foreign (CPython) handles (never renumber 0..=15).
    // A `Foreign` value is an opaque reference to a Python-side object
    // (module, callable, or instance), owned by the linked Python bridge's
    // handle registry. All dispatch (attribute access, calls) routes back
    // through the bridge; Zen never inspects the referent.
    Foreign = 16,
    // Appended for rope strings (never renumber 0..=14). A `Rope` quacks like
    // a `String` everywhere (`typeof`, `is_string`, `as_str`, equality,
    // hashing, methods): it is a binary concatenation tree whose leaves are
    // flat strings, so repeated `+` never recopies the left side.
    Rope = 15,
}

impl ObjKind {
    #[inline]
    pub const fn from_u8(b: u8) -> Option<ObjKind> {
        Some(match b {
            0 => ObjKind::List,
            1 => ObjKind::Dict,
            2 => ObjKind::Set,
            3 => ObjKind::Tuple,
            4 => ObjKind::Func,
            5 => ObjKind::Instance,
            6 => ObjKind::Error,
            7 => ObjKind::Channel,
            8 => ObjKind::Mutex,
            9 => ObjKind::Handle,
            10 => ObjKind::String,
            11 => ObjKind::Bytes,
            12 => ObjKind::Class,
            13 => ObjKind::BoundMethod,
            14 => ObjKind::Cell,
            15 => ObjKind::Rope,
            16 => ObjKind::Foreign,
            _ => return None,
        })
    }

    /// The language-level type name, as `typeof` reports it (`docs/04` §9).
    pub const fn type_name(self) -> &'static str {
        match self {
            ObjKind::List => "list",
            ObjKind::Dict => "dict",
            ObjKind::Set => "set",
            ObjKind::Tuple => "tuple",
            ObjKind::Func => "func",
            ObjKind::Instance => "instance",
            ObjKind::Error => "error",
            ObjKind::Channel => "channel",
            ObjKind::Mutex => "mutex",
            ObjKind::Handle => "handle",
            ObjKind::String => "string",
            ObjKind::Bytes => "bytes",
            ObjKind::Class => "class",
            ObjKind::BoundMethod => "func",
            ObjKind::Cell => "cell",
            ObjKind::Rope => "string",
            ObjKind::Foreign => "foreign",
        }
    }
}

// ── Object header (16 bytes, offset 0 of every heap allocation) ──────────────

#[repr(C, align(8))]
pub struct ObjHeader {
    /// offset 0 — discriminates the twelve heap kinds.
    pub kind: u8,
    /// offset 1 — M9 collector only; always 0 in M2–M8.
    pub marks: u8,
    /// offset 2..4 — object flags, see [`flags`].
    pub flags: u16,
    /// offset 4..8 — shape id; 0 = `Unshaped`.
    pub shape: u32,
    /// offset 8..12 — **manual** strong count (`docs/09-memory.md` §4.6).
    pub rc: u32,
    /// offset 12..16 — total allocation size in bytes, header included.
    pub size: u32,
    /// offset 16..24 — owner thread ID for cross-isolate affinity checks.
    pub owner_thread: u64,
}

const _: () = {
    assert!(std::mem::size_of::<ObjHeader>() == 24);
    assert!(std::mem::align_of::<ObjHeader>() == 8);
    assert!(std::mem::offset_of!(ObjHeader, kind) == 0);
    assert!(std::mem::offset_of!(ObjHeader, marks) == 1);
    assert!(std::mem::offset_of!(ObjHeader, flags) == 2);
    assert!(std::mem::offset_of!(ObjHeader, shape) == 4);
    assert!(std::mem::offset_of!(ObjHeader, rc) == 8);
    assert!(std::mem::offset_of!(ObjHeader, size) == 12);
    assert!(std::mem::offset_of!(ObjHeader, owner_thread) == 16);
};

impl ObjHeader {
    #[inline]
    pub fn new(kind: ObjKind) -> Self {
        Self {
            kind: kind as u8,
            marks: 0,
            flags: 0,
            shape: 0,
            rc: 1,
            size: 0,
            owner_thread: 0,
        }
    }

    /// # Panics
    /// If `self.kind` is not a valid [`ObjKind`]. Every header this crate
    /// allocates is constructed via [`ObjHeader::new`], so that cannot happen
    /// for live objects; it is the corruption tripwire for M9 and FFI.
    #[inline]
    pub fn kind(&self) -> ObjKind {
        ObjKind::from_u8(self.kind).expect("ObjHeader.kind is a valid ObjKind")
    }
}

/// Object flag bits (`docs/09-memory.md` §2.2). Bits 4..16 are reserved and
/// must stay 0.
pub mod flags {
    pub const CLOSED: u16 = 1 << 0;
    pub const HAS_TRACEBACK: u16 = 1 << 1;
    pub const HAS_SENDERS: u16 = 1 << 2;
    pub const FROZEN: u16 = 1 << 3;
    pub const RESERVED: u16 = !0b1111;
}

// ── Heap payloads ───────────────────────────────────────────────────────────

/// Gives `gc.rs` a way to fill the header `size` without knowing the type.
pub(crate) trait HasHeader {
    fn header(&self) -> &ObjHeader;
    fn header_mut(&mut self) -> &mut ObjHeader;
}

macro_rules! heap_type {
    ($(#[$meta:meta])* $name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        $(#[$meta])*
        #[repr(C)]
        pub struct $name {
            pub header: ObjHeader,
            $(pub $field: $ty,)*
        }
        impl HasHeader for $name {
            #[inline]
            fn header(&self) -> &ObjHeader { &self.header }
            #[inline]
            fn header_mut(&mut self) -> &mut ObjHeader { &mut self.header }
        }
    };
}

heap_type!(StringObj { data: Arc<str> });
heap_type!(BytesObj { data: Arc<[u8]> });
heap_type!(ListObj { inner: Vec<Value> });
heap_type!(DictObj { inner: IndexMap<DictKey, Value> });
heap_type!(SetObj { inner: IndexSet<Value> });
heap_type!(TupleObj { inner: Vec<Value> });
heap_type!(FuncObj { proto: Arc<FunctionProto>, upvals: Vec<Value> });
heap_type!(InstObj { shape: u32, slots: Vec<Value> });
heap_type!(ErrObj { msg: Arc<str>, code: i32, val: Value });
heap_type!(MutexObj { inner: parking_lot::Mutex<Value> });
heap_type!(HandleObj { id: u32 });
    // A class blueprint. `methods` is a `DictObj` (name → `Func`); `parent` is a
    // `Class` value or `nil`. Shared by every instance.

heap_type!(ClassObj { name: Arc<str>, parent: Value, methods: Value });
// A method bound to a receiver, produced by `instance.method` without a call.
heap_type!(BoundMethodObj {
    receiver: Value,
    method: Value
});
// A boxed captured local shared by the defining frame and every closure that
// captures it.
heap_type!(CellObj { inner: std::cell::RefCell<Value> });
// A rope-string node: `left + right` without copying. Both sides are string
// values (flat or rope, shared by reference); `chars`/`bytes` are the totals
// so `length()` stays O(1). `flat` memoizes the flattened form for `as_str`
// (`OnceLock` is race-safe, so sharing across isolates is sound).
heap_type!(RopeObj {
    left: Value,
    right: Value,
    chars: usize,
    bytes: usize,
    flat: std::sync::OnceLock<Box<str>>
});
// A foreign (CPython) handle: an opaque id into the linked Python bridge's
// object registry. No `Value` children, so `Drop` is a no-op — registry
// entries live until the bridge itself drops (bounded by distinct handles;
// explicit release is future work alongside handle finalizers).
heap_type!(ForeignObj { handle: u32 });

/// A channel's queue and endpoint. Dropping drains and releases queued values.
#[derive(Default)]
pub struct ChanState {
    pub sender: Option<crossbeam_channel::Sender<Value>>,
    pub receiver: Option<crossbeam_channel::Receiver<Value>>,
}
impl Drop for ChanState {
    fn drop(&mut self) {
        if let Some(rx) = &self.receiver {
            while let Ok(v) = rx.try_recv() {
                // SAFETY: the queue owned one reference per message.
                unsafe { release(v) };
            }
        }
    }
}

heap_type!(ChanObj { inner: ChanState });

// ── Drop discipline: release child `Value`s (docs/09-memory.md §3.3) ─────────
//
// String/Bytes hold `Arc`s (not `Value`s), so their `Arc` fields self-drop.
// The rest release every stored `Value`, matching their constructor's retains.

impl Drop for ListObj {
    fn drop(&mut self) {
        for v in self.inner.drain(..) {
            unsafe { release(v) };
        }
    }
}
impl Drop for DictObj {
    fn drop(&mut self) {
        for (k, v) in self.inner.drain(..) {
            unsafe {
                release(k.0);
                release(v);
            }
        }
    }
}
impl Drop for SetObj {
    fn drop(&mut self) {
        for v in self.inner.drain(..) {
            unsafe { release(v) };
        }
    }
}
impl Drop for TupleObj {
    fn drop(&mut self) {
        for v in self.inner.drain(..) {
            unsafe { release(v) };
        }
    }
}
impl Drop for FuncObj {
    fn drop(&mut self) {
        for v in self.upvals.drain(..) {
            unsafe { release(v) };
        }
    }
}
impl Drop for InstObj {
    fn drop(&mut self) {
        for v in self.slots.drain(..) {
            unsafe { release(v) };
        }
    }
}
impl Drop for ClassObj {
    fn drop(&mut self) {
        unsafe {
            release(self.parent);
            release(self.methods);
        }
    }
}
impl Drop for BoundMethodObj {
    fn drop(&mut self) {
        unsafe {
            release(self.receiver);
            release(self.method);
        }
    }
}
impl Drop for CellObj {
    fn drop(&mut self) {
        let inner = self.inner.get_mut();
        unsafe { release(*inner) };
    }
}
impl Drop for RopeObj {
    fn drop(&mut self) {
        // SAFETY: the rope owned one loose reference per side, mirroring how
        // lists own their elements (see the module note on `release`).
        unsafe {
            release(self.left);
            release(self.right);
        }
    }
}
impl Drop for ErrObj {
    fn drop(&mut self) {
        unsafe { release(self.val) };
    }
}
impl Drop for MutexObj {
    fn drop(&mut self) {
        let inner = self.inner.get_mut();
        unsafe { release(*inner) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_size_and_offsets() {
        assert_eq!(std::mem::size_of::<ObjHeader>(), 24);
        assert_eq!(std::mem::offset_of!(StringObj, data), 24);
        assert_eq!(std::mem::offset_of!(ListObj, inner), 24);
        assert_eq!(std::mem::offset_of!(FuncObj, proto), 24);
        assert_eq!(std::mem::offset_of!(FuncObj, upvals), 32);
    }

    #[test]
    fn kind_round_trips() {
        for k in 0u8..=16 {
            assert_eq!(ObjKind::from_u8(k).unwrap() as u8, k);
        }
        assert_eq!(ObjKind::from_u8(17), None);
        assert_eq!(ObjKind::from_u8(255), None);
    }

    #[test]
    fn kind_type_names() {
        assert_eq!(ObjKind::Handle.type_name(), "handle");
        assert_eq!(ObjKind::String.type_name(), "string");
        assert_eq!(ObjKind::Error.type_name(), "error");
        assert_eq!(ObjKind::Foreign.type_name(), "foreign");
    }

    #[test]
    fn new_header_starts_at_rc_one() {
        let h = ObjHeader::new(ObjKind::List);
        assert_eq!(h.rc, 1);
        assert_eq!(h.marks, 0);
        assert_eq!(h.flags, 0);
    }
}
