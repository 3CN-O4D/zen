# Appendix I: Platform Differences

Zen 3.0 targets Linux, macOS, and Windows. Most language behavior is identical
across platforms, but a few areas differ. This appendix documents them so you
can write portable Zen programs.

## I.1 Filesystem

| Area | Linux | macOS | Windows | Portable advice |
|------|-------|-------|---------|-----------------|
| Path separator | `/` | `/` | `\` | Use `pathlib.Path` (`a / b`) — it joins correctly per platform. |
| Line ending | `\n` | `\n` | `\r\n` | `readlines()`/`splitlines()` handle both; don't hardcode `\n` when writing cross-platform text. |
| Case sensitivity | sensitive | usually insensitive | insensitive | Don't rely on `File.z` vs `file.z` being distinct or the same. |
| Hidden files | dotfiles (`.foo`) | dotfiles | attribute-based | Use `os`/`pathlib` to list; don't assume leading-dot semantics on Windows. |
| Temp directory | `/tmp` | `/tmp` | `%TEMP%` | Use `tempfile` module or `sys.environ["TEMP"]` fallback. |

## I.2 Executables & shared libraries

| Item | Linux | macOS | Windows |
|------|-------|-------|---------|
| Shared library extension | `.so` | `.dylib` | `.dll` |
| Executable extension | (none) | (none) | `.exe` |
| FFI library loading | `dlopen` | `dlopen` | `LoadLibrary` |
| Symbol names | C names (no underscore) | C names (leading `_` historically) | `__imp__` / decorated (varies) |

**Portable advice:** When loading a native library via FFI, derive the correct
file name per platform:

```zen
import sys
func lib_name(base) {
    if sys.platform == "windows" { return base + ".dll" }
    elif sys.platform == "darwin" { return "lib" + base + ".dylib" }
    else { return "lib" + base + ".so" }
}
let lib = ffi.load_library(lib_name("mylib"))
```

## I.3 Processes & signals

| Item | Linux | macOS | Windows |
|------|-------|-------|---------|
| Signals | SIGINT, SIGTERM, etc. | SIGINT, SIGTERM, etc. | Ctrl-C / console handler (limited) |
| `sys.exit(n)` | exit code n | exit code n | exit code n |
| Background processes | `&`, `fork`/`exec` | `&`, `fork`/`exec` | `CreateProcess` semantics |

**Portable advice:** Don't rely on Unix signals on Windows. For cross-platform
program termination, use `sys.exit(n)`. Avoid `fork()`-style process control;
use `threading` and `zen build` standalone binaries instead.

## I.4 Networking

| Item | Linux | macOS | Windows |
|------|-------|-------|---------|
| Raw sockets (`AF_PACKET`) | supported | limited (BPF) | not available (use WinPcap/Npcap) |
| `SO_REUSEADDR` | supported | supported | supported |
| IPv6 | supported | supported | supported |
| Localhost | `127.0.0.1` / `::1` | `127.0.0.1` / `::1` | `127.0.0.1` / `::1` |

**Portable advice:** Raw-socket chapters (34) assume Linux. On macOS/Windows,
use higher-level TCP/UDP sockets instead of `AF_PACKET`/`SOCK_RAW`.

## I.5 FFI & calling conventions

| Item | Linux/x86-64 | macOS/x86-64 | Windows/x86-64 |
|------|--------------|--------------|----------------|
| Calling convention | System V AMD64 ABI | System V AMD64 ABI | Microsoft x64 calling convention |
| 32-bit | cdecl | cdecl | stdcall |

Zen's FFI uses the platform's default C calling convention. If you call a
function declared with a non-default convention (e.g., `__stdcall` on
32-bit Windows), you must wrap it or declare it correctly.

**Portable advice:** Keep FFI wrappers minimal and test on each target.
Prefer pure-Zen implementations where possible.

## I.6 Environment variables & paths

| Variable | Linux | macOS | Windows |
|----------|-------|-------|---------|
| `PATH` separator | `:` | `:` | `;` |
| `ZEN_PATH` separator | `:` | `:` | `;` |
| `HOME` | `/home/user` | `/Users/user` | `C:\Users\user` |

Zen's `ZEN_PATH` uses the same separator as `PATH` on each platform
(`:` on Unix, `;` on Windows). `sys.environ` reads the process environment
as-is.

## I.7 Line-ending & text encoding

Zen source is UTF-8. String I/O uses UTF-8 by default. When reading files
produced on Windows, `\r\n` line endings may appear; `splitlines()` strips
them. When writing files intended for Windows consumers, you may need to
emit `\r\n` explicitly.

## I.8 Writing portable Zen code

1. **Use `pathlib.Path`** for all path construction — never string-concatenate
   with `/` or `\`.
2. **Use `splitlines()`** instead of `split("\n")` for reading text files.
3. **Don't hardcode** `.so`/`.dll`/`.dylib` — compute per `sys.platform`.
4. **Assume case-sensitive paths** (safest common denominator).
5. **Test on each target** — CI should run Linux, macOS, and Windows.
6. **Guard platform-specific code** with `sys.platform` checks.
7. **Avoid raw sockets and signals** in cross-platform tools.

## I.9 Platform quick test

```zen
import sys
print("platform: " + sys.platform)
print("separator: " + (if sys.platform == "windows" { "\\" } else { "/" }))
print("home: " + str(sys.environ["HOME"] ?? sys.environ["USERPROFILE"] ?? ""))
```

Sample output:
```
platform: linux
separator: /
home: /home/bard
```
