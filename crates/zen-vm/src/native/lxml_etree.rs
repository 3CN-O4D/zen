//! `lxml.etree` — a small, dependency-free element tree over the HTML/XML
//! tokenizer, covering the surface most scripts actually use:
//! `fromstring`, `tostring`, `find`, `findall`, `iter`, attributes and
//! `text_content`.
//!
//! Elements are integer handles into a process-global arena. The module-level
//! functions are the accessor surface; `lxml.html` re-exports the tolerant
//! HTML variants.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::gc::{new_dict, new_list};
use crate::native::{func, host_module_mixed};
use crate::ops::string_arg;
use crate::value::{DictKey, Value};
use indexmap::IndexMap;
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
pub(crate) struct Node {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub text: String,
    pub children: Vec<usize>,
    pub parent: Option<usize>,
}

fn arena() -> &'static Mutex<Vec<Node>> {
    static ARENA: OnceLock<Mutex<Vec<Node>>> = OnceLock::new();
    ARENA.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        func("fromstring", 900, 1, 1),
        func("tostring", 901, 1, 1),
        func("tag", 902, 1, 1),
        func("text", 903, 1, 1),
        func("attrib", 904, 1, 1),
        func("get", 905, 2, 3),
        func("children", 906, 1, 1),
        func("find", 907, 2, 2),
        func("findall", 908, 2, 2),
        func("iter", 909, 1, 2),
        func("text_content", 910, 1, 1),
        func("getparent", 911, 1, 1),
        func("set", 912, 3, 3),
    ])
}

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    match id {
        900 => Ok(Value::int_promote((fromstring(&string_arg(args, 0)?) + 1) as i64)),
        901 => tostring(args),
        902 => Ok(Value::string(node(args, 0)?.tag)),
        903 => {
            let n = arena().lock().unwrap_or_else(|e| e.into_inner());
            let node = get_node(&n, &args[0])?;
            Ok(Value::string(node.text.clone()))
        }
        904 => {
            let n = arena().lock().unwrap_or_else(|e| e.into_inner());
            let node = get_node(&n, &args[0])?;
            Ok(new_dict(attrs_value(&node.attrs)))
        }
        905 => {
            let name = string_arg(args, 1)?;
            let default = args.get(2).copied().unwrap_or_else(Value::nil);
            let n = arena().lock().unwrap_or_else(|e| e.into_inner());
            let node = get_node(&n, &args[0])?;
            Ok(node
                .attrs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| Value::string(v.clone()))
                .unwrap_or(default))
        }
        906 => {
            let n = arena().lock().unwrap_or_else(|e| e.into_inner());
            let node = get_node(&n, &args[0])?;
            Ok(list_of(&node.children))
        }
        907 => {
            let path = string_arg(args, 1)?;
            let n = arena().lock().unwrap_or_else(|e| e.into_inner());
            let root = get_handle(&args[0])?;
            let found = select(&n, root, &path);
            Ok(found
                .first()
                .map(|&i| Value::int_promote((i + 1) as i64))
                .unwrap_or_else(Value::nil))
        }
        908 => {
            let path = string_arg(args, 1)?;
            let n = arena().lock().unwrap_or_else(|e| e.into_inner());
            let root = get_handle(&args[0])?;
            Ok(list_of(&select(&n, root, &path)))
        }
        909 => {
            let want = args.get(1).and_then(|v| v.as_str().map(str::to_string));
            let n = arena().lock().unwrap_or_else(|e| e.into_inner());
            let root = get_handle(&args[0])?;
            let mut out = Vec::new();
            walk(&n, root, &mut out);
            let out: Vec<usize> = match want {
                Some(t) => out.into_iter().filter(|&i| n[i].tag == t).collect(),
                None => out,
            };
            Ok(list_of(&out))
        }
        910 => {
            let n = arena().lock().unwrap_or_else(|e| e.into_inner());
            let root = get_handle(&args[0])?;
            let mut s = String::new();
            collect_text(&n, root, &mut s);
            Ok(Value::string(s))
        }
        911 => {
            let n = arena().lock().unwrap_or_else(|e| e.into_inner());
            let node = get_node(&n, &args[0])?;
            Ok(node
                .parent
                .map(|p| Value::int_promote((p + 1) as i64))
                .unwrap_or_else(Value::nil))
        }
        912 => {
            let name = string_arg(args, 1)?;
            let value = string_arg(args, 2)?;
            let mut n = arena().lock().unwrap_or_else(|e| e.into_inner());
            let idx = get_handle(&args[0])?;
            match n[idx].attrs.iter_mut().find(|(k, _)| *k == name) {
                Some((_, v)) => *v = value,
                None => n[idx].attrs.push((name, value)),
            }
            Ok(Value::nil())
        }
        _ => Err(VmError::NativeError(format!("unknown lxml.etree native id {id}"))),
    }
}

// ── element construction ─────────────────────────────────────────────────────

pub(crate) fn fromstring(text: &str) -> usize {
    use crate::native::html_parser::Event;
    let events = crate::native::html_parser::tokenize_events(text);
    let mut arena = arena().lock().unwrap_or_else(|e| e.into_inner());
    let root = arena.len();
    arena.push(Node {
        tag: String::new(),
        attrs: Vec::new(),
        text: String::new(),
        children: Vec::new(),
        parent: None,
    });
    let mut stack = vec![root];
    for ev in events {
        match ev {
            Event::Start { tag, attrs, self_closing } => {
                let attr_pairs: Vec<(String, String)> = attrs
                    .iter()
                    .filter_map(|(k, v)| Some((k.0.as_str()?.to_string(), v.as_str()?.to_string())))
                    .collect();
                let idx = arena.len();
                let parent = *stack.last().unwrap();
                arena.push(Node {
                    tag,
                    attrs: attr_pairs,
                    text: String::new(),
                    children: Vec::new(),
                    parent: Some(parent),
                });
                arena[parent].children.push(idx);
                if !self_closing {
                    stack.push(idx);
                }
            }
            Event::End { tag } => {
                if let Some(pos) = stack.iter().rposition(|&i| arena[i].tag == tag) {
                    stack.truncate(pos);
                    if stack.is_empty() {
                        stack.push(root);
                    }
                }
            }
            Event::Data(t) => {
                let top = *stack.last().unwrap();
                arena[top].text.push_str(&t);
            }
            Event::Comment(_) | Event::Decl(_) => {}
        }
    }
    // If a single root element exists, return it; otherwise the wrapper.
    let kids = arena[root].children.clone();
    if kids.len() == 1 {
        kids[0]
    } else {
        root
    }
}

fn tostring(args: &[Value]) -> VmResult<Value> {
    let n = arena().lock().unwrap_or_else(|e| e.into_inner());
    let root = get_handle(&args[0])?;
    let mut s = String::new();
    serialize(&n, root, &mut s);
    Ok(Value::string(s))
}

fn serialize(n: &[Node], idx: usize, out: &mut String) {
    let node = &n[idx];
    if node.tag.is_empty() {
        for &c in &node.children {
            serialize(n, c, out);
        }
        return;
    }
    out.push('<');
    out.push_str(&node.tag);
    for (k, v) in &node.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(&escape_attr(v));
        out.push('"');
    }
    if node.children.is_empty() && node.text.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    out.push_str(&escape_text(&node.text));
    for &c in &node.children {
        serialize(n, c, out);
    }
    out.push_str("</");
    out.push_str(&node.tag);
    out.push('>');
}

// ── accessors ────────────────────────────────────────────────────────────────

fn get_handle(v: &Value) -> VmResult<usize> {
    let id = v.as_int().ok_or_else(|| VmError::type_mismatch("element", *v))?;
    if id <= 0 {
        return Err(VmError::HandleError(format!("bad element handle {id}")));
    }
    Ok(id as usize - 1)
}

fn node(args: &[Value], i: usize) -> VmResult<Node> {
    let a = arena().lock().unwrap_or_else(|e| e.into_inner());
    Ok(get_node(&a, &args[i])?.clone())
}

fn get_node<'a>(a: &'a [Node], v: &Value) -> VmResult<&'a Node> {
    let idx = get_handle(v)?;
    a.get(idx)
        .ok_or_else(|| VmError::HandleError(format!("no such element {}", idx + 1)))
}

fn list_of(ids: &[usize]) -> Value {
    new_list(ids.iter().map(|&i| Value::int_promote((i + 1) as i64)).collect())
}

fn attrs_value(attrs: &[(String, String)]) -> IndexMap<DictKey, Value> {
    let mut d: IndexMap<DictKey, Value> = IndexMap::new();
    for (k, v) in attrs {
        d.insert(DictKey(Value::string(k.clone())), Value::string(v.clone()));
    }
    d
}

// ── path selection (subset) ──────────────────────────────────────────────────

fn select(n: &[Node], root: usize, path: &str) -> Vec<usize> {
    let path = path.trim();
    if path.is_empty() || path == "." {
        return vec![root];
    }
    let descendant = path.starts_with("//") || path.starts_with(".//");
    let cleaned = path.trim_start_matches('.').trim_start_matches('/');
    let parts: Vec<&str> = cleaned.split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return vec![root];
    }
    let mut current = vec![root];
    for (pi, part) in parts.iter().enumerate() {
        let part = *part;
        let axis_desc = descendant && pi == 0;
        let mut next = Vec::new();
        for &cur in &current {
            let cands: Vec<usize> = if axis_desc {
                let mut all = Vec::new();
                walk(n, cur, &mut all);
                all
            } else {
                n[cur].children.clone()
            };
            for c in cands {
                if matches_part(n, c, part) && !next.contains(&c) {
                    next.push(c);
                }
            }
        }
        current = next;
    }
    current
}

fn matches_part(n: &[Node], idx: usize, part: &str) -> bool {
    let (name, pred) = match part.find('[') {
        Some(i) => {
            let end = part.rfind(']').unwrap_or(part.len());
            (&part[..i], Some(&part[i + 1..end]))
        }
        None => (part, None),
    };
    if name != "*" && !name.is_empty() && n[idx].tag != name {
        return false;
    }
    match pred {
        Some(p) => matches_pred(n, idx, p.trim()),
        None => true,
    }
}

fn matches_pred(n: &[Node], idx: usize, pred: &str) -> bool {
    if let Some(rest) = pred.strip_prefix('@') {
        if let Some(eq) = rest.find('=') {
            let key = &rest[..eq];
            let want = rest[eq + 1..].trim().trim_matches(['\'', '"']);
            return n[idx].attrs.iter().any(|(k, v)| k == key && v == want);
        }
        return n[idx].attrs.iter().any(|(k, _)| k == rest);
    }
    true
}

fn walk(n: &[Node], idx: usize, out: &mut Vec<usize>) {
    out.push(idx);
    for &c in &n[idx].children {
        walk(n, c, out);
    }
}

fn collect_text(n: &[Node], idx: usize, out: &mut String) {
    let node = &n[idx];
    if !node.tag.is_empty() {
        out.push_str(&node.text);
    }
    for &c in &node.children {
        collect_text(n, c, out);
    }
}

fn escape_text(s: &str) -> String {
    crate::native::html_parser::escape_str(s)
}

fn escape_attr(s: &str) -> String {
    crate::native::html_parser::escape_str(s)
}
