# Chapter 41: sys Module

The `sys` module provides low-level system operations: file descriptors, process control, signals, and direct syscalls. It's a thin wrapper over POSIX system calls.

> ⚠️ `sys` is Linux/macOS only. On Windows, `sys_stub` provides limited functionality. Use `fs`, `os`, and `process` for portable operations.

## 41.1 File descriptor operations

```zen
import sys

// Open a file
let fd = sys.open("data.txt", sys.O_RDONLY)
print(fd)  // file descriptor integer

// Read
let data = sys.read(fd, 1024)
print(data)

// Write
sys.write(fd, "hello")

// Seek
sys.lseek(fd, 0, sys.SEEK_SET)

// Close
sys.close(fd)
```

### Open flags

| Flag | Meaning |
|------|---------|
| `sys.O_RDONLY` | Read only |
| `sys.O_WRONLY` | Write only |
| `sys.O_RDWR` | Read/write |
| `sys.O_CREAT` | Create if missing |
| `sys.O_APPEND` | Append |
| `sys.O_TRUNC` | Truncate |

### Seek whence

| Value | Meaning |
|-------|---------|
| `sys.SEEK_SET` | From start |
| `sys.SEEK_CUR` | From current position |
| `sys.SEEK_END` | From end |

## 41.2 Process control

```zen
import sys

// Get process IDs
print(sys.getpid())     // process ID
print(sys.getppid())    // parent process ID
print(sys.getuid())     // user ID
print(sys.geteuid())    // effective user ID
print(sys.getgid())     // group ID
print(sys.getegid())    // effective group ID

// Fork (Unix only)
let pid = sys.fork()
if pid == 0 {
    print("in child")
} else {
    print("in parent, child pid: " + str(pid))
    sys.wait(pid)
}

// Kill a process
sys.kill(pid, 15)  // SIGTERM
```

## 41.3 Pipes

```zen
import sys

let [read_fd, write_fd] = sys.pipe()

sys.write(write_fd, "hello through pipe")
let msg = sys.read(read_fd, 1024)
print(msg)  // hello through pipe

sys.close(read_fd)
sys.close(write_fd)
```

## 41.4 Direct syscalls

```zen
import sys

// Make a raw syscall by number
let result = sys.syscall(sys.SYS_write, 1, "hello\n", 6)
print(result)
```

Syscall numbers are platform-specific (see `/usr/include/asm/unistd.h` on Linux).

## 41.5 Signal handling (limited)

Zen's signal support is limited. Use `sys.kill(pid, signal)` to send signals:

| Signal | Number | Meaning |
|--------|--------|---------|
| `sys.SIGTERM` | 15 | Terminate gracefully |
| `sys.SIGKILL` | 9 | Kill immediately |
| `sys.SIGINT` | 2 | Interrupt (Ctrl+C) |
| `sys.SIGHUP` | 1 | Hangup |

## 41.6 Common patterns

### Read a file byte by byte

```zen
import sys

let fd = sys.open("file.txt", sys.O_RDONLY)
let chunks = []
while true {
    let chunk = sys.read(fd, 4096)
    if len(chunk) == 0 {
        break
    }
    chunks.push(chunk)
}
sys.close(fd)
let content = chunks.join("")
print(content)
```

### Duplicate a file descriptor

```zen
import sys

let fd = sys.open("file.txt", sys.O_RDONLY)
let fd2 = sys.dup(fd)
// fd2 refers to the same file
sys.close(fd)
sys.close(fd2)
```

### Create a pipe between processes

```zen
import sys
import process

let [rfd, wfd] = sys.pipe()

// Fork and have child write, parent read
let pid = sys.fork()
if pid == 0 {
    sys.close(rfd)
    sys.write(wfd, "from child")
    sys.close(wfd)
} else {
    sys.close(wfd)
    let msg = sys.read(rfd, 1024)
    print("parent got: " + msg)
    sys.close(rfd)
    sys.wait(pid)
}
```

## 41.7 Quick reference

```zen
import sys

sys.open(path, flags)          // file descriptor
sys.read(fd, n)                // read bytes
sys.write(fd, data)            // write bytes
sys.close(fd)                  // close
sys.lseek(fd, offset, whence)  // seek
sys.dup(fd)                    // duplicate fd
sys.pipe()                     // [read_fd, write_fd]
sys.getpid()                   // process ID
sys.getppid()                  // parent PID
sys.fork()                     // fork (Unix)
sys.kill(pid, sig)             // send signal
sys.wait(pid)                  // wait for child
sys.syscall(num, args...)      // raw syscall
```

## 41.8 Common pitfalls

- **`sys` is Unix-only.** On Windows, many functions are stubs or raise errors.
- **File descriptors are integers.** Track them yourself; Zen won't close them automatically.
- **`sys.read` may return fewer bytes** than requested. Loop to read all.
- **Forked processes share file descriptors.** Close what you don't need in each process.
- **Raw syscalls are dangerous.** Wrong arguments can crash the process.

## 41.9 Exercises

1. Open a file, read its contents, and print them using `sys`.
2. Create a pipe, write a message in one "thread", read it in another.
3. Use `sys.fork` to create a child process that prints its PID.
4. Use `sys.kill` to send SIGTERM to a child process and `sys.wait` for it.
5. Use `sys.lseek` to read the last 100 bytes of a file.


## Advanced Topics and Exercises

### Safety considerations
- Memory management across FFI boundary
- Type mismatches
- Calling conventions
- Null pointers
- Resource leaks

### Debugging FFI
- Check symbol names (nm, objdump)
- Use cdecl/stdcall correctly
- Test with simple functions first

### Exercises
1. Call simple C function from shared library
2. Pass strings and arrays
3. Handle pointers/structs
4. Manage memory allocation
5. Error handling across boundary
6. Write wrapper module


## 41.12 Worked examples — sys module

Typical `sys` members (exact set depends on build): `argv`, `platform`,
`version`, `exit`, `environ`, `path`, `modules`.

### Example 1: Read command-line arguments

```zen
import sys
print(sys.argv)       # ["app.z", "arg1", "arg2"]
print(len(sys.argv))  # argument count
```

### Example 2: Platform detection

```zen
if sys.platform == "linux" {
    print("running on Linux")
} elif sys.platform == "darwin" {
    print("running on macOS")
} elif sys.platform == "windows" {
    print("running on Windows")
} else {
    print("unknown platform: " + sys.platform)
}
```

### Example 3: Environment variables

```zen
print(sys.environ["PATH"])
let port = sys.environ["PORT"] ?? "8080"
print("port=" + port)
```

### Example 4: Graceful exit

```zen
func main() {
    # ... work ...
    sys.exit(0)
}
```

### Example 5: Inspect loaded modules

```zen
print(sys.modules)   # list/dict of loaded modules
```

## 41.13 Platform differences

| Area | Linux | macOS | Windows |
|------|-------|-------|---------|
| Path separator | `/` | `/` | `\` |
| Line ending | `\n` | `\n` | `\r\n` |
| Shared lib ext | `.so` | `.dylib` | `.dll` |
| Executable ext | (none) | (none) | `.exe` |
| Case sensitivity | sensitive | insensitive | insensitive |
| Signals | SIGINT/SIGTERM | similar | Ctrl-C/console handler |

Write portable code: use `pathlib.Path` (joins correctly), avoid hardcoding
`/`, test on each target.

## 41.14 Exercises

1. Print `sys.argv`, `sys.platform`, `sys.version`.
2. Write a script that behaves differently on Linux vs Windows.
3. Read `PORT` env var with a default of 8080.
4. Implement `exit_with_code(code)` calling `sys.exit`.
5. List all loaded modules at startup.
6. Write `get_env(name, default)` using `sys.environ`.
7. Detect if running under a debugger/CI via env vars.
8. Build a small CLI that prints platform info.
9. Compare `sys.path` entries before/after an import.
10. Write a portable temp-file path using `pathlib`.



## 41.12 Worked example — platform-aware script

```zen
import sys
import pathlib
import os

print("argv: " + str(sys.argv))
print("platform: " + sys.platform)
print("version: " + sys.version)
print("cwd: " + os.getcwd())
print("HOME: " + str(sys.environ["HOME"] ?? ""))

let p = pathlib.Path("data") / "config.txt"
print("path: " + str(p))
print("exists: " + str(p.exists()))

if sys.platform == "windows" {
    print("running on Windows")
}
```

## 41.13 Troubleshooting sys

| Symptom | Cause | Fix |
|---------|-------|-----|
| argv empty | no args passed | pass args after script |
| wrong platform | running on unexpected OS | check sys.platform |
| env var missing | not set | use `??` default |

## 41.14 Exercises

1. Print argv, platform, version.
2. Read `PORT` env var with default 8080.
3. List loaded modules at startup.
4. Build a portable temp path with pathlib.
5. Detect Windows vs Linux behavior.
