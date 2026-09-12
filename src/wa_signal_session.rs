//! M3 — Signal session message crypto (X3DH + double ratchet + wire
//! messages), ported from libsignal-go (Johnkhk fork)
//! `protocol/{session,ratchet,message,crypto/aes}`.
//!
//! Verification: `tests/data/signal_golden.json` was produced by a
//! deterministic two-party Go harness (`cmd_signal_golden`) that establishes a
//! full session (Alice bundle → pre-key messages → Bob decrypt → ratchet reply
//! → ack) and dumps every intermediate session state and ciphertext. The Rust
//! port replays the exact same keys/random-reader and must reproduce those
//! bytes byte-for-byte (`two_party_flow_matches_go_golden`).
//!
//! KEY NOTE: libsignal public keys are 33 bytes — `0x05` (DJB type prefix) +
//! 32-byte Montgomery u. Private keys are the raw 32 bytes. This holds for
//! every key that appears in a session structure, wire message, or MAC input.
//! The X25519 math itself consumes only the trailing 32 bytes.

#![allow(dead_code)]

use crate::wa2::{pb, Error};
use crate::wa_signal::{Chain, PendingPreKey, SessionRecord, SessionStructure};
use crate::wa_signal::ChainKey as StChainKey;
use crate::wa_signal::MessageKey as StMessageKey;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;
use x25519_dalek::{PublicKey as X25519Pub, StaticSecret as X25519Secret};

type HmacSha256 = Hmac<Sha256>;

/// A libsignal public key: type byte 0x05 + 32-byte Montgomery u.
pub type SignalPub = [u8; 33];
/// A raw X25519 private key.
pub type SignalPriv = [u8; 32];

fn unbind(pub_key: &SignalPub) -> Result<&[u8; 32], Error> {
    if pub_key.len() != 33 || pub_key[0] != 0x05 {
        return Err(Error::Noise("bad public key prefix".into()));
    }
    pub_key[1..].try_into().map_err(|_| Error::Noise("bad public key".into()))
}

fn bind(raw: [u8; 32]) -> SignalPub {
    let mut k = [0u8; 33];
    k[0] = 0x05;
    k[1..].copy_from_slice(&raw);
    k
}

// ─────────────────────────────────────────────────────────────────────────────
// Wire constants & messages (libsignal `message/{signal,prekey,ciphertext}.go`)
// ─────────────────────────────────────────────────────────────────────────────

pub const CIPHERTEXT_VERSION: u8 = 3;
pub const MAC_SIZE: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiphertextType {
    Whisper,   // 2
    PreKey,    // 3
    SenderKey, // 7
    Plaintext, // 8
}

impl CiphertextType {
    pub fn from_code(code: i32) -> CiphertextType {
        match code {
            3 => CiphertextType::PreKey,
            7 => CiphertextType::SenderKey,
            8 => CiphertextType::Plaintext,
            _ => CiphertextType::Whisper,
        }
    }
    pub fn code(self) -> i32 {
        match self {
            CiphertextType::Whisper => 2,
            CiphertextType::PreKey => 3,
            CiphertextType::SenderKey => 7,
            CiphertextType::Plaintext => 8,
        }
    }
}

/// An encrypted `SignalMessage` (v1.SignalMessage + version prefix + MAC).
#[derive(Debug, Clone, PartialEq)]
pub struct SignalMsg {
    pub version: u8,
    pub sender_ratchet_key: SignalPub,
    pub previous_counter: u32,
    pub counter: u32,
    pub ciphertext: Vec<u8>,
    pub serialized: Vec<u8>,
}

fn mac_bytes(
    mac_key: &[u8],
    sender_identity: &SignalPub,
    receiver_identity: &SignalPub,
    serialized: &[u8],
) -> Result<[u8; MAC_SIZE], Error> {
    if mac_key.len() != 32 {
        return Err(Error::Noise("mac key must be 32 bytes".into()));
    }
    let mut mac = HmacSha256::new_from_slice(mac_key)
        .map_err(|e| Error::Noise(format!("hmac: {e}")))?;
    mac.update(sender_identity);
    mac.update(receiver_identity);
    mac.update(serialized);
    let out = mac.finalize().into_bytes();
    let mut r = [0u8; MAC_SIZE];
    r.copy_from_slice(&out[..MAC_SIZE]);
    Ok(r)
}

/// SignalMessage wire body: ratchet_key(f1) counter(f2) previous_counter(f3)
/// ciphertext(f4) — proto2, always-present fields, field-number order.
fn signal_body(
    ratchet_key: &SignalPub,
    counter: u32,
    previous_counter: u32,
    ciphertext: &[u8],
) -> Vec<u8> {
    let mut b = Vec::new();
    pb::field_bytes(1, ratchet_key, &mut b);
    pb::field_uint(2, counter as u64, &mut b);
    pb::field_uint(3, previous_counter as u64, &mut b);
    pb::field_bytes(4, ciphertext, &mut b);
    b
}

impl SignalMsg {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        version: u8,
        mac_key: &[u8],
        sender_ratchet_key: &SignalPub,
        previous_counter: u32,
        counter: u32,
        ciphertext: Vec<u8>,
        sender_identity_key: &SignalPub,
        receiver_identity_key: &SignalPub,
    ) -> Result<SignalMsg, Error> {
        let body = signal_body(sender_ratchet_key, counter, previous_counter, &ciphertext);
        let version_prefix = ((version & 0xF) << 4) | CIPHERTEXT_VERSION;
        let mut serialized = Vec::with_capacity(1 + body.len() + MAC_SIZE);
        serialized.push(version_prefix);
        serialized.extend_from_slice(&body);
        let mac = mac_bytes(mac_key, sender_identity_key, receiver_identity_key, &serialized)?;
        serialized.extend_from_slice(&mac);
        Ok(SignalMsg {
            version,
            sender_ratchet_key: *sender_ratchet_key,
            previous_counter,
            counter,
            ciphertext,
            serialized,
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<SignalMsg, Error> {
        if bytes.is_empty() {
            return Err(Error::Noise("message too short".into()));
        }
        let version = bytes[0] >> 4;
        if version != CIPHERTEXT_VERSION {
            return Err(Error::Noise(format!(
                "unsupported message version: {version} != {CIPHERTEXT_VERSION}"
            )));
        }
        if bytes.len() < 1 + MAC_SIZE {
            return Err(Error::Noise("message too short".into()));
        }
        let body = &bytes[1..bytes.len() - MAC_SIZE];
        let fields = pb::parse(body)?;
        let ratchet: SignalPub = pb::find_bytes(&fields, 1)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| Error::Noise("bad ratchet key".into()))?;
        let counter = pb::find_var(&fields, 2).unwrap_or_default() as u32;
        let previous = pb::find_var(&fields, 3).unwrap_or_default() as u32;
        let ciphertext = pb::find_bytes(&fields, 4).unwrap_or_default().to_vec();
        Ok(SignalMsg {
            version,
            sender_ratchet_key: ratchet,
            previous_counter: previous,
            counter,
            ciphertext,
            serialized: bytes.to_vec(),
        })
    }

    pub fn verify_mac(
        &self,
        mac_key: &[u8],
        sender_identity_key: &SignalPub,
        receiver_identity_key: &SignalPub,
    ) -> Result<bool, Error> {
        let their_mac = &self.serialized[self.serialized.len() - MAC_SIZE..];
        let our_mac = mac_bytes(
            mac_key,
            sender_identity_key,
            receiver_identity_key,
            &self.serialized[..self.serialized.len() - MAC_SIZE],
        )?;
        Ok(their_mac == our_mac)
    }
}

/// A `PreKeySignalMessage` wrapping an inner SignalMessage.
#[derive(Debug, Clone, PartialEq)]
pub struct PreKeyMsg {
    pub version: u8,
    pub registration_id: u32,
    pub pre_key_id: Option<u32>,
    pub signed_pre_key_id: u32,
    pub base_key: SignalPub,
    pub identity_key: SignalPub,
    pub message: SignalMsg,
    pub serialized: Vec<u8>,
}

impl PreKeyMsg {
    pub fn new(
        version: u8,
        registration_id: u32,
        pre_key_id: Option<u32>,
        signed_pre_key_id: u32,
        base_key: &SignalPub,
        identity_key: &SignalPub,
        message: &SignalMsg,
    ) -> Result<PreKeyMsg, Error> {
        // proto2 explicit presence: pre_key_id omitted when absent.
        let mut body = Vec::new();
        if let Some(id) = pre_key_id {
            pb::field_uint(1, id as u64, &mut body);
        }
        pb::field_bytes(2, base_key, &mut body);
        pb::field_bytes(3, identity_key, &mut body);
        pb::field_bytes(4, &message.serialized, &mut body);
        pb::field_uint(5, registration_id as u64, &mut body);
        pb::field_uint(6, signed_pre_key_id as u64, &mut body);

        let version_prefix = ((version & 0xF) << 4) | CIPHERTEXT_VERSION;
        let mut serialized = Vec::with_capacity(1 + body.len());
        serialized.push(version_prefix);
        serialized.extend_from_slice(&body);
        Ok(PreKeyMsg {
            version,
            registration_id,
            pre_key_id,
            signed_pre_key_id,
            base_key: *base_key,
            identity_key: *identity_key,
            message: message.clone(),
            serialized,
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<PreKeyMsg, Error> {
        if bytes.is_empty() {
            return Err(Error::Noise("message too short".into()));
        }
        let version = bytes[0] >> 4;
        if version != CIPHERTEXT_VERSION {
            return Err(Error::Noise(format!(
                "unsupported message version: {version} != {CIPHERTEXT_VERSION}"
            )));
        }
        let fields = pb::parse(&bytes[1..])?;
        let base_key: SignalPub = pb::find_bytes(&fields, 2)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| Error::Noise("bad base key".into()))?;
        let identity_key: SignalPub = pb::find_bytes(&fields, 3)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| Error::Noise("bad identity key".into()))?;
        let inner = pb::find_bytes(&fields, 4)
            .ok_or_else(|| Error::Noise("missing inner signal message".into()))?;
        let message = SignalMsg::from_bytes(inner)?;
        let registration_id = pb::find_var(&fields, 5).unwrap_or_default() as u32;
        let signed_pre_key_id = pb::find_var(&fields, 6).unwrap_or_default() as u32;
        let pre_key_id = pb::find_var(&fields, 1).map(|v| v as u32);
        Ok(PreKeyMsg {
            version,
            registration_id,
            pre_key_id,
            signed_pre_key_id,
            base_key,
            identity_key,
            message,
            serialized: bytes.to_vec(),
        })
    }
}

/// Any encrypted message we can produce/consume.
#[derive(Debug, Clone, PartialEq)]
pub enum CiphertextMsg {
    Signal(SignalMsg),
    PreKey(PreKeyMsg),
}

impl CiphertextMsg {
    pub fn msg_type(&self) -> CiphertextType {
        match self {
            CiphertextMsg::Signal(_) => CiphertextType::Whisper,
            CiphertextMsg::PreKey(_) => CiphertextType::PreKey,
        }
    }
    pub fn bytes(&self) -> &[u8] {
        match self {
            CiphertextMsg::Signal(s) => &s.serialized,
            CiphertextMsg::PreKey(p) => &p.serialized,
        }
    }
    pub fn from_bytes(bytes: &[u8], typ: CiphertextType) -> Result<CiphertextMsg, Error> {
        match typ {
            CiphertextType::PreKey => Ok(CiphertextMsg::PreKey(PreKeyMsg::from_bytes(bytes)?)),
            _ => Ok(CiphertextMsg::Signal(SignalMsg::from_bytes(bytes)?)),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Curve25519 helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a keypair from seed bytes, mirroring libsignal `NewPrivateKey`
/// (clamped, stored clamped, public = 0x05-prefixed Montgomery u).
pub fn keypair_from_seed(seed: &SignalPriv) -> (SignalPriv, SignalPub) {
    let mut privk = *seed;
    privk[0] &= 248;
    privk[31] &= 63;
    privk[31] |= 64;
    let secret = X25519Secret::from(privk);
    let raw: [u8; 32] = *X25519Pub::from(&secret).as_bytes();
    (privk, bind(raw))
}

/// Generate a fresh keypair from `rnd` (32 bytes).
pub fn gen_keypair<R: RngCore>(rnd: &mut R) -> (SignalPriv, SignalPub) {
    let mut seed = [0u8; 32];
    rnd.fill_bytes(&mut seed);
    keypair_from_seed(&seed)
}

fn agreement(privk: &SignalPriv, pub_key: &SignalPub) -> Result<[u8; 32], Error> {
    let secret = X25519Secret::from(*privk);
    let shared = secret.diffie_hellman(&X25519Pub::from(*unbind(pub_key)?));
    Ok(*shared.as_bytes())
}

// ─────────────────────────────────────────────────────────────────────────────
// Ratchet keys (libsignal `ratchet/keys.go`)
// ─────────────────────────────────────────────────────────────────────────────

const MESSAGE_KEYS_INFO: &[u8] = b"WhisperMessageKeys";
const ROOT_INFO: &[u8] = b"WhisperRatchet";
const INIT_ROOT_INFO: &[u8] = b"WhisperText";
const CHAIN_KEY_SEED: &[u8] = &[0x02];
const MESSAGE_KEY_SEED: &[u8] = &[0x01];

#[derive(Debug, Clone, PartialEq)]
pub struct MessageKeys {
    cipher_key: [u8; 32],
    mac_key: [u8; 32],
    iv: [u8; 16],
    counter: u32,
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac accepts any key len");
    mac.update(data);
    let out = mac.finalize().into_bytes();
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

fn hkdf(salt: &[u8], ikm: &[u8], info: &[u8], out_len: usize) -> Vec<u8> {
    crate::wa2::hkdf_sha256(salt, ikm, info, out_len)
}

fn derive_message_keys(input: &[u8], counter: u32) -> MessageKeys {
    let out = hkdf(&[], input, MESSAGE_KEYS_INFO, 80);
    let mut cipher_key = [0u8; 32];
    let mut mac_key = [0u8; 32];
    let mut iv = [0u8; 16];
    cipher_key.copy_from_slice(&out[..32]);
    mac_key.copy_from_slice(&out[32..64]);
    iv.copy_from_slice(&out[64..80]);
    MessageKeys { cipher_key, mac_key, iv, counter }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChainKey {
    key: [u8; 32],
    index: u32,
}

impl ChainKey {
    fn next(&self) -> ChainKey {
        ChainKey { key: hmac_sha256(&self.key, CHAIN_KEY_SEED), index: self.index + 1 }
    }

    fn message_keys(&self) -> MessageKeys {
        derive_message_keys(&hmac_sha256(&self.key, MESSAGE_KEY_SEED), self.index)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RootKey {
    key: [u8; 32],
}

impl RootKey {
    fn create_chain(
        &self,
        our_ratchet_priv: &SignalPriv,
        their_ratchet_pub: &SignalPub,
    ) -> Result<(RootKey, ChainKey), Error> {
        let shared = agreement(our_ratchet_priv, their_ratchet_pub)?;
        let out = hkdf(&self.key, &shared, ROOT_INFO, 64);
        let mut root = [0u8; 32];
        let mut chain = [0u8; 32];
        root.copy_from_slice(&out[..32]);
        chain.copy_from_slice(&out[32..]);
        Ok((RootKey { key: root }, ChainKey { key: chain, index: 0 }))
    }
}

/// X3DH master secret → root key + initial chain key (`ratchet.DeriveKeys`).
fn derive_root_keys(secret: &[u8]) -> (RootKey, ChainKey) {
    let out = hkdf(&[], secret, INIT_ROOT_INFO, 64);
    let mut root = [0u8; 32];
    let mut chain = [0u8; 32];
    root.copy_from_slice(&out[..32]);
    chain.copy_from_slice(&out[32..]);
    (RootKey { key: root }, ChainKey { key: chain, index: 0 })
}

// ─────────────────────────────────────────────────────────────────────────────
// AES-256-CBC + PKCS#7 (libsignal `crypto/aes/cbc.go`)
// ─────────────────────────────────────────────────────────────────────────────

fn aes_cbc_encrypt(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, Error> {
    use aes::cipher::{BlockEncrypt, KeyInit as _};
    if iv.len() != 16 {
        return Err(Error::Noise("iv must be 16 bytes".into()));
    }
    if key.len() != 32 {
        return Err(Error::Noise("key must be 32 bytes".into()));
    }
    let cipher = aes::Aes256::new_from_slice(key).map_err(|e| Error::Noise(format!("aes: {e}")))?;
    let padded = pkcs7_pad(plaintext);
    let mut out = Vec::with_capacity(padded.len());
    let mut prev = iv.to_vec();
    for chunk in padded.chunks(16) {
        let mut block: [u8; 16] = chunk.try_into().unwrap();
        for i in 0..16 {
            block[i] ^= prev[i];
        }
        cipher.encrypt_block((&mut block).into());
        out.extend_from_slice(&block);
        prev.copy_from_slice(&block);
    }
    Ok(out)
}

fn pkcs7_pad(plaintext: &[u8]) -> Vec<u8> {
    let n = 16 - (plaintext.len() % 16);
    let mut out = plaintext.to_vec();
    out.extend(std::iter::repeat_n(n as u8, n));
    out
}

fn aes_cbc_decrypt(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, Error> {
    use aes::cipher::{BlockDecrypt, KeyInit as _};
    if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(16) {
        return Err(Error::Noise(
            "ciphertext length must be a non-zero multiple of the block size".into(),
        ));
    }
    if iv.len() != 16 {
        return Err(Error::Noise("iv must be 16 bytes".into()));
    }
    if key.len() != 32 {
        return Err(Error::Noise("key must be 32 bytes".into()));
    }
    let cipher = aes::Aes256::new_from_slice(key).map_err(|e| Error::Noise(format!("aes: {e}")))?;
    let mut out = Vec::with_capacity(ciphertext.len());
    let mut prev = iv.to_vec();
    for chunk in ciphertext.chunks(16) {
        let mut block: [u8; 16] = chunk.try_into().unwrap();
        cipher.decrypt_block((&mut block).into());
        for i in 0..16 {
            block[i] ^= prev[i];
        }
        out.extend_from_slice(&block);
        prev.copy_from_slice(chunk);
    }
    Ok(pkcs7_unpad(&out)?.to_vec())
}

fn pkcs7_unpad(plaintext: &[u8]) -> Result<&[u8], Error> {
    let length = plaintext.len();
    if length == 0 {
        return Err(Error::Noise("invalid padding".into()));
    }
    let n = plaintext[length - 1] as usize;
    if !(1..=16).contains(&n) || n > length {
        return Err(Error::Noise("invalid padding".into()));
    }
    if !plaintext[length - n..].iter().all(|&b| b == n as u8) {
        return Err(Error::Noise("invalid padding".into()));
    }
    Ok(&plaintext[..length - n])
}

// ─────────────────────────────────────────────────────────────────────────────
// Session state operations (libsignal `session/state.go`)
// ─────────────────────────────────────────────────────────────────────────────

const MAX_RECEIVER_CHAINS: usize = 5;
const MAX_MESSAGE_KEYS: usize = 2000;

const DISCONTINUITY_BYTES: [u8; 32] = [0xFF; 32];

fn to_pub(v: &[u8]) -> Result<SignalPub, Error> {
    v.try_into().map_err(|_| Error::Noise("bad public key".into()))
}

fn to_priv(v: &[u8]) -> Result<SignalPriv, Error> {
    v.try_into().map_err(|_| Error::Noise("bad private key".into()))
}

impl SessionStructure {
    pub fn session_version(&self) -> u32 {
        if self.session_version == 0 {
            2
        } else {
            self.session_version
        }
    }

    fn root_key(&self) -> RootKey {
        RootKey { key: self.root_key.clone().try_into().unwrap_or([0u8; 32]) }
    }

    fn set_root_key(&mut self, key: RootKey) {
        self.root_key = key.key.to_vec();
    }

    fn sender_ratchet_private(&self) -> Result<SignalPriv, Error> {
        self.sender_chain
            .as_ref()
            .ok_or_else(|| Error::Noise("missing sender chain".into()))?
            .sender_ratchet_key_private
            .as_slice()
            .try_into()
            .map_err(|_| Error::Noise("bad sender ratchet private key".into()))
    }

    fn sender_ratchet_key(&self) -> Result<SignalPub, Error> {
        to_pub(&self.sender_chain.as_ref().ok_or_else(|| Error::Noise("missing sender chain".into()))?.sender_ratchet_key)
    }

    /// Index of the receiver chain keyed by `sender`'s ratchet public key.
    fn receiver_chain_index(&self, sender: &SignalPub) -> Option<usize> {
        self.receiver_chains.iter().position(|c| c.sender_ratchet_key.as_slice() == sender)
    }

    fn receiver_chain_key(&self, sender: &SignalPub) -> Result<Option<ChainKey>, Error> {
        let idx = self.receiver_chain_index(sender);
        let Some(idx) = idx else { return Ok(None) };
        let ck = self.receiver_chains[idx]
            .chain_key
            .as_ref()
            .ok_or_else(|| Error::Noise("missing chain key".into()))?;
        Ok(Some(ChainKey {
            key: to_priv(&ck.key)?,
            index: ck.index,
        }))
    }

    fn set_receiver_chain_key(&mut self, sender: &SignalPub, chain_key: ChainKey) -> Result<(), Error> {
        let idx = self
            .receiver_chain_index(sender)
            .ok_or_else(|| Error::Noise("SetReceiverChainKey called for non-existent chain".into()))?;
        self.receiver_chains[idx].chain_key = Some(StChainKey {
            index: chain_key.index,
            key: chain_key.key.to_vec(),
        });
        Ok(())
    }

    fn add_receiver_chain(&mut self, sender: &SignalPub, chain_key: &ChainKey) {
        self.receiver_chains.push(Chain {
            sender_ratchet_key: sender.to_vec(),
            chain_key: Some(StChainKey { index: chain_key.index, key: chain_key.key.to_vec() }),
            ..Default::default()
        });
        if self.receiver_chains.len() > MAX_RECEIVER_CHAINS {
            self.receiver_chains.remove(0);
        }
    }

    fn set_sender_chain(&mut self, sender_priv: &SignalPriv, sender_pub: &SignalPub, chain_key: &ChainKey) {
        self.sender_chain = Some(Chain {
            sender_ratchet_key: sender_pub.to_vec(),
            sender_ratchet_key_private: sender_priv.to_vec(),
            chain_key: Some(StChainKey { index: chain_key.index, key: chain_key.key.to_vec() }),
            ..Default::default()
        });
    }

    fn sender_chain_key(&self) -> Result<ChainKey, Error> {
        let chain = self.sender_chain.as_ref().ok_or_else(|| Error::Noise("missing sender chain".into()))?;
        let ck = chain.chain_key.as_ref().ok_or_else(|| Error::Noise("missing sender chain key".into()))?;
        Ok(ChainKey {
            key: to_priv(&ck.key)?,
            index: ck.index,
        })
    }

    fn set_sender_chain_key(&mut self, next: &ChainKey) {
        let ck = StChainKey { index: next.index, key: next.key.to_vec() };
        if let Some(chain) = &mut self.sender_chain {
            chain.chain_key = Some(ck);
        } else {
            self.sender_chain = Some(Chain { chain_key: Some(ck), ..Default::default() });
        }
    }

    fn set_message_keys(&mut self, sender: &SignalPub, mk: &MessageKeys) -> Result<(), Error> {
        let idx = self
            .receiver_chain_index(sender)
            .ok_or_else(|| Error::Noise("SetMessageKeys called for non-existent chain".into()))?;
        self.receiver_chains[idx].message_keys.push(StMessageKey {
            index: mk.counter,
            cipher_key: mk.cipher_key.to_vec(),
            mac_key: mk.mac_key.to_vec(),
            iv: mk.iv.to_vec(),
        });
        let msgs = &mut self.receiver_chains[idx].message_keys;
        if msgs.len() > MAX_MESSAGE_KEYS {
            msgs.remove(0);
        }
        Ok(())
    }

    /// Remove-and-return the message key for `counter` from the matching
    /// receiver chain (mirrors `State.MessageKeys`).
    fn take_message_keys(&mut self, sender: &SignalPub, counter: u32) -> Result<Option<MessageKeys>, Error> {
        let Some(idx) = self.receiver_chain_index(sender) else { return Ok(None) };
        let chain = &mut self.receiver_chains[idx];
        let mut found = None;
        let mut keep = Vec::with_capacity(chain.message_keys.len());
        for mk in chain.message_keys.drain(..) {
            if mk.index == counter {
                found = Some(MessageKeys {
                    cipher_key: to_priv(&mk.cipher_key)?,
                    mac_key: to_priv(&mk.mac_key)?,
                    iv: mk.iv.as_slice().try_into().map_err(|_| Error::Noise("bad iv".into()))?,
                    counter,
                });
            } else {
                keep.push(mk);
            }
        }
        chain.message_keys = keep;
        Ok(found)
    }

    fn set_unacknowledged_pre_key_message(&mut self, pre_key_id: Option<u32>, signed_pre_key_id: u32, base_key: &SignalPub) {
        let mut pending = PendingPreKey {
            signed_pre_key_id,
            base_key: base_key.to_vec(),
            ..Default::default()
        };
        if let Some(id) = pre_key_id {
            pending.pre_key_id = id;
        }
        self.pending_pre_key = Some(pending);
    }

    fn clear_unacknowledged_pre_key_message(&mut self) {
        self.pending_pre_key = None;
    }

    fn unacknowledged_pre_key_message(&self) -> Result<Option<(Option<u32>, u32, SignalPub)>, Error> {
        let Some(p) = &self.pending_pre_key else { return Ok(None) };
        let base = to_pub(&p.base_key)?;
        let pre_key_id = if p.pre_key_id != 0 { Some(p.pre_key_id) } else { None };
        Ok(Some((pre_key_id, p.signed_pre_key_id, base)))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// X3DH session establishment (libsignal `session/session.go`)
// ─────────────────────────────────────────────────────────────────────────────

pub struct AliceParams<'a> {
    pub our_identity_priv: SignalPriv,
    pub our_identity_pub: SignalPub,
    pub our_base_priv: SignalPriv,
    pub our_base_pub: SignalPub,
    pub their_identity: &'a SignalPub,
    pub their_signed_prekey: &'a SignalPub,
    pub their_one_time_prekey: Option<&'a SignalPub>,
    pub their_ratchet_key: &'a SignalPub,
}

pub struct BobParams<'a> {
    pub our_identity_priv: SignalPriv,
    pub our_identity_pub: SignalPub,
    pub our_signed_prekey_priv: &'a SignalPriv,
    pub our_signed_prekey_pub: &'a SignalPub,
    pub our_one_time_prekey_priv: Option<&'a SignalPriv>,
    pub their_identity: &'a SignalPub,
    pub their_base_key: &'a SignalPub,
}

pub fn initialize_alice_session<R: RngCore>(rnd: &mut R, params: &AliceParams) -> Result<SessionStructure, Error> {
    let (sending_priv, sending_pub) = gen_keypair(rnd);

    let dh1 = agreement(&params.our_identity_priv, params.their_signed_prekey)?;
    let dh2 = agreement(&params.our_base_priv, params.their_identity)?;
    let dh3 = agreement(&params.our_base_priv, params.their_signed_prekey)?;

    let mut secrets = Vec::with_capacity(160);
    secrets.extend_from_slice(&DISCONTINUITY_BYTES);
    secrets.extend_from_slice(&dh1);
    secrets.extend_from_slice(&dh2);
    secrets.extend_from_slice(&dh3);
    if let Some(otp) = params.their_one_time_prekey {
        secrets.extend_from_slice(&agreement(&params.our_base_priv, otp)?);
    }

    let (root_key, chain_key) = derive_root_keys(&secrets);
    let (sending_root, sending_chain) = root_key.create_chain(&sending_priv, params.their_ratchet_key)?;

    let mut session = SessionStructure {
        session_version: CIPHERTEXT_VERSION as u32,
        local_identity_public: params.our_identity_pub.to_vec(),
        remote_identity_public: params.their_identity.to_vec(),
        root_key: sending_root.key.to_vec(),
        ..Default::default()
    };
    session.add_receiver_chain(params.their_ratchet_key, &chain_key);
    session.set_sender_chain(&sending_priv, &sending_pub, &sending_chain);
    Ok(session)
}

pub fn initialize_bob_session(params: &BobParams) -> Result<SessionStructure, Error> {
    let dh1 = agreement(params.our_signed_prekey_priv, params.their_identity)?;
    let dh2 = agreement(&params.our_identity_priv, params.their_base_key)?;
    let dh3 = agreement(params.our_signed_prekey_priv, params.their_base_key)?;

    let mut secrets = Vec::with_capacity(160);
    secrets.extend_from_slice(&DISCONTINUITY_BYTES);
    secrets.extend_from_slice(&dh1);
    secrets.extend_from_slice(&dh2);
    secrets.extend_from_slice(&dh3);
    if let Some(otp) = params.our_one_time_prekey_priv {
        secrets.extend_from_slice(&agreement(otp, params.their_base_key)?);
    }

    let (root_key, chain_key) = derive_root_keys(&secrets);

    let mut session = SessionStructure {
        session_version: CIPHERTEXT_VERSION as u32,
        local_identity_public: params.our_identity_pub.to_vec(),
        remote_identity_public: params.their_identity.to_vec(),
        root_key: root_key.key.to_vec(),
        ..Default::default()
    };
    // Bob's ratchet pair = his signed pre-key pair.
    session.set_sender_chain(params.our_signed_prekey_priv, params.our_signed_prekey_pub, &chain_key);
    Ok(session)
}

// ─────────────────────────────────────────────────────────────────────────────
// Session cipher (libsignal `session/cipher.go`)
// ─────────────────────────────────────────────────────────────────────────────

const MAX_JUMPS: u32 = 25_000;

/// Keys needed on the "Bob" side to process an incoming PreKey message.
pub struct LocalKeys<'a> {
    pub identity_priv: SignalPriv,
    pub identity_pub: SignalPub,
    pub signed_prekey: Option<(u32, SignalPriv, SignalPub)>,  // (id, priv, pub)
    pub one_time_prekeys: &'a [(u32, SignalPriv, SignalPub)], // (id, priv, pub)
    pub local_reg_id: u32,
}

/// Everything a fetched pre-key bundle gives an outbound ("Alice") initiator.
pub struct AliceBundle<'a> {
    pub their_identity_pub: &'a [u8; 32],
    pub their_signed_prekey: (&'a [u8; 32], &'a [u8; 64]),
    pub their_signed_prekey_id: u32,
    pub their_one_time_prekey: Option<(u32, &'a [u8; 32])>,
    pub their_registration_id: u32,
    pub our_registration_id: u32,
}

/// Initialize an Alice session from a fetched bundle, mirroring the fork's
/// `Session.ProcessPreKeyBundle` (trust store skipped for now). Returns the
/// (to-be-uploaded) record and the one-time pre-key id it references, if any.
pub fn initialize_from_bundle<R: RngCore>(
    rnd: &mut R,
    init: &AliceBundle,
    our_identity: (SignalPriv, SignalPub),
) -> Result<(SessionRecord, Option<u32>), Error> {
    let bind_key = |k: &[u8; 32]| bind(*k);

    let their_identity = bind_key(init.their_identity_pub);
    let their_signed_pub = bind_key(init.their_signed_prekey.0);
    if !crate::wa2::xed25519_verify(
        init.their_identity_pub,
        init.their_signed_prekey.1,
        &their_signed_pub,
    ) {
        return Err(Error::Noise("signature validation failed".into()));
    }
    let otpk_pub = init.their_one_time_prekey.as_ref().map(|(_, k)| bind_key(k));

    let (base_priv, base_pub) = gen_keypair(rnd);
    let alice = AliceParams {
        our_identity_priv: our_identity.0,
        our_identity_pub: our_identity.1,
        our_base_priv: base_priv,
        our_base_pub: base_pub,
        their_identity: &their_identity,
        their_signed_prekey: &their_signed_pub,
        their_one_time_prekey: otpk_pub.as_ref(),
        their_ratchet_key: &their_signed_pub,
    };
    let mut state = initialize_alice_session(rnd, &alice)?;
    state.local_registration_id = init.our_registration_id;
    state.remote_registration_id = init.their_registration_id;
    state.alice_base_key = base_pub.to_vec();
    let otpk_id = init.their_one_time_prekey.map(|(id, _)| id);
    state.set_unacknowledged_pre_key_message(otpk_id, init.their_signed_prekey_id, &base_pub);
    let record = SessionRecord::new(Some(state));
    Ok((record, otpk_id))
}

/// Init a Bob session from an incoming PreKey message (mirrors
/// `Session.ProcessPreKey`/`processPreKeyV3` minus identity trust checks).
/// Returns the consumed one-time pre-key id, if any.
pub fn process_prekey(
    record: &mut SessionRecord,
    msg: &PreKeyMsg,
    keys: &LocalKeys,
) -> Result<Option<u32>, Error> {
    if record.has_session_state(msg.version as u32, &msg.base_key) {
        return Ok(None);
    }
    let (_, signed_priv, signed_pub) = keys
        .signed_prekey
        .as_ref()
        .ok_or_else(|| Error::Noise("missing signed pre-key".into()))?;
    // The fork loads the one-time pre-key by the id carried in the message.
    let otp = msg.pre_key_id.and_then(|id| {
        keys.one_time_prekeys
            .iter()
            .find(|(kid, _, _)| *kid == id)
    });
    let bob = BobParams {
        our_identity_priv: keys.identity_priv,
        our_identity_pub: keys.identity_pub,
        our_signed_prekey_priv: signed_priv,
        our_signed_prekey_pub: signed_pub,
        our_one_time_prekey_priv: otp.map(|(_, p, _)| p),
        their_identity: &msg.identity_key,
        their_base_key: &msg.base_key,
    };
    let mut session = initialize_bob_session(&bob)?;
    session.local_registration_id = keys.local_reg_id;
    session.remote_registration_id = msg.registration_id;
    session.alice_base_key = msg.base_key.to_vec();
    record.promote_state(session);
    Ok(msg.pre_key_id)
}

/// Encrypt a plaintext for the session's remote party. If the state still has
/// an unacknowledged pre-key message the result is wrapped in a PreKeyMessage.
pub fn encrypt_message<R: RngCore>(
    _rnd: &mut R,
    record: &mut SessionRecord,
    plaintext: &[u8],
) -> Result<CiphertextMsg, Error> {
    let state = record
        .state_mut()
        .ok_or_else(|| Error::Noise("no current session".into()))?;

    let chain_key = state.sender_chain_key()?;
    let message_keys = chain_key.message_keys();
    let sender_ephemeral = state.sender_ratchet_key()?;
    let previous_counter = state.previous_counter;
    let version = state.session_version() as u8;

    let local_identity = to_pub(&state.local_identity_public)?;
    let their_identity = to_pub(&state.remote_identity_public)?;

    let ciphertext = aes_cbc_encrypt(&message_keys.cipher_key, &message_keys.iv, plaintext)?;
    let sig = SignalMsg::new(
        version,
        &message_keys.mac_key,
        &sender_ephemeral,
        previous_counter,
        chain_key.index,
        ciphertext,
        &local_identity,
        &their_identity,
    )?;

    let msg = if let Some((pre_key_id, signed_pre_key_id, base_key)) =
        state.unacknowledged_pre_key_message()?
    {
        CiphertextMsg::PreKey(PreKeyMsg::new(
            version,
            state.local_registration_id,
            pre_key_id,
            signed_pre_key_id,
            &base_key,
            &local_identity,
            &sig,
        )?)
    } else {
        CiphertextMsg::Signal(sig)
    };

    state.set_sender_chain_key(&chain_key.next());
    Ok(msg)
}

/// Decrypt a message (PreKey or Signal), trying current + archived states.
pub fn decrypt_message<R: RngCore>(
    rnd: &mut R,
    record: &mut SessionRecord,
    ciphertext: &CiphertextMsg,
    keys: &LocalKeys,
) -> Result<Vec<u8>, Error> {
    decrypt_message_consumed(rnd, record, ciphertext, keys).map(|(pt, _)| pt)
}

/// Like [`decrypt_message`], but also reports the one-time pre-key id the
/// message consumed (if any) so the caller can remove it from its store.
pub fn decrypt_message_consumed<'a, R: RngCore>(
    rnd: &mut R,
    record: &mut SessionRecord,
    ciphertext: &CiphertextMsg,
    keys: &LocalKeys<'a>,
) -> Result<(Vec<u8>, Option<u32>), Error> {
    match ciphertext {
        CiphertextMsg::PreKey(p) => {
            let consumed = process_prekey(record, p, keys)?;
            let plaintext = decrypt_record_loop(rnd, record, p.message.clone())?;
            Ok((plaintext, consumed))
        }
        CiphertextMsg::Signal(s) => {
            if record.state().is_none() {
                return Err(Error::Noise("session not found".into()));
            }
            Ok((decrypt_record_loop(rnd, record, s.clone())?, None))
        }
    }
}

fn decrypt_record_loop<R: RngCore>(
    rnd: &mut R,
    record: &mut SessionRecord,
    ciphertext: SignalMsg,
) -> Result<Vec<u8>, Error> {
    if let Some(current) = record.state().cloned() {
        let mut state = current;
        match decrypt_message_session(rnd, &mut state, &ciphertext) {
            Ok(plaintext) => {
                record.set_session_state(state);
                return Ok(plaintext);
            }
            Err(e) if is_duplicate(&e) => return Err(e),
            Err(_) => {}
        }
    }

    let mut success: Option<(usize, SessionStructure, Vec<u8>)> = None;
    for (i, mut prev) in record.previous_states().into_iter().enumerate() {
        match decrypt_message_session(rnd, &mut prev, &ciphertext) {
            Ok(plaintext) => {
                success = Some((i, prev, plaintext));
                break;
            }
            Err(e) if is_duplicate(&e) => return Err(e),
            Err(_) => {}
        }
    }

    if let Some((idx, state, plaintext)) = success {
        record.promote_old_state(idx, state);
        return Ok(plaintext);
    }
    Err(Error::Noise("decryption failed: invalid message".into()))
}

fn decrypt_message_session<R: RngCore>(
    rnd: &mut R,
    state: &mut SessionStructure,
    ciphertext: &SignalMsg,
) -> Result<Vec<u8>, Error> {
    if state.sender_chain.is_none() {
        return Err(Error::Noise("no session available to decrypt".into()));
    }
    if ciphertext.version as u32 != state.session_version() {
        return Err(Error::Noise(format!(
            "unrecognized message version: {}",
            ciphertext.version
        )));
    }

    let their_ephemeral = ciphertext.sender_ratchet_key;
    let counter = ciphertext.counter;
    let chain_key = chain_key_for(rnd, state, &their_ephemeral)?;
    let message_keys = message_keys_for(state, &their_ephemeral, &chain_key, counter)?;

    let their_identity = to_pub(&state.remote_identity_public)?;
    let local_identity = to_pub(&state.local_identity_public)?;

    let valid = ciphertext.verify_mac(&message_keys.mac_key, &their_identity, &local_identity)?;
    if !valid {
        return Err(Error::Noise("MAC verification failed".into()));
    }

    let plaintext =
        aes_cbc_decrypt(&message_keys.cipher_key, &message_keys.iv, &ciphertext.ciphertext)?;
    state.clear_unacknowledged_pre_key_message();
    Ok(plaintext)
}

fn chain_key_for<R: RngCore>(
    rnd: &mut R,
    state: &mut SessionStructure,
    their_ephemeral: &SignalPub,
) -> Result<ChainKey, Error> {
    if let Some(ck) = state.receiver_chain_key(their_ephemeral)? {
        return Ok(ck);
    }

    let root_key = state.root_key();
    let our_ephemeral = state.sender_ratchet_private()?;
    let (receiver_root, receiver_chain) = root_key.create_chain(&our_ephemeral, their_ephemeral)?;

    let (our_new_priv, our_new_pub) = gen_keypair(rnd);
    let (sender_root, sender_chain) = receiver_root.create_chain(&our_new_priv, their_ephemeral)?;

    let current_sender_chain_key = state.sender_chain_key()?;
    state.set_root_key(sender_root);
    state.add_receiver_chain(their_ephemeral, &receiver_chain);

    let previous_idx = current_sender_chain_key.index.saturating_sub(1);
    state.previous_counter = previous_idx;
    state.set_sender_chain(&our_new_priv, &our_new_pub, &sender_chain);

    Ok(receiver_chain)
}

fn message_keys_for(
    state: &mut SessionStructure,
    their_ephemeral: &SignalPub,
    chain_key: &ChainKey,
    counter: u32,
) -> Result<MessageKeys, Error> {
    let chain_idx = chain_key.index;
    if chain_idx > counter {
        if let Some(keys) = state.take_message_keys(their_ephemeral, counter)? {
            return Ok(keys);
        }
        return Err(duplicate_message());
    }

    let jump = counter - chain_idx;
    if jump > MAX_JUMPS {
        return Err(Error::Noise("message from too far in the future".into()));
    }

    let mut chain_key = chain_key.clone();
    while chain_key.index < counter {
        let keys = chain_key.message_keys();
        state.set_message_keys(their_ephemeral, &keys)?;
        chain_key = chain_key.next();
    }
    state.set_receiver_chain_key(their_ephemeral, chain_key.clone())?;
    Ok(chain_key.message_keys())
}

fn is_duplicate(e: &Error) -> bool {
    matches!(e, Error::Noise(s) if s == "duplicate message")
}

fn duplicate_message() -> Error {
    Error::Noise("duplicate message".into())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wa_signal::SessionRecord;

    fn decrypt_alice_local_keys(privk: [u8; 32], pubk: SignalPub) -> LocalKeys<'static> {
        LocalKeys {
            identity_priv: privk,
            identity_pub: pubk,
            signed_prekey: None,
            one_time_prekeys: &[],
            local_reg_id: 0,
        }
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// Mirrors the Go harness `fixedReader`: bytes are 1,2,3,… from `start`.
    struct FixedReader {
        i: u8,
    }
    impl RngCore for FixedReader {
        fn next_u32(&mut self) -> u32 {
            let mut b = [0u8; 4];
            self.fill_bytes(&mut b);
            u32::from_le_bytes(b)
        }
        fn next_u64(&mut self) -> u64 {
            let mut b = [0u8; 8];
            self.fill_bytes(&mut b);
            u64::from_le_bytes(b)
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for x in dest.iter_mut() {
                self.i = self.i.wrapping_add(1);
                *x = self.i;
            }
        }
    }

    #[test]
    fn signal_msg_roundtrip() {
        let sig = SignalMsg::new(
            3,
            &[7u8; 32],
            &bind([1u8; 32]),
            4,
            5,
            b"the quick brown fox jumps over the lazy dog".to_vec(),
            &bind([2u8; 32]),
            &bind([3u8; 32]),
        )
        .unwrap();
        let dec = SignalMsg::from_bytes(&sig.serialized).unwrap();
        assert_eq!(dec, sig);
        assert!(sig.verify_mac(&[7u8; 32], &bind([2u8; 32]), &bind([3u8; 32])).unwrap());
        assert!(!sig.verify_mac(&[8u8; 32], &bind([2u8; 32]), &bind([3u8; 32])).unwrap());
    }

    #[test]
    fn prekey_msg_roundtrip() {
        let sig = SignalMsg::new(
            3,
            &[9u8; 32],
            &bind([4u8; 32]),
            0,
            0,
            b"inner".to_vec(),
            &bind([5u8; 32]),
            &bind([6u8; 32]),
        )
        .unwrap();
        let pk = PreKeyMsg::new(3, 1111, Some(77), 55, &bind([1u8; 32]), &bind([5u8; 32]), &sig).unwrap();
        let dec = PreKeyMsg::from_bytes(&pk.serialized).unwrap();
        assert_eq!(dec, pk);
        // Absent pre-key id must decode as None (proto2 presence).
        let pk2 = PreKeyMsg::new(3, 1111, None, 55, &bind([1u8; 32]), &bind([5u8; 32]), &sig).unwrap();
        let dec2 = PreKeyMsg::from_bytes(&pk2.serialized).unwrap();
        assert_eq!(dec2.pre_key_id, None);
        assert_eq!(dec2.signed_pre_key_id, 55);
    }

    #[test]
    fn two_party_flow_matches_go_golden() {
        let golden: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/signal_golden.json"
        )))
        .unwrap();
        let g = |k: &str| -> [u8; 32] { hex(golden[k].as_str().unwrap()).try_into().unwrap() };

        // ── keys (Bob's identity / signed / one-time; Alice's identity) ──
        let alice_ident_priv: [u8; 32] = g("alice_ident_priv");
        let bob_ident_priv: [u8; 32] = g("bob_ident_priv");
        let bob_signed_priv: [u8; 32] = g("bob_signed_priv");
        let bob_onetime_priv: [u8; 32] = g("bob_onetime_priv");

        // Golden public keys are 33 bytes (0x05 prefix).
        let pubg = |k: &str| -> SignalPub { hex(golden[k].as_str().unwrap()).try_into().unwrap() };
        let alice_pub: SignalPub = pubg("alice_ident_pub");
        let bob_pub: SignalPub = pubg("bob_ident_pub");
        let bob_signed_pub: SignalPub = pubg("bob_signed_pub");
        let bob_onetime_pub: SignalPub = pubg("bob_onetime_pub");

        // Lock the public keys against the golden (clamping + base-point mult).
        assert_eq!(keypair_from_seed(&alice_ident_priv).1, alice_pub);
        assert_eq!(keypair_from_seed(&bob_ident_priv).1, bob_pub);
        assert_eq!(keypair_from_seed(&bob_signed_priv).1, bob_signed_pub);
        assert_eq!(keypair_from_seed(&bob_onetime_priv).1, bob_onetime_pub);

        let mut rng = FixedReader { i: 200 };

        // ── Alice processes Bob's bundle (consumes 64 bytes of rng) ──
        let (base_priv, base_pub) = gen_keypair(&mut rng);
        let alice = AliceParams {
            our_identity_priv: alice_ident_priv,
            our_identity_pub: alice_pub,
            our_base_priv: base_priv,
            our_base_pub: base_pub,
            their_identity: &bob_pub,
            their_signed_prekey: &bob_signed_pub,
            their_one_time_prekey: Some(&bob_onetime_pub),
            their_ratchet_key: &bob_signed_pub,
        };
        let mut alice_state = initialize_alice_session(&mut rng, &alice).unwrap();
        alice_state.local_registration_id = 1111;
        alice_state.remote_registration_id = 2222;
        alice_state.set_unacknowledged_pre_key_message(Some(77), 55, &base_pub);
        alice_state.alice_base_key = base_pub.to_vec();

        let mut alice_rec = SessionRecord::new(Some(alice_state));

        // ── Alice sends two pre-key messages ──
        let msg1 = encrypt_message(&mut rng, &mut alice_rec, b"hello bob 1").unwrap();
        let msg2 = encrypt_message(&mut rng, &mut alice_rec, b"hello bob 2").unwrap();
        assert_eq!(msg1.bytes(), hex(golden["msg1_bytes"].as_str().unwrap()).as_slice());
        assert_eq!(msg1.msg_type(), CiphertextType::PreKey);
        assert_eq!(msg2.bytes(), hex(golden["msg2_bytes"].as_str().unwrap()).as_slice());

        // Golden dumps the bare session state after both messages are sent.
        assert_eq!(
            alice_rec.state().unwrap().encode(),
            hex(golden["alice_session_after_encrypt"].as_str().unwrap()),
            "alice session state after both prekey messages must match Go"
        );

        // ── Bob decrypts both pre-key messages ──
        let bob_otp = [(77, bob_onetime_priv, bob_onetime_pub)];
        let bob_keys = LocalKeys {
            identity_priv: bob_ident_priv,
            identity_pub: bob_pub,
            signed_prekey: Some((55, bob_signed_priv, bob_signed_pub)),
            one_time_prekeys: &bob_otp,
            local_reg_id: 2222,
        };
        let mut bob_rec = SessionRecord::new(None);
        assert_eq!(
            decrypt_message(&mut rng, &mut bob_rec, &msg1, &bob_keys).unwrap(),
            b"hello bob 1"
        );
        assert_eq!(
            decrypt_message(&mut rng, &mut bob_rec, &msg2, &bob_keys).unwrap(),
            b"hello bob 2"
        );
        assert_eq!(
            bob_rec.state().unwrap().encode(),
            hex(golden["bob_session_after_decrypt"].as_str().unwrap())
        );

        // ── Bob replies; Alice decrypts (both sides ratchet) ──
        let reply = encrypt_message(&mut rng, &mut bob_rec, b"reply alice").unwrap();
        assert_eq!(reply.bytes(), hex(golden["reply_bytes"].as_str().unwrap()).as_slice());
        assert_eq!(reply.msg_type(), CiphertextType::Whisper);

        assert_eq!(
            decrypt_message(&mut rng, &mut alice_rec, &reply, &decrypt_alice_local_keys(alice_ident_priv, alice_pub))
                .unwrap(),
            b"reply alice"
        );
        assert_eq!(alice_rec.state().unwrap().encode(), hex(golden["alice_session_after_reply"].as_str().unwrap()));

        // ── Alice acks; Bob decrypts (Bob ratchets again) ──
        let ack = encrypt_message(&mut rng, &mut alice_rec, b"ack").unwrap();
        assert_eq!(ack.bytes(), hex(golden["ack_bytes"].as_str().unwrap()).as_slice());
        assert_eq!(decrypt_message(&mut rng, &mut bob_rec, &ack, &bob_keys).unwrap(), b"ack");
    }

    #[test]
    fn initialize_from_bundle_e2e() {
        // Bob's prekey material.
        let (bob_ident_priv, bob_ident_pub) = keypair_from_seed(&[0xb0; 32]);
        let (bob_signed_priv, bob_signed_pub) = keypair_from_seed(&[0xb1; 32]);
        let (bob_onetime_priv, bob_onetime_pub) = keypair_from_seed(&[0xb2; 32]);
        let (alice_ident_priv, alice_ident_pub) = keypair_from_seed(&[0xa0; 32]);

        // Signed-prekey signature over the 33-byte prefixed pub (as in the fork).
        let signed_sig = crate::wa2::xed25519_sign(&bob_ident_priv, &bob_signed_pub);
        let signed_raw = bob_signed_pub[1..].try_into().unwrap();
        let onetime_raw = bob_onetime_pub[1..].try_into().unwrap();
        let ident_raw = bob_ident_pub[1..].try_into().unwrap();

        let init = AliceBundle {
            their_identity_pub: ident_raw,
            their_signed_prekey: (signed_raw, &signed_sig),
            their_signed_prekey_id: 55,
            their_one_time_prekey: Some((77, onetime_raw)),
            their_registration_id: 2222,
            our_registration_id: 1111,
        };
        let mut rng = FixedReader { i: 200 };
        let (mut alice_rec, otpk_id) =
            initialize_from_bundle(&mut rng, &init, (alice_ident_priv, alice_ident_pub)).unwrap();
        assert_eq!(otpk_id, Some(77));

        // First message must be a PreKey message referencing both prekeys.
        let msg1 = encrypt_message(&mut rng, &mut alice_rec, b"first").unwrap();
        assert_eq!(msg1.msg_type(), CiphertextType::PreKey);
        let CiphertextMsg::PreKey(pk) = &msg1 else { panic!("expected prekey") };
        assert_eq!(pk.pre_key_id, Some(77));
        assert_eq!(pk.signed_pre_key_id, 55);
        assert_eq!(pk.registration_id, 1111);

        // Bob decrypts it using his prekey stores.
        let bob_otp = [(77, bob_onetime_priv, bob_onetime_pub)];
        let bob_keys = LocalKeys {
            identity_priv: bob_ident_priv,
            identity_pub: bob_ident_pub,
            signed_prekey: Some((55, bob_signed_priv, bob_signed_pub)),
            one_time_prekeys: &bob_otp,
            local_reg_id: 2222,
        };
        let mut bob_rec = SessionRecord::new(None);
        assert_eq!(decrypt_message(&mut rng, &mut bob_rec, &msg1, &bob_keys).unwrap(), b"first");

        // Session is now live in both directions.
        let reply = encrypt_message(&mut rng, &mut bob_rec, b"pong").unwrap();
        assert_eq!(reply.msg_type(), CiphertextType::Whisper);
        assert_eq!(
            decrypt_message(&mut rng, &mut alice_rec, &reply, &decrypt_alice_local_keys(alice_ident_priv, alice_ident_pub))
                .unwrap(),
            b"pong"
        );

        // Signature verification rejects a tampered signed prekey.
        let mut bad_sig = signed_sig;
        bad_sig[0] ^= 1;
        let bad = AliceBundle {
            their_signed_prekey: (signed_raw, &bad_sig),
            ..init
        };
        assert!(initialize_from_bundle(&mut rng, &bad, (alice_ident_priv, alice_ident_pub)).is_err());
    }
}