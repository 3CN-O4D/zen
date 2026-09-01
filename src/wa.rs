//! Zen `wa` module — WhatsApp via a Node/Baileys v7 bridge subprocess.
//!
//! The bridge (`wa_bridge.mjs`, embedded below) speaks newline-delimited JSON
//! over stdio: state transitions, incoming chat messages, and op results.
//! One bridge process per interpreter (thread-local session, like browser/CDP).

use crate::runtime::{Vm, Value};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::cell::RefCell;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const BRIDGE_JS: &str = include_str!("wa_bridge.mjs");
const BAILEYS_VERSION: &str = "7.0.0-rc14";

pub struct WaSession {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    /// Last known connection state reported by the bridge.
    state: Mutex<String>,
    qr: Mutex<Option<String>>,
    pairing_code: Mutex<Option<String>>,
    /// Incoming chat messages awaiting `wa.poll()`.
    messages: Mutex<VecDeque<Value>>,
    /// Reply channels for outstanding ops (send_text/logout/shutdown).
    pending: Mutex<HashMap<u64, mpsc::Sender<Value>>>,
    next_id: AtomicU64,
}

impl WaSession {
    /// Send an op to the bridge and await its {"type":"result"} reply.
    fn send_op(
        &self,
        op: &str,
        fields: &[(&str, Value)],
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let mut line = serde_json::json!({ "op": op, "id": id });
        if let Some(map) = line.as_object_mut() {
            for (k, v) in fields {
                map.insert(k.to_string(), zen_value_to_json(v));
            }
        }
        let mut payload = line.to_string();
        payload.push('\n');
        {
            let mut sin = self.stdin.lock().unwrap();
            sin.write_all(payload.as_bytes())
                .map_err(|e| format!("wa: bridge stdin write failed: {e}"))?;
            sin.flush().ok();
        }
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(v) => Ok(v),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                Err("wa: bridge did not answer op in time".into())
            }
        }
    }

    fn is_alive(&self) -> bool {
        matches!(self.child.lock().unwrap().try_wait(), Ok(None))
    }

    fn shutdown(&self) {
        if self.is_alive() {
            // Best effort; ignore errors, we're killing it anyway.
            let _ = self.send_op("shutdown", &[]);
            thread::sleep(Duration::from_millis(200));
        }
        let _ = self.child.lock().unwrap().kill();
        let _ = self.child.lock().unwrap().wait();
    }
}

fn zen_value_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::json!(b),
        Value::Number(n) => serde_json::json!(n),
        Value::String(s) => serde_json::json!(s),
        _ => serde_json::Value::Null,
    }
}

thread_local! {
    static WA_SESSION: std::cell::RefCell<Option<Arc<WaSession>>> = RefCell::new(None);
}

fn take_session() -> Option<Arc<WaSession>> {
    WA_SESSION.with(|s| s.borrow_mut().take())
}

// ── home/setup ──────────────────────────────────────────────────────────

fn wa_home() -> PathBuf {
    if let Ok(h) = std::env::var("ZEN_WA_HOME") {
        return PathBuf::from(h);
    }
    let mut p = dirs_home();
    p.push(".zen");
    p.push("wa_bridge");
    p
}

fn dirs_home() -> PathBuf {
    if let Ok(h) = std::env::var("HOME") {
        return PathBuf::from(h);
    }
    PathBuf::from(".")
}

/// Ensure ~/.zen/wa_bridge contains bridge.mjs (fresh copy when changed) and
/// the npm dependencies. Returns an error string when setup is impossible.
fn ensure_setup() -> Result<PathBuf, String> {
    let home = wa_home();
    std::fs::create_dir_all(&home).map_err(|e| format!("wa: cannot create {}: {e}", home.display()))?;
    let bridge_path = home.join("bridge.mjs");
    let needs_write = std::fs::read_to_string(&bridge_path)
        .map(|old| old != BRIDGE_JS)
        .unwrap_or(true);
    if needs_write {
        std::fs::write(&bridge_path, BRIDGE_JS)
            .map_err(|e| format!("wa: cannot write {}: {e}", bridge_path.display()))?;
    }
    if !home.join("node_modules/baileys").exists() {
        eprintln!(
            "\x1b[1;33m[zen-wa]\x1b[0m installing WhatsApp bridge dependencies (baileys {BAILEYS_VERSION})..."
        );
        let status = Command::new("npm")
            .args(["install", "--no-fund", "--no-audit", &format!("baileys@{BAILEYS_VERSION}"), "pino", "qrcode-terminal"])
            .current_dir(&home)
            .status()
            .map_err(|e| format!("wa: cannot run npm (is Node.js installed?): {e}"))?;
        if !status.success() {
            return Err("wa: npm install failed — see output above".into());
        }
    }
    Ok(home)
}

// ── connect / lifecycle ─────────────────────────────────────────────────

fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        let mut h = dirs_home();
        h.push(rest);
        return h;
    }
    PathBuf::from(p)
}

pub fn wa_connect(args: &Vec<Value>) -> Result<Value, String> {
    let auth_dir = match args.first() {
        Some(Value::String(s)) if !s.trim().is_empty() => s.clone(),
        _ => "~/.zen/wa_session".to_string(),
    };
    let phone = match args.get(1) {
        Some(Value::String(s)) if !s.trim().is_empty() => {
            let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() < 8 {
                return Err("wa.connect: phone number must include country code, digits only".into());
            }
            digits
        }
        _ => String::new(),
    };
    // Tear down any previous session first.
    if let Some(old) = take_session() {
        old.shutdown();
    }
    let home = ensure_setup()?;
    let auth_path = expand_home(&auth_dir);
    std::fs::create_dir_all(&auth_path).ok();

    let mut cmd = Command::new(std::env::var("ZEN_WA_NODE").unwrap_or_else(|_| "node".into()));
    cmd.arg(home.join("bridge.mjs"))
        .arg("--auth")
        .arg(&auth_path);
    if !phone.is_empty() {
        cmd.arg("--phone").arg(&phone);
    }
    cmd.current_dir(&home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("wa: cannot start node bridge: {e}"))?;
    let stdout = child.stdout.take().expect("bridge stdout");
    let stdin = child.stdin.take().expect("bridge stdin");

    let session = Arc::new(WaSession {
        child: Mutex::new(child),
        stdin: Mutex::new(stdin),
        state: Mutex::new("starting".into()),
        qr: Mutex::new(None),
        pairing_code: Mutex::new(None),
        messages: Mutex::new(VecDeque::new()),
        pending: Mutex::new(HashMap::new()),
        next_id: AtomicU64::new(1),
    });

    // Reader: one line = one JSON event.
    {
        let session = Arc::clone(&session);
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                let Ok(Value::Dict(ev)) = crate::runtime::json_decode(&line) else {
                    continue; // not JSON (shouldn't happen) — skip
                };
                let get = |k: &str| -> Value { ev.get(k).cloned().unwrap_or(Value::Null) };
                match get("type") {
                    Value::String(t) if t == "state" => {
                        if let Value::String(st) = get("state") {
                            *session.state.lock().unwrap() = st;
                        }
                        if let Value::String(q) = get("qr") {
                            *session.qr.lock().unwrap() = Some(q);
                        }
                        if let Value::String(c) = get("code") {
                            *session.pairing_code.lock().unwrap() = Some(c);
                        }
                    }
                    Value::String(t) if t == "message" => {
                        let mut m = indexmap::IndexMap::new();
                        for k in [
                            "id", "from", "from_alt", "sender", "sender_alt",
                            "push_name", "text",
                        ] {
                            m.insert(k.into(), get(k));
                        }
                        m.insert(
                            "is_group".into(),
                            Value::Bool(matches!(get("is_group"), Value::Bool(true))),
                        );
                        if let Value::Number(n) = get("timestamp") {
                            m.insert("timestamp".into(), Value::Number(n));
                        } else {
                            m.insert("timestamp".into(), Value::Number(0.0));
                        }
                        session
                            .messages
                            .lock()
                            .unwrap()
                            .push_back(Value::Dict(Arc::new(m)));
                    }
                    Value::String(t) if t == "result" => {
                        let id = match get("id") {
                            Value::Number(n) => n as u64,
                            _ => continue,
                        };
                        if let Some(tx) = session.pending.lock().unwrap().remove(&id) {
                            let ok = matches!(get("ok"), Value::Bool(true));
                            let err = match get("error") {
                                Value::String(e) => e,
                                _ => String::new(),
                            };
                            let mut r = indexmap::IndexMap::new();
                            r.insert("ok".into(), Value::Bool(ok));
                            r.insert("error".into(), Value::String(err));
                            let _ = tx.send(Value::Dict(Arc::new(r)));
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    // Wait briefly for the bridge to prove itself (any state past "starting").
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if !session.is_alive() {
            let st = session.state.lock().unwrap().clone();
            return Err(format!(
                "wa: bridge exited during startup ({st}) — is ZEN_WA_HOME's node_modules intact?"
            ));
        }
        if *session.state.lock().unwrap() != "starting" {
            break;
        }
        if Instant::now() >= deadline {
            session.shutdown();
            return Err("wa: bridge produced no events within 20s".into());
        }
        thread::sleep(Duration::from_millis(50));
    }

    WA_SESSION.with(|s| *s.borrow_mut() = Some(Arc::clone(&session)));
    Ok(Value::Bool(true))
}

fn with_session<T>(
    f: impl FnOnce(&WaSession) -> Result<T, String>,
) -> Result<T, String> {
    let sess = WA_SESSION.with(|s| s.borrow().clone());
    match sess {
        Some(s) if s.is_alive() => f(&s),
        Some(_) => Err("wa: bridge process died — call wa.connect() again".into()),
        None => Err("wa: not connected (call wa.connect() first)".into()),
    }
}

// ── native entry points ─────────────────────────────────────────────────

pub fn wa_state(_args: &Vec<Value>) -> Result<Value, String> {
    let sess = WA_SESSION.with(|s| s.borrow().clone());
    match sess {
        Some(s) => Ok(Value::String(s.state.lock().unwrap().clone())),
        None => Ok(Value::String("idle".into())),
    }
}

pub fn wa_qr(_args: &Vec<Value>) -> Result<Value, String> {
    let sess = WA_SESSION.with(|s| s.borrow().clone());
    match sess.and_then(|s| s.qr.lock().unwrap().clone()) {
        Some(q) => Ok(Value::String(q)),
        None => Ok(Value::Null),
    }
}

pub fn wa_pairing_code(_args: &Vec<Value>) -> Result<Value, String> {
    let sess = WA_SESSION.with(|s| s.borrow().clone());
    match sess.and_then(|s| s.pairing_code.lock().unwrap().clone()) {
        Some(c) => Ok(Value::String(c)),
        None => Ok(Value::Null),
    }
}

pub fn wa_poll(args: &Vec<Value>) -> Result<Value, String> {
    let timeout_ms = match args.first() {
        Some(Value::Number(n)) => (*n).max(0.0) as u64,
        _ => 0,
    };
    with_session(|s| {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            {
                let mut q = s.messages.lock().unwrap();
                if !q.is_empty() {
                    let items: Vec<Value> = q.drain(..).collect();
                    return Ok(Value::List(Arc::new(items)));
                }
            }
            if Instant::now() >= deadline || !s.is_alive() {
                return Ok(Value::List(Arc::new(Vec::new())));
            }
            thread::sleep(Duration::from_millis(25));
        }
    })
}

pub fn wa_send_text(args: &Vec<Value>) -> Result<Value, String> {
    let (jid, text) = match args.as_slice() {
        [Value::String(j), Value::String(t)] => (j.clone(), t.clone()),
        _ => return Err("wa.send_text expects (jid, text)".into()),
    };
    with_session(|s| {
        let res = s.send_op("send_text", &[("to", Value::String(jid)), ("text", Value::String(text))])?;
        if let Value::Dict(d) = &res {
            if matches!(d.get("ok"), Some(Value::Bool(true))) {
                return Ok(Value::Bool(true));
            }
            let err = match d.get("error") {
                Some(Value::String(e)) => e.clone(),
                _ => "unknown error".into(),
            };
            return Err(format!("wa.send_text failed: {err}"));
        }
        Ok(Value::Bool(false))
    })
}

pub fn wa_logout(_args: &Vec<Value>) -> Result<Value, String> {
    let sess = take_session().ok_or_else(|| "wa: not connected".to_string())?;
    let res = sess.send_op("logout", &[]).unwrap_or(Value::Bool(true));
    sess.shutdown();
    let ok = matches!(res, Value::Dict(ref d) if matches!(d.get("ok"), Some(Value::Bool(true))));
    Ok(Value::Bool(ok))
}

pub fn wa_disconnect(_args: &Vec<Value>) -> Result<Value, String> {
    match take_session() {
        Some(sess) => {
            sess.shutdown();
            Ok(Value::Bool(true))
        }
        None => Ok(Value::Bool(false)),
    }
}

// ── module registration ─────────────────────────────────────────────────

pub fn init_wa_module(vm: &mut Vm) {
    let wa = Value::Dict(Arc::new(indexmap::IndexMap::from([
        ("connect".into(), Value::NativeFunction("wa_connect".into())),
        ("state".into(), Value::NativeFunction("wa_state".into())),
        ("qr".into(), Value::NativeFunction("wa_qr".into())),
        ("pairingCode".into(), Value::NativeFunction("wa_pairing_code".into())),
        ("poll".into(), Value::NativeFunction("wa_poll".into())),
        ("sendText".into(), Value::NativeFunction("wa_send_text".into())),
        ("send".into(), Value::NativeFunction("wa_send_text".into())),
        ("logout".into(), Value::NativeFunction("wa_logout".into())),
        ("disconnect".into(), Value::NativeFunction("wa_disconnect".into())),
    ])));
    vm.vars.insert("wa".into(), wa);
}
