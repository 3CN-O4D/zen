//! wa2 — native Rust WhatsApp client (milestone 1: transport + noise handshake).
//!
//! Long-term plan (replaces the Node/Baileys bridge):
//!   M1  WSS transport + Noise XX handshake        ← this file
//!   M2  Binary node codec + QR/pairing registration
//!   M3  Signal sessions (via signalapp/libsignal) → DM send/recv
//!   M4  Groups/sender keys, app-state sync
//!
//! Reference points for anyone continuing this:
//!   - Go: github.com/tulir/whatsmeow            (noise.go, socket.go)
//!   - JS: WhiskeySockets/Baileys                (src/Socket/noise.js, Utils)
//!   - Protocol notes: https://wiki.whatsapp.com / WADump research

use native_tls::TlsStream;
use std::net::TcpStream;

/// Concrete WSS client type returned by `connect_secure`.
type Wa2Ws = websocket::client::sync::Client<TlsStream<TcpStream>>;

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Primary WS endpoint used by web clients.
pub const WA_WS_URL: &str = "wss://web.whatsapp.com/ws/chat";

/// Headers required by the endpoint; the server rejects requests without
/// a plausible Origin.
pub const WA_HEADERS: &[(&str, &str)] = &[
    ("Origin", "https://web.whatsapp.com"),
    ("User-Agent",
     "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) \
      Chrome/124.0.0.0 Safari/537.36"),
];

/// Noise protocol name bytes hashed at handshake start (prologue).
/// whatsmeow: `Noise_XX_25519_AESGCM_SHA256\x00\x00\x00\x00` — WA deviates
/// from AESGCM to CBC+HMAC-SHA256 frames after the handshake.
pub const NOISE_PROLOGUE: &[u8] = b"Noise_XX_25519_AESGCM_SHA256\0\0\0\0";

/// WhatsApp's server static Noise public key (Curve25519), pinned by all
/// third-party clients.
///
/// TODO(M1): copy the base64 constants from whatsmeow `store/keys.go`
/// (PairCryptoJSON / waStaticKeyPub) or Baileys `src/Utils/noise.js`.
/// Do NOT trust from-memory values here; a wrong pin fails es/sm3 checks
/// confusingly. Until filled in, [`Wa2Session::handshake`] returns
/// [`Error::Unimplemented`].
pub const WA_SERVER_STATIC_PUB_B64: &str = "";

// ─────────────────────────────────────────────────────────────────────────────
// Errors
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Error {
    Unimplemented(&'static str),
    Io(std::io::Error),
    Tls(String),
    Ws(String),
    Noise(String),
    /// Server sent something violating the pinned-key expectation.
    PinMismatch,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unimplemented(w) => write!(f, "wa2: not implemented yet: {w}"),
            Error::Io(e) => write!(f, "wa2: io error: {e}"),
            Error::Tls(e) => write!(f, "wa2: tls error: {e}"),
            Error::Ws(e) => write!(f, "wa2: websocket error: {e}"),
            Error::Noise(e) => write!(f, "wa2: noise error: {e}"),
            Error::PinMismatch => write!(f, "wa2: server static key does not match pin"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self { Error::Io(e) }
}

// ─────────────────────────────────────────────────────────────────────────────
// Crypto helpers (all available offline; unit-tested below)
// ─────────────────────────────────────────────────────────────────────────────

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// HKDF-SHA256 (RFC 5869) extract+expand, used for all WA key derivation.
pub fn hkdf_sha256(salt: &[u8], ikm: &[u8], info: &[u8], out_len: usize) -> Vec<u8> {
    // extract
    let mut mac = HmacSha256::new_from_slice(if salt.is_empty() { &[0u8; 32] } else { salt })
        .expect("hmac accepts any key len");
    mac.update(ikm);
    let prk = mac.finalize().into_bytes();

    // expand
    let mut okm = Vec::with_capacity(out_len);
    let mut t = Vec::new();
    let mut i: u8 = 1;
    while okm.len() < out_len {
        let mut mac = HmacSha256::new_from_slice(&prk).expect("hmac accepts any key len");
        mac.update(&t);
        mac.update(info);
        mac.update(&[i]);
        t = mac.finalize().into_bytes().to_vec();
        okm.extend_from_slice(&t);
        i = i.wrapping_add(1);
    }
    okm.truncate(out_len);
    okm
}

/// SHA-256 helper (protocol name hash / prologue).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().into()
}

/// Fresh Curve25519 keypair for the ephemeral role in XX.
pub fn generate_ephemeral() -> ([u8; 32] /*priv*/, [u8; 32] /*pub*/) {
    use x25519_dalek::{PublicKey, StaticSecret};
    use rand::Rng;
    let seed: [u8; 32] = rand::rng().random();
    let secret = StaticSecret::from(seed);
    let public = PublicKey::from(&secret);
    (*secret.as_bytes(), *public.as_bytes())
}

// ─────────────────────────────────────────────────────────────────────────────
// Frame codec — pre-handshake frames are 3-byte big-endian length prefixed;
// post-handshake every payload carries a 21-byte MAC trailer (HMAC-SHA256,
// truncated). The high bit of the first length byte is the "encrypted" flag.
// ─────────────────────────────────────────────────────────────────────────────

pub mod frame {
    pub const MAX_FRAME: usize = 10 * 1024 * 1024;

    pub fn encode(payload: &[u8], encrypted: bool) -> Vec<u8> {
        let len = payload.len();
        let flag = if encrypted { 0x80u8 } else { 0x00u8 };
        let mut out = Vec::with_capacity(3 + len);
        out.push(flag | ((len >> 16) as u8));
        out.push((len >> 8) as u8);
        out.push(len as u8);
        out.extend_from_slice(payload);
        out
    }

    /// Decode one frame from a buffered stream of bytes.
    pub fn decode(buf: &[u8]) -> Result<Option<(&[u8], bool, usize /*consumed*/)>, super::Error> {
        if buf.len() < 3 {
            return Ok(None);
        }
        let encrypted = buf[0] & 0x80 != 0;
        let len = (((buf[0] & 0x7f) as usize) << 16) | ((buf[1] as usize) << 8) | buf[2] as usize;
        if len > MAX_FRAME {
            return Err(super::Error::Noise(format!("frame too large: {len}")));
        }
        if buf.len() < 3 + len {
            return Ok(None);
        }
        Ok(Some((&buf[3..3 + len], encrypted, 3 + len)))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Binary node codec (M2 preview — structure only)
//
// WA's wire format is a tree: tag (string), attributes (ordered string pairs),
// content (children list or raw bytes). The on-wire packing uses a custom
// dictionary + varint scheme documented in whatsmeow binary/proto.go.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    /// Children when Some(Left(..)), raw payload when Some(Right(..)).
    pub content: Option<Result<Vec<Node>, Vec<u8>>>,
}

impl Node {
    pub fn new(tag: &str) -> Self {
        Node { tag: tag.into(), attrs: vec![], content: None }
    }
    pub fn attr(mut self, k: &str, v: &str) -> Self {
        self.attrs.push((k.into(), v.into()));
        self
    }
    // TODO(M2): pack()/unpack() against the token dictionaries
    // (whatsmeow binary/token.go has the full tables).
}

// ─────────────────────────────────────────────────────────────────────────────
// Session
// ─────────────────────────────────────────────────────────────────────────────

enum HandshakeState {
    NotStarted,
    HelloSent,
    Complete,
}

pub struct Wa2Session {
    ws: Wa2Ws,
    state: HandshakeState,
    e_priv: [u8; 32],
    e_pub: [u8; 32],
    /// Chaining key + cipher keys materialize here as the XX pattern advances.
    rx_key: Option<[u8; 32]>,
    tx_key: Option<[u8; 32]>,
    read_counter: u64,
    write_counter: u64,
}

impl Wa2Session {
    /// Open the WSS connection. Milestone-1 entry point.
    pub fn connect() -> Result<Self, Error> {
        let mut headers = websocket::header::Headers::new();
        for (k, v) in WA_HEADERS {
            headers.set_raw(k.to_string(), vec![v.as_bytes().to_vec()]);
        }
        let mut builder =
            websocket::ClientBuilder::new(WA_WS_URL).map_err(|e| Error::Ws(e.to_string()))?;
        let client = builder
            .custom_headers(&headers)
            .connect_secure(None)
            .map_err(|e| Error::Tls(e.to_string()))?;
        let (e_priv, e_pub) = generate_ephemeral();
        Ok(Wa2Session {
            ws: client,
            state: HandshakeState::NotStarted,
            e_priv,
            e_pub,
            rx_key: None,
            tx_key: None,
            read_counter: 0,
            write_counter: 0,
        })
    }

    /// Run the Noise XX handshake against the server.
    ///
    /// Pattern (WA flavour):  → e   (ClientHello: our ephemeral pub)
    ///                        ← e, ee, s, es  (ServerHello)
    /// then both sides derive rx/tx via HKDF; subsequent frames are
    /// AES-256-CBC + HMAC-SHA256(trunc 21) keyed per-direction with counters.
    pub fn handshake(&mut self) -> Result<(), Error> {
        if !matches!(self.state, HandshakeState::NotStarted) {
            return Err(Error::Noise("handshake already advanced".into()));
        }
        if WA_SERVER_STATIC_PUB_B64.is_empty() {
            return Err(Error::Unimplemented(
                "server static key pin not filled in (see WA_SERVER_STATIC_PUB_B64)",
            ));
        }

        // ── ClientHello ──────────────────────────────────────────────────
        // Node: <noise><l>{ephemeral_pub}</l></noise> wrapped in a handshake
        // envelope node, packed by the M2 codec, framed unencrypted.
        //
        // TODO(M1): needs Node::pack(); structure per whatsmeow noise.go
        // `sendClientHello`.
        let _client_hello = Node::new("noise")
            .attr("l", ""); // placeholder until pack() exists

        self.state = HandshakeState::HelloSent;
        Err(Error::Unimplemented("ClientHello send (needs node codec)"))
        // ── ServerHello (to implement next) ──────────────────────────────
        // 1. read frame, unpack node
        // 2. mix server ephemeral: ee
        // 3. decrypt server static with es-derived key; verify pin
        // 4. finish symmetric state → rx_key/tx_key
        // 5. send ClientFinish (contains encrypted client payload:
        //    handshake props: platforms, versions)
    }

    /// Receive one post-handshake node (plaintext). M3.
    pub fn recv_node(&mut self) -> Result<Node, Error> {
        match self.state {
            HandshakeState::Complete => {}
            _ => return Err(Error::Unimplemented("recv before handshake complete")),
        }
        Err(Error::Unimplemented("encrypted recv"))
    }

    /// Send one post-handshake node. M3.
    pub fn send_node(&mut self, _n: &Node) -> Result<(), Error> {
        match self.state {
            HandshakeState::Complete => {}
            _ => return Err(Error::Unimplemented("send before handshake complete")),
        }
        Err(Error::Unimplemented("encrypted send"))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 5869 A.1 test case 1 (HKDF-SHA256).
    #[test]
    fn hkdf_rfc5869_case1() {
        let ikm = vec![0x0b_u8; 22];
        let salt = hex("000102030405060708090a0b0c");
        let info = hex("f0f1f2f3f4f5f6f7f8f9");
        let okm = hkdf_sha256(&salt, &ikm, &info, 42);
        assert_eq!(
            okm,
            hex("3cb25f25faacd57a90434f64d0362f2a\
                 2d2d0a90cf1a5a4c5db02d56ecc4c5bf\
                 34007208d5b887185865")
        );
    }

    #[test]
    fn frame_roundtrip() {
        let payload = b"hello wa2";
        let f = frame::encode(payload, true);
        assert_eq!((f[0] & 0x80) != 0, true);
        let (got, enc, consumed) = frame::decode(&f).unwrap().unwrap();
        assert_eq!(got, payload);
        assert!(enc);
        assert_eq!(consumed, f.len());
    }

    #[test]
    fn ephemeral_diffie_hellman() {
        let (a_priv, a_pub) = generate_ephemeral();
        let (b_priv, b_pub) = generate_ephemeral();
        use x25519_dalek::{StaticSecret, PublicKey};
        let s1 = StaticSecret::from(a_priv).diffie_hellman(&PublicKey::from(b_pub));
        let s2 = StaticSecret::from(b_priv).diffie_hellman(&PublicKey::from(a_pub));
        assert_eq!(s1.as_bytes(), s2.as_bytes());
        assert_eq!(&a_pub[..], PublicKey::from(&StaticSecret::from(a_priv)).as_bytes());
    }

    /// Live check: WSS upgrade against web.whatsapp.com must succeed.
    #[test]
    #[ignore]
    fn live_wss_connect() {
        let s = Wa2Session::connect().expect("wss connect");
        assert!(matches!(s.state, HandshakeState::NotStarted));
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}
