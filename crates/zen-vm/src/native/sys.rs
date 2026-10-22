//! Zen `sys` module — raw POSIX/Linux system calls, memory access and
//! Python-style process introspection.
//!
//! Every function here is a thin wrapper over the `libc` crate. Pointers are
//! passed around as plain numbers (`sys.mmap(..)` returns an address, `sys.peek*`
//! takes one), so pointer arithmetic in Zen is ordinary integer arithmetic.
//! All failures raise catchable [`VmError`]s.
//!
//! The module owns the `F_*`, `O_*`, `PROT_*`, `MAP_*`, `SO_*`, `AF_*`, `SOCK_*`,
//! `IPPROTO_*`, `CLOCK_*`, `POLL*`, `SC_*`, `RLIMIT_*`, `SEEK_*`, `SHUT_*`,
//! `SIG*` and `WNOHANG`-family constants, in addition to the runtime members a
//! Python-style `sys` module exposes (`argv`, `exit`, `getpid`, `uname`, …).

#![allow(unsafe_code)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_long, c_ulong, c_void};

use indexmap::IndexMap;

use crate::bytecode::ModuleUnit;
use crate::errors::{VmError, VmResult};
use crate::gc::{new_dict, new_list};
use crate::native::{host_module_mixed, HostExport};
use crate::ops::string_arg;
use crate::value::{DictKey, Value};

// ── id map ──────────────────────────────────────────────────────────────────
//
// sys owns a contiguous block starting at 400. The dispatch arms and the module
// export table both key off these so a script can never reach an id the module
// does not implement (returns `None` → "unknown native").

pub const ID_SCAL: u16 = 400;
pub const ID_OPEN: u16 = 401;
pub const ID_CLOSE: u16 = 402;
pub const ID_READ: u16 = 403;
pub const ID_READ_STR: u16 = 404;
pub const ID_WRITE: u16 = 405;
pub const ID_WRITE_BYTES: u16 = 406;
pub const ID_LSEEK: u16 = 407;
pub const ID_DUP: u16 = 408;
pub const ID_DUP2: u16 = 409;
pub const ID_PIPE: u16 = 410;
pub const ID_FORK: u16 = 411;
pub const ID_EXEC: u16 = 412;
pub const ID_KILL: u16 = 413;
pub const ID_GETPID: u16 = 414;
pub const ID_GETPPID: u16 = 415;
pub const ID_GETUID: u16 = 416;
pub const ID_GETEUID: u16 = 417;
pub const ID_GETGID: u16 = 418;
pub const ID_GETEGID: u16 = 419;
pub const ID_UMASK: u16 = 420;
pub const ID_STAT: u16 = 421;
pub const ID_LSTAT: u16 = 422;
pub const ID_FSTAT: u16 = 423;
pub const ID_ACCESS: u16 = 424;
pub const ID_CHMOD: u16 = 425;
pub const ID_CHOWN: u16 = 426;
pub const ID_FCHOWN: u16 = 427;
pub const ID_SYMLINK: u16 = 428;
pub const ID_READLINK: u16 = 429;
pub const ID_LINK: u16 = 430;
pub const ID_TRUNCATE: u16 = 431;
pub const ID_FTRUNCATE: u16 = 432;
pub const ID_FSYNC: u16 = 433;
pub const ID_FDATASYNC: u16 = 434;
pub const ID_UNLINK: u16 = 435;
pub const ID_RMDIR: u16 = 436;
pub const ID_MKFIFO: u16 = 437;
pub const ID_MKNOD: u16 = 438;
pub const ID_WAITPID: u16 = 439;
pub const ID_WIFEXITED: u16 = 440;
pub const ID_WIFSIGNALED: u16 = 441;
pub const ID_WIFSTOPPED: u16 = 442;
pub const ID_WEXITSTATUS: u16 = 443;
pub const ID_WTERMSIG: u16 = 444;
pub const ID_NANOSLEEP: u16 = 445;
pub const ID_MMAP: u16 = 446;
pub const ID_MUNMAP: u16 = 447;
pub const ID_MPROTECT: u16 = 448;
pub const ID_MREMAP: u16 = 449;
pub const ID_MLOCK: u16 = 450;
pub const ID_MUNLOCK: u16 = 451;
pub const ID_PEEK8: u16 = 452;
pub const ID_PEEK16: u16 = 453;
pub const ID_PEEK32: u16 = 454;
pub const ID_PEEK64: u16 = 455;
pub const ID_POKE8: u16 = 456;
pub const ID_POKE16: u16 = 457;
pub const ID_POKE32: u16 = 458;
pub const ID_POKE64: u16 = 459;
pub const ID_MEM_READ: u16 = 460;
pub const ID_MEM_WRITE: u16 = 461;
pub const ID_IOCTL: u16 = 462;
pub const ID_FCNTL: u16 = 463;
pub const ID_POLL: u16 = 464;
pub const ID_SOCKET: u16 = 465;
pub const ID_SOCKETPAIR: u16 = 466;
pub const ID_BIND: u16 = 467;
pub const ID_LISTEN: u16 = 468;
pub const ID_ACCEPT: u16 = 469;
pub const ID_ACCEPT4: u16 = 470;
pub const ID_CONNECT: u16 = 471;
pub const ID_GETSOCKNAME: u16 = 472;
pub const ID_GETPEERNAME: u16 = 473;
pub const ID_SEND: u16 = 474;
pub const ID_SENDTO: u16 = 475;
pub const ID_RECV: u16 = 476;
pub const ID_RECVFROM: u16 = 477;
pub const ID_SHUTDOWN: u16 = 478;
pub const ID_SETSOCKOPT: u16 = 479;
pub const ID_GETSOCKOPT: u16 = 480;
pub const ID_GETHOSTNAME: u16 = 481;
pub const ID_UNAME: u16 = 482;
pub const ID_CLOCK_GETTIME: u16 = 483;
pub const ID_GETRLIMIT: u16 = 484;
pub const ID_SETRLIMIT: u16 = 485;
pub const ID_SYSCONF: u16 = 486;
pub const ID_TIMES: u16 = 487;
pub const ID_STRERROR: u16 = 488;
pub const ID_ERRNO: u16 = 489;

// Python-style runtime helpers.
pub const ID_EXIT: u16 = 490;
pub const ID_GETSIZEOF: u16 = 491;
pub const ID_GETRECURSIONLIMIT: u16 = 492;
pub const ID_GETDEFAULTENCODING: u16 = 493;
pub const ID_GETFILESYSTEMENCODING: u16 = 494;
pub const ID_INTERN: u16 = 495;
pub const ID_CALL_TRACING: u16 = 496;
pub const ID_SETTRACE: u16 = 497;
pub const ID_GETTRACE: u16 = 498;
pub const ID_MODULES: u16 = 499;

// ── numeric helpers (accept both int `Value`s and float pointers) ───────────

fn any_i64(v: Value) -> Option<i64> {
    if let Some(i) = v.as_int() {
        Some(i)
    } else if let Some(f) = v.as_float() {
        Some(f as i64)
    } else {
        None
    }
}

fn any_u64(v: Value) -> Option<u64> {
    any_i64(v).map(|i| i as u64)
}

fn arg_at(args: &[Value], i: usize) -> VmResult<Value> {
    args.get(i).copied().ok_or(VmError::ArgumentCount {
        expected: i + 1,
        got: args.len(),
    })
}

fn num_i64(args: &[Value], i: usize, what: &str) -> VmResult<i64> {
    let v = arg_at(args, i)?;
    any_i64(v).ok_or_else(|| VmError::type_mismatch(&format!("sys.{what} number"), v))
}

fn num_u64(args: &[Value], i: usize, what: &str) -> VmResult<u64> {
    let v = arg_at(args, i)?;
    any_u64(v).ok_or_else(|| VmError::type_mismatch(&format!("sys.{what} number"), v))
}

fn opt_num_i64(args: &[Value], i: usize, default: i64) -> i64 {
    match args.get(i).copied() {
        Some(v) if !v.is_nil() => any_i64(v).unwrap_or(default),
        _ => default,
    }
}

fn opt_num_u64(args: &[Value], i: usize, default: u64) -> u64 {
    match args.get(i).copied() {
        Some(v) if !v.is_nil() => any_u64(v).unwrap_or(default),
        _ => default,
    }
}

fn fd_arg(args: &[Value], i: usize) -> VmResult<c_int> {
    Ok(num_i64(args, i, "fd")? as c_int)
}

fn path_cstr(args: &[Value], i: usize, what: &str) -> VmResult<CString> {
    let s = string_arg(args, i)?;
    CString::new(s).map_err(|_| VmError::ValueError(format!("sys.{what}: path contains a NUL byte")))
}

fn mode_at(args: &[Value], i: usize, default: u32) -> VmResult<libc::mode_t> {
    if args.get(i).map(|v| v.is_nil()).unwrap_or(true) {
        Ok(default as libc::mode_t)
    } else {
        Ok(num_i64(args, i, "mode")? as libc::mode_t)
    }
}

// ── errno / hex helpers ─────────────────────────────────────────────────────

fn errno_now() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

fn errno_message(e: i32) -> String {
    let p = unsafe { libc::strerror(e) };
    if p.is_null() {
        format!("errno {e}")
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

/// Format a failing syscall as a Zen error message.
fn syserr(what: &str) -> VmError {
    let e = errno_now();
    VmError::NativeError(format!(
        "sys.{what}: {} (errno {e})",
        errno_message(e)
    ))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len() / 2);
    let mut i = 0;
    while i < b.len() {
        let hi = (b[i] as char).to_digit(16)?;
        let lo = (b[i + 1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
        i += 2;
    }
    Some(out)
}

fn hex_data(args: &[Value], i: usize, what: &str) -> VmResult<Vec<u8>> {
    let s = string_arg(args, i)?;
    hex_decode(&s).ok_or_else(|| VmError::ValueError(format!("sys.{what}: invalid hex data")))
}

/// `null` map used for the empty `sys.modules()` result.
fn empty_dict() -> IndexMap<DictKey, Value> {
    IndexMap::new()
}

// ── stat ────────────────────────────────────────────────────────────────────

fn stat_dict(s: &libc::stat) -> Value {
    let mut m = IndexMap::new();
    m.insert(DictKey(Value::string("dev")), Value::int_promote(s.st_dev as i64));
    m.insert(DictKey(Value::string("ino")), Value::int_promote(s.st_ino as i64));
    m.insert(DictKey(Value::string("mode")), Value::int_promote(s.st_mode as i64));
    m.insert(DictKey(Value::string("nlink")), Value::int_promote(s.st_nlink as i64));
    m.insert(DictKey(Value::string("uid")), Value::int_promote(s.st_uid as i64));
    m.insert(DictKey(Value::string("gid")), Value::int_promote(s.st_gid as i64));
    m.insert(DictKey(Value::string("rdev")), Value::int_promote(s.st_rdev as i64));
    m.insert(DictKey(Value::string("size")), Value::int_promote(s.st_size as i64));
    m.insert(DictKey(Value::string("blksize")), Value::int_promote(s.st_blksize as i64));
    m.insert(DictKey(Value::string("blocks")), Value::int_promote(s.st_blocks as i64));
    m.insert(DictKey(Value::string("atime")), Value::int_promote(s.st_atime as i64));
    m.insert(DictKey(Value::string("mtime")), Value::int_promote(s.st_mtime as i64));
    m.insert(DictKey(Value::string("ctime")), Value::int_promote(s.st_ctime as i64));
    new_dict(m)
}

fn do_stat(path: &str, lstat: bool) -> VmResult<Value> {
    let c = CString::new(path)
        .map_err(|_| VmError::ValueError("sys.stat: path contains a NUL byte".into()))?;
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let r = unsafe {
        if lstat {
            libc::lstat(c.as_ptr(), &mut st)
        } else {
            libc::stat(c.as_ptr(), &mut st)
        }
    };
    if r == -1 {
        return Err(syserr(if lstat { "lstat" } else { "stat" }));
    }
    Ok(stat_dict(&st))
}

// ── socket helpers ──────────────────────────────────────────────────────────

fn optval_bytes(args: &[Value], i: usize, what: &str) -> VmResult<Vec<u8>> {
    let v = arg_at(args, i)?;
    if let Some(s) = v.as_str() {
        hex_decode(s).ok_or_else(|| VmError::ValueError(format!("sys.{what}: invalid hex data")))
    } else if let Some(n) = any_i64(v) {
        Ok((n as c_int).to_ne_bytes().to_vec())
    } else if let Some(b) = v.as_bool() {
        Ok((b as c_int).to_ne_bytes().to_vec())
    } else {
        Err(VmError::type_mismatch("number or hex string", v))
    }
}

fn sockaddr_hex(fd: c_int, peer: bool, what: &str) -> VmResult<Value> {
    let mut storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    let rc = unsafe {
        let p = &mut storage as *mut libc::sockaddr_storage as *mut libc::sockaddr;
        if peer {
            libc::getpeername(fd, p, &mut len)
        } else {
            libc::getsockname(fd, p, &mut len)
        }
    };
    if rc == -1 {
        return Err(syserr(what));
    }
    let bytes = unsafe {
        std::slice::from_raw_parts(&storage as *const libc::sockaddr_storage as *const u8, len as usize)
    };
    Ok(Value::string(hex_encode(bytes)))
}

// ── dispatch ────────────────────────────────────────────────────────────────

pub fn dispatch(id: u16, args: &[Value], output: &mut String) -> VmResult<Option<Value>> {
    let v = match id {
        // raw syscalls
        ID_SCAL => sys_syscall(args)?,
        ID_OPEN => {
            let path = path_cstr(args, 0, "open")?;
            let flags = num_i64(args, 1, "open")? as c_int;
            let mode = mode_at(args, 2, 0o644)?;
            let fd = unsafe { libc::open(path.as_ptr(), flags, mode) };
            if fd < 0 {
                return Err(syserr("open"));
            }
            Value::int_promote(fd as i64)
        }
        ID_CLOSE => {
            let fd = fd_arg(args, 0)?;
            if unsafe { libc::close(fd) } == -1 {
                return Err(syserr("close"));
            }
            Value::bool(true)
        }
        ID_READ => {
            let fd = fd_arg(args, 0)?;
            let count = num_u64(args, 1, "read")? as usize;
            let mut buf = vec![0u8; count];
            let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut c_void, count) };
            if n < 0 {
                return Err(syserr("read"));
            }
            Value::string(hex_encode(&buf[..n as usize]))
        }
        ID_READ_STR => {
            let fd = fd_arg(args, 0)?;
            let count = num_u64(args, 1, "read_str")? as usize;
            let mut buf = vec![0u8; count];
            let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut c_void, count) };
            if n < 0 {
                return Err(syserr("read_str"));
            }
            Value::string(String::from_utf8_lossy(&buf[..n as usize]).into_owned())
        }
        ID_WRITE => {
            let fd = fd_arg(args, 0)?;
            let text = string_arg(args, 1)?;
            let n = unsafe { libc::write(fd, text.as_ptr() as *const c_void, text.len()) };
            if n < 0 {
                return Err(syserr("write"));
            }
            Value::int_promote(n as i64)
        }
        ID_WRITE_BYTES => {
            let fd = fd_arg(args, 0)?;
            let bytes = hex_data(args, 1, "write_bytes")?;
            let n = unsafe { libc::write(fd, bytes.as_ptr() as *const c_void, bytes.len()) };
            if n < 0 {
                return Err(syserr("write_bytes"));
            }
            Value::int_promote(n as i64)
        }
        ID_LSEEK => {
            let fd = fd_arg(args, 0)?;
            let offset = num_i64(args, 1, "lseek")? as libc::off_t;
            let whence = num_i64(args, 2, "lseek")? as c_int;
            let r = unsafe { libc::lseek(fd, offset, whence) };
            if r == -1 {
                return Err(syserr("lseek"));
            }
            Value::int_promote(r as i64)
        }
        ID_DUP => {
            let fd = fd_arg(args, 0)?;
            let r = unsafe { libc::dup(fd) };
            if r == -1 {
                return Err(syserr("dup"));
            }
            Value::int_promote(r as i64)
        }
        ID_DUP2 => {
            let old = fd_arg(args, 0)?;
            let new = fd_arg(args, 1)?;
            let r = unsafe { libc::dup2(old, new) };
            if r == -1 {
                return Err(syserr("dup2"));
            }
            Value::int_promote(r as i64)
        }
        ID_PIPE => {
            let mut fds = [0 as c_int; 2];
            if unsafe { libc::pipe(fds.as_mut_ptr()) } == -1 {
                return Err(syserr("pipe"));
            }
            new_list(vec![
                Value::int_promote(fds[0] as i64),
                Value::int_promote(fds[1] as i64),
            ])
        }
        ID_FORK => {
            let pid = unsafe { libc::fork() };
            if pid < 0 {
                return Err(syserr("fork"));
            }
            Value::int_promote(pid as i64)
        }
        ID_EXEC => sys_exec(args)?,
        ID_KILL => {
            let pid = num_i64(args, 0, "kill")? as libc::pid_t;
            let sig = num_i64(args, 1, "kill")? as c_int;
            if unsafe { libc::kill(pid, sig) } == -1 {
                return Err(syserr("kill"));
            }
            Value::bool(true)
        }
        ID_GETPID => Value::int_promote(unsafe { libc::getpid() } as i64),
        ID_GETPPID => Value::int_promote(unsafe { libc::getppid() } as i64),
        ID_GETUID => Value::int_promote(unsafe { libc::getuid() } as i64),
        ID_GETEUID => Value::int_promote(unsafe { libc::geteuid() } as i64),
        ID_GETGID => Value::int_promote(unsafe { libc::getgid() } as i64),
        ID_GETEGID => Value::int_promote(unsafe { libc::getegid() } as i64),
        ID_UMASK => {
            let mask = if args.first().map(|v| !v.is_nil()).unwrap_or(true) {
                num_i64(args, 0, "umask")? as libc::mode_t
            } else {
                0o022
            };
            let prev = unsafe { libc::umask(mask) };
            Value::int_promote(prev as i64)
        }
        ID_STAT => do_stat(&string_arg(args, 0)?, false)?,
        ID_LSTAT => do_stat(&string_arg(args, 0)?, true)?,
        ID_FSTAT => {
            let fd = fd_arg(args, 0)?;
            let mut st: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstat(fd, &mut st) } == -1 {
                return Err(syserr("fstat"));
            }
            stat_dict(&st)
        }
        ID_ACCESS => {
            let path = path_cstr(args, 0, "access")?;
            let mode = num_i64(args, 1, "access")? as c_int;
            if unsafe { libc::access(path.as_ptr(), mode) } == -1 {
                return Err(syserr("access"));
            }
            Value::bool(true)
        }
        ID_CHMOD => {
            let path = path_cstr(args, 0, "chmod")?;
            let mode = mode_at(args, 1, 0o644)?;
            if unsafe { libc::chmod(path.as_ptr(), mode) } == -1 {
                return Err(syserr("chmod"));
            }
            Value::bool(true)
        }
        ID_CHOWN => {
            let path = path_cstr(args, 0, "chown")?;
            let uid = num_i64(args, 1, "chown")? as libc::uid_t;
            let gid = num_i64(args, 2, "chown")? as libc::gid_t;
            if unsafe { libc::chown(path.as_ptr(), uid, gid) } == -1 {
                return Err(syserr("chown"));
            }
            Value::bool(true)
        }
        ID_FCHOWN => {
            let fd = fd_arg(args, 0)?;
            let uid = num_i64(args, 1, "fchown")? as libc::uid_t;
            let gid = num_i64(args, 2, "fchown")? as libc::gid_t;
            if unsafe { libc::fchown(fd, uid, gid) } == -1 {
                return Err(syserr("fchown"));
            }
            Value::bool(true)
        }
        ID_SYMLINK => {
            let target = path_cstr(args, 0, "symlink")?;
            let link = path_cstr(args, 1, "symlink")?;
            if unsafe { libc::symlink(target.as_ptr(), link.as_ptr()) } == -1 {
                return Err(syserr("symlink"));
            }
            Value::bool(true)
        }
        ID_READLINK => {
            let path = path_cstr(args, 0, "readlink")?;
            let mut buf = vec![0u8; 4096];
            let n = unsafe { libc::readlink(path.as_ptr(), buf.as_mut_ptr() as *mut c_char, 4095) };
            if n < 0 {
                return Err(syserr("readlink"));
            }
            buf.truncate(n as usize);
            Value::string(String::from_utf8_lossy(&buf).into_owned())
        }
        ID_LINK => {
            let old = path_cstr(args, 0, "link")?;
            let new = path_cstr(args, 1, "link")?;
            if unsafe { libc::link(old.as_ptr(), new.as_ptr()) } == -1 {
                return Err(syserr("link"));
            }
            Value::bool(true)
        }
        ID_TRUNCATE => {
            let path = path_cstr(args, 0, "truncate")?;
            let len = num_i64(args, 1, "truncate")? as libc::off_t;
            if unsafe { libc::truncate(path.as_ptr(), len) } == -1 {
                return Err(syserr("truncate"));
            }
            Value::bool(true)
        }
        ID_FTRUNCATE => {
            let fd = fd_arg(args, 0)?;
            let len = num_i64(args, 1, "ftruncate")? as libc::off_t;
            if unsafe { libc::ftruncate(fd, len) } == -1 {
                return Err(syserr("ftruncate"));
            }
            Value::bool(true)
        }
        ID_FSYNC => {
            let fd = fd_arg(args, 0)?;
            if unsafe { libc::fsync(fd) } == -1 {
                return Err(syserr("fsync"));
            }
            Value::bool(true)
        }
        ID_FDATASYNC => {
            let fd = fd_arg(args, 0)?;
            if unsafe { libc::fdatasync(fd) } == -1 {
                return Err(syserr("fdatasync"));
            }
            Value::bool(true)
        }
        ID_UNLINK => {
            let path = path_cstr(args, 0, "unlink")?;
            if unsafe { libc::unlink(path.as_ptr()) } == -1 {
                return Err(syserr("unlink"));
            }
            Value::bool(true)
        }
        ID_RMDIR => {
            let path = path_cstr(args, 0, "rmdir")?;
            if unsafe { libc::rmdir(path.as_ptr()) } == -1 {
                return Err(syserr("rmdir"));
            }
            Value::bool(true)
        }
        ID_MKFIFO => {
            let path = path_cstr(args, 0, "mkfifo")?;
            let mode = mode_at(args, 1, 0o644)?;
            if unsafe { libc::mkfifo(path.as_ptr(), mode) } == -1 {
                return Err(syserr("mkfifo"));
            }
            Value::bool(true)
        }
        ID_MKNOD => {
            let path = path_cstr(args, 0, "mknod")?;
            let mode = mode_at(args, 1, 0o600)?;
            let dev = num_i64(args, 2, "mknod")? as libc::dev_t;
            if unsafe { libc::mknod(path.as_ptr(), mode, dev) } == -1 {
                return Err(syserr("mknod"));
            }
            Value::bool(true)
        }
        ID_WAITPID => {
            let pid = num_i64(args, 0, "waitpid")? as libc::pid_t;
            let options = if args.get(1).map(|v| !v.is_nil()).unwrap_or(false) {
                num_i64(args, 1, "waitpid")? as c_int
            } else {
                0
            };
            let mut status: c_int = 0;
            let r = unsafe { libc::waitpid(pid, &mut status, options) };
            if r == -1 {
                return Err(syserr("waitpid"));
            }
            new_list(vec![Value::int_promote(r as i64), Value::int_promote(status as i64)])
        }
        ID_WIFEXITED => Value::bool(libc::WIFEXITED(num_i64(args, 0, "wifexited")? as c_int)),
        ID_WIFSIGNALED => Value::bool(libc::WIFSIGNALED(num_i64(args, 0, "wifsignaled")? as c_int)),
        ID_WIFSTOPPED => Value::bool(libc::WIFSTOPPED(num_i64(args, 0, "wifstopped")? as c_int)),
        ID_WEXITSTATUS => Value::int_promote(libc::WEXITSTATUS(num_i64(args, 0, "wexitstatus")? as c_int) as i64),
        ID_WTERMSIG => Value::int_promote(libc::WTERMSIG(num_i64(args, 0, "wtermsig")? as c_int) as i64),
        ID_NANOSLEEP => {
            let secs_v = arg_at(args, 0)?;
            let secs = secs_v
                .as_int()
                .map(|i| i as f64)
                .or_else(|| secs_v.as_float())
                .ok_or_else(|| VmError::type_mismatch("number", secs_v))?;
            let req = libc::timespec {
                tv_sec: secs as libc::time_t,
                tv_nsec: ((secs - (secs as i64 as f64)) * 1e9) as _,
            };
            let mut rem: libc::timespec = unsafe { std::mem::zeroed() };
            if unsafe { libc::nanosleep(&req, &mut rem) } == -1 {
                return Err(syserr("nanosleep"));
            }
            Value::nil()
        }
        ID_MMAP => {
            let len = num_u64(args, 0, "mmap")? as usize;
            let prot = num_i64(args, 1, "mmap")? as c_int;
            let flags = num_i64(args, 2, "mmap")? as c_int;
            let fd = num_i64(args, 3, "mmap")? as c_int;
            let offset = num_i64(args, 4, "mmap")? as libc::off_t;
            let ptr = unsafe { libc::mmap(std::ptr::null_mut(), len, prot, flags, fd, offset) };
            if ptr == libc::MAP_FAILED {
                return Err(syserr("mmap"));
            }
            Value::int_promote(ptr as usize as i64)
        }
        ID_MUNMAP => {
            let addr = num_u64(args, 0, "munmap")? as *mut c_void;
            let len = num_u64(args, 1, "munmap")? as usize;
            if unsafe { libc::munmap(addr, len) } == -1 {
                return Err(syserr("munmap"));
            }
            Value::bool(true)
        }
        ID_MPROTECT => {
            let addr = num_u64(args, 0, "mprotect")? as *mut c_void;
            let len = num_u64(args, 1, "mprotect")? as usize;
            let prot = num_i64(args, 2, "mprotect")? as c_int;
            if unsafe { libc::mprotect(addr, len, prot) } == -1 {
                return Err(syserr("mprotect"));
            }
            Value::bool(true)
        }
        ID_MREMAP => {
            let addr = num_u64(args, 0, "mremap")? as *mut c_void;
            let old_size = num_u64(args, 1, "mremap")? as usize;
            let new_size = num_u64(args, 2, "mremap")? as usize;
            let flags = num_i64(args, 3, "mremap")? as c_int;
            let ptr = unsafe { libc::mremap(addr, old_size, new_size, flags) };
            if ptr == libc::MAP_FAILED {
                return Err(syserr("mremap"));
            }
            Value::int_promote(ptr as usize as i64)
        }
        ID_MLOCK => {
            let addr = num_u64(args, 0, "mlock")? as *const c_void;
            let len = num_u64(args, 1, "mlock")? as usize;
            if unsafe { libc::mlock(addr, len) } == -1 {
                return Err(syserr("mlock"));
            }
            Value::bool(true)
        }
        ID_MUNLOCK => {
            let addr = num_u64(args, 0, "munlock")? as *const c_void;
            let len = num_u64(args, 1, "munlock")? as usize;
            if unsafe { libc::munlock(addr, len) } == -1 {
                return Err(syserr("munlock"));
            }
            Value::bool(true)
        }
        ID_PEEK8 => Value::int_promote(unsafe { std::ptr::read_unaligned(num_u64(args, 0, "peek8")? as *const u8) } as i64),
        ID_PEEK16 => Value::int_promote(unsafe { std::ptr::read_unaligned(num_u64(args, 0, "peek16")? as *const u16) } as i64),
        ID_PEEK32 => Value::int_promote(unsafe { std::ptr::read_unaligned(num_u64(args, 0, "peek32")? as *const u32) } as i64),
        ID_PEEK64 => Value::int_promote(unsafe { std::ptr::read_unaligned(num_u64(args, 0, "peek64")? as *const u64) } as i64),
        ID_POKE8 => {
            let addr = num_u64(args, 0, "poke8")? as *mut u8;
            let val = num_u64(args, 1, "poke8")? as u8;
            unsafe { std::ptr::write_unaligned(addr, val) };
            Value::nil()
        }
        ID_POKE16 => {
            let addr = num_u64(args, 0, "poke16")? as *mut u16;
            let val = num_u64(args, 1, "poke16")? as u16;
            unsafe { std::ptr::write_unaligned(addr, val) };
            Value::nil()
        }
        ID_POKE32 => {
            let addr = num_u64(args, 0, "poke32")? as *mut u32;
            let val = num_u64(args, 1, "poke32")? as u32;
            unsafe { std::ptr::write_unaligned(addr, val) };
            Value::nil()
        }
        ID_POKE64 => {
            let addr = num_u64(args, 0, "poke64")? as *mut u64;
            let val = num_u64(args, 1, "poke64")?;
            unsafe { std::ptr::write_unaligned(addr, val) };
            Value::nil()
        }
        ID_MEM_READ => {
            let addr = num_u64(args, 0, "mem_read")? as *const u8;
            let len = num_u64(args, 1, "mem_read")? as usize;
            let slice = unsafe { std::slice::from_raw_parts(addr, len) };
            Value::string(hex_encode(slice))
        }
        ID_MEM_WRITE => {
            let addr = num_u64(args, 0, "mem_write")? as *mut u8;
            let bytes = hex_data(args, 1, "mem_write")?;
            if !bytes.is_empty() {
                unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), addr, bytes.len()) };
            }
            Value::nil()
        }
        ID_IOCTL => {
            let fd = fd_arg(args, 0)?;
            let request = num_u64(args, 1, "ioctl")? as c_ulong;
            let r = match args.get(2).copied() {
                None => unsafe { libc::ioctl(fd, request) },
                Some(v) if v.is_nil() => unsafe { libc::ioctl(fd, request, 0 as c_ulong) },
                Some(v) if v.as_int().is_some() || v.as_float().is_some() => {
                    unsafe { libc::ioctl(fd, request, num_u64(&[v], 0, "ioctl")? as c_ulong) }
                }
                Some(v) => {
                    let s = v
                        .as_str()
                        .ok_or_else(|| VmError::type_mismatch("number, string, or null", v))?;
                    let c = CString::new(s)
                        .map_err(|_| VmError::ValueError("sys.ioctl: buffer contains a NUL byte".into()))?;
                    unsafe { libc::ioctl(fd, request, c.as_ptr() as c_ulong) }
                }
            };
            if r == -1 {
                return Err(syserr("ioctl"));
            }
            Value::int_promote(r as i64)
        }
        ID_FCNTL => {
            let fd = fd_arg(args, 0)?;
            let cmd = num_i64(args, 1, "fcntl")? as c_int;
            let r = match args.get(2).copied() {
                None => unsafe { libc::fcntl(fd, cmd) },
                Some(v) if v.is_nil() => unsafe { libc::fcntl(fd, cmd, 0 as c_int) },
                Some(v) => {
                    if let Some(s) = v.as_str() {
                        let c = CString::new(s)
                            .map_err(|_| VmError::ValueError("sys.fcntl: buffer contains a NUL byte".into()))?;
                        unsafe { libc::fcntl(fd, cmd, c.as_ptr() as c_int) }
                    } else {
                        let n = any_i64(v)
                            .ok_or_else(|| VmError::type_mismatch("number, string, or null", v))?;
                        unsafe { libc::fcntl(fd, cmd, n as c_int) }
                    }
                }
            };
            if r == -1 {
                return Err(syserr("fcntl"));
            }
            Value::int_promote(r as i64)
        }
        ID_POLL => poll_impl(args)?,
        ID_SOCKET => {
            let family = opt_num_i64(args, 0, libc::AF_INET as i64) as c_int;
            let ty = opt_num_i64(args, 1, libc::SOCK_STREAM as i64) as c_int;
            let proto = opt_num_i64(args, 2, 0) as c_int;
            let fd = unsafe { libc::socket(family, ty, proto) };
            if fd < 0 {
                return Err(syserr("socket"));
            }
            Value::int_promote(fd as i64)
        }
        ID_SOCKETPAIR => {
            let family = opt_num_i64(args, 0, libc::AF_UNIX as i64) as c_int;
            let ty = opt_num_i64(args, 1, libc::SOCK_STREAM as i64) as c_int;
            let proto = opt_num_i64(args, 2, 0) as c_int;
            let mut fds = [0 as c_int; 2];
            if unsafe { libc::socketpair(family, ty, proto, fds.as_mut_ptr()) } == -1 {
                return Err(syserr("socketpair"));
            }
            new_list(vec![
                Value::int_promote(fds[0] as i64),
                Value::int_promote(fds[1] as i64),
            ])
        }
        ID_BIND => {
            let fd = fd_arg(args, 0)?;
            let addr = hex_data(args, 1, "bind")?;
            let rc = unsafe {
                libc::bind(
                    fd,
                    addr.as_ptr() as *const libc::sockaddr,
                    addr.len() as libc::socklen_t,
                )
            };
            if rc == -1 {
                return Err(syserr("bind"));
            }
            Value::bool(true)
        }
        ID_LISTEN => {
            let fd = fd_arg(args, 0)?;
            let backlog = opt_num_i64(args, 1, 128) as c_int;
            if unsafe { libc::listen(fd, backlog) } == -1 {
                return Err(syserr("listen"));
            }
            Value::bool(true)
        }
        ID_ACCEPT => {
            let fd = fd_arg(args, 0)?;
            let c = unsafe { libc::accept(fd, std::ptr::null_mut(), std::ptr::null_mut()) };
            if c < 0 {
                return Err(syserr("accept"));
            }
            Value::int_promote(c as i64)
        }
        ID_ACCEPT4 => {
            let fd = fd_arg(args, 0)?;
            let flags = opt_num_i64(args, 1, 0) as c_int;
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                let c =
                    unsafe { libc::accept4(fd, std::ptr::null_mut(), std::ptr::null_mut(), flags) };
                if c < 0 {
                    return Err(syserr("accept4"));
                }
                Value::int_promote(c as i64)
            }
            #[cfg(not(any(target_os = "linux", target_os = "android")))]
            {
                let _ = flags;
                let c = unsafe { libc::accept(fd, std::ptr::null_mut(), std::ptr::null_mut()) };
                if c < 0 {
                    return Err(syserr("accept"));
                }
                Value::int_promote(c as i64)
            }
        }
        ID_CONNECT => {
            let fd = fd_arg(args, 0)?;
            let addr = hex_data(args, 1, "connect")?;
            let rc = unsafe {
                libc::connect(
                    fd,
                    addr.as_ptr() as *const libc::sockaddr,
                    addr.len() as libc::socklen_t,
                )
            };
            if rc == -1 {
                return Err(syserr("connect"));
            }
            Value::bool(true)
        }
        ID_GETSOCKNAME => sockaddr_hex(fd_arg(args, 0)?, false, "getsockname")?,
        ID_GETPEERNAME => sockaddr_hex(fd_arg(args, 0)?, true, "getpeername")?,
        ID_SEND => {
            let fd = fd_arg(args, 0)?;
            let data = hex_data(args, 1, "send")?;
            let flags = opt_num_i64(args, 2, 0) as c_int;
            let n = unsafe { libc::send(fd, data.as_ptr() as *const c_void, data.len(), flags) };
            if n < 0 {
                return Err(syserr("send"));
            }
            Value::int_promote(n as i64)
        }
        ID_SENDTO => {
            let fd = fd_arg(args, 0)?;
            let data = hex_data(args, 1, "sendto")?;
            let addr = hex_data(args, 2, "sendto")?;
            let flags = opt_num_i64(args, 3, 0) as c_int;
            let n = unsafe {
                libc::sendto(
                    fd,
                    data.as_ptr() as *const c_void,
                    data.len(),
                    flags,
                    addr.as_ptr() as *const libc::sockaddr,
                    addr.len() as libc::socklen_t,
                )
            };
            if n < 0 {
                return Err(syserr("sendto"));
            }
            Value::int_promote(n as i64)
        }
        ID_RECV => {
            let fd = fd_arg(args, 0)?;
            let maxlen = opt_num_u64(args, 1, 65536).max(0) as usize;
            let flags = opt_num_i64(args, 2, 0) as c_int;
            let mut buf = vec![0u8; maxlen];
            let n = unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut c_void, maxlen, flags) };
            if n < 0 {
                return Err(syserr("recv"));
            }
            Value::string(hex_encode(&buf[..n as usize]))
        }
        ID_RECVFROM => {
            let fd = fd_arg(args, 0)?;
            let maxlen = opt_num_u64(args, 1, 65536).max(0) as usize;
            let flags = opt_num_i64(args, 2, 0) as c_int;
            let mut buf = vec![0u8; maxlen];
            let mut storage: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
            let mut len = std::mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
            let n = unsafe {
                libc::recvfrom(
                    fd,
                    buf.as_mut_ptr() as *mut c_void,
                    maxlen,
                    flags,
                    &mut storage as *mut libc::sockaddr_storage as *mut libc::sockaddr,
                    &mut len,
                )
            };
            if n < 0 {
                return Err(syserr("recvfrom"));
            }
            let addr = unsafe {
                std::slice::from_raw_parts(&storage as *const libc::sockaddr_storage as *const u8, len as usize)
            };
            new_list(vec![
                Value::string(hex_encode(&buf[..n as usize])),
                Value::string(hex_encode(addr)),
            ])
        }
        ID_SHUTDOWN => {
            let fd = fd_arg(args, 0)?;
            let how = opt_num_i64(args, 1, libc::SHUT_RDWR as i64) as c_int;
            if unsafe { libc::shutdown(fd, how) } == -1 {
                return Err(syserr("shutdown"));
            }
            Value::bool(true)
        }
        ID_SETSOCKOPT => {
            let fd = fd_arg(args, 0)?;
            let level = num_i64(args, 1, "setsockopt")? as c_int;
            let optname = num_i64(args, 2, "setsockopt")? as c_int;
            let val = optval_bytes(args, 3, "setsockopt")?;
            let rc = unsafe {
                libc::setsockopt(
                    fd,
                    level,
                    optname,
                    val.as_ptr() as *const c_void,
                    val.len() as libc::socklen_t,
                )
            };
            if rc == -1 {
                return Err(syserr("setsockopt"));
            }
            Value::bool(true)
        }
        ID_GETSOCKOPT => {
            let fd = fd_arg(args, 0)?;
            let level = num_i64(args, 1, "getsockopt")? as c_int;
            let optname = num_i64(args, 2, "getsockopt")? as c_int;
            let want = opt_num_u64(args, 3, 4).max(0) as usize;
            let mut buf = vec![0u8; want];
            let mut len = want as libc::socklen_t;
            let rc = unsafe {
                libc::getsockopt(
                    fd,
                    level,
                    optname,
                    buf.as_mut_ptr() as *mut c_void,
                    &mut len,
                )
            };
            if rc == -1 {
                return Err(syserr("getsockopt"));
            }
            Value::string(hex_encode(&buf[..len as usize]))
        }
        ID_GETHOSTNAME => {
            let mut buf = vec![0u8; 256];
            if unsafe { libc::gethostname(buf.as_mut_ptr() as *mut c_char, buf.len()) } == -1 {
                return Err(syserr("gethostname"));
            }
            let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
            buf.truncate(len);
            Value::string(String::from_utf8_lossy(&buf).into_owned())
        }
        ID_UNAME => {
            let mut u: libc::utsname = unsafe { std::mem::zeroed() };
            if unsafe { libc::uname(&mut u) } == -1 {
                return Err(syserr("uname"));
            }
            fn f2s(f: [c_char; 65]) -> String {
                unsafe { CStr::from_ptr(f.as_ptr()) }
                    .to_string_lossy()
                    .into_owned()
            }
            new_dict(IndexMap::from_iter([
                (DictKey(Value::string("sysname")), Value::string(f2s(u.sysname))),
                (DictKey(Value::string("nodename")), Value::string(f2s(u.nodename))),
                (DictKey(Value::string("release")), Value::string(f2s(u.release))),
                (DictKey(Value::string("version")), Value::string(f2s(u.version))),
                (DictKey(Value::string("machine")), Value::string(f2s(u.machine))),
            ]))
        }
        ID_CLOCK_GETTIME => {
            let clk = num_i64(args, 0, "clock_gettime")? as c_int;
            let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
            if unsafe { libc::clock_gettime(clk, &mut ts) } == -1 {
                return Err(syserr("clock_gettime"));
            }
            Value::float(ts.tv_sec as f64 + ts.tv_nsec as f64 / 1e9)
        }
        ID_GETRLIMIT => {
            let which = num_i64(args, 0, "getrlimit")? as libc::__rlimit_resource_t;
            let mut rl: libc::rlimit = unsafe { std::mem::zeroed() };
            if unsafe { libc::getrlimit(which, &mut rl) } == -1 {
                return Err(syserr("getrlimit"));
            }
            let soft = if rl.rlim_cur == libc::RLIM_INFINITY {
                -1.0
            } else {
                rl.rlim_cur as f64
            };
            let hard = if rl.rlim_max == libc::RLIM_INFINITY {
                -1.0
            } else {
                rl.rlim_max as f64
            };
            new_list(vec![Value::float(soft), Value::float(hard)])
        }
        ID_SETRLIMIT => {
            let which = num_i64(args, 0, "setrlimit")? as libc::__rlimit_resource_t;
            let limits_v = arg_at(args, 1)?;
            let limits = limits_v
                .as_list()
                .ok_or_else(|| VmError::type_mismatch("list", limits_v))?;
            let to_rlim = |v: &Value| -> libc::rlim_t {
                if let Some(i) = v.as_int() {
                    if i < 0 {
                        libc::RLIM_INFINITY
                    } else {
                        i as libc::rlim_t
                    }
                } else if let Some(f) = v.as_float() {
                    if f < 0.0 {
                        libc::RLIM_INFINITY
                    } else {
                        f as libc::rlim_t
                    }
                } else {
                    libc::RLIM_INFINITY
                }
            };
            let soft = to_rlim(limits.first().unwrap_or(&Value::float(-1.0)));
            let hard = to_rlim(limits.get(1).unwrap_or(&Value::float(-1.0)));
            let rl = libc::rlimit {
                rlim_cur: soft,
                rlim_max: hard,
            };
            if unsafe { libc::setrlimit(which, &rl) } == -1 {
                return Err(syserr("setrlimit"));
            }
            Value::bool(true)
        }
        ID_SYSCONF => {
            let name = num_i64(args, 0, "sysconf")? as c_int;
            let r = unsafe { libc::sysconf(name) };
            if r == -1 {
                return Err(syserr("sysconf"));
            }
            Value::int_promote(r as i64)
        }
        ID_TIMES => {
            let mut t: libc::tms = unsafe { std::mem::zeroed() };
            if unsafe { libc::times(&mut t) } == -1 {
                return Err(syserr("times"));
            }
            let hz = 100.0_f64;
            let secs = |v: libc::clock_t| Value::float(v as f64 / hz);
            new_dict(IndexMap::from_iter([
                (DictKey(Value::string("utime")), secs(t.tms_utime)),
                (DictKey(Value::string("stime")), secs(t.tms_stime)),
                (DictKey(Value::string("cutime")), secs(t.tms_cutime)),
                (DictKey(Value::string("cstime")), secs(t.tms_cstime)),
            ]))
        }
        ID_STRERROR => {
            let e = num_i64(args, 0, "strerror")? as i32;
            Value::string(errno_message(e))
        }
        ID_ERRNO => Value::int_promote(errno_now() as i64),
        // Python-style helpers.
        ID_EXIT => crate::ops::os_exit(args, output)?,
        ID_GETSIZEOF => getsizeof(arg_at(args, 0)?),
        ID_GETRECURSIONLIMIT => Value::int(10000),
        ID_GETDEFAULTENCODING => Value::string("utf-8"),
        ID_GETFILESYSTEMENCODING => Value::string("utf-8"),
        ID_INTERN => arg_at(args, 0)?,
        ID_CALL_TRACING | ID_SETTRACE | ID_GETTRACE => Value::nil(),
        ID_MODULES => new_dict(empty_dict()),
        _ => return Ok(None),
    };
    Ok(Some(v))
}

fn sys_exec(args: &[Value]) -> VmResult<Value> {
    let path = path_cstr(args, 0, "exec")?;
    let mut cargs: Vec<CString> = vec![path.clone()];
    if let Some(list) = args.get(1).and_then(|v| v.as_list()) {
        for item in list {
            let s = item.as_str().ok_or_else(|| {
                VmError::type_mismatch("list of strings", *item)
            })?;
            cargs.push(
                CString::new(s.to_string())
                    .map_err(|_| VmError::ValueError("sys.exec: argv contains a NUL byte".into()))?,
            );
        }
    }
    let mut ptrs: Vec<*const c_char> = cargs.iter().map(|c| c.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    unsafe { libc::execv(path.as_ptr(), ptrs.as_ptr()) };
    Err(syserr("exec"))
}

fn poll_impl(args: &[Value]) -> VmResult<Value> {
    let entries = arg_at(args, 0)?;
    let entries = entries
        .as_list()
        .ok_or_else(|| VmError::type_mismatch("list", entries))?;
    let mut fds: Vec<libc::pollfd> = Vec::with_capacity(entries.len());
    for e in entries.iter() {
        let d = e
            .as_dict()
            .ok_or_else(|| VmError::type_mismatch("dict {\"fd\", \"events\"}", *e))?;
        let fd = crate::ops::dict_get(d, Value::string("fd"))
            .and_then(|v| any_i64(v))
            .ok_or_else(|| VmError::type_mismatch("numeric fd", *e))? as c_int;
        let events = crate::ops::dict_get(d, Value::string("events"))
            .and_then(|v| any_i64(v))
            .ok_or_else(|| VmError::type_mismatch("numeric events", *e))? as i16;
        fds.push(libc::pollfd {
            fd,
            events,
            revents: 0,
        });
    }
    let timeout = opt_num_i64(args, 1, -1) as c_int;
    let r = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) };
    if r == -1 {
        return Err(syserr("poll"));
    }
    let mut out: Vec<Value> = Vec::new();
    for f in fds.iter().filter(|f| f.revents != 0) {
        out.push(new_dict(IndexMap::from_iter([
            (DictKey(Value::string("fd")), Value::int_promote(f.fd as i64)),
            (DictKey(Value::string("revents")), Value::int_promote(f.revents as i64)),
        ])));
    }
    Ok(new_list(out))
}

fn sys_syscall(args: &[Value]) -> VmResult<Value> {
    let number = num_i64(args, 0, "syscall")? as c_long;
    let mut vals: Vec<i64> = Vec::new();
    if let Some(list) = args.get(1).and_then(|v| v.as_list()) {
        for item in list.iter() {
            vals.push(
                any_i64(*item).ok_or_else(|| VmError::type_mismatch("number", *item))?,
            );
        }
    }
    if vals.len() > 6 {
        return Err(VmError::ValueError("sys.syscall: at most 6 arguments".into()));
    }
    let n = number;
    let r = unsafe {
        match vals.len() {
            0 => libc::syscall(n),
            1 => libc::syscall(n, vals[0] as c_long),
            2 => libc::syscall(n, vals[0] as c_long, vals[1] as c_long),
            3 => libc::syscall(n, vals[0] as c_long, vals[1] as c_long, vals[2] as c_long),
            4 => libc::syscall(
                n,
                vals[0] as c_long,
                vals[1] as c_long,
                vals[2] as c_long,
                vals[3] as c_long,
            ),
            5 => libc::syscall(
                n,
                vals[0] as c_long,
                vals[1] as c_long,
                vals[2] as c_long,
                vals[3] as c_long,
                vals[4] as c_long,
            ),
            _ => libc::syscall(
                n,
                vals[0] as c_long,
                vals[1] as c_long,
                vals[2] as c_long,
                vals[3] as c_long,
                vals[4] as c_long,
                vals[5] as c_long,
            ),
        }
    };
    if r == -1 {
        return Err(syserr("syscall"));
    }
    Ok(Value::int_promote(r as i64))
}

fn getsizeof(v: Value) -> Value {
    // Ropes report like the flat string they read as (bytes + object words).
    if v.is_string() {
        return Value::int_promote(
            v.as_str().map(|s| s.len() as i64).unwrap_or(0) + 49,
        );
    }
    let n = match v.tag() {
        crate::value::Tag::String => v.as_str().map(|s| s.len() as i64).unwrap_or(0) + 49,
        crate::value::Tag::Heap => {
            if let Some(l) = v.as_list() {
                l.len() as i64 * 8 + 56
            } else if let Some(d) = v.as_dict() {
                d.len() as i64 * 120 + 240
            } else {
                0
            }
        }
        crate::value::Tag::Int => 28,
        crate::value::Tag::Float => 24,
        crate::value::Tag::True | crate::value::Tag::False => 24,
        _ => 0,
    };
    Value::int_promote(n)
}

// ── module registration ─────────────────────────────────────────────────────

fn func(name: &'static str, id: u16, required: u8, max: u8) -> HostExport {
    HostExport::Func {
        name,
        id,
        required,
        max,
    }
}

fn konst(name: &'static str, value: i64) -> HostExport {
    HostExport::Const {
        name,
        value: Value::int_promote(value),
    }
}

pub fn module() -> ModuleUnit {
    host_module_mixed(&[
        // Raw syscalls.
        func("syscall", ID_SCAL, 1, 2),
        func("open", ID_OPEN, 2, 3),
        func("close", ID_CLOSE, 1, 1),
        func("read", ID_READ, 2, 2),
        func("read_str", ID_READ_STR, 2, 2),
        func("write", ID_WRITE, 2, 2),
        func("write_bytes", ID_WRITE_BYTES, 2, 2),
        func("lseek", ID_LSEEK, 3, 3),
        func("dup", ID_DUP, 1, 1),
        func("dup2", ID_DUP2, 2, 2),
        func("pipe", ID_PIPE, 0, 0),
        func("fork", ID_FORK, 0, 0),
        func("exec", ID_EXEC, 1, 2),
        func("kill", ID_KILL, 2, 2),
        func("getpid", ID_GETPID, 0, 0),
        func("getppid", ID_GETPPID, 0, 0),
        func("getuid", ID_GETUID, 0, 0),
        func("geteuid", ID_GETEUID, 0, 0),
        func("getgid", ID_GETGID, 0, 0),
        func("getegid", ID_GETEGID, 0, 0),
        func("umask", ID_UMASK, 0, 1),
        func("stat", ID_STAT, 1, 1),
        func("lstat", ID_LSTAT, 1, 1),
        func("fstat", ID_FSTAT, 1, 1),
        func("access", ID_ACCESS, 2, 2),
        func("chmod", ID_CHMOD, 1, 2),
        func("chown", ID_CHOWN, 3, 3),
        func("fchown", ID_FCHOWN, 3, 3),
        func("symlink", ID_SYMLINK, 2, 2),
        func("readlink", ID_READLINK, 1, 1),
        func("link", ID_LINK, 2, 2),
        func("truncate", ID_TRUNCATE, 2, 2),
        func("ftruncate", ID_FTRUNCATE, 2, 2),
        func("fsync", ID_FSYNC, 1, 1),
        func("fdatasync", ID_FDATASYNC, 1, 1),
        func("unlink", ID_UNLINK, 1, 1),
        func("rmdir", ID_RMDIR, 1, 1),
        func("mkfifo", ID_MKFIFO, 1, 2),
        func("mknod", ID_MKNOD, 3, 3),
        func("waitpid", ID_WAITPID, 1, 2),
        func("wifexited", ID_WIFEXITED, 1, 1),
        func("wifsignaled", ID_WIFSIGNALED, 1, 1),
        func("wifstopped", ID_WIFSTOPPED, 1, 1),
        func("wexitstatus", ID_WEXITSTATUS, 1, 1),
        func("wtermsig", ID_WTERMSIG, 1, 1),
        func("nanosleep", ID_NANOSLEEP, 1, 1),
        func("mmap", ID_MMAP, 5, 5),
        func("munmap", ID_MUNMAP, 2, 2),
        func("mprotect", ID_MPROTECT, 3, 3),
        func("mremap", ID_MREMAP, 4, 4),
        func("mlock", ID_MLOCK, 2, 2),
        func("munlock", ID_MUNLOCK, 2, 2),
        func("peek8", ID_PEEK8, 1, 1),
        func("peek16", ID_PEEK16, 1, 1),
        func("peek32", ID_PEEK32, 1, 1),
        func("peek64", ID_PEEK64, 1, 1),
        func("poke8", ID_POKE8, 2, 2),
        func("poke16", ID_POKE16, 2, 2),
        func("poke32", ID_POKE32, 2, 2),
        func("poke64", ID_POKE64, 2, 2),
        func("mem_read", ID_MEM_READ, 2, 2),
        func("mem_write", ID_MEM_WRITE, 2, 2),
        func("ioctl", ID_IOCTL, 2, 3),
        func("fcntl", ID_FCNTL, 2, 3),
        func("poll", ID_POLL, 1, 2),
        func("socket", ID_SOCKET, 0, 3),
        func("socketpair", ID_SOCKETPAIR, 0, 3),
        func("bind", ID_BIND, 2, 2),
        func("listen", ID_LISTEN, 1, 2),
        func("accept", ID_ACCEPT, 1, 1),
        func("accept4", ID_ACCEPT4, 1, 2),
        func("connect", ID_CONNECT, 2, 2),
        func("getsockname", ID_GETSOCKNAME, 1, 1),
        func("getpeername", ID_GETPEERNAME, 1, 1),
        func("send", ID_SEND, 2, 3),
        func("sendto", ID_SENDTO, 3, 4),
        func("recv", ID_RECV, 1, 3),
        func("recvfrom", ID_RECVFROM, 1, 3),
        func("shutdown", ID_SHUTDOWN, 1, 2),
        func("setsockopt", ID_SETSOCKOPT, 4, 4),
        func("getsockopt", ID_GETSOCKOPT, 3, 4),
        func("gethostname", ID_GETHOSTNAME, 0, 0),
        func("uname", ID_UNAME, 0, 0),
        func("clock_gettime", ID_CLOCK_GETTIME, 1, 1),
        func("getrlimit", ID_GETRLIMIT, 1, 1),
        func("setrlimit", ID_SETRLIMIT, 2, 2),
        func("sysconf", ID_SYSCONF, 1, 1),
        func("times", ID_TIMES, 0, 0),
        func("strerror", ID_STRERROR, 1, 1),
        func("errno", ID_ERRNO, 0, 0),
        // Python-style runtime helpers.
        func("exit", ID_EXIT, 1, 1),
        func("getsizeof", ID_GETSIZEOF, 1, 1),
        func("getrecursionlimit", ID_GETRECURSIONLIMIT, 0, 0),
        func("getdefaultencoding", ID_GETDEFAULTENCODING, 0, 0),
        func("getfilesystemencoding", ID_GETFILESYSTEMENCODING, 0, 0),
        func("intern", ID_INTERN, 1, 1),
        func("call_tracing", ID_CALL_TRACING, 2, 2),
        func("settrace", ID_SETTRACE, 0, 1),
        func("gettrace", ID_GETTRACE, 0, 0),
        func("modules", ID_MODULES, 0, 0),
        // Constants.
        konst("AF_UNIX", libc::AF_UNIX as i64),
        konst("AF_LOCAL", libc::AF_UNIX as i64),
        konst("AF_INET", libc::AF_INET as i64),
        konst("AF_INET6", libc::AF_INET6 as i64),
        konst("AF_PACKET", libc::AF_PACKET as i64),
        konst("AF_NETLINK", libc::AF_NETLINK as i64),
        konst("SOCK_STREAM", libc::SOCK_STREAM as i64),
        konst("SOCK_DGRAM", libc::SOCK_DGRAM as i64),
        konst("SOCK_SEQPACKET", libc::SOCK_SEQPACKET as i64),
        konst("SOCK_RAW", libc::SOCK_RAW as i64),
        konst("SOCK_NONBLOCK", libc::SOCK_NONBLOCK as i64),
        konst("SOCK_CLOEXEC", libc::SOCK_CLOEXEC as i64),
        konst("IPPROTO_IP", libc::IPPROTO_IP as i64),
        konst("IPPROTO_ICMP", libc::IPPROTO_ICMP as i64),
        konst("IPPROTO_TCP", libc::IPPROTO_TCP as i64),
        konst("IPPROTO_UDP", libc::IPPROTO_UDP as i64),
        konst("IPPROTO_RAW", libc::IPPROTO_RAW as i64),
        konst("SOL_SOCKET", libc::SOL_SOCKET as i64),
        konst("SOL_TCP", libc::SOL_TCP as i64),
        konst("SO_REUSEADDR", libc::SO_REUSEADDR as i64),
        konst("SO_REUSEPORT", libc::SO_REUSEPORT as i64),
        konst("SO_KEEPALIVE", libc::SO_KEEPALIVE as i64),
        konst("SO_BROADCAST", libc::SO_BROADCAST as i64),
        konst("SO_LINGER", libc::SO_LINGER as i64),
        konst("SO_RCVBUF", libc::SO_RCVBUF as i64),
        konst("SO_SNDBUF", libc::SO_SNDBUF as i64),
        konst("SO_RCVTIMEO", libc::SO_RCVTIMEO as i64),
        konst("SO_SNDTIMEO", libc::SO_SNDTIMEO as i64),
        konst("SO_OOBINLINE", libc::SO_OOBINLINE as i64),
        konst("TCP_NODELAY", libc::TCP_NODELAY as i64),
        konst("TCP_KEEPIDLE", libc::TCP_KEEPIDLE as i64),
        konst("TCP_KEEPINTVL", libc::TCP_KEEPINTVL as i64),
        konst("MSG_PEEK", libc::MSG_PEEK as i64),
        konst("MSG_DONTWAIT", libc::MSG_DONTWAIT as i64),
        konst("MSG_WAITALL", libc::MSG_WAITALL as i64),
        konst("MSG_NOSIGNAL", libc::MSG_NOSIGNAL as i64),
        konst("MSG_OOB", libc::MSG_OOB as i64),
        konst("MSG_TRUNC", libc::MSG_TRUNC as i64),
        konst("MSG_DONTROUTE", libc::MSG_DONTROUTE as i64),
        konst("SHUT_RD", libc::SHUT_RD as i64),
        konst("SHUT_WR", libc::SHUT_WR as i64),
        konst("SHUT_RDWR", libc::SHUT_RDWR as i64),
        konst("PROT_NONE", libc::PROT_NONE as i64),
        konst("PROT_READ", libc::PROT_READ as i64),
        konst("PROT_WRITE", libc::PROT_WRITE as i64),
        konst("PROT_EXEC", libc::PROT_EXEC as i64),
        konst("PROT_GROWSDOWN", libc::PROT_GROWSDOWN as i64),
        konst("PROT_GROWSUP", libc::PROT_GROWSUP as i64),
        konst("MAP_SHARED", libc::MAP_SHARED as i64),
        konst("MAP_PRIVATE", libc::MAP_PRIVATE as i64),
        konst("MAP_FIXED", libc::MAP_FIXED as i64),
        konst("MAP_ANONYMOUS", libc::MAP_ANONYMOUS as i64),
        konst("MAP_ANON", libc::MAP_ANONYMOUS as i64),
        konst("MAP_GROWSDOWN", libc::MAP_GROWSDOWN as i64),
        konst("MAP_STACK", libc::MAP_STACK as i64),
        konst("MAP_DENYWRITE", libc::MAP_DENYWRITE as i64),
        konst("MAP_EXECUTABLE", libc::MAP_EXECUTABLE as i64),
        konst("MAP_LOCKED", libc::MAP_LOCKED as i64),
        konst("MAP_NORESERVE", libc::MAP_NORESERVE as i64),
        konst("MAP_POPULATE", libc::MAP_POPULATE as i64),
        konst("MAP_HUGETLB", libc::MAP_HUGETLB as i64),
        konst("O_RDONLY", libc::O_RDONLY as i64),
        konst("O_WRONLY", libc::O_WRONLY as i64),
        konst("O_RDWR", libc::O_RDWR as i64),
        konst("O_CREAT", libc::O_CREAT as i64),
        konst("O_EXCL", libc::O_EXCL as i64),
        konst("O_TRUNC", libc::O_TRUNC as i64),
        konst("O_APPEND", libc::O_APPEND as i64),
        konst("O_SYNC", libc::O_SYNC as i64),
        konst("O_DSYNC", libc::O_DSYNC as i64),
        konst("O_RSYNC", libc::O_RSYNC as i64),
        konst("O_NONBLOCK", libc::O_NONBLOCK as i64),
        konst("O_CLOEXEC", libc::O_CLOEXEC as i64),
        konst("O_DIRECTORY", libc::O_DIRECTORY as i64),
        konst("O_NOCTTY", libc::O_NOCTTY as i64),
        konst("O_NOFOLLOW", libc::O_NOFOLLOW as i64),
        konst("O_DIRECT", libc::O_DIRECT as i64),
        konst("O_LARGEFILE", libc::O_LARGEFILE as i64),
        konst("O_ASYNC", libc::O_ASYNC as i64),
        konst("SEEK_SET", libc::SEEK_SET as i64),
        konst("SEEK_CUR", libc::SEEK_CUR as i64),
        konst("SEEK_END", libc::SEEK_END as i64),
        konst("F_OK", libc::F_OK as i64),
        konst("X_OK", libc::X_OK as i64),
        konst("W_OK", libc::W_OK as i64),
        konst("R_OK", libc::R_OK as i64),
        konst("WNOHANG", libc::WNOHANG as i64),
        konst("WUNTRACED", libc::WUNTRACED as i64),
        konst("WCONTINUED", libc::WCONTINUED as i64),
        konst("POLLIN", libc::POLLIN as i64),
        konst("POLLPRI", libc::POLLPRI as i64),
        konst("POLLOUT", libc::POLLOUT as i64),
        konst("POLLERR", libc::POLLERR as i64),
        konst("POLLHUP", libc::POLLHUP as i64),
        konst("POLLNVAL", libc::POLLNVAL as i64),
        konst("POLLRDNORM", libc::POLLRDNORM as i64),
        konst("POLLWRNORM", libc::POLLWRNORM as i64),
        konst("CLOCK_REALTIME", libc::CLOCK_REALTIME as i64),
        konst("CLOCK_MONOTONIC", libc::CLOCK_MONOTONIC as i64),
        konst("CLOCK_PROCESS_CPUTIME_ID", libc::CLOCK_PROCESS_CPUTIME_ID as i64),
        konst("CLOCK_THREAD_CPUTIME_ID", libc::CLOCK_THREAD_CPUTIME_ID as i64),
        konst("CLOCK_MONOTONIC_RAW", libc::CLOCK_MONOTONIC_RAW as i64),
        konst("CLOCK_REALTIME_COARSE", libc::CLOCK_REALTIME_COARSE as i64),
        konst("CLOCK_MONOTONIC_COARSE", libc::CLOCK_MONOTONIC_COARSE as i64),
        konst("CLOCK_BOOTTIME", libc::CLOCK_BOOTTIME as i64),
        konst("RLIMIT_CPU", libc::RLIMIT_CPU as i64),
        konst("RLIMIT_FSIZE", libc::RLIMIT_FSIZE as i64),
        konst("RLIMIT_DATA", libc::RLIMIT_DATA as i64),
        konst("RLIMIT_STACK", libc::RLIMIT_STACK as i64),
        konst("RLIMIT_CORE", libc::RLIMIT_CORE as i64),
        konst("RLIMIT_RSS", libc::RLIMIT_RSS as i64),
        konst("RLIMIT_NPROC", libc::RLIMIT_NPROC as i64),
        konst("RLIMIT_NOFILE", libc::RLIMIT_NOFILE as i64),
        konst("RLIMIT_MEMLOCK", libc::RLIMIT_MEMLOCK as i64),
        konst("RLIMIT_AS", libc::RLIMIT_AS as i64),
        konst("RLIMIT_LOCKS", libc::RLIMIT_LOCKS as i64),
        konst("RLIMIT_SIGPENDING", libc::RLIMIT_SIGPENDING as i64),
        konst("RLIMIT_MSGQUEUE", libc::RLIMIT_MSGQUEUE as i64),
        konst("RLIMIT_NICE", libc::RLIMIT_NICE as i64),
        konst("RLIMIT_RTPRIO", libc::RLIMIT_RTPRIO as i64),
        konst("RLIMIT_RTTIME", libc::RLIMIT_RTTIME as i64),
        konst("F_DUPFD", libc::F_DUPFD as i64),
        konst("F_DUPFD_CLOEXEC", libc::F_DUPFD_CLOEXEC as i64),
        konst("F_GETFD", libc::F_GETFD as i64),
        konst("F_SETFD", libc::F_SETFD as i64),
        konst("F_GETFL", libc::F_GETFL as i64),
        konst("F_SETFL", libc::F_SETFL as i64),
        konst("F_GETLK", libc::F_GETLK as i64),
        konst("F_SETLK", libc::F_SETLK as i64),
        konst("F_SETLKW", libc::F_SETLKW as i64),
        konst("F_SETOWN", libc::F_SETOWN as i64),
        konst("F_GETOWN", libc::F_GETOWN as i64),
        konst("FD_CLOEXEC", libc::FD_CLOEXEC as i64),
        konst("SC_CLK_TCK", libc::_SC_CLK_TCK as i64),
        konst("SC_NPROCESSORS_ONLN", libc::_SC_NPROCESSORS_ONLN as i64),
        konst("SC_PAGESIZE", libc::_SC_PAGESIZE as i64),
        konst("SC_PHYS_PAGES", libc::_SC_PHYS_PAGES as i64),
        konst("SC_OPEN_MAX", libc::_SC_OPEN_MAX as i64),
        konst("SC_ARG_MAX", libc::_SC_ARG_MAX as i64),
        konst("SC_CHILD_MAX", libc::_SC_CHILD_MAX as i64),
        konst("SC_HOST_NAME_MAX", libc::_SC_HOST_NAME_MAX as i64),
        konst("SIGHUP", libc::SIGHUP as i64),
        konst("SIGINT", libc::SIGINT as i64),
        konst("SIGQUIT", libc::SIGQUIT as i64),
        konst("SIGILL", libc::SIGILL as i64),
        konst("SIGTRAP", libc::SIGTRAP as i64),
        konst("SIGABRT", libc::SIGABRT as i64),
        konst("SIGBUS", libc::SIGBUS as i64),
        konst("SIGFPE", libc::SIGFPE as i64),
        konst("SIGKILL", libc::SIGKILL as i64),
        konst("SIGUSR1", libc::SIGUSR1 as i64),
        konst("SIGSEGV", libc::SIGSEGV as i64),
        konst("SIGUSR2", libc::SIGUSR2 as i64),
        konst("SIGPIPE", libc::SIGPIPE as i64),
        konst("SIGALRM", libc::SIGALRM as i64),
        konst("SIGTERM", libc::SIGTERM as i64),
        konst("SIGCHLD", libc::SIGCHLD as i64),
        konst("SIGCONT", libc::SIGCONT as i64),
        konst("SIGSTOP", libc::SIGSTOP as i64),
        konst("SIGTSTP", libc::SIGTSTP as i64),
        konst("SIGTTIN", libc::SIGTTIN as i64),
        konst("SIGTTOU", libc::SIGTTOU as i64),
        konst("SIGURG", libc::SIGURG as i64),
        konst("SIGXCPU", libc::SIGXCPU as i64),
        konst("SIGXFSZ", libc::SIGXFSZ as i64),
        konst("SIGVTALRM", libc::SIGVTALRM as i64),
        konst("SIGPROF", libc::SIGPROF as i64),
        konst("SIGWINCH", libc::SIGWINCH as i64),
        konst("SIGIO", libc::SIGIO as i64),
        konst("SIGSYS", libc::SIGSYS as i64),
        // Python-style constants.
        HostExport::Const {
            name: "version",
            value: Value::string(format!("Zen {} (Rust)", env!("CARGO_PKG_VERSION"))),
        },
        HostExport::Const {
            name: "platform",
            value: Value::string(std::env::consts::OS),
        },
        HostExport::Const {
            name: "byteorder",
            value: Value::string("little"),
        },
        HostExport::Const {
            name: "maxsize",
            value: Value::float(i64::MAX as f64),
        },
        HostExport::Const {
            name: "hexversion",
            value: Value::int(0),
        },
        HostExport::Const {
            name: "executable",
            value: Value::string("zen"),
        },
        HostExport::Const {
            name: "stdin",
            value: Value::string("<stdin>"),
        },
        HostExport::Const {
            name: "stdout",
            value: Value::string("<stdout>"),
        },
        HostExport::Const {
            name: "stderr",
            value: Value::string("<stderr>"),
        },
        HostExport::Const {
            name: "argv",
            value: {
                let args: Vec<Value> = std::env::args()
                    .skip(1)
                    .map(|a| Value::string(a))
                    .collect();
                crate::gc::new_list(args)
            },
        },
        HostExport::Const {
            name: "path",
            value: crate::gc::new_list(vec![Value::string(".")]),
        },
    ])
}