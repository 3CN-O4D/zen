//! The allocation trait and the M2–M8 manual-refcount collector.
//!
//! See [`docs/09-memory.md`] §2–7 and [`ADR-0002`](docs/decisions/ADR-0002-memory-model.md).
//! The collector stores a manual refcount in [`ObjHeader::rc`] and detaches
//! shared containers with [`crate::value::cow_mut`] (`docs/09-memory.md` §4.6,
//! option B). `unsafe` is confined to this file, [`crate::object`], and
//! [`crate::value`]. A tracing collector can replace `RefCountGc` in M9 behind
//! the same [`Gc`] trait.

use std::alloc::Layout;
use std::ptr;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};

use crate::bytecode::FunctionProto;
use crate::object::{
    BoundMethodObj, BytesObj, CellObj, ChanObj, ChanState, ClassObj, DictObj, ErrObj, FuncObj,
    ForeignObj, HandleObj, HasHeader, InstObj, ListObj, MutexObj, ObjHeader, ObjKind, RopeObj, SetObj,
    StringObj, TupleObj,
};
use crate::value::{retain, DictKey, Value};

// ── Allocation trait ─────────────────────────────────────────────────────────

/// The allocation interface (`docs/09-memory.md` §6). Both the M2–M8 refcount
/// collector and the future M9 tracing collector implement it, so the VM never
/// names a concrete collector.
pub trait Gc: Send + Sync {
    /// Allocate `size` bytes at `align`, returning a pointer to the start of the
    /// block. The caller writes the [`ObjHeader`].
    fn alloc(&self, kind: u8, size: usize, align: usize) -> Result<NonNull<u8>, GcError>;

    /// Free a block previously returned by [`Gc::alloc`].
    ///
    /// # Safety
    /// `p`/`size` must be a live allocation from [`Gc::alloc`] that has not been
    /// freed, with the same `size` that was requested.
    unsafe fn dealloc(&self, p: *mut u8, size: usize);

    /// Live bytes currently allocated, for `--mem-limit` and `gc_stats`.
    fn live_bytes(&self) -> usize;

    // High-level object constructors (the M2 `allocate_*` surface).
    fn allocate_string(&self, s: String) -> Value;
    fn allocate_bytes(&self, b: Vec<u8>) -> Value;
    fn allocate_list(&self, v: Vec<Value>) -> Value;
    fn allocate_dict(&self, m: IndexMap<DictKey, Value>) -> Value;
    fn allocate_set(&self, s: IndexSet<Value>) -> Value;
    fn allocate_tuple(&self, t: Vec<Value>) -> Value;
    fn allocate_func(&self, proto: Arc<FunctionProto>, upvals: Vec<Value>) -> Value;
    fn allocate_instance(&self, slots: Vec<Value>) -> Value;
    fn allocate_class(&self, name: Arc<str>, parent: Value, methods: Value) -> Value;
    fn allocate_bound_method(&self, receiver: Value, method: Value) -> Value;
    fn allocate_error(&self, msg: String, code: i32) -> Value;
    fn allocate_channel(&self) -> Value;
    fn allocate_mutex(&self) -> Value;
    fn allocate_handle(&self, id: u32) -> Value;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GcError {
    #[error("out of memory: requested {requested} bytes")]
    OutOfMemory { requested: usize },
    #[error("invalid allocation layout")]
    BadLayout,
}

// ── RefCountGc ───────────────────────────────────────────────────────────────

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

static FREE_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Total heap objects freed since start. For leak audits in tests.
#[doc(hidden)]
pub fn free_count() -> u64 {
    FREE_COUNT.load(Ordering::Relaxed)
}

/// The M2–M8 collector: a zero-sized manual-refcount allocator.
#[derive(Debug, Default)]
pub struct RefCountGc;

/// Alias kept for the roadmap's `ArcGc` name.
pub type ArcGc = RefCountGc;

/// A borrowed collector handle, matching `docs/09-memory.md` §2.5's `gc()`.
pub type GcRef = &'static dyn Gc;

/// The process-global collector. Object constructors use this; the VM may later
/// carry its own `Box<dyn Gc>` once the collector is no longer global.
#[inline]
pub fn gc() -> &'static RefCountGc {
    static GC: RefCountGc = RefCountGc;
    &GC
}

impl Gc for RefCountGc {
    #[inline]
    fn alloc(&self, _kind: u8, size: usize, align: usize) -> Result<NonNull<u8>, GcError> {
        let layout = Layout::from_size_align(size, align).map_err(|_| GcError::BadLayout)?;
        // SAFETY: `size >= 1` (every object is at least the header) and `align`
        // is a power of two, so `layout` is valid; we null-check before use.
        let p = unsafe { std::alloc::alloc(layout) };
        let p = NonNull::new(p).ok_or(GcError::OutOfMemory { requested: size })?;
        LIVE_BYTES.fetch_add(size, Ordering::Relaxed);
        Ok(p)
    }

    #[inline]
    unsafe fn dealloc(&self, p: *mut u8, size: usize) {
        if p.is_null() {
            return;
        }
        // SAFETY: `p`/`size` came from `alloc`, and every object this module
        // allocates has alignment 8 (it begins with an 8-aligned `ObjHeader`).
        unsafe {
            let layout = Layout::from_size_align_unchecked(size, 8);
            std::alloc::dealloc(p, layout);
        }
        LIVE_BYTES.fetch_sub(size, Ordering::Relaxed);
    }

    #[inline]
    fn live_bytes(&self) -> usize {
        LIVE_BYTES.load(Ordering::Relaxed)
    }

    fn allocate_string(&self, s: String) -> Value {
        new_string(Arc::from(s))
    }
    fn allocate_bytes(&self, b: Vec<u8>) -> Value {
        new_bytes(Arc::from(b.into_boxed_slice()))
    }
    fn allocate_list(&self, v: Vec<Value>) -> Value {
        new_list(v)
    }
    fn allocate_dict(&self, m: IndexMap<DictKey, Value>) -> Value {
        new_dict(m)
    }
    fn allocate_set(&self, s: IndexSet<Value>) -> Value {
        new_set(s)
    }
    fn allocate_tuple(&self, t: Vec<Value>) -> Value {
        new_tuple(t)
    }
    fn allocate_func(&self, proto: Arc<FunctionProto>, upvals: Vec<Value>) -> Value {
        new_func(proto, upvals)
    }
    fn allocate_instance(&self, slots: Vec<Value>) -> Value {
        new_instance(slots)
    }
    fn allocate_class(&self, name: Arc<str>, parent: Value, methods: Value) -> Value {
        new_class(name, parent, methods)
    }
    fn allocate_bound_method(&self, receiver: Value, method: Value) -> Value {
        new_bound_method(receiver, method)
    }
    fn allocate_error(&self, msg: String, code: i32) -> Value {
        new_error(Arc::from(msg), code, Value::nil())
    }
    fn allocate_channel(&self) -> Value {
        new_channel()
    }
    fn allocate_mutex(&self) -> Value {
        new_mutex(Value::nil())
    }
    fn allocate_handle(&self, id: u32) -> Value {
        new_handle(id)
    }
}
pub fn safe_channel_send(channel_val: Value, msg: Value) -> Result<(), crate::errors::VmError> {
    if !matches!(channel_val.kind(), Some(crate::object::ObjKind::Channel)) {
        return Err(crate::errors::VmError::TypeError(
            "Attempted to send on a non-channel value".into(),
        ));
    }
    match unsafe { channel_send(channel_val, msg) } {
        Ok(()) => Ok(()),
        Err(_) => Err(crate::errors::VmError::NativeError(
            "channel send failed".into(),
        )),
    }
}

pub fn safe_channel_recv(channel_val: Value) -> Result<Value, crate::errors::VmError> {
    if !matches!(channel_val.kind(), Some(crate::object::ObjKind::Channel)) {
        return Err(crate::errors::VmError::TypeError(
            "Attempted to receive from a non-channel value".into(),
        ));
    }
    match unsafe { channel_recv(channel_val) } {
        Some(v) => Ok(v),
        None => Ok(Value::nil()),
    }
}

// ── Allocation internals ─────────────────────────────────────────────────────

/// Move a heap struct into a fresh allocation with its header `size` filled in.
///
/// # Safety
/// `T` must be a `#[repr(C)]` struct whose first field is an [`ObjHeader`]
/// (enforced by the [`HasHeader`] bound and the `heap_type!` macro).
unsafe fn allocate<T: HasHeader>(mut obj: T) -> Value {
    let layout = Layout::new::<T>();
    let kind = obj.header().kind;
    let p = gc()
        .alloc(kind, layout.size(), layout.align())
        .unwrap_or_else(|e| panic!("OOM is fatal (08-vm.md §4.6): {e}"));
    let dst = p.as_ptr() as *mut T;
    obj.header_mut().size = layout.size() as u32;
    // SAFETY: `dst` is a fresh allocation of `Layout::new::<T>()`; writing `obj`
    // initialises it exactly once, and it is cast back to `T` on free.
    unsafe {
        ptr::write(dst, obj);
        Value::from_header(NonNull::new_unchecked(dst as *mut ObjHeader))
    }
}

/// Free an object: run its `Drop` (releasing children), then the allocation.
///
/// # Safety
/// `h` must be a live header from one of the `new_*` constructors, with `rc == 0`.
pub(crate) unsafe fn free_object(h: NonNull<ObjHeader>) {
    // SAFETY: `h` is live with `rc == 0` and uniquely owned by this call.
    unsafe {
        let kind = (*h.as_ptr()).kind();
        let size = (*h.as_ptr()).size as usize;
        let p = h.as_ptr();
        match kind {
            ObjKind::String => ptr::drop_in_place(p as *mut StringObj),
            ObjKind::Bytes => ptr::drop_in_place(p as *mut BytesObj),
            ObjKind::List => ptr::drop_in_place(p as *mut ListObj),
            ObjKind::Dict => ptr::drop_in_place(p as *mut DictObj),
            ObjKind::Set => ptr::drop_in_place(p as *mut SetObj),
            ObjKind::Tuple => ptr::drop_in_place(p as *mut TupleObj),
            ObjKind::Func => ptr::drop_in_place(p as *mut FuncObj),
            ObjKind::Instance => ptr::drop_in_place(p as *mut InstObj),
            ObjKind::Error => ptr::drop_in_place(p as *mut ErrObj),
            ObjKind::Channel => ptr::drop_in_place(p as *mut ChanObj),
            ObjKind::Mutex => ptr::drop_in_place(p as *mut MutexObj),
            ObjKind::Handle => ptr::drop_in_place(p as *mut HandleObj),
            ObjKind::Class => ptr::drop_in_place(p as *mut ClassObj),
            ObjKind::BoundMethod => ptr::drop_in_place(p as *mut BoundMethodObj),
            ObjKind::Cell => ptr::drop_in_place(p as *mut CellObj),
            ObjKind::Rope => ptr::drop_in_place(p as *mut RopeObj),
            ObjKind::Foreign => ptr::drop_in_place(p as *mut ForeignObj),
        }
        gc().dealloc(p as *mut u8, size);
        FREE_COUNT.fetch_add(1, Ordering::Relaxed);
    }
}

/// Clone a shared object for copy-on-write, retaining each child.
///
/// # Safety
/// `h` must point at a live object.
pub(crate) unsafe fn clone_object(h: NonNull<ObjHeader>) -> Value {
    // SAFETY: `h` is live, per this function's contract; every arm re-reads the
    // same header through the matching concrete type.
    unsafe {
        let kind = (*h.as_ptr()).kind();
        match kind {
            ObjKind::String => {
                let o = &*(h.as_ptr() as *const StringObj);
                new_string(o.data.clone())
            }
            ObjKind::Bytes => {
                let o = &*(h.as_ptr() as *const BytesObj);
                new_bytes(o.data.clone())
            }
            ObjKind::List => {
                let o = &*(h.as_ptr() as *const ListObj);
                new_list(retain_slice(&o.inner))
            }
            ObjKind::Dict => {
                let o = &*(h.as_ptr() as *const DictObj);
                let mut inner = IndexMap::with_capacity(o.inner.len());
                for (k, v) in &o.inner {
                    retain(k.0);
                    retain(*v);
                    inner.insert(*k, *v);
                }
                new_dict(inner)
            }
            ObjKind::Set => {
                let o = &*(h.as_ptr() as *const SetObj);
                let mut inner = IndexSet::with_capacity(o.inner.len());
                for v in &o.inner {
                    retain(*v);
                    inner.insert(*v);
                }
                new_set(inner)
            }
            ObjKind::Tuple => {
                let o = &*(h.as_ptr() as *const TupleObj);
                new_tuple(retain_slice(&o.inner))
            }
            ObjKind::Func => {
                let o = &*(h.as_ptr() as *const FuncObj);
                new_func(o.proto.clone(), retain_slice(&o.upvals))
            }
            ObjKind::Instance => {
                let o = &*(h.as_ptr() as *const InstObj);
                new_instance_shaped(retain_slice(&o.slots), o.shape)
            }
            ObjKind::Error => {
                let o = &*(h.as_ptr() as *const ErrObj);
                retain(o.val);
                new_error(o.msg.clone(), o.code, o.val)
            }
            ObjKind::Channel => new_channel(),
            ObjKind::Mutex => {
                let o = &*(h.as_ptr() as *const MutexObj);
                let inner = *o.inner.lock();
                retain(inner);
                new_mutex(inner)
            }
            ObjKind::Handle => new_handle((*(h.as_ptr() as *const HandleObj)).id),
            ObjKind::Class => {
                let o = &*(h.as_ptr() as *const ClassObj);
                retain(o.parent);
                retain(o.methods);
                new_class(o.name.clone(), o.parent, o.methods)
            }
            ObjKind::BoundMethod => {
                let o = &*(h.as_ptr() as *const BoundMethodObj);
                retain(o.receiver);
                retain(o.method);
                new_bound_method(o.receiver, o.method)
            }
            ObjKind::Cell => {
                let o = &*(h.as_ptr() as *const CellObj);
                let inner = *o.inner.borrow();
                retain(inner);
                new_cell(inner)
            }
            ObjKind::Foreign => {
                // Handles are registry-owned (no `Value` children to retain);
                // the clone shares the referent, like a shared string buffer.
                let o = &*(h.as_ptr() as *const ForeignObj);
                new_foreign(o.handle)
            }
            ObjKind::Rope => {
                // The flattened cache (if any) is rebuilt on demand; only the
                // tree (plus loose child references, like lists) is cloned.
                let o = &*(h.as_ptr() as *const RopeObj);
                retain(o.left);
                retain(o.right);
                new_rope(o.left, o.right, o.chars, o.bytes)
            }
        }
    }
}

/// Clone a slice of values, retaining each (`docs/09-memory.md` §3.4).
unsafe fn retain_slice(src: &[Value]) -> Vec<Value> {
    let mut out = Vec::with_capacity(src.len());
    for &v in src {
        // SAFETY: cloning a container gives each element a new reference.
        unsafe { retain(v) };
        out.push(v);
    }
    out
}

// ── Object constructors ──────────────────────────────────────────────────────
//
// Each takes ownership of its children (they must already be retained by the
// caller) and returns a `Value` holding the single reference created here.

pub fn new_string(data: Arc<str>) -> Value {
    unsafe {
        allocate(StringObj {
            header: ObjHeader::new(ObjKind::String),
            data,
        })
    }
}
/// A rope-string node: `left + right` without copying either side. Both sides
/// must already be strings (flat or rope); `chars`/`bytes` are the totals so
/// `length()` stays O(1). Child references are loose, exactly like list
/// elements (see the module note on `release`).
pub fn new_rope(left: Value, right: Value, chars: usize, bytes: usize) -> Value {
    // Hot path: bump-allocate the node from a thread-local arena instead of
    // malloc — append loops create thousands of nodes and never free them
    // (the crate's loose discipline leaks short-lived garbage anyway; the
    // arena leaks ~64B per node instead of a malloc+header round-trip).
    // Arena nodes are pinned (`rc = u32::MAX`, plus saturating `retain`
    // below) so a release cascade can never hand arena memory to `dealloc`.
    // Allocation failure (or a dying thread) falls back to the regular path.
    if let Some(v) = rope_arena_alloc(left, right, chars, bytes) {
        return v;
    }
    unsafe {
        allocate(RopeObj {
            header: ObjHeader::new(ObjKind::Rope),
            left,
            right,
            chars,
            bytes,
            flat: std::sync::OnceLock::new(),
        })
    }
}

/// Bump arena for rope nodes: 8 KiB chunks, carved `RopeObj`-at-a-time, never
/// freed (like generated code). One arena per thread; ropes themselves are
/// freely shareable across isolates (nodes are immutable, `flat` is sync).
struct RopeArena {
    cur: *mut u8,
    next: usize,
    end: usize,
}

impl RopeArena {
    const CHUNK: usize = 8192;

    fn alloc_node(&mut self, left: Value, right: Value, chars: usize, bytes: usize) -> Option<Value> {
        let size = std::mem::size_of::<RopeObj>();
        debug_assert_eq!(std::mem::align_of::<RopeObj>(), 8);
        if self.next + size > self.end && !self.refill(size) {
            return None;
        }
        // SAFETY: `cur[next..next+size]` is carved-once arena space.
        unsafe {
            let p = self.cur.add(self.next) as *mut RopeObj;
            self.next += size;
            ptr::write(
                p,
                RopeObj {
                    header: ObjHeader::new(ObjKind::Rope),
                    left,
                    right,
                    chars,
                    bytes,
                    flat: std::sync::OnceLock::new(),
                },
            );
            (*p).header.rc = u32::MAX;
            (*p).header.size = size as u32;
            Some(Value::from_header(NonNull::new_unchecked(
                p as *mut ObjHeader,
            )))
        }
    }

    fn refill(&mut self, need: usize) -> bool {
        if need > Self::CHUNK {
            return false;
        }
        // SAFETY: fresh global allocation; leaked by design (never freed).
        unsafe {
            let layout = std::alloc::Layout::from_size_align(Self::CHUNK, 8)
                .unwrap_or(std::alloc::Layout::new::<u8>());
            let p = std::alloc::alloc(layout);
            if p.is_null() {
                return false;
            }
            self.cur = p;
            self.next = 0;
            self.end = Self::CHUNK;
            true
        }
    }
}

thread_local! {
    static ROPE_ARENA: std::cell::RefCell<RopeArena> = std::cell::RefCell::new(RopeArena {
        cur: std::ptr::null_mut(),
        next: 1,
        end: 0,
    });
}

fn rope_arena_alloc(left: Value, right: Value, chars: usize, bytes: usize) -> Option<Value> {
    ROPE_ARENA
        .try_with(|a| a.borrow_mut().alloc_node(left, right, chars, bytes))
        .ok()
        .flatten()
}
pub fn new_bytes(data: Arc<[u8]>) -> Value {
    unsafe {
        allocate(BytesObj {
            header: ObjHeader::new(ObjKind::Bytes),
            data,
        })
    }
}
pub fn new_list(inner: Vec<Value>) -> Value {
    unsafe {
        allocate(ListObj {
            header: ObjHeader::new(ObjKind::List),
            inner,
        })
    }
}
pub fn new_dict(inner: IndexMap<DictKey, Value>) -> Value {
    unsafe {
        allocate(DictObj {
            header: ObjHeader::new(ObjKind::Dict),
            inner,
        })
    }
}
pub fn new_set(inner: IndexSet<Value>) -> Value {
    unsafe {
        allocate(SetObj {
            header: ObjHeader::new(ObjKind::Set),
            inner,
        })
    }
}
pub fn new_tuple(inner: Vec<Value>) -> Value {
    unsafe {
        allocate(TupleObj {
            header: ObjHeader::new(ObjKind::Tuple),
            inner,
        })
    }
}
pub fn new_func(proto: Arc<FunctionProto>, upvals: Vec<Value>) -> Value {
    unsafe {
        allocate(FuncObj {
            header: ObjHeader::new(ObjKind::Func),
            proto,
            upvals,
        })
    }
}
pub fn new_instance(slots: Vec<Value>) -> Value {
    new_instance_shaped(slots, 0)
}
/// An instance with an explicit shape id (cloning preserves the shape its
/// slots were laid out under; fresh instances start at the root shape).
pub fn new_instance_shaped(slots: Vec<Value>, shape: u32) -> Value {
    unsafe {
        allocate(InstObj {
            header: ObjHeader::new(ObjKind::Instance),
            shape,
            slots,
        })
    }
}
pub fn new_class(name: Arc<str>, parent: Value, methods: Value) -> Value {
    unsafe {
        allocate(ClassObj {
            header: ObjHeader::new(ObjKind::Class),
            name,
            parent,
            methods,
        })
    }
}
pub fn new_bound_method(receiver: Value, method: Value) -> Value {
    unsafe {
        allocate(BoundMethodObj {
            header: ObjHeader::new(ObjKind::BoundMethod),
            receiver,
            method,
        })
    }
}
/// Box a captured local so the defining frame and every capturing closure
/// share one mutable storage location.
pub fn new_cell(value: Value) -> Value {
    unsafe {
        allocate(CellObj {
            header: ObjHeader::new(ObjKind::Cell),
            inner: std::cell::RefCell::new(value),
        })
    }
}
/// A foreign (CPython) handle value. The referent lives in the linked Python
/// bridge's registry under `handle`; the `Value` itself carries no children.
pub fn new_foreign(handle: u32) -> Value {
    unsafe {
        allocate(ForeignObj {
            header: ObjHeader::new(ObjKind::Foreign),
            handle,
        })
    }
}
pub fn new_error(msg: Arc<str>, code: i32, val: Value) -> Value {
    unsafe {
        allocate(ErrObj {
            header: ObjHeader::new(ObjKind::Error),
            msg,
            code,
            val,
        })
    }
}
pub fn new_channel() -> Value {
    let (tx, rx) = crossbeam_channel::unbounded();
    unsafe {
        allocate(ChanObj {
            header: ObjHeader::new(ObjKind::Channel),
            inner: ChanState {
                sender: Some(tx),
                receiver: Some(rx),
            },
        })
    }
}
pub fn new_mutex(inner: Value) -> Value {
    unsafe {
        allocate(MutexObj {
            header: ObjHeader::new(ObjKind::Mutex),
            inner: parking_lot::Mutex::new(inner),
        })
    }
}
pub fn new_handle(id: u32) -> Value {
    unsafe {
        allocate(HandleObj {
            header: ObjHeader::new(ObjKind::Handle),
            id,
        })
    }
}

/// Send `v` on a channel value, transferring one reference to the queue.
/// Returns `Err(v)` if the channel is closed, giving the reference back.
///
/// # Safety
/// `chan` must be a live channel value.
pub unsafe fn channel_send(chan: Value, v: Value) -> Result<(), Value> {
    // SAFETY: `chan` is a live channel value, per this function's contract.
    let obj = unsafe { &*(chan.header_unchecked().as_ptr() as *const ChanObj) };
    match &obj.inner.sender {
        Some(tx) if tx.send(v).is_ok() => Ok(()),
        _ => Err(v),
    }
}

/// Receive the next message from a channel, if any.
///
/// # Safety
/// `chan` must be a live channel value.
pub unsafe fn channel_recv(chan: Value) -> Option<Value> {
    // SAFETY: `chan` is a live channel value, per this function's contract.
    let obj = unsafe { &*(chan.header_unchecked().as_ptr() as *const ChanObj) };
    obj.inner
        .receiver
        .as_ref()
        .and_then(|rx| rx.try_recv().ok())
}

impl RefCountGc {
    pub fn safe_channel_send(channel_val: Value, msg: Value) -> Result<(), crate::errors::VmError> {
        if !matches!(channel_val.kind(), Some(crate::object::ObjKind::Channel)) {
            return Err(crate::errors::VmError::TypeError(
                "Attempted to send on a non-channel value".into(),
            ));
        }
        match unsafe { channel_send(channel_val, msg) } {
            Ok(()) => Ok(()),
            Err(_) => Err(crate::errors::VmError::NativeError(
                "channel send failed".into(),
            )),
        }
    }

    pub fn safe_channel_recv(channel_val: Value) -> Result<Value, crate::errors::VmError> {
        if !matches!(channel_val.kind(), Some(crate::object::ObjKind::Channel)) {
            return Err(crate::errors::VmError::TypeError(
                "Attempted to receive from a non-channel value".into(),
            ));
        }
        match unsafe { channel_recv(channel_val) } {
            Some(v) => Ok(v),
            None => Ok(Value::nil()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::release;

    #[test]
    fn live_bytes_accounting() {
        // `live_bytes` is process-global while the harness runs tests in
        // parallel, so a concurrent test can allocate between our samples.
        // Retry a few times: each attempt is exact, and sustained
        // interference fails loudly instead of flaking silently.
        for _ in 0..32 {
            let before = gc().live_bytes();
            let v = new_list(vec![]);
            let mid = gc().live_bytes();
            unsafe { release(v) };
            if mid > before && gc().live_bytes() == before {
                return;
            }
        }
        panic!("live_bytes accounting never observed a quiescent window");
    }

    #[test]
    fn channel_send_recv_round_trips() {
        let ch = new_channel();
        unsafe {
            assert!(channel_recv(ch).is_none());
            assert!(channel_send(ch, Value::int(7)).is_ok());
            assert_eq!(channel_recv(ch).map(Value::as_int), Some(Some(7)));
            release(ch);
        }
    }

    #[test]
    fn error_object_exposes_code() {
        let e = new_error(Arc::from("bad"), 9, Value::nil());
        let o = unsafe { &*(e.header_unchecked().as_ptr() as *const ErrObj) };
        assert_eq!(o.code, 9);
        assert_eq!(o.msg.as_ref(), "bad");
        unsafe { release(e) };
    }
}
