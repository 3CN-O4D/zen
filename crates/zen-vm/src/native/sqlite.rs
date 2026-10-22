//! Native `sqlite` module backed by `rusqlite`.
//!
//! Mirrors Python's `sqlite3` surface at the level the corpus exercises:
//! `open`, `openMemory`, `exec`, `query`, `close` and `escape`, with prepared
//! statements and bound parameters (so injection is impossible). Connections
//! live in a process-wide registry keyed by the integer handle returned to Zen,
//! validated on every call so a closed or forged handle raises.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use indexmap::IndexMap;
use rusqlite::types::ValueRef;
use rusqlite::{params_from_iter, Connection};
use rusqlite::types::Value as SqlValue;

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::gc::{new_dict, new_list};
use crate::native::host_module_arity;
use crate::value::{DictKey, Value};

const ID_OPEN: u16 = 300;
const ID_EXEC: u16 = 301;
const ID_QUERY: u16 = 302;
const ID_CLOSE: u16 = 303;
const ID_OPEN_MEMORY: u16 = 304;
const ID_ESCAPE: u16 = 305;

pub fn module() -> ModuleUnit {
    host_module_arity(&[
        ("open", ID_OPEN, 1, 1),
        ("exec", ID_EXEC, 2, 3),
        ("query", ID_QUERY, 2, 3),
        ("close", ID_CLOSE, 1, 1),
        ("openMemory", ID_OPEN_MEMORY, 0, 0),
        ("escape", ID_ESCAPE, 1, 1),
    ])
}

struct Registry {
    next: u32,
    open: HashMap<u32, Connection>,
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Mutex::new(Registry {
            next: 1,
            open: HashMap::new(),
        })
    })
}

fn arg(args: &[Value], i: usize) -> VmResult<Value> {
    args.get(i).copied().ok_or(VmError::ArgumentCount {
        expected: i + 1,
        got: args.len(),
    })
}

fn string_at(args: &[Value], i: usize) -> VmResult<String> {
    let v = arg(args, i)?;
    v.as_str()
        .map(str::to_string)
        .ok_or_else(|| VmError::type_mismatch("string", v))
}

fn handle_at(args: &[Value], i: usize) -> VmResult<u32> {
    let v = arg(args, i)?;
    let raw = v
        .as_int()
        .ok_or_else(|| VmError::type_mismatch("int", v))?;
    u32::try_from(raw).map_err(|_| VmError::HandleError(format!("bad sqlite handle {raw}")))
}

fn to_sql(v: Value) -> VmResult<SqlValue> {
    if v.is_nil() {
        Ok(SqlValue::Null)
    } else if let Some(b) = v.as_bool() {
        Ok(SqlValue::Integer(i64::from(b)))
    } else if let Some(i) = v.as_int() {
        Ok(SqlValue::Integer(i))
    } else if let Some(f) = v.as_float() {
        Ok(SqlValue::Real(f))
    } else if let Some(s) = v.as_str() {
        Ok(SqlValue::Text(s.to_string()))
    } else if let Some(list) = v.as_list() {
        let mut blob = Vec::with_capacity(list.len());
        for item in list.iter() {
            let b = item.as_int().ok_or_else(|| {
                VmError::type_mismatch("byte (int) in blob parameter", *item)
            })?;
            blobs_push(&mut blob, b)?;
        }
        Ok(SqlValue::Blob(blob))
    } else {
        Err(VmError::type_mismatch(
            "string, number, list or nil parameter",
            v,
        ))
    }
}

fn blobs_push(buf: &mut Vec<u8>, b: i64) -> VmResult<()> {
    if !(0..=255).contains(&b) {
        return Err(VmError::ValueError(format!("blob byte out of range: {b}")));
    }
    buf.push(b as u8);
    Ok(())
}

fn bind_params(args: &[Value], i: usize) -> VmResult<Vec<SqlValue>> {
    match args.get(i).copied() {
        None => Ok(Vec::new()),
        Some(v) if v.is_nil() => Ok(Vec::new()),
        Some(v) => match v.as_list() {
            Some(items) => items.iter().map(|x| to_sql(*x)).collect(),
            None => Err(VmError::type_mismatch("list of parameters", v)),
        },
    }
}

fn value_ref_to_value(vr: ValueRef<'_>) -> Value {
    match vr {
        ValueRef::Null => Value::nil(),
        ValueRef::Integer(i) => Value::int(i),
        ValueRef::Real(f) => Value::float(f),
        ValueRef::Text(t) => Value::string(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => new_list(b.iter().map(|byte| Value::int(*byte as i64)).collect()),
    }
}

fn with_conn<R>(
    handle: u32,
    f: impl FnOnce(&Connection) -> VmResult<R>,
) -> VmResult<R> {
    let guard = registry()
        .lock()
        .map_err(|_| VmError::NativeError("sqlite registry poisoned".into()))?;
    let conn = guard
        .open
        .get(&handle)
        .ok_or_else(|| VmError::HandleError(format!("no such sqlite handle: {handle}")))?;
    f(conn)
}

fn open_impl(path: &str, memory: bool) -> VmResult<Value> {
    let conn = if memory {
        Connection::open_in_memory()
    } else {
        Connection::open(path)
    }
    .map_err(|e| VmError::NativeError(format!("sqlite.open: {e}")))?;
    let mut guard = registry()
        .lock()
        .map_err(|_| VmError::NativeError("sqlite registry poisoned".into()))?;
    let handle = guard.next;
    guard.next += 1;
    guard.open.insert(handle, conn);
    Ok(Value::int(handle as i64))
}

fn exec_impl(args: &[Value]) -> VmResult<Value> {
    let handle = handle_at(args, 0)?;
    let sql = string_at(args, 1)?;
    let params = bind_params(args, 2)?;
    let rows = with_conn(handle, |conn| {
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| VmError::NativeError(format!("sqlite.exec prepare: {e}")))?;
        let changed = stmt
            .execute(params_from_iter(params))
            .map_err(|e| VmError::NativeError(format!("sqlite.exec: {e}")))?;
        Ok(changed as i64)
    })?;
    let mut d = IndexMap::new();
    d.insert(DictKey(Value::string("ok")), Value::bool(true));
    d.insert(DictKey(Value::string("rows")), Value::int(rows));
    Ok(new_dict(d))
}

fn query_impl(args: &[Value]) -> VmResult<Value> {
    let handle = handle_at(args, 0)?;
    let sql = string_at(args, 1)?;
    let params = bind_params(args, 2)?;
    with_conn(handle, |conn| {
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| VmError::NativeError(format!("sqlite.query prepare: {e}")))?;
        let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let mut rows = stmt
            .query(params_from_iter(params))
            .map_err(|e| VmError::NativeError(format!("sqlite.query: {e}")))?;
        let mut out = Vec::new();
        while let Some(row) = rows
            .next()
            .map_err(|e| VmError::NativeError(format!("sqlite.query row: {e}")))?
        {
            let mut d = IndexMap::new();
            for (i, name) in names.iter().enumerate() {
                let vr = row
                    .get_ref(i)
                    .map_err(|e| VmError::NativeError(format!("sqlite.query col: {e}")))?;
                d.insert(DictKey(Value::string(name.as_str())), value_ref_to_value(vr));
            }
            out.push(new_dict(d));
        }
        Ok(new_list(out))
    })
}

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Option<Value>> {
    let v = match id {
        ID_OPEN => open_impl(&string_at(args, 0)?, false)?,
        ID_OPEN_MEMORY => open_impl("", true)?,
        ID_EXEC => exec_impl(args)?,
        ID_QUERY => query_impl(args)?,
        ID_CLOSE => {
            let handle = handle_at(args, 0)?;
            let mut guard = registry()
                .lock()
                .map_err(|_| VmError::NativeError("sqlite registry poisoned".into()))?;
            Value::bool(guard.open.remove(&handle).is_some())
        }
        ID_ESCAPE => {
            let s = string_at(args, 0)?;
            Value::string(format!("'{}'", s.replace('\'', "''")))
        }
        _ => return Ok(None),
    };
    Ok(Some(v))
}