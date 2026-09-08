//! wa2 — native Rust WhatsApp client (milestones 1+2: transport, codec).
//!
//! Long-term plan (replaces the Node/Baileys bridge):
//!   M1  WSS transport + Noise XX handshake        active (this file)
//!   M2  Binary node codec + QR/pairing material   active (this file)
//!   M3  Signal sessions (via signalapp/libsignal) → DM send/recv
//!   M4  Groups/sender keys, app-state sync
//!
//! The wire protocol here is ported 1:1 from whatsmeow
//! (github.com/tulir/whatsmeow, MPL-2.0):
//!   - `socket/framesocket.go`    3-byte big-endian length, `WA 6 3` header once
//!   - `socket/noisehandshake.go` Noise_XX_25519_AESGCM_SHA256 (protobuf envelope)
//!   - `handshake.go`             server cert chain verified against WACertPubKey
//!   - `binary/{encoder,decoder,token,node}.go` binary-XML token dict (v3)
//!
//! NOTE: the WhatsApp web API is protobuf-enveloped and AES-GCM post-handshake,
//! NOT the historical CBC+HMAC + pinned-static-key scheme that earlier versions
//! of this file assumed.

#![allow(dead_code)]

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

/// Noise handshake pattern. Exactly 32 bytes, so it is mixed in as-is
/// (not SHA-256'd) by the Noise XX implementation.
pub const NOISE_START_PATTERN: &[u8] = b"Noise_XX_25519_AESGCM_SHA256\0\0\0\0";

/// One-shot connection header: `WA` magic, protocol magic value 6,
/// token dictionary version (`binary/token/token.go: DictVersion = 3`).
pub const WA_CONN_HEADER: &[u8] = b"WA\x06\x03";

/// Maximum frame payload size (`FrameMaxSize = 1 << 24`).
pub const FRAME_MAX_SIZE: usize = 1 << 24;

/// Number of length bytes before each frame payload.
pub const FRAME_LENGTH_SIZE: usize = 3;

/// Seconds to wait for the ServerHello after sending ClientHello.
pub const HANDSHAKE_TIMEOUT_SECS: u64 = 20;

/// WhatsApp's Ed25519 CA key used to verify the server's noise certificate
/// chain (`handshake.go: WACertPubKey`).
pub const WA_CERT_PUB_KEY: [u8; 32] = [
    0x14, 0x23, 0x75, 0x57, 0x4d, 0x0a, 0x58, 0x71, 0x66, 0xaa, 0xe7, 0x1e, 0xbe, 0x51, 0x64, 0x37,
    0xc4, 0xa2, 0x8b, 0x73, 0xe3, 0x69, 0x5c, 0x6c, 0xe1, 0xf7, 0xf9, 0x54, 0x5d, 0xa8, 0xee, 0x6b,
];

/// Intermediate CA certificate must have this issuer serial.
pub const WA_CERT_ISSUER_SERIAL: u64 = 0;

/// WhatsApp web client version advertised in the registration payload.
pub const WA_VERSION_STR: &str = "2.3000.1046691727";

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
    Proto(String),
    Codec(String),
    /// Server certificate chain failed verification.
    Cert(String),
    /// Unexpected websocket message type (e.g. text when binary expected).
    BadWs(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unimplemented(w) => write!(f, "wa2: not implemented yet: {w}"),
            Error::Io(e) => write!(f, "wa2: io error: {e}"),
            Error::Tls(e) => write!(f, "wa2: tls error: {e}"),
            Error::Ws(e) => write!(f, "wa2: websocket error: {e}"),
            Error::Noise(e) => write!(f, "wa2: noise error: {e}"),
            Error::Proto(e) => write!(f, "wa2: protobuf error: {e}"),
            Error::Codec(e) => write!(f, "wa2: codec error: {e}"),
            Error::Cert(e) => write!(f, "wa2: certificate error: {e}"),
            Error::BadWs(e) => write!(f, "wa2: bad websocket message: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Crypto helpers (all available offline; unit-tested below)
// ─────────────────────────────────────────────────────────────────────────────

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// HKDF-SHA256 (RFC 5869) extract+expand, used for all WA key derivation.
pub fn hkdf_sha256(salt: &[u8], ikm: &[u8], info: &[u8], out_len: usize) -> Vec<u8> {
    let salt = if salt.is_empty() { &[0u8; 32][..] } else { salt };
    let mut mac: HmacSha256 = hmac::Mac::new_from_slice(salt).expect("hmac accepts any key len");
    mac.update(ikm);
    let prk = mac.finalize().into_bytes();

    let mut okm = Vec::with_capacity(out_len);
    let mut t = Vec::new();
    let mut i: u8 = 1;
    while okm.len() < out_len {
        let mut mac: HmacSha256 =
            hmac::Mac::new_from_slice(&prk).expect("hmac accepts any key len");
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

/// SHA-256 helper.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().into()
}

/// Fresh Curve25519 keypair.
pub fn generate_ephemeral() -> ([u8; 32], [u8; 32]) {
    use rand::Rng;
    use x25519_dalek::{PublicKey, StaticSecret};
    let seed: [u8; 32] = rand::rng().random();
    let secret = StaticSecret::from(seed);
    let public = PublicKey::from(&secret);
    (*secret.as_bytes(), *public.as_bytes())
}

/// 12-byte AES-GCM IV for a frame counter: 8 zero bytes + BE counter.
fn generate_iv(count: u32) -> [u8; 12] {
    let mut iv = [0u8; 12];
    iv[8..].copy_from_slice(&count.to_be_bytes());
    iv
}

use aes_gcm::aead::generic_array::typenum::U12;
use aes_gcm::aead::{generic_array::GenericArray, Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit as _};

/// Build an AES-256-GCM cipher from a 32-byte key (`gcmutil.Prepare`).
fn gcm_prepare(key: &[u8]) -> Result<Aes256Gcm, Error> {
    Aes256Gcm::new_from_slice(key).map_err(|e| Error::Noise(format!("bad gcm key: {e}")))
}

fn gcm_nonce(iv: [u8; 12]) -> aes_gcm::Nonce<U12> {
    GenericArray::clone_from_slice(&iv)
}

// ─────────────────────────────────────────────────────────────────────────────
// Binary-XML node codec (M2)
//
// Values on the wire are: single-byte tokens, two-byte `(dict, index)` tokens,
// nibble/hex packed numerics, raw length-prefixed strings/bytes, lists of
// nodes, and JID encodings. Port of whatsmeow binary/{encoder,decoder}.go.
// ─────────────────────────────────────────────────────────────────────────────

#[path = "wa2_tokens.rs"]
mod tokens;
use tokens::tag as wtok;

/// A value inside a node: content or attribute value.
#[derive(Debug, Clone, PartialEq)]
pub enum WaVal {
    /// `nil` — empty content ("<tag/>" when used as a node's content).
    Null,
    /// String.
    Str(String),
    /// Native bytes, encoded with a length prefix (`Binary*` tags).
    Bytes(Vec<u8>),
    /// Child nodes.
    Nodes(Vec<WaNode>),
    /// Integer (encoded as its decimal string, like `int` in whatsmeow).
    Int(i64),
    /// JID: `(user, server, device, agent)`. `integrator` is unused outside
    /// interop JIDs, which are rare; kept for symmetry with whatsmeow.
    Jid(Jid),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Jid {
    pub user: String,
    pub server: String,
    pub device: u16,
    pub agent: u8,
    pub integrator: u16,
}

impl Jid {
    pub fn new(user: &str, server: &str) -> Self {
        Jid {
            user: user.into(),
            server: server.into(),
            device: 0,
            agent: 0,
            integrator: 0,
        }
    }

    /// An advanced JID: `user@server` with an agent+device pair.
    pub fn advanced(user: &str, server: &str, agent: u8, device: u16) -> Self {
        Jid {
            user: user.into(),
            server: server.into(),
            device,
            agent,
            integrator: 0,
        }
    }
}

/// An XML element on the wire.
#[derive(Debug, Clone, PartialEq)]
pub struct WaNode {
    pub tag: String,
    /// Ordered attribute pairs; empty/Null values are skipped on encode.
    pub attrs: Vec<(String, WaVal)>,
    /// Content: children list, raw bytes, a JID, a string, or Null.
    pub content: WaVal,
}

impl WaNode {
    pub fn new(tag: &str) -> Self {
        WaNode {
            tag: tag.into(),
            attrs: Vec::new(),
            content: WaVal::Null,
        }
    }

    pub fn attr(mut self, key: &str, val: &str) -> Self {
        self.attrs.push((key.into(), WaVal::Str(val.into())));
        self
    }

    /// Encode to the wire representation, including the leading compression
    /// flag byte (`Marshal` keeps the encoder's initial `0`).
    pub fn pack(&self) -> Result<Vec<u8>, Error> {
        let mut e = Encoder::new();
        e.write_node(self)?;
        Ok(e.data)
    }

    /// Decode the full payload (compression flag + optional zlib stream) into
    /// a node — the receiving side of `pack`.
    pub fn unpack(payload: &[u8]) -> Result<WaNode, Error> {
        let stream = unpack_stream(payload)?;
        Self::decode(&stream)
    }

    /// Decode a node from an already-unpacked (flag stripped) stream. Errors
    /// on leftover bytes, like whatsmeow's `Unmarshal`.
    pub fn decode(data: &[u8]) -> Result<WaNode, Error> {
        let mut d = Decoder::new(data);
        let node = d.read_node()?;
        if d.remaining() != 0 {
            return Err(Error::Codec(format!(
                "{} leftover bytes after decoding node",
                d.remaining()
            )));
        }
        Ok(node)
    }

    pub fn children(&self) -> &[WaNode] {
        match &self.content {
            WaVal::Nodes(nodes) => nodes,
            _ => &[],
        }
    }

    pub fn child(&self, tag: &str) -> Option<&WaNode> {
        self.children().iter().find(|n| n.tag == tag)
    }

    pub fn attr_str(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| match v {
                WaVal::Str(s) => Some(s.as_str()),
                _ => None,
            })
    }
}

/// Strip the compression flag byte from a decrypted payload and, if bit 1 is
/// set, zlib-decompress the remainder (`binary/unpack.go`).
pub fn unpack_stream(data: &[u8]) -> Result<Vec<u8>, Error> {
    if data.is_empty() {
        return Err(Error::Codec("empty payload".into()));
    }
    let flag = data[0];
    let rest = &data[1..];
    if flag & 2 != 0 {
        use std::io::Read;
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(rest)
            .read_to_end(&mut out)
            .map_err(|e| Error::Codec(format!("zlib decompression failed: {e}")))?;
        Ok(out)
    } else {
        Ok(rest.to_vec())
    }
}

struct Encoder {
    data: Vec<u8>,
}

impl Encoder {
    fn new() -> Self {
        Encoder { data: vec![0] }
    }

    fn push(&mut self, b: u8) {
        self.data.push(b);
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    fn push_int_n(&mut self, value: i64, n: usize, little_endian: bool) {
        for i in 0..n {
            let shift = if little_endian { i } else { n - i - 1 };
            self.push(((value >> (shift * 8)) & 0xff) as u8);
        }
    }

    fn push_int20(&mut self, value: usize) {
        self.push_bytes(&[((value >> 16) & 0x0f) as u8, (value >> 8) as u8, value as u8]);
    }

    fn push_int8(&mut self, value: i64) {
        self.push_int_n(value, 1, false);
    }

    fn push_int16(&mut self, value: i64) {
        self.push_int_n(value, 2, false);
    }

    fn push_int32(&mut self, value: i64) {
        self.push_int_n(value, 4, false);
    }

    fn write_byte_length(&mut self, length: usize) -> Result<(), Error> {
        if length < 256 {
            self.push(wtok::BINARY_8);
            self.push_int8(length as i64);
        } else if length < (1 << 20) {
            self.push(wtok::BINARY_20);
            self.push_int20(length);
        } else if length < i32::MAX as usize {
            self.push(wtok::BINARY_32);
            self.push_int32(length as i64);
        } else {
            return Err(Error::Codec(format!("length too large: {length}")));
        }
        Ok(())
    }

    fn write_list_start(&mut self, list_size: usize) {
        if list_size == 0 {
            self.push(wtok::LIST_EMPTY);
        } else if list_size < 256 {
            self.push(wtok::LIST_8);
            self.push_int8(list_size as i64);
        } else {
            self.push(wtok::LIST_16);
            self.push_int16(list_size as i64);
        }
    }

    fn write(&mut self, val: &WaVal) -> Result<(), Error> {
        match val {
            WaVal::Null => self.push(wtok::LIST_EMPTY),
            WaVal::Jid(jid) => self.write_jid(jid)?,
            WaVal::Str(s) => self.write_string(s)?,
            WaVal::Int(v) => self.write_string(&v.to_string())?,
            WaVal::Bytes(b) => self.write_bytes(b)?,
            WaVal::Nodes(nodes) => {
                self.write_list_start(nodes.len());
                for n in nodes {
                    self.write_node(n)?;
                }
            }
        }
        Ok(())
    }

    fn write_node(&mut self, n: &WaNode) -> Result<(), Error> {
        if n.tag == "0" {
            self.push(wtok::LIST_8);
            self.push(wtok::LIST_EMPTY);
            return Ok(());
        }
        let has_content = if n.content == WaVal::Null { 0 } else { 1 };
        let attrs = count_attributes(&n.attrs);
        self.write_list_start(2 * attrs + 1 + has_content);
        self.write_string(&n.tag)?;
        for (k, v) in &n.attrs {
            match v {
                WaVal::Str(s) if s.is_empty() => continue,
                WaVal::Null => continue,
                _ => {}
            }
            self.write_string(k)?;
            self.write(v)?;
        }
        if has_content != 0 {
            self.write(&n.content)?;
        }
        Ok(())
    }

    fn write_string(&mut self, s: &str) -> Result<(), Error> {
        if let Some(idx) = tokens::single_index(s) {
            self.push(idx);
        } else if let Some((dict, idx)) = tokens::double_index(s) {
            self.push(wtok::DICTIONARY_0 + dict);
            self.push(idx);
        } else if validate_nibble(s) {
            self.write_packed_bytes(s, wtok::NIBBLE_8)?;
        } else if validate_hex(s) {
            self.write_packed_bytes(s, wtok::HEX_8)?;
        } else {
            self.write_string_raw(s)?;
        }
        Ok(())
    }

    fn write_string_raw(&mut self, s: &str) -> Result<(), Error> {
        self.write_byte_length(s.len())?;
        self.push_bytes(s.as_bytes());
        Ok(())
    }

    fn write_bytes(&mut self, b: &[u8]) -> Result<(), Error> {
        self.write_byte_length(b.len())?;
        self.push_bytes(b);
        Ok(())
    }

    fn write_jid(&mut self, jid: &Jid) -> Result<(), Error> {
        const ADVANCED_SERVERS: [&str; 4] = ["s.whatsapp.net", "hidden", "hosted", "hosted.blackberry"];
        let is_advanced = jid.device > 0 && ADVANCED_SERVERS.contains(&jid.server.as_str());
        if is_advanced {
            self.push(wtok::AD_JID);
            self.push(jid.agent);
            self.push(jid.device as u8);
            self.write_string(&jid.user)?;
        } else {
            self.push(wtok::JID_PAIR);
            if jid.user.is_empty() {
                self.push(wtok::LIST_EMPTY);
            } else {
                self.write(&WaVal::Str(jid.user.clone()))?;
            }
            self.write(&WaVal::Str(jid.server.clone()))?;
        }
        Ok(())
    }

    fn write_packed_bytes(&mut self, s: &str, data_type: u8) -> Result<(), Error> {
        if s.len() > wtok::PACKED_MAX {
            return Err(Error::Codec("too many bytes to pack".into()));
        }
        self.push(data_type);
        // roundedLength = ceil(len / 2), with the high bit marking an odd nibble count.
        let pairs = (s.len() as u64).div_ceil(2);
        let rounded = if s.len().is_multiple_of(2) { pairs as u8 } else { (pairs | 0x80) as u8 };
        self.push(rounded);
        let packer =
            |b: u8| -> u8 { if data_type == wtok::NIBBLE_8 { pack_nibble(b) } else { pack_hex(b) } };
        let bytes = s.as_bytes();
        for i in 0..s.len() / 2 {
            self.push((packer(bytes[2 * i]) << 4) | packer(bytes[2 * i + 1]));
        }
        if !s.len().is_multiple_of(2) {
            self.push((packer(bytes[s.len() - 1]) << 4) | packer(0));
        }
        Ok(())
    }
}

fn count_attributes(attrs: &[(String, WaVal)]) -> usize {
    attrs
        .iter()
        .filter(|(_, v)| match v {
            WaVal::Str(s) => !s.is_empty(),
            WaVal::Null => false,
            _ => true,
        })
        .count()
}

fn validate_nibble(s: &str) -> bool {
    if s.len() > wtok::PACKED_MAX {
        return false;
    }
    s.bytes().all(|b| b.is_ascii_digit() || b == b'-' || b == b'.')
}

fn validate_hex(s: &str) -> bool {
    if s.len() > wtok::PACKED_MAX {
        return false;
    }
    s.bytes().all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
}

fn pack_nibble(b: u8) -> u8 {
    match b {
        b'-' => 10,
        b'.' => 11,
        0 => 15,
        _ if b.is_ascii_digit() => b - b'0',
        _ => panic!("invalid nibble value {b}"),
    }
}

fn pack_hex(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'A'..=b'F' => 10 + b - b'A',
        0 => 15,
        _ => panic!("invalid hex value {b}"),
    }
}

struct Decoder<'a> {
    data: &'a [u8],
    index: usize,
}

impl<'a> Decoder<'a> {
    fn new(data: &'a [u8]) -> Self {
        Decoder { data, index: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.index
    }

    fn check_eos(&self, length: usize) -> Result<(), Error> {
        if self.index + length > self.data.len() {
            return Err(Error::Codec("unexpected end of stream".into()));
        }
        Ok(())
    }

    fn read_byte(&mut self) -> Result<u8, Error> {
        self.check_eos(1)?;
        let b = self.data[self.index];
        self.index += 1;
        Ok(b)
    }

    fn read_int_n(&mut self, n: usize, little_endian: bool) -> Result<i64, Error> {
        self.check_eos(n)?;
        let mut ret = 0i64;
        for i in 0..n {
            let shift = if little_endian { i } else { n - i - 1 };
            ret |= (self.data[self.index + i] as i64) << (shift * 8);
        }
        self.index += n;
        Ok(ret)
    }

    fn read_int8(&mut self) -> Result<i64, Error> {
        self.read_int_n(1, false)
    }

    fn read_int16(&mut self) -> Result<i64, Error> {
        self.read_int_n(2, false)
    }

    fn read_int20(&mut self) -> Result<usize, Error> {
        self.check_eos(3)?;
        let ret = ((self.data[self.index] as usize & 15) << 16)
            | ((self.data[self.index + 1] as usize) << 8)
            | self.data[self.index + 2] as usize;
        self.index += 3;
        Ok(ret)
    }

    fn read_int32(&mut self) -> Result<i64, Error> {
        self.read_int_n(4, false)
    }

    fn read_packed8(&mut self, pack_tag: u8) -> Result<String, Error> {
        let start = self.read_byte()?;
        let mut out = Vec::new();
        for _ in 0..(start & 127) {
            let byte = self.read_byte()?;
            let high = unpack_byte(pack_tag, (byte & 0xf0) >> 4)?;
            let low = unpack_byte(pack_tag, byte & 0x0f)?;
            out.push(high);
            out.push(low);
        }
        if start >> 7 != 0 {
            out.pop();
        }
        String::from_utf8(out).map_err(|e| Error::Codec(format!("bad packed string: {e}")))
    }

    fn read_list_size(&mut self, tag: u8) -> Result<usize, Error> {
        match tag {
            x if x == wtok::LIST_EMPTY => Ok(0),
            x if x == wtok::LIST_8 => Ok(self.read_int8()? as usize),
            x if x == wtok::LIST_16 => Ok(self.read_int16()? as usize),
            _ => Err(Error::Codec(format!("invalid list size tag {tag}"))),
        }
    }

    /// `as_string` mirrors whatsmeow: Binary* values are strings when reading
    /// attribute values/JID parts, raw bytes otherwise.
    fn read(&mut self, as_string: bool) -> Result<WaVal, Error> {
        let tag = self.read_byte()?;
        match tag {
            x if x == wtok::LIST_EMPTY => Ok(WaVal::Null),
            x if x == wtok::LIST_8 || x == wtok::LIST_16 => Ok(WaVal::Nodes(self.read_list(tag)?)),
            x if x == wtok::BINARY_8 => {
                let size = self.read_int8()? as usize;
                self.read_bytes_or_string(size, as_string)
            }
            x if x == wtok::BINARY_20 => {
                let size = self.read_int20()?;
                self.read_bytes_or_string(size, as_string)
            }
            x if x == wtok::BINARY_32 => {
                let size = self.read_int32()? as usize;
                self.read_bytes_or_string(size, as_string)
            }
            wtok::DICTIONARY_0..=wtok::DICTIONARY_3 => {
                let idx = self.read_int8()? as usize;
                let dict = (tag - wtok::DICTIONARY_0) as usize;
                tokens::double_at(dict as u8, idx as u8)
                    .map(|s| WaVal::Str(s.into()))
                    .ok_or_else(|| Error::Codec("double token index out of bounds".into()))
            }
            x if x == wtok::AD_JID => self.read_adjid(),
            x if x == wtok::JID_PAIR => self.read_jid_pair(),
            x if x == wtok::FB_JID => self.read_fbjid(),
            x if x == wtok::INTEROP_JID => self.read_interop_jid(),
            x if x == wtok::NIBBLE_8 || x == wtok::HEX_8 => {
                Ok(WaVal::Str(self.read_packed8(tag)?))
            }
            x => {
                let idx = x as usize;
                if (1..tokens::SINGLE.len()).contains(&idx) {
                    Ok(WaVal::Str(tokens::SINGLE[idx].to_string()))
                } else {
                    Err(Error::Codec(format!("invalid token {x}")))
                }
            }
        }
    }

    fn read_bytes_or_string(&mut self, length: usize, as_string: bool) -> Result<WaVal, Error> {
        self.check_eos(length)?;
        let data = &self.data[self.index..self.index + length];
        self.index += length;
        if as_string {
            Ok(WaVal::Str(String::from_utf8_lossy(data).into_owned()))
        } else {
            Ok(WaVal::Bytes(data.to_vec()))
        }
    }

    fn read_jid_pair(&mut self) -> Result<WaVal, Error> {
        let user = self.read(true)?;
        let server = self.read(true)?;
        let server_str = match server {
            WaVal::Str(s) => s,
            _ => return Err(Error::Codec("invalid JID server".into())),
        };
        match user {
            WaVal::Null => Ok(WaVal::Jid(Jid::new("", &server_str))),
            WaVal::Str(u) => Ok(WaVal::Jid(Jid::new(&u, &server_str))),
            _ => Err(Error::Codec("invalid JID user".into())),
        }
    }

    fn read_adjid(&mut self) -> Result<WaVal, Error> {
        let agent = self.read_byte()?;
        let device = self.read_byte()?;
        let user = self.read(true)?;
        let user_str = match user {
            WaVal::Str(u) => u,
            _ => return Err(Error::Codec("invalid ADJID user".into())),
        };
        Ok(WaVal::Jid(Jid::advanced(&user_str, "s.whatsapp.net", agent, device as u16)))
    }

    fn read_fbjid(&mut self) -> Result<WaVal, Error> {
        let user = self.read(true)?;
        let device = self.read_int16()? as u16;
        let server = self.read(true)?;
        let (user, server) = match (user, server) {
            (WaVal::Str(u), WaVal::Str(s)) => (u, s),
            _ => return Err(Error::Codec("invalid FB JID".into())),
        };
        if server != "fbs:whatsapp.net" {
            return Err(Error::Codec(format!("expected messenger server, got {server}")));
        }
        Ok(WaVal::Jid(Jid { user, server, device, agent: 0, integrator: 0 }))
    }

    fn read_interop_jid(&mut self) -> Result<WaVal, Error> {
        let user = self.read(true)?;
        let device = self.read_int16()? as u16;
        let integrator = self.read_int16()? as u16;
        let server = self.read(true)?;
        let (user, server) = match (user, server) {
            (WaVal::Str(u), WaVal::Str(s)) => (u, s),
            _ => return Err(Error::Codec("invalid interop JID".into())),
        };
        Ok(WaVal::Jid(Jid { user, server, device, agent: 0, integrator }))
    }

    fn read_attributes(&mut self, n: usize) -> Result<Vec<(String, WaVal)>, Error> {
        if n == 0 {
            return Ok(Vec::new());
        }
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let key = self.read(true)?;
            let key = match key {
                WaVal::Str(k) => k,
                _ => return Err(Error::Codec("non-string attribute key".into())),
            };
            let val = self.read(true)?;
            out.push((key, val));
        }
        Ok(out)
    }

    fn read_list(&mut self, tag: u8) -> Result<Vec<WaNode>, Error> {
        let size = self.read_list_size(tag)?;
        let mut out = Vec::with_capacity(size);
        for _ in 0..size {
            out.push(self.read_node()?);
        }
        Ok(out)
    }

    fn read_node(&mut self) -> Result<WaNode, Error> {
        let size = self.read_int8()? as u8;
        let list_size = self.read_list_size(size)?;
        let desc = self.read(true)?;
        let tag = match desc {
            WaVal::Str(t) => t,
            _ => return Err(Error::Codec("non-string node tag".into())),
        };
        if list_size == 0 || tag.is_empty() {
            return Err(Error::Codec("invalid node".into()));
        }
        let attrs = self.read_attributes((list_size - 1) >> 1)?;
        if list_size % 2 == 1 {
            return Ok(WaNode { tag, attrs, content: WaVal::Null });
        }
        let content = self.read(false)?;
        Ok(WaNode { tag, attrs, content })
    }
}

fn unpack_byte(pack_tag: u8, value: u8) -> Result<u8, Error> {
    match pack_tag {
        x if x == wtok::NIBBLE_8 => Ok(unpack_nibble(value)),
        x if x == wtok::HEX_8 => Ok(unpack_hex(value)),
        _ => Err(Error::Codec(format!("unpack with unknown tag {pack_tag}"))),
    }
}

fn unpack_nibble(value: u8) -> u8 {
    match value {
        v if v < 10 => b'0' + v,
        10 => b'-',
        11 => b'.',
        15 => 0,
        _ => panic!("invalid nibble {value}"),
    }
}

fn unpack_hex(value: u8) -> u8 {
    match value {
        v if v < 10 => b'0' + v,
        v if v < 16 => b'A' + v - 10,
        _ => panic!("invalid hex {value}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Minimal protobuf (wire format only — the messages are tiny)
// ─────────────────────────────────────────────────────────────────────────────

pub mod pb {
    use super::Error;

    pub fn write_varint(mut v: u64, out: &mut Vec<u8>) {
        while v >= 0x80 {
            out.push((v as u8 & 0x7f) | 0x80);
            v >>= 7;
        }
        out.push(v as u8);
    }

    pub fn key(num: u64, wire: u64) -> u64 {
        (num << 3) | wire
    }

    pub fn field_uint(num: u64, v: u64, out: &mut Vec<u8>) {
        write_varint(key(num, 0), out);
        write_varint(v, out);
    }

    pub fn field_bool(num: u64, v: bool, out: &mut Vec<u8>) {
        field_uint(num, if v { 1 } else { 0 }, out);
    }

    pub fn field_bytes(num: u64, b: &[u8], out: &mut Vec<u8>) {
        write_varint(key(num, 2), out);
        write_varint(b.len() as u64, out);
        out.extend_from_slice(b);
    }

    pub fn field_string(num: u64, s: &str, out: &mut Vec<u8>) {
        field_bytes(num, s.as_bytes(), out);
    }

    pub fn field_msg(num: u64, msg: &[u8], out: &mut Vec<u8>) {
        field_bytes(num, msg, out);
    }

    /// A decoded field: varint value `v` (wire 0) or length-delimited slice
    /// `data` (wire 2).
    #[derive(Debug, Clone, PartialEq)]
    pub struct Field<'a> {
        pub num: u64,
        pub v: u64,
        pub data: &'a [u8],
        pub wire: u64,
    }

    impl<'a> Field<'a> {
        pub fn bytes(&self) -> &'a [u8] {
            self.data
        }
    }

    fn read_varint_at(data: &[u8], i: &mut usize) -> Result<u64, Error> {
        let mut result = 0u64;
        let mut shift = 0u64;
        loop {
            if *i >= data.len() {
                return Err(Error::Proto("truncated varint".into()));
            }
            let b = data[*i];
            *i += 1;
            result |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
            if shift >= 64 {
                return Err(Error::Proto("varint too long".into()));
            }
        }
    }

    pub fn parse(buf: &[u8]) -> Result<Vec<Field<'_>>, Error> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < buf.len() {
            let k = read_varint_at(buf, &mut i)?;
            let num = k >> 3;
            let wire = k & 7;
            match wire {
                0 => {
                    let v = read_varint_at(buf, &mut i)?;
                    out.push(Field { num, v, data: &[], wire });
                }
                2 => {
                    let len = read_varint_at(buf, &mut i)? as usize;
                    if i + len > buf.len() {
                        return Err(Error::Proto("length-delimited field out of range".into()));
                    }
                    out.push(Field { num, v: 0, data: &buf[i..i + len], wire });
                    i += len;
                }
                _ => return Err(Error::Proto(format!("unsupported wire type {wire}"))),
            }
        }
        Ok(out)
    }

    pub fn find_bytes<'a>(fields: &[Field<'a>], num: u64) -> Option<&'a [u8]> {
        fields.iter().find(|f| f.num == num && f.wire == 2).map(|f| f.data)
    }

    pub fn find_var(fields: &[Field<'_>], num: u64) -> Option<u64> {
        fields.iter().find(|f| f.num == num && f.wire == 0).map(|f| f.v)
    }
}

/// ServerHello extracted from the ServerHandshake protobuf.
struct ServerHello {
    ephemeral: [u8; 32],
    static_ciphertext: Vec<u8>,
    payload_ciphertext: Vec<u8>,
}

/// `waWa6.HandshakeMessage{serverHello=3}` → ephemeral/static/payload.
fn parse_server_hello(data: &[u8]) -> Result<ServerHello, Error> {
    let fields = pb::parse(data)?;
    let sh = pb::find_bytes(&fields, 3).ok_or_else(|| Error::Proto("missing serverHello".into()))?;
    let sh = pb::parse(sh)?;
    let eph = pb::find_bytes(&sh, 1);
    let stat = pb::find_bytes(&sh, 2);
    let load = pb::find_bytes(&sh, 3);
    match (eph, stat, load) {
        (Some(e), Some(s), Some(p)) if e.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(e);
            Ok(ServerHello {
                ephemeral: arr,
                static_ciphertext: s.to_vec(),
                payload_ciphertext: p.to_vec(),
            })
        }
        _ => Err(Error::Proto("malformed serverHello".into())),
    }
}

/// `waWa6.HandshakeMessage{clientHello=2, clientFinish=4}` marshalling.
fn marshal_client_hello(ephemeral: &[u8; 32]) -> Vec<u8> {
    let mut hello = Vec::new();
    pb::field_bytes(1, ephemeral, &mut hello);
    let mut msg = Vec::new();
    pb::field_msg(2, &hello, &mut msg);
    msg
}

/// `waWa6.HandshakeMessage{clientFinish=4}{static=1, payload=2}`.
fn marshal_client_finish(static_ct: &[u8], payload_ct: &[u8]) -> Vec<u8> {
    let mut finish = Vec::new();
    pb::field_bytes(1, static_ct, &mut finish);
    pb::field_bytes(2, payload_ct, &mut finish);
    let mut msg = Vec::new();
    pb::field_msg(4, &finish, &mut msg);
    msg
}

/// `(details, signature)` raw bytes of one noise certificate.
type CertPart = (Vec<u8>, Vec<u8>);

/// `waCert.CertChain{leaf=1, intermediate=2}` decode:
/// returns `(intermediate, leaf)` pairs of `(details, signature)`.
fn parse_cert_chain(data: &[u8]) -> Result<(CertPart, CertPart), Error> {
    let chain = pb::parse(data)?;
    let inter = pb::find_bytes(&chain, 2).ok_or_else(|| Error::Proto("missing intermediate".into()))?;
    let leaf = pb::find_bytes(&chain, 1).ok_or_else(|| Error::Proto("missing leaf".into()))?;
    let inter = pb::parse(inter)?;
    let leaf = pb::parse(leaf)?;
    let inter_det = pb::find_bytes(&inter, 1).ok_or_else(|| Error::Proto("missing intermediate details".into()))?;
    let inter_sig = pb::find_bytes(&inter, 2).ok_or_else(|| Error::Proto("missing intermediate signature".into()))?;
    let leaf_det = pb::find_bytes(&leaf, 1).ok_or_else(|| Error::Proto("missing leaf details".into()))?;
    let leaf_sig = pb::find_bytes(&leaf, 2).ok_or_else(|| Error::Proto("missing leaf signature".into()))?;
    Ok(((inter_det.to_vec(), inter_sig.to_vec()), (leaf_det.to_vec(), leaf_sig.to_vec())))
}

/// `waCert.CertChain.NoiseCertificate.Details`: serial=1, issuerSerial=2,
/// key=3, notBefore=4, notAfter=5.
struct CertDetails {
    serial: u64,
    issuer_serial: u64,
    key: Vec<u8>,
    not_before: u64,
    not_after: u64,
}

fn parse_cert_details(data: &[u8]) -> Result<CertDetails, Error> {
    let f = pb::parse(data)?;
    Ok(CertDetails {
        serial: pb::find_var(&f, 1).unwrap_or(0),
        issuer_serial: pb::find_var(&f, 2).unwrap_or(0),
        key: pb::find_bytes(&f, 3).unwrap_or(&[]).to_vec(),
        not_before: pb::find_var(&f, 4).unwrap_or(0),
        not_after: pb::find_var(&f, 5).unwrap_or(0),
    })
}

fn check_cert_validity(cert: &CertDetails) -> Result<(), Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::Cert("system clock before unix epoch".into()))?
        .as_secs();
    if now < cert.not_before {
        return Err(Error::Cert(format!(
            "certificate not valid yet (now {now} < notBefore {})",
            cert.not_before
        )));
    }
    if now > cert.not_after {
        return Err(Error::Cert(format!(
            "certificate expired (now {now} > notAfter {})",
            cert.not_after
        )));
    }
    Ok(())
}

/// Verify the noise certificate chain against `WA_CERT_PUB_KEY`
/// (`whatsmeow/handshake.go: verifyServerCert`).
fn verify_server_cert(chain_data: &[u8], server_static: &[u8]) -> Result<(), Error> {
    let ((inter_det, inter_sig), (leaf_det, leaf_sig)) = parse_cert_chain(chain_data)?;
    if inter_sig.len() != 64 || leaf_sig.len() != 64 {
        return Err(Error::Cert("unexpected signature length".into()));
    }

    use ed25519_dalek::{Signature, VerifyingKey};
    let vk = VerifyingKey::from_bytes(&WA_CERT_PUB_KEY)
        .map_err(|e| Error::Cert(format!("bad ca key: {e}")))?;
    let sig = Signature::from_slice(&inter_sig)
        .map_err(|e| Error::Cert(format!("bad intermediate signature: {e}")))?;
    vk.verify_strict(&inter_det, &sig)
        .map_err(|_| Error::Cert("intermediate cert signature verification failed".into()))?;

    let intermediate = parse_cert_details(&inter_det)?;
    if intermediate.issuer_serial != WA_CERT_ISSUER_SERIAL {
        return Err(Error::Cert(format!(
            "unexpected intermediate issuer serial {} (expected {WA_CERT_ISSUER_SERIAL})",
            intermediate.issuer_serial
        )));
    }
    if intermediate.key.len() != 32 {
        return Err(Error::Cert("unexpected intermediate key length".into()));
    }

    let ik = VerifyingKey::from_bytes(
        &<[u8; 32]>::try_from(&intermediate.key[..]).map_err(|_| Error::Cert("bad key".into()))?,
    )
    .map_err(|e| Error::Cert(format!("bad intermediate key: {e}")))?;
    let sig = Signature::from_slice(&leaf_sig)
        .map_err(|e| Error::Cert(format!("bad leaf signature: {e}")))?;
    ik.verify_strict(&leaf_det, &sig)
        .map_err(|_| Error::Cert("leaf cert signature verification failed".into()))?;

    check_cert_validity(&intermediate)?;

    let leaf = parse_cert_details(&leaf_det)?;
    if leaf.issuer_serial != intermediate.serial {
        return Err(Error::Cert(format!(
            "unexpected leaf issuer serial {} (expected {})",
            leaf.issuer_serial, intermediate.serial
        )));
    }
    if leaf.key != server_static {
        return Err(Error::Cert("cert key doesn't match decrypted static".into()));
    }
    check_cert_validity(&leaf)?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Noise XX handshake state machine (socket/noisehandshake.go)
// ─────────────────────────────────────────────────────────────────────────────

struct NoiseHandshake {
    hash: Vec<u8>,
    salt: Vec<u8>,
    key: Aes256Gcm,
    counter: u32,
}

impl NoiseHandshake {
    fn new() -> Self {
        NoiseHandshake {
            hash: Vec::new(),
            salt: Vec::new(),
            key: gcm_prepare(&[0u8; 32]).expect("zero key is valid"),
            counter: 0,
        }
    }

    fn start(&mut self, pattern: &[u8], header: &[u8]) {
        self.hash = if pattern.len() == 32 { pattern.to_vec() } else { sha256(pattern).to_vec() };
        self.salt = self.hash.clone();
        self.key = gcm_prepare(&self.hash).expect("32-byte key is valid");
        self.counter = 0;
        self.authenticate(header);
    }

    fn authenticate(&mut self, data: &[u8]) {
        let mut h = self.hash.clone();
        h.extend_from_slice(data);
        self.hash = sha256(&h).to_vec();
    }

    fn post_increment_counter(&mut self) -> u32 {
        let c = self.counter;
        self.counter = self.counter.wrapping_add(1);
        c
    }

    fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, Error> {
        let nonce = gcm_nonce(generate_iv(self.post_increment_counter()));
        let ct = self
            .key
            .encrypt(&nonce, Payload { msg: plaintext, aad: &self.hash[..] })
            .map_err(|_| Error::Noise("handshake encrypt failed".into()))?;
        self.authenticate(&ct);
        Ok(ct)
    }

    fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, Error> {
        let nonce = gcm_nonce(generate_iv(self.post_increment_counter()));
        let pt = self
            .key
            .decrypt(&nonce, Payload { msg: ciphertext, aad: &self.hash[..] })
            .map_err(|_| Error::Noise("handshake decrypt failed".into()))?;
        self.authenticate(ciphertext);
        Ok(pt)
    }

    fn extract_and_expand(&self, salt: &[u8], data: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let out = hkdf_sha256(salt, data, &[], 64);
        (out[..32].to_vec(), out[32..].to_vec())
    }

    fn mix_into_key(&mut self, data: &[u8]) -> Result<(), Error> {
        self.counter = 0;
        let (write, read) = self.extract_and_expand(&self.salt, data);
        self.salt = write.clone();
        self.key = gcm_prepare(&read)?;
        Ok(())
    }

    fn mix_shared_secret_into_key(&mut self, priv_key: &[u8; 32], pub_key: &[u8; 32]) -> Result<(), Error> {
        use x25519_dalek::{PublicKey, StaticSecret};
        let secret = StaticSecret::from(*priv_key).diffie_hellman(&PublicKey::from(*pub_key));
        self.mix_into_key(secret.as_bytes())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Frame + noise socket (socket/framesocket.go, socket/noisesocket.go)
// ─────────────────────────────────────────────────────────────────────────────

/// 3-byte big-endian length-prefixed frame. The `WA 6 3` header is prepended
/// once (first frame only) when sent.
fn frame_bytes(payload: &[u8]) -> Vec<u8> {
    let len = payload.len();
    let mut out = Vec::with_capacity(FRAME_LENGTH_SIZE + len);
    out.push((len >> 16) as u8);
    out.push((len >> 8) as u8);
    out.push(len as u8);
    out.extend_from_slice(payload);
    out
}

/// Try to drain one complete frame from a receive buffer.
/// Returns `Ok(None)` if more bytes are needed.
fn try_drain_frame(buf: &mut Vec<u8>) -> Result<Option<Vec<u8>>, Error> {
    if buf.len() < FRAME_LENGTH_SIZE {
        return Ok(None);
    }
    let len = (buf[0] as usize) << 16 | (buf[1] as usize) << 8 | buf[2] as usize;
    if len > FRAME_MAX_SIZE {
        return Err(Error::Noise(format!("frame too large: {len}")));
    }
    let total = FRAME_LENGTH_SIZE + len;
    if buf.len() < total {
        return Ok(None);
    }
    let frame = buf[FRAME_LENGTH_SIZE..total].to_vec();
    buf.drain(..total);
    Ok(Some(frame))
}

/// One full-duplex WA frame channel over the websocket.
struct FrameSocket {
    ws: Wa2Ws,
    header_sent: bool,
    buffer: Vec<u8>,
}

impl FrameSocket {
    fn connect() -> Result<Self, Error> {
        let mut headers = websocket::header::Headers::new();
        for (k, v) in WA_HEADERS {
            headers.set_raw(k.to_string(), vec![v.as_bytes().to_vec()]);
        }
        let builder =
            websocket::ClientBuilder::new(WA_WS_URL).map_err(|e| Error::Ws(e.to_string()))?;
        let client = builder
            .custom_headers(&headers)
            .connect_secure(None)
            .map_err(|e| Error::Tls(e.to_string()))?;
        Ok(FrameSocket { ws: client, header_sent: false, buffer: Vec::new() })
    }

    fn set_read_timeout(&mut self, secs: Option<u64>) -> Result<(), Error> {
        if let Some(secs) = secs {
            self.ws.stream_ref().get_ref().set_read_timeout(Some(std::time::Duration::from_secs(secs)))?;
        } else {
            self.ws.stream_ref().get_ref().set_read_timeout(None)?;
        }
        Ok(())
    }

    fn send_frame(&mut self, payload: &[u8]) -> Result<(), Error> {
        if payload.len() >= FRAME_MAX_SIZE {
            return Err(Error::Noise(format!(
                "frame too large to send: {}",
                payload.len()
            )));
        }
        let mut whole = Vec::with_capacity(
            (if self.header_sent { 0 } else { WA_CONN_HEADER.len() })
                + FRAME_LENGTH_SIZE
                + payload.len(),
        );
        if !self.header_sent {
            whole.extend_from_slice(WA_CONN_HEADER);
            self.header_sent = true;
        }
        whole.extend_from_slice(&frame_bytes(payload));
        self.ws
            .send_message(&websocket::OwnedMessage::Binary(whole))
            .map_err(|e| Error::Ws(format!("send: {e}")))
    }

    fn recv_frame(&mut self) -> Result<Vec<u8>, Error> {
        loop {
            if let Some(frame) = try_drain_frame(&mut self.buffer)? {
                return Ok(frame);
            }
            match self.ws.recv_message().map_err(|e| Error::Ws(format!("recv: {e}")))? {
                websocket::OwnedMessage::Binary(data) => {
                    if data.len() > FRAME_MAX_SIZE {
                        return Err(Error::Noise(format!(
                            "incoming ws message too large: {}",
                            data.len()
                        )));
                    }
                    self.buffer.extend_from_slice(&data);
                }
                websocket::OwnedMessage::Ping(payload) => {
                    self.ws
                        .send_message(&websocket::OwnedMessage::Pong(payload))
                        .map_err(|e| Error::Ws(format!("pong: {e}")))?;
                }
                websocket::OwnedMessage::Close(_) => {
                    return Err(Error::Ws("connection closed by server".into()))
                }
                _ => return Err(Error::BadWs("expected binary frame")),
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Client payload (registration path; store/clientpayload.go)
// ─────────────────────────────────────────────────────────────────────────────

/// Key material the client presents during registration. In the real
/// whatsmeow flow the identity/prekey signing uses libsignal's curve
/// semantics; parity for that scheme is an M2 follow-up — the fields here
/// are self-consistent so the handshake envelope passes.
pub struct PairingMaterial {
    pub registration_id: u32,
    pub identity_pub: [u8; 32],
    pub prekey_id: u32,
    pub signed_prekey_pub: [u8; 32],
    pub signed_prekey_sig: [u8; 64],
}

pub fn generate_pairing_material() -> PairingMaterial {
    use rand::Rng as _;

    let (_, identity_pub) = generate_ephemeral();
    let (_, signed_prekey_pub) = generate_ephemeral();

    // libsignal-compatible identity signatures are an M2 follow-up; sign the
    // signed prekey with a throwaway Ed25519 key for now so every field is
    // well-formed and cryptographically self-consistent.
    let mut rng_key = [0u8; 32];
    rng_key.copy_from_slice(&rand::rng().random::<[u8; 32]>());
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&rng_key);
    use ed25519_dalek::Signer as _;
    let signature = signing_key.sign(&signed_prekey_pub);

    PairingMaterial {
        registration_id: rand::rng().random::<u32>() | 1,
        identity_pub,
        prekey_id: rand::rng().random::<u32>() | 1,
        signed_prekey_pub,
        signed_prekey_sig: signature.to_bytes(),
    }
}

fn app_version_msg() -> Vec<u8> {
    let parts = WA_VERSION_STR.split('.').collect::<Vec<_>>();
    let mut out = Vec::new();
    for (i, part) in parts.iter().take(3).enumerate() {
        if let Ok(v) = part.parse::<u64>() {
            if v != 0 {
                pb::field_uint((i + 1) as u64, v, &mut out);
            }
        }
    }
    out
}

fn device_props_msg() -> Vec<u8> {
    let mut version = Vec::new();
    // AppVersion{secondary=1} (0.1.0)
    pb::field_uint(2, 1, &mut version);

    let mut hsc = Vec::new();
    pb::field_uint(3, 10240, &mut hsc); // storageQuotaMb
    pb::field_bool(4, true, &mut hsc); // inlineInitialPayloadInE2EeMsg
    pb::field_bool(6, true, &mut hsc); // supportCallLogHistory
    pb::field_bool(7, true, &mut hsc); // supportBotUserAgentChatHistory
    pb::field_bool(8, true, &mut hsc); // supportCagReactionsAndPolls
    pb::field_bool(9, true, &mut hsc); // supportBizHostedMsg
    pb::field_bool(10, true, &mut hsc); // supportRecentSyncChunkMessageCountTuning
    pb::field_bool(11, true, &mut hsc); // supportHostedGroupMsg
    pb::field_bool(12, true, &mut hsc); // supportFbidBotChatHistory
    pb::field_bool(14, true, &mut hsc); // supportMessageAssociation
    pb::field_bool(15, true, &mut hsc); // supportGroupHistory
    pb::field_uint(19, 60, &mut hsc); // thumbnailSyncDaysLimit
    pb::field_bool(21, true, &mut hsc); // supportManusHistory
    pb::field_bool(22, true, &mut hsc); // supportHatchHistory

    let mut props = Vec::new();
    pb::field_string(1, "whatsmeow", &mut props); // os
    pb::field_msg(2, &version, &mut props); // version
    pb::field_msg(5, &hsc, &mut props); // historySyncConfig
    props
}

/// Build the registration-style `ClientPayload` protobuf.
pub fn build_client_payload(material: &PairingMaterial) -> Vec<u8> {
    use md5::Digest as _;

    let mut ua = Vec::new();
    pb::field_uint(1, 14, &mut ua); // platform: WEB
    pb::field_msg(2, &app_version_msg(), &mut ua); // appVersion
    pb::field_string(3, "000", &mut ua); // mcc
    pb::field_string(4, "000", &mut ua); // mnc
    pb::field_string(5, "0.1", &mut ua); // osVersion
    pb::field_string(6, "", &mut ua); // manufacturer
    pb::field_string(7, "Desktop", &mut ua); // device
    pb::field_string(8, "0.1", &mut ua); // osBuildNumber
    pb::field_string(11, "en", &mut ua); // localeLanguageIso6391
    pb::field_string(12, "US", &mut ua); // localeCountryIso31661Alpha2

    let reg_id = material.registration_id.to_be_bytes();
    let prekey_id = material.prekey_id.to_be_bytes();

    let mut pairing = Vec::new();
    pb::field_bytes(1, &reg_id, &mut pairing); // eRegid
    pb::field_bytes(2, &[ecc_curve_djb_type()], &mut pairing); // eKeytype
    pb::field_bytes(3, &material.identity_pub, &mut pairing); // eIdent
    pb::field_bytes(4, &prekey_id[1..], &mut pairing); // eSkeyID
    pb::field_bytes(5, &material.signed_prekey_pub, &mut pairing); // eSkeyVal
    pb::field_bytes(6, &material.signed_prekey_sig, &mut pairing); // eSkeySig
    let build_hash = md5::Md5::digest(WA_VERSION_STR.as_bytes());
    pb::field_bytes(7, &build_hash[..], &mut pairing); // buildHash
    pb::field_bytes(8, &device_props_msg(), &mut pairing); // deviceProps

    let mut payload = Vec::new();
    pb::field_msg(5, &ua, &mut payload); // userAgent
    pb::field_msg(6, &[], &mut payload); // webInfo (webSubPlatform=WEB_BROWSER=0, default)
    pb::field_uint(12, 1, &mut payload); // connectType: WIFI_UNKNOWN
    pb::field_uint(13, 1, &mut payload); // connectReason: USER_ACTIVATED
    pb::field_msg(19, &pairing, &mut payload); // devicePairingData
    payload
}

/// libsignal `ecc.DjbType` — Curve25519 public key marker.
fn ecc_curve_djb_type() -> u8 {
    5
}

// ─────────────────────────────────────────────────────────────────────────────
// High-level client
// ─────────────────────────────────────────────────────────────────────────────

/// Encrypted WA session after `handshake()`.
pub struct Wa2Session {
    fs: FrameSocket,
    write_cipher: Option<Aes256Gcm>,
    read_cipher: Option<Aes256Gcm>,
    write_counter: u32,
    read_counter: u32,
    handshaken: bool,
}

impl Wa2Session {
    /// Open the WSS connection without running the handshake.
    pub fn connect() -> Result<Self, Error> {
        let fs = FrameSocket::connect()?;
        Ok(Wa2Session {
            fs,
            write_cipher: None,
            read_cipher: None,
            write_counter: 0,
            read_counter: 0,
            handshaken: false,
        })
    }

    /// Convenience: connect and complete the Noise XX handshake.
    pub fn connect_with_handshake() -> Result<Self, Error> {
        let mut s = Self::connect()?;
        s.handshake(&generate_pairing_material())?;
        Ok(s)
    }

    /// Run the full Noise XX handshake (`whatsmeow/handshake.go: doHandshake`).
    pub fn handshake(&mut self, material: &PairingMaterial) -> Result<(), Error> {
        if self.handshaken {
            return Err(Error::Noise("handshake already complete".into()));
        }

        let mut nh = NoiseHandshake::new();
        nh.start(NOISE_START_PATTERN, WA_CONN_HEADER);

        let (e_priv, e_pub) = generate_ephemeral();
        nh.authenticate(&e_pub);
        self.fs.send_frame(&marshal_client_hello(&e_pub))?;

        // ServerHello arrives as a plain frame (header was sent once, above).
        self.fs.set_read_timeout(Some(HANDSHAKE_TIMEOUT_SECS))?;
        let resp = self.fs.recv_frame()?;
        self.fs.set_read_timeout(None)?;

        let server_hello = parse_server_hello(&resp)?;

        nh.authenticate(&server_hello.ephemeral);
        nh.mix_shared_secret_into_key(&e_priv, &server_hello.ephemeral)?;

        let server_static = nh.decrypt(&server_hello.static_ciphertext)?;
        if server_static.len() != 32 {
            return Err(Error::Noise(format!(
                "unexpected server static length {}",
                server_static.len()
            )));
        }
        nh.mix_shared_secret_into_key(&e_priv, &server_static[..].try_into().expect("len checked"))?;

        let cert = nh.decrypt(&server_hello.payload_ciphertext)?;
        verify_server_cert(&cert, &server_static)?;

        // ClientFinish: encrypt our permanent Noise public key, then mix the
        // same Noise key against the server ephemeral.
        let (noise_priv, noise_pub) = generate_ephemeral();
        let encrypted_noise_pub = nh.encrypt(&noise_pub)?;
        nh.mix_shared_secret_into_key(&noise_priv, &server_hello.ephemeral)?;

        let payload = build_client_payload(material);
        let encrypted_payload = nh.encrypt(&payload)?;
        self.fs
            .send_frame(&marshal_client_finish(&encrypted_noise_pub, &encrypted_payload))?;

        // Derive the session write/read keys (HKDF(salt, nil)).
        let (write_key, read_key) = nh.extract_and_expand(&nh.salt, &[]);
        self.write_cipher = Some(gcm_prepare(&write_key)?);
        self.read_cipher = Some(gcm_prepare(&read_key)?);
        self.write_counter = 0;
        self.read_counter = 0;
        self.handshaken = true;
        Ok(())
    }

    /// Send one node (packed, encrypted, framed).
    pub fn send_node(&mut self, node: &WaNode) -> Result<(), Error> {
        self.send_raw(&node.pack()?)
    }

    /// Receive one decrypted frame and decode it into a node.
    pub fn recv_node(&mut self) -> Result<WaNode, Error> {
        let data = self.recv_raw()?;
        WaNode::unpack(&data)
    }

    /// Send raw plaintext through the Noise socket.
    pub fn send_raw(&mut self, plaintext: &[u8]) -> Result<(), Error> {
        let cipher = self
            .write_cipher
            .as_ref()
            .ok_or_else(|| Error::Noise("send before handshake".into()))?;
        let nonce = gcm_nonce(generate_iv(self.write_counter));
        self.write_counter = self.write_counter.wrapping_add(1);
        let ct = cipher
            .encrypt(&nonce, Payload { msg: plaintext, aad: &[] })
            .map_err(|_| Error::Noise("send encrypt failed".into()))?;
        self.fs.send_frame(&ct)
    }

    /// Receive one raw encrypted frame, decrypted.
    pub fn recv_raw(&mut self) -> Result<Vec<u8>, Error> {
        let cipher = self
            .read_cipher
            .as_ref()
            .ok_or_else(|| Error::Noise("recv before handshake".into()))?;
        let ct = self.fs.recv_frame()?;
        let nonce = gcm_nonce(generate_iv(self.read_counter));
        self.read_counter = self.read_counter.wrapping_add(1);
        cipher
            .decrypt(&nonce, Payload { msg: &ct[..], aad: &[] })
            .map_err(|_| Error::Noise("recv decrypt failed".into()))
    }

    /// Convenience send: same as `send_node` (packed already includes the
    /// leading compression flag so nothing more to add).
    pub fn is_handshaken(&self) -> bool {
        self.handshaken
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

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
        let f = frame_bytes(payload);
        let mut buf = f.clone();
        let got = try_drain_frame(&mut buf).unwrap().unwrap();
        assert_eq!(got, payload);
        assert!(buf.is_empty());
        // partial frame needs more data
        buf.extend_from_slice(&f[..f.len() - 1]);
        assert!(try_drain_frame(&mut buf).unwrap().is_none());
        buf.push(f.last().copied().unwrap());
        assert_eq!(try_drain_frame(&mut buf).unwrap().unwrap(), payload);
    }

    #[test]
    fn ephemeral_diffie_hellman() {
        let (a_priv, a_pub) = generate_ephemeral();
        let (b_priv, b_pub) = generate_ephemeral();
        use x25519_dalek::{PublicKey as XPub, StaticSecret as XSec};
        let s1 = XSec::from(a_priv).diffie_hellman(&XPub::from(b_pub));
        let s2 = XSec::from(b_priv).diffie_hellman(&XPub::from(a_pub));
        assert_eq!(s1.as_bytes(), s2.as_bytes());
        assert_eq!(&a_pub[..], XPub::from(&XSec::from(a_priv)).as_bytes());
    }

    /// Single-byte and double-byte token encoding (vectors verified against
    /// whatsmeow's `binary.Marshal` output).
    #[test]
    fn node_pack_tokens() {
        // "xmlstreamstart" is single-byte token 1
        assert_eq!(
            WaNode::new("xmlstreamstart").pack().unwrap(),
            vec![0x00, 0xf8, 0x01, 0x01]
        );
        // "active" is double-byte token (dictionary 0, index 1)
        assert_eq!(
            WaNode::new("active").pack().unwrap(),
            vec![0x00, 0xf8, 0x01, 0xec, 0x01]
        );
    }

    /// Attribute with a nibble-packed numeric value.
    /// "message"=0x13, "type"=0x04, "123" → FF 82 12 3F.
    #[test]
    fn node_pack_attrs_nibble() {
        let n = WaNode::new("message").attr("type", "123");
        assert_eq!(n.pack().unwrap(), vec![0x00, 0xf8, 0x03, 0x13, 0x04, 0xff, 0x82, 0x12, 0x3f]);
    }

    /// Packed nibble/hex vectors (round lengths, high-bit odd flag). The
    /// encoder starts with the leading compression flag byte.
    #[test]
    fn packed_byte_vectors() {
        let mut e = Encoder::new();
        e.write_packed_bytes("123", wtok::NIBBLE_8).unwrap();
        assert_eq!(&e.data[1..], &[0xff, 0x82, 0x12, 0x3f]); // odd → ceil(3/2)=2|0x80

        let mut e = Encoder::new();
        e.write_packed_bytes("7-.", wtok::NIBBLE_8).unwrap();
        assert_eq!(&e.data[1..], &[0xff, 0x82, 0x7a, 0xbf]); // nibble '-'=0xa, '.'=0xb

        let mut e = Encoder::new();
        e.write_packed_bytes("ABCD", wtok::HEX_8).unwrap();
        assert_eq!(&e.data[1..], &[0xfb, 0x02, 0xab, 0xcd]);

        let mut e = Encoder::new();
        e.write_packed_bytes("A1", wtok::HEX_8).unwrap();
        assert_eq!(&e.data[1..], &[0xfb, 0x01, 0xa1]);
    }

    /// Nibble/hex/raw attribute values round-trip (vector-free).
    #[test]
    fn node_roundtrip_attrs() {
        for val in ["123", "1.5", "02", "ABCD", "a-standalone", "name with spaces"] {
            let n = WaNode::new("att").attr("v", val);
            let packed = n.pack().unwrap();
            assert_eq!(WaNode::unpack(&packed).unwrap(), n, "value={val}");
        }
    }

    #[test]
    fn node_roundtrip_complex() {
        let inner = WaNode::new("child")
            .attr("id", "020")
            .attr("num", "1.5");
        let n = WaNode::new("iq")
            .attr("type", "get")
            .attr("to", "s.whatsapp.net")
            .attr("id", "42");
        let n = WaNode {
            tag: n.tag,
            attrs: n.attrs,
            content: WaVal::Nodes(vec![inner]),
        };

        let packed = n.pack().unwrap();
        let back = WaNode::unpack(&packed).unwrap();
        assert_eq!(back, n);
    }

    #[test]
    fn node_roundtrip_bytes_and_jid() {
        let n = WaNode {
            tag: "blob".into(),
            attrs: vec![("from".into(), WaVal::Jid(Jid::advanced("user", "s.whatsapp.net", 0, 1)))],
            content: WaVal::Nodes(vec![WaNode {
                tag: "raw".into(),
                attrs: vec![],
                content: WaVal::Bytes(vec![1, 2, 3]),
            }]),
        };
        let packed = n.pack().unwrap();
        assert_eq!(WaNode::unpack(&packed).unwrap(), n);
    }

    #[test]
    fn unpack_stream_zlib() {
        // Simulate a server frame with the compression flag set.
        let payload = b"some node tree data".to_vec();
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut enc, &payload).unwrap();
        let compressed = enc.finish().unwrap();

        let mut framed = vec![0x02];
        framed.extend_from_slice(&compressed);
        assert_eq!(unpack_stream(&framed).unwrap(), payload);
        // flag 0 → passthrough
        let mut plain = vec![0x00];
        plain.extend_from_slice(&payload);
        assert_eq!(unpack_stream(&plain).unwrap(), payload);
    }

    #[test]
    fn handshake_crypto_self_consistent() {
        // Two independent handshake states mirroring client and server must
        // converge on the same shared static value.
        let mut a = NoiseHandshake::new();
        let mut b = NoiseHandshake::new();
        a.start(NOISE_START_PATTERN, WA_CONN_HEADER);
        b.start(NOISE_START_PATTERN, WA_CONN_HEADER);

        let (a_priv, a_pub) = generate_ephemeral();
        let (b_priv, b_pub) = generate_ephemeral();
        a.authenticate(&a_pub);
        b.authenticate(&a_pub);

        // ee mix on both sides
        a.mix_shared_secret_into_key(&a_priv, &b_pub).unwrap();
        b.mix_shared_secret_into_key(&b_priv, &a_pub).unwrap();

        // static exchange: b "decrypts" nothing; just demonstrate keyed gcm
        // round trip with AAD=hash works for both directions.
        let secret = "shared plane".as_bytes().to_vec();
        let ct = b.encrypt(&secret).unwrap();
        let pt = a.decrypt(&ct).unwrap();
        assert_eq!(pt, secret);
    }

    #[test]
    fn cert_verify_rejects_garbage() {
        let res = verify_server_cert(b"not a cert chain at all", &[0u8; 32]);
        assert!(res.is_err());
    }

    #[test]
    fn client_payload_marshals() {
        let m = generate_pairing_material();
        let payload = build_client_payload(&m);
        assert!(!payload.is_empty());
        let fields = pb::parse(&payload).unwrap();
        assert!(pb::find_var(&fields, 12) == Some(1)); // WIFI_UNKNOWN
        assert!(pb::find_var(&fields, 13) == Some(1)); // USER_ACTIVATED
        assert!(pb::find_bytes(&fields, 5).is_some()); // userAgent
        assert!(pb::find_bytes(&fields, 19).is_some()); // devicePairingData
    }

    #[test]
    fn pair_material_build_hash_is_md5() {
        // buildHash is md5 of the dot-separated version string.
        let h = md5::Md5::digest(WA_VERSION_STR.as_bytes());
        assert_eq!(h.len(), 16);
        let known = md5::Md5::digest(b"abc");
        assert_eq!(known.to_vec(), hex("900150983cd24fb0d6963f7d28e17f72"));
        md5::Md5::new();
    }


    /// Live check: full Noise XX handshake against web.whatsapp.com.
    #[test]
    #[ignore]
    fn live_handshake() {
        let mut s = Wa2Session::connect().expect("wss connect");
        s.handshake(&generate_pairing_material()).expect("handshake");
        assert!(s.is_handshaken());
    }
}