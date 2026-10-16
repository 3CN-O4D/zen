//! Shared value semantics: the operations that both execution engines (the
//! register bytecode VM and the M3 tree-walking reference interpreter) must
//! agree on.
//!
//! Everything here is a pure function over [`Value`] plus the small amount of
//! state a native needs (the `print` sink). It is the single definition of
//! arithmetic, comparison, indexing, stringification and native dispatch, so a
//! differential mismatch between the two engines can only come from
//! *evaluation order / control flow*, never from duplicated operator code.
//!
//! `unsafe` is not used here; the functions borrow through the safe accessors
//! in [`crate::value`].

use std::cmp::Ordering;

use indexmap::{IndexMap, IndexSet};

use crate::errors::{VmError, VmResult};
use crate::gc::{new_bound_method, new_dict, new_list, new_rope, new_set, new_tuple};
use crate::object::{ObjKind, RopeObj};
use crate::value::{Tag, Value};

// ── truthiness / equality / ordering ─────────────────────────────────────────

/// Zen truthiness: everything is truthy except `nil` and `false`.
#[inline]
pub fn truthy(v: Value) -> bool {
    !(v.is_nil() || matches!(v.as_bool(), Some(false)))
}

/// Structural equality with numeric coercion (`1 == 1.0`).
pub fn values_equal(a: Value, b: Value) -> bool {
    crate::value::value_eq(a, b)
}

/// Compare two numbers, or `None` when either is NaN.
pub fn num_cmp(a: Value, b: Value) -> Option<Ordering> {
    match (a.as_int(), b.as_int()) {
        (Some(x), Some(y)) => Some(x.cmp(&y)),
        _ => {
            let x = to_f64(a)?;
            let y = to_f64(b)?;
            x.partial_cmp(&y)
        }
    }
}

/// Total order over the comparable types (numbers and strings).
pub fn compare_order(l: Value, r: Value) -> VmResult<Ordering> {
    if let (Some(x), Some(y)) = (l.as_str(), r.as_str()) {
        return Ok(x.cmp(y));
    }
    if l.is_number() && r.is_number() {
        return num_cmp(l, r).ok_or_else(|| VmError::ValueError("cannot order NaN".into()));
    }
    Err(VmError::type_mismatch("ordered value", l))
}

/// Numeric view of an `int` or `float`.
#[inline]
pub fn to_f64(v: Value) -> Option<f64> {
    v.as_float().or_else(|| v.as_int().map(|i| i as f64))
}

// ── arithmetic ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arith {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bit {
    And,
    Or,
    Xor,
    Shl,
    Shr,
}

pub fn arith(op: Arith, l: Value, r: Value) -> VmResult<Value> {
    if op == Arith::Add {
        if let (Some(a), Some(b)) = (l.as_list(), r.as_list()) {
            let mut out = a.to_vec();
            out.extend_from_slice(b);
            return Ok(new_list(out));
        }
        if let (Some(a), Some(b)) = (l.as_bytes(), r.as_bytes()) {
            let mut out = a.to_vec();
            out.extend_from_slice(b);
            return Ok(Value::bytes(out));
        }
        if let (Some(a), Some(b)) = (l.as_set(), r.as_set()) {
            let mut out = a.clone();
            out.extend(b.iter().copied());
            return Ok(new_set(out));
        }
    }
    if op == Arith::Add && (l.is_string() || r.is_string()) {
        return string_add(l, r);
    }
    if op == Arith::Mul {
        if let (Some(s), Some(n)) = (l.as_str(), r.as_int()) {
            return repeat_str(s, n).map(Value::string);
        }
        if let (Some(n), Some(s)) = (l.as_int(), r.as_str()) {
            return repeat_str(s, n).map(Value::string);
        }
    }
    match (l.as_int(), r.as_int()) {
        (Some(x), Some(y)) => int_arith(op, x, y),
        _ => {
            let x = to_f64(l).ok_or_else(|| VmError::type_mismatch("number", l))?;
            let y = to_f64(r).ok_or_else(|| VmError::type_mismatch("number", r))?;
            float_arith(op, x, y)
        }
    }
}

/// String concatenation for `Arith::Add`. Small flat+flat results stay eager
/// (cheaper than a node); everything else becomes a rope node linking the two
/// sides by reference — the left side is never recopied, so append loops are
/// O(1) per iteration instead of O(n). Non-string operands are stringified
/// once into fresh flat values. Appending never mutates, so sharing a rope
/// head (or a flat side) through another reference is always sound, no matter
/// how loose the surrounding refcount discipline is.
pub fn string_add(l: Value, r: Value) -> VmResult<Value> {
    /// Eager-concat budget: below this total, a flat copy beats a node.
    const EAGER_MAX: usize = 64;
    if l.tag() == Tag::String && r.tag() == Tag::String {
        let (ls, rs) = (l.as_str().unwrap_or(""), r.as_str().unwrap_or(""));
        if ls.len() + rs.len() <= EAGER_MAX {
            let mut out = String::with_capacity(ls.len() + rs.len());
            out.push_str(ls);
            out.push_str(rs);
            return Ok(Value::string(out));
        }
    }
    let lv = if l.is_string() {
        l
    } else {
        Value::string(stringify(l))
    };
    let rv = if r.is_string() {
        r
    } else {
        Value::string(stringify(r))
    };
    let (lc, lb) = str_lens(lv);
    let (rc, rb) = str_lens(rv);
    Ok(new_rope(lv, rv, lc + rc, lb + rb))
}

/// `(chars, bytes)` of a string value without flattening: flat strings scan
/// once (callers only pay this on the flat→rope transition — afterwards the
/// value is a rope and the totals ride along), ropes read their totals.
fn str_lens(v: Value) -> (usize, usize) {
    if v.tag() == Tag::String {
        let s = v.as_str().unwrap_or("");
        return (s.chars().count(), s.len());
    }
    if v.kind() == Some(ObjKind::Rope) {
        // SAFETY: kind-checked live node; the totals are plain fields.
        let o = unsafe { &*(v.header_unchecked().as_ptr() as *const RopeObj) };
        return (o.chars, o.bytes);
    }
    (0, 0) // Unreachable for live strings; defensive.
}

/// `(chars, bytes)` of any string value, O(1) for ropes.
pub fn rope_lens(v: Value) -> Option<(usize, usize)> {
    if v.tag() == Tag::String {
        let s = v.as_str()?;
        return Some((s.chars().count(), s.len()));
    }
    if v.kind() == Some(ObjKind::Rope) {
        // SAFETY: kind-checked live node; the totals are plain fields.
        let o = unsafe { &*(v.header_unchecked().as_ptr() as *const RopeObj) };
        return Some((o.chars, o.bytes));
    }
    None
}

/// Flatten a rope-string value into one contiguous buffer. Fully iterative
/// (rope depth is unbounded in append loops): nested ropes whose cache is
/// already filled count as single leaves, so repeated reads stay cheap. Raw
/// part pointers are copied in one pass rather than borrowed, sidestepping
/// borrow lifetimes — the tree is rooted at `v` (alive for the call) and no
/// frees interleave a synchronous walk; leaves are valid UTF-8 concatenated
/// at leaf boundaries, so the result is valid UTF-8.
pub fn flatten_rope(v: Value) -> Box<str> {
    let mut parts: Vec<(*const u8, usize)> = Vec::new();
    let mut total = 0usize;
    let mut stack = vec![v];
    while let Some(n) = stack.pop() {
        if n.tag() == Tag::String {
            if let Some(s) = n.as_str() {
                total += s.len();
                parts.push((s.as_ptr(), s.len()));
            }
            continue;
        }
        // SAFETY: rope nodes are only ever built from strings; anything else
        // here would be corruption, skipped defensively (callers pass live
        // ropes, so this is unreachable — never a wrong answer).
        let obj = match n.kind() {
            Some(ObjKind::Rope) => unsafe {
                &*(n.header_unchecked().as_ptr() as *const RopeObj)
            },
            _ => continue,
        };
        if let Some(flat) = obj.flat.get() {
            total += flat.len();
            parts.push((flat.as_ptr(), flat.len()));
            continue;
        }
        stack.push(obj.right);
        stack.push(obj.left);
    }
    let mut buf = Vec::with_capacity(total);
    for (p, len) in parts {
        // SAFETY: parts point at live string buffers (see above); the copy
        // is bounds-checked by construction.
        buf.extend_from_slice(unsafe { std::slice::from_raw_parts(p, len) });
    }
    // SAFETY: leaves are valid UTF-8 joined at leaf (char) boundaries.
    unsafe { String::from_utf8_unchecked(buf) }.into_boxed_str()
}

fn int_arith(op: Arith, a: i64, b: i64) -> VmResult<Value> {
    let overflow_float =
        |a: i64, b: i64, f: fn(f64, f64) -> f64| Value::float(f(a as f64, b as f64));
    let v = match op {
        Arith::Add => a
            .checked_add(b)
            .map(Value::int_promote)
            .unwrap_or_else(|| overflow_float(a, b, |x, y| x + y)),
        Arith::Sub => a
            .checked_sub(b)
            .map(Value::int_promote)
            .unwrap_or_else(|| overflow_float(a, b, |x, y| x - y)),
        Arith::Mul => a
            .checked_mul(b)
            .map(Value::int_promote)
            .unwrap_or_else(|| overflow_float(a, b, |x, y| x * y)),
        Arith::Div => {
            if b == 0 {
                return Err(VmError::DivideByZero("division by zero".into()));
            }
            Value::float(a as f64 / b as f64)
        }
        Arith::Mod => {
            if b == 0 {
                return Err(VmError::DivideByZero("modulo by zero".into()));
            }
            a.checked_rem(b)
                .map(Value::int_promote)
                .unwrap_or_else(|| overflow_float(a, b, |x, y| x % y))
        }
        Arith::Pow => {
            if b < 0 {
                Value::float((a as f64).powf(b as f64))
            } else if b > u32::MAX as i64 {
                overflow_float(a, b, |x, y| x.powf(y))
            } else {
                a.checked_pow(b as u32)
                    .map(Value::int_promote)
                    .unwrap_or_else(|| overflow_float(a, b, |x, y| x.powf(y)))
            }
        }
    };
    Ok(v)
}

fn float_arith(op: Arith, a: f64, b: f64) -> VmResult<Value> {
    let v = match op {
        Arith::Add => Value::float(a + b),
        Arith::Sub => Value::float(a - b),
        Arith::Mul => Value::float(a * b),
        Arith::Div => {
            if b == 0.0 {
                return Err(VmError::DivideByZero("division by zero".into()));
            }
            Value::float(a / b)
        }
        Arith::Mod => {
            if b == 0.0 {
                return Err(VmError::DivideByZero("modulo by zero".into()));
            }
            Value::float(a % b)
        }
        Arith::Pow => Value::float(a.powf(b)),
    };
    Ok(v)
}

pub fn bitwise(op: Bit, l: Value, r: Value) -> VmResult<Value> {
    let a = l.as_int().ok_or_else(|| VmError::type_mismatch("int", l))?;
    let b = r.as_int().ok_or_else(|| VmError::type_mismatch("int", r))?;
    let shift = (b & 63) as u32;
    let v = match op {
        Bit::And => a & b,
        Bit::Or => a | b,
        Bit::Xor => a ^ b,
        Bit::Shl => a.wrapping_shl(shift),
        Bit::Shr => a.wrapping_shr(shift),
    };
    Ok(Value::int_promote(v))
}

pub fn bitnot(v: Value) -> VmResult<Value> {
    let i = v.as_int().ok_or_else(|| VmError::type_mismatch("int", v))?;
    Ok(Value::int_promote(!i))
}

pub fn negate(v: Value) -> VmResult<Value> {
    match (v.as_int(), v.as_float()) {
        (Some(i), _) => Ok(Value::int_promote(i.checked_neg().ok_or_else(|| {
            VmError::ValueError("integer overflow in negation".into())
        })?)),
        (_, Some(f)) => Ok(Value::float(-f)),
        _ => Err(VmError::type_mismatch("number", v)),
    }
}

// ── containers ───────────────────────────────────────────────────────────────

pub fn dict_insert(map: &mut IndexMap<crate::value::DictKey, Value>, key: Value, val: Value) {
    // `IndexMap::insert` replaces the value for an equivalent key in place
    // (keeping the original key and its position), so one hash op does it all.
    map.insert(crate::value::DictKey(key), val);
}

/// Hash-lookup read of a dict slot. Keys compare by [`value_eq`] (numeric
/// int/float unify, strings by content), so this is O(1), not a scan.
pub fn dict_get(map: &IndexMap<crate::value::DictKey, Value>, key: Value) -> Option<Value> {
    map.get(&crate::value::DictKey(key)).copied()
}

pub fn normalize_index(index: i64, len: usize) -> Option<usize> {
    let len = len as i64;
    let idx = if index < 0 { index + len } else { index };
    if idx < 0 || idx >= len {
        None
    } else {
        Some(idx as usize)
    }
}

pub fn get_index(obj: Value, key: Value) -> VmResult<Value> {
    if let Some(list) = obj.as_list() {
        let i = key
            .as_int()
            .ok_or_else(|| VmError::type_mismatch("int", key))?;
        let idx = normalize_index(i, list.len()).ok_or(VmError::IndexError {
            index: i,
            len: list.len(),
        })?;
        return Ok(list[idx]);
    }
    if let Some(tuple) = obj.as_tuple() {
        let i = key
            .as_int()
            .ok_or_else(|| VmError::type_mismatch("int", key))?;
        let idx = normalize_index(i, tuple.len()).ok_or(VmError::IndexError {
            index: i,
            len: tuple.len(),
        })?;
        return Ok(tuple[idx]);
    }
    if let Some(s) = obj.as_str() {
        let i = key
            .as_int()
            .ok_or_else(|| VmError::type_mismatch("int", key))?;
        let chars: Vec<char> = s.chars().collect();
        let idx = normalize_index(i, chars.len()).ok_or(VmError::IndexError {
            index: i,
            len: chars.len(),
        })?;
        return Ok(Value::string(chars[idx].to_string()));
    }
    if let Some(dict) = obj.as_dict() {
        return dict_get(dict, key).ok_or_else(|| VmError::KeyError(stringify(key)));
    }
    Err(VmError::type_mismatch("indexable", obj))
}

pub fn set_index(obj: Value, key: Value, val: Value) -> VmResult<Value> {
    if obj.as_list().is_some() {
        let i = key
            .as_int()
            .ok_or_else(|| VmError::type_mismatch("int", key))?;
        let mut obj = obj;
        crate::value::cow(&mut obj);
        let list = obj.as_list_mut();
        let idx = normalize_index(i, list.len()).ok_or(VmError::IndexError {
            index: i,
            len: list.len(),
        })?;
        list[idx] = val;
        return Ok(obj);
    }
    if obj.as_dict().is_some() {
        let mut obj = obj;
        crate::value::cow(&mut obj);
        dict_insert(obj.as_dict_mut(), key, val);
        return Ok(obj);
    }
    Err(VmError::type_mismatch("mutable indexed value", obj))
}

pub fn get_field(obj: Value, name: Value) -> VmResult<Value> {
    if let Some(dict) = obj.as_dict() {
        return dict_get(dict, name).ok_or_else(|| VmError::KeyError(stringify(name)));
    }
    if let Some(slots) = obj.as_instance() {
        // Flat layout: `slots[0]` is the class, `slots[1 + offset]` the field
        // values, with offsets from the instance's shape. Only string names
        // have shape slots (the parser only ever emits those); anything else
        // falls through to the method/nil path below with the original name.
        if let Some(name_str) = name.as_str() {
            let shape = obj.instance_shape().unwrap_or(0);
            if let Some(off) = crate::shape::shape_offset(shape, name_str) {
                if let Some(v) = slots.get(1 + off).copied() {
                    return Ok(v);
                }
            }
        }
        if let Some(class) = slots.first().copied() {
            if let Some(method) = find_class_method(class, name) {
                return Ok(new_bound_method(obj, method));
            }
        }
        // Unknown attribute on an instance reads as `nil` (lenient, matches
        // the reference interpreter's field access).
        return Ok(Value::nil());
    }
    Err(VmError::type_mismatch("object with fields", obj))
}

pub fn set_field(obj: Value, name: Value, val: Value) -> VmResult<Value> {
    if obj.as_dict().is_some() {
        let mut obj = obj;
        crate::value::cow(&mut obj);
        dict_insert(obj.as_dict_mut(), name, val);
        return Ok(obj);
    }
    if obj.as_instance().is_some() {
        // Flat fields, written in place like the interpreter always has (no
        // copy-on-write for instances — aliases observe the write, matching
        // the pre-existing behavior). New names transition the shape and grow
        // the slots; the offset of a name under a shape is immutable, so the
        // slot index is exact. Non-string names have no shape slot (the
        // parser only emits strings); the write is a no-op returning the
        // receiver, and reads of it hit the lenient-`nil` path above.
        let mut obj_mut = obj;
        let shape = obj_mut.instance_shape().unwrap_or(0);
        if let Some(name_str) = name.as_str() {
            let off = match crate::shape::shape_offset(shape, name_str) {
                Some(o) => o,
                None => {
                    // A new property's slot is the pre-transition length:
                    // shapes never shrink, so this is exact and race-free.
                    let o = crate::shape::shape_len(shape);
                    let next = crate::shape::shape_transition(shape, name_str);
                    debug_assert_eq!(
                        crate::shape::shape_offset(next, name_str),
                        Some(o)
                    );
                    obj_mut.set_instance_shape(next);
                    o
                }
            };
            if let Some(slots) = obj_mut.as_instance_mut() {
                while slots.len() < off + 2 {
                    slots.push(Value::nil());
                }
                slots[1 + off] = val;
            }
        }
        return Ok(obj_mut);
    }
    Err(VmError::type_mismatch("object with fields", obj))
}

/// Walk a class and its parents for a method named `name`. The single
/// definition of class method lookup shared by the VM's dispatch and field
/// access.
pub fn find_class_method(class: Value, name: Value) -> Option<Value> {
    let mut cur = class;
    loop {
        let c = cur.as_class()?;
        if let Some(d) = c.methods.as_dict() {
            if let Some(m) = dict_get(d, name) {
                return Some(m);
            }
        }
        let parent = c.parent;
        parent.as_class()?;
        cur = parent;
    }
}

pub fn iter_len(seq: Value) -> usize {
    if let Some(l) = seq.as_list() {
        l.len()
    } else if let Some(t) = seq.as_tuple() {
        t.len()
    } else if let Some(s) = seq.as_str() {
        s.chars().count()
    } else if let Some(s) = seq.as_set() {
        s.len()
    } else if let Some(d) = seq.as_dict() {
        d.len()
    } else if let Some(b) = seq.as_bytes() {
        b.len()
    } else {
        0
    }
}

/// `subject[start:end]`, with `None` for an omitted bound.
///
/// The sliceable kinds and the bound arithmetic are the oracle's, verbatim
/// (`slice_value`/`slice_bounds` in `zen-interp`): a list slices to a list, a
/// tuple to a tuple, and a string to a string **by `char`** rather than by
/// byte, so the indices line up with what a user sees. Both ends clamp into
/// `0..=len`, a negative index counts from the end, and an end below the start
/// yields an empty result rather than an error.
pub fn slice(subject: Value, start: Option<i64>, end: Option<i64>) -> VmResult<Value> {
    if let Some(list) = subject.as_list() {
        let (a, b) = slice_bounds(start, end, list.len());
        return Ok(new_list(list[a..b].to_vec()));
    }
    if let Some(tuple) = subject.as_tuple() {
        let (a, b) = slice_bounds(start, end, tuple.len());
        return Ok(new_tuple(tuple[a..b].to_vec()));
    }
    if let Some(s) = subject.as_str() {
        let chars: Vec<char> = s.chars().collect();
        let (a, b) = slice_bounds(start, end, chars.len());
        return Ok(Value::string(chars[a..b].iter().collect::<String>()));
    }
    Err(VmError::type_mismatch("sliceable", subject))
}

/// Resolve a pair of bounds to a `[start, end)` window that is always in range.
fn slice_bounds(start: Option<i64>, end: Option<i64>, len: usize) -> (usize, usize) {
    let len_i = len as i64;
    let norm = |i: i64| -> i64 {
        let i = if i < 0 { i + len_i } else { i };
        i.clamp(0, len_i)
    };
    let a = norm(start.unwrap_or(0));
    let b = norm(end.unwrap_or(len_i));
    (a as usize, b.max(a) as usize)
}

pub fn iter_get(seq: Value, index: i64) -> VmResult<Value> {
    let i = index.max(0) as usize;
    if let Some(l) = seq.as_list() {
        return Ok(l.get(i).copied().unwrap_or(Value::nil()));
    }
    if let Some(t) = seq.as_tuple() {
        return Ok(t.get(i).copied().unwrap_or(Value::nil()));
    }
    if let Some(s) = seq.as_str() {
        return Ok(Value::string(
            s.chars().nth(i).map(|c| c.to_string()).unwrap_or_default(),
        ));
    }
    if let Some(s) = seq.as_set() {
        return Ok(s.get_index(i).copied().unwrap_or(Value::nil()));
    }
    if let Some(d) = seq.as_dict() {
        return Ok(d.get_index(i).map(|(k, _)| k.0).unwrap_or(Value::nil()));
    }
    if let Some(b) = seq.as_bytes() {
        return Ok(b
            .get(i)
            .map(|x| Value::int(*x as i64))
            .unwrap_or(Value::nil()));
    }
    Err(VmError::type_mismatch("iterable", seq))
}

// ── stringification ──────────────────────────────────────────────────────────

pub fn string_arg(args: &[Value], i: usize) -> VmResult<String> {
    let v = *args.get(i).ok_or(VmError::ArgumentCount {
        expected: i + 1,
        got: args.len(),
    })?;
    v.as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| VmError::type_mismatch("string", v))
}

pub fn int_arg(args: &[Value], i: usize) -> VmResult<i64> {
    let v = *args.get(i).ok_or(VmError::ArgumentCount {
        expected: i + 1,
        got: args.len(),
    })?;
    v.as_int().ok_or_else(|| VmError::type_mismatch("int", v))
}

/// Largest string a single repetition/padding operation may allocate. Guards
/// against `"x" * 8**64`-style inputs that would otherwise request an
/// astronomically large buffer and abort the process instead of raising a
/// catchable error. (Aggregate limits are the job of the M9 memory limit.)
const MAX_STRING_LEN: usize = 256 * 1024 * 1024;

/// Repeat a string `n` times (negative counts clamp to empty). Errors if the
/// resulting length would exceed [`MAX_STRING_LEN`] or overflow `usize`.
fn repeat_str(s: &str, n: i64) -> VmResult<String> {
    if n <= 0 {
        return Ok(String::new());
    }
    let len = s
        .len()
        .checked_mul(n as usize)
        .ok_or_else(|| VmError::ValueError("string repetition is too large".into()))?;
    if len > MAX_STRING_LEN {
        return Err(VmError::ValueError(
            "string repetition would exceed the maximum string length".into(),
        ));
    }
    Ok(s.repeat(n as usize))
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
    }
}

fn zfill(s: &str, width: i64) -> VmResult<String> {
    let len = s.chars().count();
    if width <= len as i64 {
        return Ok(s.to_string());
    }
    let pad = (width as usize) - len;
    if pad > MAX_STRING_LEN {
        return Err(VmError::ValueError(
            "zfill width would exceed the maximum string length".into(),
        ));
    }
    let (sign, rest) = if let Some(r) = s.strip_prefix('-') {
        ("-", r)
    } else if let Some(r) = s.strip_prefix('+') {
        ("+", r)
    } else {
        ("", s)
    };
    Ok(format!("{sign}{}{rest}", "0".repeat(pad)))
}

/// Parse a string as an `int` if possible, else a `float`, else `0`.
pub fn parse_num(s: &str) -> Value {
    let t = s.trim();
    if let Ok(i) = t.parse::<i64>() {
        Value::int_promote(i)
    } else if let Ok(f) = t.parse::<f64>() {
        Value::float(f)
    } else {
        Value::int(0)
    }
}

/// Human-readable rendering used by `print` and string concatenation.
pub fn stringify(v: Value) -> String {
    if v.is_nil() {
        "nil".to_string()
    } else if let Some(b) = v.as_bool() {
        b.to_string()
    } else if let Some(i) = v.as_int() {
        i.to_string()
    } else if let Some(f) = v.as_float() {
        stringify_float(f)
    } else if let Some(s) = v.as_str() {
        s.to_string()
    } else if let Some(b) = v.as_bytes() {
        String::from_utf8_lossy(b).into_owned()
} else if let Some(dict) = v.as_dict() {
        // Objects are dicts with no class support; a `str` field (as written
        // by `std/uuid.z`) supplies the canonical form, matching Python's
        // `__str__`. String keys hash by identity, so compare contents.
        let mut out = None;
        for (k, val) in dict.iter() {
            if k.0.as_str() == Some("str") {
                if let Some(s) = val.as_str() {
                    out = Some(s.to_string());
                }
                break;
            }
        }
        out.unwrap_or_else(|| format!("<{}>", v.type_name()))
    } else {
        format!("<{}>", v.type_name())
    }
}

/// Float rendering shared by `print`, `str()` and concatenation.
pub fn stringify_float(f: f64) -> String {
    if f.fract() == 0.0 && f.is_finite() {
        format!("{f:.1}")
    } else {
        format!("{f}")
    }
}

// ── native function dispatch ─────────────────────────────────────────────────
//
// Native ids are the ABI the compiler emits (`crates/zen-compiler` BUILTINS):
// 0 print, 1 str, 2 int, 3 float, 4 bool, 5 len, 6 max, 7 min, 8 sum, 9 abs,
// 10 exit. `output` receives `print`'s text.

pub fn native_call(id: u16, args: &[Value], output: &mut String) -> VmResult<Value> {
    match id {
        0 => {
            let mut line = String::new();
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    line.push(' ');
                }
                line.push_str(&stringify(*a));
            }
            line.push('\n');
            output.push_str(&line);
            // need to append - easier to push via mutable access?
            // better approach: access mutable field? output is String, we can use a method if we add one
            // but let us add helper: append_output
            Ok(Value::nil())
        }
        1 => {
            let v = *args.first().ok_or(VmError::ArgumentCount {
                expected: 1,
                got: 0,
            })?;
            Ok(Value::string(stringify(v)))
        }
        2 => {
            let v = *args.first().ok_or(VmError::ArgumentCount {
                expected: 1,
                got: 0,
            })?;
            if let Some(s) = v.as_str() {
                // `int("42")` parses; a non-integer string is a ValueError.
                return s
                    .trim()
                    .parse::<i64>()
                    .map(Value::int_promote)
                    .map_err(|_| VmError::ValueError(format!("invalid literal for int(): '{s}'")));
            }
            let i = match v.as_int() {
                Some(i) => i,
                None => v
                    .as_float()
                    .map(|f| f as i64)
                    .ok_or_else(|| VmError::type_mismatch("number", v))?,
            };
            Ok(Value::int_promote(i))
        }
        3 => {
            let v = *args.first().ok_or(VmError::ArgumentCount {
                expected: 1,
                got: 0,
            })?;
            if let Some(s) = v.as_str() {
                let f = s.trim().parse::<f64>().map_err(|_| {
                    VmError::ValueError(format!("could not convert string to float: {s:?}"))
                })?;
                return Ok(Value::float(f));
            }
            let f = v
                .as_float()
                .or_else(|| v.as_int().map(|i| i as f64))
                .ok_or_else(|| VmError::type_mismatch("number", v))?;
            Ok(Value::float(f))
        }
        4 => {
            let v = *args.first().ok_or(VmError::ArgumentCount {
                expected: 1,
                got: 0,
            })?;
            Ok(Value::bool(truthy(v)))
        }
        5 => {
            let v = *args.first().ok_or(VmError::ArgumentCount {
                expected: 1,
                got: 0,
            })?;
            Ok(Value::int(iter_len(v) as i64))
        }
        6 => {
            let items = arg_items(args)?;
            if items.is_empty() {
                return Err(VmError::ValueError("max() arg is empty".into()));
            }
            let mut max_v = items[0];
            for item in &items[1..] {
                if compare_order(*item, max_v)? == Ordering::Greater {
                    max_v = *item;
                }
            }
            Ok(max_v)
        }
        7 => {
            let items = arg_items(args)?;
            if items.is_empty() {
                return Err(VmError::ValueError("min() arg is empty".into()));
            }
            let mut min_v = items[0];
            for item in &items[1..] {
                if compare_order(*item, min_v)? == Ordering::Less {
                    min_v = *item;
                }
            }
            Ok(min_v)
        }
        8 => {
            let items = arg_items(args)?;
            let mut sum_v = Value::int(0);
            for item in &items {
                sum_v = arith(Arith::Add, sum_v, *item)?;
            }
            Ok(sum_v)
        }
        9 => {
            let v = *args.first().ok_or(VmError::ArgumentCount {
                expected: 1,
                got: 0,
            })?;
            if let Some(i) = v.as_int() {
                Ok(Value::int_promote(i.checked_abs().unwrap_or(i64::MAX)))
            } else {
                let f = v
                    .as_float()
                    .or_else(|| v.as_int().map(|i| i as f64))
                    .ok_or_else(|| VmError::type_mismatch("number", v))?;
                Ok(Value::float(f.abs()))
            }
        }
        10 => Ok(Value::nil()),
        // Container constructors and reflection. These mirror the reference
        // interpreter exactly (same iteration helpers) so the two engines agree.
        12 => Ok(new_list(opt_items(args)?)),
        13 => Ok(new_tuple(one_items(args)?)),
        14 => {
            let items: IndexSet<Value> = opt_items(args)?.into_iter().collect();
            Ok(new_set(items))
        }
        15 => {
            let mut map = IndexMap::new();
            for item in opt_items(args)? {
                if let Some(pair) = item.as_list() {
                    if pair.len() == 2 {
                        dict_insert(&mut map, pair[0], pair[1]);
                    }
                }
            }
            Ok(new_dict(map))
        }
        16 => {
            let mut nums = Vec::with_capacity(args.len());
            for a in args {
                let n = a
                    .as_int()
                    .ok_or_else(|| VmError::type_mismatch("int", *a))?;
                nums.push(n);
            }
            let (start, stop, step) = match nums.as_slice() {
                [stop] => (0, *stop, 1),
                [start, stop] => (*start, *stop, 1),
                [start, stop, step] => (*start, *stop, *step),
                _ => {
                    return Err(VmError::ArgumentCount {
                        expected: 1,
                        got: args.len(),
                    })
                }
            };
            let mut out = Vec::new();
            let mut i = start;
            if step > 0 {
                while i < stop {
                    out.push(Value::int_promote(i));
                    i += step;
                }
            } else if step < 0 {
                while i > stop {
                    out.push(Value::int_promote(i));
                    i += step;
                }
            }
            Ok(new_list(out))
        }
        17 => {
            let v = *args.first().ok_or(VmError::ArgumentCount {
                expected: 1,
                got: 0,
            })?;
            let name = if v.is_nil() {
                "null"
            } else if v.as_instance().is_some() {
                "instance"
            } else if v.as_class().is_some() {
                "class"
            } else if v.func_proto().is_some() || v.as_bound_method().is_some() {
                "func"
            } else {
                v.type_name()
            };
            Ok(Value::string(name))
        }
        18 => {
            let items = one_items(args)?;
            let out = items
                .into_iter()
                .enumerate()
                .map(|(i, item)| new_list(vec![Value::int_promote(i as i64), item]))
                .collect();
            Ok(new_list(out))
        }
        19 => {
            // cli_args(): return list of command-line arguments (excluding program name)
            let args: Vec<Value> = std::env::args().skip(1).map(Value::string).collect();
            Ok(new_list(args))
        }
        20 => {
            // zen_version(): return the Zen version string
            Ok(Value::string(env!("CARGO_PKG_VERSION")))
        }
        21 => {
            // os_platform(): return the OS platform identifier
            Ok(Value::string(std::env::consts::OS))
        }
        22 => chr(args),
        // std/fs (100-104)
        100 => fs_read_file(args),
        101 => fs_write_file(args),
        102 => fs_exists(args),
        103 => fs_mkdir(args),
        104 => fs_remove(args),
        160 => fs_mkdirs(args),
        161 => fs_is_dir(args),
        162 => fs_is_file(args),
        163 => fs_rmdir(args),
        175 => fs_rmtree(args),
        164 => fs_list(args),
        165 => fs_join(args),
        166 => fs_basename(args),
        167 => fs_dirname(args),
        168 => fs_copy(args),
        169 => fs_move(args),
        176 => fs_read_binary(args),
        177 => fs_write_binary(args),
        178 => fs_size(args),
        // std/os (105-108)
        105 => os_env_get(args),
        106 => os_env_set(args),
        107 => os_cwd(args),
        108 => os_exit(args, output),
        // std/time (109-110, 113)
        109 => time_now_ms(args),
        110 => time_sleep(args),
        113 => time_unix(args),
        // std/os cpu_count (114)
        114 => Ok(Value::int(
            std::thread::available_parallelism()
                .map(|n| n.get() as i64)
                .unwrap_or(1),
        )),
        // std/json (111-112)
        111 => crate::native::json_parse(args),
        112 => crate::native::json_stringify(args),
        // std/ffi (120-129, 139, 148-149, 153-156)
        120 => crate::native::ffi::ffi_load(args),
        121 => crate::native::ffi::ffi_symbol(args),
        122 => crate::native::ffi::ffi_call(args),
        123 => crate::native::ffi::ffi_close(args),
        124 => crate::native::ffi::ffi_call_at(args),
        125 => crate::native::ffi::ffi_malloc(args),
        126 => crate::native::ffi::ffi_realloc(args),
        127 => crate::native::ffi::ffi_free(args),
        128 => crate::native::ffi::ffi_read(args),
        129 => crate::native::ffi::ffi_write(args),
        139 => crate::native::ffi::ffi_str(args),
        148 => crate::native::ffi::ffi_set(args),
        149 => crate::native::ffi::ffi_copy(args),
        153 => crate::native::ffi::ffi_errno(args),
        154 => crate::native::ffi::ffi_strerror(args),
        155 => crate::native::ffi::ffi_sizeof(args),
        156 => crate::native::ffi::ffi_alignof(args),
        // std/socket (500-599)
        500..=599 => crate::native::socket::dispatch(id, args),
        // net.dns (600-619)
        600..=619 => crate::native::dns::dispatch(id, args),
        // requests (700-759)
        700..=759 => crate::native::requests::dispatch(id, args),
        // http (760-779)
        760..=779 => crate::native::http::dispatch(id, args),
        // http.server (780-799)
        780..=799 => crate::native::http_server::dispatch(id, args),
        // urllib.parse (800-819)
        800..=819 => crate::native::urllib_parse::dispatch(id, args),
        // urllib.request (820-839)
        820..=839 => crate::native::urllib_request::dispatch(id, args),
        // urllib umbrella (840-859)
        840..=859 => crate::native::urllib::dispatch(id, args),
        // html (860-879)
        860..=879 => crate::native::html::dispatch(id, args),
        // html.parser (880-899)
        880..=899 => crate::native::html_parser::dispatch(id, args),
        // lxml.etree (900-929)
        900..=929 => crate::native::lxml_etree::dispatch(id, args),
        // lxml.html (930-949)
        930..=949 => crate::native::lxml_html::dispatch(id, args),
        // lxml umbrella (950-959)
        950..=959 => crate::native::lxml::dispatch(id, args),
        // std/crypto (210-224)
        210 => crate::native::crypto::crypto_hash(args),
        211 => crate::native::crypto::crypto_md5(args),
        212 => crate::native::crypto::crypto_sha1(args),
        213 => crate::native::crypto::crypto_sha224(args),
        214 => crate::native::crypto::crypto_sha256(args),
        215 => crate::native::crypto::crypto_sha384(args),
        216 => crate::native::crypto::crypto_sha512(args),
        217 => crate::native::crypto::crypto_sha3_256(args),
        218 => crate::native::crypto::crypto_sha3_512(args),
        219 => crate::native::crypto::crypto_blake2b(args),
        220 => crate::native::crypto::crypto_blake2s(args),
        221 => crate::native::crypto::crypto_hmac(args),
        222 => crate::native::crypto::crypto_pbkdf2(args),
        223 => crate::native::crypto::crypto_hex_encode(args),
        224 => crate::native::crypto::crypto_hex_decode(args),
        other => {
            if let Some(v) = crate::native::sqlite::dispatch(other, args)? {
                return Ok(v);
            }
            if let Some(v) = crate::native::qr::dispatch(other, args)? {
                return Ok(v);
            }
            if let Some(v) = crate::native::wa::dispatch(other, args)? {
                return Ok(v);
            }
            if let Some(v) = crate::native::ffmpeg::dispatch(other, args)? {
                return Ok(v);
            }
            if let Some(v) = crate::native::hydra::dispatch(other, args)? {
                return Ok(v);
            }
            if let Some(v) = crate::native::sys::dispatch(other, args, output)? {
                return Ok(v);
            }
            crate::native::std_tiers::dispatch(other, args)
        }
    }
}

/// `list(x)` with no argument is the empty list; otherwise iterate `x`.
fn opt_items(args: &[Value]) -> VmResult<Vec<Value>> {
    match args.first() {
        None => Ok(Vec::new()),
        Some(v) => iter_items(*v),
    }
}

/// `tuple(x)` / `set(x)` / `enumerate(x)`: exactly one iterable argument.
fn one_items(args: &[Value]) -> VmResult<Vec<Value>> {
    let v = *args.first().ok_or(VmError::ArgumentCount {
        expected: 1,
        got: 0,
    })?;
    iter_items(v)
}

fn iter_items(v: Value) -> VmResult<Vec<Value>> {
    let len = iter_len(v);
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        out.push(iter_get(v, i as i64)?);
    }
    Ok(out)
}

fn seq_items(v: Value) -> VmResult<Vec<Value>> {
    if let Some(list) = v.as_list() {
        Ok(list.to_vec())
    } else if let Some(tuple) = v.as_tuple() {
        Ok(tuple.to_vec())
    } else {
        Err(VmError::type_mismatch("list or tuple", v))
    }
}

/// Aggregator arguments: `max(a, b, ...)` uses the varargs directly, while
/// `max([a, b, ...])` / `max((a, b))` unwraps a single sequence argument.
fn arg_items(args: &[Value]) -> VmResult<Vec<Value>> {
    match args {
        [] => Err(VmError::ArgumentCount {
            expected: 1,
            got: 0,
        }),
        [only] => seq_items(*only),
        many => Ok(many.to_vec()),
    }
}

// ── method dispatch ──────────────────────────────────────────────────────────
//
// `recv` is mutated in place for the containers (`push`/`pop`); the caller is
// responsible for writing the (possibly CoW-detached) value back to its slot.

pub fn method_call(recv: &mut Value, method: &str, args: &[Value]) -> VmResult<Value> {
    match recv.type_name() {
        "string" => {
            // `length` answers from the totals without flattening: O(1) for
            // ropes however often it is asked mid-build.
            if method == "length" || method == "len" {
                if let Some((chars, _)) = rope_lens(*recv) {
                    return Ok(Value::int(chars as i64));
                }
            }
            let s = recv.as_str().unwrap_or("");
            match method {
                "length" | "len" => Ok(Value::int(s.chars().count() as i64)),
                "upper" => Ok(Value::string(s.to_uppercase())),
                "lower" => Ok(Value::string(s.to_lowercase())),
                "trim" | "strip" => Ok(Value::string(s.trim())),
                "lstrip" => Ok(Value::string(s.trim_start())),
                "rstrip" => Ok(Value::string(s.trim_end())),
                "title" => Ok(Value::string(
                    s.split_whitespace()
                        .map(capitalize)
                        .collect::<Vec<_>>()
                        .join(" "),
                )),
                "capitalize" => Ok(Value::string(capitalize(s))),
                "contains" => {
                    let needle = string_arg(args, 0)?;
                    Ok(Value::bool(s.contains(&needle)))
                }
                "starts_with" | "startswith" => {
                    let p = string_arg(args, 0)?;
                    Ok(Value::bool(s.starts_with(&p)))
                }
                "ends_with" | "endswith" => {
                    let p = string_arg(args, 0)?;
                    Ok(Value::bool(s.ends_with(&p)))
                }
                "find" | "index" => {
                    let needle = string_arg(args, 0)?;
                    Ok(Value::int(match s.find(&needle) {
                        Some(byte) => s[..byte].chars().count() as i64,
                        None => -1,
                    }))
                }
                "count" => {
                    let needle = string_arg(args, 0)?;
                    if needle.is_empty() {
                        Ok(Value::int(s.chars().count() as i64 + 1))
                    } else {
                        Ok(Value::int(s.matches(&needle).count() as i64))
                    }
                }
                "replace" => {
                    let from = string_arg(args, 0)?;
                    let to = string_arg(args, 1)?;
                    Ok(Value::string(s.replace(&from, &to)))
                }
                "split" => {
                    let sep = args.first().copied();
                    let parts: Vec<Value> = match sep {
                        Some(v) => match v.as_str() {
                            Some(sep) if !sep.is_empty() => {
                                s.split(sep).map(Value::string).collect()
                            }
                            Some(_) => s.chars().map(|c| Value::string(c.to_string())).collect(),
                            None => {
                                return Err(VmError::type_mismatch("string", v));
                            }
                        },
                        None => s.split_whitespace().map(Value::string).collect(),
                    };
                    Ok(new_list(parts))
                }
                "zfill" => {
                    let width = *args.first().ok_or(VmError::ArgumentCount {
                        expected: 1,
                        got: 0,
                    })?;
                    let width = width
                        .as_int()
                        .ok_or_else(|| VmError::type_mismatch("int", width))?;
                    zfill(s, width).map(Value::string)
                }
                "substring" | "slice" => {
                    let chars: Vec<char> = s.chars().collect();
                    let start = int_arg(args, 0)?.max(0) as usize;
                    let end = match args.get(1) {
                        Some(_) => int_arg(args, 1)?,
                        None => chars.len() as i64,
                    };
                    let start = start.min(chars.len());
                    let end = end.max(0).min(chars.len() as i64) as usize;
                    if start >= end {
                        Ok(Value::string(String::new()))
                    } else {
                        Ok(Value::string(chars[start..end].iter().collect::<String>()))
                    }
                }
                "repeat" => {
                    let n =
                        args.first()
                            .and_then(|v| v.as_int())
                            .ok_or(VmError::ArgumentCount {
                                expected: 1,
                                got: 0,
                            })?;
                    repeat_str(s, n).map(Value::string)
                }
                "to_num" | "to_int_or_float" | "parse_num" => Ok(parse_num(s)),
                "to_int" => Ok(Value::int_promote(parse_num(s).as_int().unwrap_or(0))),
                "to_float" => Ok(Value::float(
                    parse_num(s)
                        .as_float()
                        .or_else(|| parse_num(s).as_int().map(|i| i as f64))
                        .unwrap_or(0.0),
                )),
                // Character-class predicates (Python semantics: an empty
                // string is false for all of them).
                "isalnum" => Ok(Value::bool(
                    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric()),
                )),
                "isalpha" => Ok(Value::bool(
                    !s.is_empty() && s.chars().all(|c| c.is_alphabetic()),
                )),
                "isdigit" => Ok(Value::bool(
                    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()),
                )),
                "isspace" => Ok(Value::bool(
                    !s.is_empty() && s.chars().all(|c| c.is_whitespace()),
                )),
                "isupper" => {
                    let cased: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
                    Ok(Value::bool(
                        !cased.is_empty() && cased.iter().all(|c| c.is_uppercase()),
                    ))
                }
                "islower" => {
                    let cased: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
                    Ok(Value::bool(
                        !cased.is_empty() && cased.iter().all(|c| c.is_lowercase()),
                    ))
                }
                "ord" => {
                    let c = s
                        .chars()
                        .next()
                        .ok_or(VmError::IndexError { index: 0, len: 0 })?;
                    Ok(Value::int(c as i64))
                }
                _ => Err(VmError::NativeError(format!(
                    "unknown string method {method}"
                ))),
            }
        }
        "list" => match method {
            "length" | "len" => Ok(Value::int(recv.as_list().map_or(0, |l| l.len()) as i64)),
            "push" | "append" => {
                let v = *args.first().ok_or(VmError::ArgumentCount {
                    expected: 1,
                    got: 0,
                })?;
                crate::value::cow(recv);
                recv.as_list_mut().push(v);
                Ok(Value::nil())
            }
            "pop" => {
                crate::value::cow(recv);
                recv.as_list_mut()
                    .pop()
                    .ok_or(VmError::IndexError { index: -1, len: 0 })
            }
            "contains" => {
                let needle = *args.first().ok_or(VmError::ArgumentCount {
                    expected: 1,
                    got: 0,
                })?;
                let found = recv
                    .as_list()
                    .is_some_and(|l| l.iter().any(|v| values_equal(*v, needle)));
                Ok(Value::bool(found))
            }
            "sort" => {
                let mut items = recv.as_list().map_or_else(Vec::new, |l| l.to_vec());
                items.sort_by(|a, b| compare_order(*a, *b).unwrap_or(Ordering::Equal));
                Ok(new_list(items))
            }
            "reverse" => {
                let mut items = recv.as_list().map_or_else(Vec::new, |l| l.to_vec());
                items.reverse();
                Ok(new_list(items))
            }
            "extend" => {
                let extra = *args.first().ok_or(VmError::ArgumentCount {
                    expected: 1,
                    got: 0,
                })?;
                let items = iter_items(extra)?;
                crate::value::cow(recv);
                recv.as_list_mut().extend(items);
                Ok(Value::nil())
            }
            "index" => {
                let needle = *args.first().ok_or(VmError::ArgumentCount {
                    expected: 1,
                    got: 0,
                })?;
                let pos = recv
                    .as_list()
                    .map(|l| l.iter().position(|v| values_equal(*v, needle)))
                    .unwrap_or(None);
                Ok(pos.map_or(Value::int(-1), |i| Value::int(i as i64)))
            }
            "join" => {
                let sep = args.first().map(|v| stringify(*v)).unwrap_or_default();
                let items = recv.as_list().map_or_else(Vec::new, |l| l.to_vec());
                let mut out = String::new();
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(&sep);
                    }
                    out.push_str(&stringify(*item));
                }
                Ok(Value::string(out))
            }
            _ => Err(VmError::NativeError(format!(
                "unknown list method {method}"
            ))),
        },
        "dict" => match method {
            "length" | "len" => Ok(Value::int(recv.as_dict().map_or(0, |d| d.len()) as i64)),
            "keys" => {
                let keys: Vec<Value> = recv
                    .as_dict()
                    .map(|d| d.keys().map(|k| k.0).collect())
                    .unwrap_or_default();
                Ok(new_list(keys))
            }
            "values" => {
                let vals: Vec<Value> = recv
                    .as_dict()
                    .map(|d| d.values().copied().collect())
                    .unwrap_or_default();
                Ok(new_list(vals))
            }
            "items" => {
                let pairs: Vec<Value> = recv
                    .as_dict()
                    .map(|d| d.iter().map(|(k, v)| new_tuple(vec![k.0, *v])).collect())
                    .unwrap_or_default();
                Ok(new_list(pairs))
            }
            "contains" | "has" => {
                let key = *args.first().ok_or(VmError::ArgumentCount {
                    expected: 1,
                    got: 0,
                })?;
                let found = recv.as_dict().and_then(|d| dict_get(d, key)).is_some();
                Ok(Value::bool(found))
            }
            "get" => {
                let key = *args.first().ok_or(VmError::ArgumentCount {
                    expected: 1,
                    got: 0,
                })?;
                let default = args.get(1).copied().unwrap_or_else(Value::nil);
                Ok(recv
                    .as_dict()
                    .and_then(|d| dict_get(d, key))
                    .unwrap_or(default))
            }
            _ => Err(VmError::NativeError(format!(
                "unknown dict method {method}"
            ))),
        },
        "set" => match method {
            "length" | "len" => Ok(Value::int(recv.as_set().map_or(0, |s| s.len()) as i64)),
            "contains" => {
                let needle = *args.first().ok_or(VmError::ArgumentCount {
                    expected: 1,
                    got: 0,
                })?;
                let found = recv
                    .as_set()
                    .is_some_and(|s| s.iter().any(|v| values_equal(*v, needle)));
                Ok(Value::bool(found))
            }
            "add" | "insert" => {
                let v = *args.first().ok_or(VmError::ArgumentCount {
                    expected: 1,
                    got: 0,
                })?;
                crate::value::cow(recv);
                Ok(Value::bool(recv.as_set_mut().insert(v)))
            }
            "remove" => {
                let v = *args.first().ok_or(VmError::ArgumentCount {
                    expected: 1,
                    got: 0,
                })?;
                crate::value::cow(recv);
                Ok(Value::bool(recv.as_set_mut().shift_remove(&v)))
            }
            _ => Err(VmError::NativeError(format!("unknown set method {method}"))),
        },
        other => Err(VmError::type_mismatch(
            &format!("method {method} on {other}"),
            *recv,
        )),
    }
}

// ── host module native functions (IDs 100+) ──────────────────────────────────

// std/fs
pub fn fs_read_file(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    let content = std::fs::read_to_string(&path)
        .map_err(|e| VmError::NativeError(format!("read_file: {e}")))?;
    Ok(Value::string(content))
}

pub fn fs_write_file(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    let content = string_arg(args, 1)?;
    std::fs::write(&path, content).map_err(|e| VmError::NativeError(format!("write_file: {e}")))?;
    Ok(Value::nil())
}

pub fn fs_exists(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    Ok(Value::bool(std::path::Path::new(&path).exists()))
}

pub fn fs_mkdir(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    std::fs::create_dir_all(&path).map_err(|e| VmError::NativeError(format!("mkdir: {e}")))?;
    Ok(Value::nil())
}

pub fn fs_remove(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    let p = std::path::Path::new(&path);
    let result = if p.is_dir() {
        std::fs::remove_dir_all(&path)
    } else {
        std::fs::remove_file(&path)
    };
    result.map_err(|e| VmError::NativeError(format!("remove: {e}")))?;
    Ok(Value::nil())
}

pub fn fs_mkdirs(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    std::fs::create_dir_all(&path).map_err(|e| VmError::NativeError(format!("mkdirs: {e}")))?;
    Ok(Value::nil())
}

pub fn fs_is_dir(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    Ok(Value::bool(std::path::Path::new(&path).is_dir()))
}

pub fn fs_is_file(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    Ok(Value::bool(std::path::Path::new(&path).is_file()))
}

pub fn fs_size(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    let meta = std::fs::metadata(&path)
        .map_err(|e| VmError::NativeError(format!("size: {e}")))?;
    Ok(Value::int(meta.len() as i64))
}

pub fn fs_rmdir(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    std::fs::remove_dir(&path).map_err(|e| VmError::NativeError(format!("rmdir: {e}")))?;
    Ok(Value::nil())
}

pub fn fs_rmtree(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    if std::path::Path::new(&path).exists() {
        std::fs::remove_dir_all(&path).map_err(|e| VmError::NativeError(format!("rmtree: {e}")))?;
    }
    Ok(Value::nil())
}

pub fn fs_list(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    let mut names: Vec<Value> = Vec::new();
    let entries =
        std::fs::read_dir(&path).map_err(|e| VmError::NativeError(format!("list: {e}")))?;
    for entry in entries {
        let entry = entry.map_err(|e| VmError::NativeError(format!("list: {e}")))?;
        names.push(Value::string(
            entry.file_name().to_string_lossy().into_owned(),
        ));
    }
    names.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    Ok(new_list(names))
}

pub fn fs_join(args: &[Value]) -> VmResult<Value> {
    let mut acc = std::path::PathBuf::new();
    for i in 0..args.len() {
        let part = string_arg(args, i)?;
        if !part.is_empty() {
            acc.push(&part);
        }
    }
    Ok(Value::string(acc.to_string_lossy().into_owned()))
}

pub fn fs_basename(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    let p = std::path::Path::new(&path);
    Ok(Value::string(p.file_name().map_or(String::new(), |n| {
        n.to_string_lossy().into_owned()
    })))
}

pub fn fs_dirname(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    let p = std::path::Path::new(&path);
    Ok(Value::string(p.parent().map_or(String::new(), |d| {
        d.to_string_lossy().into_owned()
    })))
}

pub fn fs_copy(args: &[Value]) -> VmResult<Value> {
    let src = string_arg(args, 0)?;
    let dst = string_arg(args, 1)?;
    std::fs::copy(&src, &dst).map_err(|e| VmError::NativeError(format!("copy: {e}")))?;
    Ok(Value::nil())
}

pub fn fs_move(args: &[Value]) -> VmResult<Value> {
    let src = string_arg(args, 0)?;
    let dst = string_arg(args, 1)?;
    std::fs::rename(&src, &dst).map_err(|e| VmError::NativeError(format!("move: {e}")))?;
    Ok(Value::nil())
}

pub fn fs_read_binary(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    let bytes =
        std::fs::read(&path).map_err(|e| VmError::NativeError(format!("read_binary: {e}")))?;
    Ok(Value::string(String::from_utf8_lossy(&bytes).into_owned()))
}

pub fn fs_write_binary(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    let content = string_arg(args, 1)?;
    std::fs::write(&path, content.as_bytes())
        .map_err(|e| VmError::NativeError(format!("write_binary: {e}")))?;
    Ok(Value::nil())
}

// std/os
pub fn os_env_get(args: &[Value]) -> VmResult<Value> {
    let key = string_arg(args, 0)?;
    match std::env::var(&key) {
        Ok(val) => Ok(Value::string(val)),
        Err(std::env::VarError::NotPresent) => Ok(Value::nil()),
        Err(e) => Err(VmError::NativeError(format!("env_get: {e}"))),
    }
}

pub fn os_env_set(args: &[Value]) -> VmResult<Value> {
    let key = string_arg(args, 0)?;
    let val = string_arg(args, 1)?;
    std::env::set_var(&key, &val);
    Ok(Value::nil())
}

pub fn os_cwd(args: &[Value]) -> VmResult<Value> {
    let _ = args; // unused
    match std::env::current_dir() {
        Ok(path) => Ok(Value::string(path.to_string_lossy())),
        Err(e) => Err(VmError::NativeError(format!("cwd: {e}"))),
    }
}

pub fn os_exit(args: &[Value], output: &mut String) -> VmResult<Value> {
    let code = int_arg(args, 0)?;
    // Flush anything buffered by the VM before terminating; `std::process::exit`
    // skips the host's normal end-of-run flush.
    if !output.is_empty() {
        print!("{output}");
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
    std::process::exit(code as i32);
}

// std/time
pub fn time_now_ms(args: &[Value]) -> VmResult<Value> {
    let _ = args;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| VmError::NativeError(format!("now_ms: {e}")))?;
    Ok(Value::int(now.as_millis() as i64))
}

pub fn time_sleep(args: &[Value]) -> VmResult<Value> {
    // Python's `time.sleep` takes seconds as a float.
    let v = args.first().copied().ok_or(VmError::ArgumentCount {
        expected: 1,
        got: 0,
    })?;
    let secs = to_f64(v).ok_or_else(|| VmError::type_mismatch("number", v))?;
    if !secs.is_finite() || secs < 0.0 {
        return Err(VmError::ValueError("sleep: negative or invalid duration".into()));
    }
    std::thread::sleep(std::time::Duration::from_secs_f64(secs));
    Ok(Value::nil())
}

/// `time.unix()`: seconds since the Unix epoch as a float (Python `time.time`).
pub fn time_unix(args: &[Value]) -> VmResult<Value> {
    let _ = args;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| VmError::NativeError(format!("unix: {e}")))?;
    Ok(Value::float(now.as_secs_f64()))
}

/// `chr(n)`: the one-character string for code point `n`.
///
/// `std/bytes.z` builds its strings through this, so it is the counterpart of
/// the string `ord` method rather than a host-module member.
pub fn chr(args: &[Value]) -> VmResult<Value> {
    let code = int_arg(args, 0)?;
    let ch = u32::try_from(code)
        .ok()
        .and_then(char::from_u32)
        .ok_or_else(|| {
            VmError::ValueError(format!("chr: {} is not a valid Unicode code point", code))
        })?;
    let mut buf = [0u8; 4];
    Ok(Value::string(ch.encode_utf8(&mut buf).to_string()))
}
