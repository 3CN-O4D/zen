//! `urllib.request` — `urlopen`/`urlretrieve` over the `requests` engine.
//!
//! `urlopen(url, opts?)` returns a `requests` response dict (its `.text`,
//! `.content`, `.status_code`, `.headers`, `.url` are the file-like surface).
//! `urlretrieve(url, filename?)` writes the body and returns `[path, headers]`.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::gc::{new_dict, new_list};
use crate::native::{func, host_module_mixed};
use crate::ops::dict_get;
use crate::value::{DictKey, Value};
use indexmap::IndexMap;
use std::path::PathBuf;

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        func("urlopen", 820, 1, 2),
        func("urlretrieve", 821, 1, 2),
    ])
}

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    match id {
        820 => urlopen(args),
        821 => urlretrieve(args),
        _ => Err(VmError::NativeError(format!(
            "unknown urllib.request native id {id}"
        ))),
    }
}

fn has_body(opts: Option<&IndexMap<DictKey, Value>>) -> bool {
    opts.and_then(|m| dict_get(m, Value::string("data")))
        .map(|v| !v.is_nil())
        .unwrap_or(false)
}

fn urlopen(args: &[Value]) -> VmResult<Value> {
    let opts = args.get(1).and_then(|v| v.as_dict());
    // A `data` option turns urlopen into a POST, matching Python.
    let id = if has_body(opts) { 702 } else { 701 };
    crate::native::requests::dispatch(id, args)
}

fn urlretrieve(args: &[Value]) -> VmResult<Value> {
    let resp = urlopen(args)?;
    let d = resp
        .as_dict()
        .ok_or_else(|| VmError::type_mismatch("response dict", resp))?;
    let content = dict_get(d, Value::string("content"))
        .and_then(|v| v.as_bytes().map(<[u8]>::to_vec))
        .unwrap_or_default();
    let headers = dict_get(d, Value::string("headers")).unwrap_or_else(|| new_dict(IndexMap::new()));
    let url = dict_get(d, Value::string("url"))
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();

    let path = match args.get(1).and_then(|v| v.as_str()) {
        Some(name) if !name.is_empty() => PathBuf::from(name),
        _ => default_path(&url),
    };
    std::fs::write(&path, &content)
        .map_err(|e| VmError::NativeError(format!("urlretrieve: write {}: {e}", path.display())))?;
    Ok(new_list(vec![
        Value::string(path.to_string_lossy().to_string()),
        headers,
    ]))
}

fn default_path(url: &str) -> PathBuf {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let path = match after_scheme.find('/') {
        Some(i) => &after_scheme[i + 1..],
        None => "",
    };
    let base = path.split(['?', '#']).next().unwrap_or("");
    let name = base.rsplit('/').next().unwrap_or("");
    let name = if name.is_empty() { "download" } else { name };
    std::env::temp_dir().join(format!("zen-urlretrieve-{}", name))
}
