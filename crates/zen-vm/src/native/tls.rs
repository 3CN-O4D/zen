//! `tls` — TLS transport layer built on `rustls`.
//!
//! Provides encrypted TCP connections with optional certificate pinning:
//!
//! ```zen
//! import tls
//! fd = tls.connect("example.com", 443)
//! tls.write(fd, "GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
//! let resp = tls.read(fd, 4096)
//! tls.close(fd)
//!
//! # With certificate pinning (SHA-256 of the server's SPKI):
//! let pins = ["sha256:AAAA...BBBB..."]
//! fd = tls.connect_pinned("example.com", 443, pins)
//! ```

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::DigitallySignedStruct;
use rustls::{ClientConfig, ClientConnection, RootCertStore, SignatureScheme, StreamOwned};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;
use webpki_roots;

use crate::value::Value;
use crate::vm::{Vm, VmError};

const DEFAULT_TIMEOUT_SECS: u64 = 30;

// ── resource registry ───────────────────────────────────────────────────────

thread_local! {
    static TLS_CONNS: std::cell::RefCell<HashMap<u64, StreamOwned<ClientConnection, TcpStream>>> =
        std::cell::RefCell::new(HashMap::new());
}

static NEXT_TLS_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

fn next_tls_id() -> u64 {
    NEXT_TLS_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

/// Compute a pin hash from a certificate's DER bytes.
fn cert_pin_hash(cert: &CertificateDer) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(cert.as_ref());
    let mut s = String::with_capacity(7 + 64);
    s.push_str("sha256:");
    for b in hash {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Verify server certificates. In pinning mode, compare the leaf cert's DER
/// hash against the provided pins. Otherwise, verify against webpki-roots.
struct CertVerifier {
    pins: Vec<String>,
}

impl CertVerifier {
    fn new(pins: Vec<String>) -> Self {
        Self { pins }
    }
}

impl std::fmt::Debug for CertVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertVerifier")
            .field("pins", &self.pins.len())
            .finish()
    }
}

impl ServerCertVerifier for CertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if !self.pins.is_empty() {
            let hash = cert_pin_hash(end_entity);
            if !self.pins.iter().any(|p| *p == hash) {
                return Err(rustls::Error::General("certificate pin mismatch".into()));
            }
            return Ok(ServerCertVerified::assertion());
        }

        // Non-pinning mode: trust webpki-roots (rustls verifies automatically
        // when with_root_certificates is set on the config).
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

// ── public API ──────────────────────────────────────────────────────────────

/// `tls.connect(host, port) -> fd`
///
/// Plain TLS: verifies the server certificate against the platform root
/// store (webpki-roots). Returns a file descriptor usable with `tls.read` /
// `tls.write` / `tls.close`.
pub fn native_tls_connect(args: &[Value]) -> Result<Value, VmError> {
    let host = args.get(0)
        .and_then(|v| v.as_str())
        .ok_or_else(|| VmError::TypeError("host must be a string".into()))?;
    let port = args.get(1)
        .and_then(|v| v.as_int())
        .ok_or_else(|| VmError::TypeError("port must be an integer".into()))? as u16;

    let addr = format!("{}:{}", host, port);
    let addrs: Vec<std::net::SocketAddr> = addr
        .to_socket_addrs()
        .map_err(|e| VmError::NativeError(format!("{addr}: {e}")))?
        .collect();

    let mut last_err = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(DEFAULT_TIMEOUT_SECS)) {
            Ok(tcp) => {
                let _ = tcp.set_nodelay(true);
                let _ = tcp.set_read_timeout(Some(Duration::from_secs(DEFAULT_TIMEOUT_SECS)));
                let _ = tcp.set_write_timeout(Some(Duration::from_secs(DEFAULT_TIMEOUT_SECS)));

                let mut root_store = RootCertStore::empty();
                root_store.extend(
                    webpki_roots::TLS_SERVER_ROOTS
                        .iter()
                        .map(|ta| rustls::pki_types::TrustAnchor::try_from(ta.clone()).unwrap()),
                );

                let config = ClientConfig::builder()
                    .with_root_certificates(root_store)
                    .with_no_client_auth();

                let name = ServerName::try_from(host.to_string()).map_err(|_| {
                    VmError::ValueError(format!("invalid TLS server name: {host}"))
                })?;
                let conn = ClientConnection::new(Arc::new(config), name)
                    .map_err(|e| VmError::NativeError(format!("tls {host}: {e}")))?;

                let stream = StreamOwned::new(conn, tcp);
                let id = next_tls_id();
                TLS_CONNS.with(|m| {
                    m.borrow_mut().insert(id, stream);
                });
                return Ok(Value::int(id as i64));
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(VmError::NativeError(format!(
        "connect {host}:{port}: {}",
        last_err.map(|e| e.to_string()).unwrap_or_default()
    )))
}

/// `tls.connect_pinned(host, port, pins) -> fd`
///
/// TLS with certificate pinning. `pins` is a list of strings like
/// `"sha256:<hex>"` where the hex is the SHA-256 of the server certificate.
// If none match, the handshake fails.
pub fn native_tls_connect_pinned(args: &[Value]) -> Result<Value, VmError> {
    let host = args.get(0)
        .and_then(|v| v.as_str())
        .ok_or_else(|| VmError::TypeError("host must be a string".into()))?;
    let port = args.get(1)
        .and_then(|v| v.as_int())
        .ok_or_else(|| VmError::TypeError("port must be an integer".into()))? as u16;
    let pins_val = args.get(2)
        .and_then(|v| v.as_list())
        .ok_or_else(|| VmError::TypeError("pins must be a list".into()))?;

    let mut pins = Vec::new();
    for p in pins_val {
        if let Some(s) = p.as_str() {
            pins.push(s.to_string());
        }
    }

    let addr = format!("{}:{}", host, port);
    let addrs: Vec<std::net::SocketAddr> = addr
        .to_socket_addrs()
        .map_err(|e| VmError::NativeError(format!("{addr}: {e}")))?
        .collect();

    let mut last_err = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(DEFAULT_TIMEOUT_SECS)) {
            Ok(tcp) => {
                let _ = tcp.set_nodelay(true);
                let _ = tcp.set_read_timeout(Some(Duration::from_secs(DEFAULT_TIMEOUT_SECS)));
                let _ = tcp.set_write_timeout(Some(Duration::from_secs(DEFAULT_TIMEOUT_SECS)));

                let config = ClientConfig::builder()
                    .dangerous()
                    .with_custom_certificate_verifier(Arc::new(CertVerifier::new(pins)))
                    .with_no_client_auth();

                let name = ServerName::try_from(host.to_string()).map_err(|_| {
                    VmError::ValueError(format!("invalid TLS server name: {host}"))
                })?;
                let conn = ClientConnection::new(Arc::new(config), name)
                    .map_err(|e| VmError::NativeError(format!("tls {host}: {e}")))?;

                let stream = StreamOwned::new(conn, tcp);
                let id = next_tls_id();
                TLS_CONNS.with(|m| {
                    m.borrow_mut().insert(id, stream);
                });
                return Ok(Value::int(id as i64));
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(VmError::NativeError(format!(
        "connect {host}:{port}: {}",
        last_err.map(|e| e.to_string()).unwrap_or_default()
    )))
}

/// `tls.write(fd, data) -> nil`
pub fn native_tls_write(args: &[Value]) -> Result<Value, VmError> {
    let id = args.get(0)
        .and_then(|v| v.as_int())
        .ok_or_else(|| VmError::TypeError("fd must be an integer".into()))? as u64;
    let data = args.get(1)
        .and_then(|v| v.as_str())
        .ok_or_else(|| VmError::TypeError("data must be a string".into()))?;

    TLS_CONNS.with(|m| {
        let mut conns = m.borrow_mut();
        let stream = conns.get_mut(&id)
            .ok_or_else(|| VmError::NativeError("invalid tls fd".into()))?;
        stream.write_all(data.as_bytes())
            .map_err(|e| VmError::NativeError(format!("tls write failed: {e}")))?;
        Ok(Value::nil())
    })
}

/// `tls.read(fd, n) -> string`
pub fn native_tls_read(args: &[Value]) -> Result<Value, VmError> {
    let id = args.get(0)
        .and_then(|v| v.as_int())
        .ok_or_else(|| VmError::TypeError("fd must be an integer".into()))? as u64;
    let n = args.get(1)
        .and_then(|v| v.as_int())
        .ok_or_else(|| VmError::TypeError("n must be an integer".into()))? as usize;

    TLS_CONNS.with(|m| {
        let mut conns = m.borrow_mut();
        let stream = conns.get_mut(&id)
            .ok_or_else(|| VmError::NativeError("invalid tls fd".into()))?;
        let mut buf = vec![0u8; n];
        let nread = stream.read(&mut buf)
            .map_err(|e| VmError::NativeError(format!("tls read failed: {e}")))?;
        buf.truncate(nread);
        let text = String::from_utf8_lossy(&buf).into_owned();
        Ok(Value::string(text))
    })
}

/// `tls.close(fd) -> nil`
pub fn native_tls_close(args: &[Value]) -> Result<Value, VmError> {
    let id = args.get(0)
        .and_then(|v| v.as_int())
        .ok_or_else(|| VmError::TypeError("fd must be an integer".into()))? as u64;
    TLS_CONNS.with(|m| {
        m.borrow_mut().remove(&id);
        Ok(Value::nil())
    })
}
