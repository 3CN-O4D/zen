//! Zen `sqlite` module — embedded SQLite backed by bundled rusqlite.
//!
//! ```python
//! import sqlite
//! h = sqlite.open("state.db")
//! sqlite.exec(h, "CREATE TABLE IF NOT EXISTS kv (k TEXT PRIMARY KEY, v TEXT)")
//! sqlite.exec(h, "INSERT INTO kv VALUES (?, ?)", ["foo", "bar"])
//! rows = sqlite.query(h, "SELECT * FROM kv ORDER BY k")
//! sqlite.close(h)
//! ```
//!
//! Handles are integers; the connection is kept alive in a process-global
//! registry until `sqlite.close`. SQL parameters are passed as a list
//! (never interpolated), mapping Zen values onto SQLite storage classes:
//! numbers, strings, nulls, booleans, and byte arrays (list of 0..255).

use crate::runtime::{Value, Vm};
use rusqlite::types::ValueRef;
use rusqlite::{params_from_iter, Connection};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const MAX_HANDLES: usize = 256;

static HANDLES: OnceLock<Mutex<HashMap<u64, Connection>>> = OnceLock::new();
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

fn handles() -> &'static Mutex<HashMap<u64, Connection>> {
    HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn handle_arg(args: &[Value]) -> Result<u64, String> {
    match args.first() {
        Some(Value::Number(n)) if *n >= 1.0 && *n <= 1e15 => Ok(*n as u64),
        _ => Err("sqlite: first argument must be the numeric handle from sqlite.open()".into()),
    }
}

fn conn_for(args: &[Value]) -> Result<u64, String> {
    let h = handle_arg(args)?;
    if !handles().lock().unwrap().contains_key(&h) {
        return Err(format!("sqlite: handle {h} is not open (was it closed?)"));
    }
    Ok(h)
}

fn params_from(value: Option<&Value>) -> Result<Vec<rusqlite::types::Value>, String> {
    let Some(v) = value else { return Ok(Vec::new()) };
    match v {
        Value::List(items) => items.iter().map(zen_to_sql).collect(),
        Value::Null => Ok(Vec::new()),
        other => Ok(vec![zen_to_sql(other)?]),
    }
}

fn zen_to_sql(v: &Value) -> Result<rusqlite::types::Value, String> {
    use rusqlite::types::Value as S;
    match v {
        Value::Null | Value::NativeFunction(_) | Value::Function(_) | Value::Cell(_) => {
            Ok(S::Null)
        }
        Value::Bool(b) => Ok(S::Integer(if *b { 1 } else { 0 })),
        Value::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 9.007199254740992e15 {
                Ok(S::Integer(*n as i64))
            } else {
                Ok(S::Real(*n))
            }
        }
        Value::String(s) => Ok(S::Text(s.clone())),
        // A list of whole numbers 0..255 is treated as a raw byte blob.
        Value::List(items) => {
            let mut bytes = Vec::with_capacity(items.len());
            for it in items.iter() {
                let Value::Number(n) = it else {
                    return Err("sqlite: blob params must be lists of byte numbers".into());
                };
                let n = *n;
                if !(0.0..=255.0).contains(&n) || n.fract() != 0.0 {
                    return Err("sqlite: blob byte out of range 0..255".into());
                }
                bytes.push(n as u8);
            }
            Ok(S::Blob(bytes))
        }
        other => Err(format!("sqlite: unsupported parameter value {other:?}")),
    }
}

fn sql_to_zen(v: ValueRef) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::Number(i as f64),
        ValueRef::Real(r) => Value::Number(r),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => Value::List(Arc::new(b.iter().map(|x| Value::Number(*x as f64)).collect())),
    }
}

/// `sqlite.open(path) -> handle`.
pub fn sqlite_open(args: &Vec<Value>) -> Result<Value, String> {
    let path = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("sqlite.open: expected a database path".into()),
    };
    if let Some(dir) = std::path::Path::new(&path).parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("sqlite.open: cannot create {dir:?}: {e}"))?;
        }
    }
    let conn = Connection::open(&path)
        .map_err(|e| format!("sqlite.open {path:?}: {e}"))?;
    let mut map = handles().lock().unwrap();
    if map.len() >= MAX_HANDLES {
        return Err(format!("sqlite: too many open handles (max {MAX_HANDLES})"));
    }
    let h = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    map.insert(h, conn);
    Ok(Value::Number(h as f64))
}

/// `sqlite.openMemory() -> handle` — an in-memory database.
pub fn sqlite_open_memory(_args: &Vec<Value>) -> Result<Value, String> {
    let conn =
        Connection::open_in_memory().map_err(|e| format!("sqlite.openMemory: {e}"))?;
    let mut map = handles().lock().unwrap();
    if map.len() >= MAX_HANDLES {
        return Err(format!("sqlite: too many open handles (max {MAX_HANDLES})"));
    }
    let h = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    map.insert(h, conn);
    Ok(Value::Number(h as f64))
}

/// `sqlite.close(handle) -> bool`.
pub fn sqlite_close(args: &Vec<Value>) -> Result<Value, String> {
    let h = handle_arg(args)?;
    let removed = handles().lock().unwrap().remove(&h).is_some();
    Ok(Value::Bool(removed))
}

/// `sqlite.exec(handle, sql, [params...]) -> {rows, lastId}`.
pub fn sqlite_exec(args: &Vec<Value>) -> Result<Value, String> {
    let h = conn_for(args)?;
    let sql = match args.get(1) {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("sqlite.exec: expected an SQL string as the 2nd argument".into()),
    };
    let params = params_from(args.get(2))?;
    let conn = handles().lock().unwrap();
    let conn = conn.get(&h).expect("conn_for guaranteed presence");
    let n = conn
        .execute(&sql, params_from_iter(params))
        .map_err(|e| format!("sqlite.exec: {e}"))?;
    let last = conn.last_insert_rowid();
    let mut r = indexmap::IndexMap::new();
    r.insert("ok".into(), Value::Bool(true));
    r.insert("rows".into(), Value::Number(n as f64));
    r.insert("lastId".into(), Value::Number(last as f64));
    Ok(Value::Dict(Arc::new(r)))
}

/// `sqlite.query(handle, sql, [params...]) -> list of row dicts`.
pub fn sqlite_query(args: &Vec<Value>) -> Result<Value, String> {
    let h = conn_for(args)?;
    let sql = match args.get(1) {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("sqlite.query: expected an SQL string as the 2nd argument".into()),
    };
    let params = params_from(args.get(2))?;
    let conn = handles().lock().unwrap();
    let conn = conn.get(&h).expect("conn_for guaranteed presence");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("sqlite.query: {e}"))?;
    let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
    let mut rows = Vec::new();
    let mut q = stmt
        .query(params_from_iter(params))
        .map_err(|e| format!("sqlite.query: {e}"))?;
    while let Some(row) = q.next().map_err(|e| format!("sqlite.query: {e}"))? {
        let mut d = indexmap::IndexMap::new();
        for (i, name) in columns.iter().enumerate() {
            let v = row.get_ref(i).map_err(|e| format!("sqlite.query: {e}"))?;
            d.insert(name.clone(), sql_to_zen(v));
        }
        rows.push(Value::Dict(Arc::new(d)));
    }
    Ok(Value::List(Arc::new(rows)))
}

/// `sqlite.escape(text) -> text` — quote a literal for embedding in SQL when
/// only string interpolation is available. Prefer bound parameters instead.
pub fn sqlite_escape(args: &Vec<Value>) -> Result<Value, String> {
    let s = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("sqlite.escape: expected a string".into()),
    };
    Ok(Value::String(format!("'{}'", s.replace('\'', "''"))))
}

pub fn init_sqlite_module(vm: &mut Vm) {
    let sqlite = Value::Dict(Arc::new(indexmap::IndexMap::from([
        ("open".into(), Value::NativeFunction("sqlite_open".into())),
        ("openMemory".into(), Value::NativeFunction("sqlite_open_memory".into())),
        ("close".into(), Value::NativeFunction("sqlite_close".into())),
        ("exec".into(), Value::NativeFunction("sqlite_exec".into())),
        ("query".into(), Value::NativeFunction("sqlite_query".into())),
        ("escape".into(), Value::NativeFunction("sqlite_escape".into())),
    ])));
    vm.vars.insert("sqlite".into(), sqlite);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_script() -> Result<(), String> {
        let dir = std::env::temp_dir().join("zen_sqlite_test");
        std::fs::create_dir_all(&dir).ok();
        let db = dir.join("t.db");
        let _ = std::fs::remove_file(&db);
        let h = match sqlite_open(&vec![Value::String(db.to_string_lossy().into_owned())])? {
            Value::Number(n) => n as u64,
            _ => return Err("bad handle".into()),
        };
        let exec = |sql: &str, params: Option<Value>| {
            let mut a = vec![Value::Number(h as f64), Value::String(sql.into())];
            if let Some(p) = params {
                a.push(p);
            }
            sqlite_exec(&a).map(|v| match v {
                Value::Dict(d) => {
                    let d = d.clone();
                    if let Some(Value::Number(n)) = d.get("rows") {
                        *n as usize
                    } else {
                        0
                    }
                }
                _ => 0,
            })
        };
        exec(
            "CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT, n INTEGER)",
            None,
        )?;
        exec(
            "INSERT INTO kv VALUES (?, ?, ?)",
            Some(Value::List(Arc::new(vec![
                Value::String("foo".into()),
                Value::String("bar".into()),
                Value::Number(42.0),
            ]))),
        )?;
        let rows = sqlite_query(&vec![
            Value::Number(h as f64),
            Value::String("SELECT k, v, n FROM kv ORDER BY k".into()),
        ])?;
        let Value::List(list) = &rows else { return Err("not a list".into()) };
        assert_eq!(list.len(), 1);
        let Value::Dict(row) = &list[0] else { return Err("not a dict".into()) };
        let row = row.clone();
        assert_eq!(row.get("k"), Some(&Value::String("foo".into())));
        assert_eq!(row.get("v"), Some(&Value::String("bar".into())));
        assert_eq!(row.get("n"), Some(&Value::Number(42.0)));
        // Blob round-trip
        exec("CREATE TABLE b (bl BLOB)", None)?;
        let blob: Vec<Value> = (0u8..=5).map(|i| Value::Number(i as f64)).collect();
        exec(
            "INSERT INTO b VALUES (?)",
            Some(Value::List(Arc::new(vec![Value::List(Arc::new(blob))]))),
        )?;
        let rows = sqlite_query(&vec![
            Value::Number(h as f64),
            Value::String("SELECT bl FROM b".into()),
        ])?;
        let Value::List(list) = rows else { return Err("not a list".into()) };
        let Value::Dict(row) = &list[0] else { return Err("not a dict".into()) };
        let Value::List(vals) = row.get("bl").unwrap() else {
            return Err("not blob".into());
        };
        assert_eq!(vals.len(), 6);
        assert_eq!(vals[5], Value::Number(5.0));
        // close then use → error
        sqlite_close(&vec![Value::Number(h as f64)])?;
        assert!(sqlite_query(&vec![
            Value::Number(h as f64),
            Value::String("SELECT 1".into())
        ])
        .is_err());
        Ok(())
    }

    #[test]
    fn sqlite_end_to_end() {
        run_script().expect("sqlite script failed");
    }

    #[test]
    fn memory_db_round_trips() {
        let h = sqlite_open_memory(&vec![]).unwrap();
        let ex = |sql: &str| sqlite_exec(&vec![h.clone(), Value::String(sql.into())]);
        ex("CREATE TABLE t (x INTEGER)").unwrap();
        ex("INSERT INTO t VALUES (7)").unwrap();
        let rows = sqlite_query(&vec![h, Value::String("SELECT x FROM t".into())]).unwrap();
        let Value::List(l) = &rows else { panic!() };
        assert_eq!(l.len(), 1);
    }
}