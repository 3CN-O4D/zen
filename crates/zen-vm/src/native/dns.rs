//! `net.dns` — name resolution (M7).
//!
//! Python-parity surface for name resolution: `resolve` (getaddrinfo-shaped),
//! `gethostbyname`, `gethostbyname_ex`, `gethostbyaddr`, `getfqdn` and
//! `getnameinfo`, plus the `AI_*` constants. Forward resolution rides on
//! `std::net::ToSocketAddrs`, reverse on `libc::getnameinfo`. Calls are
//! blocking and return plain values — no handles, nothing to leak.

#![allow(unsafe_code)]

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::gc::{new_dict, new_list};
use crate::native::{func, host_module_mixed, HostExport};
use crate::value::{DictKey, Value};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};

// ── constants ────────────────────────────────────────────────────────────────

const AF_UNSPEC: libc::c_int = libc::AF_UNSPEC;
const AF_INET: libc::c_int = libc::AF_INET;
const AF_INET6: libc::c_int = libc::AF_INET6;
const SOCK_STREAM: libc::c_int = libc::SOCK_STREAM;
const SOCK_DGRAM: libc::c_int = libc::SOCK_DGRAM;

// glibc getaddrinfo flags (not exported by every `libc` build).
const AI_PASSIVE: i32 = 0x01;
const AI_CANONNAME: i32 = 0x02;
const AI_NUMERICHOST: i32 = 0x04;
const AI_V4MAPPED: i32 = 0x08;
const AI_ALL: i32 = 0x10;
const AI_ADDRCONFIG: i32 = 0x20;
const AI_NUMERICSERV: i32 = 0x0400;

// ── helpers ─────────────────────────────────────────────────────────────────

/// Resolve `host:port` to concrete socket addresses, honouring the family
/// filter (0 = any).
fn resolve_addrs(host: &str, port: u16, family: i32) -> VmResult<Vec<SocketAddr>> {
    if host.is_empty() {
        return Ok(match family {
            AF_INET6 => vec![SocketAddr::from((Ipv6Addr::UNSPECIFIED, port))],
            _ => vec![SocketAddr::from((Ipv4Addr::UNSPECIFIED, port))],
        });
    }
    let iter = (host, port)
        .to_socket_addrs()
        .map_err(|e| VmError::NativeError(format!("resolve {host}:{port}: {e}")))?;
    let mut out: Vec<SocketAddr> = Vec::new();
    for addr in iter {
        if family == AF_INET && !addr.is_ipv4() {
            continue;
        }
        if family == AF_INET6 && !addr.is_ipv6() {
            continue;
        }
        out.push(addr);
    }
    if out.is_empty() {
        return Err(VmError::NativeError(format!(
            "no address for {host}:{port} matching family {family}"
        )));
    }
    Ok(out)
}

fn family_of(addr: &SocketAddr) -> i32 {
    if addr.is_ipv6() {
        AF_INET6
    } else {
        AF_INET
    }
}

fn addr_storage_and_len(addr: &SocketAddr) -> (libc::sockaddr_storage, libc::socklen_t) {
    let mut storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let len = unsafe {
        match addr {
        SocketAddr::V4(v4) => {
            let sin = &mut *(&mut storage as *mut _ as *mut libc::sockaddr_in);
            sin.sin_family = AF_INET as libc::sa_family_t;
            sin.sin_port = v4.port().to_be();
            sin.sin_addr.s_addr = u32::from_ne_bytes(v4.ip().octets());
            std::mem::size_of::<libc::sockaddr_in>()
        }
        SocketAddr::V6(v6) => {
            let sin6 = &mut *(&mut storage as *mut _ as *mut libc::sockaddr_in6);
            sin6.sin6_family = AF_INET6 as libc::sa_family_t;
            sin6.sin6_port = v6.port().to_be();
            sin6.sin6_flowinfo = v6.flowinfo();
            sin6.sin6_addr.s6_addr = v6.ip().octets();
            sin6.sin6_scope_id = v6.scope_id();
            std::mem::size_of::<libc::sockaddr_in6>()
        }
    }};
    (storage, len as libc::socklen_t)
}

/// Reverse resolution of a numeric address; `None` when the system cannot name
/// it (no PTR record, request refused, …).
fn reverse_name(addr: &SocketAddr) -> Option<String> {
    let (storage, len) = addr_storage_and_len(addr);
    let mut buf = vec![0u8; libc::NI_MAXHOST as usize];
    let rc = unsafe {
        libc::getnameinfo(
            &storage as *const _ as *const libc::sockaddr,
            len,
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len() as libc::socklen_t,
            std::ptr::null_mut() as *mut libc::c_char,
            0,
            libc::NI_NAMEREQD,
        )
    };
    if rc != 0 {
        return None;
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf8_lossy(&buf[..n]).into_owned())
}

fn opt_int(args: &[Value], i: usize, default: i64) -> i64 {
    match args.get(i).copied() {
        Some(v) if !v.is_nil() => v.as_int().unwrap_or(default),
        _ => default,
    }
}

fn parse_ip(text: &str) -> Result<IpAddr, VmError> {
    text.parse()
        .map_err(|_| VmError::ValueError(format!("net.dns: not an IP address: {text}")))
}

fn gethostname_str() -> String {
    let mut buf = vec![0u8; 256];
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if rc != 0 {
        return "localhost".into();
    }
    let n = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

// ── natives ─────────────────────────────────────────────────────────────────

/// `net.dns.resolve(host, port?, family?, socktype?, proto?, flags?)`
/// → a list of getaddrinfo-style records:
/// `{family, type, proto, sockaddr: [host, port], canonicalname}`.
fn resolve(args: &[Value]) -> VmResult<Value> {
    let host = crate::ops::string_arg(args, 0)?;
    let port = opt_int(args, 1, 0).clamp(0, i64::from(u16::MAX)) as u16;
    let family = opt_int(args, 2, i64::from(AF_UNSPEC)) as i32;
    let socktype = opt_int(args, 3, 0) as i32;
    let proto = opt_int(args, 4, 0) as i64;
    let flags = opt_int(args, 5, 0) as i32;
    let addrs = resolve_addrs(&host, port, family)?;
    let mut out: Vec<Value> = Vec::with_capacity(addrs.len());
    for addr in &addrs {
        let mut rec = indexmap::IndexMap::new();
        rec.insert(DictKey(Value::string("family")),
            Value::int_promote(family_of(&addr) as i64),
        );
        let ty = if socktype != 0 { socktype } else { SOCK_STREAM };
        rec.insert(DictKey(Value::string("type")), Value::int_promote(ty as i64));
        rec.insert(DictKey(Value::string("proto")), Value::int_promote(proto));
        rec.insert(DictKey(Value::string("sockaddr")),
            new_list(vec![
                Value::string(addr.ip().to_string()),
                Value::int_promote(addr.port() as i64),
            ]),
        );
        let canonical = if flags & AI_CANONNAME != 0 {
            host.clone()
        } else {
            String::new()
        };
        rec.insert(DictKey(Value::string("canonicalname")), Value::string(canonical));
        out.push(new_dict(rec));
    }
    Ok(new_list(out))
}

/// `net.dns.gethostbyname(name)` → first IPv4 dotted string.
fn gethostbyname(args: &[Value]) -> VmResult<Value> {
    let name = crate::ops::string_arg(args, 0)?;
    let addrs = resolve_addrs(&name, 0, AF_INET)?;
    match addrs.first() {
        Some(SocketAddr::V4(v4)) => Ok(Value::string(v4.ip().to_string())),
        _ => Err(VmError::NativeError(format!(
            "gethostbyname: no IPv4 address for {name}"
        ))),
    }
}

/// `net.dns.gethostbyname_ex(name)` → `[hostname, aliases, addresses]`.
fn gethostbyname_ex(args: &[Value]) -> VmResult<Value> {
    let name = crate::ops::string_arg(args, 0)?;
    let addrs = resolve_addrs(&name, 0, AF_INET)?;
    let canonical = addrs
        .last()
        .and_then(reverse_name)
        .unwrap_or_else(|| name.clone());
    let v4s: Vec<Value> = addrs
        .iter()
        .filter_map(|a| match a {
            SocketAddr::V4(v4) => Some(Value::string(v4.ip().to_string())),
            _ => None,
        })
        .collect();
    if v4s.is_empty() {
        return Err(VmError::NativeError(format!(
            "gethostbyname_ex: no IPv4 address for {name}"
        )));
    }
    Ok(new_list(vec![
        Value::string(canonical),
        new_list(vec![]),
        new_list(v4s),
    ]))
}

/// `net.dns.gethostbyaddr(addr)` → `[hostname, aliases, addresses]`.
fn gethostbyaddr(args: &[Value]) -> VmResult<Value> {
    let text = crate::ops::string_arg(args, 0)?;
    let ip = parse_ip(&text)?;
    let addr = SocketAddr::from((ip, 0));
    let host = reverse_name(&addr).unwrap_or_else(|| text.clone());
    Ok(new_list(vec![
        Value::string(host),
        new_list(vec![]),
        new_list(vec![Value::string(text)]),
    ]))
}

/// `net.dns.getfqdn(name?)` → fully-qualified domain name for `name`, or the
/// local host when omitted.
fn getfqdn(args: &[Value]) -> VmResult<Value> {
    let name = args
        .first()
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(gethostname_str);
    if name.is_empty() {
        return Err(VmError::ValueError("getfqdn: empty hostname".into()));
    }
    let addrs = resolve_addrs(&name, 0, 0).unwrap_or_default();
    for addr in &addrs {
        if let Some(canon) = reverse_name(addr) {
            if !canon.is_empty() {
                return Ok(Value::string(canon));
            }
        }
    }
    Ok(Value::string(name))
}

// ── export table ────────────────────────────────────────────────────────────

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        func("resolve", 600, 1, 6),
        func("gethostbyname", 601, 1, 1),
        func("gethostbyname_ex", 602, 1, 1),
        func("gethostbyaddr", 603, 1, 1),
        func("getfqdn", 604, 0, 1),
        func("getnameinfo", 605, 2, 3),
        HostExport::Const {
            name: "AF_UNSPEC",
            value: Value::int_promote(AF_UNSPEC as i64),
        },
        HostExport::Const {
            name: "AF_INET",
            value: Value::int_promote(AF_INET as i64),
        },
        HostExport::Const {
            name: "AF_INET6",
            value: Value::int_promote(AF_INET6 as i64),
        },
        HostExport::Const {
            name: "SOCK_STREAM",
            value: Value::int_promote(SOCK_STREAM as i64),
        },
        HostExport::Const {
            name: "SOCK_DGRAM",
            value: Value::int_promote(SOCK_DGRAM as i64),
        },
        HostExport::Const {
            name: "AI_PASSIVE",
            value: Value::int_promote(AI_PASSIVE as i64),
        },
        HostExport::Const {
            name: "AI_CANONNAME",
            value: Value::int_promote(AI_CANONNAME as i64),
        },
        HostExport::Const {
            name: "AI_NUMERICHOST",
            value: Value::int_promote(AI_NUMERICHOST as i64),
        },
        HostExport::Const {
            name: "AI_V4MAPPED",
            value: Value::int_promote(AI_V4MAPPED as i64),
        },
        HostExport::Const {
            name: "AI_ALL",
            value: Value::int_promote(AI_ALL as i64),
        },
        HostExport::Const {
            name: "AI_ADDRCONFIG",
            value: Value::int_promote(AI_ADDRCONFIG as i64),
        },
        HostExport::Const {
            name: "AI_NUMERICSERV",
            value: Value::int_promote(AI_NUMERICSERV as i64),
        },
    ])
}

/// Route ids in the `net.dns` block (600–619) to their handler.
pub fn dispatch(id: u16, args: &[Value]) -> VmResult<Value> {
    match id {
        600 => resolve(args),
        601 => gethostbyname(args),
        602 => gethostbyname_ex(args),
        603 => gethostbyaddr(args),
        604 => getfqdn(args),
        605 => crate::native::socket::dispatch(529, args),
        _ => Err(VmError::NativeError(format!(
            "unknown net.dns native id {id}"
        ))),
    }
}