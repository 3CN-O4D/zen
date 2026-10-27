# Chapter 39: FFI Fundamentals

Zen's `ffi` module lets you call functions in native C shared libraries (`.so`, `.dll`, `.dylib`) using libffi, with true C ABI support. This chapter covers loading libraries, finding symbols, calling functions, and managing memory.

## 39.1 Loading a library

```zen
import ffi

// Load a shared library
let libc = ffi.load("libc.so.6")        // Linux
// let libc = ffi.load("libc.dylib")     // macOS
// let libc = ffi.load("msvcrt.dll")     // Windows

print(libc)  // library handle (integer)
```

`ffi.load(path)` returns a library handle you pass to `ffi.symbol` and `ffi.call`.

## 39.2 Finding symbols

```zen
import ffi

let libc = ffi.load("libc.so.6")

// Get the address of a function
let puts_addr = ffi.symbol(libc, "puts")
let printf_addr = ffi.symbol(libc, "printf")

print(puts_addr)    // integer address
print(printf_addr)  // integer address
```

`ffi.symbol(lib, name)` returns the address of `name` in `lib`.

## 39.3 Calling functions

```zen
import ffi

let libc = ffi.load("libc.so.6")
let puts_addr = ffi.symbol(libc, "puts")

// Call puts("hello") — returns int
let result = ffi.call(puts_addr, ["hello"], {"type": "int"})
print(result)  // number of chars printed (or 0)
```

### `ffi.call(address, args, ret?)`

- `address` — function address (from `ffi.symbol`)
- `args` — list of argument values
- `ret` — return type spec (optional, defaults to `int`)

### Argument types

Arguments can be:
- **Integers** — passed as C `int`/`long`
- **Floats** — passed as C `double`
- **Strings** — passed as C `char*` (null-terminated)
- **`{"type": "double", "value": 1.5}`** — typed scalar
- **`{"buf": "hex..."}`** — raw byte buffer
- **`{"type": "bytes", "len": n}`** — buffer of size n

### Return types

| `ret` | Meaning |
|-------|---------|
| `nil` or omitted | `int` (default) |
| `"double"` | double |
| `"void"` | no return value |
| `"str"` | char* → string |
| `{"type": "bytes", "len": n}` | raw buffer of n bytes |

### Examples

```zen
import ffi

let libc = ffi.load("libc.so.6")

// puts("hello") → int
let r1 = ffi.call(ffi.symbol(libc, "puts"), ["hello"])
print(r1)

// printf("value: %d\n", 42) → int
let r2 = ffi.call(ffi.symbol(libc, "printf"), ["value: %d\n", 42])
print(r2)

// sqrt(2.0) → double
let sqrt_addr = ffi.symbol(libc, "sqrt")
let r3 = ffi.call(sqrt_addr, [{"type": "double", "value": 2.0}], "double")
print(r3)  // 1.4142135623730951
```

## 39.4 Memory management

```zen
import ffi

// Allocate memory
let ptr = ffi.malloc(1024)
print(ptr)  // address

// Reallocate
let ptr2 = ffi.realloc(ptr, 2048)

// Free
ffi.free(ptr)

// Read / write memory
ffi.write(ptr, "hello")
let s = ffi.read(ptr, 5)
print(s)  // hello

// String conversion
let s2 = ffi.str(ptr)
print(s2)  // hello

// Set bytes at offset
ffi.set(ptr, 0, {"buf": "776f726c64"})  // "world" in hex
let s3 = ffi.read(ptr, 5)
print(s3)  // world

// Size of types
print(ffi.sizeof("int"))    # 4
print(ffi.sizeof("double")) # 8

// Alignment
print(ffi.alignof("double")) # 8

// errno
print(ffi.errno())
print(ffi.strerror(ffi.errno()))
```

### Memory functions

| Function | Description |
|----------|-------------|
| `ffi.malloc(size)` | Allocate `size` bytes |
| `ffi.realloc(ptr, size)` | Reallocate |
| `ffi.free(ptr)` | Free memory |
| `ffi.read(ptr, n)` | Read `n` bytes as string |
| `ffi.write(ptr, data)` | Write string to memory |
| `ffi.str(ptr)` | Read null-terminated string |
| `ffi.set(ptr, offset, bytes)` | Write raw bytes at offset |
| `ffi.sizeof(type)` | Size of C type |
| `ffi.alignof(type)` | Alignment of C type |
| `ffi.errno()` | Last errno value |
| `ffi.strerror(errno)` | Error message for errno |

## 39.5 Complete example: calling `qsort`

```zen
import ffi

let libc = ffi.load("libc.so.6")
let qsort_addr = ffi.symbol(libc, "qsort")

// qsort(base, nel, width, compar)
// We'd need a callback for compar — FFI doesn't support Zen closures as
// C callbacks directly. See Chapter 40 for workarounds.

print("qsort loaded at " + str(qsort_addr))
```

## 39.6 Safety warnings

⚠️ **FFI is unsafe.** Zen cannot validate:
- Whether the address is a real function
- Whether argument types match the C signature
- Whether memory accesses are in bounds

Misuse can crash the process or corrupt memory. Only load libraries you trust, and use `ffi.allow`/`ffi.deny` to restrict which libraries can be loaded (see Chapter 40).

## 39.7 Quick reference

```zen
import ffi

let lib = ffi.load("libc.so.6")
let addr = ffi.symbol(lib, "puts")
let r = ffi.call(addr, ["hello"])            # int return
let r = ffi.call(addr, ["hello"], "int")     # explicit int
let r = ffi.call(addr, [4.5], "double")      # double return

let ptr = ffi.malloc(1024)
ffi.write(ptr, "data")
let s = ffi.read(ptr, 4)
ffi.free(ptr)

print(ffi.sizeof("int"))     # 4
print(ffi.alignof("double")) # 8
print(ffi.errno())
print(ffi.strerror(2))
```

## 39.8 Exercises

1. Load `libc.so.6` and call `puts` with a string.
2. Call `printf` with a format string and an integer.
3. Call `sqrt` from `libm` and print the result as a double.
4. Allocate memory with `ffi.malloc`, write a string, read it back, free it.
5. Check `ffi.errno()` after a failing operation and print `ffi.strerror`.
