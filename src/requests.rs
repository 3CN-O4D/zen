//! Zen `requests` module — Python `requests` library parity, implemented in Rust.
//!
//! Exposed globally as `requests`. Provides:
//!   - `requests.request/get/post/put/patch/delete/head/options(url, opts)`
//!   - `requests.Session()` — stateful sessions (cookie jar, headers, auth,
//!     proxies, verify, timeout, open/closed).
//!   - A rich `Response` object (status_code, headers, url, reason, encoding,
//!     elapsed, history, cookies, ok, is_redirect, next, plus json/text/content,
//!     iter_content/iter_lines, raise_for_status, close).
//!   - `requests.codes` / `status_codes` and `requests.exceptions.*`.
//!   - `requests.utils.quote/unquote/default_user_agent`.
//!
//! Cookies and redirects are managed manually (Python `requests` semantics):
//! redirects populate `Response.history`, and session cookies persist.

use crate::runtime::{json_decode, json_encode, Value, Vm};
use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use base64::Engine;

// ─────────────────────────────────────────────────────────────────────────────
// Persistent state
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Cookie {
    name: String,
    value: String,
    path: String,
    secure: bool,
    http_only: bool,
    expires: Option<f64>,
}

#[derive(Clone, Default)]
#[allow(dead_code)]
struct SessionState {
    jar: IndexMap<String, Vec<Cookie>>,
    headers: IndexMap<String, String>,
    auth: Option<(String, String)>,
    verify: bool,
    timeout: Option<f64>,
    max_redirects: u32,
    proxies: IndexMap<String, String>,
    trust_env: bool,
    closed: bool,
}

#[derive(Clone)]
#[allow(dead_code)]
struct ResponseState {
    status: u16,
    headers: IndexMap<String, Vec<String>>,
    body: Vec<u8>,
    url: String,
    reason: String,
    encoding: String,
    elapsed_ms: u64,
    history: Vec<u64>,
    request_line: String,
    cookies_added: Vec<Cookie>,
    next_url: Option<String>,
    next_method: String,
    method: String,
}

fn sessions() -> &'static Mutex<HashMap<u64, SessionState>> {
    static S: OnceLock<Mutex<HashMap<u64, SessionState>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

fn responses() -> &'static Mutex<HashMap<u64, ResponseState>> {
    static R: OnceLock<Mutex<HashMap<u64, ResponseState>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_id() -> u64 {
    static C: AtomicU64 = AtomicU64::new(1);
    C.fetch_add(1, Ordering::Relaxed)
}

// ─────────────────────────────────────────────────────────────────────────────
// Value helpers
// ─────────────────────────────────────────────────────────────────────────────

fn to_str(v: &Value) -> String {
    v.to_string()
}

fn get_str(m: &IndexMap<String, Value>, key: &str) -> Option<String> {
    m.get(key).map(to_str)
}

#[allow(dead_code)]
fn get_num(m: &IndexMap<String, Value>, key: &str) -> Option<f64> {
    m.get(key).and_then(|v| match v {
        Value::Number(n) => Some(*n),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    })
}

fn get_bool(m: &IndexMap<String, Value>, key: &str, default: bool) -> bool {
    match m.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => matches!(s.as_str(), "true" | "1" | "True"),
        Some(Value::Number(n)) => *n != 0.0,
        _ => default,
    }
}

fn dict_to_strings(m: &IndexMap<String, Value>) -> IndexMap<String, String> {
    m.iter().map(|(k, v)| (k.clone(), to_str(v))).collect()
}

#[allow(dead_code)]
fn extract_list(v: &Value) -> Vec<Value> {
    match v {
        Value::List(l) => (**l).clone(),
        _ => vec![v.clone()],
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// URL / query / encoding helpers
// ─────────────────────────────────────────────────────────────────────────────

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (
                (bytes[i + 1] as char).to_digit(16),
                (bytes[i + 2] as char).to_digit(16),
            ) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn build_query(params: &IndexMap<String, Value>) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (k, v) in params {
        if let Value::List(items) = v {
            for item in items.iter() {
                parts.push(format!(
                    "{}={}",
                    percent_encode(k),
                    percent_encode(&to_str(item))
                ));
            }
        } else {
            parts.push(format!("{}={}", percent_encode(k), percent_encode(&to_str(v))));
        }
    }
    parts.join("&")
}

fn reason_for(status: u16) -> String {
    let r = match status {
        100 => "Continue",
        101 => "Switching Protocols",
        102 => "Processing",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoritative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        207 => "Multi-Status",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        305 => "Use Proxy",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition Failed",
        413 => "Request Entity Too Large",
        414 => "Request-URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Requested Range Not Satisfiable",
        417 => "Expectation Failed",
        418 => "I'm a teapot",
        421 => "Misdirected Request",
        422 => "Unprocessable Entity",
        423 => "Locked",
        424 => "Failed Dependency",
        425 => "Too Early",
        426 => "Upgrade Required",
        428 => "Precondition Required",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        451 => "Unavailable For Legal Reasons",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version Not Supported",
        506 => "Variant Also Negotiates",
        507 => "Insufficient Storage",
        508 => "Loop Detected",
        510 => "Not Extended",
        511 => "Network Authentication Required",
        _ => "Unknown",
    };
    r.to_string()
}

fn now_epoch() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn parse_http_date(s: &str) -> Option<f64> {
    let formats = [
        "%a, %d %b %Y %H:%M:%S %Z",
        "%A, %d-%b-%y %H:%M:%S %Z",
        "%a %b %e %H:%M:%S %Y",
    ];
    for fmt in formats {
        if let Ok(dt) = chrono::DateTime::parse_from_str(s.trim(), fmt) {
            return Some(dt.timestamp() as f64);
        }
    }
    None
}

fn guess_charset(headers: &IndexMap<String, Vec<String>>, body: &[u8]) -> String {
    if let Some(ct) = headers.get("content-type").and_then(|v| v.first()) {
        let lower = ct.to_ascii_lowercase();
        if let Some(idx) = lower.find("charset=") {
            let val = lower[idx + 8..].split([';', ' ', '"']).next().unwrap_or("");
            if !val.is_empty() {
                return val.to_string();
            }
        }
    }
    if body.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return "utf-8-sig".to_string();
    }
    "utf-8".to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Cookie handling
// ─────────────────────────────────────────────────────────────────────────────

fn parse_cookie(header: &str) -> Option<Cookie> {
    let first = header.split(';').next()?.trim();
    let (name, value) = first.split_once('=')?;
    let mut c = Cookie {
        name: name.trim().to_string(),
        value: value.trim().trim_matches('"').to_string(),
        path: "/".to_string(),
        secure: false,
        http_only: false,
        expires: None,
    };
    for part in header.split(';').skip(1) {
        let part = part.trim();
        let lower = part.to_ascii_lowercase();
        if let Some((k, v)) = part.split_once('=') {
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim().trim_matches('"');
            if k == "path" && !v.is_empty() {
                c.path = v.to_string();
            } else if k == "max-age" {
                if let Ok(secs) = v.parse::<f64>() {
                    c.expires = Some(now_epoch() + secs);
                }
            } else if k == "expires" {
                if let Some(ts) = parse_http_date(v) {
                    c.expires = Some(ts);
                }
            }
        } else if lower == "secure" {
            c.secure = true;
        } else if lower == "httponly" {
            c.http_only = true;
        }
    }
    Some(c)
}

fn domain_matches(cookie_domain: &str, host: &str) -> bool {
    let dc = cookie_domain.to_ascii_lowercase();
    let h = host.to_ascii_lowercase();
    h == dc || h.ends_with(&format!(".{}", dc))
}

fn path_matches(cookie_path: &str, url_path: &str) -> bool {
    let up = if url_path.is_empty() { "/" } else { url_path };
    up == cookie_path
        || up.starts_with(cookie_path)
        || (cookie_path.ends_with('/')
            && up.starts_with(&cookie_path[..cookie_path.len() - 1]))
}

// ─────────────────────────────────────────────────────────────────────────────
// Request execution
// ─────────────────────────────────────────────────────────────────────────────

fn execute(
    session: &mut SessionState,
    method: &str,
    url: &str,
    opts: &IndexMap<String, Value>,
    allow_redirects: bool,
) -> Result<Value, String> {
    if session.closed {
        return Err("requests.exceptions.RequestException: Session is closed".into());
    }

    let mut headers: IndexMap<String, String> = match opts.get("headers") {
        Some(Value::Dict(d)) => dict_to_strings(d),
        _ => IndexMap::new(),
    };
    for (k, v) in session.headers.clone() {
        if !headers.keys().any(|h| h.eq_ignore_ascii_case(&k)) {
            headers.insert(k, v);
        }
    }

    let timeout = match opts.get("timeout") {
        Some(Value::Number(n)) => Some(*n),
        _ => session.timeout,
    };
    let verify = get_bool(opts, "verify", session.verify);
    let max_redirects = match opts.get("max_redirects") {
        Some(Value::Number(n)) => Some(*n as u32),
        _ => Some(session.max_redirects),
    };

    let basic_auth: Option<(String, String)> = match opts.get("auth") {
        Some(Value::String(s)) => {
            let t = s.splitn(2, ':').collect::<Vec<_>>();
            Some((t[0].to_string(), t.get(1).cloned().unwrap_or_default().to_string()))
        }
        Some(Value::List(l)) if !l.is_empty() => {
            Some((to_str(&l[0]), if l.len() > 1 { to_str(&l[1]) } else { String::new() }))
        }
        _ => session.auth.clone(),
    };

    // Explicit per-request cookies.
    let mut explicit: Vec<(String, String)> = Vec::new();
    if let Some(Value::Dict(cd)) = opts.get("cookies") {
        for (k, v) in cd.iter() {
            explicit.push((k.clone(), to_str(v)));
        }
    }
    if let Some(Value::List(cl)) = opts.get("cookies") {
        for item in cl.iter() {
            if let Value::Dict(d) = item {
                if let Some(n) = d.get("name").map(to_str) {
                    explicit.push((n, d.get("value").map(to_str).unwrap_or_default()));
                }
            }
        }
    }

    // Query params.
    let mut params: IndexMap<String, Value> = IndexMap::new();
    if let Some(Value::Dict(pd)) = opts.get("params") {
        for (k, v) in pd.iter() {
            params.insert(k.clone(), v.clone());
        }
    }
    let mut eff_url = url.to_string();
    if !params.is_empty() {
        let qs = build_query(&params);
        eff_url = if eff_url.contains('?') {
            format!("{}&{}", eff_url, qs)
        } else {
            format!("{}?{}", eff_url, qs)
        };
    }

    // Body.
    let mut body: Option<Vec<u8>> = None;
    if let Some(Value::String(s)) = opts.get("data") {
        body = Some(s.clone().into_bytes());
    } else if let Some(Value::Dict(d)) = opts.get("data") {
        let mut parts = Vec::new();
        for (k, v) in d.iter() {
            parts.push(format!(
                "{}={}",
                percent_encode(&k),
                percent_encode(&to_str(v))
            ));
        }
        body = Some(parts.join("&").into_bytes());
        headers
            .entry("content-type".into())
            .or_insert_with(|| "application/x-www-form-urlencoded".into());
    }

    if let Some(v) = opts.get("json") {
        let js = json_encode(v, false);
        body = Some(js.into_bytes());
        headers.insert("content-type".into(), "application/json".into());
    }

    if let Some(Value::Dict(fd)) = opts.get("files") {
        let boundary = format!("------------ZenBoundary{}", next_id());
        let mut buf: Vec<u8> = Vec::new();
        for (name, spec) in fd.iter() {
            let (filename, content): (String, Vec<u8>) = match spec {
                Value::Dict(d) => (
                    d.get("filename").map(to_str).unwrap_or_else(|| name.clone()),
                    d.get("data")
                        .map(|v| to_str(v).into_bytes())
                        .unwrap_or_default(),
                ),
                _ => (name.clone(), to_str(spec).into_bytes()),
            };
            buf.extend_from_slice(
                format!(
                    "--{}\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n\r\n",
                    boundary, name, filename
                )
                .as_bytes(),
            );
            buf.extend_from_slice(&content);
            buf.extend_from_slice(b"\r\n");
        }
        buf.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
        body = Some(buf);
        headers.insert(
            "content-type".into(),
            format!("multipart/form-data; boundary={}", boundary),
        );
    }

    let mut jar = session.jar.clone();
    let res = run_loop(
        &mut jar,
        method,
        &eff_url,
        &headers,
        body,
        timeout,
        verify,
        max_redirects.unwrap_or(30),
        allow_redirects,
        &basic_auth,
        &explicit,
    )?;
    // Persist the (possibly updated) cookie jar back into the session.
    session.jar = jar;
    Ok(res)
}

fn run_loop(
    jar: &mut IndexMap<String, Vec<Cookie>>,
    method: &str,
    url: &str,
    headers: &IndexMap<String, String>,
    body: Option<Vec<u8>>,
    timeout: Option<f64>,
    verify: bool,
    limit: u32,
    allow_redirects: bool,
    basic_auth: &Option<(String, String)>,
    explicit: &[(String, String)],
) -> Result<Value, String> {
    let mut current_method = method.to_string();
    let mut current_url = url.to_string();
    let mut body = body;
    let mut history: Vec<u64> = Vec::new();
    let mut hops: u32 = 0;

    loop {
        let mut builder = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none());
        if !verify {
            builder = builder.danger_accept_invalid_certs(true);
        }
        if let Some(t) = timeout {
            builder = builder.timeout(std::time::Duration::from_secs_f64(t));
        }
        let client = builder
            .build()
            .map_err(|e| format!("requests.exceptions.RequestException: client build failed: {}", e))?;

        let method_obj = reqwest::Method::from_bytes(current_method.as_bytes())
            .map_err(|e| format!("requests.exceptions.RequestException: invalid method {}: {}", current_method, e))?;
        let mut req = client.request(method_obj, &current_url);
        for (k, v) in headers {
            req = req.header(k, v);
        }

        // Build Cookie header from jar (for current URL) + explicit cookies.
        let mut ratio: Vec<String> = Vec::new();
        for (n, v) in explicit {
            ratio.push(format!("{}={}", n, v));
        }
        if let Ok(u) = reqwest::Url::parse(&current_url) {
            let host = u.host_str().unwrap_or("").to_ascii_lowercase();
            let path = u.path().to_string();
            for (domain, cookies) in jar.iter() {
                if !domain_matches(domain, &host) {
                    continue;
                }
                for c in cookies {
                    if c.expires.map(|e| e < now_epoch()).unwrap_or(false) {
                        continue;
                    }
                    if !path_matches(&c.path, &path) {
                        continue;
                    }
                    if explicit.iter().any(|(n, _)| *n == c.name) {
                        continue;
                    }
                    ratio.push(format!("{}={}", c.name, c.value));
                }
            }
        }
        if !ratio.is_empty() {
            req = req.header("Cookie", ratio.join("; "));
        }

        // Auth.
        let mut used = basic_auth.clone();
        if used.is_none() {
            if let Ok(u) = reqwest::Url::parse(&current_url) {
                if !u.username().is_empty() {
                    used = Some((u.username().to_string(), u.password().unwrap_or("").to_string()));
                }
            }
        }
        if let Some((user, pass)) = &used {
            let cred = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", user, pass));
            req = req.header("Authorization", format!("Basic {}", cred));
        }

        if let Some(b) = &body {
            req = req.body(b.clone());
        }

        let start = Instant::now();
        let resp = req.send().map_err(|e| {
            if e.is_timeout() {
                format!("requests.exceptions.ConnectTimeout: {}", current_url)
            } else if e.is_connect() {
                format!("requests.exceptions.ConnectionError: {}", current_url)
            } else {
                format!("requests.exceptions.ConnectionError: {}: {}", current_url, e)
            }
        })?;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let status = resp.status().as_u16();
        let resp_url = resp.url().to_string();
        let resp_headers = resp.headers().clone();

        let mut header_map: IndexMap<String, Vec<String>> = IndexMap::new();
        for (name, v) in resp_headers.iter() {
            let key = name.as_str().to_ascii_lowercase();
            let val = v.to_str().unwrap_or_default().to_string();
            header_map.entry(key).or_default().push(val);
        }

        // Capture Set-Cookie.
        let mut added: Vec<Cookie> = Vec::new();
        let host_for_jar = reqwest::Url::parse(&current_url)
            .ok()
            .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
            .unwrap_or_default();
        for sc in resp_headers.get_all("set-cookie").iter() {
            if let Some(sc) = sc.to_str().ok() {
                if let Some(c) = parse_cookie(sc) {
                    if c.expires.map(|e| e < now_epoch()).unwrap_or(false) {
                        continue;
                    }
                    added.push(c.clone());
                    let list = jar.entry(host_for_jar.clone()).or_default();
                    list.retain(|x| x.name != c.name);
                    list.push(c);
                }
            }
        }

        let body_bytes = resp
            .bytes()
            .map(|b| b.to_vec())
            .map_err(|e| format!("requests.exceptions.ChunkedEncodingError: {}", e))?;

        let rid = next_id();
        {
            let mut rs = responses()
                .lock()
                .map_err(|_| "requests: response store poisoned".to_string())?;
            let encoding = guess_charset(&header_map, &body_bytes);
            rs.insert(
                rid,
                ResponseState {
                    status,
                    headers: header_map.clone(),
                    body: body_bytes,
                    url: resp_url.clone(),
                    reason: reason_for(status),
                    encoding,
                    elapsed_ms,
                    history: history.clone(),
                    request_line: format!("<Request [{}]>", current_method),
                    cookies_added: added,
                    next_url: None,
                    next_method: current_method.clone(),
                    method: current_method.clone(),
                },
            );
        }

        // Redirect handling.
        if status >= 300 && status < 400 && allow_redirects && hops <= limit {
            let loc = header_map
                .get("location")
                .and_then(|v| v.first())
                .cloned();
            if let Some(loc) = loc {
                let next_url = resolve_url(&current_url, &loc);
                if status == 303 || ((status == 301 || status == 302) && current_method == "POST") {
                    current_method = "GET".to_string();
                    body = None;
                }
                history.push(rid);
                current_url = next_url;
                hops += 1;
                if hops > limit {
                    return Err("requests.exceptions.TooManyRedirects: exceeded max_redirects".into());
                }
                continue;
            }
        }

        let next_url_val = if status >= 300 && status < 400 {
            header_map
                .get("location")
                .and_then(|v| v.first())
                .map(|l| resolve_url(&current_url, l))
        } else {
            None
        };
        {
            let mut rs = responses()
                .lock()
                .map_err(|_| "requests: response store poisoned".to_string())?;
            if let Some(s) = rs.get_mut(&rid) {
                s.next_url = next_url_val.clone();
            }
        }

        return Ok(make_response_value(rid, &next_url_val, current_method.as_str()));
    }
}

fn resolve_url(base: &str, loc: &str) -> String {
    if let Ok(b) = reqwest::Url::parse(base) {
        if let Ok(l) = b.join(loc) {
            return l.to_string();
        }
        return base.to_string();
    }
    if let Ok(l) = reqwest::Url::parse(loc) {
        return l.to_string();
    }
    loc.to_string()
}

fn make_response_value(rid: u64, next_url: &Option<String>, method: &str) -> Value {
    let rs = responses().lock().unwrap().get(&rid).cloned();
    let rs = match rs {
        Some(r) => r,
        None => {
            return Value::Dict(Arc::new(IndexMap::new()));
        }
    };

    let mut headers_dict = IndexMap::new();
    for (k, v) in &rs.headers {
        if v.len() == 1 {
            headers_dict.insert(k.clone(), Value::String(v[0].clone()));
        } else {
            headers_dict.insert(
                k.clone(),
                Value::List(Arc::new(v.iter().map(|s| Value::String(s.clone())).collect())),
            );
        }
    }

    let is_redirect = (300..400).contains(&rs.status);
    let is_permanent = rs.status == 301 || rs.status == 308;

    let mut fields: IndexMap<String, Value> = IndexMap::new();
    fields.insert("__rid".into(), Value::Number(rid as f64));
    fields.insert("status_code".into(), Value::Number(rs.status as f64));
    fields.insert("ok".into(), Value::Bool((200..400).contains(&rs.status)));
    fields.insert("url".into(), Value::String(rs.url.clone()));
    fields.insert("reason".into(), Value::String(rs.reason.clone()));
    fields.insert("encoding".into(), Value::String(rs.encoding.clone()));
    fields.insert("elapsed".into(), Value::Number(rs.elapsed_ms as f64 / 1000.0));
    fields.insert("headers".into(), Value::Dict(Arc::new(headers_dict.clone())));
    fields.insert("is_redirect".into(), Value::Bool(is_redirect));
    fields.insert("is_permanent_redirect".into(), Value::Bool(is_permanent));
    fields.insert(
        "request".into(),
        Value::Dict(Arc::new(IndexMap::from([
            ("type".into(), Value::String("PreparedRequest".into())),
            ("method".into(), Value::String(rs.method.clone())),
            ("url".into(), Value::String(rs.url.clone())),
            ("headers".into(), Value::Dict(Arc::new(headers_dict))),
        ]))),
    );

    let next = match next_url {
        Some(n) => Value::Dict(Arc::new(IndexMap::from([
            ("type".into(), Value::String("PreparedRequest".into())),
            ("method".into(), Value::String(method.to_string())),
            ("url".into(), Value::String(n.clone())),
        ]))),
        None => Value::Null,
    };
    fields.insert("next".into(), next);

    let mut hist: Vec<Value> = Vec::new();
    {
        let all = responses().lock().unwrap();
        for h in &rs.history {
            if let Some(hs) = all.get(h) {
                let mut hf = IndexMap::new();
                hf.insert("status_code".into(), Value::Number(hs.status as f64));
                hf.insert("ok".into(), Value::Bool((200..400).contains(&hs.status)));
                hf.insert("url".into(), Value::String(hs.url.clone()));
                hf.insert("reason".into(), Value::String(hs.reason.clone()));
                hf.insert("is_redirect".into(), Value::Bool((300..400).contains(&hs.status)));
                hf.insert("history".into(), Value::List(Arc::new(Vec::new())));
                hist.push(Value::Dict(Arc::new(hf)));
            }
        }
    }
    fields.insert("history".into(), Value::List(Arc::new(hist)));

    let mut ck: Vec<Value> = Vec::new();
    for c in &rs.cookies_added {
        ck.push(Value::Dict(Arc::new(IndexMap::from([
            ("name".into(), Value::String(c.name.clone())),
            ("value".into(), Value::String(c.value.clone())),
            ("path".into(), Value::String(c.path.clone())),
            ("secure".into(), Value::Bool(c.secure)),
            ("http_only".into(), Value::Bool(c.http_only)),
        ]))));
    }
    fields.insert("cookies".into(), Value::List(Arc::new(ck)));

    fields.insert("json".into(), Value::NativeFunction("__requests_resp_json".into()));
    fields.insert("text".into(), Value::NativeFunction("__requests_resp_text".into()));
    fields.insert("content".into(), Value::NativeFunction("__requests_resp_content".into()));
    fields.insert("iter_content".into(), Value::NativeFunction("__requests_resp_iter_content".into()));
    fields.insert("iter_lines".into(), Value::NativeFunction("__requests_resp_iter_lines".into()));
    fields.insert("raise_for_status".into(), Value::NativeFunction("__requests_resp_raise_for_status".into()));
    fields.insert("close".into(), Value::NativeFunction("__requests_resp_close".into()));

    Value::Dict(Arc::new(fields))
}

// ─────────────────────────────────────────────────────────────────────────────
// Module assembly
// ─────────────────────────────────────────────────────────────────────────────

pub fn init_requests_module(vm: &mut Vm) {
    let requests = Value::Dict(Arc::new(indexmap::IndexMap::from([
        ("request".into(), Value::NativeFunction("requests_request".into())),
        ("get".into(), Value::NativeFunction("requests_get".into())),
        ("options".into(), Value::NativeFunction("requests_options".into())),
        ("head".into(), Value::NativeFunction("requests_head".into())),
        ("post".into(), Value::NativeFunction("requests_post".into())),
        ("put".into(), Value::NativeFunction("requests_put".into())),
        ("patch".into(), Value::NativeFunction("requests_patch".into())),
        ("delete".into(), Value::NativeFunction("requests_delete".into())),
        ("Session".into(), Value::NativeFunction("requests_session".into())),
        (
            "exceptions".into(),
            Value::Dict(Arc::new(indexmap::IndexMap::from([
                ("RequestException".into(), Value::NativeFunction("requests_exc".into())),
                ("HTTPError".into(), Value::NativeFunction("requests_exc_http".into())),
                ("ConnectionError".into(), Value::NativeFunction("requests_exc".into())),
                ("ProxyError".into(), Value::NativeFunction("requests_exc".into())),
                ("SSLError".into(), Value::NativeFunction("requests_exc".into())),
                ("Timeout".into(), Value::NativeFunction("requests_exc".into())),
                ("ConnectTimeout".into(), Value::NativeFunction("requests_exc".into())),
                ("ReadTimeout".into(), Value::NativeFunction("requests_exc".into())),
                ("URLRequired".into(), Value::NativeFunction("requests_exc".into())),
                ("TooManyRedirects".into(), Value::NativeFunction("requests_exc".into())),
                ("MissingSchema".into(), Value::NativeFunction("requests_exc".into())),
                ("InvalidSchema".into(), Value::NativeFunction("requests_exc".into())),
                ("InvalidURL".into(), Value::NativeFunction("requests_exc".into())),
                ("ChunkedEncodingError".into(), Value::NativeFunction("requests_exc".into())),
            ]))),
        ),
        ("codes".into(), Value::NativeFunction("requests_codes".into())),
        ("status_codes".into(), Value::NativeFunction("requests_codes".into())),
        (
            "utils".into(),
            Value::Dict(Arc::new(indexmap::IndexMap::from([
                ("quote".into(), Value::NativeFunction("requests_utils_quote".into())),
                ("unquote".into(), Value::NativeFunction("requests_utils_unquote".into())),
                ("default_user_agent".into(), Value::NativeFunction("requests_utils_ua".into())),
                ("get_encoding_from_headers".into(), Value::NativeFunction("requests_utils_encode".into())),
            ]))),
        ),
    ])));
    vm.vars.insert("requests".into(), requests);
}

// ─────────────────────────────────────────────────────────────────────────────
// Native entry points
// ─────────────────────────────────────────────────────────────────────────────

fn opts_from(args: &[Value], idx: usize) -> IndexMap<String, Value> {
    match args.get(idx) {
        Some(Value::Dict(d)) => (**d).clone(),
        _ => IndexMap::new(),
    }
}

fn rid_from(args: &[Value]) -> Result<u64, String> {
    match args.first() {
        Some(Value::Dict(d)) => match d.get("__rid") {
            Some(Value::Number(n)) => Ok(*n as u64),
            _ => Err("requests: expected a Response object".into()),
        },
        _ => Err("requests: expected a Response object".into()),
    }
}

fn do_get_response_body(rid: u64) -> Result<Vec<u8>, String> {
    responses()
        .lock()
        .map_err(|_| "requests: response store poisoned".to_string())?
        .get(&rid)
        .map(|r| r.body.clone())
        .ok_or_else(|| "requests: Response no longer available".into())
}

pub fn requests_request(args: &[Value]) -> Result<Value, String> {
    let method = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("requests.request: expects a method string".into()),
    };
    let url = match args.get(1) {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("requests.request: expects a url string".into()),
    };
    let opts = opts_from(args, 2);
    let mut session = SessionState {
        verify: true,
        max_redirects: 30,
        ..Default::default()
    };
    let allow = get_bool(&opts, "allow_redirects", true);
    execute(&mut session, &method.to_ascii_uppercase(), &url, &opts, allow)
}

pub fn requests_get(args: &[Value]) -> Result<Value, String> {
    let url = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("requests.get: expects a url string".into()),
    };
    let opts = opts_from(args, 1);
    let mut session = SessionState {
        verify: true,
        max_redirects: 30,
        ..Default::default()
    };
    execute(&mut session, "GET", &url, &opts, get_bool(&opts, "allow_redirects", true))
}

pub fn requests_post(args: &[Value]) -> Result<Value, String> {
    let url = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("requests.post: expects a url string".into()),
    };
    let opts = opts_from(args, 1);
    let mut session = SessionState {
        verify: true,
        max_redirects: 30,
        ..Default::default()
    };
    execute(&mut session, "POST", &url, &opts, get_bool(&opts, "allow_redirects", true))
}

pub fn requests_put(args: &[Value]) -> Result<Value, String> {
    let url = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("requests.put: expects a url string".into()),
    };
    let opts = opts_from(args, 1);
    let mut session = SessionState {
        verify: true,
        max_redirects: 30,
        ..Default::default()
    };
    execute(&mut session, "PUT", &url, &opts, get_bool(&opts, "allow_redirects", true))
}

pub fn requests_patch(args: &[Value]) -> Result<Value, String> {
    let url = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("requests.patch: expects a url string".into()),
    };
    let opts = opts_from(args, 1);
    let mut session = SessionState {
        verify: true,
        max_redirects: 30,
        ..Default::default()
    };
    execute(&mut session, "PATCH", &url, &opts, get_bool(&opts, "allow_redirects", true))
}

pub fn requests_delete(args: &[Value]) -> Result<Value, String> {
    let url = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("requests.delete: expects a url string".into()),
    };
    let opts = opts_from(args, 1);
    let mut session = SessionState {
        verify: true,
        max_redirects: 30,
        ..Default::default()
    };
    execute(&mut session, "DELETE", &url, &opts, get_bool(&opts, "allow_redirects", true))
}

pub fn requests_head(args: &[Value]) -> Result<Value, String> {
    let url = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("requests.head: expects a url string".into()),
    };
    let opts = opts_from(args, 1);
    let mut session = SessionState {
        verify: true,
        max_redirects: 30,
        ..Default::default()
    };
    execute(&mut session, "HEAD", &url, &opts, get_bool(&opts, "allow_redirects", true))
}

pub fn requests_options(args: &[Value]) -> Result<Value, String> {
    let url = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("requests.options: expects a url string".into()),
    };
    let opts = opts_from(args, 1);
    let mut session = SessionState {
        verify: true,
        max_redirects: 30,
        ..Default::default()
    };
    execute(&mut session, "OPTIONS", &url, &opts, get_bool(&opts, "allow_redirects", true))
}

// Session

pub fn requests_session(args: &[Value]) -> Result<Value, String> {
    let _ = args;
    let sid = next_id();
    let sess = SessionState {
        verify: true,
        max_redirects: 30,
        ..Default::default()
    };
    sessions()
        .lock()
        .map_err(|_| "requests: session store poisoned".to_string())?
        .insert(sid, sess);

    let mut s = IndexMap::new();
    s.insert("__sid".into(), Value::Number(sid as f64));
    s.insert("type".into(), Value::String("Session".into()));
    s.insert("get".into(), Value::NativeFunction("__requests_sess_get".into()));
    s.insert("post".into(), Value::NativeFunction("__requests_sess_post".into()));
    s.insert("put".into(), Value::NativeFunction("__requests_sess_put".into()));
    s.insert("patch".into(), Value::NativeFunction("__requests_sess_patch".into()));
    s.insert("delete".into(), Value::NativeFunction("__requests_sess_delete".into()));
    s.insert("head".into(), Value::NativeFunction("__requests_sess_head".into()));
    s.insert("options".into(), Value::NativeFunction("__requests_sess_options".into()));
    s.insert("request".into(), Value::NativeFunction("__requests_sess_request".into()));
    s.insert("close".into(), Value::NativeFunction("__requests_sess_close".into()));
    s.insert("headers".into(), Value::NativeFunction("__requests_sess_headers".into()));
    s.insert("cookies".into(), Value::NativeFunction("__requests_sess_cookies".into()));
    Ok(Value::Dict(Arc::new(s)))
}

#[allow(dead_code)]
fn sess_access(args: &[Value]) -> Result<(u64, IndexMap<String, Value>), String> {
    let sid = sid_from(&args.first().cloned().unwrap_or(Value::Null))?;
    let opts = opts_from(args, 1);
    Ok((sid, opts))
}

fn sid_from(v: &Value) -> Result<u64, String> {
    match v {
        Value::Dict(d) => match d.get("__sid") {
            Some(Value::Number(n)) => Ok(*n as u64),
            _ => Err("requests: expected a Session object".into()),
        },
        _ => Err("requests: expected a Session object".into()),
    }
}

// s.request(method, url, opts): args = [session, method, url, opts]
fn sess_request_explicit(args: &[Value]) -> Result<Value, String> {
    let sid = sid_from(&args.first().cloned().unwrap_or(Value::Null))?;
    let method = match args.get(1) {
        Some(Value::String(s)) => s.to_ascii_uppercase(),
        _ => return Err("requests.Session.request: expects a method string".into()),
    };
    let url = match args.get(2) {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("requests.Session.request: expects a url string".into()),
    };
    let opts = opts_from(args, 3);
    let mut sess = sessions()
        .lock()
        .map_err(|_| "requests: session store poisoned".to_string())?
        .get(&sid)
        .cloned()
        .ok_or_else(|| "requests: invalid Session".to_string())?;
    let allow = get_bool(&opts, "allow_redirects", true);
    let res = execute(&mut sess, &method, &url, &opts, allow);
    sessions()
        .lock()
        .map_err(|_| "requests: session store poisoned".to_string())?
        .insert(sid, sess);
    res
}

// s.get(url, opts) etc.: args = [session, url, opts], method fixed.
fn sess_verb_inner(args: &[Value], method: &str) -> Result<Value, String> {
    let sid = sid_from(&args.first().cloned().unwrap_or(Value::Null))?;
    let url = match args.get(1) {
        Some(Value::String(s)) => s.clone(),
        _ => return Err(format!("requests.Session.{}: expects a url string", method.to_lowercase())),
    };
    let opts = opts_from(args, 2);
    let mut sess = sessions()
        .lock()
        .map_err(|_| "requests: session store poisoned".to_string())?
        .get(&sid)
        .cloned()
        .ok_or_else(|| "requests: invalid Session".to_string())?;
    let allow = get_bool(&opts, "allow_redirects", true);
    let res = execute(&mut sess, method, &url, &opts, allow);
    sessions()
        .lock()
        .map_err(|_| "requests: session store poisoned".to_string())?
        .insert(sid, sess);
    res
}

pub fn requests_sess_request(args: &[Value]) -> Result<Value, String> {
    sess_request_explicit(args)
}

pub fn requests_sess_get(args: &[Value]) -> Result<Value, String> {
    sess_verb_inner(args, "GET")
}
pub fn requests_sess_post(args: &[Value]) -> Result<Value, String> {
    sess_verb_inner(args, "POST")
}
pub fn requests_sess_put(args: &[Value]) -> Result<Value, String> {
    sess_verb_inner(args, "PUT")
}
pub fn requests_sess_patch(args: &[Value]) -> Result<Value, String> {
    sess_verb_inner(args, "PATCH")
}
pub fn requests_sess_delete(args: &[Value]) -> Result<Value, String> {
    sess_verb_inner(args, "DELETE")
}
pub fn requests_sess_head(args: &[Value]) -> Result<Value, String> {
    sess_verb_inner(args, "HEAD")
}
pub fn requests_sess_options(args: &[Value]) -> Result<Value, String> {
    sess_verb_inner(args, "OPTIONS")
}

pub fn requests_sess_close(args: &[Value]) -> Result<Value, String> {
    let sid = sid_from(&args.first().cloned().unwrap_or(Value::Null))?;
    let mut store = sessions()
        .lock()
        .map_err(|_| "requests: session store poisoned".to_string())?;
    if let Some(s) = store.get_mut(&sid) {
        s.closed = true;
    }
    Ok(Value::Null)
}

pub fn requests_sess_headers(args: &[Value]) -> Result<Value, String> {
    let sid = sid_from(&args.first().cloned().unwrap_or(Value::Null))?;
    let mut store = sessions()
        .lock()
        .map_err(|_| "requests: session store poisoned".to_string())?;
    let s = store
        .get_mut(&sid)
        .ok_or_else(|| "requests: invalid Session".to_string())?;
    if let Some(Value::Dict(d)) = args.get(1) {
        s.headers = (**d)
            .iter()
            .map(|(k, v)| (k.clone(), to_str(v)))
            .collect();
    }
    let mut out = IndexMap::new();
    for (k, v) in &s.headers {
        out.insert(k.clone(), Value::String(v.clone()));
    }
    Ok(Value::Dict(Arc::new(out)))
}

pub fn requests_sess_cookies(args: &[Value]) -> Result<Value, String> {
    let sid = sid_from(&args.first().cloned().unwrap_or(Value::Null))?;
    let store = sessions()
        .lock()
        .map_err(|_| "requests: session store poisoned".to_string())?;
    let s = store
        .get(&sid)
        .ok_or_else(|| "requests: invalid Session".to_string())?;
    let mut jar_list: Vec<Value> = Vec::new();
    for (domain, cookies) in &s.jar {
        for c in cookies {
            jar_list.push(Value::Dict(Arc::new(IndexMap::from([
                ("name".into(), Value::String(c.name.clone())),
                ("value".into(), Value::String(c.value.clone())),
                ("domain".into(), Value::String(domain.clone())),
                ("path".into(), Value::String(c.path.clone())),
                ("secure".into(), Value::Bool(c.secure)),
                ("http_only".into(), Value::Bool(c.http_only)),
            ]))));
        }
    }
    Ok(Value::List(Arc::new(jar_list)))
}

// Response methods

pub fn requests_resp_json(args: &[Value]) -> Result<Value, String> {
    let rid = rid_from(args)?;
    let body = do_get_response_body(rid)?;
    json_decode(&String::from_utf8_lossy(&body))
}

pub fn requests_resp_text(args: &[Value]) -> Result<Value, String> {
    let rid = rid_from(args)?;
    let body = do_get_response_body(rid)?;
    Ok(Value::String(String::from_utf8_lossy(&body).into_owned()))
}

pub fn requests_resp_content(args: &[Value]) -> Result<Value, String> {
    let rid = rid_from(args)?;
    let body = do_get_response_body(rid)?;
    Ok(Value::String(String::from_utf8_lossy(&body).into_owned()))
}

pub fn requests_resp_iter_content(args: &[Value]) -> Result<Value, String> {
    let rid = rid_from(args)?;
    let chunk = match args.get(1) {
        Some(Value::Number(n)) => (*n as usize).max(1),
        _ => 1024 * 128,
    };
    let body = do_get_response_body(rid)?;
    let mut chunks: Vec<Value> = Vec::new();
    if body.is_empty() {
        return Ok(Value::List(Arc::new(chunks)));
    }
    for part in body.chunks(chunk) {
        chunks.push(Value::String(String::from_utf8_lossy(part).into_owned()));
    }
    Ok(Value::List(Arc::new(chunks)))
}

pub fn requests_resp_iter_lines(args: &[Value]) -> Result<Value, String> {
    let rid = rid_from(args)?;
    let body = do_get_response_body(rid)?;
    let text = String::from_utf8_lossy(&body).into_owned();
    let mut lines: Vec<Value> = Vec::new();
    for line in text.lines() {
        let l = line.to_string();
        if let Some(stripped) = l.strip_suffix('\r') {
            lines.push(Value::String(stripped.to_string()));
        } else {
            lines.push(Value::String(l));
        }
    }
    Ok(Value::List(Arc::new(lines)))
}

pub fn requests_resp_raise_for_status(args: &[Value]) -> Result<Value, String> {
    let rid = rid_from(args)?;
    let store = responses()
        .lock()
        .map_err(|_| "requests: response store poisoned".to_string())?;
    let s = store
        .get(&rid)
        .ok_or_else(|| "requests: Response no longer available".to_string())?;
    if s.status >= 400 {
        return Err(format!(
            "requests.exceptions.HTTPError: {} Server Error: {} for url: {}",
            s.status, s.reason, s.url
        ));
    }
    Ok(Value::Null)
}

pub fn requests_resp_close(args: &[Value]) -> Result<Value, String> {
    let rid = rid_from(args)?;
    responses()
        .lock()
        .map_err(|_| "requests: response store poisoned".to_string())?
        .remove(&rid);
    Ok(Value::Null)
}

// codes / status_codes

pub fn requests_codes(args: &[Value]) -> Result<Value, String> {
    let _ = args;
    let mut m = IndexMap::new();
    let table: &[(&str, u16)] = &[
        ("CONTINUE", 100), ("SWITCHING_PROTOCOLS", 101), ("PROCESSING", 102),
        ("OK", 200), ("CREATED", 201), ("ACCEPTED", 202),
        ("NON_AUTHORITATIVE_INFORMATION", 203), ("NO_CONTENT", 204),
        ("RESET_CONTENT", 205), ("PARTIAL_CONTENT", 206), ("MULTI_STATUS", 207),
        ("MULTIPLE_CHOICES", 300), ("MOVED_PERMANENTLY", 301), ("FOUND", 302),
        ("SEE_OTHER", 303), ("NOT_MODIFIED", 304), ("USE_PROXY", 305),
        ("TEMPORARY_REDIRECT", 307), ("PERMANENT_REDIRECT", 308),
        ("BAD_REQUEST", 400), ("UNAUTHORIZED", 401), ("PAYMENT_REQUIRED", 402),
        ("FORBIDDEN", 403), ("NOT_FOUND", 404), ("METHOD_NOT_ALLOWED", 405),
        ("NOT_ACCEPTABLE", 406), ("PROXY_AUTHENTICATION_REQUIRED", 407),
        ("REQUEST_TIMEOUT", 408), ("CONFLICT", 409), ("GONE", 410),
        ("LENGTH_REQUIRED", 411), ("PRECONDITION_FAILED", 412),
        ("REQUEST_ENTITY_TOO_LARGE", 413), ("REQUEST_URI_TOO_LONG", 414),
        ("UNSUPPORTED_MEDIA_TYPE", 415), ("REQUESTED_RANGE_NOT_SATISFIABLE", 416),
        ("EXPECTATION_FAILED", 417), ("IM_A_TEAPOT", 418),
        ("UNPROCESSABLE_ENTITY", 422), ("LOCKED", 423), ("FAILED_DEPENDENCY", 424),
        ("UPGRADE_REQUIRED", 426), ("PRECONDITION_REQUIRED", 428),
        ("TOO_MANY_REQUESTS", 429), ("REQUEST_HEADER_FIELDS_TOO_LARGE", 431),
        ("INTERNAL_SERVER_ERROR", 500), ("NOT_IMPLEMENTED", 501), ("BAD_GATEWAY", 502),
        ("SERVICE_UNAVAILABLE", 503), ("GATEWAY_TIMEOUT", 504),
        ("HTTP_VERSION_NOT_SUPPORTED", 505), ("VARIANT_ALSO_NEGOTIATES", 506),
        ("INSUFFICIENT_STORAGE", 507), ("LOOP_DETECTED", 508),
        ("NOT_EXTENDED", 510), ("NETWORK_AUTHENTICATION_REQUIRED", 511),
    ];
    for (name, code) in table {
        m.insert(name.to_string(), Value::Number(*code as f64));
        m.insert(code.to_string(), Value::String(name.to_string()));
    }
    Ok(Value::Dict(Arc::new(m)))
}

// exceptions (throw a categorized Zen error string)

pub fn requests_exc(args: &[Value]) -> Result<Value, String> {
    let name = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => "RequestException".to_string(),
    };
    let empty = IndexMap::new();
    let opts = match args.get(1) {
        Some(Value::Dict(d)) => d,
        _ => &empty,
    };
    let msg = get_str(opts, "msg").unwrap_or_default();
    Err(format!("requests.exceptions.{}: {}", name, msg))
}

pub fn requests_exc_http(args: &[Value]) -> Result<Value, String> {
    let empty = IndexMap::new();
    let opts = match args.first() {
        Some(Value::Dict(d)) => d,
        _ => &empty,
    };
    let msg = get_str(opts, "msg").unwrap_or_default();
    Err(format!("requests.exceptions.HTTPError: {}", msg))
}

// utils

pub fn requests_utils_quote(args: &[Value]) -> Result<Value, String> {
    let s = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("requests.utils.quote expects a string".into()),
    };
    Ok(Value::String(percent_encode(&s)))
}

pub fn requests_utils_unquote(args: &[Value]) -> Result<Value, String> {
    let s = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("requests.utils.unquote expects a string".into()),
    };
    Ok(Value::String(percent_decode(&s)))
}

pub fn requests_utils_ua(args: &[Value]) -> Result<Value, String> {
    let _ = args;
    Ok(Value::String(
        "python-requests/2.31.0 (Zen)".to_string(),
    ))
}

pub fn requests_utils_encode(args: &[Value]) -> Result<Value, String> {
    let _ = args;
    Ok(Value::String("utf-8".to_string()))
}
