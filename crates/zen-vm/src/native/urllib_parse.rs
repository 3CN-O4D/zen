//! `urllib.parse` — URL parsing and quoting (Python parity).
//!
//! `quote`/`quote_plus`/`unquote`/`unquote_plus`, `urlencode`, `parse_qs`/
//! `parse_qsl`, `urlparse`/`urlsplit`/`urlunsplit`/`urljoin`/`urldefrag`.
//! All return plain values; no handles.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::gc::{new_dict, new_list};
use crate::native::{func, host_module_mixed};
use crate::ops::string_arg;
use crate::value::{DictKey, Value};
use indexmap::IndexMap;

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        func("quote", 800, 1, 2),
        func("quote_plus", 801, 1, 2),
        func("unquote", 802, 1, 1),
        func("unquote_plus", 803, 1, 1),
        func("urlencode", 804, 1, 1),
        func("parse_qs", 805, 1, 1),
        func("parse_qsl", 806, 1, 1),
        func("urlparse", 807, 1, 1),
        func("urlsplit", 808, 1, 1),
        func("urlunsplit", 809, 1, 1),
        func("urljoin", 810, 2, 2),
        func("urldefrag", 811, 1, 1),
    ])
}

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    match id {
        800 => quote(args, false),
        801 => quote(args, true),
        802 => unquote(args, false),
        803 => unquote(args, true),
        804 => urlencode(args),
        805 => parse_qs(args, true),
        806 => parse_qs(args, false),
        807 => urlparse(args),
        808 => urlsplit(args),
        809 => urlunsplit(args),
        810 => urljoin(args),
        811 => urldefrag(args),
        _ => Err(VmError::NativeError(format!(
            "unknown urllib.parse native id {id}"
        ))),
    }
}

// ── percent coding ───────────────────────────────────────────────────────────

const HEX: &[u8; 16] = b"0123456789ABCDEF";

fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~')
}

fn encode(s: &str, safe: &str, plus_space: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if is_unreserved(b) || safe.as_bytes().contains(&b) {
            out.push(b as char);
        } else if b == b' ' && plus_space {
            out.push('+');
        } else {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0xf) as usize] as char);
        }
    }
    out
}

fn decode(s: &str, plus_space: bool) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hi = (b[i + 1] as char).to_digit(16);
            let lo = (b[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        if b[i] == b'+' && plus_space {
            out.push(b' ');
        } else {
            out.push(b[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn safe_arg(args: &[Value], i: usize, default: &str) -> String {
    match args.get(i) {
        Some(v) if !v.is_nil() => v.as_str().unwrap_or("").to_string(),
        _ => default.to_string(),
    }
}

fn scalar(v: Value) -> String {
    if let Some(s) = v.as_str() {
        s.to_string()
    } else if let Some(i) = v.as_int() {
        i.to_string()
    } else if let Some(f) = v.as_float() {
        let t = format!("{f}");
        t.strip_suffix(".0").map(str::to_string).unwrap_or(t)
    } else if let Some(b) = v.as_bool() {
        b.to_string()
    } else if v.is_nil() {
        String::new()
    } else {
        String::new()
    }
}

fn quote(args: &[Value], plus_space: bool) -> VmResult<Value> {
    let s = string_arg(args, 0)?;
    let safe = safe_arg(args, 1, if plus_space { "" } else { "/" });
    Ok(Value::string(encode(&s, &safe, plus_space)))
}

fn unquote(args: &[Value], plus_space: bool) -> VmResult<Value> {
    let s = string_arg(args, 0)?;
    Ok(Value::string(decode(&s, plus_space)))
}

// ── query string ─────────────────────────────────────────────────────────────

fn urlencode(args: &[Value]) -> VmResult<Value> {
    let q = args
        .first()
        .ok_or(VmError::ArgumentCount { expected: 1, got: 0 })?;
    let mut parts: Vec<String> = Vec::new();
    if let Some(d) = q.as_dict() {
        for (k, v) in d {
            push_pair(&mut parts, &k.0, v, true);
        }
    } else if let Some(list) = q.as_list() {
        for item in list {
            if let Some(pair) = item.as_list() {
                if pair.len() == 2 {
                    push_pair(&mut parts, &pair[0], &pair[1], false);
                }
            } else if let Some(pd) = item.as_dict() {
                for (k, v) in pd {
                    push_pair(&mut parts, &k.0, v, false);
                }
            }
        }
    }
    Ok(Value::string(parts.join("&")))
}

fn push_pair(parts: &mut Vec<String>, k: &Value, v: &Value, doseq: bool) {
    if doseq {
        if let Some(list) = v.as_list() {
            for item in list {
                parts.push(format!(
                    "{}={}",
                    encode(&scalar(*k), "", true),
                    encode(&scalar(*item), "", true)
                ));
            }
            return;
        }
    }
    parts.push(format!(
        "{}={}",
        encode(&scalar(*k), "", true),
        encode(&scalar(*v), "", true)
    ));
}

fn parse_qs(args: &[Value], as_dict: bool) -> VmResult<Value> {
    let qs = string_arg(args, 0)?;
    let pairs = split_query(&qs);
    if !as_dict {
        let list = pairs
            .into_iter()
            .map(|(k, v)| new_list(vec![Value::string(k), Value::string(v)]))
            .collect();
        return Ok(new_list(list));
    }
    let mut order: Vec<(String, Vec<String>)> = Vec::new();
    for (k, v) in pairs {
        match order.iter_mut().find(|(name, _)| *name == k) {
            Some((_, vals)) => vals.push(v),
            None => order.push((k, vec![v])),
        }
    }
    let mut map: IndexMap<DictKey, Value> = IndexMap::new();
    for (k, vals) in order {
        let list = vals.into_iter().map(Value::string).collect();
        map.insert(DictKey(Value::string(k)), new_list(list));
    }
    Ok(new_dict(map))
}

fn split_query(qs: &str) -> Vec<(String, String)> {
    let qs = qs.strip_prefix('?').unwrap_or(qs);
    let mut out = Vec::new();
    for field in qs.split('&') {
        if field.is_empty() {
            continue;
        }
        let (k, v) = match field.split_once('=') {
            Some((k, v)) => (k, v),
            None => (field, ""),
        };
        out.push((decode(k, true), decode(v, true)));
    }
    out
}

// ── url parsing ──────────────────────────────────────────────────────────────

struct Split {
    scheme: String,
    netloc: String,
    path: String,
    query: String,
    fragment: String,
}

fn split_url(url: &str) -> Split {
    let mut rest = url;
    let mut scheme = String::new();
    if let Some(idx) = rest.find("://") {
        let candidate = &rest[..idx];
        if candidate
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
            && !candidate.is_empty()
        {
            scheme = candidate.to_string();
            rest = &rest[idx + 1..];
        }
    } else if let Some((candidate, tail)) = rest.split_once(':') {
        if !candidate.is_empty()
            && candidate
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
            && !tail.starts_with("//")
            && !tail.contains('/')
        {
            scheme = candidate.to_string();
            rest = tail;
        }
    }
    let (before_frag, fragment) = match rest.split_once('#') {
        Some((a, b)) => (a, b.to_string()),
        None => (rest, String::new()),
    };
    let (before_query, query) = match before_frag.split_once('?') {
        Some((a, b)) => (a, b.to_string()),
        None => (before_frag, String::new()),
    };
    let netloc = if let Some(after) = before_query.strip_prefix("//") {
        let end = after.find(['/', '?', '#']).unwrap_or(after.len());
        after[..end].to_string()
    } else {
        String::new()
    };
    let path = if netloc.is_empty() {
        before_query.to_string()
    } else {
        before_query[2 + netloc.len()..].to_string()
    };
    Split {
        scheme,
        netloc,
        path,
        query,
        fragment,
    }
}

fn make_dict(s: &Split, with_params: bool) -> Value {
    let (path, params) = if with_params {
        match s.path.rsplit_once(';') {
            Some((p, par)) => (p.to_string(), par.to_string()),
            None => (s.path.clone(), String::new()),
        }
    } else {
        (s.path.clone(), String::new())
    };
    let mut d: IndexMap<DictKey, Value> = IndexMap::new();
    d.insert(DictKey(Value::string("scheme")), Value::string(s.scheme.clone()));
    d.insert(DictKey(Value::string("netloc")), Value::string(s.netloc.clone()));
    d.insert(DictKey(Value::string("path")), Value::string(path));
    d.insert(DictKey(Value::string("params")), Value::string(params));
    d.insert(DictKey(Value::string("query")), Value::string(s.query.clone()));
    d.insert(DictKey(Value::string("fragment")), Value::string(s.fragment.clone()));
    new_dict(d)
}

fn urlparse(args: &[Value]) -> VmResult<Value> {
    let url = string_arg(args, 0)?;
    Ok(make_dict(&split_url(&url), true))
}

fn urlsplit(args: &[Value]) -> VmResult<Value> {
    let url = string_arg(args, 0)?;
    Ok(make_dict(&split_url(&url), false))
}

fn urldefrag(args: &[Value]) -> VmResult<Value> {
    let url = string_arg(args, 0)?;
    let (base, frag) = match url.split_once('#') {
        Some((b, f)) => (b.to_string(), f.to_string()),
        None => (url, String::new()),
    };
    Ok(new_list(vec![Value::string(base), Value::string(frag)]))
}

fn urlsplit_get(s: &Value, key: &str) -> String {
    s.as_dict()
        .and_then(|d| crate::ops::dict_get(d, Value::string(key)))
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn urlunsplit(args: &[Value]) -> VmResult<Value> {
    let parts = args
        .first()
        .ok_or(VmError::ArgumentCount { expected: 1, got: 0 })?;
    let (scheme, netloc, path, query, fragment) = if let Some(list) = parts.as_list() {
        (
            list.get(0).map(|v| scalar(*v)).unwrap_or_default(),
            list.get(1).map(|v| scalar(*v)).unwrap_or_default(),
            list.get(2).map(|v| scalar(*v)).unwrap_or_default(),
            list.get(3).map(|v| scalar(*v)).unwrap_or_default(),
            list.get(4).map(|v| scalar(*v)).unwrap_or_default(),
        )
    } else {
        (
            urlsplit_get(parts, "scheme"),
            urlsplit_get(parts, "netloc"),
            urlsplit_get(parts, "path"),
            urlsplit_get(parts, "query"),
            urlsplit_get(parts, "fragment"),
        )
    };
    let mut out = String::new();
    if !scheme.is_empty() {
        out.push_str(&scheme);
        out.push(':');
    }
    if !netloc.is_empty() {
        out.push_str("//");
        out.push_str(&netloc);
    }
    out.push_str(&path);
    if !query.is_empty() {
        out.push('?');
        out.push_str(&query);
    }
    if !fragment.is_empty() {
        out.push('#');
        out.push_str(&fragment);
    }
    Ok(Value::string(out))
}

fn urljoin(args: &[Value]) -> VmResult<Value> {
    let base = string_arg(args, 0)?;
    let rel = string_arg(args, 1)?;
    let b = split_url(&base);
    if rel.is_empty() {
        return Ok(Value::string(base));
    }
    if rel.contains("://") {
        return Ok(Value::string(rel));
    }
    let r = split_url(&rel);
    if rel.starts_with("//") {
        return Ok(Value::string(format!("{}://{}", b.scheme, rel.trim_start_matches('/'))));
    }
    let scheme = if r.scheme.is_empty() { b.scheme } else { r.scheme };
    let netloc = if r.netloc.is_empty() { b.netloc } else { r.netloc };
    let (path, query) = if rel.starts_with('?') {
        (b.path.clone(), r.query.clone())
    } else if rel.starts_with('#') {
        return Ok(Value::string(format!(
            "{}://{}{}?{}#{}",
            scheme, netloc, b.path, b.query, r.fragment
        )));
    } else if r.path.is_empty() {
        (b.path.clone(), if r.query.is_empty() { b.query } else { r.query })
    } else if rel.starts_with('/') {
        (r.path.clone(), r.query.clone())
    } else {
        let dir = match b.path.rfind('/') {
            Some(i) => &b.path[..=i],
            None => "/",
        };
        (normalize_path(&format!("{dir}{}", r.path)), r.query.clone())
    };
    let mut out = String::new();
    if !scheme.is_empty() {
        out.push_str(&scheme);
        out.push_str("://");
    }
    out.push_str(&netloc);
    out.push_str(&path);
    if !query.is_empty() {
        out.push('?');
        out.push_str(&query);
    }
    if !r.fragment.is_empty() {
        out.push('#');
        out.push_str(&r.fragment);
    }
    Ok(Value::string(out))
}

/// Collapse `.`/`..` segments in an absolute URL path.
fn normalize_path(path: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    let trailing = path.ends_with('/');
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }
    let mut out = String::from("/");
    out.push_str(&stack.join("/"));
    if trailing && !out.ends_with('/') {
        out.push('/');
    }
    out
}
