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


## 40.12 Worked examples — FFI in practice

### Example: Wrapping a small C library

C source (`cryptokey.c`):
```c
#include <stdint.h>
uint32_t hash32(const char *s) {
    uint32_t h = 2166136261u;
    while (*s) { h ^= (unsigned char)*s++; h *= 16777619u; }
    return h;
}
```

Zen wrapper (`crypto_key.z`):
```zen
import ffi

let lib = ffi.load_library("cryptokey.so")
let hash32_fn = ffi.get_symbol(lib, "hash32")

pub func hash32(s) {
    return ffi.call(hash32_fn, [s])
}

ffi.close(lib)
```

Usage:
```zen
import crypto_key
print(crypto_key.hash32("hello"))   # deterministic uint32
```

### Example: Error codes from C

C functions often return -1 or set `errno`. Wrap them:

```zen
pub func safe_call(fn, args) {
    let r = ffi.call(fn, args)
    if r == -1 {
        let errno = ffi.call(get_errno_fn, [])
        throw "C error: errno=" + str(errno)
    }
    return r
}
```

### Example: Struct layout (if supported)

If Zen FFI supports structs, you declare field offsets/sizes and use
`ffi.struct_read`/`ffi.struct_write`. Otherwise, pass raw bytes and interpret
with `struct.calcsize` / manual unpacking.

```zen
import struct
let size = struct.calcsize("<ii")   # two little-endian ints = 8 bytes
```

### Example: Resource lifecycle with try/finally

```zen
pub func with_resource(acquire_fn, release_fn, use_fn) {
    let h = ffi.call(acquire_fn, [])
    try {
        return use_fn(h)
    } finally {
        ffi.call(release_fn, [h])
    }
}
```

## 40.13 FFI best practices

1. Wrap raw FFI calls in idiomatic Zen functions; don't expose `ffi.*`
   directly to users.
2. Validate all inputs before crossing the boundary.
3. Use `try/finally` for every acquired resource.
4. Document which Zen types map to which C types.
5. Keep FFI boundary small; do heavy logic in Zen, not C.
6. Test with valgrind/ASan on the C side for memory errors.

## 40.14 Exercises

1. Write a Zen module wrapping `libc`'s `puts` to print a string via C.
2. Wrap a C function computing CRC32; compare with a pure-Zen CRC32.
3. Build `with_resource` helper and use it for a DB handle.
4. Implement `errno` checking wrapper for C calls returning -1.
5. Call a C function returning a struct via raw bytes + `struct` module.
6. Create a FFI wrapper for a small compression library (zlib conceptually).
7. Write a FFI stress test: call a C function 100000 times.
8. Demonstrate catching a C error and rethrowing as a Zen error.
9. Document the C type mapping for your wrapper module.
10. Build a tiny "native addon": C + Zen working together.



## 40.12 Worked example — wrap a C hash function

C (`cryptokey.c`):
```c
uint32_t hash32(const char *s) {
    uint32_t h = 2166136261u;
    while (*s) { h ^= (unsigned char)*s++; h *= 16777619u; }
    return h;
}
```

Zen (`crypto_key.z`):
```zen
import ffi
let lib = ffi.load_library("cryptokey.so")
let hash32_fn = ffi.get_symbol(lib, "hash32")
pub func hash32(s) { return ffi.call(hash32_fn, [s]) }
ffi.close(lib)
```

Usage:
```
import crypto_key
print(crypto_key.hash32("hello"))   # a uint32
```

## 40.13 Troubleshooting FFI

| Symptom | Cause | Fix |
|---------|-------|-----|
| symbol not found | wrong lib name / not exported | check `nm -D` |
| crash on call | NULL pointer / bad type | validate args |
| wrong result | calling convention mismatch | match platform |
| leak | handle never freed | release in `finally` |

## 40.14 Exercises

1. Wrap C `puts` to print via C.
2. Wrap a CRC32 C function; compare to pure-Zen CRC32.
3. Wrap with `try/finally` for resource release.
4. Check `errno` after a C call returning -1.
5. Call a C function 100000 times; benchmark.
