//! Native `wa` module: surface of the WhatsApp Web client.
//!
//! The transport itself (WebSocket + Noise handshake + Signal session) lives
//! outside this offline build, so every operation that would touch the wire
//! raises a clear "not connected" error. Read-only state queries report the
//! disconnected baseline the corpus asserts against.

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::native::host_module_arity;
use crate::value::Value;

const ID_STATE: u16 = 330;
const ID_QR: u16 = 331;
const ID_PAIRING_CODE: u16 = 332;
const ID_DISCONNECT: u16 = 333;
const ID_POLL: u16 = 334;
const ID_SEND_TEXT: u16 = 335;
const ID_LOGOUT: u16 = 336;
const ID_IS_CONNECTED: u16 = 337;
const ID_CONNECT: u16 = 338;
const ID_SEND: u16 = 339;
const ID_SEND_FILE: u16 = 340;

fn not_connected(what: &str) -> VmError {
    VmError::NativeError(format!("wa.{what}: not connected"))
}

pub fn module() -> ModuleUnit {
    host_module_arity(&[
        ("state", ID_STATE, 0, 1),
        ("qr", ID_QR, 0, 0),
        ("pairingCode", ID_PAIRING_CODE, 0, 0),
        ("disconnect", ID_DISCONNECT, 0, 0),
        ("poll", ID_POLL, 0, 1),
        ("sendText", ID_SEND_TEXT, 2, 3),
        ("logout", ID_LOGOUT, 0, 0),
        ("isConnected", ID_IS_CONNECTED, 0, 0),
        ("connect", ID_CONNECT, 0, 1),
        ("send", ID_SEND, 2, 3),
        ("sendFile", ID_SEND_FILE, 2, 4),
    ])
}

pub fn dispatch(id: u16, _args: &[Value]) -> VmResult<Option<Value>> {
    let v = match id {
        ID_STATE => Value::string("idle"),
        ID_QR => Value::nil(),
        ID_PAIRING_CODE => Value::nil(),
        ID_DISCONNECT => Value::bool(false),
        ID_IS_CONNECTED => Value::bool(false),
        ID_POLL => return Err(not_connected("poll")),
        ID_SEND_TEXT => return Err(not_connected("sendText")),
        ID_SEND => return Err(not_connected("send")),
        ID_SEND_FILE => return Err(not_connected("sendFile")),
        ID_LOGOUT => return Err(not_connected("logout")),
        ID_CONNECT => return Err(not_connected("connect")),
        _ => return Ok(None),
    };
    Ok(Some(v))
}