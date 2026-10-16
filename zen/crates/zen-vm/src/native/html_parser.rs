//! `html.parser` — a tolerant HTML tokenizer and tag extractor.
//!
//! Python's `HTMLParser` is callback-based and meant to be subclassed. Zen
//! exposes the same information as data instead: `parse(text)` returns a list
//! of start-tag dicts, `events(text)` the full token stream, and the
//! `HTMLParser()` handle accumulates through `feed`/`close`. Also hosts the
//! `escape`/`unescape` helpers reused by the `html` module.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::gc::{new_dict, new_list};
use crate::native::{func, host_module_mixed};
use crate::ops::string_arg;
use crate::value::{DictKey, Value};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        func("HTMLParser", 880, 0, 0),
        func("feed", 881, 2, 2),
        func("close", 882, 1, 1),
        func("tags", 883, 1, 1),
        func("data", 884, 1, 1),
        func("events", 885, 1, 1),
        func("parse", 886, 1, 1),
        func("unescape", 887, 1, 1),
        func("escape", 888, 1, 1),
    ])
}

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    match id {
        880 => parser_new(),
        881 => feed(args),
        882 => Ok(Value::nil()),
        883 => Ok(list_of(tags_of(&buffer(args)?))),
        884 => Ok(Value::string(data_of(&buffer(args)?))),
        885 => Ok(list_of(events_of(&buffer(args)?))),
        886 => Ok(list_of(tags_of(&string_arg(args, 0)?))),
        887 => Ok(Value::string(unescape_str(&string_arg(args, 0)?))),
        888 => Ok(Value::string(escape_str(&string_arg(args, 0)?))),
        889 => Ok(Value::string(data_of(&string_arg(args, 0)?))),
        890 => Ok(list_of(events_of(&string_arg(args, 0)?))),
        _ => Err(VmError::NativeError(format!(
            "unknown html.parser native id {id}"
        ))),
    }
}

// ── handle registry ──────────────────────────────────────────────────────────

fn parsers() -> &'static Mutex<HashMap<u64, String>> {
    static REG: OnceLock<Mutex<HashMap<u64, String>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

fn parser_new() -> VmResult<Value> {
    let mut reg = parsers().lock().unwrap_or_else(|e| e.into_inner());
    let mut id = 1u64;
    while reg.contains_key(&id) {
        id += 1;
    }
    reg.insert(id, String::new());
    Ok(Value::int_promote(id as i64))
}

fn handle(args: &[Value]) -> VmResult<u64> {
    let id = crate::ops::int_arg(args, 0)?;
    if id <= 0 {
        return Err(VmError::HandleError(format!("bad HTMLParser handle {id}")));
    }
    Ok(id as u64)
}

fn feed(args: &[Value]) -> VmResult<Value> {
    let id = handle(args)?;
    let text = string_arg(args, 1)?;
    let mut reg = parsers().lock().unwrap_or_else(|e| e.into_inner());
    let buf = reg
        .get_mut(&id)
        .ok_or_else(|| VmError::HandleError(format!("no such HTMLParser {id}")))?;
    buf.push_str(&text);
    Ok(Value::nil())
}

fn buffer(args: &[Value]) -> VmResult<String> {
    let id = handle(args)?;
    let reg = parsers().lock().unwrap_or_else(|e| e.into_inner());
    reg.get(&id)
        .cloned()
        .ok_or_else(|| VmError::HandleError(format!("no such HTMLParser {id}")))
}

// ── tokenizer ────────────────────────────────────────────────────────────────

pub(crate) enum Event {
    Start { tag: String, attrs: IndexMap<DictKey, Value>, self_closing: bool },
    End { tag: String },
    Data(String),
    Comment(String),
    Decl(String),
}

pub(crate) fn tokenize_events(text: &str) -> Vec<Event> {
    tokenize(text)
}

fn tokenize(text: &str) -> Vec<Event> {
    let bytes = text.as_bytes();
    let mut events = Vec::new();
    let mut i = 0;
    let mut data = String::new();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if !data.is_empty() {
                events.push(Event::Data(unescape_str(&data)));
                data.clear();
            }
            // Comment
            if text[i..].starts_with("<!--") {
                let end = text[i + 4..].find("-->").map(|p| i + 4 + p);
                match end {
                    Some(e) => {
                        events.push(Event::Comment(text[i + 4..e].to_string()));
                        i = e + 3;
                    }
                    None => {
                        events.push(Event::Comment(text[i + 4..].to_string()));
                        break;
                    }
                }
                continue;
            }
            // Declaration <! ... > or <? ... ?>
            if text[i..].starts_with("<!") || text[i..].starts_with("<?") {
                let end = text[i..].find('>').map(|p| i + p);
                match end {
                    Some(e) => {
                        events.push(Event::Decl(text[i..=e].to_string()));
                        i = e + 1;
                    }
                    None => {
                        events.push(Event::Decl(text[i..].to_string()));
                        break;
                    }
                }
                continue;
            }
            match text[i..].find('>') {
                Some(rel) => {
                    let inner = &text[i + 1..i + rel];
                    parse_tag(inner, &mut events);
                    i += rel + 1;
                }
                None => {
                    data.push_str(&text[i..]);
                    break;
                }
            }
        } else {
            data.push(bytes[i] as char);
            i += 1;
        }
    }
    if !data.is_empty() {
        events.push(Event::Data(unescape_str(&data)));
    }
    events
}

fn parse_tag(inner: &str, events: &mut Vec<Event>) {
    let inner = inner.trim();
    let self_closing = inner.ends_with('/');
    let body = inner.trim_end_matches('/').trim();
    if let Some(name) = body.strip_prefix('/') {
        events.push(Event::End {
            tag: name.trim().to_ascii_lowercase(),
        });
        return;
    }
    let mut chars = body.char_indices().peekable();
    let mut name_len = 0;
    for (idx, c) in body.char_indices() {
        if c.is_whitespace() || c == '/' {
            break;
        }
        name_len = idx + c.len_utf8();
        let _ = chars.peek();
    }
    let (tag, rest) = body.split_at(name_len);
    if tag.is_empty() {
        return;
    }
    let attrs = parse_attrs(rest);
    events.push(Event::Start {
        tag: tag.to_ascii_lowercase(),
        attrs,
        self_closing,
    });
}

fn parse_attrs(rest: &str) -> IndexMap<DictKey, Value> {
    let mut attrs = IndexMap::new();
    let b = rest.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i] as char).is_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let name_start = i;
        while i < b.len() && !(b[i] as char).is_whitespace() && b[i] != b'=' {
            i += 1;
        }
        let name = rest[name_start..i].trim().to_ascii_lowercase();
        while i < b.len() && (b[i] as char).is_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && (b[i] as char).is_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let quote = b[i];
                i += 1;
                let vstart = i;
                while i < b.len() && b[i] != quote {
                    i += 1;
                }
                value = rest[vstart..i].to_string();
                if i < b.len() {
                    i += 1;
                }
            } else {
                let vstart = i;
                while i < b.len() && !(b[i] as char).is_whitespace() {
                    i += 1;
                }
                value = rest[vstart..i].to_string();
            }
        }
        if !name.is_empty() {
            attrs.insert(DictKey(Value::string(name)), Value::string(unescape_str(&value)));
        }
    }
    attrs
}

// ── projection ───────────────────────────────────────────────────────────────

fn events_of(text: &str) -> Vec<Value> {
    events_to_values(&tokenize(text))
}

fn events_to_values(events: &[Event]) -> Vec<Value> {
    events.iter().map(event_value).collect()
}

fn event_value(e: &Event) -> Value {
    let mut d: IndexMap<DictKey, Value> = IndexMap::new();
    match e {
        Event::Start { tag, attrs, self_closing } => {
            d.insert(DictKey(Value::string("type")), Value::string(if *self_closing { "startend" } else { "start" }));
            d.insert(DictKey(Value::string("tag")), Value::string(tag.clone()));
            d.insert(DictKey(Value::string("attrs")), new_dict(attrs.clone()));
            d.insert(DictKey(Value::string("data")), Value::nil());
        }
        Event::End { tag } => {
            d.insert(DictKey(Value::string("type")), Value::string("end"));
            d.insert(DictKey(Value::string("tag")), Value::string(tag.clone()));
            d.insert(DictKey(Value::string("attrs")), new_dict(IndexMap::new()));
            d.insert(DictKey(Value::string("data")), Value::nil());
        }
        Event::Data(text) => {
            d.insert(DictKey(Value::string("type")), Value::string("data"));
            d.insert(DictKey(Value::string("tag")), Value::nil());
            d.insert(DictKey(Value::string("attrs")), new_dict(IndexMap::new()));
            d.insert(DictKey(Value::string("data")), Value::string(text.clone()));
        }
        Event::Comment(text) => {
            d.insert(DictKey(Value::string("type")), Value::string("comment"));
            d.insert(DictKey(Value::string("tag")), Value::nil());
            d.insert(DictKey(Value::string("attrs")), new_dict(IndexMap::new()));
            d.insert(DictKey(Value::string("data")), Value::string(text.clone()));
        }
        Event::Decl(text) => {
            d.insert(DictKey(Value::string("type")), Value::string("decl"));
            d.insert(DictKey(Value::string("tag")), Value::nil());
            d.insert(DictKey(Value::string("attrs")), new_dict(IndexMap::new()));
            d.insert(DictKey(Value::string("data")), Value::string(text.clone()));
        }
    }
    new_dict(d)
}

fn tags_of(text: &str) -> Vec<Value> {
    tokenize(text)
        .iter()
        .filter_map(|e| match e {
            Event::Start { .. } => Some(event_value(e)),
            _ => None,
        })
        .collect()
}

fn data_of(text: &str) -> String {
    let mut out = String::new();
    for e in tokenize(text) {
        if let Event::Data(t) = e {
            out.push_str(&t);
        }
    }
    out
}

fn list_of(items: Vec<Value>) -> Value {
    new_list(items)
}

// ── entities ─────────────────────────────────────────────────────────────────

pub(crate) fn unescape_str(s: &str) -> String {
    const NAMED: &[(&str, &str)] = &[
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&apos;", "'"),
        ("&nbsp;", "\u{a0}"),
        ("&copy;", "\u{a9}"),
        ("&reg;", "\u{ae}"),
        ("&hellip;", "\u{2026}"),
        ("&mdash;", "\u{2014}"),
        ("&ndash;", "\u{2013}"),
        ("&rsquo;", "\u{2019}"),
        ("&lsquo;", "\u{2018}"),
        ("&rdquo;", "\u{201d}"),
        ("&ldquo;", "\u{201c}"),
    ];
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'&' {
            if let Some(semi) = s[i..].find(';').filter(|p| *p <= 12) {
                let entity = &s[i..i + semi + 1];
                let replaced = NAMED
                    .iter()
                    .find(|(name, _)| *name == entity)
                    .map(|(_, repl)| repl.to_string())
                    .or_else(|| numeric_entity(entity));
                if let Some(r) = replaced {
                    out.push_str(&r);
                    i += semi + 1;
                    continue;
                }
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

fn numeric_entity(entity: &str) -> Option<String> {
    let inner = entity.strip_prefix("&#")?.strip_suffix(';')?;
    let code = if let Some(hex) = inner.strip_prefix(['x', 'X']) {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        inner.parse::<u32>().ok()?
    };
    char::from_u32(code).map(|c| c.to_string())
}

pub(crate) fn escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}
