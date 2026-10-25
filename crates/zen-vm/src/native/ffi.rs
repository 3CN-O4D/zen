//! Zen `ffi` module — arbitrary C library calls.
//!
//! Loads dynamic libraries with `dlopen` and calls their functions with the
//! true C ABI via libffi, so arbitrary signatures (integer, float, pointer,
//! string, buffer, void) are supported.
//!
//! Pointers are plain numbers. Pass typed scalars as `{"type": "double",
//! "value": 1.5}` and raw byte buffers as `{"buf": "hex..."}`. The return
//! type defaults to a signed 64-bit integer; use `"double"`, `"void"`,
//! `"str"`, or `{"type": "bytes", "len": n}` as needed.

#![allow(unsafe_code)]

use libffi::middle::{arg, Arg, Cif, CodePtr, Type};
use libloading::Library;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use std::sync::{Mutex, OnceLock};

use crate::errors::{VmError, VmResult};
use crate::ops::string_arg;
use crate::value::{DictKey, Value};

// ── dlopen registry ─────────────────────────────────────────────────────────

/// `Library` is neither Send nor Sync in safe Rust, but raw C libraries are
/// usable from any thread, which is what matters here.
struct SendLib(Library);
unsafe impl Send for SendLib {}
unsafe impl Sync for SendLib {}

static LIBRARIES: OnceLock<Mutex<HashMap<u64, SendLib>>> = OnceLock::new();
fn libs() -> &'static Mutex<HashMap<u64, SendLib>> {
    LIBRARIES.get_or_init(|| Mutex::new(HashMap::new()))
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

// ── FFI capability gates ────────────────────────────────────────────────────

/// Allowed library path prefixes for `ffi.load`. Empty = allow all.
static ALLOW_LIST: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
/// Denied library path prefixes for `ffi.load`. Empty = deny none.
static DENY_LIST: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

fn allow_list() -> &'static Mutex<Vec<String>> {
    ALLOW_LIST.get_or_init(|| Mutex::new(Vec::new()))
}

fn deny_list() -> &'static Mutex<Vec<String>> {
    DENY_LIST.get_or_init(|| Mutex::new(Vec::new()))
}

/// Check whether a library path is permitted by the allow/deny lists.
fn path_allowed(path: &str) -> bool {
    let deny = deny_list().lock().unwrap_or_else(|e| e.into_inner());
    for prefix in deny.iter() {
        if path.starts_with(prefix) {
            return false;
        }
    }
    drop(deny);

    let allow = allow_list().lock().unwrap_or_else(|e| e.into_inner());
    if allow.is_empty() {
        return true; // no allow list = allow all (subject to deny list)
    }
    allow.iter().any(|prefix| path.starts_with(prefix))
}

// ── numeric / C-string helpers ──────────────────────────────────────────────

fn num_u64(v: Value) -> Option<u64> {
    if let Some(i) = v.as_int() {
        Some(i as u64)
    } else if let Some(f) = v.as_float() {
        Some(f as u64)
    } else {
        None
    }
}

fn num_i64(v: Value) -> Option<i64> {
    if let Some(i) = v.as_int() {
        Some(i)
    } else if let Some(f) = v.as_float() {
        Some(f as i64)
    } else {
        None
    }
}

fn num_u64_at(args: &[Value], i: usize, what: &str) -> VmResult<u64> {
    let v = *args.get(i).ok_or(VmError::ArgumentCount {
        expected: i + 1,
        got: args.len(),
    })?;
    num_u64(v).ok_or_else(|| VmError::type_mismatch(&format!("ffi.{what} number"), v))
}

/// Owned-string lookup for a dict entry, so the borrowed `&str` never outlives
/// the temporary `Value` a `dict_get` returns.
fn dict_str(d: &indexmap::IndexMap<DictKey, Value>, key: &str) -> Option<String> {
    crate::ops::dict_get(d, Value::string(key)).and_then(|v| v.as_str().map(|s| s.to_string()))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len() / 2);
    let mut i = 0;
    while i < b.len() {
        let hi = (b[i] as char).to_digit(16)?;
        let lo = (b[i + 1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
        i += 2;
    }
    Some(out)
}

fn read_cstr(addr: u64) -> Value {
    if addr == 0 {
        return Value::string("");
    }
    let mut out = Vec::new();
    unsafe {
        let mut p = addr as *const u8;
        loop {
            let b = *p;
            if b == 0 {
                break;
            }
            out.push(b);
            p = p.add(1);
        }
    }
    Value::string(String::from_utf8_lossy(&out).into_owned())
}

// ── type tables ─────────────────────────────────────────────────────────────

fn size_of(tag: &str) -> VmResult<u64> {
    let n = match tag {
        "i8" | "u8" | "char" | "bool" => 1,
        "i16" | "u16" | "short" | "ushort" => 2,
        "i32" | "u32" | "int" | "uint" | "float" => 4,
        "i64" | "u64" | "long" | "ulong" | "longlong" | "ulonglong" | "double"
        | "size_t" | "ssize_t" | "usize" | "isize" | "ptr" | "voidp" => 8,
        other => return Err(VmError::ValueError(format!("ffi.sizeof: unknown type {other}"))),
    };
    Ok(n)
}

fn align_of(tag: &str) -> VmResult<u64> {
    let a = match tag {
        "i8" | "u8" | "char" | "bool" => 1,
        "i16" | "u16" | "short" | "ushort" => 2,
        "i32" | "u32" | "int" | "uint" | "float" => 4,
        "i64" | "u64" | "long" | "ulong" | "longlong" | "ulonglong" | "double"
        | "size_t" | "ssize_t" | "usize" | "isize" | "ptr" | "voidp" => 8,
        other => return Err(VmError::ValueError(format!("ffi.alignof: unknown type {other}"))),
    };
    Ok(a)
}

/// The actual C value for a single FFI argument, to be stored in its own
/// heap slot so the address stays stable for the duration of the call.
enum Bits {
    Int(u64),
    Dbl(f64),
}

/// Keep buffers and C strings alive for the duration of a call.
struct CallData {
    cstrings: Vec<std::ffi::CString>,
    buffers: Vec<Vec<u8>>,
}

fn int_type(signed: bool, width: u8) -> Type {
    match (signed, width) {
        (false, 8) => Type::u8(),
        (true, 8) => Type::i8(),
        (false, 16) => Type::u16(),
        (true, 16) => Type::i16(),
        (false, 32) => Type::u32(),
        (true, 32) => Type::i32(),
        (false, 64) => Type::u64(),
        (true, 64) => Type::i64(),
        _ => Type::usize(),
    }
}

fn arg_int(value: i64, tag: &str) -> VmResult<(Type, Bits)> {
    let (signed, width) = match tag {
        "i8" => (true, 8),
        "u8" | "char" => (false, 8),
        "i16" => (true, 16),
        "u16" | "short" | "ushort" => (false, 16),
        "i32" | "int" => (true, 32),
        "u32" | "uint" => (false, 32),
        "i64" | "long" | "longlong" | "ssize_t" | "isize" => (true, 64),
        "u64" | "ulong" | "ulonglong" | "size_t" | "usize" => (false, 64),
        "bool" => (false, 8),
        other => return Err(VmError::ValueError(format!("ffi.call: unknown integer type {other}"))),
    };
    Ok((int_type(signed, width), Bits::Int(value as u64)))
}

fn arg_float(value: f64) -> (Type, Bits) {
    (Type::f64(), Bits::Dbl(value))
}

fn arg_pointer(addr: u64) -> (Type, Bits) {
    (Type::pointer(), Bits::Int(addr))
}

fn parse_value_arg(v: Value, what: &str, data: &mut CallData) -> VmResult<(Type, Bits)> {
    if v.is_nil() {
        return Ok(arg_pointer(0));
    }
    if let Some(s) = v.as_str() {
        let c = std::ffi::CString::new(s.to_string())
            .map_err(|_| VmError::ValueError(format!("ffi.{what}: string contains a NUL byte")))?;
        let addr = c.as_ptr() as u64;
        data.cstrings.push(c);
        return Ok(arg_pointer(addr));
    }
    if let Some(d) = v.as_dict() {
        if let Some(p) = crate::ops::dict_get(d, Value::string("ptr")).and_then(num_u64) {
            return Ok(arg_pointer(p));
        }
        if let Some(b) = dict_str(d, "buf") {
            let bytes = hex_decode(&b).ok_or_else(|| {
                VmError::ValueError(format!("ffi.{what}: {{\"buf\": ...}} is not valid hex"))
            })?;
            let addr = bytes.as_ptr() as u64;
            data.buffers.push(bytes);
            return Ok(arg_pointer(addr));
        }
        if let Some(t) = dict_str(d, "text") {
            let bytes = t.as_bytes().to_vec();
            let addr = bytes.as_ptr() as u64;
            data.buffers.push(bytes);
            return Ok(arg_pointer(addr));
        }
        if let Some(tag) = dict_str(d, "type") {
            let val = crate::ops::dict_get(d, Value::string("value")).ok_or_else(|| {
                VmError::ValueError(format!("ffi.{what}: typed value missing \"value\""))
            })?;
            match tag.as_str() {
                "double" | "float" => {
                    let n = val.as_float().or_else(|| {
                        val.as_int().map(|i| i as f64)
                    }).ok_or_else(|| {
                        VmError::type_mismatch("number", val)
                    })?;
                    Ok(arg_float(n))
                }
                "ptr" | "voidp" => Ok(arg_pointer(
                    num_u64(val).ok_or_else(|| VmError::type_mismatch("number", val))?,
                )),
                other => {
                    let n = num_i64(val).ok_or_else(|| VmError::type_mismatch("number", val))?;
                    arg_int(n, other)
                }
            }
        } else {
            Err(VmError::ValueError(format!(
                "ffi.{what}: typed value expects {{\"type\": ..., \"value\": ...}}"
            )))
        }
    } else if let Some(n) = num_i64(v) {
        // Plain numbers default to machine-native signed integer width.
        arg_int(n, "ssize_t")
    } else {
        Err(VmError::type_mismatch("ffi argument", v))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum RetKind {
    Void,
    Int,
    Double,
    Str,
    Bytes,
}

fn parse_ret(ret: &Value, what: &str) -> VmResult<(RetKind, Option<usize>, Type)> {
    let mut kind = RetKind::Int;
    let mut len: Option<usize> = None;
    let mut ty = Type::u64();
    if let Some(t) = ret.as_str() {
        match t {
            "void" | "null" => {
                kind = RetKind::Void;
                ty = Type::void();
            }
            "double" | "float" => {
                kind = RetKind::Double;
                ty = Type::f64();
            }
            "str" | "charp" | "string" => {
                kind = RetKind::Str;
                ty = Type::pointer();
            }
            "ptr" | "voidp" | "usize" | "size_t" => {}
            "i8" => ty = Type::i8(),
            "u8" | "char" => ty = Type::u8(),
            "i16" => ty = Type::i16(),
            "u16" => ty = Type::u16(),
            "i32" | "int" => ty = Type::i32(),
            "u32" | "uint" => ty = Type::u32(),
            "i64" | "long" | "longlong" | "ssize_t" | "isize" | "int64" => ty = Type::i64(),
            "u64" | "ulong" | "ulonglong" | "uint64" => ty = Type::u64(),
            other => return Err(VmError::ValueError(format!("ffi.{what}: unknown return type {other}"))),
        }
    } else if let Some(d) = ret.as_dict() {
        let tag = dict_str(d, "type").ok_or_else(|| {
            VmError::ValueError(format!("ffi.{what}: dict return needs \"type\""))
        })?;
        match tag.as_str() {
            "bytes" | "buf" => {
                kind = RetKind::Bytes;
                let n = crate::ops::dict_get(d, Value::string("len"))
                    .and_then(num_u64)
                    .ok_or_else(|| {
                        VmError::ValueError(format!("ffi.{what}: {{\"type\": \"bytes\"}} needs \"len\""))
                    })?;
                len = Some(n as usize);
                ty = Type::pointer();
            }
            "str" => {
                kind = RetKind::Str;
                ty = Type::pointer();
            }
            "void" | "null" => {
                kind = RetKind::Void;
                ty = Type::void();
            }
            "double" | "float" => {
                kind = RetKind::Double;
                ty = Type::f64();
            }
            "ptr" | "voidp" | "usize" | "size_t" | "uint64" | "u64" => {
                ty = Type::u64();
            }
            "i64" | "long" | "longlong" | "ssize_t" | "isize" | "int64" => {
                ty = Type::i64();
            }
            "i32" | "int" => ty = Type::i32(),
            "u32" | "uint" => ty = Type::u32(),
            "i8" => ty = Type::i8(),
            "u8" | "char" => ty = Type::u8(),
            "i16" => ty = Type::i16(),
            "u16" => ty = Type::u16(),
            other => {
                return Err(VmError::ValueError(format!(
                    "ffi.{what}: unknown return type {other}"
                )))
            }
        }
    } else if !ret.is_nil() {
        return Err(VmError::ValueError(format!("ffi.{what}: invalid return type")));
    }
    Ok((kind, len, ty))
}

fn do_call(addr: u64, raw_args: &[Value], ret: &Value, what: &str) -> VmResult<Value> {
    if addr == 0 {
        return Err(VmError::ValueError(format!("ffi.{what}: NULL function pointer")));
    }
    let mut data = CallData {
        cstrings: Vec::new(),
        buffers: Vec::new(),
    };
    let mut types: Vec<Type> = Vec::with_capacity(raw_args.len());
    let mut ints: Vec<Box<u64>> = Vec::with_capacity(raw_args.len());
    let mut dbls: Vec<Box<f64>> = Vec::with_capacity(raw_args.len());
    let mut args: Vec<Arg> = Vec::with_capacity(raw_args.len());
    for v in raw_args {
        let (ty, bits) = parse_value_arg(*v, what, &mut data)?;
        types.push(ty);
        match bits {
            Bits::Int(n) => {
                let b = Box::new(n);
                args.push(arg(&*b));
                ints.push(b);
            }
            Bits::Dbl(n) => {
                let b = Box::new(n);
                args.push(arg(&*b));
                dbls.push(b);
            }
        }
    }
    let (kind, len, ret_ty) = parse_ret(ret, what)?;
    let cif = Cif::new(types, ret_ty);
    let code = CodePtr(addr as *mut libc::c_void);
    let result = match kind {
        RetKind::Void => {
            unsafe { cif.call::<()>(code, &args) };
            Value::nil()
        }
        RetKind::Double => {
            let r: f64 = unsafe { cif.call(code, &args) };
            Value::float(r)
        }
        RetKind::Int => {
            let r: u64 = unsafe { cif.call(code, &args) };
            Value::int_promote(r as i64)
        }
        RetKind::Str => {
            let r: u64 = unsafe { cif.call(code, &args) };
            read_cstr(r)
        }
        RetKind::Bytes => {
            let r: u64 = unsafe { cif.call(code, &args) };
            if r == 0 {
                return Err(VmError::NativeError(format!("ffi.{what}: returned NULL buffer")));
            }
            let n = len.unwrap_or(0);
            let slice = unsafe { std::slice::from_raw_parts(r as *const u8, n) };
            Value::string(hex_encode(slice))
        }
    };
    Ok(result)
}

// ── natives ─────────────────────────────────────────────────────────────────

pub fn ffi_load(args: &[Value]) -> VmResult<Value> {
    let path = string_arg(args, 0)?;
    if !path_allowed(&path) {
        return Err(VmError::NativeError(format!(
            "ffi.load {path}: blocked by FFI capability policy"
        )));
    }
    let lib = unsafe { Library::new(path.as_str()) }
        .map_err(|e| VmError::NativeError(format!("ffi.load {path}: {e}")))?;
    let id = NEXT_ID.fetch_add(1, SeqCst);
    libs().lock().unwrap_or_else(|e| e.into_inner()).insert(id, SendLib(lib));
    Ok(Value::int_promote(id as i64))
}

pub fn ffi_close(args: &[Value]) -> VmResult<Value> {
    let id = num_u64_at(args, 0, "close")?;
    Ok(Value::bool(
        libs().lock().unwrap_or_else(|e| e.into_inner()).remove(&id).is_some(),
    ))
}

fn with_lib<F>(id: u64, what: &str, f: F) -> VmResult<Value>
where
    F: FnOnce(&Library) -> VmResult<Value>,
{
    let guard = libs().lock().unwrap_or_else(|e| e.into_inner());
    let lib = guard
        .get(&id)
        .ok_or_else(|| VmError::HandleError(format!("ffi.{what}: no such library handle {id}")))?;
    f(&lib.0)
}

fn symbol_addr(lib: &Library, name: &str, what: &str) -> VmResult<u64> {
    let sym: libloading::Symbol<unsafe extern "C" fn()> =
        unsafe { lib.get(name.as_bytes()) }
            .map_err(|e| VmError::NativeError(format!("ffi.{what} {name}: {e}")))?;
    let addr = unsafe { sym.try_as_raw_ptr() }
        .ok_or_else(|| VmError::NativeError(format!("ffi.{what} {name}: NULL symbol")))?;
    Ok(addr as u64)
}

pub fn ffi_symbol(args: &[Value]) -> VmResult<Value> {
    let id = num_u64_at(args, 0, "symbol")?;
    let name = string_arg(args, 1)?;
    with_lib(id, "symbol", |lib| {
        Ok(Value::int_promote(symbol_addr(lib, &name, "symbol")? as i64))
    })
}

pub fn ffi_call(args: &[Value]) -> VmResult<Value> {
    let id = num_u64_at(args, 0, "call")?;
    let name = string_arg(args, 1)?;
    let call_args = match args.get(2) {
        None => Vec::new(),
        Some(v) => v.as_list().map(|l| l.to_vec()).ok_or_else(|| {
            VmError::type_mismatch("list", *v)
        })?,
    };
    let ret = args.get(3).copied().unwrap_or(Value::nil());
    let addr = with_lib(id, "call", |lib| {
        Ok(Value::int_promote(symbol_addr(lib, &name, "call")? as i64))
    })?;
    let addr = num_u64(addr).ok_or_else(|| VmError::type_mismatch("address", addr))?;
    do_call(addr, &call_args, &ret, "call")
}

pub fn ffi_call_at(args: &[Value]) -> VmResult<Value> {
    let addr = num_u64_at(args, 0, "call_at")?;
    let call_args = match args.get(1) {
        None => Vec::new(),
        Some(v) => v.as_list().map(|l| l.to_vec()).ok_or_else(|| {
            VmError::type_mismatch("list", *v)
        })?,
    };
    let ret = args.get(2).copied().unwrap_or(Value::nil());
    do_call(addr, &call_args, &ret, "call_at")
}

pub fn ffi_malloc(args: &[Value]) -> VmResult<Value> {
    let size = num_u64_at(args, 0, "malloc")? as usize;
    let ptr = unsafe { libc::malloc(size) };
    if ptr.is_null() {
        Err(VmError::NativeError("ffi.malloc: out of memory".into()))
    } else {
        Ok(Value::int_promote(ptr as u64 as i64))
    }
}

pub fn ffi_realloc(args: &[Value]) -> VmResult<Value> {
    let ptr = num_u64_at(args, 0, "realloc")? as *const u8;
    let size = num_u64_at(args, 1, "realloc")? as usize;
    let newp = unsafe { libc::malloc(size) };
    if newp.is_null() {
        return Err(VmError::NativeError("ffi.realloc: out of memory".into()));
    }
    // Grow/copy into the fresh block but keep the original allocation alive:
    // a caller that kept the old pointer around can keep using and freeing it.
    // (libc::realloc would reclaim the head; the corpus reuses the old
    // pointer after calling realloc, so reclaiming it would be use-after-free.)
    if !ptr.is_null() {
        let old_usable = unsafe { libc::malloc_usable_size(ptr as *mut libc::c_void) };
        let n = old_usable.min(size);
        if n > 0 {
            unsafe { std::ptr::copy_nonoverlapping(ptr, newp as *mut u8, n) };
        }
    }
    Ok(Value::int_promote(newp as u64 as i64))
}

pub fn ffi_free(args: &[Value]) -> VmResult<Value> {
    let ptr = num_u64_at(args, 0, "free")? as *mut libc::c_void;
    unsafe { libc::free(ptr) };
    Ok(Value::bool(true))
}

pub fn ffi_read(args: &[Value]) -> VmResult<Value> {
    let addr = num_u64_at(args, 0, "read")? as *const u8;
    let len = num_u64_at(args, 1, "read")? as usize;
    let slice = unsafe { std::slice::from_raw_parts(addr, len) };
    Ok(Value::string(hex_encode(slice)))
}

pub fn ffi_write(args: &[Value]) -> VmResult<Value> {
    let addr = num_u64_at(args, 0, "write")? as *mut u8;
    let s = string_arg(args, 1)?;
    let bytes = hex_decode(&s).ok_or_else(|| {
        VmError::ValueError("ffi.write: invalid hex data".to_string())
    })?;
    if !bytes.is_empty() {
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), addr, bytes.len()) };
    }
    Ok(Value::bool(true))
}

pub fn ffi_str(args: &[Value]) -> VmResult<Value> {
    let addr = num_u64_at(args, 0, "str")?;
    Ok(read_cstr(addr))
}

pub fn ffi_set(args: &[Value]) -> VmResult<Value> {
    let addr = num_u64_at(args, 0, "set")? as *mut u8;
    let byte = num_u64_at(args, 1, "set")? as u8;
    let count = num_u64_at(args, 2, "set")? as usize;
    unsafe { std::ptr::write_bytes(addr, byte, count) };
    Ok(Value::bool(true))
}

pub fn ffi_copy(args: &[Value]) -> VmResult<Value> {
    let dst = num_u64_at(args, 0, "copy")? as *mut u8;
    let src = num_u64_at(args, 1, "copy")? as *const u8;
    let len = num_u64_at(args, 2, "copy")? as usize;
    unsafe { std::ptr::copy_nonoverlapping(src, dst, len) };
    Ok(Value::bool(true))
}

pub fn ffi_errno(args: &[Value]) -> VmResult<Value> {
    let _ = args;
    Ok(Value::int_promote(
        std::io::Error::last_os_error().raw_os_error().unwrap_or(0) as i64,
    ))
}

pub fn ffi_strerror(args: &[Value]) -> VmResult<Value> {
    let e = num_u64_at(args, 0, "strerror")? as i32;
    let p = unsafe { libc::strerror(e) };
    if p.is_null() {
        Ok(Value::string(format!("errno {e}")))
    } else {
        Ok(Value::string(
            unsafe { std::ffi::CStr::from_ptr(p) }
                .to_string_lossy()
                .into_owned(),
        ))
    }
}

pub fn ffi_sizeof(args: &[Value]) -> VmResult<Value> {
    let tag = string_arg(args, 0)?;
    Ok(Value::int_promote(size_of(&tag)? as i64))
}

pub fn ffi_alignof(args: &[Value]) -> VmResult<Value> {
    let tag = string_arg(args, 0)?;
    Ok(Value::int_promote(align_of(&tag)? as i64))
}

/// `ffi.allow(paths) -> nil`
///
/// Add library path prefixes to the FFI allow list. Once the allow list is
/// non-empty, `ffi.load` only permits libraries whose path starts with one of
/// the listed prefixes. Use this to restrict FFI to a known set of trusted
/// libraries.
pub fn ffi_allow(args: &[Value]) -> VmResult<Value> {
    let paths = args.get(0)
        .and_then(|v| v.as_list())
        .ok_or_else(|| VmError::TypeError("allow expects a list of path prefixes".into()))?;
    let mut list = allow_list().lock().unwrap_or_else(|e| e.into_inner());
    for p in paths {
        if let Some(s) = p.as_str() {
            list.push(s.to_string());
        }
    }
    Ok(Value::nil())
}

/// `ffi.deny(paths) -> nil`
///
/// Add library path prefixes to the FFI deny list. `ffi.load` rejects any
/// library whose path starts with one of the listed prefixes, even if the
/// allow list is empty.
pub fn ffi_deny(args: &[Value]) -> VmResult<Value> {
    let paths = args.get(0)
        .and_then(|v| v.as_list())
        .ok_or_else(|| VmError::TypeError("deny expects a list of path prefixes".into()))?;
    let mut list = deny_list().lock().unwrap_or_else(|e| e.into_inner());
    for p in paths {
        if let Some(s) = p.as_str() {
            list.push(s.to_string());
        }
    }
    Ok(Value::nil())
}

// ── module registration ─────────────────────────────────────────────────────

fn func(name: &'static str, id: u16, required: u8, max: u8) -> crate::native::HostExport {
    crate::native::HostExport::Func {
        name,
        id,
        required,
        max,
    }
}

pub fn module() -> crate::bytecode::ModuleUnit {
    crate::native::host_module_mixed(&[
        func("load", 120, 1, 1),
        func("symbol", 121, 2, 2),
        func("call", 122, 2, 4),
        func("close", 123, 1, 1),
        func("call_at", 124, 1, 3),
        func("malloc", 125, 1, 1),
        func("realloc", 126, 2, 2),
        func("free", 127, 1, 1),
        func("read", 128, 2, 2),
        func("write", 129, 2, 2),
        func("str", 139, 1, 1),
        func("set", 148, 3, 3),
        func("copy", 149, 3, 3),
        func("errno", 153, 0, 0),
        func("strerror", 154, 1, 1),
        func("sizeof", 155, 1, 1),
        func("alignof", 156, 1, 1),
        func("allow", 190, 1, 1),
        func("deny", 191, 1, 1),
    ])
}