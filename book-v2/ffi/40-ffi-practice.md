# Chapter 40: FFI in Practice

This chapter covers practical FFI patterns: capability gates, calling real libraries, error handling, and safe integration.

## 40.1 FFI capability gates

⚠️ FFI can load arbitrary shared libraries and call arbitrary functions. Use `ffi.allow` and `ffi.deny` to restrict which libraries can be loaded.

```zen
import ffi

// Deny loading from dangerous locations
ffi.deny(["/tmp/", "/var/tmp/"])

// Only allow loading from trusted directories
ffi.allow(["/usr/lib/", "/lib/"])

// Now ffi.load("/tmp/evil.so") raises an error
// ffi.load("/usr/lib/libc.so.6") works
```

### Behavior

| Allow list | Deny list | Result |
|------------|-----------|--------|
| empty | empty | All paths allowed |
| non-empty | empty | Only paths matching an allow prefix |
| empty | non-empty | All except deny prefixes |
| non-empty | non-empty | Must match allow AND not match deny |

## 40.2 Calling libc functions

```zen
import ffi

ffi.allow(["/usr/lib/", "/lib/"])
let libc = ffi.load("libc.so.6")

// getpid() → int
let getpid = ffi.symbol(libc, "getpid")
let pid = ffi.call(getpid, [])
print("PID: " + str(pid))

// sleep(seconds) → int (seconds remaining)
let sleep = ffi.symbol(libc, "sleep")
let remaining = ffi.call(sleep, [1])
print("slept, remaining: " + str(remaining))
```

## 40.3 Working with strings

C strings are null-terminated. `ffi.str(ptr)` reads until the first NUL:

```zen
import ffi

let libc = ffi.load("libc.so.6")

// gethostname(buf, len) → int
let gethostname = ffi.symbol(libc, "gethostname")
let buf = ffi.malloc(256)
ffi.call(gethostname, [buf, 256])
let hostname = ffi.str(buf)
print("Hostname: " + hostname)
ffi.free(buf)
```

## 40.4 Working with buffers

```zen
import ffi

let libc = ffi.load("libc.so.6")
let memcpy = ffi.symbol(libc, "memcpy")

let src = ffi.malloc(16)
ffi.write(src, "HelloWorld")

let dst = ffi.malloc(16)
ffi.call(memcpy, [dst, src, 11])
let copied = ffi.read(dst, 11)
print(copied)  // HelloWorld

ffi.free(src)
ffi.free(dst)
```

## 40.5 Error handling

```zen
import ffi

// ffi.load raises on failure
var lib = nil
try {
    lib = ffi.load("/nonexistent/lib.so")
} catch as e {
    print("load failed: " + e.msg)
    lib = nil
}

// ffi.symbol raises if symbol not found
if lib != nil {
    try {
        let addr = ffi.symbol(lib, "nonexistent_func")
    } catch as e {
        print("symbol not found: " + e.msg)
    }
}
```

## 40.6 Calling conventions and types

Zen's FFI uses the platform's default C calling convention (cdecl on x86, System V AMD64 ABI on x86_64). Supported argument types:

- Zen `int` → C `int`/`long`
- Zen `float` → C `double`
- Zen `string` → C `char*` (null-terminated)
- Typed scalars: `{"type": "double", "value": x}`
- Raw buffers: `{"buf": "hex..."}`

Return types: `int` (default), `double`, `void`, `str`, bytes.

> ⚠️ Zen FFI does **not** support Zen closures as C callbacks. If a C function needs a callback, you must pass a native function pointer obtained from `ffi.symbol`.

## 40.7 Complete example: libc time

```zen
import ffi

ffi.allow(["/usr/lib/", "/lib/"])
let libc = ffi.load("libc.so.6")

// time(NULL) → time_t (int)
let time_fn = ffi.symbol(libc, "time")
let now = ffi.call(time_fn, [nil])
print("Unix time: " + str(now))

// getpid() → int
let getpid = ffi.symbol(libc, "getpid")
let pid = ffi.call(getpid, [])
print("Process ID: " + str(pid))
```

## 40.8 Quick reference

```zen
import ffi

// Security
ffi.allow(["/usr/lib/"])
ffi.deny(["/tmp/"])

// Load + symbol
let lib = ffi.load("libc.so.6")
let addr = ffi.symbol(lib, "function_name")

// Call
let r = ffi.call(addr, [arg1, arg2], "int")
let r = ffi.call(addr, [4.5], "double")

// Memory
let ptr = ffi.malloc(1024)
ffi.write(ptr, "data")
let s = ffi.read(ptr, 10)
let str = ffi.str(ptr)
ffi.free(ptr)

// Types
ffi.sizeof("int")
ffi.alignof("double")
ffi.errno()
ffi.strerror(n)
```

## 40.9 Exercises

1. Load `libc`, call `getpid`, and print the current process ID.
2. Call `time(NULL)` and print the Unix timestamp.
3. Use `ffi.malloc`, `ffi.write`, `ffi.read`, `ffi.free` to copy a string.
4. Call `gethostname` and print the hostname.
5. Set `ffi.deny(["/tmp/"])` and confirm loading from `/tmp` fails.
