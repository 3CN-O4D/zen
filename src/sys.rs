//! Zen `sys` module — raw POSIX/Linux system calls and memory access.
//!
//! Every function here is a thin wrapper over the `libc` crate. Pointers are
//! passed around as plain numbers (`Value::Number`), so pointer arithmetic in
//! Zen is ordinary integer arithmetic. All failures raise Zen errors that can
//! be caught with `try { } catch`.

use crate::runtime::Value;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_long, c_ulong, c_void};
use std::sync::Arc;

fn num_i64(v: &Value, what: &str) -> Result<i64, String> {
    match v {
        Value::Number(n) => Ok(*n as i64),
        Value::Bool(b) => Ok(*b as i64),
        _ => Err(format!(
            "sys.{what}: expected a number, got {}",
            v.type_name()
        )),
    }
}

fn num_u64(v: &Value, what: &str) -> Result<u64, String> {
    Ok(num_i64(v, what)? as u64)
}

fn errno_now() -> i32 {
    #[cfg(target_os = "linux")]
    {
        unsafe { *libc::__errno_location() }
    }
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
    {
        unsafe { *libc::__error() }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "ios", target_os = "freebsd")))]
    {
        0
    }
}

fn errno_message(e: i32) -> String {
    let p = unsafe { libc::strerror(e) };
    if p.is_null() {
        format!("errno {e}")
    } else {
        unsafe { CStr::from_ptr(p) }
            .to_string_lossy()
            .into_owned()
    }
}

/// Format a failing syscall as a Zen error message.
fn syserr(what: &str) -> String {
    let e = errno_now();
    format!("sys.{what}: {} (errno {e})", errno_message(e))
}

fn path_cstr(v: &Value, what: &str) -> Result<CString, String> {
    match v {
        Value::String(s) => CString::new(s.clone())
            .map_err(|_| format!("sys.{what}: path contains a NUL byte")),
        _ => Err(format!(
            "sys.{what}: expected a string path, got {}",
            v.type_name()
        )),
    }
}

fn opt_num_i64(v: Option<&Value>, default: i64, what: &str) -> Result<i64, String> {
    match v {
        Some(x) => num_i64(x, what),
        None => Ok(default),
    }
}

fn mode_from(v: Option<&Value>, default: i64, what: &str) -> Result<libc::mode_t, String> {
    Ok(opt_num_i64(v, default, what)? as libc::mode_t)
}

fn fd_from(v: &Value, what: &str) -> Result<c_int, String> {
    Ok(num_i64(v, what)? as c_int)
}

fn stat_dict(s: &libc::stat) -> Value {
    let mut m = indexmap::IndexMap::new();
    m.insert("dev".into(), Value::Number(s.st_dev as f64));
    m.insert("ino".into(), Value::Number(s.st_ino as f64));
    m.insert("mode".into(), Value::Number(s.st_mode as f64));
    m.insert("nlink".into(), Value::Number(s.st_nlink as f64));
    m.insert("uid".into(), Value::Number(s.st_uid as f64));
    m.insert("gid".into(), Value::Number(s.st_gid as f64));
    m.insert("rdev".into(), Value::Number(s.st_rdev as f64));
    m.insert("size".into(), Value::Number(s.st_size as f64));
    m.insert("blksize".into(), Value::Number(s.st_blksize as f64));
    m.insert("blocks".into(), Value::Number(s.st_blocks as f64));
    m.insert("atime".into(), Value::Number(s.st_atime as f64));
    m.insert("mtime".into(), Value::Number(s.st_mtime as f64));
    m.insert("ctime".into(), Value::Number(s.st_ctime as f64));
    Value::Dict(Arc::new(m))
}

fn do_stat(path: &str, lstat: bool) -> Result<Value, String> {
    let c = CString::new(path).map_err(|_| "sys.stat: path contains a NUL byte".to_string())?;
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

fn buf_hex(data: &[u8]) -> Value {
    Value::String(crate::runtime::hex_encode(data))
}

fn hex_data(v: &Value, what: &str) -> Result<Vec<u8>, String> {
    match v {
        Value::String(s) => crate::runtime::hex_decode(s)
            .ok_or_else(|| format!("sys.{what}: invalid hex data")),
        _ => Err(format!(
            "sys.{what}: expected a hex string, got {}",
            v.type_name()
        )),
    }
}

// ---------------------------------------------------------------------------
// syscalls
// ---------------------------------------------------------------------------

pub fn sys_syscall(args: Vec<Value>) -> Result<Value, String> {
    let number = num_i64(args.first().ok_or("sys.syscall expects (number, args?)")?, "syscall")?;
    let mut vals: Vec<i64> = Vec::new();
    if let Some(Value::List(l)) = args.get(1) {
        for item in l.iter() {
            vals.push(num_i64(item, "syscall")?);
        }
    }
    if vals.len() > 6 {
        return Err("sys.syscall: at most 6 arguments".into());
    }
    let n = number as c_long;
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
    Ok(Value::Number(r as f64))
}

pub fn sys_open(args: Vec<Value>) -> Result<Value, String> {
    let path = path_cstr(&args[0], "open")?;
    let flags = num_i64(args.get(1).ok_or("sys.open expects (path, flags, mode?)")?, "open")? as c_int;
    let mode = mode_from(args.get(2), 0o644, "open")?;
    let fd = unsafe { libc::open(path.as_ptr(), flags, mode) };
    if fd < 0 {
        Err(syserr("open"))
    } else {
        Ok(Value::Number(fd as f64))
    }
}

pub fn sys_close(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.close expects fd")?, "close")?;
    if unsafe { libc::close(fd) } == -1 {
        Err(syserr("close"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_read(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.read expects (fd, count)")?, "read")?;
    let count = num_u64(args.get(1).ok_or("sys.read expects (fd, count)")?, "read")? as usize;
    let mut buf = vec![0u8; count];
    let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut c_void, count) };
    if n < 0 {
        Err(syserr("read"))
    } else {
        Ok(buf_hex(&buf[..n as usize]))
    }
}

pub fn sys_read_str(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.read_str expects (fd, count)")?, "read_str")?;
    let count = num_u64(args.get(1).ok_or("sys.read_str expects (fd, count)")?, "read_str")? as usize;
    let mut buf = vec![0u8; count];
    let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut c_void, count) };
    if n < 0 {
        Err(syserr("read_str"))
    } else {
        Ok(Value::String(String::from_utf8_lossy(&buf[..n as usize]).into_owned()))
    }
}

pub fn sys_write(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.write expects (fd, text)")?, "write")?;
    let text = match args.get(1) {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("sys.write expects (fd, string)".into()),
    };
    let n = unsafe { libc::write(fd, text.as_ptr() as *const c_void, text.len()) };
    if n < 0 {
        Err(syserr("write"))
    } else {
        Ok(Value::Number(n as f64))
    }
}

pub fn sys_write_bytes(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.write_bytes expects (fd, hex)")?, "write_bytes")?;
    let bytes = hex_data(args.get(1).ok_or("sys.write_bytes expects (fd, hex)")?, "write_bytes")?;
    let n = unsafe { libc::write(fd, bytes.as_ptr() as *const c_void, bytes.len()) };
    if n < 0 {
        Err(syserr("write_bytes"))
    } else {
        Ok(Value::Number(n as f64))
    }
}

pub fn sys_lseek(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.lseek expects (fd, offset, whence)")?, "lseek")?;
    let offset = num_i64(args.get(1).ok_or("sys.lseek expects (fd, offset, whence)")?, "lseek")?;
    let whence = num_i64(args.get(2).ok_or("sys.lseek expects (fd, offset, whence)")?, "lseek")? as c_int;
    let r = unsafe { libc::lseek(fd, offset as libc::off_t, whence) };
    if r == -1 {
        Err(syserr("lseek"))
    } else {
        Ok(Value::Number(r as f64))
    }
}

pub fn sys_dup(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.dup expects fd")?, "dup")?;
    let r = unsafe { libc::dup(fd) };
    if r == -1 {
        Err(syserr("dup"))
    } else {
        Ok(Value::Number(r as f64))
    }
}

pub fn sys_dup2(args: Vec<Value>) -> Result<Value, String> {
    let old = fd_from(args.first().ok_or("sys.dup2 expects (fd, newfd)")?, "dup2")?;
    let new = fd_from(args.get(1).ok_or("sys.dup2 expects (fd, newfd)")?, "dup2")?;
    let r = unsafe { libc::dup2(old, new) };
    if r == -1 {
        Err(syserr("dup2"))
    } else {
        Ok(Value::Number(r as f64))
    }
}

pub fn sys_pipe(args: Vec<Value>) -> Result<Value, String> {
    let _ = args;
    let mut fds = [0 as c_int; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } == -1 {
        Err(syserr("pipe"))
    } else {
        Ok(Value::List(Arc::new(vec![
            Value::Number(fds[0] as f64),
            Value::Number(fds[1] as f64),
        ])))
    }
}

pub fn sys_fork(args: Vec<Value>) -> Result<Value, String> {
    let _ = args;
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        Err(syserr("fork"))
    } else {
        Ok(Value::Number(pid as f64))
    }
}

pub fn sys_exec(args: Vec<Value>) -> Result<Value, String> {
    let path = path_cstr(args.first().ok_or("sys.exec expects (path, argv?)")?, "exec")?;
    let mut cargs: Vec<CString> = vec![path.clone()];
    if let Some(Value::List(l)) = args.get(1) {
        for item in l.iter() {
            match item {
                Value::String(s) => cargs.push(
                    CString::new(s.clone())
                        .map_err(|_| "sys.exec: argv contains a NUL byte".to_string())?,
                ),
                _ => return Err("sys.exec: argv must be a list of strings".into()),
            }
        }
    }
    let mut ptrs: Vec<*const c_char> = cargs.iter().map(|c| c.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    unsafe { libc::execv(path.as_ptr(), ptrs.as_ptr()) };
    Err(syserr("exec"))
}

pub fn sys_kill(args: Vec<Value>) -> Result<Value, String> {
    let pid = num_i64(args.first().ok_or("sys.kill expects (pid, sig)")?, "kill")? as libc::pid_t;
    let sig = num_i64(args.get(1).ok_or("sys.kill expects (pid, sig)")?, "kill")? as c_int;
    if unsafe { libc::kill(pid, sig) } == -1 {
        Err(syserr("kill"))
    } else {
        Ok(Value::Bool(true))
    }
}

fn getnum(name: &str, f: unsafe extern "C" fn() -> libc::pid_t) -> Result<Value, String> {
    let _ = name;
    Ok(Value::Number(unsafe { f() } as f64))
}

pub fn sys_getpid(args: Vec<Value>) -> Result<Value, String> {
    let _ = args;
    getnum("getpid", libc::getpid)
}

pub fn sys_getppid(args: Vec<Value>) -> Result<Value, String> {
    let _ = args;
    getnum("getppid", libc::getppid)
}

pub fn sys_getuid(args: Vec<Value>) -> Result<Value, String> {
    let _ = args;
    Ok(Value::Number(unsafe { libc::getuid() } as f64))
}

pub fn sys_geteuid(args: Vec<Value>) -> Result<Value, String> {
    let _ = args;
    Ok(Value::Number(unsafe { libc::geteuid() } as f64))
}

pub fn sys_getgid(args: Vec<Value>) -> Result<Value, String> {
    let _ = args;
    Ok(Value::Number(unsafe { libc::getgid() } as f64))
}

pub fn sys_getegid(args: Vec<Value>) -> Result<Value, String> {
    let _ = args;
    Ok(Value::Number(unsafe { libc::getegid() } as f64))
}

pub fn sys_umask(args: Vec<Value>) -> Result<Value, String> {
    let mask = opt_num_i64(args.first(), 0o022, "umask")? as libc::mode_t;
    let prev = unsafe { libc::umask(mask) };
    Ok(Value::Number(prev as f64))
}

pub fn sys_stat(args: Vec<Value>) -> Result<Value, String> {
    let path = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("sys.stat expects a path".into()),
    };
    do_stat(&path, false)
}

pub fn sys_lstat(args: Vec<Value>) -> Result<Value, String> {
    let path = match args.first() {
        Some(Value::String(s)) => s.clone(),
        _ => return Err("sys.lstat expects a path".into()),
    };
    do_stat(&path, true)
}

pub fn sys_fstat(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.fstat expects fd")?, "fstat")?;
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut st) } == -1 {
        Err(syserr("fstat"))
    } else {
        Ok(stat_dict(&st))
    }
}

pub fn sys_access(args: Vec<Value>) -> Result<Value, String> {
    let path = path_cstr(args.first().ok_or("sys.access expects (path, mode)")?, "access")?;
    let mode = num_i64(args.get(1).ok_or("sys.access expects (path, mode)")?, "access")? as c_int;
    let r = unsafe { libc::access(path.as_ptr(), mode) };
    if r == -1 {
        Err(syserr("access"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_chmod(args: Vec<Value>) -> Result<Value, String> {
    let path = path_cstr(args.first().ok_or("sys.chmod expects (path, mode)")?, "chmod")?;
    let mode = mode_from(args.get(1), 0o644, "chmod")?;
    if unsafe { libc::chmod(path.as_ptr(), mode) } == -1 {
        Err(syserr("chmod"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_chown(args: Vec<Value>) -> Result<Value, String> {
    let path = path_cstr(args.first().ok_or("sys.chown expects (path, uid, gid)")?, "chown")?;
    let uid = num_i64(args.get(1).ok_or("sys.chown expects (path, uid, gid)")?, "chown")? as libc::uid_t;
    let gid = num_i64(args.get(2).ok_or("sys.chown expects (path, uid, gid)")?, "chown")? as libc::gid_t;
    if unsafe { libc::chown(path.as_ptr(), uid, gid) } == -1 {
        Err(syserr("chown"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_fchown(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.fchown expects (fd, uid, gid)")?, "fchown")?;
    let uid = num_i64(args.get(1).ok_or("sys.fchown expects (fd, uid, gid)")?, "fchown")? as libc::uid_t;
    let gid = num_i64(args.get(2).ok_or("sys.fchown expects (fd, uid, gid)")?, "fchown")? as libc::gid_t;
    if unsafe { libc::fchown(fd, uid, gid) } == -1 {
        Err(syserr("fchown"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_symlink(args: Vec<Value>) -> Result<Value, String> {
    let target = path_cstr(args.first().ok_or("sys.symlink expects (target, linkpath)")?, "symlink")?;
    let link = path_cstr(args.get(1).ok_or("sys.symlink expects (target, linkpath)")?, "symlink")?;
    if unsafe { libc::symlink(target.as_ptr(), link.as_ptr()) } == -1 {
        Err(syserr("symlink"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_readlink(args: Vec<Value>) -> Result<Value, String> {
    let path = path_cstr(args.first().ok_or("sys.readlink expects a path")?, "readlink")?;
    let mut buf = vec![0u8; 4096];
    let n = unsafe { libc::readlink(path.as_ptr(), buf.as_mut_ptr() as *mut c_char, 4095) };
    if n < 0 {
        Err(syserr("readlink"))
    } else {
        buf.truncate(n as usize);
        Ok(Value::String(String::from_utf8_lossy(&buf).into_owned()))
    }
}

pub fn sys_link(args: Vec<Value>) -> Result<Value, String> {
    let old = path_cstr(args.first().ok_or("sys.link expects (oldpath, newpath)")?, "link")?;
    let new = path_cstr(args.get(1).ok_or("sys.link expects (oldpath, newpath)")?, "link")?;
    if unsafe { libc::link(old.as_ptr(), new.as_ptr()) } == -1 {
        Err(syserr("link"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_truncate(args: Vec<Value>) -> Result<Value, String> {
    let path = path_cstr(args.first().ok_or("sys.truncate expects (path, length)")?, "truncate")?;
    let len = num_i64(args.get(1).ok_or("sys.truncate expects (path, length)")?, "truncate")? as libc::off_t;
    if unsafe { libc::truncate(path.as_ptr(), len) } == -1 {
        Err(syserr("truncate"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_ftruncate(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.ftruncate expects (fd, length)")?, "ftruncate")?;
    let len = num_i64(args.get(1).ok_or("sys.ftruncate expects (fd, length)")?, "ftruncate")? as libc::off_t;
    if unsafe { libc::ftruncate(fd, len) } == -1 {
        Err(syserr("ftruncate"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_fsync(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.fsync expects fd")?, "fsync")?;
    if unsafe { libc::fsync(fd) } == -1 {
        Err(syserr("fsync"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_fdatasync(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.fdatasync expects fd")?, "fdatasync")?;
    if unsafe { libc::fdatasync(fd) } == -1 {
        Err(syserr("fdatasync"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_unlink(args: Vec<Value>) -> Result<Value, String> {
    let path = path_cstr(args.first().ok_or("sys.unlink expects a path")?, "unlink")?;
    if unsafe { libc::unlink(path.as_ptr()) } == -1 {
        Err(syserr("unlink"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_rmdir(args: Vec<Value>) -> Result<Value, String> {
    let path = path_cstr(args.first().ok_or("sys.rmdir expects a path")?, "rmdir")?;
    if unsafe { libc::rmdir(path.as_ptr()) } == -1 {
        Err(syserr("rmdir"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_mkfifo(args: Vec<Value>) -> Result<Value, String> {
    let path = path_cstr(args.first().ok_or("sys.mkfifo expects (path, mode)")?, "mkfifo")?;
    let mode = mode_from(args.get(1), 0o644, "mkfifo")?;
    if unsafe { libc::mkfifo(path.as_ptr(), mode) } == -1 {
        Err(syserr("mkfifo"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_mknod(args: Vec<Value>) -> Result<Value, String> {
    let path = path_cstr(args.first().ok_or("sys.mknod expects (path, mode, dev)")?, "mknod")?;
    let mode = mode_from(args.get(1), 0o600, "mknod")?;
    let dev = num_i64(args.get(2).ok_or("sys.mknod expects (path, mode, dev)")?, "mknod")? as libc::dev_t;
    if unsafe { libc::mknod(path.as_ptr(), mode, dev) } == -1 {
        Err(syserr("mknod"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_waitpid(args: Vec<Value>) -> Result<Value, String> {
    let pid = num_i64(args.first().ok_or("sys.waitpid expects (pid, options?)")?, "waitpid")? as libc::pid_t;
    let options = match args.get(1) {
        Some(v) => num_i64(v, "waitpid")? as c_int,
        None => 0,
    };
    let mut status: c_int = 0;
    let r = unsafe { libc::waitpid(pid, &mut status, options) };
    if r == -1 {
        Err(syserr("waitpid"))
    } else {
        Ok(Value::List(Arc::new(vec![
            Value::Number(r as f64),
            Value::Number(status as f64),
        ])))
    }
}

macro_rules! wait_macros {
    ($($name:ident: $f:path),* $(,)?) => {
        $(
            pub fn $name(args: Vec<Value>) -> Result<Value, String> {
                let status = num_i64(args.first().ok_or(concat!("sys.", stringify!($name), " expects status"))?, stringify!($name))? as c_int;
                Ok(Value::Bool($f(status)))
            }
        )*
    };
}

wait_macros! {
    sys_wifexited: libc::WIFEXITED,
    sys_wifsignaled: libc::WIFSIGNALED,
    sys_wifstopped: libc::WIFSTOPPED,
}

pub fn sys_wexitstatus(args: Vec<Value>) -> Result<Value, String> {
    let status = num_i64(args.first().ok_or("sys.wexitstatus expects status")?, "wexitstatus")? as c_int;
    Ok(Value::Number(libc::WEXITSTATUS(status) as f64))
}

pub fn sys_wtermsig(args: Vec<Value>) -> Result<Value, String> {
    let status = num_i64(args.first().ok_or("sys.wtermsig expects status")?, "wtermsig")? as c_int;
    Ok(Value::Number(libc::WTERMSIG(status) as f64))
}

pub fn sys_nanosleep(args: Vec<Value>) -> Result<Value, String> {
    let secs = args
        .first()
        .and_then(|v| match v {
            Value::Number(n) => Some(*n),
            _ => None,
        })
        .ok_or("sys.nanosleep expects seconds".to_string())?;
    let req = libc::timespec {
        tv_sec: secs as libc::time_t,
        tv_nsec: ((secs - (secs as i64 as f64)) * 1e9) as i64,
    };
    let mut rem: libc::timespec = unsafe { std::mem::zeroed() };
    if unsafe { libc::nanosleep(&req, &mut rem) } == -1 {
        Err(syserr("nanosleep"))
    } else {
        Ok(Value::Null)
    }
}

pub fn sys_mmap(args: Vec<Value>) -> Result<Value, String> {
    let len = num_u64(args.first().ok_or("sys.mmap expects (len, prot, flags, fd, offset)")?, "mmap")?;
    let prot = num_i64(args.get(1).ok_or("sys.mmap expects (len, prot, flags, fd, offset)")?, "mmap")? as c_int;
    let flags = num_i64(args.get(2).ok_or("sys.mmap expects (len, prot, flags, fd, offset)")?, "mmap")? as c_int;
    let fd = num_i64(args.get(3).ok_or("sys.mmap expects (len, prot, flags, fd, offset)")?, "mmap")? as c_int;
    let offset = num_i64(args.get(4).ok_or("sys.mmap expects (len, prot, flags, fd, offset)")?, "mmap")? as libc::off_t;
    let ptr = unsafe { libc::mmap(std::ptr::null_mut(), len as usize, prot, flags, fd, offset) };
    if ptr == libc::MAP_FAILED {
        Err(syserr("mmap"))
    } else {
        Ok(Value::Number(ptr as usize as f64))
    }
}

pub fn sys_munmap(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.munmap expects (addr, len)")?, "munmap")? as *mut c_void;
    let len = num_u64(args.get(1).ok_or("sys.munmap expects (addr, len)")?, "munmap")?;
    if unsafe { libc::munmap(addr, len as usize) } == -1 {
        Err(syserr("munmap"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_mprotect(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.mprotect expects (addr, len, prot)")?, "mprotect")? as *mut c_void;
    let len = num_u64(args.get(1).ok_or("sys.mprotect expects (addr, len, prot)")?, "mprotect")?;
    let prot = num_i64(args.get(2).ok_or("sys.mprotect expects (addr, len, prot)")?, "mprotect")? as c_int;
    if unsafe { libc::mprotect(addr, len as usize, prot) } == -1 {
        Err(syserr("mprotect"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_mremap(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.mremap expects (addr, old_size, new_size, flags)")?, "mremap")? as *mut c_void;
    let old_size = num_u64(args.get(1).ok_or("sys.mremap expects (addr, old_size, new_size, flags)")?, "mremap")?;
    let new_size = num_u64(args.get(2).ok_or("sys.mremap expects (addr, old_size, new_size, flags)")?, "mremap")?;
    let flags = num_i64(args.get(3).ok_or("sys.mremap expects (addr, old_size, new_size, flags)")?, "mremap")? as c_int;
    let ptr = unsafe { libc::mremap(addr, old_size as usize, new_size as usize, flags) };
    if ptr == libc::MAP_FAILED {
        Err(syserr("mremap"))
    } else {
        Ok(Value::Number(ptr as usize as f64))
    }
}

pub fn sys_mlock(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.mlock expects (addr, len)")?, "mlock")? as *const c_void;
    let len = num_u64(args.get(1).ok_or("sys.mlock expects (addr, len)")?, "mlock")?;
    if unsafe { libc::mlock(addr, len as usize) } == -1 {
        Err(syserr("mlock"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_munlock(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.munlock expects (addr, len)")?, "munlock")? as *const c_void;
    let len = num_u64(args.get(1).ok_or("sys.munlock expects (addr, len)")?, "munlock")?;
    if unsafe { libc::munlock(addr, len as usize) } == -1 {
        Err(syserr("munlock"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_peek8(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.peek* expects an address")?, "peek8")? as *const u8;
    Ok(Value::Number(unsafe { std::ptr::read_unaligned(addr) } as f64))
}

pub fn sys_peek16(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.peek* expects an address")?, "peek16")? as *const u16;
    Ok(Value::Number(unsafe { std::ptr::read_unaligned(addr) } as f64))
}

pub fn sys_peek32(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.peek* expects an address")?, "peek32")? as *const u32;
    Ok(Value::Number(unsafe { std::ptr::read_unaligned(addr) } as f64))
}

pub fn sys_peek64(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.peek* expects an address")?, "peek64")? as *const u64;
    Ok(Value::Number(unsafe { std::ptr::read_unaligned(addr) } as f64))
}

pub fn sys_poke8(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.poke* expects (addr, value)")?, "poke8")? as *mut u8;
    let val = num_u64(args.get(1).ok_or("sys.poke* expects (addr, value)")?, "poke8")? as u8;
    unsafe { std::ptr::write_unaligned(addr, val) };
    Ok(Value::Null)
}

pub fn sys_poke16(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.poke* expects (addr, value)")?, "poke16")? as *mut u16;
    let val = num_u64(args.get(1).ok_or("sys.poke* expects (addr, value)")?, "poke16")? as u16;
    unsafe { std::ptr::write_unaligned(addr, val) };
    Ok(Value::Null)
}

pub fn sys_poke32(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.poke* expects (addr, value)")?, "poke32")? as *mut u32;
    let val = num_u64(args.get(1).ok_or("sys.poke* expects (addr, value)")?, "poke32")? as u32;
    unsafe { std::ptr::write_unaligned(addr, val) };
    Ok(Value::Null)
}

pub fn sys_poke64(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.poke* expects (addr, value)")?, "poke64")? as *mut u64;
    let val = num_u64(args.get(1).ok_or("sys.poke* expects (addr, value)")?, "poke64")?;
    unsafe { std::ptr::write_unaligned(addr, val) };
    Ok(Value::Null)
}

pub fn sys_mem_read(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.mem_read expects (addr, len)")?, "mem_read")? as *const u8;
    let len = num_u64(args.get(1).ok_or("sys.mem_read expects (addr, len)")?, "mem_read")? as usize;
    let slice = unsafe { std::slice::from_raw_parts(addr, len) };
    Ok(buf_hex(slice))
}

pub fn sys_mem_write(args: Vec<Value>) -> Result<Value, String> {
    let addr = num_u64(args.first().ok_or("sys.mem_write expects (addr, hex)")?, "mem_write")? as *mut u8;
    let bytes = hex_data(args.get(1).ok_or("sys.mem_write expects (addr, hex)")?, "mem_write")?;
    if bytes.is_empty() {
        return Ok(Value::Null);
    }
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), addr, bytes.len()) };
    Ok(Value::Null)
}

pub fn sys_ioctl(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.ioctl expects (fd, request, arg?)")?, "ioctl")?;
    let request = num_u64(args.get(1).ok_or("sys.ioctl expects (fd, request, arg?)")?, "ioctl")? as c_ulong;
    let r = match args.get(2) {
        None => unsafe { libc::ioctl(fd, request) },
        Some(Value::Number(n)) => unsafe { libc::ioctl(fd, request, *n as c_ulong) },
        Some(Value::Null) => unsafe { libc::ioctl(fd, request, 0 as c_ulong) },
        Some(Value::String(s)) => {
            let c = CString::new(s.clone())
                .map_err(|_| "sys.ioctl: buffer contains a NUL byte".to_string())?;
            unsafe { libc::ioctl(fd, request, c.as_ptr() as c_ulong) }
        }
        Some(v) => {
            return Err(format!(
                "sys.ioctl: arg must be a number, string buffer, or null, got {}",
                v.type_name()
            ))
        }
    };
    if r == -1 {
        Err(syserr("ioctl"))
    } else {
        Ok(Value::Number(r as f64))
    }
}

pub fn sys_fcntl(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.fcntl expects (fd, cmd, arg?)")?, "fcntl")?;
    let cmd = num_i64(args.get(1).ok_or("sys.fcntl expects (fd, cmd, arg?)")?, "fcntl")? as c_int;
    let r = match args.get(2) {
        None => unsafe { libc::fcntl(fd, cmd) },
        Some(Value::Number(n)) => unsafe { libc::fcntl(fd, cmd, *n as c_int) },
        Some(Value::Null) => unsafe { libc::fcntl(fd, cmd, 0 as c_int) },
        Some(Value::String(s)) => {
            let c = CString::new(s.clone())
                .map_err(|_| "sys.fcntl: buffer contains a NUL byte".to_string())?;
            unsafe { libc::fcntl(fd, cmd, c.as_ptr() as c_int) }
        }
        Some(v) => {
            return Err(format!(
                "sys.fcntl: arg must be a number, string, or null, got {}",
                v.type_name()
            ))
        }
    };
    if r == -1 {
        Err(syserr("fcntl"))
    } else {
        Ok(Value::Number(r as f64))
    }
}

pub fn sys_poll(args: Vec<Value>) -> Result<Value, String> {
    let entries = match args.first() {
        Some(Value::List(l)) => l.clone(),
        _ => return Err("sys.poll expects (fds, timeout_ms?)".into()),
    };
    let mut fds: Vec<libc::pollfd> = Vec::with_capacity(entries.len());
    for e in entries.iter() {
        let d = match e {
            Value::Dict(d) => d.clone(),
            _ => return Err("sys.poll: entries must be dicts {\"fd\": n, \"events\": n}".into()),
        };
        let fd = match d.get("fd") {
            Some(Value::Number(n)) => *n as c_int,
            _ => return Err("sys.poll: entry missing numeric \"fd\"".into()),
        };
        let events = match d.get("events") {
            Some(Value::Number(n)) => *n as i16,
            _ => return Err("sys.poll: entry missing numeric \"events\"".into()),
        };
        fds.push(libc::pollfd { fd, events, revents: 0 });
    }
    let timeout = match args.get(1) {
        Some(Value::Number(n)) => *n as c_int,
        _ => -1,
    };
    let r = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) };
    if r == -1 {
        Err(syserr("poll"))
    } else {
        let mut out: Vec<Value> = Vec::new();
        for f in fds.iter().filter(|f| f.revents != 0) {
            let mut m = indexmap::IndexMap::new();
            m.insert("fd".into(), Value::Number(f.fd as f64));
            m.insert("revents".into(), Value::Number(f.revents as f64));
            out.push(Value::Dict(Arc::new(m)));
        }
        Ok(Value::List(Arc::new(out)))
    }
}

// ---------------------------------------------------------------------------
// sockets (raw, hex-addressed — enough for a pure-Zen socket stack)
// ---------------------------------------------------------------------------

fn optval_bytes(v: &Value, what: &str) -> Result<Vec<u8>, String> {
    match v {
        Value::String(s) => crate::runtime::hex_decode(s)
            .ok_or_else(|| format!("sys.{what}: invalid hex data")),
        Value::Number(n) => Ok((*n as c_int).to_ne_bytes().to_vec()),
        Value::Bool(b) => Ok((*b as c_int).to_ne_bytes().to_vec()),
        _ => Err(format!(
            "sys.{what}: expected a number or hex string, got {}",
            v.type_name()
        )),
    }
}

fn sockaddr_hex(fd: c_int, peer: bool, what: &str) -> Result<Value, String> {
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
    Ok(buf_hex(bytes))
}

pub fn sys_socket(args: Vec<Value>) -> Result<Value, String> {
    let family = opt_num_i64(args.first(), libc::AF_INET as i64, "socket")? as c_int;
    let ty = opt_num_i64(args.get(1), libc::SOCK_STREAM as i64, "socket")? as c_int;
    let proto = opt_num_i64(args.get(2), 0, "socket")? as c_int;
    let fd = unsafe { libc::socket(family, ty, proto) };
    if fd < 0 {
        Err(syserr("socket"))
    } else {
        Ok(Value::Number(fd as f64))
    }
}

pub fn sys_socketpair(args: Vec<Value>) -> Result<Value, String> {
    let family = opt_num_i64(args.first(), libc::AF_UNIX as i64, "socketpair")? as c_int;
    let ty = opt_num_i64(args.get(1), libc::SOCK_STREAM as i64, "socketpair")? as c_int;
    let proto = opt_num_i64(args.get(2), 0, "socketpair")? as c_int;
    let mut fds = [0 as c_int; 2];
    if unsafe { libc::socketpair(family, ty, proto, fds.as_mut_ptr()) } == -1 {
        Err(syserr("socketpair"))
    } else {
        Ok(Value::List(Arc::new(vec![
            Value::Number(fds[0] as f64),
            Value::Number(fds[1] as f64),
        ])))
    }
}

pub fn sys_bind(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.bind expects (fd, addr_hex)")?, "bind")?;
    let addr = hex_data(args.get(1).ok_or("sys.bind expects (fd, addr_hex)")?, "bind")?;
    let rc = unsafe {
        libc::bind(
            fd,
            addr.as_ptr() as *const libc::sockaddr,
            addr.len() as libc::socklen_t,
        )
    };
    if rc == -1 {
        Err(syserr("bind"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_listen(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.listen expects (fd, backlog?)")?, "listen")?;
    let backlog = opt_num_i64(args.get(1), 128, "listen")? as c_int;
    if unsafe { libc::listen(fd, backlog) } == -1 {
        Err(syserr("listen"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_accept(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.accept expects (fd)")?, "accept")?;
    let c = unsafe { libc::accept(fd, std::ptr::null_mut(), std::ptr::null_mut()) };
    if c < 0 {
        Err(syserr("accept"))
    } else {
        Ok(Value::Number(c as f64))
    }
}

pub fn sys_accept4(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.accept4 expects (fd, flags?)")?, "accept4")?;
    let flags = opt_num_i64(args.get(1), 0, "accept4")? as c_int;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let c = unsafe { libc::accept4(fd, std::ptr::null_mut(), std::ptr::null_mut(), flags) };
        if c < 0 {
            return Err(syserr("accept4"));
        }
        Ok(Value::Number(c as f64))
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = flags;
        let c = unsafe { libc::accept(fd, std::ptr::null_mut(), std::ptr::null_mut()) };
        if c < 0 {
            Err(syserr("accept"))
        } else {
            Ok(Value::Number(c as f64))
        }
    }
}

pub fn sys_connect(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.connect expects (fd, addr_hex)")?, "connect")?;
    let addr = hex_data(args.get(1).ok_or("sys.connect expects (fd, addr_hex)")?, "connect")?;
    let rc = unsafe {
        libc::connect(
            fd,
            addr.as_ptr() as *const libc::sockaddr,
            addr.len() as libc::socklen_t,
        )
    };
    if rc == -1 {
        Err(syserr("connect"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_getsockname(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.getsockname expects (fd)")?, "getsockname")?;
    sockaddr_hex(fd, false, "getsockname")
}

pub fn sys_getpeername(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.getpeername expects (fd)")?, "getpeername")?;
    sockaddr_hex(fd, true, "getpeername")
}

pub fn sys_send(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.send expects (fd, data_hex, flags?)")?, "send")?;
    let data = hex_data(args.get(1).ok_or("sys.send expects (fd, data_hex, flags?)")?, "send")?;
    let flags = opt_num_i64(args.get(2), 0, "send")? as c_int;
    let n = unsafe { libc::send(fd, data.as_ptr() as *const c_void, data.len(), flags) };
    if n < 0 {
        Err(syserr("send"))
    } else {
        Ok(Value::Number(n as f64))
    }
}

pub fn sys_sendto(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(
        args.first().ok_or("sys.sendto expects (fd, data_hex, addr_hex, flags?)")?,
        "sendto",
    )?;
    let data = hex_data(
        args.get(1).ok_or("sys.sendto expects (fd, data_hex, addr_hex, flags?)")?,
        "sendto",
    )?;
    let addr = hex_data(
        args.get(2).ok_or("sys.sendto expects (fd, data_hex, addr_hex, flags?)")?,
        "sendto",
    )?;
    let flags = opt_num_i64(args.get(3), 0, "sendto")? as c_int;
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
        Err(syserr("sendto"))
    } else {
        Ok(Value::Number(n as f64))
    }
}

pub fn sys_recv(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.recv expects (fd, maxlen?, flags?)")?, "recv")?;
    let maxlen = opt_num_i64(args.get(1), 65536, "recv")?.max(0) as usize;
    let flags = opt_num_i64(args.get(2), 0, "recv")? as c_int;
    let mut buf = vec![0u8; maxlen];
    let n = unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut c_void, maxlen, flags) };
    if n < 0 {
        Err(syserr("recv"))
    } else {
        Ok(buf_hex(&buf[..n as usize]))
    }
}

pub fn sys_recvfrom(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(
        args.first().ok_or("sys.recvfrom expects (fd, maxlen?, flags?)")?,
        "recvfrom",
    )?;
    let maxlen = opt_num_i64(args.get(1), 65536, "recvfrom")?.max(0) as usize;
    let flags = opt_num_i64(args.get(2), 0, "recvfrom")? as c_int;
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
    Ok(Value::List(Arc::new(vec![
        buf_hex(&buf[..n as usize]),
        buf_hex(addr),
    ])))
}

pub fn sys_shutdown(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(args.first().ok_or("sys.shutdown expects (fd, how?)")?, "shutdown")?;
    let how = opt_num_i64(args.get(1), libc::SHUT_RDWR as i64, "shutdown")? as c_int;
    if unsafe { libc::shutdown(fd, how) } == -1 {
        Err(syserr("shutdown"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_setsockopt(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(
        args.first().ok_or("sys.setsockopt expects (fd, level, optname, value)")?,
        "setsockopt",
    )?;
    let level = num_i64(
        args.get(1).ok_or("sys.setsockopt expects (fd, level, optname, value)")?,
        "setsockopt",
    )? as c_int;
    let optname = num_i64(
        args.get(2).ok_or("sys.setsockopt expects (fd, level, optname, value)")?,
        "setsockopt",
    )? as c_int;
    let val = optval_bytes(
        args.get(3).ok_or("sys.setsockopt expects (fd, level, optname, value)")?,
        "setsockopt",
    )?;
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
        Err(syserr("setsockopt"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_getsockopt(args: Vec<Value>) -> Result<Value, String> {
    let fd = fd_from(
        args.first().ok_or("sys.getsockopt expects (fd, level, optname, optlen?)")?,
        "getsockopt",
    )?;
    let level = num_i64(
        args.get(1).ok_or("sys.getsockopt expects (fd, level, optname, optlen?)")?,
        "getsockopt",
    )? as c_int;
    let optname = num_i64(
        args.get(2).ok_or("sys.getsockopt expects (fd, level, optname, optlen?)")?,
        "getsockopt",
    )? as c_int;
    let want = opt_num_i64(args.get(3), 4, "getsockopt")?.max(0) as usize;
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
        Err(syserr("getsockopt"))
    } else {
        Ok(buf_hex(&buf[..len as usize]))
    }
}

pub fn sys_gethostname(args: Vec<Value>) -> Result<Value, String> {
    let _ = args;
    let mut buf = vec![0u8; 256];
    if unsafe { libc::gethostname(buf.as_mut_ptr() as *mut c_char, buf.len()) } == -1 {
        Err(syserr("gethostname"))
    } else {
        let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        buf.truncate(len);
        Ok(Value::String(String::from_utf8_lossy(&buf).into_owned()))
    }
}

pub fn sys_uname(args: Vec<Value>) -> Result<Value, String> {
    let _ = args;
    let mut u: libc::utsname = unsafe { std::mem::zeroed() };
    if unsafe { libc::uname(&mut u) } == -1 {
        return Err(syserr("uname"));
    }
    fn f2s(f: [c_char; 65]) -> String {
        let raw = unsafe { CStr::from_ptr(f.as_ptr()) };
        raw.to_string_lossy().into_owned()
    }
    let mut m = indexmap::IndexMap::new();
    m.insert("sysname".into(), Value::String(f2s(u.sysname)));
    m.insert("nodename".into(), Value::String(f2s(u.nodename)));
    m.insert("release".into(), Value::String(f2s(u.release)));
    m.insert("version".into(), Value::String(f2s(u.version)));
    m.insert("machine".into(), Value::String(f2s(u.machine)));
    Ok(Value::Dict(Arc::new(m)))
}

pub fn sys_clock_gettime(args: Vec<Value>) -> Result<Value, String> {
    let clk = num_i64(args.first().ok_or("sys.clock_gettime expects a clock id")?, "clock_gettime")? as c_int;
    let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
    if unsafe { libc::clock_gettime(clk, &mut ts) } == -1 {
        Err(syserr("clock_gettime"))
    } else {
        Ok(Value::Number(ts.tv_sec as f64 + ts.tv_nsec as f64 / 1e9))
    }
}

pub fn sys_getrlimit(args: Vec<Value>) -> Result<Value, String> {
    let which = num_i64(args.first().ok_or("sys.getrlimit expects a resource")?, "getrlimit")? as libc::__rlimit_resource_t;
    let mut rl: libc::rlimit = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrlimit(which, &mut rl) } == -1 {
        Err(syserr("getrlimit"))
    } else {
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
        Ok(Value::List(Arc::new(vec![
            Value::Number(soft),
            Value::Number(hard),
        ])))
    }
}

pub fn sys_setrlimit(args: Vec<Value>) -> Result<Value, String> {
    let which = num_i64(args.first().ok_or("sys.setrlimit expects (resource, [soft, hard])")?, "setrlimit")? as libc::__rlimit_resource_t;
    let limits = match args.get(1) {
        Some(Value::List(l)) => l.clone(),
        _ => return Err("sys.setrlimit expects (resource, [soft, hard])".into()),
    };
    let to_rlim = |v: &Value| -> libc::rlim_t {
        match v {
            Value::Number(n) if *n < 0.0 => libc::RLIM_INFINITY,
            Value::Number(n) => *n as libc::rlim_t,
            _ => libc::RLIM_INFINITY,
        }
    };
    let soft = to_rlim(limits.first().unwrap_or(&Value::Number(-1.0)));
    let hard = to_rlim(limits.get(1).unwrap_or(&Value::Number(-1.0)));
    let rl = libc::rlimit {
        rlim_cur: soft,
        rlim_max: hard,
    };
    if unsafe { libc::setrlimit(which, &rl) } == -1 {
        Err(syserr("setrlimit"))
    } else {
        Ok(Value::Bool(true))
    }
}

pub fn sys_sysconf(args: Vec<Value>) -> Result<Value, String> {
    let name = num_i64(args.first().ok_or("sys.sysconf expects a name")?, "sysconf")? as c_int;
    let r = unsafe { libc::sysconf(name) };
    if r == -1 {
        Err(syserr("sysconf"))
    } else {
        Ok(Value::Number(r as f64))
    }
}

pub fn sys_times(args: Vec<Value>) -> Result<Value, String> {
    let _ = args;
    let mut t: libc::tms = unsafe { std::mem::zeroed() };
    if unsafe { libc::times(&mut t) } == -1 {
        return Err(syserr("times"));
    }
    let hz = 100.0_f64; // times() uses CLK_TCK = 100 on Linux
    fn secs(v: libc::clock_t, hz: f64) -> Value {
        Value::Number(v as f64 / hz)
    }
    let mut m = indexmap::IndexMap::new();
    m.insert("utime".into(), secs(t.tms_utime, hz));
    m.insert("stime".into(), secs(t.tms_stime, hz));
    m.insert("cutime".into(), secs(t.tms_cutime, hz));
    m.insert("cstime".into(), secs(t.tms_cstime, hz));
    Ok(Value::Dict(Arc::new(m)))
}

pub fn sys_strerror(args: Vec<Value>) -> Result<Value, String> {
    let e = num_i64(args.first().ok_or("sys.strerror expects an errno")?, "strerror")? as i32;
    Ok(Value::String(errno_message(e)))
}

pub fn sys_errno(args: Vec<Value>) -> Result<Value, String> {
    let _ = args;
    Ok(Value::Number(errno_now() as f64))
}

// ---------------------------------------------------------------------------
// module registration
// ---------------------------------------------------------------------------

fn mkconst(pairs: &[(&str, i64)]) -> indexmap::IndexMap<String, Value> {
    let mut m = indexmap::IndexMap::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), Value::Number(*v as f64));
    }
    m
}

pub fn init_sys_module(vm: &mut crate::runtime::Vm) {
    let mut fns = indexmap::IndexMap::new();
    let mut add = |name: &str, n: &str| {
        fns.insert(name.to_string(), Value::NativeFunction(n.to_string()));
    };
    add("syscall", "sys_syscall");
    add("open", "sys_open");
    add("close", "sys_close");
    add("read", "sys_read");
    add("read_str", "sys_read_str");
    add("write", "sys_write");
    add("write_bytes", "sys_write_bytes");
    add("lseek", "sys_lseek");
    add("dup", "sys_dup");
    add("dup2", "sys_dup2");
    add("pipe", "sys_pipe");
    add("fork", "sys_fork");
    add("exec", "sys_exec");
    add("kill", "sys_kill");
    add("getpid", "sys_getpid");
    add("getppid", "sys_getppid");
    add("getuid", "sys_getuid");
    add("geteuid", "sys_geteuid");
    add("getgid", "sys_getgid");
    add("getegid", "sys_getegid");
    add("umask", "sys_umask");
    add("stat", "sys_stat");
    add("lstat", "sys_lstat");
    add("fstat", "sys_fstat");
    add("access", "sys_access");
    add("chmod", "sys_chmod");
    add("chown", "sys_chown");
    add("fchown", "sys_fchown");
    add("symlink", "sys_symlink");
    add("readlink", "sys_readlink");
    add("link", "sys_link");
    add("truncate", "sys_truncate");
    add("ftruncate", "sys_ftruncate");
    add("fsync", "sys_fsync");
    add("fdatasync", "sys_fdatasync");
    add("unlink", "sys_unlink");
    add("rmdir", "sys_rmdir");
    add("mkfifo", "sys_mkfifo");
    add("mknod", "sys_mknod");
    add("waitpid", "sys_waitpid");
    add("wifexited", "sys_wifexited");
    add("wifsignaled", "sys_wifsignaled");
    add("wifstopped", "sys_wifstopped");
    add("wexitstatus", "sys_wexitstatus");
    add("wtermsig", "sys_wtermsig");
    add("nanosleep", "sys_nanosleep");
    add("mmap", "sys_mmap");
    add("munmap", "sys_munmap");
    add("mprotect", "sys_mprotect");
    add("mremap", "sys_mremap");
    add("mlock", "sys_mlock");
    add("munlock", "sys_munlock");
    add("peek8", "sys_peek8");
    add("peek16", "sys_peek16");
    add("peek32", "sys_peek32");
    add("peek64", "sys_peek64");
    add("poke8", "sys_poke8");
    add("poke16", "sys_poke16");
    add("poke32", "sys_poke32");
    add("poke64", "sys_poke64");
    add("mem_read", "sys_mem_read");
    add("mem_write", "sys_mem_write");
    add("ioctl", "sys_ioctl");
    add("fcntl", "sys_fcntl");
    add("poll", "sys_poll");
    add("socket", "sys_socket");
    add("socketpair", "sys_socketpair");
    add("bind", "sys_bind");
    add("listen", "sys_listen");
    add("accept", "sys_accept");
    add("accept4", "sys_accept4");
    add("connect", "sys_connect");
    add("getsockname", "sys_getsockname");
    add("getpeername", "sys_getpeername");
    add("send", "sys_send");
    add("sendto", "sys_sendto");
    add("recv", "sys_recv");
    add("recvfrom", "sys_recvfrom");
    add("shutdown", "sys_shutdown");
    add("setsockopt", "sys_setsockopt");
    add("getsockopt", "sys_getsockopt");
    add("gethostname", "sys_gethostname");
    add("uname", "sys_uname");
    add("clock_gettime", "sys_clock_gettime");
    add("getrlimit", "sys_getrlimit");
    add("setrlimit", "sys_setrlimit");
    add("sysconf", "sys_sysconf");
    add("times", "sys_times");
    add("strerror", "sys_strerror");
    add("errno", "sys_errno");

    // Constants (values taken straight from libc so they always match the ABI).
    let prot = mkconst(&[
        ("PROT_NONE", libc::PROT_NONE as i64),
        ("PROT_READ", libc::PROT_READ as i64),
        ("PROT_WRITE", libc::PROT_WRITE as i64),
        ("PROT_EXEC", libc::PROT_EXEC as i64),
        ("PROT_GROWSDOWN", libc::PROT_GROWSDOWN as i64),
        ("PROT_GROWSUP", libc::PROT_GROWSUP as i64),
    ]);
    let map = mkconst(&[
        ("MAP_SHARED", libc::MAP_SHARED as i64),
        ("MAP_PRIVATE", libc::MAP_PRIVATE as i64),
        ("MAP_FIXED", libc::MAP_FIXED as i64),
        ("MAP_ANONYMOUS", libc::MAP_ANONYMOUS as i64),
        ("MAP_ANON", libc::MAP_ANONYMOUS as i64),
        ("MAP_GROWSDOWN", libc::MAP_GROWSDOWN as i64),
        ("MAP_STACK", libc::MAP_STACK as i64),
        ("MAP_DENYWRITE", libc::MAP_DENYWRITE as i64),
        ("MAP_EXECUTABLE", libc::MAP_EXECUTABLE as i64),
        ("MAP_LOCKED", libc::MAP_LOCKED as i64),
        ("MAP_NORESERVE", libc::MAP_NORESERVE as i64),
        ("MAP_POPULATE", libc::MAP_POPULATE as i64),
        ("MAP_HUGETLB", libc::MAP_HUGETLB as i64),
    ]);
    let oflags = mkconst(&[
        ("O_RDONLY", libc::O_RDONLY as i64),
        ("O_WRONLY", libc::O_WRONLY as i64),
        ("O_RDWR", libc::O_RDWR as i64),
        ("O_CREAT", libc::O_CREAT as i64),
        ("O_EXCL", libc::O_EXCL as i64),
        ("O_TRUNC", libc::O_TRUNC as i64),
        ("O_APPEND", libc::O_APPEND as i64),
        ("O_SYNC", libc::O_SYNC as i64),
        ("O_DSYNC", libc::O_DSYNC as i64),
        ("O_RSYNC", libc::O_RSYNC as i64),
        ("O_NONBLOCK", libc::O_NONBLOCK as i64),
        ("O_CLOEXEC", libc::O_CLOEXEC as i64),
        ("O_DIRECTORY", libc::O_DIRECTORY as i64),
        ("O_NOCTTY", libc::O_NOCTTY as i64),
        ("O_NOFOLLOW", libc::O_NOFOLLOW as i64),
        ("O_DIRECT", libc::O_DIRECT as i64),
        ("O_LARGEFILE", libc::O_LARGEFILE as i64),
        ("O_ASYNC", libc::O_ASYNC as i64),
    ]);
    let seek = mkconst(&[
        ("SEEK_SET", libc::SEEK_SET as i64),
        ("SEEK_CUR", libc::SEEK_CUR as i64),
        ("SEEK_END", libc::SEEK_END as i64),
    ]);
    let acc = mkconst(&[
        ("F_OK", libc::F_OK as i64),
        ("X_OK", libc::X_OK as i64),
        ("W_OK", libc::W_OK as i64),
        ("R_OK", libc::R_OK as i64),
    ]);
    let wait = mkconst(&[
        ("WNOHANG", libc::WNOHANG as i64),
        ("WUNTRACED", libc::WUNTRACED as i64),
        ("WCONTINUED", libc::WCONTINUED as i64),
    ]);
    let pollc = mkconst(&[
        ("POLLIN", libc::POLLIN as i64),
        ("POLLPRI", libc::POLLPRI as i64),
        ("POLLOUT", libc::POLLOUT as i64),
        ("POLLERR", libc::POLLERR as i64),
        ("POLLHUP", libc::POLLHUP as i64),
        ("POLLNVAL", libc::POLLNVAL as i64),
        ("POLLRDNORM", libc::POLLRDNORM as i64),
        ("POLLWRNORM", libc::POLLWRNORM as i64),
    ]);
    let clocks = mkconst(&[
        ("CLOCK_REALTIME", libc::CLOCK_REALTIME as i64),
        ("CLOCK_MONOTONIC", libc::CLOCK_MONOTONIC as i64),
        ("CLOCK_PROCESS_CPUTIME_ID", libc::CLOCK_PROCESS_CPUTIME_ID as i64),
        ("CLOCK_THREAD_CPUTIME_ID", libc::CLOCK_THREAD_CPUTIME_ID as i64),
        ("CLOCK_MONOTONIC_RAW", libc::CLOCK_MONOTONIC_RAW as i64),
        ("CLOCK_REALTIME_COARSE", libc::CLOCK_REALTIME_COARSE as i64),
        ("CLOCK_MONOTONIC_COARSE", libc::CLOCK_MONOTONIC_COARSE as i64),
        ("CLOCK_BOOTTIME", libc::CLOCK_BOOTTIME as i64),
    ]);
    let rl = mkconst(&[
        ("RLIMIT_CPU", libc::RLIMIT_CPU as i64),
        ("RLIMIT_FSIZE", libc::RLIMIT_FSIZE as i64),
        ("RLIMIT_DATA", libc::RLIMIT_DATA as i64),
        ("RLIMIT_STACK", libc::RLIMIT_STACK as i64),
        ("RLIMIT_CORE", libc::RLIMIT_CORE as i64),
        ("RLIMIT_RSS", libc::RLIMIT_RSS as i64),
        ("RLIMIT_NPROC", libc::RLIMIT_NPROC as i64),
        ("RLIMIT_NOFILE", libc::RLIMIT_NOFILE as i64),
        ("RLIMIT_MEMLOCK", libc::RLIMIT_MEMLOCK as i64),
        ("RLIMIT_AS", libc::RLIMIT_AS as i64),
        ("RLIMIT_LOCKS", libc::RLIMIT_LOCKS as i64),
        ("RLIMIT_SIGPENDING", libc::RLIMIT_SIGPENDING as i64),
        ("RLIMIT_MSGQUEUE", libc::RLIMIT_MSGQUEUE as i64),
        ("RLIMIT_NICE", libc::RLIMIT_NICE as i64),
        ("RLIMIT_RTPRIO", libc::RLIMIT_RTPRIO as i64),
        ("RLIMIT_RTTIME", libc::RLIMIT_RTTIME as i64),
    ]);
    let fcntlc = mkconst(&[
        ("F_DUPFD", libc::F_DUPFD as i64),
        ("F_DUPFD_CLOEXEC", libc::F_DUPFD_CLOEXEC as i64),
        ("F_GETFD", libc::F_GETFD as i64),
        ("F_SETFD", libc::F_SETFD as i64),
        ("F_GETFL", libc::F_GETFL as i64),
        ("F_SETFL", libc::F_SETFL as i64),
        ("F_GETLK", libc::F_GETLK as i64),
        ("F_SETLK", libc::F_SETLK as i64),
        ("F_SETLKW", libc::F_SETLKW as i64),
        ("F_SETOWN", libc::F_SETOWN as i64),
        ("F_GETOWN", libc::F_GETOWN as i64),
        ("FD_CLOEXEC", libc::FD_CLOEXEC as i64),
    ]);
    let sigs = mkconst(&[
        ("SIGHUP", libc::SIGHUP as i64),
        ("SIGINT", libc::SIGINT as i64),
        ("SIGQUIT", libc::SIGQUIT as i64),
        ("SIGILL", libc::SIGILL as i64),
        ("SIGTRAP", libc::SIGTRAP as i64),
        ("SIGABRT", libc::SIGABRT as i64),
        ("SIGBUS", libc::SIGBUS as i64),
        ("SIGFPE", libc::SIGFPE as i64),
        ("SIGKILL", libc::SIGKILL as i64),
        ("SIGUSR1", libc::SIGUSR1 as i64),
        ("SIGSEGV", libc::SIGSEGV as i64),
        ("SIGUSR2", libc::SIGUSR2 as i64),
        ("SIGPIPE", libc::SIGPIPE as i64),
        ("SIGALRM", libc::SIGALRM as i64),
        ("SIGTERM", libc::SIGTERM as i64),
        ("SIGCHLD", libc::SIGCHLD as i64),
        ("SIGCONT", libc::SIGCONT as i64),
        ("SIGSTOP", libc::SIGSTOP as i64),
        ("SIGTSTP", libc::SIGTSTP as i64),
        ("SIGTTIN", libc::SIGTTIN as i64),
        ("SIGTTOU", libc::SIGTTOU as i64),
        ("SIGURG", libc::SIGURG as i64),
        ("SIGXCPU", libc::SIGXCPU as i64),
        ("SIGXFSZ", libc::SIGXFSZ as i64),
        ("SIGVTALRM", libc::SIGVTALRM as i64),
        ("SIGPROF", libc::SIGPROF as i64),
        ("SIGWINCH", libc::SIGWINCH as i64),
        ("SIGIO", libc::SIGIO as i64),
        ("SIGSYS", libc::SIGSYS as i64),
    ]);
    let conv = mkconst(&[
        ("SC_CLK_TCK", libc::_SC_CLK_TCK as i64),
        ("SC_NPROCESSORS_ONLN", libc::_SC_NPROCESSORS_ONLN as i64),
        ("SC_PAGESIZE", libc::_SC_PAGESIZE as i64),
        ("SC_PHYS_PAGES", libc::_SC_PHYS_PAGES as i64),
        ("SC_OPEN_MAX", libc::_SC_OPEN_MAX as i64),
        ("SC_ARG_MAX", libc::_SC_ARG_MAX as i64),
        ("SC_CHILD_MAX", libc::_SC_CHILD_MAX as i64),
        ("SC_HOST_NAME_MAX", libc::_SC_HOST_NAME_MAX as i64),
    ]);
    let af = mkconst(&[
        ("AF_UNIX", libc::AF_UNIX as i64),
        ("AF_LOCAL", libc::AF_UNIX as i64),
        ("AF_INET", libc::AF_INET as i64),
        ("AF_INET6", libc::AF_INET6 as i64),
        ("AF_PACKET", libc::AF_PACKET as i64),
        ("AF_NETLINK", libc::AF_NETLINK as i64),
    ]);
    let socktype = mkconst(&[
        ("SOCK_STREAM", libc::SOCK_STREAM as i64),
        ("SOCK_DGRAM", libc::SOCK_DGRAM as i64),
        ("SOCK_SEQPACKET", libc::SOCK_SEQPACKET as i64),
        ("SOCK_RAW", libc::SOCK_RAW as i64),
        ("SOCK_NONBLOCK", libc::SOCK_NONBLOCK as i64),
        ("SOCK_CLOEXEC", libc::SOCK_CLOEXEC as i64),
    ]);
    let ipproto = mkconst(&[
        ("IPPROTO_IP", libc::IPPROTO_IP as i64),
        ("IPPROTO_ICMP", libc::IPPROTO_ICMP as i64),
        ("IPPROTO_TCP", libc::IPPROTO_TCP as i64),
        ("IPPROTO_UDP", libc::IPPROTO_UDP as i64),
        ("IPPROTO_RAW", libc::IPPROTO_RAW as i64),
    ]);
    let sockopt = mkconst(&[
        ("SOL_SOCKET", libc::SOL_SOCKET as i64),
        ("SOL_TCP", libc::SOL_TCP as i64),
        ("SO_REUSEADDR", libc::SO_REUSEADDR as i64),
        ("SO_REUSEPORT", libc::SO_REUSEPORT as i64),
        ("SO_KEEPALIVE", libc::SO_KEEPALIVE as i64),
        ("SO_BROADCAST", libc::SO_BROADCAST as i64),
        ("SO_LINGER", libc::SO_LINGER as i64),
        ("SO_RCVBUF", libc::SO_RCVBUF as i64),
        ("SO_SNDBUF", libc::SO_SNDBUF as i64),
        ("SO_RCVTIMEO", libc::SO_RCVTIMEO as i64),
        ("SO_SNDTIMEO", libc::SO_SNDTIMEO as i64),
        ("SO_OOBINLINE", libc::SO_OOBINLINE as i64),
        ("TCP_NODELAY", libc::TCP_NODELAY as i64),
        ("TCP_KEEPIDLE", libc::TCP_KEEPIDLE as i64),
        ("TCP_KEEPINTVL", libc::TCP_KEEPINTVL as i64),
    ]);
    let msgs = mkconst(&[
        ("MSG_PEEK", libc::MSG_PEEK as i64),
        ("MSG_DONTWAIT", libc::MSG_DONTWAIT as i64),
        ("MSG_WAITALL", libc::MSG_WAITALL as i64),
        ("MSG_NOSIGNAL", libc::MSG_NOSIGNAL as i64),
        ("MSG_OOB", libc::MSG_OOB as i64),
        ("MSG_TRUNC", libc::MSG_TRUNC as i64),
        ("MSG_DONTROUTE", libc::MSG_DONTROUTE as i64),
    ]);
    let shuts = mkconst(&[
        ("SHUT_RD", libc::SHUT_RD as i64),
        ("SHUT_WR", libc::SHUT_WR as i64),
        ("SHUT_RDWR", libc::SHUT_RDWR as i64),
    ]);

    fns.extend(prot);
    fns.extend(map);
    fns.extend(oflags);
    fns.extend(seek);
    fns.extend(acc);
    fns.extend(wait);
    fns.extend(pollc);
    fns.extend(clocks);
    fns.extend(rl);
    fns.extend(fcntlc);
    fns.extend(sigs);
    fns.extend(conv);
    fns.extend(af);
    fns.extend(socktype);
    fns.extend(ipproto);
    fns.extend(sockopt);
    fns.extend(msgs);
    fns.extend(shuts);

    vm.vars.insert(
        "sys".into(),
        Value::Dict(Arc::new(indexmap::IndexMap::from_iter(fns))),
    );
}