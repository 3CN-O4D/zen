//! Zen `wa` module — WhatsApp on the **native** Rust client (`wa2` + `wa_bot`).
//!
//! Replaces the old Node/Baileys bridge with a worker thread that owns the
//! Noise session. The zen script drives it with the same public API as before:
//!
//! ```python
//! import wa
//! wa.connect("~/.zen/wa_zen", "")
//! while wa.state() not in ("open", "error"):
//!     if wa.qr():
//!         print("SCAN:", wa.qr())
//!         break
//!     sleep(0.5)
//! while True:
//!     for msg in wa.poll(500):
//!         print("incoming:", msg)
//!     sleep(0.5)
//! ```
//!
//! States: `starting`, `connecting`, `qr`, `pairing`, `open`, `closed`,
//! `error`, `idle`.

use crate::runtime::Value;
use crate::wa_bot::{
    build_pair_response, build_usync_devices_iq, enc_child, handshake_login, handle_iq, keepalive,
    unpad_message, wait_pair_device, BotDevice,
};
use base64::Engine as _;
use crate::wa_signal::SessionRecord;
use crate::wa_signal_session::{self, CiphertextMsg, LocalKeys};
use crate::wa2::{pb, Jid, Wa2Session, WaNode, WaVal};
use rand::Rng as _;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// How long to block on one socket read inside the worker loop.
const RECV_TICK_SECS: u64 = 5;
/// Send a keepalive ping after this much socket silence.
const KEEPALIVE_AFTER_SECS: u64 = 20;
/// How long a fresh QR is valid before the server expires it (wait budget).
const PAIR_WAIT_SECS: u64 = 120;

// ── shared per-interpreter session state ────────────────────────────────

/// Media metadata for one downloadable attachment (routed from an event).
#[derive(Clone, Debug)]
struct MediaSpec {
    direct_path: String,
    media_key: Option<Vec<u8>>,
    enc_sha: Option<Vec<u8>>,
    sha: Option<Vec<u8>>,
    media_type: String,
    mime: String,
}

/// A file to send with `wa.sendFile`.
struct FilePayload {
    bytes: Vec<u8>,
    kind: String, // document | image | video | audio | sticker
    mime: String,
    filename: String,
    caption: String,
}

/// An outbound message, handed to the worker thread (which owns the
/// socket and the Signal session store) from the zen interpreter thread.
struct SendJob {
    to: String,
    text: String,
    /// Non-empty when this job sends a file instead of text.
    file: Option<FilePayload>,
    ack: mpsc::Sender<Result<(), String>>,
}

/// A media download request, also handled on the worker thread.
struct DownloadJob {
    spec: MediaSpec,
    ack: mpsc::Sender<Result<Vec<u8>, String>>,
}

struct NativeSession {
    /// Set by `disconnect`/`logout` to ask the worker to stop.
    stop: AtomicBool,
    state: Mutex<String>,
    qr: Mutex<Option<String>>,
    last_error: Mutex<Option<String>>,
    messages: Mutex<VecDeque<Value>>,
    /// Outbound message requests queued by the interpreter thread.
    send_pending: Mutex<VecDeque<SendJob>>,
    /// Media download requests queued by the interpreter thread.
    download_pending: Mutex<VecDeque<DownloadJob>>,
    send_condvar: Condvar,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl NativeSession {
    fn opened() -> Arc<NativeSession> {
        Arc::new(NativeSession {
            stop: AtomicBool::new(false),
            state: Mutex::new("starting".into()),
            qr: Mutex::new(None),
            last_error: Mutex::new(None),
            messages: Mutex::new(VecDeque::new()),
            send_pending: Mutex::new(VecDeque::new()),
            download_pending: Mutex::new(VecDeque::new()),
            send_condvar: Condvar::new(),
            worker: Mutex::new(None),
        })
    }

    fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.worker.lock().unwrap().take() {
            // The worker polls stop() every few seconds, so a bounded wait is
            // enough; if it ends up stuck on the network we give up and leak
            // the OS thread (the process is exiting anyway in that case).
            let deadline = Instant::now() + Duration::from_secs(30);
            while !h.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

thread_local! {
    static NATIVE_SESSION: std::cell::RefCell<Option<Arc<NativeSession>>> =
        const { RefCell::new(None) };
}

fn take_session() -> Option<Arc<NativeSession>> {
    NATIVE_SESSION.with(|s| s.borrow_mut().take())
}

fn current_session() -> Option<Arc<NativeSession>> {
    NATIVE_SESSION.with(|s| s.borrow().clone())
}

fn set_state(sess: &Arc<NativeSession>, state: &str) {
    *sess.state.lock().unwrap() = state.to_string();
}

fn report_error(sess: &Arc<NativeSession>, err: &str) {
    *sess.last_error.lock().unwrap() = Some(err.to_string());
    set_state(sess, "error");
}

// ── paths / device file ──────────────────────────────────────────────────

fn dirs_home() -> PathBuf {
    if let Ok(h) = std::env::var("HOME") {
        return PathBuf::from(h);
    }
    PathBuf::from(".")
}

fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        return dirs_home().join(rest);
    }
    PathBuf::from(p)
}

// ── worker ──────────────────────────────────────────────────────────────

fn run_worker(sess: Arc<NativeSession>, auth_dir: PathBuf) {
    std::fs::create_dir_all(&auth_dir).ok();
    let device_path = auth_dir.join("zen_wa_bot.txt");
    let device_str = device_path.to_string_lossy().into_owned();

    let mut dev = if device_path.exists() {
        BotDevice::load(&device_str).unwrap_or_else(|_| BotDevice::generate())
    } else {
        BotDevice::generate()
    };

    loop {
        if sess.stop.load(Ordering::Relaxed) {
            set_state(&sess, "idle");
            return;
        }

        if !dev.is_paired() {
            // ── Pairing phase: QR → phone scans → pair-success ──
            if let Err(err) = run_pair_phase(&sess, &mut dev, &device_str) {
                if sess.stop.load(Ordering::Relaxed) {
                    continue;
                }
                report_error(&sess, &err);
                std::thread::sleep(Duration::from_secs(3));
            }
            continue;
        }

        // ── Logged-in phase: (re)connect and serve ──
        set_state(&sess, "connecting");
        let result = run_login_phase(&sess, &mut dev);
        if sess.stop.load(Ordering::Relaxed) {
            set_state(&sess, "idle");
            return;
        }
        match result {
            Ok(()) => set_state(&sess, "closed"),
            Err(err) => report_error(&sess, &err),
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

/// Registration handshake → QR offer → wait for the phone's pair-success.
fn run_pair_phase(
    sess: &Arc<NativeSession>,
    dev: &mut BotDevice,
    device_path: &str,
) -> Result<(), String> {
    set_state(sess, "connecting");
    let mut s = Wa2Session::connect().map_err(|e| format!("connect: {e}"))?;
    s.handshake(&dev.material()).map_err(|e| format!("handshake: {e}"))?;
    let codes = wait_pair_device(&mut s, dev)
        .map_err(|e| format!("wait pair-device: {e}"))?
        .ok_or_else(|| "server closed during registration".to_string())?;
    let qr = codes.into_iter().next().unwrap_or_default();
    *sess.qr.lock().unwrap() = Some(qr);
    // Stay in the `qr` state (with `wa.qr()` set) until the phone scans.
    set_state(sess, "qr");

    loop {
        if sess.stop.load(Ordering::Relaxed) {
            return Err("stopped".into());
        }
        match s.recv_node_timeout(PAIR_WAIT_SECS * 1000).map_err(|e| format!("pair-success recv: {e}"))? {
            None => continue, // QR still valid; keep waiting for the scan
            Some(node) => {
                if node.tag == "stream:error" {
                    return Err("stream error while pairing".into());
                }
                if node.tag != "iq" {
                    continue;
                }
                let reply = build_pair_response(&node, dev)
                    .map_err(|e| format!("pair-success verify failed: {e}"))?;
                s.send_node(&reply).map_err(|e| format!("pair-device-sign send: {e}"))?;
                dev.save(device_path).map_err(|e| format!("save device: {e}"))?;
                set_state(sess, "open");
                return Ok(());
            }
        }
    }
}

/// Login handshake and an event loop that also answers server pings,
/// sends keepalives, watches `stop()`, drains outbound text messages, and
/// decrypts inbound Signal messages.
fn run_login_phase(sess: &Arc<NativeSession>, dev: &mut BotDevice) -> Result<(), String> {
    let mut s = Wa2Session::connect().map_err(|e| format!("connect: {e}"))?;
    handshake_login(&mut s, dev).map_err(|e| format!("login handshake: {e}"))?;
    set_state(sess, "open");

    let mut last_rx = Instant::now();
    let mut first_frame = true;
    loop {
        if sess.stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        // While a send/download is queued we poll the socket briskly so the
        // request is not delayed by a full receive timeout.
        let pending_send = sess.send_pending.lock().unwrap().front().is_some()
            || sess.download_pending.lock().unwrap().front().is_some();
        let tick = if pending_send { 200 } else { RECV_TICK_SECS * 1000 };
        match s
            .recv_node_timeout(tick)
            .map_err(|e| format!("recv: {e}"))?
        {
            None => {
                if last_rx.elapsed() >= Duration::from_secs(KEEPALIVE_AFTER_SECS) {
                    keepalive(&mut s).map_err(|e| format!("keepalive: {e}"))?;
                }
            }
            Some(node) => {
                last_rx = Instant::now();
                if node.tag == "stream:error" {
                    return Err("stream error from server".into());
                }
                if first_frame {
                    // The first post-login frame is `<success>` or `<failure>`.
                    first_frame = false;
                    match node.tag.as_str() {
                        "success" => {
                            set_state(sess, "open");
                            upload_prekeys(&mut s, dev)
                                .map_err(|e| format!("prekey upload: {e}"))?;
                        }
                        "failure" => {
                            return Err(format!(
                                "login rejected: {}",
                                node.attr_str("reason").unwrap_or("unknown")
                            ))
                        }
                        _ => {}
                    }
                }
                if handle_iq(&mut s, &node).map_err(|e| format!("iq: {e}"))? {
                    continue;
                }
                push_event(sess, dev, &node);
            }
        }
        drain_jobs(&mut s, sess, dev)?;
    }
}

/// Upload a fresh batch of one-time prekeys after a successful login.
fn upload_prekeys(s: &mut Wa2Session, dev: &mut BotDevice) -> Result<(), String> {
    if !dev.one_time_prekeys.iter().any(|k| !k.uploaded) {
        return Ok(());
    }
    let fresh = dev.gen_prekeys(20);
    let node = crate::wa_bot::build_upload_prekeys_iq(&new_id(), dev, &fresh);
    s.send_node(&node).map_err(|e| format!("send upload: {e}"))?;
    let resp = wait_iq(s, node.attr_str("id").unwrap_or_default())?;
    if resp.attr_str("type") != Some("result") {
        return Err("prekey upload rejected".into());
    }
    dev.mark_prekeys_uploaded(fresh.iter().map(|k| k.id).max().unwrap_or(0));
    Ok(())
}

/// Wrap a raw 32-byte Curve25519 key in the 33-byte `0x05` DJB format the
/// Signal layer uses everywhere.
fn signal_pub(raw: &[u8; 32]) -> [u8; 33] {
    let mut out = [0u8; 33];
    out[0] = 0x05;
    out[1..].copy_from_slice(raw);
    out
}

/// Parse a `user@server` (or `user:device@server`) send target.
fn parse_to_jid(raw: &str) -> Result<(String, String), String> {
    let (user, server) = raw
        .rsplit_once('@')
        .ok_or_else(|| format!("invalid jid {raw:?} (expected user@server)"))?;
    let user = user.split(':').next().unwrap_or("").to_string();
    if user.is_empty() || server.is_empty() {
        return Err(format!("invalid jid {raw:?}"));
    }
    Ok((user, server.to_string()))
}

fn new_id() -> String {
    format!("{:016x}", rand::rng().random::<u64>())
}

/// The `(user, device)` of a node's `from` jid.
fn from_jid_of(node: &WaNode) -> Option<(String, u16)> {
    let j = node.attrs.iter().find(|(k, _)| k == "from")?.1.clone();
    match j {
        WaVal::Jid(j) => Some((j.user, j.device)),
        WaVal::Str(s) => {
            let (user, rest) = s.split_once('@')?;
            let device = user.rsplit_once(':').map(|(_, d)| d.parse().unwrap_or(0)).unwrap_or(0);
            let user = user.rsplit_once(':').map(|(u, _)| u).unwrap_or(user).to_string();
            let _ = rest;
            Some((user, device))
        }
        _ => None,
    }
}

/// Try to decrypt the `<enc>` children of an inbound `<message>`, mutating the
/// session store as sessions advance. Returns the unpadded plaintext Message
/// protobuf and the sending device.
fn decrypt_inbound(dev: &mut BotDevice, node: &WaNode) -> Result<(Vec<u8>, u16), String> {
    let (user, rdevice) = from_jid_of(node).ok_or_else(|| "message has no from".to_string())?;
    let user_prefix = format!("{user}.");
    let mut addrs = vec![BotDevice::signal_address(&user, rdevice)];
    for s in &dev.sessions {
        if s.address.starts_with(&user_prefix) && !addrs.contains(&s.address) {
            addrs.push(s.address.clone());
        }
    }

    for child in node.children() {
        if child.tag != "enc" {
            continue;
        }
        let tmp = WaNode {
            tag: child.tag.clone(),
            attrs: child.attrs.clone(),
            content: child.content.clone(),
        };
        let Some((v, typ, bytes)) = enc_child(&tmp) else {
            continue;
        };
        if typ != "msg" && typ != "pkmsg" {
            continue;
        }
        for addr in &addrs {
            if let Ok(plain) = decrypt_message(dev, &typ, bytes, v, addr) {
                let device_of = addr
                    .rsplit_once('.')
                    .map(|(_, d)| d.parse::<u16>().unwrap_or(0))
                    .unwrap_or(0);
                return Ok((plain, device_of));
            }
        }
    }
    Err("no decryptable enc child".into())
}

fn decrypt_message(
    dev: &mut BotDevice,
    typ: &str,
    bytes: &[u8],
    v: u8,
    addr: &str,
) -> Result<Vec<u8>, String> {
    let clean = if typ == "pkmsg" {
        let pk =
            wa_signal_session::PreKeyMsg::from_bytes(bytes).map_err(|e| format!("pkmsg: {e}"))?;
        CiphertextMsg::PreKey(pk)
    } else {
        let sig =
            wa_signal_session::SignalMsg::from_bytes(bytes).map_err(|e| format!("signal: {e}"))?;
        CiphertextMsg::Signal(sig)
    };

    let mut record = match dev.get_session(addr) {
        Some(b) => SessionRecord::from_bytes(b).map_err(|e| format!("session load: {e}"))?,
        None => SessionRecord::new(None),
    };
    let otp: Vec<(u32, [u8; 32], [u8; 33])> = dev
        .one_time_prekeys
        .iter()
        .map(|k| (k.id, k.priv_key, signal_pub(&k.pub_key)))
        .collect();
    let keys = LocalKeys {
        identity_priv: dev.identity_priv,
        identity_pub: signal_pub(&dev.identity_pub),
        signed_prekey: Some((
            dev.prekey_id,
            dev.signed_prekey_priv,
            signal_pub(&dev.signed_prekey_pub),
        )),
        one_time_prekeys: &otp,
        local_reg_id: dev.registration_id,
    };
    let (plaintext, consumed) =
        wa_signal_session::decrypt_message_consumed(&mut rand::rng(), &mut record, &clean, &keys)
            .map_err(|e| format!("decrypt: {e}"))?;
    if let Some(id) = consumed {
        dev.take_prekey(id);
    }
    let padded = unpad_message(&plaintext, v).map_err(|e| format!("unpad: {e}"))?;
    let encoded = record.to_bytes();
    dev.put_session(addr, encoded);
    Ok(padded)
}

// ── inbound message introspection ────────────────────────────────────────

/// What a polled event knows about the underlying protobuf message.
struct MsgInfo {
    kind: String,
    text: String,
    caption: String,
    media: Option<MediaSpec>,
    view_once: bool,
    protocol: Option<(String, String, bool)>,
}

fn base64_std(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Pull the common downloadable-attachment fields out of a media sub-message.
fn parse_media_fields(fields: &[pb::Field], media_type: &str, mime_field: u64) -> Option<MediaSpec> {
    Some(MediaSpec {
        direct_path: String::from_utf8_lossy(pb::find_bytes(fields, 11).or_else(|| pb::find_bytes(fields, 10))?).to_string(),
        media_key: pb::find_bytes(fields, 8).or_else(|| pb::find_bytes(fields, 7)).map(|b| b.to_vec()),
        enc_sha: pb::find_bytes(fields, 9).or_else(|| pb::find_bytes(fields, 3)).map(|b| b.to_vec()),
        sha: pb::find_bytes(fields, 4).or_else(|| pb::find_bytes(fields, 2)).map(|b| b.to_vec()),
        media_type: media_type.to_string(),
        mime: String::from_utf8_lossy(pb::find_bytes(fields, mime_field).unwrap_or_default()).to_string(),
    })
}

/// Turn the unpadded `waE2E.Message` protobuf into a compact summary.
fn describe_message(proto: &[u8]) -> MsgInfo {
    let mut info = MsgInfo {
        kind: "unknown".into(),
        text: String::new(),
        caption: String::new(),
        media: None,
        view_once: false,
        protocol: None,
    };
    let Ok(fields) = pb::parse(proto) else { return info };
    if let Some(t) = pb::find_bytes(&fields, 1) {
        info.kind = "text".into();
        info.text = String::from_utf8_lossy(t).into_owned();
        return info;
    }
    if let Some(m) = pb::find_bytes(&fields, 6) {
        info.kind = "extendedText".into();
        if let Ok(mf) = pb::parse(m) {
            if let Some(t) = pb::find_bytes(&mf, 1) {
                info.text = String::from_utf8_lossy(t).into_owned();
            }
        }
        return info;
    }
    if let Some(m) = pb::find_bytes(&fields, 3) {
        info.kind = "image".into();
        if let Ok(mf) = pb::parse(m) {
            info.caption = String::from_utf8_lossy(pb::find_bytes(&mf, 3).unwrap_or_default()).into_owned();
            info.media = parse_media_fields(&mf, "WhatsApp Image Keys", 2);
        }
        return info;
    }
    if let Some(m) = pb::find_bytes(&fields, 7) {
        info.kind = "document".into();
        if let Ok(mf) = pb::parse(m) {
            info.caption = String::from_utf8_lossy(pb::find_bytes(&mf, 20).unwrap_or_default()).into_owned();
            info.media =
                Some(MediaSpec {
                    direct_path: String::from_utf8_lossy(pb::find_bytes(&mf, 10).unwrap_or_default()).to_string(),
                    media_key: Some(pb::find_bytes(&mf, 7).unwrap_or_default().to_vec()),
                    enc_sha: Some(pb::find_bytes(&mf, 9).unwrap_or_default().to_vec()),
                    sha: Some(pb::find_bytes(&mf, 4).unwrap_or_default().to_vec()),
                    media_type: "WhatsApp Document Keys".into(),
                    mime: String::from_utf8_lossy(pb::find_bytes(&mf, 2).unwrap_or_default()).to_string(),
                });
        }
        return info;
    }
    if let Some(m) = pb::find_bytes(&fields, 8) {
        info.kind = "audio".into();
        if let Ok(mf) = pb::parse(m) {
            info.media =
                Some(MediaSpec {
                    direct_path: String::from_utf8_lossy(pb::find_bytes(&mf, 9).unwrap_or_default()).to_string(),
                    media_key: Some(pb::find_bytes(&mf, 7).unwrap_or_default().to_vec()),
                    enc_sha: Some(pb::find_bytes(&mf, 8).unwrap_or_default().to_vec()),
                    sha: Some(pb::find_bytes(&mf, 3).unwrap_or_default().to_vec()),
                    media_type: "WhatsApp Audio Keys".into(),
                    mime: String::from_utf8_lossy(pb::find_bytes(&mf, 2).unwrap_or_default()).to_string(),
                });
        }
        return info;
    }
    if let Some(m) = pb::find_bytes(&fields, 9) {
        info.kind = "video".into();
        if let Ok(mf) = pb::parse(m) {
            info.caption = String::from_utf8_lossy(pb::find_bytes(&mf, 7).unwrap_or_default()).into_owned();
            info.media =
                Some(MediaSpec {
                    direct_path: String::from_utf8_lossy(pb::find_bytes(&mf, 13).unwrap_or_default()).to_string(),
                    media_key: Some(pb::find_bytes(&mf, 6).unwrap_or_default().to_vec()),
                    enc_sha: Some(pb::find_bytes(&mf, 11).unwrap_or_default().to_vec()),
                    sha: Some(pb::find_bytes(&mf, 3).unwrap_or_default().to_vec()),
                    media_type: "WhatsApp Video Keys".into(),
                    mime: String::from_utf8_lossy(pb::find_bytes(&mf, 2).unwrap_or_default()).to_string(),
                });
        }
        return info;
    }
    if let Some(m) = pb::find_bytes(&fields, 26) {
        info.kind = "sticker".into();
        if let Ok(mf) = pb::parse(m) {
            info.media =
                Some(MediaSpec {
                    direct_path: String::from_utf8_lossy(pb::find_bytes(&mf, 8).unwrap_or_default()).to_string(),
                    media_key: Some(pb::find_bytes(&mf, 4).unwrap_or_default().to_vec()),
                    enc_sha: Some(pb::find_bytes(&mf, 3).unwrap_or_default().to_vec()),
                    sha: Some(pb::find_bytes(&mf, 2).unwrap_or_default().to_vec()),
                    media_type: "WhatsApp Image Keys".into(),
                    mime: String::from_utf8_lossy(pb::find_bytes(&mf, 5).unwrap_or_default()).to_string(),
                });
        }
        return info;
    }
    if let Some(m) = pb::find_bytes(&fields, 12) {
        info.kind = "protocol".into();
        if let Ok(pf) = pb::parse(m) {
            let typ = match pb::find_var(&pf, 2) {
                Some(0) => "revoke",
                Some(14) => "edit",
                Some(3) => "ephemeralSetting",
                Some(5) => "historySync",
                _ => "other",
            };
            let (mut target, mut from_me) = (String::new(), false);
            if let Some(k) = pb::find_bytes(&pf, 1) {
                if let Ok(kf) = pb::parse(k) {
                    if let Some(t) = pb::find_bytes(&kf, 3) {
                        target = String::from_utf8_lossy(t).into_owned();
                    }
                    from_me = pb::find_var(&kf, 2).unwrap_or(0) == 1;
                }
            }
            info.protocol = Some((typ.to_string(), target, from_me));
        }
        return info;
    }
    // viewOnceMessage(37) / ephemeralMessage(40): the real message is nested.
    for shell in [37u64, 40u64] {
        if let Some(w) = pb::find_bytes(&fields, shell) {
            if let Ok(wf) = pb::parse(w) {
                let mut inner = describe_message(wf.iter().find(|f| f.wire == 2 && f.num != 0).map(|f| f.data).unwrap_or_default());
                inner.view_once = shell == 37;
                return inner;
            }
        }
    }
    info
}

/// Send an iq and block until the matching result (answering server pings
/// on the way). The Response to encrypted messages is handled by the caller.
fn wait_iq(s: &mut Wa2Session, req_id: &str) -> Result<WaNode, String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if deadline <= Instant::now() {
            return Err("iq timed out".into());
        }
        match s.recv_node_timeout(1000).map_err(|e| format!("iq recv: {e}"))? {
            None => continue,
            Some(node) => {
                if node.tag == "stream:error" {
                    return Err("stream error while waiting for iq".into());
                }
                if node.tag == "iq" && node.attr_str("id") == Some(req_id) {
                    return Ok(node);
                }
                if node.tag == "iq" {
                    handle_iq(s, &node).map_err(|e| format!("iq ping: {e}"))?;
                }
            }
        }
    }
}

fn get_user_devices(s: &mut Wa2Session, user: &str) -> Result<Vec<u16>, String> {
    let req = build_usync_devices_iq(&new_id(), &new_id(), user);
    let req_id = req.attr_str("id").unwrap_or_default().to_string();
    s.send_node(&req).map_err(|e| format!("usync send: {e}"))?;
    let resp = wait_iq(s, &req_id)?;
    let devices = crate::wa_bot::parse_usync_device_list(&resp);
    if devices.is_empty() {
        return Err(format!("no devices for {user}"));
    }
    let mut seen = std::collections::HashSet::new();
    Ok(devices
        .into_iter()
        .filter(|(u, _)| u == user)
        .map(|(_, d)| d)
        .filter(|d| seen.insert(*d))
        .collect())
}

/// Fetch and process a pre-key bundle for a remote device, establishing an
/// outbound Alice session (stored in `dev`).
fn establish_session(s: &mut Wa2Session, dev: &mut BotDevice, user: &str, device: u16) -> Result<(), String> {
    let addr = BotDevice::signal_address(user, device);
    if dev.get_session(&addr).is_some() {
        return Ok(());
    }
    let device_jid = Jid::advanced(user, "s.whatsapp.net", 0, device);
    let req = crate::wa_bot::build_fetch_prekeys_iq(&new_id(), &device_jid);
    let req_id = req.attr_str("id").unwrap_or_default().to_string();
    s.send_node(&req).map_err(|e| format!("prekey fetch: {e}"))?;
    let resp = wait_iq(s, &req_id)?;
    let user_node = resp
        .child("list")
        .and_then(|l| l.child("user"))
        .ok_or_else(|| "prekey response missing user".to_string())?;
    let (_user, _device, bundle) = crate::wa_bot::parse_prekey_bundle_user(user_node)
        .map_err(|e| format!("prekey parse: {e}"))?;

    let prekey_ref = bundle.pre_key.as_ref().map(|(id, p)| (*id, p));
    let init = wa_signal_session::AliceBundle {
        their_identity_pub: &bundle.identity_pub,
        their_signed_prekey: (&bundle.signed_pre_key.1, &bundle.signed_pre_key.2),
        their_signed_prekey_id: bundle.signed_pre_key.0,
        their_one_time_prekey: prekey_ref,
        their_registration_id: bundle.registration_id,
        our_registration_id: dev.registration_id,
    };
    let (record, _otpk) = wa_signal_session::initialize_from_bundle(
        &mut rand::rng(),
        &init,
        (dev.identity_priv, signal_pub(&dev.identity_pub)),
    )
    .map_err(|e| format!("session init: {e}"))?;
    let encoded = record.to_bytes();
    dev.put_session(&addr, encoded);
    Ok(())
}

/// Build the sender `<message>` stanza and send it.
fn send_plaintext(
    s: &mut Wa2Session,
    dev: &mut BotDevice,
    to: &str,
    plaintext: &[u8],
) -> Result<(), String> {
    let (user, server) = parse_to_jid(to)?;
    if server != "s.whatsapp.net" {
        return Err(format!("can only message users on s.whatsapp.net, not {server}"));
    }
    let padded = crate::wa_bot::pad_message(plaintext);

    let devices = get_user_devices(s, &user)?;

    let mut participants = Vec::new();
    let mut any_pkmsg = false;
    for d in &devices {
        if user == dev.user && *d == dev.device {
            continue; // ourselves
        }
        establish_session(s, dev, &user, *d)?;
        let addr = BotDevice::signal_address(&user, *d);
        let record_bytes = dev.get_session(&addr).ok_or_else(|| "session lost".to_string())?;
        let mut record = SessionRecord::from_bytes(record_bytes)
            .map_err(|e| format!("session load: {e}"))?;
        let ct = wa_signal_session::encrypt_message(&mut rand::rng(), &mut record, &padded)
            .map_err(|e| format!("encrypt: {e}"))?;
        if ct.msg_type() == wa_signal_session::CiphertextType::PreKey {
            any_pkmsg = true;
        }
        let encoded = record.to_bytes();
        dev.put_session(&addr, encoded);

        let enc = crate::wa_bot::make_enc_node(&ct);
        let mut to = WaNode::new("to");
        to.attrs.push(("jid".into(), WaVal::Jid(Jid::advanced(&user, "s.whatsapp.net", 0, *d))));
        to.content = WaVal::Nodes(vec![enc]);
        participants.push(to);
    }
    if participants.is_empty() {
        return Err(format!("no reachable devices for {user}"));
    }

    let mut msg = WaNode::new("message");
    msg.attrs.push(("id".into(), WaVal::Str(new_id())));
    msg.attrs.push(("type".into(), WaVal::Str("text".into())));
    msg.attrs.push(("to".into(), WaVal::Jid(Jid::new(&user, "s.whatsapp.net"))));
    let mut content = vec![WaNode {
        tag: "participants".into(),
        attrs: vec![],
        content: WaVal::Nodes(participants),
    }];
    if any_pkmsg && !dev.account.is_empty() {
        content.push(WaNode {
            tag: "device-identity".into(),
            attrs: vec![],
            content: WaVal::Bytes(dev.account.clone()),
        });
    }
    msg.content = WaVal::Nodes(content);
    s.send_node(&msg).map_err(|e| format!("send message: {e}"))?;
    Ok(())
}

/// Cached media connection info (auth + hosts) for upload/download.
#[derive(Clone)]
struct MediaConn {
    auth: String,
    hosts: Vec<String>,
    fetched: Instant,
}

fn load_media_conn(s: &mut Wa2Session, cache: &mut Option<MediaConn>) -> Result<MediaConn, String> {
    if let Some(mc) = cache {
        if mc.fetched.elapsed() < Duration::from_secs(240) {
            return Ok(mc.clone());
        }
    }
    let mut iq = WaNode::new("iq");
    iq.attrs.push(("id".into(), WaVal::Str(new_id())));
    iq.attrs.push(("type".into(), WaVal::Str("set".into())));
    iq.attrs.push(("to".into(), WaVal::Jid(Jid::new("s.whatsapp.net", "s.whatsapp.net"))));
    iq.attrs.push(("xmlns".into(), WaVal::Str("w:m".into())));
    iq.content = WaVal::Nodes(vec![WaNode::new("media_conn")]);
    let req_id = iq.attr_str("id").unwrap_or_default().to_string();
    s.send_node(&iq).map_err(|e| format!("media_conn send: {e}"))?;
    let resp = wait_iq(s, &req_id)?;
    let mc = resp
        .child("media_conn")
        .ok_or_else(|| "media_conn response missing media_conn".to_string())?;
    let auth = mc.attr_str("auth").unwrap_or_default().to_string();
    let hosts: Vec<String> = mc
        .children()
        .iter()
        .filter(|c| c.tag == "host")
        .filter_map(|c| c.attr_str("hostname").map(str::to_string))
        .collect();
    if hosts.is_empty() {
        return Err("media_conn response has no hosts".into());
    }
    let mc = MediaConn { auth, hosts, fetched: Instant::now() };
    *cache = Some(mc.clone());
    Ok(mc)
}

/// (message proto field number, media-type key, is-sticker)
fn media_kind_params(kind: &str) -> Result<(u64, &'static str), String> {
    match kind {
        "image" => Ok((3, crate::wa_media::MT_IMAGE)),
        "video" => Ok((9, crate::wa_media::MT_VIDEO)),
        "audio" => Ok((8, crate::wa_media::MT_AUDIO)),
        "document" => Ok((7, crate::wa_media::MT_DOCUMENT)),
        "sticker" => Ok((26, crate::wa_media::MT_IMAGE)),
        _ => Err(format!("wa.sendFile: unknown kind {kind:?} (image|video|audio|document|sticker)")),
    }
}

/// Compose the inner media protobuf for the given kind (fields in ascending
/// order, matching the `waE2E` message layout).
fn build_media_proto(
    kind: &str,
    up: &crate::wa_media::UploadResponse,
    mkey: &[u8; 32],
    enc: &crate::wa_media::EncryptedMedia,
    mime: &str,
    filename: &str,
    caption: &str,
) -> Result<Vec<u8>, String> {
    let ts = chrono::Utc::now().timestamp_millis();
    let mut o = Vec::new();
    macro_rules! s {
        ($num:expr, $val:expr) => {
            pb::field_string($num, $val, &mut o)
        };
    }
    macro_rules! bytes {
        ($num:expr, $val:expr) => {
            pb::field_bytes($num, $val, &mut o)
        };
    }
    match kind {
        "image" => {
            s!(1, &up.url); // URL
            s!(2, mime); // mimetype
            s!(3, caption); // caption
            bytes!(4, &enc.file_sha256); // fileSHA256
            pb::field_uint(5, enc.file_length, &mut o);
            bytes!(8, mkey); // mediaKey
            bytes!(9, &enc.file_enc_sha256); // fileEncSHA256
            s!(11, &up.direct_path); // directPath
            pb::field_uint(12, ts as u64, &mut o); // mediaKeyTimestamp
        }
        "document" => {
            s!(1, &up.url);
            s!(2, mime);
            s!(3, filename); // title
            bytes!(4, &enc.file_sha256);
            pb::field_uint(5, enc.file_length, &mut o);
            bytes!(7, mkey); // mediaKey
            s!(8, filename); // fileName
            bytes!(9, &enc.file_enc_sha256);
            s!(10, &up.direct_path);
            pb::field_uint(11, ts as u64, &mut o);
            s!(20, caption);
        }
        "audio" => {
            s!(1, &up.url);
            s!(2, mime);
            bytes!(3, &enc.file_sha256);
            pb::field_uint(4, enc.file_length, &mut o);
            bytes!(7, mkey); // mediaKey
            bytes!(8, &enc.file_enc_sha256);
            s!(9, &up.direct_path);
            pb::field_uint(10, ts as u64, &mut o);
        }
        "video" => {
            s!(1, &up.url);
            s!(2, mime);
            bytes!(3, &enc.file_sha256);
            pb::field_uint(4, enc.file_length, &mut o);
            bytes!(6, mkey); // mediaKey
            s!(7, caption);
            bytes!(11, &enc.file_enc_sha256);
            s!(13, &up.direct_path);
            pb::field_uint(14, ts as u64, &mut o);
        }
        "sticker" => {
            s!(1, &up.url);
            bytes!(2, &enc.file_sha256);
            bytes!(3, &enc.file_enc_sha256);
            bytes!(4, mkey); // mediaKey
            s!(5, mime);
            s!(8, &up.direct_path);
            pb::field_uint(9, enc.file_length, &mut o);
            pb::field_uint(10, ts as u64, &mut o);
        }
        _ => return Err(format!("wa.sendFile: unsupported kind {kind:?}")),
    }
    Ok(o)
}

/// Encrypt, upload, build the media protobuf and deliver an attachment.
fn send_media_message(
    s: &mut Wa2Session,
    dev: &mut BotDevice,
    mc: &mut Option<MediaConn>,
    job: &SendJob,
) -> Result<(), String> {
    let payload = job.file.as_ref().ok_or_else(|| "send_media_message: no file".to_string())?;
    let (shell_field, media_type) = media_kind_params(&payload.kind)?;
    let media_key: [u8; 32] = rand::rng().random();
    let enc = crate::wa_media::encrypt_media(&payload.bytes, &media_key, media_type.as_bytes());

    let conn = load_media_conn(s, mc)?;
    let token = crate::wa_media::b64url(&enc.file_enc_sha256);
    let mms = crate::wa_media::mms_type(media_type);
    let up = match crate::wa_media::upload(
        &conn.hosts[0],
        &conn.auth,
        mms,
        &token,
        &enc.data,
    ) {
        Ok(u) => u,
        Err(_) => {
            // Host/auth may have rotated — refresh mediaConn once and retry.
            *mc = None;
            let conn = load_media_conn(s, mc)?;
            crate::wa_media::upload(&conn.hosts[0], &conn.auth, mms, &token, &enc.data)?
        }
    };
    if up.url.is_empty() || up.direct_path.is_empty() {
        return Err("wa.sendFile: upload returned no url/direct_path".into());
    }

    let inner = build_media_proto(
        &payload.kind,
        &up,
        &media_key,
        &enc,
        &payload.mime,
        &payload.filename,
        &payload.caption,
    )?;
    let mut plaintext = Vec::new();
    pb::field_msg(shell_field, &inner, &mut plaintext);
    send_plaintext(s, dev, &job.to, &plaintext)
}

/// Download + decrypt the attachment described by `spec`.
fn download_media(s: &mut Wa2Session, mc: &mut Option<MediaConn>, spec: &MediaSpec) -> Result<Vec<u8>, String> {
    if !spec.direct_path.starts_with('/') {
        return Err("wa.download: directPath must start with '/'".into());
    }
    let mms = crate::wa_media::mms_type(&spec.media_type);
    let enc_b64 = spec
        .enc_sha
        .as_deref()
        .map(crate::wa_media::b64url)
        .unwrap_or_default();
    let hosts = if let Ok(conn) = load_media_conn(s, mc) {
        conn.hosts.clone()
    } else {
        vec![
            "mmg.whatsapp.net".to_string(),
            "mmg-alb.whatsapp.net".to_string(),
        ]
    };
    let mut last_err = None;
    for host in &hosts {
        let url = format!(
            "https://{host}{}&hash={enc_b64}&mms-type={mms}&__wa-mms=",
            spec.direct_path
        );
        let file = match crate::wa_media::download(&url) {
            Ok(f) => f,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };
        let result = match spec.media_key.as_deref() {
            Some(mk) if mk.len() == 32 => {
                let mut key = [0u8; 32];
                key.copy_from_slice(mk);
                crate::wa_media::decrypt_media(
                    &file,
                    &key,
                    spec.media_type.as_bytes(),
                    spec.enc_sha.as_deref(),
                    spec.sha.as_deref(),
                )
            }
            _ => Ok(file),
        };
        return result;
    }
    Err(last_err.unwrap_or_else(|| "no media hosts".into()))
}

fn drain_jobs(s: &mut Wa2Session, sess: &Arc<NativeSession>, dev: &mut BotDevice) -> Result<(), String> {
    let mut mc: Option<MediaConn> = None;
    loop {
        let job = sess.send_pending.lock().unwrap().pop_front();
        let Some(job) = job else { break };
        let res = match &job.file {
            None => {
                let mut plaintext = Vec::new();
                pb::field_string(1, &job.text, &mut plaintext);
                send_plaintext(s, dev, &job.to, &plaintext)
            }
            Some(_) => send_media_message(s, dev, &mut mc, &job),
        };
        let _ = job.ack.send(res);
    }
    loop {
        let job = sess.download_pending.lock().unwrap().pop_front();
        let Some(job) = job else { break };
        let res = download_media(s, &mut mc, &job.spec);
        let _ = job.ack.send(res);
    }
    Ok(())
}

/// Turn a raw server node into the dict a zen `wa.poll()` returns, decrypting
/// inbound Signal messages into the classic `text` field.
fn push_event(sess: &Arc<NativeSession>, dev: &mut BotDevice, node: &WaNode) {
    let from = node
        .attrs
        .iter()
        .find(|(k, _)| k == "from")
        .map(|(_, v)| match v {
            WaVal::Jid(j) => format_jid_simple(j),
            WaVal::Str(s) => s.clone(),
            _ => String::new(),
        })
        .unwrap_or_default();
    let id = node.attr_str("id").unwrap_or_default().to_string();
    let typ = node.attr_str("type").unwrap_or_default().to_string();
    let participant = node.attr_str("participant").unwrap_or_default().to_string();

    let mut m = indexmap::IndexMap::new();
    m.insert("tag".into(), Value::String(node.tag.clone()));
    m.insert("id".into(), Value::String(id));
    let from_clone = from.clone();
    m.insert("from".into(), Value::String(from));
    m.insert("type".into(), Value::String(typ));
    m.insert("participant".into(), Value::String(participant));
    m.insert("is_group".into(), Value::Bool(from_clone.contains("@g.us")));
    m.insert("timestamp".into(), Value::Number(0.0));

    if node.tag == "message" {
        let mut kind = String::from("unknown");
        let mut text = String::new();
        let mut caption = String::new();
        let mut media: Option<MediaSpec> = None;
        let mut protocol: Option<(String, String, bool)> = None;
        let mut view_once = false;
        let mut protobuf: Option<Vec<u8>> = None;
        if let Ok((proto, dev_)) = decrypt_inbound(dev, node) {
            protobuf = Some(proto.clone());
            let info = describe_message(&proto);
            kind = info.kind;
            text = info.text;
            caption = info.caption;
            media = info.media;
            protocol = info.protocol;
            view_once = info.view_once;
            // my own send echo lands here through the session cursor
            let _ = dev_;
        }
        for k in ["from_alt", "sender", "sender_alt", "push_name", "device", "text", "kind", "caption"] {
            m.entry(k.into()).or_insert_with(|| Value::String(String::new()));
        }
        m.insert("text".into(), Value::String(text));
        m.insert("kind".into(), Value::String(kind));
        m.insert("caption".into(), Value::String(caption));
        m.insert("view_once".into(), Value::Bool(view_once));
        if let Some(p) = &protobuf {
            m.insert("protobuf".into(), Value::String(base64_std(p)));
        }
        if let Some((ptype, target, pme)) = protocol {
            let mut p = indexmap::IndexMap::new();
            p.insert("type".into(), Value::String(ptype));
            p.insert("targetId".into(), Value::String(target));
            p.insert("fromMe".into(), Value::Bool(pme));
            m.insert("protocol".into(), Value::Dict(Arc::new(p)));
        }
        if let Some(spec) = media {
            let mut md = indexmap::IndexMap::new();
            md.insert("directPath".into(), Value::String(spec.direct_path.clone()));
            md.insert("mediaKey".into(), Value::String(base64_std(&spec.media_key.clone().unwrap_or_default())));
            md.insert("fileEncSHA256".into(), Value::String(base64_std(&spec.enc_sha.clone().unwrap_or_default())));
            md.insert("fileSHA256".into(), Value::String(base64_std(&spec.sha.clone().unwrap_or_default())));
            md.insert("mime".into(), Value::String(spec.mime.clone()));
            md.insert("mediaType".into(), Value::String(spec.media_type.clone()));
            m.insert("media".into(), Value::Dict(Arc::new(md)));
            // Convenience top-level b64 fields for wa.download(msg).
            m.insert("directPath".into(), Value::String(spec.direct_path.clone()));
            m.insert("mediaKey".into(), Value::String(base64_std(&spec.media_key.clone().unwrap_or_default())));
            m.insert("fileEncSHA256".into(), Value::String(base64_std(&spec.enc_sha.clone().unwrap_or_default())));
            m.insert("fileSHA256".into(), Value::String(base64_std(&spec.sha.clone().unwrap_or_default())));
        }
        // from_me: the from-jid is (or belongs to) our own registered user.
        let self_from = {
            let me = &dev.user;
            if me.is_empty() {
                false
            } else {
                from_clone == format!("{}@s.whatsapp.net", me)
                    || from_clone == format!("{}:{}@s.whatsapp.net", me, dev.device)
            }
        };
        m.insert("from_me".into(), Value::Bool(self_from));
    }

    sess.messages.lock().unwrap().push_back(Value::Dict(Arc::new(m)));
}

fn format_jid_simple(j: &Jid) -> String {
    if j.device != 0 {
        format!("{}:{}@{}", j.user, j.device, j.server)
    } else {
        format!("{}@{}", j.user, j.server)
    }
}

// ── native entry points ─────────────────────────────────────────────────

/// `wa.connect(auth_dir, phone)` — open (and if needed pair) a native
/// WhatsApp session. `auth_dir` also stores the device key file.
pub fn wa_connect(args: &Vec<Value>) -> Result<Value, String> {
    let auth_dir = match args.first() {
        Some(Value::String(s)) if !s.trim().is_empty() => expand_home(s),
        _ => dirs_home().join(".zen").join("wa_native"),
    };
    let phone = match args.get(1) {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    };

    if let Some(old) = take_session() {
        old.request_stop();
    }

    let sess = NativeSession::opened();
    let worker_sess = Arc::clone(&sess);
    let handle = std::thread::Builder::new()
        .name("wa-native".into())
        .spawn(move || {
            // A phone number starts the pairing code flow in Baileys; the
            // native client only supports QR pairing right now, so we ignore
            // the hint (kept for API compatibility).
            if !phone.is_empty() {
                eprintln!("[zen-wa] note: phone-number pairing not implemented; using QR");
            }
            run_worker(worker_sess, auth_dir);
        })
        .map_err(|e| format!("wa: cannot spawn worker: {e}"))?;
    *sess.worker.lock().unwrap() = Some(handle);

    NATIVE_SESSION.with(|s| *s.borrow_mut() = Some(Arc::clone(&sess)));
    Ok(Value::Bool(true))
}

pub fn wa_state(_args: &Vec<Value>) -> Result<Value, String> {
    Ok(match current_session() {
        Some(s) => Value::String(s.state.lock().unwrap().clone()),
        None => Value::String("idle".into()),
    })
}

pub fn wa_qr(_args: &Vec<Value>) -> Result<Value, String> {
    Ok(match current_session().and_then(|s| s.qr.lock().unwrap().clone()) {
        Some(q) => Value::String(q),
        None => Value::Null,
    })
}

pub fn wa_pairing_code(_args: &Vec<Value>) -> Result<Value, String> {
    Ok(Value::Null)
}

pub fn wa_last_error(_args: &Vec<Value>) -> Result<Value, String> {
    Ok(match current_session().and_then(|s| s.last_error.lock().unwrap().clone()) {
        Some(e) => Value::String(e),
        None => Value::Null,
    })
}

/// `wa.poll(timeout_ms)` — drain pending events, waiting up to `timeout_ms`.
pub fn wa_poll(args: &Vec<Value>) -> Result<Value, String> {
    let timeout_ms = match args.first() {
        Some(Value::Number(n)) => (*n).max(0.0) as u64,
        _ => 0,
    };
    let sess = current_session()
        .ok_or_else(|| String::from("wa: not connected (call wa.connect() first)"))?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        {
            let mut q = sess.messages.lock().unwrap();
            if !q.is_empty() {
                return Ok(Value::List(Arc::new(q.drain(..).collect::<Vec<Value>>())));
            }
        }
        if Instant::now() >= deadline {
            return Ok(Value::List(Arc::new(Vec::new())));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// `wa.sendText(jid, text)` — queue the message on the worker thread, which
/// owns the socket and the Signal session store, then wait for its outcome.
pub fn wa_send_text(args: &Vec<Value>) -> Result<Value, String> {
    let to = match args.first() {
        Some(Value::String(s)) if !s.trim().is_empty() => s.clone(),
        _ => return Err("wa.sendText: missing recipient jid".into()),
    };
    let text = match args.get(1) {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("wa.sendText: missing message text".into()),
    };
    let sess = current_session()
        .ok_or_else(|| String::from("wa.sendText: not connected (call wa.connect() first)"))?;
    if sess.state.lock().unwrap().as_str() != "open" {
        return Err("wa.sendText: session is not open".into());
    }

    let (tx, rx) = mpsc::channel();
    {
        let mut pending = sess.send_pending.lock().unwrap();
        pending.push_back(SendJob { to, text, file: None, ack: tx });
        sess.send_condvar.notify_one();
    }
    match rx.recv_timeout(Duration::from_secs(45)) {
        Ok(Ok(())) => Ok(Value::Bool(true)),
        Ok(Err(e)) => Err(format!("wa.sendText: {e}")),
        Err(e) => Err(format!("wa.sendText timed out: {e}")),
    }
}

/// Resolve a base64 string or `(path, mime)` dict into raw bytes.
fn payload_bytes(arg: &Value, opts: &indexmap::IndexMap<String, Value>) -> Result<Vec<u8>, String> {
    if let Some(Value::String(b64)) = opts.get("base64") {
        if !b64.trim().is_empty() {
            return base64::engine::general_purpose::STANDARD
                .decode(b64.trim())
                .map_err(|e| format!("wa.sendFile: invalid base64: {e}"));
        }
    }
    if let Some(Value::String(p)) = opts.get("path") {
        return std::fs::read(p).map_err(|e| format!("wa.sendFile: cannot read {p:?}: {e}"));
    }
    match arg {
        Value::String(s) => {
            // If it is a valid base64 blob treat it as bytes, else as a path.
            if s.starts_with('/') || s.ends_with(".png") || s.ends_with(".jpg")
                || s.ends_with(".jpeg") || s.ends_with(".gif") || s.ends_with(".webp")
                || s.ends_with(".mp4") || s.ends_with(".mp3") || s.ends_with(".ogg")
                || s.ends_with(".opus") || s.ends_with(".pdf") || s.ends_with(".ogg")
            {
                std::fs::read(s).map_err(|e| format!("wa.sendFile: cannot read {s:?}: {e}"))
            } else {
                base64::engine::general_purpose::STANDARD
                    .decode(s)
                    .map_err(|e| format!("wa.sendFile: invalid base64: {e}"))
            }
        }
        _ => Err(
            "wa.sendFile: expected base64 bytes or a file path (or {path:...} / {base64:...})"
                .into(),
        ),
    }
}

/// `wa.sendFile(jid, bytesOrPath, {kind, mime, filename, caption, base64, path})`
///
/// Encrypts, uploads and pushes an attachment. `kind` is one of
/// `image|video|audio|document|sticker`.
pub fn wa_send_file(args: &Vec<Value>) -> Result<Value, String> {
    let to = match args.first() {
        Some(Value::String(s)) if !s.trim().is_empty() => s.clone(),
        _ => return Err("wa.sendFile: missing recipient jid".into()),
    };
    let opts = match args.get(2) {
        Some(Value::Dict(d)) => (**d).clone(),
        _ => indexmap::IndexMap::new(),
    };
    let kind = opts
        .get("kind")
        .and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_else(|| "document".to_string());
    media_kind_params(&kind)?;
    let bytes = args
        .get(1)
        .ok_or_else(|| "wa.sendFile: missing file payload".to_string())
        .and_then(|a| payload_bytes(a, &opts))?;
    if bytes.is_empty() {
        return Err("wa.sendFile: refusing to send an empty file".into());
    }
    let str_opt = |k: &str| {
        opts.get(k)
            .and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default()
    };
    let mime = str_opt("mime");
    let mime = if mime.is_empty() {
        match kind.as_str() {
            "image" => "image/png",
            "video" => "video/mp4",
            "audio" => "audio/ogg",
            "sticker" => "image/webp",
            _ => "application/octet-stream",
        }
        .to_string()
    } else {
        mime
    };
    let filename = str_opt("filename");
    let caption = str_opt("caption");

    let sess = current_session()
        .ok_or_else(|| String::from("wa.sendFile: not connected (call wa.connect() first)"))?;
    if sess.state.lock().unwrap().as_str() != "open" {
        return Err("wa.sendFile: session is not open".into());
    }

    let (tx, rx) = mpsc::channel();
    {
        let mut pending = sess.send_pending.lock().unwrap();
        pending.push_back(SendJob {
            to,
            text: String::new(),
            file: Some(FilePayload { bytes, kind, mime, filename, caption }),
            ack: tx,
        });
        sess.send_condvar.notify_one();
    }
    match rx.recv_timeout(Duration::from_secs(90)) {
        Ok(Ok(())) => Ok(Value::Bool(true)),
        Ok(Err(e)) => Err(format!("wa.sendFile: {e}")),
        Err(e) => Err(format!("wa.sendFile timed out: {e}")),
    }
}

fn event_str(d: &indexmap::IndexMap<String, Value>, k: &str) -> String {
    d.get(k)
        .and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// `wa.download(msgOrMedia, {toPath: "...", mediaType, mime})`
///
/// Fetches + decrypts the attachment of a polled message event (or of a media
/// dict with `directPath`/`mediaKey`/`fileEncSHA256`), returning the plaintext
/// bytes as base64 — or the written file path when `toPath` is given.
pub fn wa_download(args: &Vec<Value>) -> Result<Value, String> {
    let dict = match args.first() {
        Some(Value::Dict(d)) => (**d).clone(),
        _ => return Err("wa.download: expected a message/media dict from wa.poll()".into()),
    };
    let opts = match args.get(1) {
        Some(Value::Dict(d)) => (**d).clone(),
        _ => indexmap::IndexMap::new(),
    };
    let direct_path = event_str(&dict, "directPath");
    let media_key = event_str(&dict, "mediaKey");
    let enc_sha = event_str(&dict, "fileEncSHA256");
    let sha = event_str(&dict, "fileSHA256");
    if direct_path.is_empty() {
        return Err("wa.download: event has no media directPath".into());
    }
    // Prefer an explicit option, then the event, then the mime/mediaType keys.
    let media_type = event_str(&opts, "mediaType");
    let media_type = if media_type.is_empty() { event_str(&dict, "mediaType") } else { media_type };
    let media_type = if media_type.is_empty() { "WhatsApp Document Keys" } else { media_type.as_str() };
    let b64dec = |s: &str, what: &str| -> Result<Vec<u8>, String> {
        if s.is_empty() {
            Ok(Vec::new())
        } else {
            base64::engine::general_purpose::STANDARD
                .decode(s)
                .map_err(|e| format!("wa.download: bad {what} base64: {e}"))
        }
    };
    let spec = MediaSpec {
        direct_path,
        media_key: Some(if media_key.is_empty() { Vec::new() } else { b64dec(&media_key, "mediaKey")? }),
        enc_sha: if enc_sha.is_empty() { None } else { Some(b64dec(&enc_sha, "fileEncSHA256")?) },
        sha: if sha.is_empty() { None } else { Some(b64dec(&sha, "fileSHA256")?) },
media_type: media_type.to_string(),
        mime: event_str(&dict, "mime"),
    };

    let sess = current_session()
        .ok_or_else(|| String::from("wa.download: not connected (call wa.connect() first)"))?;
    if sess.state.lock().unwrap().as_str() != "open" {
        return Err("wa.download: session is not open".into());
    }
    let (tx, rx) = mpsc::channel();
    {
        let mut pending = sess.download_pending.lock().unwrap();
        pending.push_back(DownloadJob { spec, ack: tx });
        sess.send_condvar.notify_one();
    }
    let bytes = match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => return Err(format!("wa.download: {e}")),
        Err(e) => return Err(format!("wa.download timed out: {e}")),
    };
    if let Some(Value::String(p)) = opts.get("toPath") {
        std::fs::write(p, &bytes).map_err(|e| format!("wa.download: cannot write {p:?}: {e}"))?;
        return Ok(Value::String(p.clone()));
    }
    Ok(Value::String(base64_std(&bytes)))
}

pub fn wa_logout(_args: &Vec<Value>) -> Result<Value, String> {
    let sess = take_session().ok_or_else(|| String::from("wa: not connected"))?;
    sess.request_stop();
    set_state(&sess, "idle");
    Ok(Value::Bool(true))
}

pub fn wa_disconnect(_args: &Vec<Value>) -> Result<Value, String> {
    match take_session() {
        Some(sess) => {
            sess.request_stop();
            Ok(Value::Bool(true))
        }
        None => Ok(Value::Bool(false)),
    }
}

// ── module registration ─────────────────────────────────────────────────

pub fn init_wa_module(vm: &mut crate::runtime::Vm) {
    let wa = Value::Dict(Arc::new(indexmap::IndexMap::from([
        ("connect".into(), Value::NativeFunction("wa_connect".into())),
        ("state".into(), Value::NativeFunction("wa_state".into())),
        ("qr".into(), Value::NativeFunction("wa_qr".into())),
        (
            "pairingCode".into(),
            Value::NativeFunction("wa_pairing_code".into()),
        ),
        ("lastError".into(), Value::NativeFunction("wa_last_error".into())),
        ("poll".into(), Value::NativeFunction("wa_poll".into())),
        ("sendText".into(), Value::NativeFunction("wa_send_text".into())),
        ("send".into(), Value::NativeFunction("wa_send_text".into())),
        ("sendFile".into(), Value::NativeFunction("wa_send_file".into())),
        ("download".into(), Value::NativeFunction("wa_download".into())),
        ("logout".into(), Value::NativeFunction("wa_logout".into())),
        ("disconnect".into(), Value::NativeFunction("wa_disconnect".into())),
    ])));
    vm.vars.insert("wa".into(), wa);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_to_jid_accepts_bare_and_device_forms() {
        assert_eq!(parse_to_jid("6012345678@s.whatsapp.net").unwrap(), ("6012345678".into(), "s.whatsapp.net".into()));
        assert_eq!(parse_to_jid("6012345678:2@s.whatsapp.net").unwrap(), ("6012345678".into(), "s.whatsapp.net".into()));
        assert!(parse_to_jid("no-at-sign").is_err());
        assert!(parse_to_jid("@s.whatsapp.net").is_err());
    }

    #[test]
    fn signal_pub_has_djb_prefix() {
        let raw = [7u8; 32];
        let pubk = signal_pub(&raw);
        assert_eq!(pubk[0], 0x05);
        assert_eq!(&pubk[1..], &raw);
    }

    #[test]
    fn describe_message_reads_conversation_field() {
        let mut msg = Vec::new();
        pb::field_string(1, "hello zen", &mut msg);
        assert_eq!(describe_message(&msg).text, "hello zen");
        assert_eq!(describe_message(b"garbage").text, "");
        assert_eq!(describe_message(b"garbage").kind, "unknown");
    }

    #[test]
    fn from_jid_accepts_jid_val_and_string() {
        let mut n = WaNode::new("message");
        n.attrs.push(("from".into(), WaVal::Jid(Jid::advanced("6012345678", "s.whatsapp.net", 0, 3))));
        assert_eq!(from_jid_of(&n), Some(("6012345678".into(), 3)));
        n.attrs[0].1 = WaVal::Str("6012345678:3@s.whatsapp.net".into());
        assert_eq!(from_jid_of(&n), Some(("6012345678".into(), 3)));
        n.attrs[0].1 = WaVal::Str("6012345678@s.whatsapp.net".into());
        assert_eq!(from_jid_of(&n), Some(("6012345678".into(), 0)));
    }

    #[test]
    fn send_job_queue_round_trips() {
        let sess = NativeSession::opened();
        let (tx, rx) = mpsc::channel();
        sess.send_pending.lock().unwrap().push_back(SendJob {
            to: "6012345678@s.whatsapp.net".into(),
            text: "hi".into(),
            file: None,
            ack: tx,
        });
        let job = sess.send_pending.lock().unwrap().pop_front().unwrap();
        assert_eq!(job.to, "6012345678@s.whatsapp.net");
        assert_eq!(job.text, "hi");
        job.ack.send(Ok(())).unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_millis(100)), Ok(Ok(())));
    }

    #[test]
    fn describe_message_parses_text() {
        let mut proto = Vec::new();
        pb::field_string(1, "a quiet hello", &mut proto);
        let info = describe_message(&proto);
        assert_eq!(info.kind, "text");
        assert_eq!(info.text, "a quiet hello");
        assert!(info.media.is_none());
        assert!(info.protocol.is_none());
    }

    #[test]
    fn describe_message_parses_image_attachment() {
        // waE2E.Message{imageMessage: {url, mimetype, fileSHA256, mediaKey,
        // fileEncSHA256, directPath}}
        let mut img = Vec::new();
        pb::field_string(1, "https://mmg.example/a", &mut img);
        pb::field_string(2, "image/png", &mut img);
        pb::field_bytes(4, &[7u8; 32], &mut img);
        pb::field_bytes(8, &[9u8; 32], &mut img);
        pb::field_bytes(9, &[8u8; 32], &mut img);
        pb::field_string(11, "/mms/xyz?extra=1", &mut img);
        let mut proto = Vec::new();
        pb::field_msg(3, &img, &mut proto);
        let info = describe_message(&proto);
        assert_eq!(info.kind, "image");
        let spec = info.media.expect("media present");
        assert_eq!(spec.direct_path, "/mms/xyz?extra=1");
        assert_eq!(spec.mime, "image/png");
        assert_eq!(spec.media_key, Some(vec![9u8; 32]));
        assert_eq!(spec.enc_sha, Some(vec![8u8; 32]));
        assert_eq!(spec.sha, Some(vec![7u8; 32]));
        assert_eq!(spec.media_type, "WhatsApp Image Keys");
    }

    #[test]
    fn describe_message_parses_protocol_revoke() {
        // waE2E.Message{protocolMessage: {key: {id: "target-1", fromMe: true},
        // type: REVOKE(0)}}
        let mut key = Vec::new();
        pb::field_string(3, "target-1", &mut key);
        pb::field_bool(2, true, &mut key);
        let mut pm = Vec::new();
        pb::field_msg(1, &key, &mut pm);
        pb::field_uint(2, 0, &mut pm); // REVOKE
        let mut proto = Vec::new();
        pb::field_msg(12, &pm, &mut proto);
        let info = describe_message(&proto);
        assert_eq!(info.kind, "protocol");
        let (ptype, target, from_me) = info.protocol.expect("protocol");
        assert_eq!(ptype, "revoke");
        assert_eq!(target, "target-1");
        assert!(from_me);
    }

    #[test]
    fn payload_bytes_resolves_path_and_base64() {
        let dir = std::env::temp_dir().join("zen_wa_test");
        std::fs::create_dir_all(&dir).ok();
        let p = dir.join("payload.bin");
        std::fs::write(&p, b"zen-payload").unwrap();
        let opts_empty = indexmap::IndexMap::new();
        let bytes = payload_bytes(&Value::String(p.to_string_lossy().into_owned()), &opts_empty).unwrap();
        assert_eq!(bytes, b"zen-payload");
        let b64 = base64_std(b"zen-payload");
        let bytes = payload_bytes(&Value::String(b64.clone()), &opts_empty).unwrap();
        assert_eq!(bytes, b"zen-payload");
        let mut opts = indexmap::IndexMap::new();
        opts.insert("base64".into(), Value::String(b64));
        let bytes = payload_bytes(&Value::String(String::new()), &opts).unwrap();
        assert_eq!(bytes, b"zen-payload");
    }

    #[test]
    fn build_media_proto_round_trips_through_describe() {
        let up = crate::wa_media::UploadResponse {
            url: "https://upload.example/u".into(),
            direct_path: "/mms/up1".into(),
        };
        let key = [0x33u8; 32];
        let enc = crate::wa_media::encrypt_media(b"steamy png", &key, crate::wa_media::MT_IMAGE.as_bytes());
        let inner = build_media_proto("image", &up, &key, &enc, "image/png", "", "so nice").unwrap();
        let mut proto = Vec::new();
        pb::field_msg(3, &inner, &mut proto);
        let info = describe_message(&proto);
        assert_eq!(info.kind, "image");
        assert_eq!(info.caption, "so nice");
        let spec = info.media.unwrap();
        assert_eq!(spec.direct_path, "/mms/up1");
        assert_eq!(spec.media_key, Some(key.to_vec()));
        assert_eq!(spec.sha, Some(enc.file_sha256.to_vec()));
        assert_eq!(spec.enc_sha, Some(enc.file_enc_sha256.to_vec()));
    }
}