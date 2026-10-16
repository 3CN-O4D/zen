//! Native runtime bridges that JIT-compiled Zen code calls back into.
//!
//! These are the only place native code touches the VM. Everything here uses
//! raw pointers and is `unsafe`; the safe wrappers the interpreter calls live
//! in [`crate::jit`]. The functions are `#[no_mangle]` so the lowerer can take
//! their address and emit a direct `call_indirect` to them.
#![allow(unsafe_code)]

use std::sync::atomic::Ordering;

use crate::jit::{NativeEntry, STATE_JITTED};
use crate::gc::new_func;
use crate::value::Value;
use crate::vm::{Vm, REGISTERS};

/// Read a module global for the JIT. Out-of-range reads yield `nil`; a compiled
/// function is only ever entered after a thousand interpreted calls, so an
/// out-of-range index would already have raised there.
#[no_mangle]
pub extern "C" fn zen_jit_get_global(vm: *const Vm, module_id: usize, index: usize) -> u64 {
    // SAFETY: the caller passes a live VM for the duration of the call.
    let vm = unsafe { &*vm };
    vm.jit_global(module_id, index).to_bits()
}

/// Perform a `Call` from native code. The callee value sits at
/// `regs[base + first - 1]` and its arguments at `regs[base + first ..]`.
///
/// Returns `0` and writes the result through `out` on success, or `1` to request
/// a deopt: the interpreter re-runs the *calling* function (whose register window
/// the native frame has already restored), so any call shape this bridge does
/// not handle is simply executed by the interpreter instead.
#[no_mangle]
pub extern "C" fn zen_jit_call(
    vm: *mut Vm,
    regs: *mut u64,
    base: usize,
    first: usize,
    argc: usize,
    ret: usize,
    out: *mut u64,
) -> u32 {
    let _ = ret;
    // SAFETY: the JIT frame owns `regs[..]` and passes a live register file.
    let callee_bits = unsafe { *regs.add(base + first - 1) };
    let callee = unsafe { Value::from_bits(callee_bits) };
    let Some(proto) = callee.func_proto() else {
        return 1;
    };
    if argc < proto.num_required as usize || argc > proto.params.len() {
        return 1;
    }
    let Some(slot) = proto.jit.get() else {
        return 1;
    };
    if slot.state.load(Ordering::Acquire) != STATE_JITTED {
        // Native-only call graphs still tier up: record the call so the
        // interpreter-side threshold trips even when no interpreter frame
        // ever runs this callee.
        crate::jit::note_call(unsafe { &*vm }.jit.as_ref(), &proto);
        return 1;
    }
    let addr = slot.entry.load(Ordering::Acquire);
    if addr == 0 || base + first + proto.max_reg as usize > REGISTERS {
        return 1;
    }
    // SAFETY: the compiler published a `NativeEntry`-typed pointer here.
    let entry: NativeEntry = unsafe { std::mem::transmute(addr) };
    // SAFETY: `regs`/`vm` stay valid across the nested native call; the callee
    // window `[base+first, base+first+max_reg)` was bounds-checked above.
    unsafe { entry(regs, base + first, vm, out, callee_bits) }
}

/// Fast-path instance field read through the calling prototype's inline cache.
///
/// `closure_bits` is the value the native frame was entered with; its prototype
/// owns the IC slots, so nested native calls resolve the correct cache. Returns
/// `0` and writes the value through `out` on a monomorphic hit, else `1` (deopt).
#[no_mangle]
pub extern "C" fn zen_jit_ic_get_field(
    vm: *const Vm,
    obj_bits: u64,
    pc: usize,
    closure_bits: u64,
    out: *mut u64,
) -> u32 {
    // SAFETY: the caller passes a live VM and live values for this call.
    let vm = unsafe { &*vm };
    let obj = unsafe { Value::from_bits(obj_bits) };
    let Some(proto) = unsafe { Value::from_bits(closure_bits) }.func_proto() else {
        return 1;
    };
    if let Some(v) = vm.ic_get_field(&proto, pc, obj) {
        // SAFETY: `out` is the live result cell the lowerer passed.
        unsafe { *out = v.to_bits() };
        return 0;
    }
    1
}

/// Fast-path instance field write through the calling prototype's inline cache.
/// Returns `0` when the cached slot was written, else `1` (deopt); the callee
/// register is left unchanged either way (the interpreter's fast path also
/// yields the receiver).
#[no_mangle]
pub extern "C" fn zen_jit_ic_set_field(
    vm: *const Vm,
    obj_bits: u64,
    pc: usize,
    val_bits: u64,
    closure_bits: u64,
) -> u32 {
    // SAFETY: the caller passes a live VM and live values for this call.
    let vm = unsafe { &*vm };
    let obj = unsafe { Value::from_bits(obj_bits) };
    let val = unsafe { Value::from_bits(val_bits) };
    let Some(proto) = unsafe { Value::from_bits(closure_bits) }.func_proto() else {
        return 1;
    };
    if vm.ic_set_field(&proto, pc, obj, val) {
        0
    } else {
        1
    }
}

// Upvalue access helpers

#[no_mangle]
pub extern "C" fn zen_jit_get_upval(vm: *const Vm, closure_bits: u64, idx: usize, out: *mut u64) -> u32 {
    let _ = vm;
    let closure = unsafe { Value::from_bits(closure_bits) };
    let upvals = match closure.func_upvals() {
        Some(u) => u,
        None => return 1,
    };
    let held = match upvals.get(idx).copied() {
        Some(h) => h,
        None => return 1,
    };
    let v = match held.as_cell() {
        Some(cell) => *cell.borrow(),
        None => held,
    };
    unsafe { *out = v.to_bits() };
    0
}

#[no_mangle]
pub extern "C" fn zen_jit_set_upval(vm: *mut Vm, closure_bits: u64, idx: usize, val_bits: u64) -> u32 {
    let _ = vm;
    // The *current* function's closure arrives as a native parameter: native
    // code runs without an interpreter frame, so `frames.last()` would be the
    // caller's frame (and writing through it would corrupt the caller).
    let mut closure = unsafe { Value::from_bits(closure_bits) };
    let val = unsafe { Value::from_bits(val_bits) };
    let upvals = match closure.func_upvals_mut() {
        Some(u) => u,
        None => return 1,
    };
    if idx >= upvals.len() {
        return 1;
    }
    match upvals[idx].as_cell() {
        Some(cell) => *cell.borrow_mut() = val,
        None => upvals[idx] = val,
    }
    0
}

/// Module-global write for JIT-compiled (module-level) code. Mirrors the
/// interpreter's `SetGlobal` exactly (plain slot overwrite); out-of-range
/// deopts so the interpreter raises.
#[no_mangle]
pub extern "C" fn zen_jit_set_global(vm: *mut Vm, module_id: usize, index: usize, val_bits: u64) -> u32 {
    // SAFETY: the caller passes a live VM for the duration of the call.
    let vm = unsafe { &mut *vm };
    let val = unsafe { Value::from_bits(val_bits) };
    if vm.jit_set_global(module_id, index, val) {
        0
    } else {
        1
    }
}

/// `MakeDict`: build a dict from `pairs` alternating key/value slots at
/// `regs[base + first..]`. Mirrors the interpreter's `MakeDict` call sequence.
#[no_mangle]
pub extern "C" fn zen_jit_make_dict(
    vm: *mut Vm,
    regs: *const u64,
    base: usize,
    first: usize,
    pairs: usize,
    out: *mut u64,
) -> u32 {
    let _ = vm;
    // SAFETY: the caller passes the live register file and a window the
    // interpreter keeps coherent; indices were bounds-checked by the lowerer.
    let get = |i: usize| unsafe { Value::from_bits(*regs.add(base + first + i)) };
    let mut map = indexmap::IndexMap::new();
    for i in 0..pairs {
        let k = get(i * 2);
        let v = get(i * 2 + 1);
        crate::ops::dict_insert(&mut map, k, v);
    }
    let d = crate::gc::new_dict(map);
    unsafe { *out = d.to_bits() };
    0
}

/// `GetIndex` through the shared implementation. Errors (index/type) deopt so
/// the interpreter raises them.
#[no_mangle]
pub extern "C" fn zen_jit_get_index(
    vm: *const Vm,
    obj_bits: u64,
    key_bits: u64,
    out: *mut u64,
) -> u32 {
    let _ = vm;
    // SAFETY: the caller passes live values for the duration of the call.
    let (obj, key) = unsafe { (Value::from_bits(obj_bits), Value::from_bits(key_bits)) };
    match crate::ops::get_index(obj, key) {
        Ok(v) => {
            unsafe { *out = v.to_bits() };
            0
        }
        Err(_) => 1,
    }
}

/// `SetIndex` on a local copy (the shared implementation detaches shared
/// storage itself, exactly like the interpreter). Errors deopt.
#[no_mangle]
pub extern "C" fn zen_jit_set_index(
    vm: *const Vm,
    obj_bits: u64,
    key_bits: u64,
    val_bits: u64,
    out: *mut u64,
) -> u32 {
    let _ = vm;
    // SAFETY: the caller passes live values for the duration of the call.
    let (obj, key, val) =
        unsafe { (Value::from_bits(obj_bits), Value::from_bits(key_bits), Value::from_bits(val_bits)) };
    match crate::ops::set_index(obj, key, val) {
        Ok(v) => {
            unsafe { *out = v.to_bits() };
            0
        }
        Err(_) => 1,
    }
}

/// Resolve a `CallMethod` receiver to the method value (instance inline-cache
/// path plus module-namespace dict path), mirroring the interpreter. Anything
/// else (missing method, higher-order/user-defined slow paths) deopts so the
/// interpreter runs the full dispatch.
#[no_mangle]
pub extern "C" fn zen_jit_resolve_method(
    vm: *mut Vm,
    recv_bits: u64,
    const_idx: usize,
    pc: usize,
    closure_bits: u64,
    out: *mut u64,
) -> u32 {
    // SAFETY: the caller passes a live VM and live values for this call.
    let vm = unsafe { &mut *vm };
    let recv = unsafe { Value::from_bits(recv_bits) };
    let Some(proto) = unsafe { Value::from_bits(closure_bits) }.func_proto() else {
        return 1;
    };
    let name = match proto.constants.get(const_idx).copied() {
        Some(n) => n,
        None => return 1,
    };
    if let Some(slots) = recv.as_instance() {
        let class = match slots.first().copied() {
            Some(c) => c,
            None => return 1,
        };
        let method_val = match vm.ic_resolve_method(&proto, pc, class) {
            Some(m) => m,
            None => {
                let name_str = match name.as_str() {
                    Some(s) => s,
                    None => return 1,
                };
                let m = match crate::ops::find_class_method(class, Value::string(name_str)) {
                    Some(m) => m,
                    None => return 1,
                };
                vm.ic_update_method(&proto, pc, class, m);
                m
            }
        };
        unsafe { *out = method_val.to_bits() };
        return 0;
    }
    if let Some(map) = recv.as_dict() {
        let name_str = match name.as_str() {
            Some(s) => s,
            None => return 1,
        };
        if let Some(f) = crate::ops::dict_get(map, Value::string(name_str)) {
            if f.func_proto().is_some() {
                unsafe { *out = f.to_bits() };
                return 0;
            }
        }
        return 1;
    }
    1
}

/// Call a resolved method value from native code. Methods take the receiver
/// in window slot 0 (`is_method`); plain namespace functions do not. Argument
/// mismatches, uncompiled callees and window overflows deopt.
#[no_mangle]
pub extern "C" fn zen_jit_method_call(
    vm: *mut Vm,
    regs: *mut u64,
    base: usize,
    dst: usize,
    recv_first: usize,
    argc: usize,
    method_bits: u64,
    out: *mut u64,
) -> u32 {
    // SAFETY: the JIT frame owns `regs[..]` and passes a live VM/register file.
    let method = unsafe { Value::from_bits(method_bits) };
    let Some(proto) = method.func_proto() else {
        return 1;
    };
    if argc < proto.num_required as usize || argc > proto.params.len() {
        return 1;
    }
    let mbase = if proto.is_method {
        recv_first
    } else {
        match dst.checked_sub(argc) {
            Some(b) => b,
            None => return 1,
        }
    };
    let Some(slot) = proto.jit.get() else {
        return 1;
    };
    if slot.state.load(Ordering::Acquire) != STATE_JITTED {
        crate::jit::note_call(unsafe { &*vm }.jit.as_ref(), &proto);
        return 1;
    }
    let addr = slot.entry.load(Ordering::Acquire);
    if addr == 0 || base + mbase + proto.max_reg as usize > REGISTERS {
        return 1;
    }
    // SAFETY: the compiler published a `NativeEntry`-typed pointer here.
    let entry: NativeEntry = unsafe { std::mem::transmute(addr) };
    // SAFETY: `regs`/`vm` stay valid across the nested native call.
    unsafe { entry(regs, base + mbase, vm, out, method_bits) }
}

/// Monomorphic instance field read with the shape + slot baked in at compile
/// time. Skips the inline-cache lookup entirely (no `RefCell` borrow, no shape
/// table): one shape compare, then a direct flat-slot load. Any miss deopts
/// so the interpreter runs the slow path (which may re-specialize the site
/// with fresh shapes).
#[no_mangle]
pub extern "C" fn zen_jit_get_field_mono(
    obj_bits: u64,
    shape: u32,
    offset: usize,
    out: *mut u64,
) -> u32 {
    // SAFETY: the caller passes a live value for the duration of the call.
    let obj = unsafe { Value::from_bits(obj_bits) };
    if obj.instance_shape() != Some(shape) {
        return 1;
    }
    match obj.as_instance().and_then(|s| s.get(1 + offset).copied()) {
        Some(v) => {
            // SAFETY: `out` is the live result cell the lowerer passed.
            unsafe { *out = v.to_bits() };
            0
        }
        None => 1,
    }
}

/// Monomorphic instance field write with the shape + slot baked in. Mirrors
/// `ic_set_field` minus the cache lookup; a miss deopts.
#[no_mangle]
pub extern "C" fn zen_jit_set_field_mono(
    obj_bits: u64,
    shape: u32,
    offset: usize,
    val_bits: u64,
) -> u32 {
    // SAFETY: the caller passes live values for the duration of the call.
    let obj = unsafe { Value::from_bits(obj_bits) };
    let val = unsafe { Value::from_bits(val_bits) };
    if obj.instance_shape() != Some(shape) {
        return 1;
    }
    let mut obj_mut = obj;
    match obj_mut
        .as_instance_mut()
        .and_then(|slots| slots.get_mut(1 + offset))
    {
        Some(v) => {
            *v = val;
            0
        }
        None => 1,
    }
}

/// `MakeList`/`MakeSet`/`MakeTuple`: build the collection from `count` window
/// slots at `regs[base + first..]`. `kind` is 0 = list, 1 = set, else tuple.
/// Mirrors the interpreter's arm exactly (shared refs, no extra retains).
#[no_mangle]
pub extern "C" fn zen_jit_make_list(
    regs: *const u64,
    base: usize,
    first: usize,
    count: usize,
    kind: u32,
    out: *mut u64,
) -> u32 {
    // SAFETY: the caller passes the live register file; indices were
    // bounds-checked by the lowerer.
    let get = |i: usize| unsafe { Value::from_bits(*regs.add(base + first + i)) };
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        items.push(get(i));
    }
    let v = match kind {
        0 => crate::gc::new_list(items),
        1 => crate::gc::new_set(items.into_iter().collect()),
        _ => crate::gc::new_tuple(items),
    };
    // SAFETY: `out` is the live result cell the lowerer passed.
    unsafe { *out = v.to_bits() };
    0
}

/// `MakeClass dst, name_const=b, parent=c|0xFF`: the class-definition prefix
/// runs once per function entry, so a faithful (unspecialized) bridge is the
/// right trade — it lets the loop below it go native. Mirrors the
/// interpreter's arm; anything unexpected deopts.
#[no_mangle]
pub extern "C" fn zen_jit_make_class(
    closure_bits: u64,
    const_idx: usize,
    parent_bits: u64,
    has_parent: u32,
    out: *mut u64,
) -> u32 {
    // SAFETY: the caller passes a live closure for the duration of the call.
    let Some(proto) = (unsafe { Value::from_bits(closure_bits) }).func_proto() else {
        return 1;
    };
    let name = match proto.constants.get(const_idx).copied() {
        Some(n) => n,
        None => return 1,
    };
    let name_str = match name.as_str() {
        Some(s) => s,
        None => return 1,
    };
    // SAFETY: `parent_bits` is live when `has_parent != 0`.
    let parent = if has_parent == 0 {
        Value::nil()
    } else {
        unsafe { Value::from_bits(parent_bits) }
    };
    let methods = crate::gc::new_dict(indexmap::IndexMap::new());
    let class = crate::gc::new_class(std::sync::Arc::from(name_str), parent, methods);
    // SAFETY: `out` is the live result cell the lowerer passed.
    unsafe { *out = class.to_bits() };
    0
}

/// `AddMethod class=a, name_const=b, method=c`: attach a method to the class
/// under construction. Mirrors the interpreter (direct dict write, no COW);
/// the result is the class itself, so only a status is returned.
#[no_mangle]
pub extern "C" fn zen_jit_add_method(
    closure_bits: u64,
    const_idx: usize,
    class_bits: u64,
    method_bits: u64,
) -> u32 {
    // SAFETY: the caller passes live values for the duration of the call.
    let Some(proto) = (unsafe { Value::from_bits(closure_bits) }).func_proto() else {
        return 1;
    };
    let name = match proto.constants.get(const_idx).copied() {
        Some(n) => n,
        None => return 1,
    };
    let class = unsafe { Value::from_bits(class_bits) };
    let method = unsafe { Value::from_bits(method_bits) };
    let cls = match class.as_class() {
        Some(c) => c,
        None => return 1,
    };
    let mut methods = cls.methods;
    if methods.as_dict().is_some() {
        crate::ops::dict_insert(methods.as_dict_mut(), name, method);
    }
    0
}

/// `MakeClosure` with zero captures: no defining-frame access is needed (there
/// is nothing to capture), so the module id travels as an explicit parameter
/// instead of being read off `frames.last()` — which, under native execution,
/// would be the *caller's* frame. Capturing definers stay interpreted.
#[no_mangle]
pub extern "C" fn zen_jit_make_closure0(
    vm: *const Vm,
    module_id: usize,
    proto_idx: usize,
    out: *mut u64,
) -> u32 {
    // SAFETY: the caller passes a live VM for the duration of the call.
    let vm = unsafe { &*vm };
    let child = match vm
        .module_children(module_id)
        .and_then(|c| c.get(proto_idx))
        .cloned()
    {
        Some(ch) => ch,
        None => return 1,
    };
    let closure = new_func(child, Vec::new());
    // SAFETY: `out` is the live result cell the lowerer passed.
    unsafe { *out = closure.to_bits() };
    0
}

/// `MakeClosureRO` from native code: snapshot the proven-read-only captures
/// straight out of the spilled register window. The lowerer spills every live
/// register first, so the window holds the current values; no interpreter
/// frame (and no promotion) is involved. A non-local descriptor was never
/// proven (the compiler only emits local captures as RO) and deopts so the
/// interpreter runs the exact capture.
#[no_mangle]
pub extern "C" fn zen_jit_make_closure_ro(
    vm: *mut Vm,
    regs: *const u64,
    base: usize,
    module_id: usize,
    proto_idx: usize,
    count: usize,
    max_reg: usize,
    out: *mut u64,
) -> u32 {
    // SAFETY: the caller passes a live VM and its spilled register file.
    let vm = unsafe { &*vm };
    let child = match vm
        .module_children(module_id)
        .and_then(|c| c.get(proto_idx))
        .cloned()
    {
        Some(ch) => ch,
        None => return 1,
    };
    let taken = match child.upvalue_descriptors.get(..count) {
        Some(t) => t,
        None => return 1,
    };
    let mut upvals = Vec::with_capacity(count);
    for desc in taken {
        if !desc.is_local {
            return 1;
        }
        let i = desc.index as usize;
        if i >= max_reg {
            return 1;
        }
        // SAFETY: `i` is bounds-checked; the lowerer spilled the window.
        upvals.push(unsafe { Value::from_bits(*regs.add(base + i)) });
    }
    let closure = new_func(child, upvals);
    // SAFETY: `out` is the live result cell the lowerer passed.
    unsafe { *out = closure.to_bits() };
    0
}
/// Perform a `Call` from native code, with a per-site cache cell holding
/// `(callee_bits, entry_addr, max_reg)` from the last success. On success the
/// cell is refreshed (self-healing across respecialization); anything this
/// bridge does not handle returns `1` so the interpreter re-runs the call.
#[no_mangle]
pub extern "C" fn zen_jit_call_cached(
    vm: *mut Vm,
    regs: *mut u64,
    base: usize,
    first: usize,
    argc: usize,
    ret: usize,
    out: *mut u64,
    cell: *mut u64,
) -> u32 {
    let _ = ret;
    // SAFETY: the JIT frame owns `regs[..]` and passes a live register file.
    let callee_bits = unsafe { *regs.add(base + first - 1) };
    let callee = unsafe { Value::from_bits(callee_bits) };
    let Some(proto) = callee.func_proto() else {
        return 1;
    };
    if argc < proto.num_required as usize || argc > proto.params.len() {
        return 1;
    }
    let Some(slot) = proto.jit.get() else {
        return 1;
    };
    if slot.state.load(Ordering::Acquire) != STATE_JITTED {
        crate::jit::note_call(unsafe { &*vm }.jit.as_ref(), &proto);
        return 1;
    }
    let addr = slot.entry.load(Ordering::Acquire);
    if addr == 0 || base + first + proto.max_reg as usize > REGISTERS {
        return 1;
    }
    // SAFETY: the compiler published a `NativeEntry`-typed pointer here.
    let entry: NativeEntry = unsafe { std::mem::transmute(addr) };
    // SAFETY: `regs`/`vm` stay valid across the nested native call; the callee
    // window `[base+first, base+first+max_reg)` was bounds-checked above.
    let status = unsafe { entry(regs, base + first, vm, out, callee_bits) };
    if status == 0 {
        // SAFETY: `cell` is three live u64s owned by the calling function.
        unsafe {
            *cell.add(0) = callee_bits;
            *cell.add(1) = addr as u64;
            *cell.add(2) = proto.max_reg as u64;
        }
    }
    status
}

/// String (and friends) `+` from native code: the full `arith` fallback for
/// whatever the inline int/float guards decline (strings, ropes, lists,
/// bytes, sets). Errors deopt so the interpreter raises them.
#[no_mangle]
pub extern "C" fn zen_jit_str_add(
    vm: *const Vm,
    l_bits: u64,
    r_bits: u64,
    out: *mut u64,
) -> u32 {
    let _ = vm;
    // SAFETY: the caller passes live values for the duration of the call.
    let (l, r) = unsafe { (Value::from_bits(l_bits), Value::from_bits(r_bits)) };
    match crate::ops::arith(crate::ops::Arith::Add, l, r) {
        Ok(v) => {
            // SAFETY: `out` is the live result cell the lowerer passed.
            unsafe { *out = v.to_bits() };
            0
        }
        Err(_) => 1,
    }
}

/// `CallNative`: invoke a native builtin by id with `argc` window slots at
/// `regs[base + first..]`. Errors deopt so the interpreter raises them.
#[no_mangle]
pub extern "C" fn zen_jit_call_native(
    vm: *mut Vm,
    regs: *const u64,
    base: usize,
    first: usize,
    argc: usize,
    native_id: u64,
    out: *mut u64,
) -> u32 {
    // SAFETY: the caller passes a live VM/register file; indices were
    // bounds-checked by the lowerer.
    let vm = unsafe { &mut *vm };
    let mut args = Vec::with_capacity(argc);
    for i in 0..argc {
        args.push(unsafe { Value::from_bits(*regs.add(base + first + i)) });
    }
    match vm.call_native(native_id as u16, &args) {
        Ok(v) => {
            unsafe { *out = v.to_bits() };
            0
        }
        Err(_) => 1,
    }
}

/// `IterNext`: `seq` lives in `a`, the cursor in `a+1`, the item goes to `b`.
/// Mirrors the interpreter exactly (cursor defaults to 0 when not an int;
/// `iter_get` errors deopt).
#[no_mangle]
pub extern "C" fn zen_jit_iter_next(
    seq_bits: u64,
    cursor_bits: u64,
    item_out: *mut u64,
    cursor_out: *mut u64,
) -> u32 {
    // SAFETY: the caller passes live values for the duration of the call.
    let seq = unsafe { Value::from_bits(seq_bits) };
    let cursor = unsafe { Value::from_bits(cursor_bits) }.as_int().unwrap_or(0);
    match crate::ops::iter_get(seq, cursor) {
        Ok(item) => {
            // SAFETY: both cells are live stack slots the lowerer passed.
            unsafe {
                *item_out = item.to_bits();
                *cursor_out = Value::int(cursor + 1).to_bits();
            }
            0
        }
        Err(_) => 1,
    }
}

/// `IterDone`: done-bit for `seq`/`cursor` into `out`. Infallible, like the
/// interpreter's arm.
#[no_mangle]
pub extern "C" fn zen_jit_iter_done(seq_bits: u64, cursor_bits: u64, out: *mut u64) -> u32 {
    // SAFETY: the caller passes live values for the duration of the call.
    let seq = unsafe { Value::from_bits(seq_bits) };
    let cursor = unsafe { Value::from_bits(cursor_bits) }.as_int().unwrap_or(0);
    let len = crate::ops::iter_len(seq) as i64;
    // SAFETY: `out` is the live result cell the lowerer passed.
    unsafe { *out = Value::bool(cursor >= len).to_bits() };
    0
}

/// `CallMethod` slow-path fast lane: `list.push(x)` / `list.append(x)`.
/// Mirrors `ops::method_call`'s list arm exactly (copy-on-write detach, push,
/// `nil` result). The possibly-detached receiver is written back through
/// `recv_out`, exactly like the interpreter's `set_reg(receiver, recv)`.
/// Anything else deopts to the full interpreter dispatch.
#[no_mangle]
pub extern "C" fn zen_jit_method_push(
    recv_bits: u64,
    arg_bits: u64,
    recv_out: *mut u64,
    out: *mut u64,
) -> u32 {
    // SAFETY: the caller passes live values for the duration of the call.
    let mut recv = unsafe { Value::from_bits(recv_bits) };
    let arg = unsafe { Value::from_bits(arg_bits) };
    if recv.kind() != Some(crate::object::ObjKind::List) {
        return 1;
    }
    crate::value::cow(&mut recv);
    recv.as_list_mut().push(arg);
    // SAFETY: both cells are live stack slots the lowerer passed.
    unsafe {
        *recv_out = recv.to_bits();
        *out = Value::nil().to_bits();
    }
    0
}
