# Appendix H: Troubleshooting & FAQ

A practical troubleshooting guide and frequently asked questions for Zen 3.0,
organized by symptom. Each entry gives the likely cause, a diagnostic step, and
a fix.

## H.1 Installation & CLI

### "zen: command not found"

**Cause:** The `zen` binary is not on your `PATH`, or wasn't installed.
**Diagnostic:**
```bash
which zen
ls -la $(dirname $(which zen))  # if which finds it
```
**Fix:**
- Add the install directory to `PATH`: `export PATH="$HOME/.local/bin:$PATH"`
- Or reinstall from the release tarball/deb.
- From source: `cargo build --release --package zen-cli --bin zen` then use
  `target/release/zen`.

### `zen --version` prints nothing / crashes

**Cause:** Missing shared libraries or a corrupt install.
**Fix:** Reinstall from a release binary; on Linux ensure `libssl`/`openssl`
compat packages are present if the binary was dynamically linked.

### `zen check` reports errors I don't understand

**Cause:** Syntax or type errors in your source.
**Fix:** Read the error code (e.g. `Z0301`) and run `zen explain Z0301` for the
full explanation, common causes, and a suggested fix.

## H.2 Runtime errors

### Z0304 DivideByZero

**Cause:** Dividing or modulo by zero (`x / 0`, `x % 0`).
**Fix:** Guard the divisor:
```zen
if b != 0 { result = a / b } else { result = nil }
```

### Z0302 IndexError / Z0303 KeyError

**Cause:** Index out of range on a list, or missing key on a dict.
**Fix:** Check bounds / membership first:
```zen
if i < len(xs) { print(xs[i]) }
if k in d { print(d[k]) } else { print(d[k] ?? "default") }
```

### Z0305 ValueError from int() / float()

**Cause:** `int("abc")` or `float("not a number")`.
**Fix:** Validate or wrap in try:
```zen
func safe_int(s) {
    try { return int(s) } catch as e { return nil }
}
```

### Z0310 MaxRecursionDepth / Z0317 StackOverflow

**Cause:** Too-deep recursion (e.g., naive Fibonacci on large n).
**Fix:** Convert to iteration, or raise an explicit depth guard:
```zen
func fib(n, depth = 0) {
    if depth > 1000 { throw "too deep" }
    ...
}
```

### "Program hangs / doesn't exit"

**Cause:** A background thread, channel, or server loop never terminates.
**Fix:**
- Join all threads before `main` returns.
- Close channels so consumers' `recv()` returns `nil`.
- Servers: handle `SIGINT`/Ctrl-C and break the accept loop.

## H.3 Networking

### Connection refused (Z0408)

**Cause:** Nothing is listening on the target host:port.
**Fix:** Start the server; verify the port with `ss -tlnp` or `netstat`; check
firewall rules.

### Address already in use (Z0413)

**Cause:** Binding a port still in TIME_WAIT from a previous run.
**Fix:** Set `SO_REUSEADDR` on the socket before `bind()`, or wait a minute.

### DNS resolution fails (Z0411 / NXDOMAIN)

**Cause:** Domain doesn't exist, or no DNS resolver configured.
**Fix:** Test with `dig example.com` or `nslookup`; try a public resolver
(1.1.1.1, 8.8.8.8); check `/etc/resolv.conf`.

### TLS certificate verify failed

**Cause:** Self-signed, expired, or untrusted certificate; hostname mismatch.
**Fix:** Use `verify=true` in production and fix the cert. For testing only,
`verify=false` bypasses validation (dangerous — never in production).

### Requests hang indefinitely

**Cause:** No timeout set; the server never responds.
**Fix:** Always pass `timeout=` to `requests.get/post/etc.`.

## H.4 Concurrency

### Race condition (counter ends up too low)

**Cause:** Unsynchronized read-modify-write on shared state.
**Fix:** Guard with a Mutex:
```zen
mu.lock()
counter = counter + 1
mu.unlock()
```

### Deadlock (program freezes)

**Cause:** Two threads each hold a lock the other needs.
**Fix:** Always acquire locks in the same global order; keep critical sections
small; never call blocking code (sleep, network) while holding a lock.

### Consumer blocks forever on channel recv

**Cause:** Producer never closed the channel, or sent fewer values than expected.
**Fix:** `ch.close()` when done producing; use `select`-like timeouts if
available.

## H.5 Files & I/O

### FileNotFound (Z0401)

**Cause:** Wrong path, relative path depends on cwd, file missing.
**Fix:** Use absolute paths or `pathlib.Path.resolve()`; check `exists()` first.

### Permission denied (Z0402)

**Cause:** File/directory not readable/writable by the process user.
**Fix:** Fix file permissions (`chmod`) or run as the right user.

## H.6 FFI & pymport

### Symbol not found (Z0501)

**Cause:** The C function isn't exported, or the wrong library/version loaded.
**Fix:** `nm -D libfoo.so | grep function_name`; confirm the library is on the
link path and the symbol is exported (not `static`).

### Python module not found (Z0506)

**Cause:** Module not installed, or `PYTHONPATH`/venv not set.
**Fix:** Activate the venv; `export PYTHONPATH="$PWD/pylibs"`; install the
module with pip.

## H.7 FAQ

**Q: Is `0` falsy?**
**A:** Yes. Falsy values are `false`, `nil`, `null`, `0`, `0.0`, `""`,
`bytes([])`, `[]`, `#{}`, `%{}`, `#()`. Everything else is truthy.

**Q: `int / int` gives a float. How do I get integer division?**
**A:** Use `//` (floor division): `7 // 2 == 3`.

**Q: `nan == nan` is false. How do I detect NaN?**
**A:** `x != x` is true only for NaN, or use `math.isnan(x)`.

**Q: Are dict keys ordered?**
**A:** No — iteration order is not guaranteed. Sort keys explicitly if order
matters.

**Q: Can I mutate a `let` list?**
**A:** Yes — `let` prevents rebinding the variable, not mutating the object.
`lst.push(3)` works; `lst = [4]` errors.

**Q: How do I copy a list without sharing?**
**A:** `list(lst)` copies the top level (shallow). Nested lists are still
shared; write a recursive `deep_copy` for full independence.

**Q: Does Zen have tail-call optimization?**
**A:** No guaranteed TCO. Deep recursion can overflow the stack; use iteration
for large depths.

**Q: Is the JIT on by default?**
**A:** No — the JIT (M11) is optional and feature-gated (`--features jit`).
The interpreter is the default and always correct.

**Q: How do I explain an error code?**
**A:** `zen explain Z####` — e.g. `zen explain Z0301`.

**Q: Where do I report a bug?**
**A:** File an issue on the Zen repository with the error code, `zen explain`
output, and a minimal reproducer.

## H.8 Quick diagnostic checklist

When something goes wrong, work down this list:

1. **Read the error message** — note the code (Z####) and file:line:col.
2. **Run `zen explain Z####`** — read the cause and fix.
3. **Check `zen check file.z`** — catches compile errors before running.
4. **Add `print()` statements** — narrow down where it fails.
5. **Verify inputs** — are files present, ports open, arguments correct?
6. **Check `ZEN_DEBUG=1 zen run file.z`** — verbose runtime logging (if your
   build supports it).
7. **Simplify** — reduce to a minimal reproducer; the bug usually becomes
   obvious.
