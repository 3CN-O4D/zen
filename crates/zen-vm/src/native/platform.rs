//! Platform abstraction layer for sys module.
//!
//! Provides cross-platform implementations of system calls using Rust's
//! standard library where possible, falling back to raw libc syscalls on Linux/macOS.
//! Windows uses windows-sys crate for native APIs.
//! Platform is resolved at compile time via `#[cfg]` - no runtime overhead.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_long, c_void};
use std::io::{Read, Write, Seek, SeekFrom};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
#[cfg(target_os = "windows")]
use std::os::windows::io::{AsRawHandle, FromRawHandle, IntoRawHandle};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use libc;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::*;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::IO::*;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::*;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::SystemServices::*;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::*;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::ProcessStatus::*;

use crate::errors::{VmError, VmResult};
use crate::value::Value;

// ── Windows constants (subset of libc equivalents) ────────────────────────

#[cfg(target_os = "windows")]
mod windows_consts {
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::System::IO::*;
    use windows_sys::Win32::Storage::FileSystem::*;
    
    // File access flags (O_* equivalents)
    pub const O_RDONLY: i32 = GENERIC_READ as i32;
    pub const O_WRONLY: i32 = GENERIC_WRITE as i32;
    pub const O_RDWR: i32 = (GENERIC_READ | GENERIC_WRITE) as i32;
    pub const O_CREAT: i32 = 0x00000100; // FILE_CREATE
    pub const O_TRUNC: i32 = 0x00000200; // FILE_TRUNCATE_EXISTING
    pub const O_APPEND: i32 = 0x00000400; // FILE_APPEND_DATA
    
    // File share modes
    pub const FILE_SHARE_READ: u32 = 0x00000001;
    pub const FILE_SHARE_WRITE: u32 = 0x00000002;
    pub const FILE_SHARE_DELETE: u32 = 0x00000004;
    
    // Creation dispositions
    pub const OPEN_EXISTING: u32 = 3;
    pub const CREATE_NEW: u32 = 1;
    pub const CREATE_ALWAYS: u32 = 2;
    pub const OPEN_ALWAYS: u32 = 4;
    pub const TRUNCATE_EXISTING: u32 = 5;
    
    // File attributes
    pub const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
    pub const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x08000000;
    pub const FILE_FLAG_RANDOM_ACCESS: u32 = 0x10000000;
    
    // Seek methods
    pub const FILE_BEGIN: u32 = 0;
    pub const FILE_CURRENT: u32 = 1;
    pub const FILE_END: u32 = 2;
    
    // Process
    pub const PROCESS_ALL_ACCESS: u32 = 0x001F0FFF;
    
    // Wait
    pub const INFINITE: u32 = 0xFFFFFFFF;
    pub const WAIT_OBJECT_0: u32 = 0;
    pub const WAIT_TIMEOUT: u32 = 258;
    
    // Signals (limited on Windows)
    pub const SIGTERM: i32 = 15;
    pub const SIGKILL: i32 = 9;
    pub const SIGINT: i32 = 2;
}

// Re-export for platform-specific use
#[cfg(target_os = "windows")]
use windows_consts::*;

// ── File operations ────────────────────────────────────────────────────────

/// Open a file and return a file descriptor/handle
pub fn open(path: &str, flags: i32, mode: u32) -> VmResult<i32> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;
        
        let mut opts = OpenOptions::new();
        if flags & libc::O_RDONLY as i32 != 0 { opts.read(true); }
        if flags & libc::O_WRONLY as i32 != 0 { opts.write(true); }
        if flags & libc::O_RDWR as i32 != 0 { opts.read(true).write(true); }
        if flags & libc::O_CREAT as i32 != 0 { opts.create(true); }
        if flags & libc::O_TRUNC as i32 != 0 { opts.truncate(true); }
        if flags & libc::O_APPEND as i32 != 0 { opts.append(true); }
        opts.mode(mode);
        
        let file = opts.open(path).map_err(|e| VmError::NativeError(format!("open: {e}")))?;
        Ok(file.into_raw_fd())
    }
    #[cfg(target_os = "windows")]
    {
        use std::fs::OpenOptions;
        use windows_sys::Win32::Foundation::*;
        use windows_sys::Win32::Storage::FileSystem::*;
        
        let mut opts = OpenOptions::new();
        let mut access = 0u32;
        let mut share = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
        let mut creation = OPEN_EXISTING;
        
        if flags & O_RDWR as i32 != 0 {
            access = GENERIC_READ | GENERIC_WRITE;
        } else if flags & O_WRONLY as i32 != 0 {
            access = GENERIC_WRITE;
        } else {
            access = GENERIC_READ;
        }
        
        if flags & O_CREAT as i32 != 0 {
            if flags & O_TRUNC as i32 != 0 {
                creation = CREATE_ALWAYS;
            } else {
                creation = OPEN_ALWAYS;
            }
        }
        if flags & O_TRUNC as i32 != 0 {
            creation = TRUNCATE_EXISTING;
        }
        
        let c_path: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        
        let handle = unsafe {
            CreateFileW(
                c_path.as_ptr(),
                access,
                share,
                std::ptr::null_mut(),
                creation,
                FILE_ATTRIBUTE_NORMAL,
                HANDLE(0),
            )
        };
        
        if handle == INVALID_HANDLE_VALUE {
            return Err(syserr("open"));
        }
        Ok(handle as i32)
    }
}

/// Close a file descriptor/handle
pub fn close(fd: i32) -> VmResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let result = unsafe { libc::close(fd) };
        if result == -1 {
            return Err(syserr("close"));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::*;
        let result = unsafe { CloseHandle(HANDLE(fd as *mut _)) };
        if result == 0 {
            return Err(syserr("close"));
        }
        Ok(())
    }
}

/// Read from a file descriptor/handle
pub fn read(fd: i32, buf: &mut [u8]) -> VmResult<usize> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let result = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if result == -1 {
            return Err(syserr("read"));
        }
        Ok(result as usize)
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::*;
        use windows_sys::Win32::Storage::FileSystem::*;
        
        let mut bytes_read: u32 = 0;
        let result = unsafe {
            ReadFile(
                HANDLE(fd as *mut _),
                buf.as_mut_ptr() as *mut _,
                buf.len() as u32,
                &mut bytes_read,
                std::ptr::null_mut(),
            )
        };
        if result == 0 {
            return Err(syserr("read"));
        }
        Ok(bytes_read as usize)
    }
}

/// Write to a file descriptor/handle
pub fn write(fd: i32, buf: &[u8]) -> VmResult<usize> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let result = unsafe { libc::write(fd, buf.as_ptr() as *const libc::c_void, buf.len()) };
        if result == -1 {
            return Err(syserr("write"));
        }
        Ok(result as usize)
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::*;
        use windows_sys::Win32::Storage::FileSystem::*;
        
        let mut bytes_written: u32 = 0;
        let result = unsafe {
            WriteFile(
                HANDLE(fd as *mut _),
                buf.as_ptr() as *const _,
                buf.len() as u32,
                &mut bytes_written,
                std::ptr::null_mut(),
            )
        };
        if result == 0 {
            return Err(syserr("write"));
        }
        Ok(bytes_written as usize)
    }
}

/// Seek on a file descriptor/handle
pub fn lseek(fd: i32, offset: i64, whence: i32) -> VmResult<i64> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let result = unsafe { libc::lseek(fd, offset, whence) };
        if result == -1 {
            return Err(syserr("lseek"));
        }
        Ok(result)
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::*;
        use windows_sys::Win32::Storage::FileSystem::*;
        
        let mut distance_low = offset as u32;
        let mut distance_high = (offset >> 32) as i32;
        
        let move_method = match whence {
            0 => FILE_BEGIN,
            1 => FILE_CURRENT,
            2 => FILE_END,
            _ => return Err(VmError::ValueError("invalid whence".into())),
        };
        
        let new_pos = unsafe {
            SetFilePointer(
                HANDLE(fd as *mut _),
                distance_low,
                &mut distance_high,
                move_method,
            )
        };
        
        if new_pos == INVALID_SET_FILE_POINTER {
            let err = unsafe { GetLastError() };
            if err != 0 {
                return Err(syserr("lseek"));
            }
        }
        Ok(((distance_high as i64) << 32) | (new_pos as i64))
    }
}

/// Dup a file descriptor/handle
pub fn dup(fd: i32) -> VmResult<i32> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let result = unsafe { libc::dup(fd) };
        if result == -1 {
            return Err(syserr("dup"));
        }
        Ok(result)
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::*;
        use windows_sys::Win32::System::Threading::*;
        
        let mut new_handle = HANDLE(0);
        let result = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                HANDLE(fd as *mut _),
                GetCurrentProcess(),
                &mut new_handle,
                0,
                FALSE,
                DUPLICATE_SAME_ACCESS,
            )
        };
        if result == 0 {
            return Err(syserr("dup"));
        }
        Ok(new_handle.0 as i32)
    }
}

/// Dup2 a file descriptor/handle
pub fn dup2(oldfd: i32, newfd: i32) -> VmResult<i32> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let result = unsafe { libc::dup2(oldfd, newfd) };
        if result == -1 {
            return Err(syserr("dup2"));
        }
        Ok(result)
    }
    #[cfg(target_os = "windows")]
    {
        // On Windows, close newfd first if valid, then dup oldfd to newfd
        let _ = close(newfd);
        let mut new_handle = HANDLE(0);
        let result = unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                HANDLE(oldfd as *mut _),
                GetCurrentProcess(),
                &mut new_handle,
                0,
                FALSE,
                DUPLICATE_SAME_ACCESS,
            )
        };
        if result == 0 {
            return Err(syserr("dup2"));
        }
        // Note: We can't easily force the handle to a specific value on Windows
        Ok(new_handle.0 as i32)
    }
}

/// Pipe
pub fn pipe() -> VmResult<(i32, i32)> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let mut fds = [0 as c_int; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } == -1 {
            return Err(syserr("pipe"));
        }
        Ok((fds[0], fds[1]))
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::*;
        use windows_sys::Win32::Storage::FileSystem::*;
        
        let mut read_handle = HANDLE(0);
        let mut write_handle = HANDLE(0);
        let sa = std::ptr::null_mut();
        let result = unsafe {
            CreatePipe(
                &mut read_handle,
                &mut write_handle,
                sa,
                0,
            )
        };
        if result == 0 {
            return Err(syserr("pipe"));
        }
        Ok((read_handle.0 as i32, write_handle.0 as i32))
    }
}

/// Fork
pub fn fork() -> VmResult<i32> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return Err(syserr("fork"));
        }
        Ok(pid)
    }
    #[cfg(target_os = "windows")]
    {
        Err(VmError::NativeError("fork not available on Windows".into()))
    }
}

/// Get PID
pub fn getpid() -> i32 {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    { unsafe { libc::getpid() } }
    #[cfg(target_os = "windows")]
    { (unsafe { GetCurrentProcessId() }) as i32 }
}

/// Get PPID
pub fn getppid() -> i32 {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    { unsafe { libc::getppid() } }
    #[cfg(target_os = "windows")]
    { 0 }
}

/// Get UID
pub fn getuid() -> i32 {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    { unsafe { libc::getuid() as i32 } }
    #[cfg(target_os = "windows")]
    { 1000 }
}

/// Get GID
pub fn getgid() -> i32 {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    { unsafe { libc::getgid() as i32 } }
    #[cfg(target_os = "windows")]
    { 1000 }
}

/// Get EUID
pub fn geteuid() -> i32 {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    { unsafe { libc::geteuid() as i32 } }
    #[cfg(target_os = "windows")]
    { 1000 }
}

/// Get EGID
pub fn getegid() -> i32 {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    { unsafe { libc::getegid() as i32 } }
    #[cfg(target_os = "windows")]
    { 1000 }
}

/// Umask
pub fn umask(mask: u32) -> u32 {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    { unsafe { libc::umask(mask as libc::mode_t) as u32 } }
    #[cfg(target_os = "windows")]
    { 0o022 }
}

/// Kill
pub fn kill(pid: i32, sig: i32) -> VmResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if unsafe { libc::kill(pid, sig) } == -1 {
            return Err(syserr("kill"));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::*;
        use windows_sys::Win32::System::Threading::*;
        
        let handle = unsafe { OpenProcess(PROCESS_ALL_ACCESS, FALSE, pid as u32) };
        if handle == HANDLE(0) {
            return Err(syserr("kill"));
        }
        let result = unsafe { TerminateProcess(handle, 1) };
        unsafe { CloseHandle(handle) };
        if result == 0 {
            return Err(syserr("kill"));
        }
        Ok(())
    }
}

// ── Stat operations ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct FileStat {
    pub st_mode: u32,
    pub st_size: i64,
    pub st_mtime: i64,
    pub st_atime: i64,
    pub st_ctime: i64,
}

pub fn stat(path: &str) -> VmResult<FileStat> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let c_path = CString::new(path).map_err(|_| VmError::ValueError("invalid path".into()))?;
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        let result = unsafe { libc::stat(c_path.as_ptr(), &mut stat) };
        if result == -1 {
            return Err(syserr("stat"));
        }
        Ok(FileStat {
            st_mode: stat.st_mode as u32,
            st_size: stat.st_size,
            st_mtime: stat.st_mtime,
            st_atime: stat.st_atime,
            st_ctime: stat.st_ctime,
        })
    }
    #[cfg(target_os = "windows")]
    {
        use std::fs::metadata;
        let meta = metadata(path).map_err(|e| VmError::NativeError(format!("stat: {e}")))?;
        Ok(FileStat {
            st_mode: if meta.is_dir() { 0o040000 } else { 0o100000 },
            st_size: meta.len() as i64,
            st_mtime: 0,
            st_atime: 0,
            st_ctime: 0,
        })
    }
}

pub fn lstat(path: &str) -> VmResult<FileStat> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let c_path = CString::new(path).map_err(|_| VmError::ValueError("invalid path".into()))?;
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        let result = unsafe { libc::lstat(c_path.as_ptr(), &mut stat) };
        if result == -1 {
            return Err(syserr("lstat"));
        }
        Ok(FileStat {
            st_mode: stat.st_mode as u32,
            st_size: stat.st_size,
            st_mtime: stat.st_mtime,
            st_atime: stat.st_atime,
            st_ctime: stat.st_ctime,
        })
    }
    #[cfg(target_os = "windows")]
    {
        stat(path)
    }
}

pub fn fstat(fd: i32) -> VmResult<FileStat> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let mut stat: libc::stat = unsafe { std::mem::zeroed() };
        let result = unsafe { libc::fstat(fd, &mut stat) };
        if result == -1 {
            return Err(syserr("fstat"));
        }
        Ok(FileStat {
            st_mode: stat.st_mode as u32,
            st_size: stat.st_size,
            st_mtime: stat.st_mtime,
            st_atime: stat.st_atime,
            st_ctime: stat.st_ctime,
        })
    }
    #[cfg(target_os = "windows")]
    {
        Err(VmError::NativeError("fstat not implemented on Windows".into()))
    }
}

pub fn access(path: &str, mode: i32) -> VmResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let c_path = CString::new(path).map_err(|_| VmError::ValueError("invalid path".into()))?;
        if unsafe { libc::access(c_path.as_ptr(), mode) } == -1 {
            return Err(syserr("access"));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        use std::fs::metadata;
        metadata(path).map_err(|e| VmError::NativeError(format!("access: {e}")))?;
        Ok(())
    }
}

pub fn chmod(path: &str, mode: u32) -> VmResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let c_path = CString::new(path).map_err(|_| VmError::ValueError("invalid path".into()))?;
        if unsafe { libc::chmod(c_path.as_ptr(), mode as libc::mode_t) } == -1 {
            return Err(syserr("chmod"));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        Err(VmError::NativeError("chmod not supported on Windows".into()))
    }
}

pub fn unlink(path: &str) -> VmResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let c_path = CString::new(path).map_err(|_| VmError::ValueError("invalid path".into()))?;
        if unsafe { libc::unlink(c_path.as_ptr()) } == -1 {
            return Err(syserr("unlink"));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        std::fs::remove_file(path).map_err(|e| VmError::NativeError(format!("unlink: {e}")))?;
        Ok(())
    }
}

pub fn mkdir(path: &str, mode: u32) -> VmResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let c_path = CString::new(path).map_err(|_| VmError::ValueError("invalid path".into()))?;
        if unsafe { libc::mkdir(c_path.as_ptr(), mode as libc::mode_t) } == -1 {
            return Err(syserr("mkdir"));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        std::fs::create_dir(path).map_err(|e| VmError::NativeError(format!("mkdir: {e}")))?;
        Ok(())
    }
}

pub fn rmdir(path: &str) -> VmResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let c_path = CString::new(path).map_err(|_| VmError::ValueError("invalid path".into()))?;
        if unsafe { libc::rmdir(c_path.as_ptr()) } == -1 {
            return Err(syserr("rmdir"));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        std::fs::remove_dir(path).map_err(|e| VmError::NativeError(format!("rmdir: {e}")))?;
        Ok(())
    }
}

pub fn readlink(path: &str, buf: &mut [u8]) -> VmResult<usize> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let c_path = CString::new(path).map_err(|_| VmError::ValueError("invalid path".into()))?;
        let result = unsafe { libc::readlink(c_path.as_ptr(), buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
        if result == -1 {
            return Err(syserr("readlink"));
        }
        Ok(result as usize)
    }
    #[cfg(target_os = "windows")]
    {
        Err(VmError::NativeError("readlink not implemented on Windows".into()))
    }
}

pub fn symlink(target: &str, linkpath: &str) -> VmResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let c_target = CString::new(target).map_err(|_| VmError::ValueError("invalid target".into()))?;
        let c_linkpath = CString::new(linkpath).map_err(|_| VmError::ValueError("invalid linkpath".into()))?;
        if unsafe { libc::symlink(c_target.as_ptr(), c_linkpath.as_ptr()) } == -1 {
            return Err(syserr("symlink"));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        Err(VmError::NativeError("symlink not implemented on Windows".into()))
    }
}

pub fn link(oldpath: &str, newpath: &str) -> VmResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let c_old = CString::new(oldpath).map_err(|_| VmError::ValueError("invalid oldpath".into()))?;
        let c_new = CString::new(newpath).map_err(|_| VmError::ValueError("invalid newpath".into()))?;
        if unsafe { libc::link(c_old.as_ptr(), c_new.as_ptr()) } == -1 {
            return Err(syserr("link"));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        std::fs::hard_link(oldpath, newpath).map_err(|e| VmError::NativeError(format!("link: {e}")))?;
        Ok(())
    }
}

pub fn truncate(path: &str, length: i64) -> VmResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let c_path = CString::new(path).map_err(|_| VmError::ValueError("invalid path".into()))?;
        if unsafe { libc::truncate(c_path.as_ptr(), length) } == -1 {
            return Err(syserr("truncate"));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        use std::fs::OpenOptions;
        let mut file = OpenOptions::new().write(true).open(path)
            .map_err(|e| VmError::NativeError(format!("truncate: {e}")))?;
        file.set_len(length as u64).map_err(|e| VmError::NativeError(format!("truncate: {e}")))?;
        Ok(())
    }
}

pub fn ftruncate(fd: i32, length: i64) -> VmResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if unsafe { libc::ftruncate(fd, length) } == -1 {
            return Err(syserr("ftruncate"));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::*;
        use windows_sys::Win32::Storage::FileSystem::*;
        
        let mut distance_low = length as u32;
        let mut distance_high = (length >> 32) as i32;
        
        let old_pos = unsafe { SetFilePointer(HANDLE(fd as *mut _), 0, std::ptr::null_mut(), FILE_CURRENT) };
        if old_pos == INVALID_SET_FILE_POINTER {
            let err = unsafe { GetLastError() };
            if err != 0 {
                return Err(syserr("ftruncate"));
            }
        }
        
        let new_pos = unsafe { SetFilePointer(HANDLE(fd as *mut _), length as u32, &mut ((length >> 32) as i32), FILE_BEGIN) };
        if new_pos == INVALID_SET_FILE_POINTER {
            let err = unsafe { GetLastError() };
            if err != 0 {
                return Err(syserr("ftruncate"));
            }
        }
        
        let result = unsafe { SetEndOfFile(HANDLE(fd as *mut _)) };
        if result == 0 {
            return Err(syserr("ftruncate"));
        }
        
        unsafe { SetFilePointer(HANDLE(fd as *mut _), old_pos, std::ptr::null_mut(), FILE_BEGIN) };
        Ok(())
    }
}

pub fn fsync(fd: i32) -> VmResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if unsafe { libc::fsync(fd) } == -1 {
            return Err(syserr("fsync"));
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::*;
        use windows_sys::Win32::Storage::FileSystem::*;
        
        let result = unsafe { FlushFileBuffers(HANDLE(fd as *mut _)) };
        if result == 0 {
            return Err(syserr("fsync"));
        }
        Ok(())
    }
}

pub fn fdatasync(fd: i32) -> VmResult<()> {
    #[cfg(target_os = "linux")]
    {
        if unsafe { libc::fdatasync(fd) } == -1 {
            return Err(syserr("fdatasync"));
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        fsync(fd) // macOS doesn't have fdatasync
    }
    #[cfg(target_os = "windows")]
    {
        fsync(fd)
    }
}

// ── Process operations ────────────────────────────────────────────────────

pub fn exec(path: &str, args: &[String]) -> VmResult<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let c_path = CString::new(path).map_err(|_| VmError::ValueError("invalid path".into()))?;
        let c_args: Vec<CString> = args.iter()
            .map(|a| CString::new(a.as_str()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| VmError::ValueError("invalid argument".into()))?;
        let c_argv: Vec<*const libc::c_char> = c_args.iter().map(|c| c.as_ptr()).collect();
        let mut c_argv = c_argv;
        c_argv.push(std::ptr::null());
        
        unsafe { libc::execvp(c_path.as_ptr(), c_argv.as_ptr()) };
        Err(syserr("exec"))
    }
    #[cfg(target_os = "windows")]
    {
        Err(VmError::NativeError("exec not implemented on Windows".into()))
    }
}

pub fn waitpid(pid: i32, options: i32) -> VmResult<(i32, i32)> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let mut status: libc::c_int = 0;
        let result = unsafe { libc::waitpid(pid, &mut status, options) };
        if result == -1 {
            return Err(syserr("waitpid"));
        }
        Ok((result, status))
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::*;
        use windows_sys::Win32::System::Threading::*;
        
        let handle = unsafe { OpenProcess(PROCESS_ALL_ACCESS, FALSE, pid as u32) };
        if handle == HANDLE(0) {
            return Err(syserr("waitpid"));
        }
        
        let mut exit_code: u32 = 0;
        let result = unsafe { GetExitCodeProcess(handle, &mut exit_code) };
        if result == 0 {
            unsafe { CloseHandle(handle) };
            return Err(syserr("waitpid"));
        }
        
        if exit_code == STILL_ACTIVE {
            // Process still running, wait for it
            let wait_result = unsafe { WaitForSingleObject(HANDLE(handle), INFINITE) };
            if wait_result == WAIT_OBJECT_0 {
                let mut exit_code: u32 = 0;
                unsafe { GetExitCodeProcess(handle, &mut exit_code) };
            }
        }
        
        unsafe { CloseHandle(handle) };
        Ok((pid, exit_code as i32))
    }
}

pub fn wexitstatus(status: i32) -> i32 {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if unsafe { libc::WIFEXITED(status) } {
            unsafe { libc::WEXITSTATUS(status) }
        } else {
            -1
        }
    }
    #[cfg(target_os = "windows")]
    {
        status
    }
}

// ── Helper ────────────────────────────────────────────────────────────────

fn syserr(op: &str) -> VmError {
    VmError::NativeError(format!("{op}: {}", std::io::Error::last_os_error()))
}
