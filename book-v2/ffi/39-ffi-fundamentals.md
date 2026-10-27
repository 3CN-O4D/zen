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


## 39.12 Worked examples — FFI fundamentals

### Conceptual model

Zen's FFI lets you call C functions in shared libraries (`.so`, `.dll`,
`.dylib`) and pass data across the boundary. Values are marshalled: Zen ints,
floats, strings, bytes, lists, dicts map to C primitives/pointers/opaque
handles.

Typical flow:
1. `load_library("libfoo.so")` → library handle
2. `get_symbol(lib, "foo_function")` → callable symbol
3. Call the symbol with Zen arguments; Zen marshals to C.
4. Receive the C return value, marshalled back to Zen.
5. Close/drop the handle when done.

### Example 1: Call a simple C function (conceptual pseudocode)

Assuming a C library `libmathutils.so` with:
```c
// mathutils.c
int add(int a, int b) { return a + b; }
double scale(double v, double f) { return v * f; }
```

Zen side (exact API depends on build):
```zen
import ffi

let lib = ffi.load_library("libmathutils.so")
let add_fn = ffi.get_symbol(lib, "add")
let result = ffi.call(add_fn, [21, 21])   # 42
print(result)

let scale_fn = ffi.get_symbol(lib, "scale")
let scaled = ffi.call(scale_fn, [10.0, 3.5])   # 35.0
print(scaled)

ffi.close(lib)
```

### Example 2: Working with strings across FFI

```zen
let lib = ffi.load_library("libcstring.so")
let len_fn = ffi.get_symbol(lib, "string_length")
let n = ffi.call(len_fn, ["hello"])   # 5
print(n)
```

Zen marshals a Zen string to a C `const char*`. The C side must not modify it
unless the API specifies a mutable buffer (pass `bytes` or a pointer handle).

### Example 3: Pointers and handles

Some APIs return opaque pointers (e.g., file handles, database connections).
Zen represents these as foreign handles. You must track their lifetime and
release them via a C `destroy`/`free` function.

```zen
let db = ffi.call(open_db_fn, ["data.db"])   # foreign handle
try {
    let row = ffi.call(query_fn, [db, "SELECT * FROM t"])
    print(row)
} finally {
    ffi.call(close_db_fn, [db])   # always release
}
```

### Example 4: Callbacks (Zen function passed to C)

If the C API accepts a function pointer, Zen can pass a closure. The FFI layer
creates a trampoline so C can call back into Zen.

```zen
let cmp = ffi.get_symbol(lib, "compare")
ffi.call(cmp, [list_a, list_b, fn(a, b) => a - b])
```

**Caution:** the Zen callback must not outlive the Zen closure's scope, and
must not raise exceptions that cross the C boundary uncaught.

## 39.13 Data type mapping (typical)

| Zen type | C equivalent | Notes |
|----------|--------------|-------|
| int | int32_t / int64_t | sign depends on value |
| float | double | |
| string | const char* | NUL-terminated; Zen owns it |
| bytes | unsigned char* | raw buffer |
| list | array / pointer | marshalled copy |
| dict | not directly | serialize first (JSON/msgpack) |
| nil | NULL | |
| handle | void* | opaque pointer |

## 39.14 Safety considerations

1. **Memory ownership:** know who frees what. Zen-allocated strings passed to
   C must remain valid during the call; C-allocated memory returned to Zen
   must be freed by Zen or the C side per convention.
2. **Null pointers:** dereferencing a NULL handle crashes. Check before use.
3. **Type mismatches:** passing a float where C expects int corrupts data.
4. **Calling convention:** cdecl/stdcall must match the platform/library.
5. **Thread safety:** a C library not thread-safe must be called with a Zen
   Mutex held.
6. **Exceptions across boundary:** don't let a Zen throw unwind through C
   frames; catch at the Zen edge.

## 39.15 Debugging FFI

- `nm -D libfoo.so` to list exported symbols.
- `objdump -T libfoo.so` for dynamic symbols.
- Confirm symbol names match exactly (C name mangling).
- Start with a trivial function (add two ints) before structs/pointers.
- Log every FFI call and return value.

## 39.16 Exercises

1. Load a shared library and call a function that adds two ints.
2. Call a C function that takes a string and returns its length.
3. Pass a bytes buffer to C and read back modified bytes (if supported).
4. Open an opaque handle, use it, then close it in `finally`.
5. Pass a Zen callback to a C `qsort`-style comparison function.
6. Write a wrapper `ZenMath` module exposing `add`, `scale` via FFI.
7. Demonstrate a null-handle guard before calling C.
8. Use `nm -D` to inspect a library's exported symbols.
9. Wrap a C file-reading function with error handling.
10. Benchmark a pure-Zen function vs the same logic via FFI.

