//! Host modules written in Rust and registered under their `import` names.
//!
//! A host module is an ordinary [`ModuleUnit`]: its top-level `<module>` proto
//! creates one closure per exported member and parks it in that module's own
//! global slot. When the VM finishes the module it zips `exports` with those
//! globals to build the import dictionary, so `import fs` and `from fs import
//! read_file` see the same dictionary every other module gets.
//!
//! Bare names (`fs`, `os`, …) are the keys: `compile_import` joins the dotted
//! import path verbatim, so `import fs` asks for `"fs"`. A `std.`-prefixed alias
//! is registered alongside each one so `import std.fs` keeps working.

use std::collections::HashMap;

use crate::bytecode::{FunctionProto, Instruction, ModuleUnit, Opcode};
use crate::errors::{VmError, VmResult};
use crate::value::{DictKey, Tag, Value};
use std::cell::RefCell;

pub mod crypto;
pub mod platform;
pub(crate) mod sqlite;
pub(crate) mod qr;
pub(crate) mod wa;
pub(crate) mod ffmpeg;
pub(crate) mod hydra;
pub(crate) mod std_tiers;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) mod sys;
#[cfg(target_os = "windows")]
pub(crate) mod sys_stub;
pub(crate) mod ffi;
pub(crate) mod socket;
pub(crate) mod dns;
pub(crate) mod requests;
pub(crate) mod http;
pub(crate) mod http_server;
pub(crate) mod urllib_parse;
pub(crate) mod urllib_request;
pub(crate) mod urllib;
pub(crate) mod html;
pub(crate) mod html_parser;
pub(crate) mod lxml_etree;
pub(crate) mod lxml_html;
pub(crate) mod lxml;

// ── module plumbing ────────────────────────────────────────────────────────

/// One exported member: `(member name, native id, arity)`.
type Export = (&'static str, u16, u8);

/// Child prototype for a single native function: forward the incoming `argc`
/// arguments to `id`, then return the result register the native wrote.
fn native_fn(name: &str, id: u16, argc: u8) -> FunctionProto {
    native_fn_arity(name, id, argc, argc)
}

/// A native member with a minimum arity (required) and a maximum (declared
/// parameter slots). The VM nils the omitted trailing slots, so the Rust
/// handler sees `max` arguments with `nil` in every optional position.
fn native_fn_arity(name: &str, id: u16, required: u8, argc: u8) -> FunctionProto {
    FunctionProto {
        name: name.to_string(),
        params: (0..argc).map(|i| format!("arg{i}")).collect(),
        module_id: 0,
        num_globals: 0,
        num_locals: argc as u16 + 1,
        max_reg: argc as u16 + 1,
        num_required: required as u16,
        num_upvalues: 0,
        code: vec![
            // Arguments land in registers 0..argc, so the window starts at 0
            // and the native result belongs in the next free register. The
            // native id rides in the following payload word so ids beyond 255
            // are addressable.
            Instruction::new(Opcode::CallNativeW, argc, argc, 0),
            Instruction::native_wide_payload(id),
            Instruction::new(Opcode::Return, argc, 0, 0),
        ],
        constants: Vec::new(),
        line_table: Vec::new(),
        upvalue_descriptors: Vec::new(),
        ic_slots: RefCell::new(Vec::new()),
        jit: std::sync::OnceLock::new(),
        is_method: false,
    }
}

/// Build a host module from its export table.
///
/// The main proto's register 0 is the return slot the VM overwrites with the
/// export dictionary, so closures are built from register 1 upwards.
fn host_module(exports: &[Export]) -> ModuleUnit {
    let slots = exports.len() as u16;
    let mut main = FunctionProto {
        name: "<module>".to_string(),
        params: Vec::new(),
        module_id: 0,
        num_globals: slots,
        num_locals: slots + 1,
        max_reg: slots + 1,
        num_required: 0,
        num_upvalues: 0,
        code: Vec::new(),
        constants: Vec::new(),
        line_table: Vec::new(),
        upvalue_descriptors: Vec::new(),
        ic_slots: RefCell::new(Vec::new()),
        jit: std::sync::OnceLock::new(),
        is_method: false,
    };

    let mut manifest = Vec::with_capacity(exports.len());
    for (slot, (name, _, _)) in exports.iter().enumerate() {
        let reg = (slot + 1) as u8;
        main.code
            .push(Instruction::new(Opcode::MakeClosure, reg, slot as u8, 0));
        main.code
            .push(Instruction::new(Opcode::SetGlobal, slot as u8, reg, 0));
        manifest.push(((*name).to_string(), slot as u16));
    }
    main.code.push(Instruction::new(Opcode::LoadNil, 0, 0, 0));
    main.code.push(Instruction::new(Opcode::Return, 0, 0, 0));

    let children = exports
        .iter()
        .map(|(name, id, argc)| native_fn(name, *id, *argc))
        .collect();

    ModuleUnit {
        main,
        children,
        exports: manifest,
        const_globals: Vec::new(),
    }
}

/// Like [`host_module`], but each export is `(name, id, required, max_args)`,
/// allowing optional trailing arguments (omitted slots arrive as `nil`).
fn host_module_arity(exports: &[(&'static str, u16, u8, u8)]) -> ModuleUnit {
    let slots = exports.len() as u16;
    let mut main = FunctionProto {
        name: "<module>".to_string(),
        params: Vec::new(),
        module_id: 0,
        num_globals: slots,
        num_locals: slots + 1,
        max_reg: slots + 1,
        num_required: 0,
        num_upvalues: 0,
        code: Vec::new(),
        constants: Vec::new(),
        line_table: Vec::new(),
        upvalue_descriptors: Vec::new(),
        ic_slots: RefCell::new(Vec::new()),
        jit: std::sync::OnceLock::new(),
        is_method: false,
    };

    let mut manifest = Vec::with_capacity(exports.len());
    for (slot, (name, _, _, _)) in exports.iter().enumerate() {
        let reg = (slot + 1) as u8;
        main.code
            .push(Instruction::new(Opcode::MakeClosure, reg, slot as u8, 0));
        main.code
            .push(Instruction::new(Opcode::SetGlobal, slot as u8, reg, 0));
        manifest.push(((*name).to_string(), slot as u16));
    }
    main.code.push(Instruction::new(Opcode::LoadNil, 0, 0, 0));
    main.code.push(Instruction::new(Opcode::Return, 0, 0, 0));

    let children = exports
        .iter()
        .map(|(name, id, required, argc)| native_fn_arity(name, *id, *required, *argc))
        .collect();

    ModuleUnit {
        main,
        children,
        exports: manifest,
        const_globals: Vec::new(),
    }
}

/// One member of a host module: either a native function or a constant value.
enum HostExport {
    Func {
        name: &'static str,
        id: u16,
        required: u8,
        max: u8,
    },
    Const {
        name: &'static str,
        value: Value,
    },
}

fn func(name: &'static str, id: u16, required: u8, max: u8) -> HostExport {
    HostExport::Func {
        name,
        id,
        required,
        max,
    }
}

/// Build a host module whose members may be functions or constant values.
/// Constants are emitted with `LoadConst`; functions with `MakeClosure`.
///
/// Functions are packed into the low global slots and constants into the high
/// ones. Constants are recorded in `const_globals` and pre-filled by the VM at
/// import time, so a module with many constants only reserves registers for its
/// function members.
fn host_module_mixed(exports: &[HostExport]) -> ModuleUnit {
    let funcs: Vec<&HostExport> = exports
        .iter()
        .filter(|e| matches!(e, HostExport::Func { .. }))
        .collect();
    let consts: Vec<&HostExport> = exports
        .iter()
        .filter(|e| matches!(e, HostExport::Const { .. }))
        .collect();
    let slots = (funcs.len() + consts.len()) as u16;
    let mut main = FunctionProto {
        name: "<module>".to_string(),
        params: Vec::new(),
        module_id: 0,
        num_globals: slots,
        num_locals: funcs.len() as u16 + 1,
        max_reg: funcs.len() as u16 + 1,
        num_required: 0,
        num_upvalues: 0,
        code: Vec::new(),
        constants: Vec::new(),
        line_table: Vec::new(),
        upvalue_descriptors: Vec::new(),
        ic_slots: RefCell::new(Vec::new()),
        jit: std::sync::OnceLock::new(),
        is_method: false,
    };

    let mut manifest = Vec::with_capacity(exports.len());
    let mut children = Vec::new();
    let mut const_globals = Vec::with_capacity(consts.len());
    for (slot, export) in funcs.iter().enumerate() {
        if let HostExport::Func {
            name,
            id,
            required,
            max,
        } = *export
        {
            let reg = (slot + 1) as u8;
            children.push(native_fn_arity(name, *id, *required, *max));
            main.code.push(Instruction::new(
                Opcode::MakeClosure,
                reg,
                (children.len() - 1) as u8,
                0,
            ));
            main.code
                .push(Instruction::new(Opcode::SetGlobal, slot as u8, reg, 0));
            manifest.push(((*name).to_string(), slot as u16));
        }
    }
    for (offset, export) in consts.iter().enumerate() {
        if let HostExport::Const { name, value } = *export {
            let slot = funcs.len() + offset;
            const_globals.push((slot as u16, *value));
            manifest.push(((*name).to_string(), slot as u16));
        }
    }
    main.code.push(Instruction::new(Opcode::LoadNil, 0, 0, 0));
    main.code.push(Instruction::new(Opcode::Return, 0, 0, 0));

    ModuleUnit {
        main,
        children,
        exports: manifest,
        const_globals,
    }
}

// ── std/fs (100-104) ───────────────────────────────────────────────────────

fn fs_module() -> ModuleUnit {
    host_module(&[
        ("read", 100, 1),
        ("write", 101, 2),
        ("exists", 102, 1),
        ("mkdir", 103, 1),
        ("remove", 104, 1),
        ("mkdirs", 160, 1),
        ("is_dir", 161, 1),
        ("is_file", 162, 1),
        ("rmdir", 163, 1),
        ("list", 164, 1),
        ("join", 165, 2),
        ("basename", 166, 1),
        ("dirname", 167, 1),
        ("copy", 168, 2),
        ("move", 169, 2),
        ("rmtree", 175, 1),
        ("read_binary", 176, 1),
        ("write_binary", 177, 2),
        ("size", 178, 1),
    ])
}

// ── std/os (105-108) ───────────────────────────────────────────────────────

fn os_module() -> ModuleUnit {
    host_module_mixed(&[
        HostExport::Const {
            name: "name",
            value: Value::string(std::env::consts::OS),
        },
        func("getenv", 105, 1, 1),
        func("setenv", 106, 2, 2),
        func("cwd", 107, 0, 0),
        func("exit", 108, 1, 1),
        func("execute", 245, 1, 1),
        func("cpu_count", 114, 0, 0),
    ])
}

// ── std/time (109-110) ─────────────────────────────────────────────────────

fn time_module() -> ModuleUnit {
    host_module(&[("now", 109, 0), ("sleep", 110, 1), ("unix", 113, 0)])
}

// ── std/json (111-112) ─────────────────────────────────────────────────────

/// The pure-Zen `std/json.z` is the default tier for `import json`; the native
/// tier stays reachable as `import json.native`.
fn json_module() -> ModuleUnit {
    host_module_arity(&[("parse", 111, 1, 1), ("stringify", 112, 1, 2)])
}

fn value_to_json(v: &Value) -> VmResult<serde_json::Value> {
    let unsupported = || VmError::type_mismatch("JSON-serializable", *v);
    Ok(match v.tag() {
        Tag::Nil => serde_json::Value::Null,
        Tag::False => serde_json::Value::Bool(false),
        Tag::True => serde_json::Value::Bool(true),
        Tag::Int => serde_json::Value::Number(v.as_int().unwrap_or(0).into()),
        Tag::Float => serde_json::Number::from_f64(v.as_float().unwrap_or(0.0))
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Tag::String => serde_json::Value::String(v.as_str().unwrap_or_default().to_string()),
        Tag::Bytes => serde_json::Value::String(
            String::from_utf8_lossy(v.as_bytes().unwrap_or_default()).into_owned(),
        ),
        Tag::Heap => {
            // Ropes are strings (flattened on read, cached thereafter).
            if v.is_string() {
                serde_json::Value::String(v.as_str().unwrap_or_default().to_string())
            } else if let Some(items) = v.as_list() {
                let items = items
                    .iter()
                    .map(value_to_json)
                    .collect::<VmResult<Vec<_>>>()?;
                serde_json::Value::Array(items)
            } else if let Some(map) = v.as_dict() {
                let mut obj = serde_json::Map::new();
                for (key, val) in map {
                    let key = key
                        .0
                        .as_str()
                        .ok_or_else(|| VmError::type_mismatch("string key", key.0))?;
                    obj.insert(key.to_string(), value_to_json(val)?);
                }
                serde_json::Value::Object(obj)
            } else {
                return Err(unsupported());
            }
        }
    })
}

fn json_to_value(j: &serde_json::Value) -> VmResult<Value> {
    Ok(match j {
        serde_json::Value::Null => Value::nil(),
        serde_json::Value::Bool(b) => Value::bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::int_promote(i)
            } else if let Some(f) = n.as_f64() {
                Value::float(f)
            } else {
                return Err(VmError::NativeError(format!("unrepresentable number {n}")));
            }
        }
        serde_json::Value::String(s) => Value::string(s.clone()),
        serde_json::Value::Array(items) => {
            let items = items
                .iter()
                .map(json_to_value)
                .collect::<VmResult<Vec<_>>>()?;
            crate::gc::new_list(items)
        }
        serde_json::Value::Object(fields) => {
            let mut map = indexmap::IndexMap::new();
            for (key, val) in fields {
                map.insert(DictKey(Value::string(key.clone())), json_to_value(val)?);
            }
            crate::gc::new_dict(map)
        }
    })
}

pub fn json_parse(args: &[Value]) -> VmResult<Value> {
    let text = crate::ops::string_arg(args, 0)?;
    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| VmError::ValueError(format!("json parse: {e}")))?;
    json_to_value(&parsed)
}

pub fn json_stringify(args: &[Value]) -> VmResult<Value> {
    let value = *args.first().ok_or(VmError::ArgumentCount {
        expected: 1,
        got: 0,
    })?;
    // Optional second argument: a positive number or `{pretty: true}` selects
    // the indented form. The pure tier always indents by two spaces, so the
    // native tier matches with `to_string_pretty` regardless of the value.
    let pretty = match args.get(1) {
        Some(indent) if !indent.is_nil() => {
            if let Some(map) = indent.as_dict() {
                map.get(&DictKey(Value::string("pretty")))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            } else {
                indent
                    .as_int()
                    .map(|i| i > 0)
                    .or_else(|| indent.as_float().map(|f| f > 0.0))
                    .unwrap_or(false)
            }
        }
        _ => false,
    };
    let json = value_to_json(&value)?;
    let text = if pretty {
        serde_json::to_string_pretty(&json)
    } else {
        serde_json::to_string(&json)
    }
    .map_err(|e| VmError::ValueError(format!("json stringify: {e}")))?;
    Ok(Value::string(text))
}

// ── std/ffi (120-129, 139, 148-149, 153-156) ───────────────────────────────

// Scaffolding for the channel/thread natives; registered when those modules
// land in M7. Kept here so the host-side plumbing is in one place.
#[allow(dead_code)]
fn channel_make(_args: &[Value]) -> VmResult<Value> {
    Ok(crate::gc::new_channel())
}

#[allow(dead_code)]
fn channel_send(args: &[Value]) -> VmResult<Value> {
    if args.len() < 2 {
        return Err(VmError::ArgumentCount {
            expected: 2,
            got: args.len(),
        });
    }
    let ch = args[0];
    let v = args[1];
    // Need to retain v? channel_send transfers reference conceptually; gc::channel_send takes ownership
    match crate::gc::RefCountGc::safe_channel_send(ch, v) {
        Ok(()) => Ok(Value::bool(true)),
        Err(_v) => Ok(Value::bool(false)), // or return error based on contract
    }
}

#[allow(dead_code)]
fn channel_recv(args: &[Value]) -> VmResult<Value> {
    if args.is_empty() {
        return Err(VmError::ArgumentCount {
            expected: 1,
            got: 0,
        });
    }
    crate::gc::RefCountGc::safe_channel_recv(args[0])
}

#[allow(dead_code)]
fn channel_module() -> ModuleUnit {
    host_module(&[("make", 140, 0), ("send", 141, 2), ("recv", 142, 1)])
}
#[allow(dead_code)]
fn thread_spawn(_args: &[Value]) -> VmResult<Value> {
    // Stub wrapper without VM access in this form - but dispatcher takes different shape
    Err(VmError::NativeError("thread.spawn needs vm context".into()))
}

#[allow(dead_code)]
fn thread_join(_args: &[Value]) -> VmResult<Value> {
    Err(VmError::NativeError("thread.join needs vm context".into()))
}

#[allow(dead_code)]
fn thread_current(_args: &[Value]) -> VmResult<Value> {
    Ok(Value::nil())
}

fn thread_module() -> ModuleUnit {
    host_module(&[("spawn", 150, 1), ("join", 151, 1), ("current", 152, 0)])
}
fn shared_module() -> ModuleUnit {
    host_module(&[
        ("create", 191, 1),
        ("get", 192, 1),
        ("set", 193, 2),
        ("increment", 194, 2),
    ])
}

fn net_module() -> ModuleUnit {
    host_module(&[
        ("tcp_connect", 201, 1),
        ("tcp_write", 202, 2),
        ("tcp_read", 203, 2),
        ("tcp_listen", 204, 1),
        ("tcp_accept", 205, 1),
        ("udp_bind", 206, 1),
        ("udp_send", 207, 3),
        ("udp_recv", 208, 2),
        ("close", 209, 1),
    ])
}

fn raw_module() -> ModuleUnit {
    host_module(&[
        ("raw_socket", 140, 1),
        ("raw_bind", 141, 2),
        ("raw_send", 142, 2),
        ("raw_recv", 143, 2),
        ("craft_ethernet", 144, 4),
        ("craft_ipv4", 145, 4),
        ("craft_tcp", 146, 6),
        ("raw_close", 147, 1),
    ])
}

fn process_module() -> ModuleUnit {
    host_module(&[
        ("spawn", 171, 2),
        ("read_line", 172, 1),
        ("write", 173, 2),
        ("wait", 174, 1),
    ])
}

fn ffi_module() -> ModuleUnit {
    ffi::module()
}

/// Native hashing/encoding kernel. The pure-Zen `hashlib` and `uuid` modules
/// import this; the `crypto` bare name has no `.z` counterpart.
fn crypto_module() -> ModuleUnit {
    host_module(&[
        ("hash", 210, 2),
        ("md5", 211, 1),
        ("sha1", 212, 1),
        ("sha224", 213, 1),
        ("sha256", 214, 1),
        ("sha384", 215, 1),
        ("sha512", 216, 1),
        ("sha3_256", 217, 1),
        ("sha3_512", 218, 1),
        ("blake2b", 219, 1),
        ("blake2s", 220, 1),
        ("hmac", 221, 3),
        ("pbkdf2", 222, 4),
        ("hex_encode", 223, 1),
        ("hex_decode", 224, 1),
    ])
}

// ── registry ───────────────────────────────────────────────────────────────

/// Every host module, keyed by its bare name.
///
/// The linker owns tier selection, not this table. A module with no `.z`
/// counterpart (`fs`, `socket`) is imported under this bare name; one that ships
/// both (`json`) is linked from `std/json.z` and reaches its Rust tier through
/// the `native` member the linker attaches — under the `<name>.native` key.
/// Registering a pre-suffixed key here would collide with that, so no module
/// below carries a `.native` suffix of its own.
pub fn native_modules() -> HashMap<String, ModuleUnit> {
    let table: [(&str, ModuleUnit); 31] = [
        ("fs", fs_module()),
        ("os", os_module()),
        ("time", time_module()),
        ("json", json_module()),
        ("socket", socket::module()),
        ("ffi", ffi_module()),
        ("channel", channel_module()),
        ("thread", thread_module()),
        ("shared", shared_module()),
        ("process", process_module()),
        ("net", net_module()),
        ("net.dns", dns::module()),
        ("requests", requests::module()),
        ("http", http::module()),
        ("http.server", http_server::module()),
        ("urllib", urllib::module()),
        ("urllib.parse", urllib_parse::module()),
        ("urllib.request", urllib_request::module()),
        ("html", html::module()),
        ("html.parser", html_parser::module()),
        ("lxml", lxml::module()),
        ("lxml.etree", lxml_etree::module()),
        ("lxml.html", lxml_html::module()),
        ("raw", raw_module()),
        ("crypto", crypto_module()),
        ("sqlite", sqlite::module()),
        ("qr", qr::module()),
        ("wa", wa::module()),
        ("ffmpeg", ffmpeg::module()),
        ("hydra", hydra::module()),
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        ("sys", sys::module()),
        #[cfg(target_os = "windows")]
        ("sys", sys_stub::module()),
    ];
    let mut map: HashMap<String, ModuleUnit> =
        table.into_iter().map(|(n, u)| (n.to_string(), u)).collect();
    for (name, unit) in std_tiers::native_modules() {
        map.insert(name.to_string(), unit);
    }
    map
}
