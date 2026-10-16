//! Zen `tls` module — live TLS certificate fetching.
//!
//! ```python
//! import tls
//! pem = tls.fetch("example.com", 443)
//! cert = x509.parse_pem(pem)   # M1 inspection takes it from here
//! ```
//!
//! Only the leaf certificate is returned (as PEM armor around the peer's
//! DER), after a normal verifying handshake: `host` drives both SNI and
//! hostname verification. Pass `{insecure: true}` only for lab hosts with
//! self-signed certificates — it disables verification the way curl's `-k`
//! does, and the returned dict-equivalent flags nothing by itself.
//!
//! Only test systems you own or are authorized to audit.

use crate::runtime::{Value, Vm};
use base64::Engine;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

fn string_arg(args: &[Value], idx: usize, what: &str) -> Result<String, String> {
    match args.get(idx) {
        Some(Value::String(s)) => Ok(s.clone()),
        _ => Err(format!("tls.{what}: expected a string argument")),
    }
}

fn num_arg(args: &[Value], idx: usize, default: f64) -> f64 {
    match args.get(idx) {
        Some(Value::Number(n)) => *n,
        _ => default,
    }
}

fn opt_bool(args: &[Value], idx: usize, key: &str, default: bool) -> bool {
    match args.get(idx) {
        Some(Value::Dict(d)) => match d.get(key) {
            Some(Value::Bool(b)) => *b,
            _ => default,
        },
        _ => default,
    }
}

fn opt_num(args: &[Value], idx: usize, key: &str, default: f64) -> f64 {
    match args.get(idx) {
        Some(Value::Dict(d)) => match d.get(key) {
            Some(Value::Number(n)) => *n,
            _ => default,
        },
        _ => default,
    }
}

fn armor(der: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = String::from("-----BEGIN CERTIFICATE-----\n");
    let mut i = 0;
    while i < b64.len() {
        let j = (i + 64).min(b64.len());
        out.push_str(&b64[i..j]);
        out.push('\n');
        i = j;
    }
    out.push_str("-----END CERTIFICATE-----\n");
    out
}

/// Fetch the leaf certificate from `host:port` and return it as PEM text.
/// Throws a plain error string on DNS, TCP, timeout, or TLS failures.
pub fn tls_fetch(args: &[Value]) -> Result<Value, String> {
    let host = string_arg(args, 0, "fetch")?;
    let port = num_arg(args, 1, 443.0) as u16;
    let timeout_ms = opt_num(args, 2, "timeout_ms", 10_000.0).max(1.0);
    let insecure = opt_bool(args, 2, "insecure", false);
    let timeout = Duration::from_millis(timeout_ms as u64);

    let addr = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("tls.fetch: cannot resolve {host}:{port}: {e}"))?
        .next()
        .ok_or_else(|| format!("tls.fetch: cannot resolve {host}:{port}"))?;
    let stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|e| format!("tls.fetch: cannot connect to {host}:{port}: {e}"))?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));

    let connector = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(insecure)
        .build()
        .map_err(|e| format!("tls.fetch: TLS setup failed: {e}"))?;
    let stream = connector
        .connect(&host, stream)
        .map_err(|e| format!("tls.fetch: TLS handshake with {host}:{port} failed: {e}"))?;
    let cert = stream
        .peer_certificate()
        .map_err(|e| format!("tls.fetch: cannot read peer certificate: {e}"))?
        .ok_or_else(|| format!("tls.fetch: {host}:{port} presented no certificate"))?;
    let der = cert
        .to_der()
        .map_err(|e| format!("tls.fetch: cannot encode peer certificate: {e}"))?;
    Ok(Value::String(armor(&der)))
}

pub fn init_tls_module(vm: &mut Vm) {
    let tls = Value::Dict(Arc::new(indexmap::IndexMap::from([(
        "fetch".into(),
        Value::NativeFunction("tls_fetch".into()),
    )])));
    vm.vars.insert("tls".into(), tls);
}
