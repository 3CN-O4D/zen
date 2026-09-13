//! wa_bot — persistent WhatsApp device + live bot loop on top of `wa2`.
//!
//! Ported from whatsmeow (MPL-2.0):
//!   - `pair.go`     pair-success handling (ADVSignedDeviceIdentity check/sign)
//!   - `store/clientpayload.go`    getLoginPayload
//!   - `keepalive.go`              `w:p` ping loop
//!
//! The bot owns a persistent key material (identity / noise / adv secret, plus
//! the signed prekey), which lets it survive restarts between the QR phase and
//! the logged-in phase without the phone needing to re-scan.

use crate::wa2::{
    base_user_agent_msg, base_web_info_msg, ecc_curve_djb_type, generate_ephemeral, pb,
    xed25519_sign, xed25519_verify, Error, Jid, PairingMaterial, Wa2Session, WaNode, WaVal,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::io::{Read, Write as _};

type HmacSha256 = Hmac<Sha256>;

// ─────────────────────────────────────────────────────────────────────────────
// ADV signature prefixes (whatsmeow/adv.go / pair.go)
// ─────────────────────────────────────────────────────────────────────────────

/// `AdvAccountSignaturePrefix` — message prefix for the phone's own
/// (account-level) signature on the device identity.
const ADV_ACCOUNT_PREFIX: &[u8] = &[0x06, 0x00];
/// `AdvDeviceSignaturePrefix` — message prefix for the device's self-signature.
const ADV_DEVICE_PREFIX: &[u8] = &[0x06, 0x01];
/// `AdvHostedAccountSignaturePrefix`.
const ADV_HOSTED_ACCOUNT_PREFIX: &[u8] = &[0x06, 0x05];
/// `AdvHostedDeviceSignaturePrefix`.
const ADV_HOSTED_DEVICE_PREFIX: &[u8] = &[0x06, 0x06];

// ─────────────────────────────────────────────────────────────────────────────
// waAdv protobuf structures (proto/waAdv)
// ─────────────────────────────────────────────────────────────────────────────

/// `ADVSignedDeviceIdentity` — phone-issued, device-signed during pairing.
#[derive(Debug, Clone, Default)]
struct AdvSignedDeviceIdentity {
    details: Vec<u8>,
    account_signature_key: Vec<u8>,
    account_signature: Vec<u8>,
    device_signature: Vec<u8>,
}

impl AdvSignedDeviceIdentity {
    fn parse(data: &[u8]) -> Result<Self, Error> {
        let f = pb::parse(data)?;
        Ok(AdvSignedDeviceIdentity {
            details: pb::find_bytes(&f, 1).unwrap_or_default().to_vec(),
            account_signature_key: pb::find_bytes(&f, 2).unwrap_or_default().to_vec(),
            account_signature: pb::find_bytes(&f, 3).unwrap_or_default().to_vec(),
            device_signature: pb::find_bytes(&f, 4).unwrap_or_default().to_vec(),
        })
    }

    /// Marshal the full proto (all four fields) — the stored device account.
    fn marshal_full(&self) -> Vec<u8> {
        let mut out = Vec::new();
        pb::field_bytes(1, &self.details, &mut out);
        if !self.account_signature_key.is_empty() {
            pb::field_bytes(2, &self.account_signature_key, &mut out);
        }
        if !self.account_signature.is_empty() {
            pb::field_bytes(3, &self.account_signature, &mut out);
        }
        if !self.device_signature.is_empty() {
            pb::field_bytes(4, &self.device_signature, &mut out);
        }
        out
    }

    /// Marshal the self-signed proto sent back to the server — identical to
    /// whatsmeow's `pair.go`, which clears `accountSignatureKey` before
    /// marshaling the response.
    fn marshal_self_signed(&self) -> Vec<u8> {
        let mut out = Vec::new();
        pb::field_bytes(1, &self.details, &mut out);
        if !self.account_signature.is_empty() {
            pb::field_bytes(3, &self.account_signature, &mut out);
        }
        if !self.device_signature.is_empty() {
            pb::field_bytes(4, &self.device_signature, &mut out);
        }
        out
    }
}

/// `ADVSignedDeviceIdentityHMAC` — the envelope the phone hands the server,
/// which must be re-validated locally with the adv secret key.
struct AdvSignedDeviceIdentityHmac {
    details: Vec<u8>,
    hmac: Vec<u8>,
    account_type: u8,
}

impl AdvSignedDeviceIdentityHmac {
    fn parse(data: &[u8]) -> Result<Self, Error> {
        let f = pb::parse(data)?;
        Ok(AdvSignedDeviceIdentityHmac {
            details: pb::find_bytes(&f, 1).unwrap_or_default().to_vec(),
            hmac: pb::find_bytes(&f, 2).unwrap_or_default().to_vec(),
            account_type: pb::find_var(&f, 3).unwrap_or_default() as u8,
        })
    }
}

/// `ADVDeviceIdentity` — the inner key-index + encryption type payload.
struct AdvDeviceIdentityDetails {
    key_index: u32,
    account_type: u8,
    device_type: u8,
}

impl AdvDeviceIdentityDetails {
    fn parse(data: &[u8]) -> Result<Self, Error> {
        let f = pb::parse(data)?;
        Ok(AdvDeviceIdentityDetails {
            key_index: pb::find_var(&f, 3).unwrap_or_default() as u32,
            account_type: pb::find_var(&f, 4).unwrap_or_default() as u8,
            device_type: pb::find_var(&f, 5).unwrap_or_default() as u8,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Persistent device state
// ─────────────────────────────────────────────────────────────────────────────

/// A one-time (pick-up) prekey in the pool we upload to the server for
/// receiving sessions.
#[derive(Debug, Clone, PartialEq)]
pub struct OneTimePreKey {
    pub id: u32,
    pub priv_key: [u8; 32],
    pub pub_key: [u8; 32],
    pub uploaded: bool,
}

/// A stored Signal session, keyed by the remote's Signal address
/// (`{user}.{device}` on the wire).
#[derive(Debug, Clone, PartialEq)]
pub struct StoredSession {
    pub address: String,
    pub record: Vec<u8>,
}

/// Everything the bot must keep across restarts: the pairing key material, the
/// Signal identity (+ signed prekey), the one-time prekey pool, and stored
/// sessions, plus (once paired) the assigned JID/LID.
#[derive(Debug, Clone)]
pub struct BotDevice {
    pub registration_id: u32,
    pub identity_priv: [u8; 32],
    pub identity_pub: [u8; 32],
    pub noise_priv: [u8; 32],
    pub noise_pub: [u8; 32],
    pub prekey_id: u32,
    pub signed_prekey_priv: [u8; 32],
    pub signed_prekey_pub: [u8; 32],
    pub signed_prekey_sig: [u8; 64],
    pub adv_secret_key: [u8; 32],
    pub one_time_prekeys: Vec<OneTimePreKey>,
    pub next_prekey_id: u32,
    pub sessions: Vec<StoredSession>,
    pub user: String,
    pub device: u16,
    pub lid_user: String,
    pub lid_device: u16,
    pub business_name: String,
    pub platform: String,
    pub account: Vec<u8>,
}

impl BotDevice {
    pub fn generate() -> Self {
        use rand::Rng as _;

        let (identity_priv, identity_pub) = generate_ephemeral();
        let (noise_priv, noise_pub) = generate_ephemeral();
        let (signed_prekey_priv, signed_prekey_pub) = generate_ephemeral();

        let mut skey_to_sign = [0u8; 33];
        skey_to_sign[0] = ecc_curve_djb_type();
        skey_to_sign[1..].copy_from_slice(&signed_prekey_pub);
        let signed_prekey_sig = xed25519_sign(&identity_priv, &skey_to_sign);

        BotDevice {
            registration_id: rand::rng().random::<u32>() | 1,
            identity_priv,
            identity_pub,
            noise_priv,
            noise_pub,
            prekey_id: rand::rng().random::<u32>() | 1,
            signed_prekey_priv,
            signed_prekey_pub,
            signed_prekey_sig,
            adv_secret_key: rand::rng().random::<[u8; 32]>(),
            one_time_prekeys: Vec::new(),
            next_prekey_id: 1000,
            sessions: Vec::new(),
            user: String::new(),
            device: 0,
            lid_user: String::new(),
            lid_device: 0,
            business_name: String::new(),
            platform: String::new(),
            account: Vec::new(),
        }
    }

    pub fn is_paired(&self) -> bool {
        !self.user.is_empty()
    }

    /// The registration-side presentation of this device's key material.
    pub fn material(&self) -> PairingMaterial {
        PairingMaterial {
            registration_id: self.registration_id,
            identity_priv: self.identity_priv,
            identity_pub: self.identity_pub,
            noise_priv: self.noise_priv,
            noise_pub: self.noise_pub,
            prekey_id: self.prekey_id,
            signed_prekey_pub: self.signed_prekey_pub,
            signed_prekey_sig: self.signed_prekey_sig,
            adv_secret_key: self.adv_secret_key,
        }
    }

    /// Signal address (`user.device`) for a remote JID.
    pub fn signal_address(user: &str, device: u16) -> String {
        format!("{user}.{device}")
    }

    pub fn get_session(&self, address: &str) -> Option<&[u8]> {
        self.sessions.iter().find(|s| s.address == address).map(|s| s.record.as_slice())
    }

    pub fn put_session(&mut self, address: &str, record: Vec<u8>) {
        if let Some(existing) = self.sessions.iter_mut().find(|s| s.address == address) {
            existing.record = record;
        } else {
            self.sessions.push(StoredSession { address: address.into(), record });
        }
    }

    #[allow(dead_code)] // used when clearing sessions on identity change
    pub fn del_session(&mut self, address: &str) {
        self.sessions.retain(|s| s.address != address);
    }

    /// Generate (and return) `count` fresh one-time prekeys, bumping the id
    /// cursor so subsequent calls don't collide.
    pub fn gen_prekeys(&mut self, count: u32) -> Vec<OneTimePreKey> {
        let mut out = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let (priv_key, pub_key) = generate_ephemeral();
            let id = self.next_prekey_id;
            self.next_prekey_id =
                if self.next_prekey_id >= 0xfffffe { 1 } else { self.next_prekey_id + 1 };
            let key = OneTimePreKey { id, priv_key, pub_key, uploaded: false };
            out.push(key.clone());
            self.one_time_prekeys.push(key);
        }
        out
    }

    pub fn mark_prekeys_uploaded(&mut self, upto_id: u32) {
        for k in &mut self.one_time_prekeys {
            if k.id <= upto_id {
                k.uploaded = true;
            }
        }
    }

    /// Remove (consume) a one-time prekey from the pool, returning it.
    pub fn take_prekey(&mut self, id: u32) -> Option<OneTimePreKey> {
        let idx = self.one_time_prekeys.iter().position(|k| k.id == id)?;
        Some(self.one_time_prekeys.remove(idx))
    }

    /// Load state from a line-based `key=value` file (hex for byte fields).
    pub fn load(path: &str) -> Result<BotDevice, Error> {
        let mut file = std::fs::File::open(path)
            .map_err(|e| Error::Io(std::io::Error::new(e.kind(), e.to_string())))?;
        let mut data = String::new();
        file.read_to_string(&mut data)
            .map_err(|e| Error::Io(std::io::Error::new(e.kind(), e.to_string())))?;

        let mut dev = BotDevice::generate();
        let parse_hex = |s: &str| -> Vec<u8> {
            let s = s.trim();
            if !s.len().is_multiple_of(2) {
                return Vec::new();
            }
            (0..s.len()).step_by(2).fold(Vec::new(), |mut acc, i| {
                if let Ok(b) = u8::from_str_radix(&s[i..i + 2], 16) {
                    acc.push(b);
                }
                acc
            })
        };

        for line in data.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let v = v.trim();
            match k {
                "registration_id" => dev.registration_id = v.parse().unwrap_or(1),
                "identity_priv" => copy_32(&parse_hex(v), &mut dev.identity_priv),
                "identity_pub" => copy_32(&parse_hex(v), &mut dev.identity_pub),
                "noise_priv" => copy_32(&parse_hex(v), &mut dev.noise_priv),
                "noise_pub" => copy_32(&parse_hex(v), &mut dev.noise_pub),
                "prekey_id" => dev.prekey_id = v.parse().unwrap_or(1),
                "signed_prekey_priv" => copy_32(&parse_hex(v), &mut dev.signed_prekey_priv),
                "signed_prekey_pub" => copy_32(&parse_hex(v), &mut dev.signed_prekey_pub),
                "signed_prekey_sig" => {
                    let b = parse_hex(v);
                    if b.len() == 64 {
                        dev.signed_prekey_sig.copy_from_slice(&b);
                    }
                }
                "adv_secret_key" => copy_32(&parse_hex(v), &mut dev.adv_secret_key),
                "user" => dev.user = v.to_string(),
                "device" => dev.device = v.parse().unwrap_or(0),
                "lid_user" => dev.lid_user = v.to_string(),
                "lid_device" => dev.lid_device = v.parse().unwrap_or(0),
                "business_name" => dev.business_name = v.to_string(),
                "platform" => dev.platform = v.to_string(),
                "account" => dev.account = parse_hex(v),
                "next_prekey_id" => dev.next_prekey_id = v.parse().unwrap_or(1000),
                k if k.starts_with("session.") => {
                    let addr = k.strip_prefix("session.").unwrap_or_default();
                    dev.sessions.push(StoredSession { address: addr.to_string(), record: parse_hex(v) });
                }
                _ if k.starts_with("prekey.") => {
                    // prekey.NID.priv / prekey.NID.pub / prekey.NID.uploaded
                    let Some(rest) = k.strip_prefix("prekey.") else { continue };
                    let Some((id_str, field)) = rest.split_once('.') else { continue };
                    let Some(id) = id_str.parse::<u32>().ok() else { continue };
                    if !dev.one_time_prekeys.iter().any(|p| p.id == id) {
                        dev.one_time_prekeys.push(OneTimePreKey {
                            id,
                            priv_key: [0u8; 32],
                            pub_key: [0u8; 32],
                            uploaded: false,
                        });
                    }
                    let slot = dev.one_time_prekeys.iter_mut().find(|p| p.id == id);
                    match (field, slot) {
                        ("priv", Some(p)) => copy_32(&parse_hex(v), &mut p.priv_key),
                        ("pub", Some(p)) => copy_32(&parse_hex(v), &mut p.pub_key),
                        ("uploaded", Some(p)) => p.uploaded = v == "1",
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        Ok(dev)
    }

    /// Persist state to the same line-based format as [`BotDevice::load`].
    pub fn save(&self, path: &str) -> Result<(), Error> {
        let hex = |b: &[u8]| {
            b.iter()
                .map(|x| format!("{x:02x}"))
                .collect::<String>()
        };
        let mut out = String::new();
        let push = |out: &mut String, k: &str, v: String| {
            out.push_str(k);
            out.push('=');
            out.push_str(&v);
            out.push('\n');
        };
        push(&mut out, "registration_id", self.registration_id.to_string());
        push(&mut out, "identity_priv", hex(&self.identity_priv));
        push(&mut out, "identity_pub", hex(&self.identity_pub));
        push(&mut out, "noise_priv", hex(&self.noise_priv));
        push(&mut out, "noise_pub", hex(&self.noise_pub));
        push(&mut out, "prekey_id", self.prekey_id.to_string());
        push(&mut out, "signed_prekey_pub", hex(&self.signed_prekey_pub));
        push(&mut out, "signed_prekey_sig", hex(&self.signed_prekey_sig));
        push(&mut out, "adv_secret_key", hex(&self.adv_secret_key));
        push(&mut out, "user", self.user.clone());
        push(&mut out, "device", self.device.to_string());
        push(&mut out, "lid_user", self.lid_user.clone());
        push(&mut out, "lid_device", self.lid_device.to_string());
        push(&mut out, "business_name", self.business_name.clone());
        push(&mut out, "platform", self.platform.clone());
        push(&mut out, "account", hex(&self.account));
        push(&mut out, "signed_prekey_priv", hex(&self.signed_prekey_priv));
        push(&mut out, "next_prekey_id", self.next_prekey_id.to_string());
        for pk in &self.one_time_prekeys {
            let prefix = format!("prekey.{}", pk.id);
            push(&mut out, &format!("{prefix}.priv"), hex(&pk.priv_key));
            push(&mut out, &format!("{prefix}.pub"), hex(&pk.pub_key));
            push(&mut out, &format!("{prefix}.uploaded"), if pk.uploaded { "1" } else { "0" }.into());
        }
        for s in &self.sessions {
            push(&mut out, &format!("session.{}", s.address), hex(&s.record));
        }

        let mut file = std::fs::File::create(path)
            .map_err(|e| Error::Io(std::io::Error::new(e.kind(), e.to_string())))?;
        file.write_all(out.as_bytes())
            .map_err(|e| Error::Io(std::io::Error::new(e.kind(), e.to_string())))?;
        Ok(())
    }
}

fn copy_32(src: &[u8], dst: &mut [u8; 32]) {
    if src.len() == 32 {
        dst.copy_from_slice(src);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pair-success handling (whatsmeow/pair.go: handlePair)
// ─────────────────────────────────────────────────────────────────────────────

/// Parse the `<pair-success><device-identity>` payload, verify the phone's
/// signature chain, stamp the device's own signature in, and build the reply
/// `<iq type="result"><pair-device-sign>` node. Returns the reply node (the
/// caller sends it) after mutating `dev` with the paired identity.
pub fn build_pair_response(iq: &WaNode, dev: &mut BotDevice) -> Result<WaNode, Error> {
    let pair_success = iq
        .child("pair-success")
        .ok_or_else(|| Error::Noise("pair-success missing from iq".into()))?;
    let iq_id = iq.attr_str("id").unwrap_or_default().to_string();

    let container_bytes = pair_success
        .child("device-identity")
        .and_then(|n| match &n.content {
            WaVal::Bytes(b) => Some(b.clone()),
            _ => None,
        })
        .ok_or_else(|| Error::Noise("device-identity missing from pair-success".into()))?;

    let container = AdvSignedDeviceIdentityHmac::parse(&container_bytes)?;

    // 1. The adv secret key proves we are the device whose keys were in the QR.
    let mut mac = HmacSha256::new_from_slice(&dev.adv_secret_key)
        .map_err(|_| Error::Proto("adv secret key unusable for hmac".into()))?;
    if container.account_type == 1 {
        mac.update(ADV_HOSTED_ACCOUNT_PREFIX);
    }
    mac.update(&container.details);
    if mac.verify_slice(&container.hmac).is_err() {
        return Err(Error::Cert("pair-success hmac mismatch".into()));
    }

    let mut device_identity = AdvSignedDeviceIdentity::parse(&container.details)?;
    let details = AdvDeviceIdentityDetails::parse(&device_identity.details)?;

    if details.device_type == 1 {
        return Err(Error::Unimplemented("hosted (agent) pairing"));
    }
    let account_prefix = if details.account_type == 1 {
        ADV_HOSTED_ACCOUNT_PREFIX
    } else {
        ADV_ACCOUNT_PREFIX
    };
    let device_prefix = if details.account_type == 1 {
        ADV_HOSTED_DEVICE_PREFIX
    } else {
        ADV_DEVICE_PREFIX
    };

    // 2. Verify the phone's account signature over (prefix ∥ details ∥ our identity).
    let mut account_message = Vec::with_capacity(
        account_prefix.len() + device_identity.details.len() + dev.identity_pub.len(),
    );
    account_message.extend_from_slice(account_prefix);
    account_message.extend_from_slice(&device_identity.details);
    account_message.extend_from_slice(&dev.identity_pub);
    let account_key: [u8; 32] = device_identity
        .account_signature_key
        .as_slice()
        .try_into()
        .map_err(|_| Error::Cert("account signature key wrong length".into()))?;
    let account_sig: [u8; 64] = device_identity
        .account_signature
        .as_slice()
        .try_into()
        .map_err(|_| Error::Cert("account signature wrong length".into()))?;
    if !xed25519_verify(&account_key, &account_sig, &account_message) {
        return Err(Error::Cert("pair-success account signature mismatch".into()));
    }

    // 3. Sign our half: (prefix ∥ details ∥ identity ∥ account sig key).
    let mut device_message =
        Vec::with_capacity(device_prefix.len() + device_identity.details.len()
            + dev.identity_pub.len()
            + device_identity.account_signature_key.len());
    device_message.extend_from_slice(device_prefix);
    device_message.extend_from_slice(&device_identity.details);
    device_message.extend_from_slice(&dev.identity_pub);
    device_message.extend_from_slice(&device_identity.account_signature_key);
    let device_signature = xed25519_sign(&dev.identity_priv, &device_message);

    device_identity.device_signature = device_signature.to_vec();
    dev.account = device_identity.marshal_full();

    // 4. The assigned identity comes from the `<device>` / `<biz>` / `<platform>` attrs.
    let dev_jid = attr_strish(pair_success, "device", "jid");
    let lid_jid = attr_strish(pair_success, "device", "lid");
    let (user, device) = split_jid(&dev_jid);
    dev.user = user;
    dev.device = device;
    let (lid_user, lid_device) = split_jid(&lid_jid);
    dev.lid_user = lid_user;
    dev.lid_device = lid_device;
    dev.business_name = attr_strish(pair_success, "biz", "name");
    dev.platform = attr_strish(pair_success, "platform", "name");

    // 5. Reply with the self-signed identity (accountSignatureKey stripped).
    let self_signed = device_identity.marshal_self_signed();
    let mut dev_identity = WaNode::new("device-identity")
        .attr("key-index", &details.key_index.to_string());
    dev_identity.content = WaVal::Bytes(self_signed);

    let mut pair_sign = WaNode::new("pair-device-sign");
    pair_sign.content = WaVal::Nodes(vec![dev_identity]);

    let mut reply = WaNode::new("iq")
        .attr("to", "s.whatsapp.net")
        .attr("type", "result")
        .attr("id", &iq_id);
    reply.content = WaVal::Nodes(vec![pair_sign]);
    Ok(reply)
}

fn attr_strish(node: &WaNode, child_tag: &str, key: &str) -> String {
    let Some(child) = node.child(child_tag) else {
        return String::new();
    };
    for (k, v) in &child.attrs {
        if k != key {
            continue;
        }
        return match v {
            WaVal::Str(s) => s.clone(),
            WaVal::Jid(j) => format_jid(j),
            WaVal::Int(i) => i.to_string(),
            _ => String::new(),
        };
    }
    String::new()
}

fn format_jid(j: &Jid) -> String {
    if j.device != 0 {
        format!("{}:{}@{}", j.user, j.device, j.server)
    } else {
        format!("{}@{}", j.user, j.server)
    }
}

/// Split `user[:device]@server` into `(user, device)`.
fn split_jid(s: &str) -> (String, u16) {
    let user_part = s.split('@').next().unwrap_or_default();
    let mut parts = user_part.split(':');
    let user = parts.next().unwrap_or_default();
    if let Some(dev) = parts.next() {
        if let Ok(d) = dev.parse() {
            return (user.to_string(), d);
        }
    }
    (user.to_string(), 0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Login payload (whatsmeow/store/clientpayload.go: getLoginPayload)
// ─────────────────────────────────────────────────────────────────────────────

/// Build the authenticated `ClientPayload` for an already-paired device.
fn build_login_payload(dev: &BotDevice) -> Vec<u8> {
    let user = dev.user.parse::<u64>().unwrap_or(0);
    let ua = base_user_agent_msg();
    let web_info = base_web_info_msg();

    let mut payload = Vec::new();
    pb::field_uint(1, user, &mut payload); // username
    pb::field_bool(3, true, &mut payload); // passive — server-buffered events
    pb::field_msg(5, &ua, &mut payload); // userAgent
    pb::field_msg(6, &web_info, &mut payload); // webInfo
    pb::field_uint(12, 1, &mut payload); // connectType: WIFI_UNKNOWN
    pb::field_uint(13, 1, &mut payload); // connectReason: USER_ACTIVATED
    pb::field_uint(18, dev.device as u64, &mut payload); // device
    pb::field_uint(24, 1, &mut payload); // lc
    pb::field_bool(33, true, &mut payload); // pull
    pb::field_bool(41, true, &mut payload); // lidDbMigrated
    payload
}

/// Complete the Noise handshake presenting the login payload.
pub fn handshake_login(sess: &mut Wa2Session, dev: &BotDevice) -> Result<(), Error> {
    sess.handshake_with_payload(build_login_payload(dev), (dev.noise_priv, dev.noise_pub))
}

// ─────────────────────────────────────────────────────────────────────────────
// Session loop
// ─────────────────────────────────────────────────────────────────────────────

const KEEPALIVE_INTERVAL_SECS: u64 = 20;

fn send_keepalive(sess: &mut Wa2Session) -> Result<(), Error> {
    use rand::Rng as _;
    let mut node = WaNode::new("iq")
        .attr("to", "s.whatsapp.net")
        .attr("type", "get")
        .attr("id", &format!("ka{}", rand::rng().random::<u32>()));
    node.attrs.push(("xmlns".into(), WaVal::Str("w:p".into())));
    sess.send_node(&node)
}

pub fn keepalive(sess: &mut Wa2Session) -> Result<(), Error> {
    send_keepalive(sess)
}

/// Wait for the post-login `<success>` (or `<failure>`) node, then run the
/// live event loop: keepalive pings, ping responses, and event dispatch to
/// `event(kind, node)`. `stop()` is polled to allow a graceful shutdown.
pub fn serve(
    sess: &mut Wa2Session,
    event: &mut dyn FnMut(SessionEvent),
    stop: &dyn Fn() -> bool,
) -> Result<(), Error> {
    loop {
        if stop() {
            return Ok(());
        }
        match sess.recv_node_timeout(KEEPALIVE_INTERVAL_SECS)? {
            None => send_keepalive(sess)?,
            Some(node) => {
                match node.tag.as_str() {
                    "success" => {
                        event(SessionEvent::Connected);
                        break;
                    }
                    "failure" => {
                        return Err(Error::Ws(format!(
                            "login rejected: {}",
                            node.attr_str("reason").unwrap_or("unknown reason")
                        )));
                    }
                    "stream:error" => return Err(Error::Ws("stream error during login".into())),
                    _ => {
                        // The pre-`success` window can carry a few nodes
                        // (acks for the passive stream, etc.).
                        event(SessionEvent::Node(node));
                    }
                }
            }
        }
    }

    loop {
        if stop() {
            return Ok(());
        }
        match sess.recv_node_timeout(KEEPALIVE_INTERVAL_SECS)? {
            None => send_keepalive(sess)?,
            Some(node) => {
                if node.tag == "stream:error" {
                    return Err(Error::Ws("stream error from server".into()));
                }
                if handle_iq(sess, &node)? {
                    continue;
                }
                match node.tag.as_str() {
                    "message" => event(SessionEvent::Message(node)),
                    _ => event(SessionEvent::Node(node)),
                }
            }
        }
    }
}

/// Dispatch server iqs the bot understands. Returns `true` if handled.
pub fn handle_iq(sess: &mut Wa2Session, node: &WaNode) -> Result<bool, Error> {
    let kind = node.attr_str("type").unwrap_or_default();
    let is_ping = node
        .attr_str("xmlns")
        .is_some_and(|ns| ns == "urn:xmpp:ping")
        || node.child("ping").is_some();
    if is_ping || (kind == "get" && node.child("ping").is_some()) {
        let reply = WaNode::new("iq")
            .attr("to", node.attr_str("from").unwrap_or("s.whatsapp.net"))
            .attr("type", "result")
            .attr("id", node.attr_str("id").unwrap_or_default());
        sess.send_node(&reply)?;
        return Ok(true);
    }
    Ok(false)
}

/// Events the bot understands; everything else is a generic node log.
pub enum SessionEvent {
    Connected,
    Message(WaNode),
    Node(WaNode),
}

// ─────────────────────────────────────────────────────────────────────────────
// Top-level bot
// ─────────────────────────────────────────────────────────────────────────────

const PAIR_WAIT_SECS: u64 = 90;

/// Load or create the device, pair if needed (printing a scan QR), then serve
/// the logged-in session. Reconnects on disconnect.
pub fn run_bot(
    device_path: &str,
    log: &mut dyn FnMut(&str, &str),
) -> Result<(), Error> {
    let mut dev = if std::path::Path::new(device_path).exists() {
        BotDevice::load(device_path)?
    } else {
        BotDevice::generate()
    };

    loop {
        if !dev.is_paired() {
            let mut sess = Wa2Session::connect()?;
            sess.handshake(&dev.material())?;
            log("handshake", "Noise XX complete (registration payload accepted)");

            match wait_pair_device(&mut sess, &dev)? {
                Some(codes) => {
                    for code in codes {
                        log("qr", &code);
                        eprintln!("QR <- phone: {code}");
                    }
                }
                None => return Err(Error::Ws("server closed before pair-device".into())),
            }

            // The phone must scan and approve; the server then pushes pair-success.
            let mut got = sess.recv_node_timeout(PAIR_WAIT_SECS)?;
            while got.is_none() {
                got = sess.recv_node_timeout(PAIR_WAIT_SECS)?;
            }
            let node = got.expect("loop guarantees node");
            let reply = match node.tag.as_str() {
                "iq" => build_pair_response(&node, &mut dev)?,
                "stream:error" => {
                    return Err(Error::Ws("stream error while waiting for pair-success".into()))
                }
                "failure" => {
                    return Err(Error::Ws(format!(
                        "pairing rejected: {}",
                        node.attr_str("reason").unwrap_or("unknown")
                    )))
                }
                _ => return Err(Error::Ws(format!("unexpected node {}", node.tag))),
            };
            sess.send_node(&reply)?;
            dev.save(device_path)?;
            log(
                "paired",
                &format!(
                    "jid={}@s.whatsapp.net lid=({}) biz={:?} platform={:?}",
                    dev.user, dev.lid_user, dev.business_name, dev.platform
                ),
            );
            log("pairing", "self-signature sent; server closing connection");
            continue; // reconnect as a logged-in device
        }

        // ─── Logged-in session ───
        log("connect", &format!("logging in as {}:{}", dev.user, dev.device));
        let mut sess = Wa2Session::connect()?;
        handshake_login(&mut sess, &dev)?;
        log("handshake", "Noise XX complete (login payload accepted)");

        let mut events_logger = |e: SessionEvent| match e {
            SessionEvent::Connected => {
                log("session", "authenticated — event loop running (keepalive 20s)")
            }
            SessionEvent::Message(n) => {
                log("message", &format!("encrypted message node: {n:?}"))
            }
            SessionEvent::Node(n) => log("node", &format!("{} {:?}", n.tag, n.attrs)),
        };
        match serve(&mut sess, &mut events_logger, &|| false) {
            Err(Error::Ws(e)) if e.contains("closed") => {
                log("disconnect", "server closed the socket — reconnecting");
                continue;
            }
            Err(e) => return Err(e),
            Ok(()) => {
                log("disconnect", "session loop stopped");
                continue;
            }
        }
    }
}

/// Wait for the server's `<iq …><pair-device>` offer and return the QR codes.
pub fn wait_pair_device(
    sess: &mut Wa2Session,
    dev: &BotDevice,
) -> Result<Option<Vec<String>>, Error> {
    loop {
        let node = sess.recv_node()?;
        match node.tag.as_str() {
            "iq" if node.child("pair-device").is_some() => {
                return sess.pair_device(&node, &dev.material()).map(Some);
            }
            "failure" => {
                return Err(Error::Ws(format!(
                    "registration rejected: {}",
                    node.attr_str("reason").unwrap_or("unknown")
                )));
            }
            "stream:error" => return Err(Error::Ws("stream error during registration".into())),
            _ => continue,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Signal wire helpers (matching whatsmeow `prekeys.go` / `send.go` / `user.go`)
// ─────────────────────────────────────────────────────────────────────────────

/// A device bundle handed back by the server's prekey query.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceBundle {
    pub device: u16,
    pub registration_id: u32,
    pub identity_pub: [u8; 32],
    pub pre_key: Option<(u32, [u8; 32])>,
    pub signed_pre_key: (u32, [u8; 32], [u8; 64]),
}

fn u24_be(id: u32) -> [u8; 3] {
    [(id >> 16) as u8, (id >> 8) as u8, id as u8]
}

fn jid_attr(jid: &crate::wa2::Jid) -> WaVal {
    WaVal::Jid(jid.clone())
}

/// `<key id=… value=…>` / `<skey id=… value=… signature=…>` upload node.
pub fn prekey_to_node(id: u32, pub32: &[u8; 32], signature: Option<&[u8; 64]>) -> WaNode {
    let content = vec![
        WaNode { tag: "id".into(), attrs: vec![], content: WaVal::Bytes(u24_be(id).to_vec()) },
        WaNode { tag: "value".into(), attrs: vec![], content: WaVal::Bytes(pub32.to_vec()) },
    ];
    let tag = if signature.is_some() { "skey" } else { "key" };
    let mut node = WaNode { tag: tag.into(), attrs: vec![], content: WaVal::Nodes(content) };
    if let Some(sig) = signature {
        let sig_node =
            WaNode { tag: "signature".into(), attrs: vec![], content: WaVal::Bytes(sig.to_vec()) };
        node.content = WaVal::Nodes(vec![
            WaNode { tag: "id".into(), attrs: vec![], content: WaVal::Bytes(u24_be(id).to_vec()) },
            WaNode { tag: "value".into(), attrs: vec![], content: WaVal::Bytes(pub32.to_vec()) },
            sig_node,
        ]);
    }
    node
}

/// Usync query asking for the device list of one PN (`mode=query`,
/// `context=message`, `<devices version="2">`).
pub fn build_usync_devices_iq(req_id: &str, sid: &str, target_pn: &str) -> WaNode {
    let mut user = WaNode::new("user");
    user.attrs.push(("jid".into(), jid_attr(&Jid::new(target_pn, "s.whatsapp.net"))));

    let mut devices = WaNode::new("devices");
    devices.attrs.push(("version".into(), WaVal::Str("2".into())));

    let mut query = WaNode::new("query");
    query.content = WaVal::Nodes(vec![devices]);
    let mut list = WaNode::new("list");
    list.content = WaVal::Nodes(vec![user]);

    let mut usync = WaNode::new("usync");
    usync.attrs.push(("sid".into(), WaVal::Str(sid.into())));
    usync.attrs.push(("mode".into(), WaVal::Str("query".into())));
    usync.attrs.push(("last".into(), WaVal::Str("true".into())));
    usync.attrs.push(("index".into(), WaVal::Str("0".into())));
    usync.attrs.push(("context".into(), WaVal::Str("message".into())));
    usync.content = WaVal::Nodes(vec![query, list]);

    WaNode::new("iq")
        .attr("type", "get")
        .attr("to", "s.whatsapp.net")
        .attr("xmlns", "usync")
        .attr("id", req_id)
        .with_child(usync)
}

impl WaNode {
    fn with_child(mut self, child: WaNode) -> Self {
        let mut children = match self.content {
            WaVal::Nodes(n) => n,
            _ => Vec::new(),
        };
        children.push(child);
        self.content = WaVal::Nodes(children);
        self
    }
}

/// Parse the device list out of a usync response (`<usync><list><user …>
/// <devices><device-list><device id=…/>`). Returns `(user, device)` pairs.
pub fn parse_usync_device_list(node: &WaNode) -> Vec<(String, u16)> {
    let mut out = Vec::new();
    let Some(usync) = node.child("usync") else { return out };
    let Some(list) = usync.child("list") else { return out };
    for user in list.children() {
        if user.tag != "user" {
            continue;
        }
        let user_val = user.attrs.iter().find(|(k, _)| k == "jid").and_then(|(_, v)| match v {
            WaVal::Jid(j) => Some(j.user.clone()),
            WaVal::Str(s) => Some(s.split_once('@').map(|(u, _)| u.to_string()).unwrap_or_else(|| s.clone())),
            _ => None,
        });
        let Some(user_val) = user_val else { continue };
        let Some(devices) = user.child("devices") else { continue };
        let Some(device_list) = devices.child("device-list") else { continue };
        for device in device_list.children() {
            if device.tag != "device" {
                continue;
            }
            let Some(id) = device.attr_str("id").and_then(|s| s.parse::<u16>().ok()) else {
                continue;
            };
            if id == 0 {
                continue;
            }
            out.push((user_val.clone(), id));
        }
    }
    out
}

/// Prekey-bundle fetch for one device: `<key><user jid="<addr>" reason="identity"/>`.
pub fn build_fetch_prekeys_iq(req_id: &str, device_jid: &Jid) -> WaNode {
    let mut user = WaNode::new("user");
    user.attrs.push(("jid".into(), jid_attr(device_jid)));
    user.attrs.push(("reason".into(), WaVal::Str("identity".into())));
    let mut key = WaNode::new("key");
    key.content = WaVal::Nodes(vec![user]);

    WaNode::new("iq")
        .attr("type", "get")
        .attr("to", "s.whatsapp.net")
        .attr("xmlns", "encrypt")
        .attr("id", req_id)
        .with_child(key)
}

fn node_bytes(node: &WaNode, tag: &str, len: usize) -> Result<Vec<u8>, Error> {
    let child = node.child(tag).ok_or_else(|| Error::Codec(format!("missing {tag}")))?;
    let WaVal::Bytes(b) = &child.content else {
        return Err(Error::Codec(format!("{tag} is not bytes")));
    };
    if b.len() != len {
        return Err(Error::Codec(format!("{tag} length {} != {len}", b.len())));
    }
    Ok(b.clone())
}

/// Parse one `<user>` from a prekey response into a [`DeviceBundle`].
pub fn parse_prekey_bundle_user(node: &WaNode) -> Result<(String, u16, DeviceBundle), Error> {
    let jid = match node.attrs.iter().find(|(k, _)| k == "jid") {
        Some((_, WaVal::Jid(j))) => j.clone(),
        _ => return Err(Error::Codec("no jid on user node".into())),
    };
    let user = jid.user.clone();
    let device = jid.device;
    if node.child("error").is_some() {
        return Err(Error::Codec(format!("server refused prekey for {user}:{device}")));
    }
    let registration = node_bytes(node, "registration", 4)?;
    let registration_id = u32::from_be_bytes(registration.try_into().unwrap());
    let identity_pub: [u8; 32] = node_bytes(node, "identity", 32)?.try_into().unwrap();

    let keys = match node.child("keys") {
        Some(k) => k.clone(),
        None => node.clone(),
    };
    let mut pre_key = None;
    if let Some(k) = keys.child("key") {
        let id = parse_prekey_id(k)?;
        let pub32 = node_bytes(k, "value", 32)?;
        pre_key = Some((id, pub32.try_into().unwrap()));
    }
    let skey = keys.child("skey").ok_or_else(|| Error::Codec("missing skey".into()))?;
    let signed_id = parse_prekey_id(skey)?;
    let signed_pub: [u8; 32] = node_bytes(skey, "value", 32)?.try_into().unwrap();
    let signed_sig: [u8; 64] = node_bytes(skey, "signature", 64)?.try_into().unwrap();

    Ok((
        user,
        device,
        DeviceBundle {
            device,
            registration_id,
            identity_pub,
            pre_key,
            signed_pre_key: (signed_id, signed_pub, signed_sig),
        },
    ))
}

fn parse_prekey_id(node: &WaNode) -> Result<u32, Error> {
    let id = node_bytes(node, "id", 3)?;
    Ok(u32::from_be_bytes([0, id[0], id[1], id[2]]))
}

/// Upload our registration + prekeys to the server (mirrors whatsmeow
/// `uploadPreKeys`, minus the `type` prefix on `identity` — that stays raw).
pub fn build_upload_prekeys_iq(
    req_id: &str,
    dev: &BotDevice,
    prekeys: &[OneTimePreKey],
) -> WaNode {
    let registration = WaNode { tag: "registration".into(), attrs: vec![], content: WaVal::Bytes(dev.registration_id.to_be_bytes().to_vec()) };
    let mut ty = WaNode::new("type");
    ty.content = WaVal::Bytes(vec![ecc_curve_djb_type()]);
    let mut identity = WaNode::new("identity");
    identity.content = WaVal::Bytes(dev.identity_pub.to_vec());

    let mut list = WaNode::new("list");
    list.content = WaVal::Nodes(
        prekeys.iter().map(|k| prekey_to_node(k.id, &k.pub_key, None)).collect(),
    );
    let skey = prekey_to_node(
        dev.prekey_id,
        &dev.signed_prekey_pub,
        Some(&dev.signed_prekey_sig),
    );

    WaNode::new("iq")
        .attr("type", "set")
        .attr("to", "s.whatsapp.net")
        .attr("xmlns", "encrypt")
        .attr("id", req_id)
        .with_child(registration)
        .with_child(ty)
        .with_child(identity)
        .with_child(list)
        .with_child(skey)
}

/// whatsmeow `padMessage`: one random byte `n` in `1..=15`, then `n` copies.
pub fn pad_message(plaintext: &[u8]) -> Vec<u8> {
    let n = (rand::random::<u8>() & 0xf).max(1) as usize;
    let mut out = Vec::with_capacity(plaintext.len() + n);
    out.extend_from_slice(plaintext);
    out.extend(std::iter::repeat_n(n as u8, n));
    out
}

/// whatsmeow `unpadMessage` — `v==3` messages carry no padding.
pub fn unpad_message(plaintext: &[u8], version: u8) -> Result<Vec<u8>, Error> {
    if version == 3 {
        return Ok(plaintext.to_vec());
    }
    if plaintext.is_empty() {
        return Err(Error::Codec("empty plaintext".into()));
    }
    let n = plaintext[plaintext.len() - 1] as usize;
    if n == 0 || n > plaintext.len() {
        return Err(Error::Codec("bad padding".into()));
    }
    if plaintext[plaintext.len() - n..].iter().any(|&b| b != n as u8) {
        return Err(Error::Codec("bad padding".into()));
    }
    Ok(plaintext[..plaintext.len() - n].to_vec())
}

/// Build an `<enc v="2" type=msg|pkmsg>` node for the wire.
pub fn make_enc_node(ciphertext: &crate::wa_signal_session::CiphertextMsg) -> WaNode {
    let typ = match ciphertext.msg_type() {
        crate::wa_signal_session::CiphertextType::PreKey => "pkmsg",
        _ => "msg",
    };
    WaNode {
        tag: "enc".into(),
        attrs: vec![
            ("v".into(), WaVal::Str("2".into())),
            ("type".into(), WaVal::Str(typ.into())),
        ],
        content: WaVal::Bytes(ciphertext.bytes().to_vec()),
    }
}

/// The `(version, type, payload)` of a message node's `<enc>` child, if any.
pub fn enc_child(node: &WaNode) -> Option<(u8, String, &[u8])> {
    let enc = node.child("enc")?;
    let v = enc.attr_str("v").and_then(|s| s.parse().ok()).unwrap_or(2);
    let typ = enc.attr_str("type").unwrap_or("msg").to_string();
    let WaVal::Bytes(b) = &enc.content else { return None };
    Some((v, typ, b))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// Simulate a phone that scanned our QR: forge the phone-side ADV
    /// signature chain using the QR's adv secret key.
    fn forge_phone_pair_success(dev: &BotDevice) -> WaNode {
        // The phone owns an "account key". Sign (adv prefix ∥ details ∥ our
        // identity pub) — exactly what handlePair will re-verify.
        let (account_priv, account_pub) = generate_ephemeral();

        let mut adv_details = Vec::new();
        pb::field_uint(1, 719, &mut adv_details); // rawID
        pb::field_uint(2, 1720000000, &mut adv_details); // timestamp
        pb::field_uint(3, 1, &mut adv_details); // keyIndex
        pb::field_uint(4, 0, &mut adv_details); // accountType E2EE
        pb::field_uint(5, 0, &mut adv_details); // deviceType E2EE

        let mut account_message = vec![];
        account_message.extend_from_slice(ADV_ACCOUNT_PREFIX);
        account_message.extend_from_slice(&adv_details);
        account_message.extend_from_slice(&dev.identity_pub);
        let account_sig = xed25519_sign(&account_priv, &account_message);

        let mut signed = Vec::new();
        pb::field_bytes(1, &adv_details, &mut signed);
        pb::field_bytes(2, &account_pub, &mut signed);
        pb::field_bytes(3, &account_sig, &mut signed);

        // ADVSignedDeviceIdentityHMAC: hmac over the signed ADV payload.
        let mut mac = HmacSha256::new_from_slice(&dev.adv_secret_key).unwrap();
        mac.update(&signed);
        let hmac = mac.finalize().into_bytes();

        let mut container = Vec::new();
        pb::field_bytes(1, &signed, &mut container);
        pb::field_bytes(2, &hmac, &mut container);
        pb::field_uint(3, 0, &mut container); // accountType E2EE

        let mut dev_identity = WaNode::new("device-identity");
        dev_identity.content = WaVal::Bytes(container);
        let mut dev_node = WaNode::new("device");
        dev_node.attrs.push((
            "jid".into(),
            WaVal::Jid(Jid {
                user: "6012345678".into(),
                server: "s.whatsapp.net".into(),
                device: 12,
                agent: 0,
                integrator: 0,
            }),
        ));
        dev_node.attrs.push((
            "lid".into(),
            WaVal::Str("9123456789012:0@s.whatsapp.net".into()),
        ));
        let mut biz = WaNode::new("biz");
        biz.attrs.push(("name".into(), WaVal::Str("ZEN Bot Co".into())));
        let mut platform = WaNode::new("platform");
        platform.attrs.push(("name".into(), WaVal::Str("SMBA".into())));

        let pair_success = {
            let mut n = WaNode::new("pair-success");
            n.content = WaVal::Nodes(vec![dev_identity, dev_node, biz, platform]);
            n
        };
        let mut iq = WaNode::new("iq");
        iq.attrs.push(("to".into(), WaVal::Str("s.whatsapp.net".into())));
        iq.attrs.push(("type".into(), WaVal::Str("set".into())));
        iq.attrs.push(("id".into(), WaVal::Str("pair-req-7".into())));
        iq.content = WaVal::Nodes(vec![pair_success]);
        iq
    }

    #[test]
    fn pair_success_roundtrip() {
        let mut dev = BotDevice::generate();
        assert!(!dev.is_paired());

        let iq = forge_phone_pair_success(&dev);
        let reply = build_pair_response(&iq, &mut dev).expect("pair-success accepted");

        assert!(dev.is_paired(), "device must be marked paired");
        assert_eq!(dev.user, "6012345678");
        assert_eq!(dev.device, 12);
        assert_eq!(dev.lid_user, "9123456789012");
        assert!(!dev.account.is_empty(), "account proto must be stored");
        assert_eq!(dev.platform, "SMBA");
        assert_eq!(dev.business_name, "ZEN Bot Co");

        // Reply shape: <iq type=result><pair-device-sign><device-identity key-index=1>…
        assert_eq!(reply.attr_str("type"), Some("result"));
        let sign = reply.child("pair-device-sign").expect("pair-device-sign node");
        let di = sign.child("device-identity").expect("device-identity node");
        assert_eq!(di.attr_str("key-index"), Some("1"));
        let WaVal::Bytes(self_signed) = &di.content else {
            panic!("device-identity must be bytes");
        };

        // The self-signed proto must contain details + accountSignature +
        // deviceSignature, and our device signature must verify.
        let stored = AdvSignedDeviceIdentity::parse(self_signed).unwrap();
        assert!(!stored.device_signature.is_empty());
        assert!(stored.account_signature_key.is_empty(), "self-signed strips the account key");

        let sig: [u8; 64] = stored.device_signature.as_slice().try_into().unwrap();
        let mut msg = vec![];
        msg.extend_from_slice(ADV_DEVICE_PREFIX);
        msg.extend_from_slice(&stored.details);
        msg.extend_from_slice(&dev.identity_pub);
        let account_key = AdvSignedDeviceIdentity::parse(&dev.account)
            .unwrap()
            .account_signature_key;
        msg.extend_from_slice(&account_key);
        assert!(
            xed25519_verify(&dev.identity_pub, &sig, &msg),
            "device self-signature must verify"
        );
    }

    #[test]
    fn pair_success_rejects_wrong_hmac() {
        let mut dev = BotDevice::generate();
        let iq = forge_phone_pair_success(&dev);
        // Corrupt the stored adv key so the HMAC no longer matches.
        let (_, nk) = generate_ephemeral();
        dev.adv_secret_key = nk;
        assert!(build_pair_response(&iq, &mut dev).is_err(), "hmac mismatch must fail");
        assert!(!dev.is_paired(), "failed pairing must not mark the device paired");
    }

    #[test]
    fn bot_device_persistence() {
        let mut dev = BotDevice::generate();
        // One-time prekeys + sessions must survive a save/load cycle.
        dev.gen_prekeys(3);
        dev.mark_prekeys_uploaded(dev.one_time_prekeys[1].id);
        let sess = vec![9u8; 32];
        let addr = "6012345678.1".to_string();
        dev.put_session(&addr, sess.clone());
        let path = std::env::temp_dir().join(format!("zen_bot_{}.txt", hex(&dev.adv_secret_key)));
        let path = path.to_str().unwrap().to_string();
        dev.save(&path).unwrap();
        let loaded = BotDevice::load(&path).unwrap();
        assert_eq!(dev.identity_priv, loaded.identity_priv);
        assert_eq!(dev.noise_pub, loaded.noise_pub);
        assert_eq!(dev.signed_prekey_sig, loaded.signed_prekey_sig);
        assert_eq!(dev.signed_prekey_priv, loaded.signed_prekey_priv);
        assert_eq!(dev.adv_secret_key, loaded.adv_secret_key);
        assert_eq!(dev.next_prekey_id, loaded.next_prekey_id);
        assert_eq!(dev.one_time_prekeys, loaded.one_time_prekeys);
        assert_eq!(loaded.get_session(&addr), Some(sess.as_slice()));
        // Consuming a prekey removes it from the pool.
        let mut dev2 = loaded;
        let took = dev2.take_prekey(dev2.one_time_prekeys[0].id).expect("prekey present");
        assert!(took.pub_key.iter().any(|&b| b != 0));
        assert_eq!(dev2.one_time_prekeys.len(), 2);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn login_payload_shape() {
        use crate::wa2::pb;
        let mut dev = BotDevice::generate();
        dev.user = "6012345678".into();
        dev.device = 2;
        let payload = build_login_payload(&dev);
        let fields = pb::parse(&payload).unwrap();
        assert_eq!(pb::find_var(&fields, 1), Some(6012345678)); // username
        assert_eq!(pb::find_var(&fields, 3), Some(1)); // passive
        assert_eq!(pb::find_var(&fields, 18), Some(2)); // device
        assert_eq!(pb::find_var(&fields, 24), Some(1)); // lc
        assert_eq!(pb::find_var(&fields, 33), Some(1)); // pull
        assert_eq!(pb::find_var(&fields, 41), Some(1)); // lidDbMigrated
        assert!(pb::find_bytes(&fields, 5).is_some()); // userAgent
    }

    #[test]
    fn pad_message_round_trips_v2() {
        let original = b"hi there, this is a test message!";
        for _ in 0..50 {
            let padded = pad_message(original);
            assert!(padded.len() >= original.len());
            assert_eq!(unpad_message(&padded, 2).unwrap(), original);
        }
        assert_eq!(unpad_message(original, 3).unwrap(), original);
        assert!(unpad_message(b"", 2).is_err());
        assert!(unpad_message(b"\x05abcde", 2).is_err());
    }

    #[test]
    fn usync_devices_query_shape_and_parse() {
        let iq = build_usync_devices_iq("req-1", "abc123", "6012345678");
        assert_eq!(iq.attr_str("type"), Some("get"));
        assert_eq!(iq.attr_str("xmlns"), Some("usync"));
        let usync = iq.child("usync").unwrap();
        assert_eq!(usync.attr_str("mode"), Some("query"));
        assert_eq!(usync.attr_str("context"), Some("message"));
        assert_eq!(usync.attr_str("last"), Some("true"));
        let query = usync.child("query").unwrap();
        assert_eq!(query.child("devices").unwrap().attr_str("version"), Some("2"));

        // Fake server response.
        let mkdev = |id: u16| {
            let mut d = WaNode::new("device");
            d.attrs.push(("id".into(), WaVal::Str(id.to_string())));
            d
        };
        let mut dl = WaNode::new("device-list");
        dl.content = WaVal::Nodes(vec![mkdev(1), mkdev(3)]);
        let mut devs = WaNode::new("devices");
        devs.attrs.push(("version".into(), WaVal::Str("2".into())));
        devs.content = WaVal::Nodes(vec![dl]);
        let mut user = WaNode::new("user");
        user.attrs.push(("jid".into(), jid_attr(&Jid::new("6012345678", "s.whatsapp.net"))));
        user.content = WaVal::Nodes(vec![devs]);
        let mut list = WaNode::new("list");
        list.content = WaVal::Nodes(vec![user]);
        let mut usync_resp = WaNode::new("usync");
        usync_resp.content = WaVal::Nodes(vec![list]);
        let mut resp = WaNode::new("iq");
        resp.content = WaVal::Nodes(vec![usync_resp]);

        assert_eq!(
            parse_usync_device_list(&resp),
            vec![("6012345678".to_string(), 1), ("6012345678".to_string(), 3)]
        );
        assert_eq!(parse_usync_device_list(&WaNode::new("iq")), vec![]);
    }

    #[test]
    fn prekey_upload_and_fetch_nodes() {
        let mut dev = BotDevice::generate();
        let keys = dev.gen_prekeys(2);

        let up = build_upload_prekeys_iq("up-1", &dev, &keys);
        assert_eq!(up.attr_str("type"), Some("set"));
        assert_eq!(up.attr_str("xmlns"), Some("encrypt"));
        let WaVal::Bytes(reg) = &up.child("registration").unwrap().content else {
            panic!("registration must be bytes")
        };
        assert_eq!(u32::from_be_bytes(reg.as_slice().try_into().unwrap()), dev.registration_id);
        let WaVal::Bytes(ty) = &up.child("type").unwrap().content else { panic!("type bytes") };
        assert_eq!(ty, &[0x05]);
        let list = up.child("list").unwrap();
        let keys_nodes = list.children();
        assert_eq!(keys_nodes.len(), 2);
        let WaVal::Bytes(id) = &keys_nodes[0].child("id").unwrap().content else { panic!("id") };
        assert_eq!(u32::from_be_bytes([0, id[0], id[1], id[2]]), keys[0].id);
        let skey = up.child("skey").unwrap();
        assert!(skey.child("signature").is_some());

        // Server response to a fetch query.
        let mut user = WaNode::new("user");
        user.attrs.push(("jid".into(), jid_attr(&Jid::advanced("6012345678", "s.whatsapp.net", 0, 4))));
        let mut reg = WaNode::new("registration");
        reg.content = WaVal::Bytes(2222u32.to_be_bytes().to_vec());
        let mut ident = WaNode::new("identity");
        ident.content = WaVal::Bytes(vec![7u8; 32]);
        let mut key = WaNode::new("key");
        key.content = WaVal::Nodes(vec![
            WaNode { tag: "id".into(), attrs: vec![], content: WaVal::Bytes(u24_be(9).to_vec()) },
            WaNode { tag: "value".into(), attrs: vec![], content: WaVal::Bytes(vec![9u8; 32]) },
        ]);
        let mut skey2 = WaNode::new("skey");
        skey2.content = WaVal::Nodes(vec![
            WaNode { tag: "id".into(), attrs: vec![], content: WaVal::Bytes(u24_be(5).to_vec()) },
            WaNode { tag: "value".into(), attrs: vec![], content: WaVal::Bytes(vec![5u8; 32]) },
            WaNode { tag: "signature".into(), attrs: vec![], content: WaVal::Bytes(vec![8u8; 64]) },
        ]);
        user.content = WaVal::Nodes(vec![reg.clone(), ident.clone(), key, skey2.clone()]);

        let (u, d, bundle) = parse_prekey_bundle_user(&user).unwrap();
        assert_eq!(u, "6012345678");
        assert_eq!(d, 4);
        assert_eq!(bundle.device, 4);
        assert_eq!(bundle.registration_id, 2222);
        assert_eq!(bundle.pre_key, Some((9, [9u8; 32])));
        assert_eq!(bundle.signed_pre_key.0, 5);
        assert_eq!(bundle.signed_pre_key.1, [5u8; 32]);
        assert_eq!(bundle.signed_pre_key.2, [8u8; 64]);

        // No one-time key → pre_key stays None.
        let mut user2 = WaNode::new("user");
        user2.attrs.push(("jid".into(), jid_attr(&Jid::new("6012345678", "s.whatsapp.net"))));
        user2.content = WaVal::Nodes(vec![reg.clone(), ident.clone(), skey2.clone()]);
        let (_, _, b2) = parse_prekey_bundle_user(&user2).unwrap();
        assert_eq!(b2.pre_key, None);
    }

    #[test]
    fn enc_node_round_trip() {
        let mut body = vec![0x0a, 33];
        body.extend_from_slice(&[0x07; 33]);
        body.extend_from_slice(&[0x10, 0, 0x18, 0, 0x22, 0]);
        body.extend_from_slice(&[0u8; 8]); // trailing MAC slice captured by from_bytes
        let sig = crate::wa_signal_session::SignalMsg::from_bytes(&[vec![0x33], body.clone()].concat())
            .unwrap();
        let ct = crate::wa_signal_session::CiphertextMsg::Signal(sig);
        let node = make_enc_node(&ct);
        let mut msg = WaNode::new("message");
        msg.content = WaVal::Nodes(vec![node]);
        let (v, typ, bytes) = enc_child(&msg).unwrap();
        assert_eq!(v, 2);
        assert_eq!(typ, "msg");
        assert!(!bytes.is_empty());

        // PreKey messages must be tagged pkmsg.
        let pk = crate::wa_signal_session::PreKeyMsg {
            version: 3,
            registration_id: 1,
            pre_key_id: Some(9),
            signed_pre_key_id: 5,
            base_key: [0x05; 33],
            identity_key: [0x06; 33],
            message: crate::wa_signal_session::SignalMsg {
                version: 3,
                sender_ratchet_key: [0x07; 33],
                previous_counter: 0,
                counter: 0,
                ciphertext: vec![1, 2, 3],
                serialized: vec![0x33, 1, 2, 3],
            },
            serialized: vec![0x33, 9, 9],
        };
        let n2 = make_enc_node(&crate::wa_signal_session::CiphertextMsg::PreKey(pk));
        assert_eq!(n2.attr_str("type"), Some("pkmsg"));
    }
}