//! Native `hydra` module: online credential tester for a small set of
//! line-oriented protocols. Each attempt opens a fresh connection, walks the
//! protocol handshake and reports `success`/`fail` the way THC-Hydra does.
//!
//! Only `ftp` is wired up so far; other protocol names report `unsupported`
//! rather than silently succeeding.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::gc::{new_dict, new_list};
use crate::native::host_module_arity;
use crate::value::{DictKey, Value};
use indexmap::IndexMap;

const ID_TEST: u16 = 360;
const ID_RUN: u16 = 361;
const ID_RUN_FILE: u16 = 362;

const READ_TIMEOUT: Duration = Duration::from_secs(3);

pub fn module() -> ModuleUnit {
    host_module_arity(&[
        ("test", ID_TEST, 4, 4),
        ("run", ID_RUN, 4, 4),
        ("run_file", ID_RUN_FILE, 4, 4),
    ])
}

fn arg(args: &[Value], i: usize) -> VmResult<Value> {
    args.get(i).copied().ok_or(VmError::ArgumentCount {
        expected: i + 1,
        got: args.len(),
    })
}

fn string_at(args: &[Value], i: usize) -> VmResult<String> {
    let v = arg(args, i)?;
    v.as_str()
        .map(str::to_string)
        .ok_or_else(|| VmError::type_mismatch("string", v))
}

/// `host:port` -> (host, port).
fn split_target(target: &str) -> VmResult<(String, u16)> {
    let (host, port) = target
        .rsplit_once(':')
        .ok_or_else(|| VmError::ValueError(format!("target '{target}' must be host:port")))?;
    let port: u16 = port
        .parse()
        .map_err(|_| VmError::ValueError(format!("bad port in '{target}'")))?;
    Ok((host.to_string(), port))
}

/// Read one FTP response: lines end in `\r\n`; a line whose 4th byte is a
/// space terminates the reply, a `-` means "more lines follow".
fn read_response(stream: &mut TcpStream) -> VmResult<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let n = stream
            .read(&mut chunk)
            .map_err(|e| VmError::NativeError(format!("hydra read: {e}")))?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        // Complete once the buffer ends with CRLF and the reply code is
        // followed by a space (single-line reply) on the final line.
        if buf.len() >= 4 && buf.ends_with(b"\r\n") {
            let line_start = buf
                .windows(2)
                .rposition(|w| w == b"\r\n")
                .and_then(|pos| {
                    buf[..pos]
                        .windows(2)
                        .rposition(|w| w == b"\r\n")
                        .map(|p| p + 2)
                })
                .unwrap_or(0);
            if let Some(line) = buf.get(line_start..buf.len() - 2) {
                if line.len() >= 4 && line[3] == b' ' {
                    break;
                }
                if line.len() >= 4 && line[3] == b'-' {
                    continue;
                }
                break;
            }
        }
        if buf.len() > 64 * 1024 {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Attempt one FTP login. Returns `true` only on a `230` (logged in).
fn ftp_login(host: &str, port: u16, user: &str, pass: &str) -> VmResult<bool> {
    let mut stream =
        TcpStream::connect((host, port)).map_err(|e| VmError::NativeError(format!("connect: {e}")))?;
    stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .map_err(|e| VmError::NativeError(format!("set_read_timeout: {e}")))?;

    let banner = read_response(&mut stream)?;
    if !banner.starts_with("220") {
        return Ok(false);
    }
    stream
        .write_all(format!("USER {user}\r\n").as_bytes())
        .and_then(|_| stream.flush())
        .map_err(|e| VmError::NativeError(format!("send USER: {e}")))?;
    let _ = read_response(&mut stream)?;
    stream
        .write_all(format!("PASS {pass}\r\n").as_bytes())
        .and_then(|_| stream.flush())
        .map_err(|e| VmError::NativeError(format!("send PASS: {e}")))?;
    let reply = read_response(&mut stream)?;
    Ok(reply.starts_with("230"))
}

/// One credential attempt against `protocol`.
fn attempt(
    protocol: &str,
    target: &str,
    user: &str,
    pass: &str,
) -> VmResult<&'static str> {
    match protocol {
        "ftp" => {
            let (host, port) = split_target(target)?;
            let ok = ftp_login(&host, port, user, pass)?;
            Ok(if ok { "success" } else { "fail" })
        }
        other => Err(VmError::NativeError(format!(
            "hydra: unsupported protocol '{other}'"
        ))),
    }
}

fn status_dict(protocol: &str, target: &str, user: &str, status: &'static str) -> Value {
    let mut d = IndexMap::new();
    d.insert(DictKey(Value::string("protocol")), Value::string(protocol));
    d.insert(DictKey(Value::string("target")), Value::string(target));
    d.insert(DictKey(Value::string("username")), Value::string(user));
    d.insert(DictKey(Value::string("status")), Value::string(status));
    new_dict(d)
}

fn run_passwords(
    protocol: &str,
    target: &str,
    user: &str,
    passwords: &[Value],
) -> VmResult<Value> {
    let mut results = Vec::new();
    let mut found: Option<String> = None;
    let mut attempts = 0usize;
    for pw in passwords {
        attempts += 1;
        let text = pw.as_str().map(str::to_string).unwrap_or_default();
        let status = attempt(protocol, target, user, &text)?;
        let mut r = IndexMap::new();
        r.insert(DictKey(Value::string("attempt")), Value::int(attempts as i64));
        r.insert(DictKey(Value::string("password")), Value::string(text.as_str()));
        r.insert(DictKey(Value::string("status")), Value::string(status));
        results.push(new_dict(r));
        if status == "success" {
            found = Some(text);
            break;
        }
    }
    let mut out = IndexMap::new();
    let success = found.is_some();
    out.insert(
        DictKey(Value::string("success")),
        Value::bool(success),
    );
    out.insert(
        DictKey(Value::string("password")),
        match &found {
            Some(p) => Value::string(p.as_str()),
            None => Value::nil(),
        },
    );
    out.insert(DictKey(Value::string("attempts")), Value::int(attempts as i64));
    out.insert(DictKey(Value::string("results")), new_list(results));
    Ok(new_dict(out))
}

fn password_list(args: &[Value], i: usize) -> VmResult<Vec<Value>> {
    let v = arg(args, i)?;
    match v.as_list() {
        Some(items) => Ok(items.to_vec()),
        None => Err(VmError::type_mismatch("list of passwords", v)),
    }
}

pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Option<Value>> {
    let v = match id {
        ID_TEST => {
            let protocol = string_at(args, 0)?;
            let target = string_at(args, 1)?;
            let user = string_at(args, 2)?;
            let pass = string_at(args, 3)?;
            let status = attempt(&protocol, &target, &user, &pass)?;
            status_dict(&protocol, &target, &user, status)
        }
        ID_RUN => {
            let protocol = string_at(args, 0)?;
            let target = string_at(args, 1)?;
            let user = string_at(args, 2)?;
            let pws = password_list(args, 3)?;
            run_passwords(&protocol, &target, &user, &pws)?
        }
        ID_RUN_FILE => {
            let protocol = string_at(args, 0)?;
            let target = string_at(args, 1)?;
            let user = string_at(args, 2)?;
            let path = string_at(args, 3)?;
            let text = std::fs::read_to_string(&path)
                .map_err(|e| VmError::NativeError(format!("run_file: {e}")))?;
            let pws: Vec<Value> = text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(|l| Value::string(l))
                .collect();
            run_passwords(&protocol, &target, &user, &pws)?
        }
        _ => return Ok(None),
    };
    Ok(Some(v))
}