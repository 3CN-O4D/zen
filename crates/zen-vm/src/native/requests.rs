//! `requests` — Python `requests`-parity HTTP client (Tier 4).
//!
//! A blocking HTTP/1.1 client over the OS socket layer (Tier 1) with `rustls`
//! for `https`. Sessions keep a cookie jar plus default headers, auth,
//! timeout and verify; every response is a plain dict:
//! `{url, method, status_code, reason, headers, text, content, encoding, ok,
//! elapsed, is_redirect, history, cookies}`.
//!
//! Surface: `request/get/post/put/patch/delete/head/options`, `Session`,
//! `session.request/close/headers/cookies`, `json`, `raise_for_status`,
//! `codes`, `utils.quote/unquote/default_user_agent`. Options are one dict:
//! `{params, data, json, headers, timeout, allow_redirects, auth, verify,
//! cookies}`.

#![allow(unsafe_code)]

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::gc::{new_dict, new_list};
use crate::native::{func, host_module_mixed};
use crate::ops::dict_get;
use crate::value::{DictKey, Value};
use indexmap::IndexMap;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, TrustAnchor, UnixTime};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, RootCertStore, SignatureScheme, StreamOwned};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT: f64 = 30.0;
const MAX_REDIRECTS: u32 = 30;
const UA: &str = "python-requests/2.32.3 (zen)";

// ── session registry ─────────────────────────────────────────────────────────

#[derive(Clone)]
struct Cookie {
    name: String,
    value: String,
    domain: Option<String>,
    path: String,
    // Kept for Python parity; not yet enforced during matching/sending.
    #[allow(dead_code)]
    secure: bool,
    #[allow(dead_code)]
    http_only: bool,
}

#[derive(Clone, Default)]
struct SessionState {
    headers: IndexMap<String, String>,
    auth: Option<(String, String)>,
    timeout: Option<f64>,
    verify: bool,
    jar: HashMap<String, Vec<Cookie>>,
}

fn sessions() -> &'static Mutex<HashMap<u64, SessionState>> {
    static REG: OnceLock<Mutex<HashMap<u64, SessionState>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

// ── percent coding, query and URL merging ────────────────────────────────────

const HEX: &[u8; 16] = b"0123456789ABCDEF";

fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        let keep = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if keep {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0xf) as usize] as char);
        }
    }
    out
}

fn decode_component(s: &str) -> String {
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
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn scalar_string(v: Value) -> String {
    if let Some(s) = v.as_str() {
        s.to_string()
    } else if let Some(n) = v.as_int() {
        n.to_string()
    } else if let Some(f) = v.as_float() {
        let t = format!("{f}");
        if t.ends_with(".0") {
            t[..t.len() - 2].to_string()
        } else {
            t
        }
    } else if let Some(b) = v.as_bool() {
        b.to_string()
    } else {
        String::new()
    }
}

fn build_query(params: &IndexMap<DictKey, Value>) -> VmResult<String> {
    let mut parts: Vec<String> = Vec::new();
    for (k, v) in params {
        if let Some(list) = v.as_list() {
            for item in list {
                parts.push(format!(
                    "{}={}",
                    encode_component(&scalar_string(k.0)),
                    encode_component(&scalar_string(*item))
                ));
            }
        } else if !v.is_nil() {
            parts.push(format!(
                "{}={}",
                encode_component(&scalar_string(k.0)),
                encode_component(&scalar_string(*v))
            ));
        }
    }
    Ok(parts.join("&"))
}

/// RFC3986 redirect merging: absolute URL passes through; `//host` keeps the
/// scheme; `/root-relative` is dropped onto the authority; a plain path is
/// merged against the base directory.
fn resolve_url(base: &str, loc: &str) -> String {
    let b = base.trim();
    let l = loc.trim();
    if l.starts_with("http://") || l.starts_with("https://") {
        return l.to_string();
    }
    let (scheme, rest) = match b.split_once("://") {
        Some((s, r)) => (s, r),
        None => return l.to_string(),
    };
    if let Some(after) = l.strip_prefix("//") {
        return format!("{scheme}://{after}");
    }
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    if let Some(path) = l.strip_prefix('/') {
        return format!("{scheme}://{authority}/{path}");
    }
    let base_path = rest.split(['?', '#']).next().unwrap_or("");
    let dir = match base_path.rfind('/') {
        Some(i) => &base_path[..=i],
        None => "/",
    };
    let merged = if l.is_empty() {
        base_path.to_string()
    } else {
        format!("{dir}{}", l.trim_start_matches('/'))
    };
    format!("{scheme}://{authority}{merged}")
}

// ── TLS ──────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct NoVerifier;

impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

fn root_store() -> RootCertStore {
    let mut store = RootCertStore::empty();
    store.extend(
        webpki_roots::TLS_SERVER_ROOTS
            .iter()
            .map(|ta| TrustAnchor::try_from(ta.clone()).unwrap()),
    );
    store
}

enum Stream {
    Plain(TcpStream),
    Tls(StreamOwned<ClientConnection, TcpStream>),
}

impl Stream {
    fn connect(host: &str, port: u16, tls: bool, verify: bool, timeout: f64) -> VmResult<Stream> {
        let t = Duration::from_secs_f64(timeout.max(0.001));
        let addrs: Vec<SocketAddr> = (host, port)
            .to_socket_addrs()
            .map_err(|e| VmError::NativeError(format!("{host}:{port}: {e}")))?
            .collect();
        let mut last = None;
        for addr in addrs {
            match TcpStream::connect_timeout(&addr, t) {
                Ok(s) => {
                    s.set_nodelay(true).ok();
                    let _ = s.set_read_timeout(Some(t));
                    let _ = s.set_write_timeout(Some(t));
                    if !tls {
                        return Ok(Stream::Plain(s));
                    }
                    let config = if verify {
                        ClientConfig::builder()
                            .with_root_certificates(root_store())
                            .with_no_client_auth()
                    } else {
                        ClientConfig::builder()
                            .dangerous()
                            .with_custom_certificate_verifier(Arc::new(NoVerifier))
                            .with_no_client_auth()
                    };
                    let name = ServerName::try_from(host.to_string()).map_err(|_| {
                        VmError::ValueError(format!("invalid TLS server name: {host}"))
                    })?;
                    let conn = ClientConnection::new(Arc::new(config), name)
                        .map_err(|e| VmError::NativeError(format!("tls {host}: {e}")))?;
                    return Ok(Stream::Tls(StreamOwned::new(conn, s)));
                }
                Err(e) => last = Some(e),
            }
        }
        Err(VmError::NativeError(format!(
            "connect {host}:{port}: {}",
            last.map(|e| e.to_string()).unwrap_or_default()
        )))
    }

    fn read(&mut self, buf: &mut [u8]) -> VmResult<usize> {
        match self {
            Stream::Plain(s) => s
                .read(buf)
                .map_err(|e| VmError::NativeError(format!("read: {e}"))),
            Stream::Tls(s) => s
                .read(buf)
                .map_err(|e| VmError::NativeError(format!("read: {e}"))),
        }
    }

    fn write_all(&mut self, data: &[u8]) -> VmResult<()> {
        match self {
            Stream::Plain(s) => s
                .write_all(data)
                .map_err(|e| VmError::NativeError(format!("write: {e}"))),
            Stream::Tls(s) => s
                .write_all(data)
                .map_err(|e| VmError::NativeError(format!("write: {e}"))),
        }
    }

    /// Read a CRLF- or LF-terminated line (headers, status, chunk size).
    fn read_line(&mut self) -> VmResult<Vec<u8>> {
        let mut out = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            let n = self.read(&mut byte)?;
            if n == 0 {
                break;
            }
            out.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
            if out.len() > 64 * 1024 {
                return Err(VmError::NativeError("requests: header line too long".into()));
            }
        }
        Ok(out)
    }
}

// ── URL parsing ──────────────────────────────────────────────────────────────

struct ParsedUrl {
    scheme: String,
    host: String,
    port: u16,
    path: String,
}

fn parse_url(url: &str) -> VmResult<ParsedUrl> {
    let (scheme, rest) = match url.split_once("://") {
        Some((s, r)) if s == "http" || s == "https" => (s.to_string(), r),
        Some(_) => {
            return Err(VmError::ValueError(format!(
                "requests: unsupported scheme in {url}"
            )))
        }
        None => {
            return Err(VmError::ValueError(format!(
                "requests: URL must carry a scheme: {url}"
            )))
        }
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let (host, port) = split_authority(authority)?;
    if host.is_empty() {
        return Err(VmError::ValueError(format!("requests: empty host in {url}")));
    }
    let port = match port {
        Some(p) => p.parse::<u16>().map_err(|_| {
            VmError::ValueError(format!("requests: bad port in {url}: {p}"))
        })?,
        None if scheme == "https" => 443,
        None => 80,
    };
    let path = rest[authority_end..].split('#').next().unwrap_or("");
    let path = if path.is_empty() { "/".to_string() } else { path.to_string() };
    Ok(ParsedUrl { scheme, host, port, path })
}

fn split_authority(authority: &str) -> VmResult<(String, Option<&str>)> {
    if let Some(after) = authority.strip_prefix('[') {
        // IPv6 literal [::1]:port
        let (host, port) = match after.split_once(']') {
            Some((h, rest)) => (h.to_string(), rest.strip_prefix(':')),
            None => (after.to_string(), None),
        };
        return Ok((host, port));
    }
    match authority.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => {
            Ok((h.to_string(), Some(p)))
        }
        _ => Ok((authority.to_string(), None)),
    }
}

// ── options ──────────────────────────────────────────────────────────────────

struct Opts {
    params: Option<IndexMap<DictKey, Value>>,
    body: Vec<u8>,
    headers: IndexMap<String, String>,
    timeout: f64,
    allow_redirects: bool,
    auth: Option<(String, String)>,
    verify: bool,
}

fn dict_num(m: &IndexMap<DictKey, Value>, key: &str) -> Option<f64> {
    let v = dict_get(m, Value::string(key))?;
    v.as_float().or_else(|| v.as_int().map(|i| i as f64))
}

fn dict_bool(m: &IndexMap<DictKey, Value>, key: &str, default: bool) -> bool {
    dict_get(m, Value::string(key))
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

fn make_body(m: &IndexMap<DictKey, Value>, headers: &mut IndexMap<String, String>) -> VmResult<Vec<u8>> {
    if let Some(json) = dict_get(m, Value::string("json")) {
        if !json.is_nil() {
            let text = crate::native::json_stringify(&[json])?
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| VmError::ValueError("requests: json body must be serializable".into()))?;
            headers
                .entry("Content-Type".to_string())
                .or_insert_with(|| "application/json".to_string());
            return Ok(text.into_bytes());
        }
    }
    if let Some(data) = dict_get(m, Value::string("data")) {
        if let Some(map) = data.as_dict() {
            let q = build_query(map)?;
            headers
                .entry("Content-Type".to_string())
                .or_insert_with(|| "application/x-www-form-urlencoded".to_string());
            return Ok(q.into_bytes());
        }
        if let Some(list) = data.as_list() {
            let mut out = Vec::new();
            let mut first = true;
            for p in list {
                if let Some(pm) = p.as_dict() {
                    if !first {
                        out.extend_from_slice(b"&");
                    }
                    first = false;
                    out.extend_from_slice(build_query(pm)?.as_bytes());
                }
            }
            return Ok(out);
        }
        if let Some(b) = data.as_bytes() {
            return Ok(b.to_vec());
        }
        if let Some(s) = data.as_str() {
            return Ok(s.as_bytes().to_vec());
        }
        if !data.is_nil() {
            return Err(VmError::type_mismatch("str, bytes, list or dict for data", data));
        }
    }
    Ok(Vec::new())
}

fn opts_from(args: &[Value], idx: usize, sess: Option<&SessionState>) -> VmResult<Opts> {
    let m: IndexMap<DictKey, Value> = args
        .get(idx)
        .and_then(|v| v.as_dict())
        .cloned()
        .unwrap_or_default();

    let mut headers: IndexMap<String, String> = sess
        .map(|s| s.headers.clone())
        .unwrap_or_default();
    if let Some(h) = dict_get(&m, Value::string("headers")).and_then(|v| v.as_dict().cloned()) {
        for (k, v) in h {
            if let Some(val) = v.as_str() {
                headers.insert(scalar_string(k.0), val.to_string());
            }
        }
    }

    let mut auth = sess.and_then(|s| s.auth.clone());
    if let Some(a) = dict_get(&m, Value::string("auth")).and_then(|v| v.as_list().map(<[Value]>::to_vec)) {
        auth = if a.len() == 2 {
            Some((
                scalar_string(*a.get(0).unwrap_or(&Value::nil())),
                scalar_string(*a.get(1).unwrap_or(&Value::nil())),
            ))
        } else {
            None
        };
    }

    let body = make_body(&m, &mut headers)?;
    let timeout = dict_num(&m, "timeout")
        .or_else(|| sess.and_then(|s| s.timeout))
        .unwrap_or(DEFAULT_TIMEOUT)
        .max(0.0);
    let allow_redirects = dict_bool(&m, "allow_redirects", true);
    let verify = dict_bool(&m, "verify", sess.map(|s| s.verify).unwrap_or(true));
    let params = dict_get(&m, Value::string("params"))
        .and_then(|v| v.as_dict().cloned());

    Ok(Opts {
        params,
        body,
        headers,
        timeout,
        allow_redirects,
        auth,
        verify,
    })
}

// ── HTTP/1.1 exchange ────────────────────────────────────────────────────────

struct HttpResponse {
    status: u16,
    reason: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn header_value(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
}

fn chunked_read(s: &mut Stream) -> VmResult<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line = s.read_line()?;
        let text = String::from_utf8_lossy(&line).trim().to_string();
        let size_str = text.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_str, 16)
            .map_err(|_| VmError::NativeError(format!("requests: bad chunk size '{text}'")))?;
        if size == 0 {
            loop {
                let l = s.read_line()?;
                if l.len() <= 2 {
                    break;
                }
            }
            break;
        }
        let mut got = 0usize;
        let mut buf = vec![0u8; 8192];
        while got < size {
            let want = (size - got).min(buf.len());
            match s.read(&mut buf[..want])? {
                0 => break,
                n => {
                    out.extend_from_slice(&buf[..n]);
                    got += n;
                }
            }
        }
        let mut crlf = [0u8; 2];
        let _ = s.read(&mut crlf);
    }
    Ok(out)
}

fn read_body(s: &mut Stream, headers: &[(String, String)], method: &str) -> VmResult<Vec<u8>> {
    let mut out = Vec::new();
    if method.eq_ignore_ascii_case("HEAD") {
        return Ok(out);
    }
    if let Some(te) = header_value(headers, "transfer-encoding") {
        if te.eq_ignore_ascii_case("chunked") {
            return chunked_read(s);
        }
    }
    if let Some(cl) = header_value(headers, "content-length") {
        let len: usize = cl.trim().parse().unwrap_or(0);
        let mut buf = vec![0u8; 8192];
        let mut got = 0usize;
        while got < len {
            let want = (len - got).min(buf.len());
            match s.read(&mut buf[..want])? {
                0 => break,
                n => {
                    out.extend_from_slice(&buf[..n]);
                    got += n;
                }
            }
        }
        return Ok(out);
    }
    let mut buf = [0u8; 8192];
    loop {
        match s.read(&mut buf)? {
            0 => break,
            n => out.extend_from_slice(&buf[..n]),
        }
    }
    Ok(out)
}

fn send_and_receive(
    method: &str,
    url: &str,
    opts: &Opts,
    cookie_header: &str,
) -> VmResult<(HttpResponse, f64)> {
    let pu = parse_url(url)?;
    let mut path = pu.path.clone();
    if let Some(params) = &opts.params {
        let q = build_query(params)?;
        if !q.is_empty() {
            let sep = if path.contains('?') { '&' } else { '?' };
            path = format!("{path}{sep}{q}");
        }
    }
    let tls = pu.scheme == "https";
    let mut s = Stream::connect(&pu.host, pu.port, tls, opts.verify, opts.timeout)?;

    let display_host = if pu.host.contains(':') {
        format!("[{}]", pu.host)
    } else {
        pu.host.clone()
    };
    let authority =
        if (pu.port == 80 && pu.scheme == "http") || (pu.port == 443 && pu.scheme == "https") {
            display_host
        } else {
            format!("{display_host}:{}", pu.port)
        };
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nHost: {authority}\r\nUser-Agent: {UA}\r\nAccept: */*\r\nAccept-Encoding: identity\r\nConnection: close\r\n"
    );
    if let Some((u, p)) = &opts.auth {
        use base64::Engine;
        let token = base64::engine::general_purpose::STANDARD.encode(format!("{u}:{p}"));
        head.push_str(&format!("Authorization: Basic {token}\r\n"));
    }
    for (k, v) in &opts.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    if !cookie_header.is_empty() {
        head.push_str(&format!("Cookie: {cookie_header}\r\n"));
    }
    if !opts.body.is_empty() {
        head.push_str(&format!("Content-Length: {}\r\n", opts.body.len()));
    }
    head.push_str("\r\n");

    let started = Instant::now();
    s.write_all(head.as_bytes())?;
    if !opts.body.is_empty() {
        s.write_all(&opts.body)?;
    }

    let status_buf = s.read_line()?;
    let status_line = String::from_utf8_lossy(&status_buf).trim_end().to_string();
    if !status_line.starts_with("HTTP/") {
        return Err(VmError::NativeError(format!(
            "requests: malformed response line from {}: {status_line:?}",
            pu.host
        )));
    }
    let mut parts = status_line.splitn(3, ' ');
    let _version = parts.next();
    let status: u16 = parts
        .next()
        .and_then(|p| p.trim().parse().ok())
        .ok_or_else(|| VmError::NativeError(format!("requests: bad status in {status_line}")))?;
    let reason = parts.next().unwrap_or("").to_string();

    let mut headers: Vec<(String, String)> = Vec::new();
    loop {
        let line = s.read_line()?;
        if line.len() <= 2 {
            break;
        }
        let tr = String::from_utf8_lossy(&line);
        let tr = tr.trim_end();
        if let Some((k, v)) = tr.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let body = read_body(&mut s, &headers, method)?;
    let elapsed = started.elapsed().as_secs_f64();
    Ok((HttpResponse { status, reason, headers, body }, elapsed))
}

// ── cookies ──────────────────────────────────────────────────────────────────

fn domain_matches(cookie: &str, host: &str) -> bool {
    cookie.is_empty() || cookie == host || host.ends_with(&format!(".{cookie}"))
}

fn path_matches(cookie_path: &str, url_path: &str) -> bool {
    cookie_path.is_empty()
        || cookie_path == "/"
        || url_path == cookie_path
        || url_path.starts_with(&format!("{cookie_path}/"))
}

fn parse_cookie(header: &str) -> Option<Cookie> {
    let mut parts = header.split(';');
    let first = parts.next()?.trim();
    let (name, value) = first.split_once('=')?;
    let mut domain = None;
    let mut path = String::new();
    let mut secure = false;
    let mut http_only = false;
    for part in parts {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            match k.trim().to_ascii_lowercase().as_str() {
                "domain" => domain = Some(v.trim().trim_start_matches('.').to_string()),
                "path" => path = v.trim().to_string(),
                _ => {}
            }
        } else if part.eq_ignore_ascii_case("secure") {
            secure = true;
        } else if part.eq_ignore_ascii_case("httponly") {
            http_only = true;
        }
    }
    Some(Cookie {
        name: name.trim().to_string(),
        value: value.trim().to_string(),
        domain,
        path,
        secure,
        http_only,
    })
}

fn cookie_header(jar: &HashMap<String, Vec<Cookie>>, host: &str, path: &str) -> String {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for (domain, list) in jar {
        if domain_matches(domain, host) {
            for c in list {
                if path_matches(&c.path, path) {
                    pairs.push((c.name.clone(), c.value.clone()));
                }
            }
        }
    }
    if pairs.is_empty() {
        String::new()
    } else {
        pairs
            .iter()
            .map(|(n, v)| format!("{n}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

fn absorb_set_cookie(jar: &mut HashMap<String, Vec<Cookie>>, host: &str, headers: &[(String, String)]) {
    let mut inserts: Vec<Cookie> = Vec::new();
    for (k, v) in headers {
        if k.eq_ignore_ascii_case("set-cookie") {
            if let Some(c) = parse_cookie(v) {
                inserts.push(c);
            }
        }
    }
    for c in inserts {
        let dom = c.domain.clone().unwrap_or_else(|| host.to_string());
        jar.entry(dom).or_default().push(c);
    }
}

fn response_cookies(jar: &HashMap<String, Vec<Cookie>>, host: &str, path: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (domain, list) in jar {
        if domain_matches(domain, host) {
            for c in list {
                if path_matches(&c.path, path) {
                    out.push((c.name.clone(), c.value.clone()));
                }
            }
        }
    }
    out
}

// ── response value ───────────────────────────────────────────────────────────

fn charset_of(headers: &[(String, String)]) -> String {
    let ct = header_value(headers, "content-type").unwrap_or_default();
    for part in ct.split(';') {
        if let Some(v) = part.trim().strip_prefix("charset=") {
            return v.trim().trim_matches('"').to_string();
        }
    }
    if ct.contains("text/") {
        "iso-8859-1".to_string()
    } else {
        "utf-8".to_string()
    }
}

fn decode_text(body: &[u8], charset: &str) -> String {
    let cs = charset.to_ascii_lowercase();
    if cs.contains("utf-8") || cs.is_empty() {
        String::from_utf8_lossy(body).into_owned()
    } else if cs.contains("iso-8859-1") || cs.contains("latin-1") || cs.contains("windows-1252") {
        body.iter().map(|&b| b as char).collect()
    } else {
        String::from_utf8_lossy(body).into_owned()
    }
}

#[allow(clippy::too_many_arguments)]
fn response_value(
    resp: &HttpResponse,
    url: &str,
    method: &str,
    elapsed: f64,
    history: Vec<Value>,
    cookies: Vec<(String, String)>,
) -> VmResult<Value> {
    let mut headers = IndexMap::new();
    for (k, v) in &resp.headers {
        let key = Value::string(k.to_ascii_lowercase());
        let entry = headers.entry(DictKey(key)).or_insert_with(|| Value::string(String::new()));
        if let Some(old) = entry.as_str() {
            let joined = if old.is_empty() {
                v.clone()
            } else {
                format!("{old}, {v}")
            };
            *entry = Value::string(joined);
        }
    }
    let status = resp.status;
    let encoding = charset_of(&resp.headers);
    let text = decode_text(&resp.body, &encoding);
    let mut cookies_map = IndexMap::new();
    for (name, value) in cookies {
        cookies_map.insert(DictKey(Value::string(name)), Value::string(value));
    }
    let is_redirect = (300..400).contains(&status);
    let mut d = IndexMap::new();
    d.insert(DictKey(Value::string("url")), Value::string(url.to_string()));
    d.insert(DictKey(Value::string("method")), Value::string(method.to_string()));
    d.insert(DictKey(Value::string("status_code")), Value::int_promote(status as i64));
    d.insert(DictKey(Value::string("reason")), Value::string(resp.reason.clone()));
    d.insert(DictKey(Value::string("headers")), new_dict(headers));
    d.insert(DictKey(Value::string("content")), Value::bytes(resp.body.clone()));
    d.insert(DictKey(Value::string("text")), Value::string(text));
    d.insert(DictKey(Value::string("encoding")), Value::string(encoding));
    d.insert(DictKey(Value::string("ok")), Value::bool(status < 400));
    d.insert(DictKey(Value::string("elapsed")), Value::float(elapsed));
    d.insert(DictKey(Value::string("is_redirect")), Value::bool(is_redirect));
    d.insert(DictKey(Value::string("history")), new_list(history));
    d.insert(DictKey(Value::string("cookies")), new_dict(cookies_map));
    Ok(new_dict(d))
}

// ── request driver (redirects + session jar) ─────────────────────────────────

fn do_request(
    method: &str,
    url: &str,
    opts: &Opts,
    jar: &mut HashMap<String, Vec<Cookie>>,
) -> VmResult<Value> {
    let mut current = url.to_string();
    let mut cur_method = method.to_string();
    let mut cur_body = opts.body.clone();
    let mut cur_params = opts.params.clone();
    let mut history: Vec<Value> = Vec::new();

    for _ in 0..=MAX_REDIRECTS {
        let pu = parse_url(&current)?;
        let cookie = cookie_header(jar, &pu.host, &pu.path);
        let hop = Opts {
            params: cur_params.clone(),
            body: cur_body.clone(),
            headers: opts.headers.clone(),
            timeout: opts.timeout,
            allow_redirects: opts.allow_redirects,
            auth: opts.auth.clone(),
            verify: opts.verify,
        };
        let (resp, elapsed) = send_and_receive(&cur_method, &current, &hop, &cookie)?;
        let pu = parse_url(&current)?;
        absorb_set_cookie(jar, &pu.host, &resp.headers);

        let wants_redirect = (300..400).contains(&resp.status)
            && opts.allow_redirects
            && header_value(&resp.headers, "location").is_some();
        if !wants_redirect {
            let final_cookies = response_cookies(jar, &pu.host, &pu.path);
            return response_value(
                &resp,
                &current,
                method,
                elapsed,
                history,
                final_cookies,
            );
        }

        let loc = header_value(&resp.headers, "location").unwrap_or_default();
        history.push(response_value(
            &resp, &current, &cur_method, elapsed, Vec::new(), Vec::new(),
        )?);
        cur_params = None;
        match resp.status {
            303 => {
                cur_method = "GET".to_string();
                cur_body = Vec::new();
            }
            301 | 302 if cur_method.eq_ignore_ascii_case("POST") => {
                cur_method = "GET".to_string();
                cur_body = Vec::new();
            }
            _ => {}
        }
        current = resolve_url(&current, &loc);
    }
    Err(VmError::NativeError("requests: too many redirects".to_string()))
}

// ── native entry points ──────────────────────────────────────────────────────

fn request(args: &[Value]) -> VmResult<Value> {
    let method = crate::ops::string_arg(args, 0)?;
    let url = crate::ops::string_arg(args, 1)?;
    let opts = opts_from(args, 2, None)?;
    let mut jar = HashMap::new();
    do_request(&method, &url, &opts, &mut jar)
}

fn verb(args: &[Value], method: &str) -> VmResult<Value> {
    let url = crate::ops::string_arg(args, 0)?;
    let opts = opts_from(args, 1, None)?;
    let mut jar = HashMap::new();
    do_request(method, &url, &opts, &mut jar)
}

fn session_new(_args: &[Value]) -> VmResult<Value> {
    let mut reg = sessions().lock().unwrap_or_else(|e| e.into_inner());
    let mut id = 1u64;
    while reg.contains_key(&id) {
        id += 1;
    }
    reg.insert(id, SessionState::default());
    Ok(Value::int_promote(id as i64))
}

fn session_id(args: &[Value]) -> VmResult<u64> {
    let id = crate::ops::int_arg(args, 0)?;
    if id <= 0 || id > i64::from(u32::MAX) {
        return Err(VmError::HandleError(format!("bad session id {id}")));
    }
    Ok(id as u64)
}

fn session_request(args: &[Value]) -> VmResult<Value> {
    let sid = session_id(args)?;
    let method = crate::ops::string_arg(args, 1)?;
    let url = crate::ops::string_arg(args, 2)?;
    let (snapshot, opts) = {
        let reg = sessions().lock().unwrap_or_else(|e| e.into_inner());
        let state = reg
            .get(&sid)
            .cloned()
            .ok_or_else(|| VmError::HandleError(format!("no such session {sid}")))?;
        let opts = opts_from(args, 3, Some(&state))?;
        (state.jar, opts)
    };
    let mut jar = snapshot;
    let result = do_request(&method, &url, &opts, &mut jar)?;
    {
        let mut reg = sessions().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = reg.get_mut(&sid) {
            state.jar = jar;
        }
    }
    Ok(result)
}

fn session_verb(args: &[Value], method: &str) -> VmResult<Value> {
    let sid = session_id(args)?;
    let url = crate::ops::string_arg(args, 1)?;
    let (snapshot, opts) = {
        let reg = sessions().lock().unwrap_or_else(|e| e.into_inner());
        let state = reg
            .get(&sid)
            .cloned()
            .ok_or_else(|| VmError::HandleError(format!("no such session {sid}")))?;
        let opts = opts_from(args, 2, Some(&state))?;
        (state.jar, opts)
    };
    let mut jar = snapshot;
    let result = do_request(method, &url, &opts, &mut jar)?;
    {
        let mut reg = sessions().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = reg.get_mut(&sid) {
            state.jar = jar;
        }
    }
    Ok(result)
}

fn session_close(args: &[Value]) -> VmResult<Value> {
    let sid = session_id(args)?;
    let mut reg = sessions().lock().unwrap_or_else(|e| e.into_inner());
    if reg.remove(&sid).is_none() {
        return Err(VmError::HandleError(format!("no such session {sid}")));
    }
    Ok(Value::nil())
}

fn session_headers(args: &[Value]) -> VmResult<Value> {
    let sid = session_id(args)?;
    let mut reg = sessions().lock().unwrap_or_else(|e| e.into_inner());
    let state = reg
        .get_mut(&sid)
        .ok_or_else(|| VmError::HandleError(format!("no such session {sid}")))?;
    if let Some(m) = args.get(1).and_then(|v| v.as_dict()) {
        state.headers.clear();
        for (k, v) in m {
            if let Some(val) = v.as_str() {
                state.headers.insert(scalar_string(k.0), val.to_string());
            }
        }
        return Ok(Value::nil());
    }
    let mut out = IndexMap::new();
    for (k, v) in &state.headers {
        out.insert(DictKey(Value::string(k.clone())), Value::string(v.clone()));
    }
    Ok(new_dict(out))
}

fn session_cookies(args: &[Value]) -> VmResult<Value> {
    let sid = session_id(args)?;
    let mut reg = sessions().lock().unwrap_or_else(|e| e.into_inner());
    let state = reg
        .get_mut(&sid)
        .ok_or_else(|| VmError::HandleError(format!("no such session {sid}")))?;
    if let Some(m) = args.get(1).and_then(|v| v.as_dict()) {
        state.jar.clear();
        for (k, v) in m {
            if let Some(val) = v.as_str() {
                state.jar.entry(String::new()).or_default().push(Cookie {
                    name: scalar_string(k.0),
                    value: val.to_string(),
                    domain: None,
                    path: String::new(),
                    secure: false,
                    http_only: false,
                });
            }
        }
        return Ok(Value::nil());
    }
    // Flat projection, most recent cookie per name (Python jar access).
    let mut out = IndexMap::new();
    for list in state.jar.values() {
        for c in list {
            out.insert(DictKey(Value::string(c.name.clone())), Value::string(c.value.clone()));
        }
    }
    Ok(new_dict(out))
}

fn resp_json(args: &[Value]) -> VmResult<Value> {
    let resp = *args.first().ok_or(VmError::ArgumentCount { expected: 1, got: 0 })?;
    let d = resp
        .as_dict()
        .ok_or_else(|| VmError::type_mismatch("response dict", resp))?;
    let text = dict_get(d, Value::string("text"))
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    crate::native::json_parse(&[Value::string(text)])
}

fn raise_for_status(args: &[Value]) -> VmResult<Value> {
    let resp = *args.first().ok_or(VmError::ArgumentCount { expected: 1, got: 0 })?;
    let d = resp
        .as_dict()
        .ok_or_else(|| VmError::type_mismatch("response dict", resp))?;
    let status = dict_get(d, Value::string("status_code"))
        .and_then(Value::as_int)
        .unwrap_or(0);
    let reason = dict_get(d, Value::string("reason"))
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    if (400..600).contains(&status) {
        return Err(VmError::NativeError(format!(
            "HTTP {status} {reason} (requests.raise_for_status)"
        )));
    }
    Ok(Value::nil())
}

fn codes(_args: &[Value]) -> VmResult<Value> {
    let names: &[(&str, u16)] = &[
        ("continuation", 100),
        ("ok", 200),
        ("created", 201),
        ("accepted", 202),
        ("no_content", 204),
        ("reset_content", 205),
        ("partial_content", 206),
        ("multiple_choices", 300),
        ("moved_permanently", 301),
        ("found", 302),
        ("see_other", 303),
        ("not_modified", 304),
        ("temporary_redirect", 307),
        ("permanent_redirect", 308),
        ("bad_request", 400),
        ("unauthorized", 401),
        ("forbidden", 403),
        ("not_found", 404),
        ("method_not_allowed", 405),
        ("not_acceptable", 406),
        ("conflict", 409),
        ("gone", 410),
        ("precondition_failed", 412),
        ("payload_too_large", 413),
        ("unsupported_media_type", 415),
        ("too_many_requests", 429),
        ("internal_server_error", 500),
        ("not_implemented", 501),
        ("bad_gateway", 502),
        ("service_unavailable", 503),
        ("gateway_timeout", 504),
    ];
    let mut d = IndexMap::new();
    for (name, code) in names {
        d.insert(DictKey(Value::string(name.to_string())), Value::int_promote(*code as i64));
    }
    Ok(new_dict(d))
}

fn utils_quote(args: &[Value]) -> VmResult<Value> {
    Ok(Value::string(encode_component(&crate::ops::string_arg(args, 0)?)))
}

fn utils_unquote(args: &[Value]) -> VmResult<Value> {
    Ok(Value::string(decode_component(&crate::ops::string_arg(args, 0)?)))
}

fn default_user_agent(_args: &[Value]) -> VmResult<Value> {
    Ok(Value::string(UA.to_string()))
}

// ── export table ─────────────────────────────────────────────────────────────

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        func("request", 700, 2, 3),
        func("get", 701, 1, 2),
        func("post", 702, 1, 2),
        func("put", 703, 1, 2),
        func("patch", 704, 1, 2),
        func("delete", 705, 1, 2),
        func("head", 706, 1, 2),
        func("options", 707, 1, 2),
        func("Session", 708, 0, 0),
        func("session_request", 709, 3, 4),
        func("session_close", 710, 1, 1),
        func("session_headers", 711, 1, 2),
        func("session_cookies", 712, 1, 2),
        func("json", 713, 1, 1),
        func("raise_for_status", 714, 1, 1),
        func("codes", 715, 0, 0),
        func("utils_quote", 716, 1, 1),
        func("utils_unquote", 717, 1, 1),
        func("default_user_agent", 718, 0, 0),
        func("sess_get", 719, 2, 3),
        func("sess_post", 720, 2, 3),
        func("sess_put", 721, 2, 3),
        func("sess_delete", 722, 2, 3),
        func("sess_patch", 723, 2, 3),
        func("sess_head", 724, 2, 3),
        func("sess_options", 725, 2, 3),
    ])
}

/// Route ids in the `requests` block (700–759) to their handler.
pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    match id {
        700 => request(args),
        701 => verb(args, "GET"),
        702 => verb(args, "POST"),
        703 => verb(args, "PUT"),
        704 => verb(args, "PATCH"),
        705 => verb(args, "DELETE"),
        706 => verb(args, "HEAD"),
        707 => verb(args, "OPTIONS"),
        708 => session_new(args),
        709 => session_request(args),
        710 => session_close(args),
        711 => session_headers(args),
        712 => session_cookies(args),
        713 => resp_json(args),
        714 => raise_for_status(args),
        715 => codes(args),
        716 => utils_quote(args),
        717 => utils_unquote(args),
        718 => default_user_agent(args),
        719 => session_verb(args, "GET"),
        720 => session_verb(args, "POST"),
        721 => session_verb(args, "PUT"),
        722 => session_verb(args, "DELETE"),
        723 => session_verb(args, "PATCH"),
        724 => session_verb(args, "HEAD"),
        725 => session_verb(args, "OPTIONS"),
        _ => Err(VmError::NativeError(format!(
            "unknown requests native id {id}"
        ))),
    }
}