//! Zen `ffi` module — arbitrary C library calls.
//!
//! Loads dynamic libraries with `dlopen` and calls their functions with the
//! true C ABI via libffi, so arbitrary signatures (integer, float, pointer,
//! string, buffer, void) are supported — no Rust per library.
//!
//! Pointers are plain numbers. Pass typed scalars as `{"type": "double",
//! "value": 1.5}` and raw byte buffers as `{"buf": "hex..."}`. The return
//! type defaults to a signed 64-bit integer; use `"double"`, `"void"`,
//! `"str"`, or `{"type": "bytes", "len": n}` as needed.

use crate::runtime::Value;
use libffi::middle::{arg, Arg, Cif, CodePtr, Type};
use libloading::Library;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use std::sync::{Arc, Mutex, OnceLock};

/// dlopen'ed libraries, keyed by an integer handle handed to Zen scripts.
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

fn num_u64(v: &Value, what: &str) -> Result<u64, String> {
    match v {
        Value::Number(n) => Ok(*n as u64),
        _ => Err(format!(
            "ffi.{what}: expected a number, got {}",
            v.type_name()
        )),
    }
}

fn num_i64(v: &Value, what: &str) -> Result<i64, String> {
    match v {
        Value::Number(n) => Ok(*n as i64),
        _ => Err(format!(
            "ffi.{what}: expected a number, got {}",
            v.type_name()
        )),
    }
}

fn read_cstr(addr: u64) -> Value {
    if addr == 0 {
        return Value::String(String::new());
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
    Value::String(String::from_utf8_lossy(&out).into_owned())
}

// ---------------------------------------------------------------------------
// type tables
// ---------------------------------------------------------------------------

fn size_of(tag: &str) -> Result<u64, String> {
    let n = match tag {
        "i8" | "u8" | "char" | "bool" => 1,
        "i16" | "u16" | "short" | "ushort" => 2,
        "i32" | "u32" | "int" | "uint" | "float" => 4,
        "i64" | "u64" | "long" | "ulong" | "longlong" | "ulonglong" | "double"
        | "size_t" | "ssize_t" | "usize" | "isize" | "ptr" | "voidp" => 8,
        _ => return Err(format!("ffi.sizeof: unknown type {tag}")),
    };
    Ok(n)
}

fn align_of(tag: &str) -> Result<u64, String> {
    let a = match tag {
        "i8" | "u8" | "char" | "bool" => 1,
        "i16" | "u16" | "short" | "ushort" => 2,
        "i32" | "u32" | "int" | "uint" | "float" => 4,
        "i64" | "u64" | "long" | "ulong" | "longlong" | "ulonglong" | "double"
        | "size_t" | "ssize_t" | "usize" | "isize" | "ptr" | "voidp" => 8,
        _ => return Err(format!("ffi.alignof: unknown type {tag}")),
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

fn int_type(max_signed: bool, width: u8) -> Type {
    let signed = max_signed;
    match (signed, width) {
        (false, 8) => Type::u8(),
        (true, 8) => Type::i8(),
        (false, 16) => Type::u16(),
        (true, 16) => Type::i16(),
        (false, 32) => Type::u32(),
        (true, 32) => Type::i32(),
        _ => Type::u64(),
    }
}

fn arg_int(value: i64, tag: &str) -> Result<(Type, Bits), String> {
    let (signed, width) = match tag {
        "i8" => (true, 8),
        "u8" | "char" => (false, 8),
        "i16" => (true, 16),
        "u16" | "short" | "ushort" => (false, 16),
        "i32" | "int" => (true, 32),
        "u32" | "uint" => (false, 32),
        "i64" | "long" | "longlong" | "ssize_t" | "isize" => (true, 64),
        "u64" | "ulong" | "ulonglong" | "size_t" | "usize" | "bool" => (false, 8),
        _ => return Err(format!("ffi.call: unknown integer type {tag}")),
    };
    let ty = int_type(signed, width);
    Ok((ty, Bits::Int(value as u64)))
}

fn arg_float(value: f64) -> (Type, Bits) {
    (Type::f64(), Bits::Dbl(value))
}

fn arg_pointer(addr: u64) -> (Type, Bits) {
    (Type::pointer(), Bits::Int(addr))
}

fn parse_value_arg(
    v: &Value,
    what: &str,
    data: &mut CallData,
) -> Result<(Type, Bits), String> {
    match v {
        Value::Null => Ok(arg_pointer(0)),
        Value::Number(n) => {
            // Plain numbers default to machine-native signed integer width.
            arg_int(*n as i64, "ssize_t")
        }
        Value::String(s) => {
            let c = std::ffi::CString::new(s.clone())
                .map_err(|_| format!("ffi.{what}: string contains a NUL byte"))?;
            let addr = c.as_ptr() as u64;
            data.cstrings.push(c);
            Ok(arg_pointer(addr))
        }
        Value::Dict(d) => {
            if let Some(Value::Number(p)) = d.get("ptr") {
                return Ok(arg_pointer(*p as u64));
            }
            if let Some(Value::String(b)) = d.get("buf") {
                let bytes = crate::runtime::hex_decode(b)
                    .ok_or_else(|| format!("ffi.{what}: {{\"buf\": ...}} is not valid hex"))?;
                let addr = bytes.as_ptr() as u64;
                data.buffers.push(bytes);
                return Ok(arg_pointer(addr));
            }
            if let Some(Value::String(t)) = d.get("text") {
                let bytes = t.as_bytes().to_vec();
                let addr = bytes.as_ptr() as u64;
                data.buffers.push(bytes);
                return Ok(arg_pointer(addr));
            }
            let tag = match d.get("type") {
                Some(Value::String(t)) => t.clone(),
                _ => {
                    return Err(format!(
                        "ffi.{what}: typed value expects {{\"type\": ..., \"value\": ...}}"
                    ))
                }
            };
            let val = d
                .get("value")
                .ok_or_else(|| format!("ffi.{what}: typed value missing \"value\""))?;
            match tag.as_str() {
                "double" | "float" => {
                    let n = match val {
                        Value::Number(n) => *n,
                        _ => {
                            return Err(format!(
                                "ffi.{what}: {{\"type\": \"{tag}\", ...}} expects a number"
                            ))
                        }
                    };
                    Ok(arg_float(n))
                }
                "ptr" | "voidp" => Ok(arg_pointer(num_i64(val, what)? as u64)),
                _ => arg_int(num_i64(val, what)?, &tag),
            }
        }
        _ => Err(format!(
            "ffi.{what}: unsupported argument {}",
            v.type_name()
        )),
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

fn parse_ret(ret: &Value, what: &str) -> Result<(RetKind, Option<usize>, Type), String> {
    let mut kind = RetKind::Int;
    let mut len: Option<usize> = None;
    let mut ty = Type::u64();
    match ret {
        Value::Null => {}
        Value::String(t) => match t.as_str() {
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
            "ptr" | "voidp" | "usize" | "size_t" => {
                ty = Type::u64();
            }
            "i8" => ty = Type::i8(),
            "u8" | "char" => ty = Type::u8(),
            "i16" => ty = Type::i16(),
            "u16" => ty = Type::u16(),
            "i32" | "int" => ty = Type::i32(),
            "u32" | "uint" => ty = Type::u32(),
            "i64" | "long" | "longlong" | "ssize_t" | "isize" | "int64" => ty = Type::i64(),
            "u64" | "ulong" | "ulonglong" | "uint64" => ty = Type::u64(),
            other => {
                return Err(format!("ffi.{what}: unknown return type {other}"));
            }
        },
        Value::Dict(d) => {
            let tag = match d.get("type") {
                Some(Value::String(t)) => t.clone(),
                _ => return Err(format!("ffi.{what}: dict return needs \"type\"")),
            };
            match tag.as_str() {
                "bytes" => {
                    kind = RetKind::Bytes;
                    let n = match d.get("len") {
                        Some(Value::Number(n)) => *n as usize,
                        _ => return Err(format!("ffi.{what}: {{\"type\": \"bytes\"}} needs \"len\"")),
                    };
                    len = Some(n);
                    ty = Type::pointer();
                }
                "str" => {
                    kind = RetKind::Str;
                    ty = Type::pointer();
                }
                _ => return parse_ret(&Value::String(tag), what),
            }
        }
        _ => return Err(format!("ffi.{what}: invalid return type")),
    }
    Ok((kind, len, ty))
}

fn do_call(addr: u64, raw_args: &[Value], ret: &Value, what: &str) -> Result<Value, String> {
    if addr == 0 {
        return Err(format!("ffi.{what}: NULL function pointer"));
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
        let (ty, bits) = parse_value_arg(v, what, &mut data)?;
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
            Value::Null
        }
        RetKind::Double => {
            let r: f64 = unsafe { cif.call(code, &args) };
            Value::Number(r)
        }
        RetKind::Int => {
            let r: u64 = unsafe { cif.call(code, &args) };
            Value::Number(r as f64)
        }
        RetKind::Str => {
            let r: u64 = unsafe { cif.call(code, &args) };
            read_cstr(r)
        }
        RetKind::Bytes => {
            let r: u64 = unsafe { cif.call(code, &args) };
            if r == 0 {
                return Err(format!("ffi.{what}: returned NULL buffer"));
            }
            let n = len.unwrap_or(0);
            let slice = unsafe { std::slice::from_raw_parts(r as *const u8, n) };
            Value::String(crate::runtime::hex_encode(slice))
        }
    };
    Ok(result)
}

// ---------------------------------------------------------------------------
// natives
// ---------------------------------------------------------------------------

pub fn ffi_load(args: Vec<Value>) -> Result<Value, String> {
    let path = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("ffi.load expects a library path".into()),
    };
    let lib = unsafe { Library::new(path.as_str()) }
        .map_err(|e| format!("ffi.load {path}: {e}"))?;
    let id = NEXT_ID.fetch_add(1, SeqCst);
    libs().lock().unwrap().insert(id, SendLib(lib));
    Ok(Value::Number(id as f64))
}

pub fn ffi_close(args: Vec<Value>) -> Result<Value, String> {
    let id = num_u64(args.first().ok_or("ffi.close expects a handle")?, "close")?;
    Ok(Value::Bool(libs().lock().unwrap().remove(&id).is_some()))
}

fn with_lib<F>(id: u64, what: &str, f: F) -> Result<Value, String>
where
    F: FnOnce(&Library) -> Result<Value, String>,
{
    let guard = libs().lock().unwrap();
    let lib = guard
        .get(&id)
        .ok_or_else(|| format!("ffi.{what}: no such library handle {id}"))?;
    f(&lib.0)
}

pub fn ffi_symbol(args: Vec<Value>) -> Result<Value, String> {
    let id = num_u64(args.first().ok_or("ffi.symbol expects (handle, name)")?, "symbol")?;
    let name = match args.get(1) {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("ffi.symbol expects (handle, name)".into()),
    };
    with_lib(id, "symbol", |lib| {
        let sym: libloading::Symbol<unsafe extern "C" fn()> =
            unsafe { lib.get(name.as_bytes()) }
                .map_err(|e| format!("ffi.symbol {name}: {e}"))?;
        let addr = unsafe { sym.try_as_raw_ptr() }
            .ok_or_else(|| format!("ffi.symbol {name}: NULL symbol"))?;
        Ok(Value::Number(addr as u64 as f64))
    })
}

pub fn ffi_call(args: Vec<Value>) -> Result<Value, String> {
    let id = num_u64(args.first().ok_or("ffi.call expects (handle, name, args?, ret?)")?, "call")?;
    let name = match args.get(1) {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("ffi.call expects (handle, name, args?, ret?)".into()),
    };
    let call_args = match args.get(2) {
        Some(Value::List(l)) => l.iter().cloned().collect::<Vec<_>>(),
        None => Vec::new(),
        Some(v) => {
            return Err(format!(
                "ffi.call: args must be a list, got {}",
                v.type_name()
            ))
        }
    };
    let ret = args.get(3).cloned().unwrap_or(Value::Null);
    let addr = with_lib(id, "call", |lib| {
        let sym: libloading::Symbol<unsafe extern "C" fn()> =
            unsafe { lib.get(name.as_bytes()) }
                .map_err(|e| format!("ffi.call {name}: {e}"))?;
        let addr = unsafe { sym.try_as_raw_ptr() }
            .ok_or_else(|| format!("ffi.call {name}: NULL symbol"))?;
        Ok(Value::Number(addr as u64 as f64))
    })?;
    let addr = num_u64(&addr, "call")?;
    do_call(addr, &call_args, &ret, "call")
}

pub fn ffi_call_at(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("ffi.call_at expects (addr, args?, ret?)")?, "call_at")?;
    let call_args = match args.get(1) {
        Some(Value::List(l)) => l.iter().cloned().collect::<Vec<_>>(),
        None => Vec::new(),
        Some(v) => {
            return Err(format!(
                "ffi.call_at: args must be a list, got {}",
                v.type_name()
            ))
        }
    };
    let ret = args.get(2).cloned().unwrap_or(Value::Null);
    do_call(addr, &call_args, &ret, "call_at")
}

pub fn ffi_malloc(args: Vec<Value>) -> Result<Value, String> {
    let size = num_u64(args.first().ok_or("ffi.malloc expects a size")?, "malloc")? as usize;
    let ptr = unsafe { libc::malloc(size) };
    if ptr.is_null() {
        Err("ffi.malloc: out of memory".into())
    } else {
        Ok(Value::Number(ptr as u64 as f64))
    }
}

pub fn ffi_realloc(args: Vec<Value>) -> Result<Value, String> {
    let ptr = num_u64(args.first().ok_or("ffi.realloc expects (ptr, size)")?, "realloc")? as *mut libc::c_void;
    let size = num_u64(args.get(1).ok_or("ffi.realloc expects (ptr, size)")?, "realloc")? as usize;
    let p = unsafe { libc::realloc(ptr, size) };
    if p.is_null() {
        Err("ffi.realloc: out of memory".into())
    } else {
        Ok(Value::Number(p as u64 as f64))
    }
}

pub fn ffi_free(args: Vec<Value>) -> Result<Value, String> {
    let ptr = num_u64(args.first().ok_or("ffi.free expects a pointer")?, "free")? as *mut libc::c_void;
    unsafe { libc::free(ptr) };
    Ok(Value::Bool(true))
}

pub fn ffi_read(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("ffi.read expects (ptr, len)")?, "read")? as *const u8;
    let len = num_u64(args.get(1).ok_or("ffi.read expects (ptr, len)")?, "read")? as usize;
    let slice = unsafe { std::slice::from_raw_parts(addr, len) };
    Ok(Value::String(crate::runtime::hex_encode(slice)))
}

pub fn ffi_write(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("ffi.write expects (ptr, hex)")?, "write")? as *mut u8;
    let bytes = match args.get(1) {
        Some(Value::String(s)) => crate::runtime::hex_decode(s)
            .ok_or("ffi.write: invalid hex data")?,
        _ => return Err("ffi.write expects (ptr, hex_string)".into()),
    };
    if !bytes.is_empty() {
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), addr, bytes.len()) };
    }
    Ok(Value::Bool(true))
}

pub fn ffi_str(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("ffi.str expects a pointer")?, "str")?;
    Ok(read_cstr(addr))
}

pub fn ffi_set(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("ffi.set expects (ptr, byte, count)")?, "set")? as *mut u8;
    let byte = num_u64(args.get(1).ok_or("ffi.set expects (ptr, byte, count)")?, "set")? as u8;
    let count = num_u64(args.get(2).ok_or("ffi.set expects (ptr, byte, count)")?, "set")? as usize;
    unsafe { std::ptr::write_bytes(addr, byte, count) };
    Ok(Value::Bool(true))
}

pub fn ffi_copy(args: Vec<Value>) -> Result<Value, String> {
    let dst = num_u64(args.first().ok_or("ffi.copy expects (dst, src, len)")?, "copy")? as *mut u8;
    let src = num_u64(args.get(1).ok_or("ffi.copy expects (dst, src, len)")?, "copy")? as *const u8;
    let len = num_u64(args.get(2).ok_or("ffi.copy expects (dst, src, len)")?, "copy")? as usize;
    unsafe { std::ptr::copy_nonoverlapping(src, dst, len) };
    Ok(Value::Bool(true))
}

pub fn ffi_errno(args: Vec<Value>) -> Result<Value, String> {
    let _ = args;
    #[cfg(target_os = "linux")]
    {
        Ok(Value::Number(unsafe { *libc::__errno_location() } as f64))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(Value::Number(0.0))
    }
}

pub fn ffi_strerror(args: Vec<Value>) -> Result<Value, String> {
    let e = num_i64(args.first().ok_or("ffi.strerror expects an errno")?, "strerror")? as i32;
    let p = unsafe { libc::strerror(e) };
    if p.is_null() {
        Ok(Value::String(format!("errno {e}")))
    } else {
        Ok(Value::String(
            unsafe { std::ffi::CStr::from_ptr(p) }
                .to_string_lossy()
                .into_owned(),
        ))
    }
}

pub fn ffi_sizeof(args: Vec<Value>) -> Result<Value, String> {
    let tag = match args.first() {
        Some(Value::String(t)) => t.clone(),
        _ => return Err("ffi.sizeof expects a type name".into()),
    };
    Ok(Value::Number(size_of(&tag)? as f64))
}

pub fn ffi_alignof(args: Vec<Value>) -> Result<Value, String> {
    let tag = match args.first() {
        Some(Value::String(t)) => t.clone(),
        _ => return Err("ffi.alignof expects a type name".into()),
    };
    Ok(Value::Number(align_of(&tag)? as f64))
}

// ---------------------------------------------------------------------------
// module registration
// ---------------------------------------------------------------------------

pub fn init_ffi_module(vm: &mut crate::runtime::Vm) {
    let fns: Vec<(&str, &str)> = vec![
        ("load", "ffi_load"),
        ("close", "ffi_close"),
        ("symbol", "ffi_symbol"),
        ("call", "ffi_call"),
        ("call_at", "ffi_call_at"),
        ("malloc", "ffi_malloc"),
        ("realloc", "ffi_realloc"),
        ("free", "ffi_free"),
        ("read", "ffi_read"),
        ("write", "ffi_write"),
        ("str", "ffi_str"),
        ("set", "ffi_set"),
        ("copy", "ffi_copy"),
        ("errno", "ffi_errno"),
        ("strerror", "ffi_strerror"),
        ("sizeof", "ffi_sizeof"),
        ("alignof", "ffi_alignof"),
    ];
    let mut m = indexmap::IndexMap::new();
    for (k, n) in fns {
        m.insert(k.to_string(), Value::NativeFunction(n.to_string()));
    }
    vm.vars
        .insert("ffi".into(), Value::Dict(Arc::new(indexmap::IndexMap::from_iter(m))));
}