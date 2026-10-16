//! Python-parity BSD socket layer.
//!
//! This is Tier 1+3 of the networking stack: OS-backed sockets (`AF_INET`,
//! `AF_INET6`, `AF_UNIX`) plus raw sockets (`SOCK_RAW`), implemented directly on
//! `libc` so the same code path serves TCP, UDP, Unix-domain and raw sockets and
//! gives exact control over blocking, timeouts and socket options.
//!
//! Everything is handle based: `create` returns an integer handle, every other
//! call takes that handle first. The surface mirrors Python's `socket` module
//! (`socket`, `bind`, `listen`, `accept`, `connect`, `send`, `recv`,
//! `setsockopt`, `getaddrinfo`, `inet_pton`, `socketpair`, …) plus the legacy
//! Zen names (`open`, `recv_text`, `set_timeout`) the existing corpus uses.

#![allow(unsafe_code)]

use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::gc::{new_dict, new_list};
use crate::native::{func, host_module_mixed, HostExport};
use crate::value::{DictKey, Value};
use indexmap::IndexMap;
use libc::{c_int, c_void, socklen_t};

// ── module-level constants ──────────────────────────────────────────────────

const AF_UNIX: c_int = libc::AF_UNIX;
const AF_INET: c_int = libc::AF_INET;
const AF_INET6: c_int = libc::AF_INET6;
const AF_UNSPEC: c_int = libc::AF_UNSPEC;

const SOCK_STREAM: c_int = libc::SOCK_STREAM;
const SOCK_DGRAM: c_int = libc::SOCK_DGRAM;
const SOCK_RAW: c_int = libc::SOCK_RAW;

const SOL_SOCKET: c_int = libc::SOL_SOCKET;
const IPPROTO_IP: c_int = libc::IPPROTO_IP;
const IPPROTO_ICMP: c_int = libc::IPPROTO_ICMP;
const IPPROTO_TCP: c_int = libc::IPPROTO_TCP;
const IPPROTO_UDP: c_int = libc::IPPROTO_UDP;
const IPPROTO_IPV6: c_int = libc::IPPROTO_IPV6;
const IPPROTO_RAW: c_int = libc::IPPROTO_RAW;

const SHUT_RD: c_int = libc::SHUT_RD;
const SHUT_WR: c_int = libc::SHUT_WR;
const SHUT_RDWR: c_int = libc::SHUT_RDWR;

const MSG_NOSIGNAL: c_int = libc::MSG_NOSIGNAL;

/// `NI_MAXSERV` is not exported by every `libc` build; Linux uses 32.
const NI_MAXSERV: usize = 32;

// ── registry ────────────────────────────────────────────────────────────────

struct Entry {
    fd: OwnedFd,
    family: c_int,
    socktype: c_int,
    proto: c_int,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
}

struct Registry {
    next: u32,
    open: HashMap<u32, Arc<Mutex<Entry>>>,
}

impl Registry {
    fn new() -> Self {
        Self {
            next: 1,
            open: HashMap::new(),
        }
    }
}

fn registry() -> std::sync::MutexGuard<'static, Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(Registry::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn insert(entry: Entry) -> Value {
    let mut reg = registry();
    let handle = reg.next;
    reg.next += 1;
    reg.open.insert(handle, Arc::new(Mutex::new(entry)));
    Value::int_promote(handle as i64)
}

fn lookup(handle: u32) -> VmResult<Arc<Mutex<Entry>>> {
    registry()
        .open
        .get(&handle)
        .cloned()
        .ok_or_else(|| VmError::HandleError(format!("no such socket: {handle}")))
}

fn handle_arg(args: &[Value], i: usize) -> VmResult<u32> {
    let raw = crate::ops::int_arg(args, i)?;
    u32::try_from(raw).map_err(|_| VmError::HandleError(format!("bad socket handle {raw}")))
}

fn last_err(op: &str) -> VmError {
    VmError::NativeError(format!("{op}: {}", std::io::Error::last_os_error()))
}

fn errno_of(err: &std::io::Error) -> i64 {
    err.raw_os_error().unwrap_or(0) as i64
}

// ── argument coercion ───────────────────────────────────────────────────────

fn data_arg(args: &[Value], i: usize) -> VmResult<Vec<u8>> {
    let v = *args.get(i).ok_or(VmError::ArgumentCount {
        expected: i + 1,
        got: args.len(),
    })?;
    if let Some(b) = v.as_bytes() {
        Ok(b.to_vec())
    } else if let Some(s) = v.as_str() {
        Ok(s.as_bytes().to_vec())
    } else {
        Err(VmError::type_mismatch("bytes or string", v))
    }
}

fn opt_int(args: &[Value], i: usize, default: i64) -> i64 {
    match args.get(i).copied() {
        Some(v) if !v.is_nil() => v.as_int().unwrap_or(default),
        _ => default,
    }
}

// ── sockaddr helpers ────────────────────────────────────────────────────────

fn to_storage(addr: &SocketAddr) -> (libc::sockaddr_storage, socklen_t) {
    unsafe {
        let mut storage: libc::sockaddr_storage = std::mem::zeroed();
        match addr {
            SocketAddr::V4(v4) => {
                let sin = &mut *(&mut storage as *mut _ as *mut libc::sockaddr_in);
                sin.sin_family = AF_INET as libc::sa_family_t;
                sin.sin_port = v4.port().to_be();
                // `octets()` is already network order; `from_ne_bytes` keeps the
                // byte order identical in memory (which `s_addr` expects).
                sin.sin_addr.s_addr = u32::from_ne_bytes(v4.ip().octets());
                (storage, std::mem::size_of::<libc::sockaddr_in>() as socklen_t)
            }
            SocketAddr::V6(v6) => {
                let sin6 = &mut *(&mut storage as *mut _ as *mut libc::sockaddr_in6);
                sin6.sin6_family = AF_INET6 as libc::sa_family_t;
                sin6.sin6_port = v6.port().to_be();
                sin6.sin6_flowinfo = v6.flowinfo();
                sin6.sin6_addr.s6_addr = v6.ip().octets();
                sin6.sin6_scope_id = v6.scope_id();
                (storage, std::mem::size_of::<libc::sockaddr_in6>() as socklen_t)
            }
        }
    }
}

fn unix_storage(path: &str) -> VmResult<(libc::sockaddr_un, socklen_t)> {
    unsafe {
        let mut storage: libc::sockaddr_un = std::mem::zeroed();
        storage.sun_family = AF_UNIX as libc::sa_family_t;
        let bytes = path.as_bytes();
        // sun_path is a fixed-size C array; leave room for the NUL terminator.
        if bytes.len() >= storage.sun_path.len() {
            return Err(VmError::ValueError(format!(
                "AF_UNIX path too long ({} bytes)",
                bytes.len()
            )));
        }
        for (i, b) in bytes.iter().enumerate() {
            storage.sun_path[i] = *b as libc::c_char;
        }
        let len = std::mem::size_of::<libc::sa_family_t>() + bytes.len() + 1;
        Ok((storage, len as socklen_t))
    }
}

/// Resolve `host:port` to concrete addresses honouring `family`.
fn resolve_addrs(family: c_int, host: &str, port: u16) -> VmResult<Vec<SocketAddr>> {
    if host.is_empty() || host == "*" {
        return Ok(match family {
            AF_INET6 => vec![SocketAddr::from((std::net::Ipv6Addr::UNSPECIFIED, port))],
            _ => vec![SocketAddr::from((std::net::Ipv4Addr::UNSPECIFIED, port))],
        });
    }
    let iter = (host, port)
        .to_socket_addrs()
        .map_err(|e| VmError::NativeError(format!("resolve {host}:{port}: {e}")))?;
    let mut out: Vec<SocketAddr> = Vec::new();
    for a in iter {
        let keep = match family {
            AF_INET => a.is_ipv4(),
            AF_INET6 => a.is_ipv6(),
            _ => true,
        };
        if keep {
            out.push(a);
        }
    }
    if out.is_empty() {
        return Err(VmError::NativeError(format!(
            "no address for {host}:{port} matching family {family}"
        )));
    }
    Ok(out)
}

fn storage_to_value(storage: &libc::sockaddr_storage, family: c_int) -> Value {
    unsafe {
        match family {
            AF_INET => {
                let sin = &*(storage as *const _ as *const libc::sockaddr_in);
                let octets = sin.sin_addr.s_addr.to_ne_bytes();
                let ip = std::net::Ipv4Addr::from(octets);
                new_list(vec![
                    Value::string(ip.to_string()),
                    Value::int_promote(u16::from_be(sin.sin_port) as i64),
                ])
            }
            AF_INET6 => {
                let sin6 = &*(storage as *const _ as *const libc::sockaddr_in6);
                let ip = std::net::Ipv6Addr::from(sin6.sin6_addr.s6_addr);
                new_list(vec![
                    Value::string(ip.to_string()),
                    Value::int_promote(u16::from_be(sin6.sin6_port) as i64),
                ])
            }
            AF_UNIX => {
                let sun = &*(storage as *const _ as *const libc::sockaddr_un);
                let bytes: Vec<u8> = sun
                    .sun_path
                    .iter()
                    .take_while(|c| **c != 0)
                    .map(|c| *c as u8)
                    .collect();
                Value::string(String::from_utf8_lossy(&bytes).into_owned())
            }
            _ => Value::nil(),
        }
    }
}

// ── native operations ───────────────────────────────────────────────────────

fn socket_create(args: &[Value]) -> VmResult<Value> {
    let family = opt_int(args, 0, AF_INET as i64) as c_int;
    let socktype = opt_int(args, 1, SOCK_STREAM as i64) as c_int;
    let proto = opt_int(args, 2, 0) as c_int;
    let fd = unsafe { libc::socket(family, socktype | libc::SOCK_CLOEXEC, proto) };
    if fd < 0 {
        return Err(last_err("socket"));
    }
    Ok(insert(Entry {
        fd: unsafe { OwnedFd::from_raw_fd(fd) },
        family,
        socktype,
        proto,
        read_timeout: None,
        write_timeout: None,
    }))
}

/// Legacy `socket.open(host, port)`: create an IPv4 TCP stream and connect.
fn socket_open(args: &[Value]) -> VmResult<Value> {
    let host = crate::ops::string_arg(args, 0)?;
    let port = crate::ops::int_arg(args, 1)?;
    if !(0..=i64::from(u16::MAX)).contains(&port) {
        return Err(VmError::ValueError(format!("port {port} out of range")));
    }
    let handle = socket_create(&[
        Value::int_promote(AF_INET as i64),
        Value::int_promote(SOCK_STREAM as i64),
        Value::int_promote(0),
    ])?;
    let h = handle.as_int().unwrap_or(0) as u32;
    let addrs = resolve_addrs(AF_INET, &host, port as u16)?;
    let entry = lookup(h)?;
    let fd = {
        let guard = entry.lock().unwrap_or_else(|e| e.into_inner());
        guard.fd.as_raw_fd()
    };
    let mut last = None;
    for addr in addrs {
        let (storage, len) = to_storage(&addr);
        let rc = unsafe {
            libc::connect(
                fd,
                &storage as *const _ as *const libc::sockaddr,
                len,
            )
        };
        if rc == 0 {
            return Ok(handle);
        }
        last = Some(std::io::Error::last_os_error());
    }
    // Drop the handle we created before reporting the failure.
    registry().open.remove(&h);
    Err(VmError::NativeError(format!(
        "connect {host}:{port}: {}",
        last.map(|e| e.to_string()).unwrap_or_default()
    )))
}

fn socket_bind(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let entry = lookup(handle)?;
    let (family, fd) = {
        let g = entry.lock().unwrap_or_else(|e| e.into_inner());
        (g.family, g.fd.as_raw_fd())
    };
    let rc = if family == AF_UNIX {
        let path = crate::ops::string_arg(args, 1)?;
        let (storage, len) = unix_storage(&path)?;
        unsafe {
            libc::bind(
                fd,
                &storage as *const _ as *const libc::sockaddr,
                len,
            )
        }
    } else {
        let host = crate::ops::string_arg(args, 1)?;
        let port = crate::ops::int_arg(args, 2)?;
        if !(0..=i64::from(u16::MAX)).contains(&port) {
            return Err(VmError::ValueError(format!("port {port} out of range")));
        }
        let addrs = resolve_addrs(family, &host, port as u16)?;
        let (storage, len) = to_storage(&addrs[0]);
        unsafe {
            libc::bind(
                fd,
                &storage as *const _ as *const libc::sockaddr,
                len,
            )
        }
    };
    if rc != 0 {
        return Err(last_err("bind"));
    }
    Ok(Value::nil())
}

fn socket_listen(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let backlog = opt_int(args, 1, 128) as c_int;
    let entry = lookup(handle)?;
    let fd = entry.lock().unwrap_or_else(|e| e.into_inner()).fd.as_raw_fd();
    if unsafe { libc::listen(fd, backlog) } != 0 {
        return Err(last_err("listen"));
    }
    Ok(Value::nil())
}

fn socket_accept(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let entry = lookup(handle)?;
    let (fd, family, socktype, proto) = {
        let g = entry.lock().unwrap_or_else(|e| e.into_inner());
        (g.fd.as_raw_fd(), g.family, g.socktype, g.proto)
    };
    let mut storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::sockaddr_storage>() as socklen_t;
    let newfd = unsafe {
        libc::accept4(
            fd,
            &mut storage as *mut _ as *mut libc::sockaddr,
            &mut len,
            libc::SOCK_CLOEXEC,
        )
    };
    if newfd < 0 {
        return Err(last_err("accept"));
    }
    Ok(insert(Entry {
        fd: unsafe { OwnedFd::from_raw_fd(newfd) },
        family,
        socktype,
        proto,
        read_timeout: None,
        write_timeout: None,
    }))
}

fn do_connect(handle: u32, host: &str, port: u16) -> VmResult<()> {
    let entry = lookup(handle)?;
    let (family, fd) = {
        let g = entry.lock().unwrap_or_else(|e| e.into_inner());
        (g.family, g.fd.as_raw_fd())
    };
    if family == AF_UNIX {
        let (storage, len) = unix_storage(host)?;
        let rc = unsafe {
            libc::connect(fd, &storage as *const _ as *const libc::sockaddr, len)
        };
        return if rc == 0 {
            Ok(())
        } else {
            Err(last_err("connect"))
        };
    }
    let addrs = resolve_addrs(family, host, port)?;
    let mut last = None;
    for addr in addrs {
        let (storage, len) = to_storage(&addr);
        let rc = unsafe {
            libc::connect(fd, &storage as *const _ as *const libc::sockaddr, len)
        };
        if rc == 0 {
            return Ok(());
        }
        last = Some(std::io::Error::last_os_error());
    }
    Err(VmError::NativeError(format!(
        "connect {host}:{port}: {}",
        last.map(|e| e.to_string()).unwrap_or_default()
    )))
}

fn socket_connect(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let host = crate::ops::string_arg(args, 1)?;
    let entry = lookup(handle)?;
    let family = entry.lock().unwrap_or_else(|e| e.into_inner()).family;
    let port = if family == AF_UNIX {
        0
    } else {
        let p = crate::ops::int_arg(args, 2)?;
        if !(0..=i64::from(u16::MAX)).contains(&p) {
            return Err(VmError::ValueError(format!("port {p} out of range")));
        }
        p as u16
    };
    do_connect(handle, &host, port)?;
    Ok(Value::nil())
}

fn socket_connect_ex(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let host = crate::ops::string_arg(args, 1)?;
    let entry = lookup(handle)?;
    let family = entry.lock().unwrap_or_else(|e| e.into_inner()).family;
    let port = if family == AF_UNIX {
        0
    } else {
        crate::ops::int_arg(args, 2)? as u16
    };
    match do_connect(handle, &host, port) {
        Ok(()) => Ok(Value::int(0)),
        Err(VmError::NativeError(m)) => {
            let e = std::io::Error::last_os_error();
            let _ = m;
            Ok(Value::int(errno_of(&e)))
        }
        Err(e) => Err(e),
    }
}

fn socket_send(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let data = data_arg(args, 1)?;
    let entry = lookup(handle)?;
    let fd = entry.lock().unwrap_or_else(|e| e.into_inner()).fd.as_raw_fd();
    let n = unsafe {
        libc::send(
            fd,
            data.as_ptr() as *const c_void,
            data.len(),
            MSG_NOSIGNAL,
        )
    };
    if n < 0 {
        return Err(last_err("send"));
    }
    Ok(Value::int_promote(n as i64))
}

fn socket_sendall(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let data = data_arg(args, 1)?;
    let entry = lookup(handle)?;
    let fd = entry.lock().unwrap_or_else(|e| e.into_inner()).fd.as_raw_fd();
    let mut sent = 0usize;
    while sent < data.len() {
        let n = unsafe {
            libc::send(
                fd,
                data[sent..].as_ptr() as *const c_void,
                data.len() - sent,
                MSG_NOSIGNAL,
            )
        };
        if n < 0 {
            return Err(last_err("sendall"));
        }
        if n == 0 {
            break;
        }
        sent += n as usize;
    }
    Ok(Value::nil())
}

fn socket_recv(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let want = opt_int(args, 1, 4096);
    if want < 0 {
        return Err(VmError::ValueError(format!("recv size {want} must be >= 0")));
    }
    let want = want.min(64 * 1024 * 1024) as usize;
    let entry = lookup(handle)?;
    let fd = entry.lock().unwrap_or_else(|e| e.into_inner()).fd.as_raw_fd();
    let mut buf = vec![0u8; want];
    let n = unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut c_void, want, 0) };
    if n < 0 {
        return Err(last_err("recv"));
    }
    buf.truncate(n as usize);
    Ok(Value::bytes(buf))
}

fn socket_recv_text(args: &[Value]) -> VmResult<Value> {
    let v = socket_recv(args)?;
    let bytes = v.as_bytes().map(|b| b.to_vec()).unwrap_or_default();
    Ok(Value::string(String::from_utf8_lossy(&bytes).into_owned()))
}

fn socket_sendto(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let data = data_arg(args, 1)?;
    let entry = lookup(handle)?;
    let (family, fd) = {
        let g = entry.lock().unwrap_or_else(|e| e.into_inner());
        (g.family, g.fd.as_raw_fd())
    };
    let n = if family == AF_UNIX {
        let path = crate::ops::string_arg(args, 2)?;
        let (storage, len) = unix_storage(&path)?;
        unsafe {
            libc::sendto(
                fd,
                data.as_ptr() as *const c_void,
                data.len(),
                MSG_NOSIGNAL,
                &storage as *const _ as *const libc::sockaddr,
                len,
            )
        }
    } else {
        let host = crate::ops::string_arg(args, 2)?;
        let port = crate::ops::int_arg(args, 3)?;
        let addrs = resolve_addrs(family, &host, port as u16)?;
        let (storage, len) = to_storage(&addrs[0]);
        unsafe {
            libc::sendto(
                fd,
                data.as_ptr() as *const c_void,
                data.len(),
                MSG_NOSIGNAL,
                &storage as *const _ as *const libc::sockaddr,
                len,
            )
        }
    };
    if n < 0 {
        return Err(last_err("sendto"));
    }
    Ok(Value::int_promote(n as i64))
}

fn socket_recvfrom(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let want = opt_int(args, 1, 4096).max(0).min(64 * 1024 * 1024) as usize;
    let entry = lookup(handle)?;
    let (family, fd) = {
        let g = entry.lock().unwrap_or_else(|e| e.into_inner());
        (g.family, g.fd.as_raw_fd())
    };
    let mut buf = vec![0u8; want];
    let mut storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::sockaddr_storage>() as socklen_t;
    let n = unsafe {
        libc::recvfrom(
            fd,
            buf.as_mut_ptr() as *mut c_void,
            want,
            0,
            &mut storage as *mut _ as *mut libc::sockaddr,
            &mut len,
        )
    };
    if n < 0 {
        return Err(last_err("recvfrom"));
    }
    buf.truncate(n as usize);
    let addr = storage_to_value(&storage, family);
    Ok(new_list(vec![Value::bytes(buf), addr]))
}

fn socket_setsockopt(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let level = crate::ops::int_arg(args, 1)? as c_int;
    let opt = crate::ops::int_arg(args, 2)? as c_int;
    let value = *args.get(3).ok_or(VmError::ArgumentCount {
        expected: 4,
        got: 3,
    })?;
    let entry = lookup(handle)?;
    let fd = entry.lock().unwrap_or_else(|e| e.into_inner()).fd.as_raw_fd();

    let rc = if opt == libc::SO_RCVTIMEO || opt == libc::SO_SNDTIMEO {
        let secs = if let Some(f) = value.as_float() {
            f
        } else if let Some(i) = value.as_int() {
            i as f64
        } else if value.is_nil() {
            0.0
        } else {
            return Err(VmError::type_mismatch("number or null", value));
        };
        let tv = libc::timeval {
            tv_sec: secs as libc::time_t,
            tv_usec: ((secs.fract()) * 1_000_000.0) as libc::suseconds_t,
        };
        unsafe {
            libc::setsockopt(
                fd,
                level,
                opt,
                &tv as *const _ as *const c_void,
                std::mem::size_of::<libc::timeval>() as socklen_t,
            )
        }
    } else if opt == libc::SO_LINGER {
        let (onoff, linger) = if let Some(list) = value.as_list() {
            (
                list.first().and_then(|v| v.as_int()).unwrap_or(0) as c_int,
                list.get(1).and_then(|v| v.as_int()).unwrap_or(0) as c_int,
            )
        } else {
            (1, value.as_int().unwrap_or(0) as c_int)
        };
        let l = libc::linger {
            l_onoff: onoff,
            l_linger: linger,
        };
        unsafe {
            libc::setsockopt(
                fd,
                level,
                opt,
                &l as *const _ as *const c_void,
                std::mem::size_of::<libc::linger>() as socklen_t,
            )
        }
    } else if let Some(b) = value.as_bytes() {
        unsafe {
            libc::setsockopt(
                fd,
                level,
                opt,
                b.as_ptr() as *const c_void,
                b.len() as socklen_t,
            )
        }
    } else {
        let n = value
            .as_int()
            .unwrap_or_else(|| if value.as_bool() == Some(true) { 1 } else { 0 }) as c_int;
        unsafe {
            libc::setsockopt(
                fd,
                level,
                opt,
                &n as *const _ as *const c_void,
                std::mem::size_of::<c_int>() as socklen_t,
            )
        }
    };
    if rc != 0 {
        return Err(last_err("setsockopt"));
    }
    Ok(Value::nil())
}

fn socket_getsockopt(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let level = crate::ops::int_arg(args, 1)? as c_int;
    let opt = crate::ops::int_arg(args, 2)? as c_int;
    let default_len = if opt == libc::SO_LINGER {
        std::mem::size_of::<libc::linger>()
    } else if opt == libc::SO_RCVTIMEO || opt == libc::SO_SNDTIMEO {
        std::mem::size_of::<libc::timeval>()
    } else {
        4
    };
    let want = opt_int(args, 3, default_len as i64).max(1) as usize;
    let entry = lookup(handle)?;
    let fd = entry.lock().unwrap_or_else(|e| e.into_inner()).fd.as_raw_fd();
    let mut buf = vec![0u8; want];
    let mut len = want as socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            fd,
            level,
            opt,
            buf.as_mut_ptr() as *mut c_void,
            &mut len,
        )
    };
    if rc != 0 {
        return Err(last_err("getsockopt"));
    }
    buf.truncate(len as usize);
    if len as usize == 4 {
        let n = i32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]);
        Ok(Value::int_promote(n as i64))
    } else {
        Ok(Value::bytes(buf))
    }
}

fn socket_getpeername(args: &[Value]) -> VmResult<Value> {
    socket_peer_name(args, true)
}

fn socket_getsockname(args: &[Value]) -> VmResult<Value> {
    socket_peer_name(args, false)
}

fn socket_peer_name(args: &[Value], peer: bool) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let entry = lookup(handle)?;
    let (family, fd) = {
        let g = entry.lock().unwrap_or_else(|e| e.into_inner());
        (g.family, g.fd.as_raw_fd())
    };
    let mut storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::sockaddr_storage>() as socklen_t;
    let rc = unsafe {
        if peer {
            libc::getpeername(fd, &mut storage as *mut _ as *mut libc::sockaddr, &mut len)
        } else {
            libc::getsockname(fd, &mut storage as *mut _ as *mut libc::sockaddr, &mut len)
        }
    };
    if rc != 0 {
        return Err(last_err(if peer { "getpeername" } else { "getsockname" }));
    }
    Ok(storage_to_value(&storage, family))
}

fn socket_shutdown(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let how = opt_int(args, 1, SHUT_RDWR as i64) as c_int;
    let entry = lookup(handle)?;
    let fd = entry.lock().unwrap_or_else(|e| e.into_inner()).fd.as_raw_fd();
    if unsafe { libc::shutdown(fd, how) } != 0 {
        return Err(last_err("shutdown"));
    }
    Ok(Value::nil())
}

fn socket_close(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    registry()
        .open
        .remove(&handle)
        .ok_or_else(|| VmError::HandleError(format!("no such socket: {handle}")))?;
    Ok(Value::nil())
}

fn socket_fileno(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let entry = lookup(handle)?;
    let fd = entry.lock().unwrap_or_else(|e| e.into_inner()).fd.as_raw_fd();
    Ok(Value::int_promote(fd as i64))
}

fn socket_dup(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let entry = lookup(handle)?;
    let (fd, family, socktype, proto) = {
        let g = entry.lock().unwrap_or_else(|e| e.into_inner());
        (g.fd.as_raw_fd(), g.family, g.socktype, g.proto)
    };
    let newfd = unsafe { libc::dup(fd) };
    if newfd < 0 {
        return Err(last_err("dup"));
    }
    Ok(insert(Entry {
        fd: unsafe { OwnedFd::from_raw_fd(newfd) },
        family,
        socktype,
        proto,
        read_timeout: None,
        write_timeout: None,
    }))
}

fn apply_timeout(handle: u32, secs: Option<f64>) -> VmResult<()> {
    let entry = lookup(handle)?;
    let mut g = entry.lock().unwrap_or_else(|e| e.into_inner());
    let dur = secs.map(Duration::from_secs_f64);
    g.read_timeout = dur;
    g.write_timeout = dur;
    let (secs_i, usec) = match secs {
        Some(s) => (s as libc::time_t, (s.fract() * 1_000_000.0) as libc::suseconds_t),
        None => (0, 0),
    };
    let tv = libc::timeval {
        tv_sec: secs_i,
        tv_usec: usec,
    };
    let fd = g.fd.as_raw_fd();
    for opt in [libc::SO_RCVTIMEO, libc::SO_SNDTIMEO] {
        let rc = unsafe {
            libc::setsockopt(
                fd,
                SOL_SOCKET,
                opt,
                &tv as *const _ as *const c_void,
                std::mem::size_of::<libc::timeval>() as socklen_t,
            )
        };
        if rc != 0 {
            return Err(last_err("settimeout"));
        }
    }
    Ok(())
}

fn socket_settimeout(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let v = *args.get(1).ok_or(VmError::ArgumentCount {
        expected: 2,
        got: 1,
    })?;
    if v.is_nil() {
        apply_timeout(handle, None)?;
    } else if let Some(f) = v.as_float() {
        apply_timeout(handle, Some(f.max(0.0)))?;
    } else if let Some(i) = v.as_int() {
        apply_timeout(handle, Some(i as f64))?;
    } else {
        return Err(VmError::type_mismatch("number or null", v));
    }
    Ok(Value::nil())
}

/// Legacy `socket.set_timeout(handle, millis)`.
fn socket_set_timeout_ms(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let millis = crate::ops::int_arg(args, 1)?;
    if millis < 0 {
        return Err(VmError::ValueError(format!(
            "timeout {millis}ms must be >= 0"
        )));
    }
    apply_timeout(handle, Some(millis as f64 / 1000.0))?;
    Ok(Value::nil())
}

fn socket_gettimeout(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let entry = lookup(handle)?;
    let timeout = entry
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .read_timeout;
    Ok(match timeout {
        Some(d) => Value::float(d.as_secs_f64()),
        None => Value::nil(),
    })
}

fn socket_setblocking(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let blocking = args.get(1).map(|v| crate::ops::truthy(*v)).unwrap_or(true);
    let entry = lookup(handle)?;
    let fd = entry.lock().unwrap_or_else(|e| e.into_inner()).fd.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(last_err("fcntl"));
    }
    let new = if blocking {
        flags & !libc::O_NONBLOCK
    } else {
        flags | libc::O_NONBLOCK
    };
    if unsafe { libc::fcntl(fd, libc::F_SETFL, new) } < 0 {
        return Err(last_err("fcntl"));
    }
    Ok(Value::nil())
}

fn socket_type_of(args: &[Value]) -> VmResult<Value> {
    let handle = handle_arg(args, 0)?;
    let entry = lookup(handle)?;
    let (family, socktype) = {
        let g = entry.lock().unwrap_or_else(|e| e.into_inner());
        (g.family, g.socktype)
    };
    let name = if socktype == SOCK_DGRAM {
        "udp"
    } else if socktype == SOCK_RAW {
        "raw"
    } else if family == AF_UNIX {
        "unix"
    } else {
        "tcp"
    };
    Ok(Value::string(name))
}

// ── module level helpers ────────────────────────────────────────────────────

fn gethostname(_args: &[Value]) -> VmResult<Value> {
    let mut buf = vec![0u8; 256];
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if rc != 0 {
        return Err(last_err("gethostname"));
    }
    let n = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    buf.truncate(n);
    Ok(Value::string(String::from_utf8_lossy(&buf).into_owned()))
}

fn gethostbyname(args: &[Value]) -> VmResult<Value> {
    let name = crate::ops::string_arg(args, 0)?;
    let addrs = resolve_addrs(AF_UNSPEC, &name, 0)?;
    for a in &addrs {
        if let SocketAddr::V4(v4) = a {
            return Ok(Value::string(v4.ip().to_string()));
        }
    }
    Ok(Value::string(addrs[0].ip().to_string()))
}

fn gethostbyaddr(args: &[Value]) -> VmResult<Value> {
    let addr = crate::ops::string_arg(args, 0)?;
    let (host, _) = getnameinfo_impl(&addr, 0, libc::NI_NAMEREQD as i32)?;
    Ok(new_list(vec![
        Value::string(host),
        new_list(vec![]),
        new_list(vec![Value::string(addr)]),
    ]))
}

fn getaddrinfo(args: &[Value]) -> VmResult<Value> {
    let host = crate::ops::string_arg(args, 0)?;
    let port = opt_int(args, 1, 0);
    let family = opt_int(args, 2, AF_UNSPEC as i64) as c_int;
    let socktype = opt_int(args, 3, 0) as c_int;
    let proto = opt_int(args, 4, 0) as c_int;
    let addrs = resolve_addrs(family, &host, port as u16)?;
    let mut out = Vec::new();
    for a in addrs {
        let mut rec = IndexMap::new();
        let fam = if a.is_ipv4() { AF_INET } else { AF_INET6 };
        rec.insert(DictKey(Value::string("family")), Value::int_promote(fam as i64));
        rec.insert(DictKey(Value::string("type")),
            Value::int_promote(if socktype != 0 { socktype } else { SOCK_STREAM } as i64),
        );
        rec.insert(DictKey(Value::string("proto")), Value::int_promote(proto as i64));
        rec.insert(DictKey(Value::string("sockaddr")),
            new_list(vec![
                Value::string(a.ip().to_string()),
                Value::int_promote(a.port() as i64),
            ]),
        );
        out.push(new_dict(rec));
    }
    Ok(new_list(out))
}

fn getnameinfo_impl(host: &str, port: u16, flags: i32) -> VmResult<(String, String)> {
    let addrs = resolve_addrs(AF_UNSPEC, host, port)?;
    let (storage, len) = to_storage(&addrs[0]);
    let mut hostbuf = vec![0u8; libc::NI_MAXHOST as usize];
    let mut servbuf = vec![0u8; NI_MAXSERV as usize];
    let rc = unsafe {
        libc::getnameinfo(
            &storage as *const _ as *const libc::sockaddr,
            len,
            hostbuf.as_mut_ptr() as *mut libc::c_char,
            hostbuf.len() as socklen_t,
            servbuf.as_mut_ptr() as *mut libc::c_char,
            servbuf.len() as socklen_t,
            flags,
        )
    };
    if rc != 0 {
        return Err(VmError::NativeError(format!(
            "getnameinfo: {}",
            std::io::Error::from_raw_os_error(rc)
        )));
    }
    let trim = |b: Vec<u8>| {
        let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
        String::from_utf8_lossy(&b[..n]).into_owned()
    };
    Ok((trim(hostbuf), trim(servbuf)))
}

fn getnameinfo(args: &[Value]) -> VmResult<Value> {
    let host = crate::ops::string_arg(args, 0)?;
    let port = opt_int(args, 1, 0) as u16;
    let flags = opt_int(args, 2, 0) as i32;
    let (h, s) = getnameinfo_impl(&host, port, flags)?;
    Ok(new_list(vec![Value::string(h), Value::string(s)]))
}

fn inet_aton(args: &[Value]) -> VmResult<Value> {
    let text = crate::ops::string_arg(args, 0)?;
    let ip: Ipv4Addr = text.parse().map_err(|_| {
        VmError::ValueError(format!("invalid IPv4 address: {text}"))
    })?;
    Ok(Value::bytes(ip.octets().to_vec()))
}

fn inet_pton(args: &[Value]) -> VmResult<Value> {
    let family = crate::ops::int_arg(args, 0)? as c_int;
    let text = crate::ops::string_arg(args, 1)?;
    if family == AF_INET {
        let ip: Ipv4Addr = text.parse().map_err(|_| {
            VmError::ValueError(format!("invalid IPv4 address: {text}"))
        })?;
        Ok(Value::bytes(ip.octets().to_vec()))
    } else if family == AF_INET6 {
        let ip: Ipv6Addr = text.parse().map_err(|_| {
            VmError::ValueError(format!("invalid IPv6 address: {text}"))
        })?;
        Ok(Value::bytes(ip.octets().to_vec()))
    } else {
        Err(VmError::ValueError(format!(
            "inet_pton: unsupported family {family}"
        )))
    }
}

/// Coerce a packed address: raw bytes, a string of binary data, or a list of
/// byte values (the ergonomic Zen form).
fn octets_arg(args: &[Value], i: usize) -> VmResult<Vec<u8>> {
    let v = *args.get(i).ok_or(VmError::ArgumentCount {
        expected: i + 1,
        got: args.len(),
    })?;
    if let Some(b) = v.as_bytes() {
        return Ok(b.to_vec());
    }
    if let Some(s) = v.as_str() {
        return Ok(s.as_bytes().to_vec());
    }
    if let Some(list) = v.as_list() {
        let mut out = Vec::with_capacity(list.len());
        for item in list {
            let n = item
                .as_int()
                .ok_or_else(|| VmError::type_mismatch("integer byte value", *item))?;
            if !(0..=255).contains(&n) {
                return Err(VmError::ValueError(format!(
                    "byte value {n} out of range"
                )));
            }
            out.push(n as u8);
        }
        return Ok(out);
    }
    Err(VmError::type_mismatch("bytes or list of bytes", v))
}

fn inet_ntoa(args: &[Value]) -> VmResult<Value> {
    let b = octets_arg(args, 0)?;
    if b.len() != 4 {
        return Err(VmError::ValueError(format!(
            "inet_ntoa expects 4 bytes, got {}",
            b.len()
        )));
    }
    let ip = Ipv4Addr::new(b[0], b[1], b[2], b[3]);
    Ok(Value::string(ip.to_string()))
}

fn inet_ntop(args: &[Value]) -> VmResult<Value> {
    let family = crate::ops::int_arg(args, 0)? as c_int;
    let b = octets_arg(args, 1)?;
    if family == AF_INET {
        if b.len() != 4 {
            return Err(VmError::ValueError(
                "inet_ntop AF_INET expects 4 bytes".into(),
            ));
        }
        let ip = Ipv4Addr::new(b[0], b[1], b[2], b[3]);
        Ok(Value::string(ip.to_string()))
    } else if family == AF_INET6 {
        if b.len() != 16 {
            return Err(VmError::ValueError(
                "inet_ntop AF_INET6 expects 16 bytes".into(),
            ));
        }
        let mut octets = [0u8; 16];
        octets.copy_from_slice(&b);
        let ip = Ipv6Addr::from(octets);
        Ok(Value::string(ip.to_string()))
    } else {
        Err(VmError::ValueError(format!(
            "inet_ntop: unsupported family {family}"
        )))
    }
}

fn htons(args: &[Value]) -> VmResult<Value> {
    Ok(Value::int((crate::ops::int_arg(args, 0)? as u16).to_be() as i64 & 0xFFFF))
}
fn htonl(args: &[Value]) -> VmResult<Value> {
    Ok(Value::int((crate::ops::int_arg(args, 0)? as u32).to_be() as i64))
}
fn ntohs(args: &[Value]) -> VmResult<Value> {
    Ok(Value::int(u16::from_be(crate::ops::int_arg(args, 0)? as u16) as i64))
}
fn ntohl(args: &[Value]) -> VmResult<Value> {
    Ok(Value::int(u32::from_be(crate::ops::int_arg(args, 0)? as u32) as i64))
}

fn socketpair(args: &[Value]) -> VmResult<Value> {
    let family = opt_int(args, 0, AF_UNIX as i64) as c_int;
    let socktype = opt_int(args, 1, SOCK_STREAM as i64) as c_int;
    let proto = opt_int(args, 2, 0) as c_int;
    let mut fds = [0 as RawFd; 2];
    let rc = unsafe { libc::socketpair(family, socktype | libc::SOCK_CLOEXEC, proto, fds.as_mut_ptr()) };
    if rc != 0 {
        return Err(last_err("socketpair"));
    }
    let a = insert(Entry {
        fd: unsafe { OwnedFd::from_raw_fd(fds[0]) },
        family,
        socktype,
        proto,
        read_timeout: None,
        write_timeout: None,
    });
    let b = insert(Entry {
        fd: unsafe { OwnedFd::from_raw_fd(fds[1]) },
        family,
        socktype,
        proto,
        read_timeout: None,
        write_timeout: None,
    });
    Ok(new_list(vec![a, b]))
}

fn create_connection(args: &[Value]) -> VmResult<Value> {
    let host = crate::ops::string_arg(args, 0)?;
    let port = crate::ops::int_arg(args, 1)?;
    if !(0..=i64::from(u16::MAX)).contains(&port) {
        return Err(VmError::ValueError(format!("port {port} out of range")));
    }
    let timeout = args.get(2).and_then(|v| v.as_float());
    let addrs = resolve_addrs(AF_UNSPEC, &host, port as u16)?;
    let mut last = None;
    for addr in &addrs {
        let family = if addr.is_ipv4() { AF_INET } else { AF_INET6 };
        let handle = socket_create(&[
            Value::int_promote(family as i64),
            Value::int_promote(SOCK_STREAM as i64),
            Value::int_promote(0),
        ])?;
        let h = handle.as_int().unwrap_or(0) as u32;
        if let Some(t) = timeout {
            if let Err(e) = apply_timeout(h, Some(t)) {
                registry().open.remove(&h);
                return Err(e);
            }
        }
        let (storage, len) = to_storage(addr);
        let entry = lookup(h)?;
        let fd = {
            let g = entry.lock().unwrap_or_else(|e| e.into_inner());
            g.fd.as_raw_fd()
        };
        let rc = unsafe {
            libc::connect(fd, &storage as *const _ as *const libc::sockaddr, len)
        };
        if rc == 0 {
            return Ok(handle);
        }
        last = Some(std::io::Error::last_os_error());
        registry().open.remove(&h);
    }
    Err(VmError::NativeError(format!(
        "create_connection {host}:{port}: {}",
        last.map(|e| e.to_string()).unwrap_or_default()
    )))
}

// ── export table ────────────────────────────────────────────────────────────

fn c(name: &'static str, value: i64) -> HostExport {
    HostExport::Const {
        name,
        value: Value::int_promote(value),
    }
}

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        func("create", 500, 0, 3),
        func("socket", 500, 0, 3),
        func("open", 501, 2, 2),
        func("bind", 502, 2, 3),
        func("listen", 503, 1, 2),
        func("accept", 504, 1, 1),
        func("connect", 505, 2, 3),
        func("connect_ex", 506, 2, 3),
        func("send", 507, 2, 3),
        func("sendall", 508, 2, 3),
        func("recv", 509, 1, 2),
        func("recv_text", 510, 1, 2),
        func("sendto", 511, 3, 4),
        func("recvfrom", 512, 1, 2),
        func("setsockopt", 513, 4, 4),
        func("getsockopt", 514, 3, 4),
        func("getpeername", 515, 1, 1),
        func("getsockname", 516, 1, 1),
        func("shutdown", 517, 1, 2),
        func("close", 518, 1, 1),
        func("fileno", 519, 1, 1),
        func("dup", 520, 1, 1),
        func("settimeout", 521, 2, 2),
        func("gettimeout", 522, 1, 1),
        func("setblocking", 523, 2, 2),
        func("set_timeout", 524, 2, 2),
        func("gethostname", 525, 0, 0),
        func("gethostbyname", 526, 1, 1),
        func("gethostbyaddr", 527, 1, 1),
        func("getaddrinfo", 528, 1, 5),
        func("getnameinfo", 529, 1, 3),
        func("inet_aton", 530, 1, 1),
        func("inet_ntoa", 531, 1, 1),
        func("inet_pton", 532, 2, 2),
        func("inet_ntop", 533, 2, 2),
        func("htons", 534, 1, 1),
        func("htonl", 535, 1, 1),
        func("ntohs", 536, 1, 1),
        func("ntohl", 537, 1, 1),
        func("socketpair", 538, 0, 3),
        func("create_connection", 539, 2, 3),
        func("type", 540, 1, 1),
        // constants
        c("AF_UNSPEC", AF_UNSPEC as i64),
        c("AF_UNIX", AF_UNIX as i64),
        c("AF_INET", AF_INET as i64),
        c("AF_INET6", AF_INET6 as i64),
        c("SOCK_STREAM", SOCK_STREAM as i64),
        c("SOCK_DGRAM", SOCK_DGRAM as i64),
        c("SOCK_RAW", SOCK_RAW as i64),
        c("SOL_SOCKET", SOL_SOCKET as i64),
        c("SO_REUSEADDR", libc::SO_REUSEADDR as i64),
        c("SO_REUSEPORT", libc::SO_REUSEPORT as i64),
        c("SO_KEEPALIVE", libc::SO_KEEPALIVE as i64),
        c("SO_BROADCAST", libc::SO_BROADCAST as i64),
        c("SO_SNDBUF", libc::SO_SNDBUF as i64),
        c("SO_RCVBUF", libc::SO_RCVBUF as i64),
        c("SO_ERROR", libc::SO_ERROR as i64),
        c("SO_LINGER", libc::SO_LINGER as i64),
        c("SO_RCVTIMEO", libc::SO_RCVTIMEO as i64),
        c("SO_SNDTIMEO", libc::SO_SNDTIMEO as i64),
        c("IPPROTO_IP", IPPROTO_IP as i64),
        c("IPPROTO_ICMP", IPPROTO_ICMP as i64),
        c("IPPROTO_TCP", IPPROTO_TCP as i64),
        c("IPPROTO_UDP", IPPROTO_UDP as i64),
        c("IPPROTO_IPV6", IPPROTO_IPV6 as i64),
        c("IPPROTO_RAW", IPPROTO_RAW as i64),
        c("TCP_NODELAY", libc::TCP_NODELAY as i64),
        c("SHUT_RD", SHUT_RD as i64),
        c("SHUT_WR", SHUT_WR as i64),
        c("SHUT_RDWR", SHUT_RDWR as i64),
        HostExport::Const {
            name: "INADDR_ANY",
            value: Value::string("0.0.0.0"),
        },
        HostExport::Const {
            name: "INADDR_LOOPBACK",
            value: Value::string("127.0.0.1"),
        },
        HostExport::Const {
            name: "INADDR_BROADCAST",
            value: Value::string("255.255.255.255"),
        },
    ])
}

/// Route a native id in the socket block (500–599) to its handler.
pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    match id {
        500 => socket_create(args),
        501 => socket_open(args),
        502 => socket_bind(args),
        503 => socket_listen(args),
        504 => socket_accept(args),
        505 => socket_connect(args),
        506 => socket_connect_ex(args),
        507 => socket_send(args),
        508 => socket_sendall(args),
        509 => socket_recv(args),
        510 => socket_recv_text(args),
        511 => socket_sendto(args),
        512 => socket_recvfrom(args),
        513 => socket_setsockopt(args),
        514 => socket_getsockopt(args),
        515 => socket_getpeername(args),
        516 => socket_getsockname(args),
        517 => socket_shutdown(args),
        518 => socket_close(args),
        519 => socket_fileno(args),
        520 => socket_dup(args),
        521 => socket_settimeout(args),
        522 => socket_gettimeout(args),
        523 => socket_setblocking(args),
        524 => socket_set_timeout_ms(args),
        525 => gethostname(args),
        526 => gethostbyname(args),
        527 => gethostbyaddr(args),
        528 => getaddrinfo(args),
        529 => getnameinfo(args),
        530 => inet_aton(args),
        531 => inet_ntoa(args),
        532 => inet_pton(args),
        533 => inet_ntop(args),
        534 => htons(args),
        535 => htonl(args),
        536 => ntohs(args),
        537 => ntohl(args),
        538 => socketpair(args),
        539 => create_connection(args),
        540 => socket_type_of(args),
        _ => Err(VmError::NativeError(format!("unknown socket native id {id}"))),
    }
}
