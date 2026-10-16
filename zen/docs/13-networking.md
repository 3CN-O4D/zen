# 13 — Networking: 7 layers, and what happens to 48 modules

> **Status:** Draft for review. The rulings a reviewer should attack first are §2.2 (the layer
> split, and whether a script gets the raw layers at all), §4 (raw sockets are a *language-visible*
> feature behind a flag, or a `sys` capability), §5.4 (the TLS 1.3-only ruling and what it costs
> corporate proxies), §8 (the module dispositions — 10 of the 25 network-side modules are dropped
> outright and 7 of the 25 are WhatsApp, so "drop" needs a reason per row and a bytes-per-reason
> ratio), and §9.2 (the SSRF posture, which is a design decision and not a checklist).
> The premise this document was asked to accept — that Zen 2.x ships gRPC/tonic, MQTT, SMTP, IMAP
> and FTP — is **partly false**, and §8.1 says exactly which half and what replaces it. §12.6 records the
> eight factual errors this document contained on its first pass, including a claim about `std/nmap.z`
> that was wrong in the way that mattered most.
> **Milestone:** M10 (Concurrency + 7-layer net) for §2–§7, §9 and §10. The L7 `grpc` and `mqtt`
> modules are M10.13 and M10.14, the last two steps of M10, and §8.2 explains why the two
> lowest-value layers land last.

---

## Contents

1. [Purpose and the ruling](#1-purpose-and-the-ruling)
2. [The seven layers](#2-the-seven-layers)
3. [The runtime, the handle table, and the three ways a socket is owned](#3-the-runtime-the-handle-table-and-the-three-ways-a-socket-is-owned)
4. [Layer 1–2: sockets and packet I/O](#4-layer-12-sockets-and-packet-io)
5. [Layer 3–4: transport and TLS](#5-layer-34-transport-and-tls)
6. [Layer 5–6: session and presentation](#6-layer-56-session-and-presentation)
7. [Layer 7: application protocols](#7-layer-7-application-protocols)
8. [Module inventory and dispositions](#8-module-inventory-and-dispositions)
9. [Security posture](#9-security-posture)
10. [Test plan](#10-test-plan)
11. [Milestone sequencing](#11-milestone-sequencing)
12. [Rulings a reviewer should double-check](#12-rulings-a-reviewer-should-double-check)

**Consumes:** the `ResKind` table and the `ResKind::TlsSocket` contract from
[`11-ffi-handles.md`](11-ffi-handles.md) §4.3; the `BlockingClass` field and the one-runtime-per-task
rule from [`12-concurrency.md`](12-concurrency.md) §7.3; the `ResKind::DnsResolver` slot from
[`11-ffi-handles.md`](11-ffi-handles.md) §4.4; the `error{kind}` value type from
[`00-blueprint-review.md`](00-blueprint-review.md) §3.
**Governs:** the `--allow-raw-sockets` flag that §7.3 of `11-ffi-handles.md` promises, the
loopback-only test rule that §11 of `12-concurrency.md` assumes, and the module list that
`15`–`17` (the module system and packaging milestones) must ship.

---

## 1. Purpose and the ruling

**The ruling, in one line: wrap mature Rust crates at every layer, expose the raw layers behind a
flag, and ship ten application protocols instead of thirty thin ones.**

[`00-blueprint-review.md`](00-blueprint-review.md) §3 endorses the 7-layer decomposition as an
*ordering* of work and is equally clear that Zen must **wrap** the layers rather than reimplement
them. This document is the wrapping plan: which crate owns which layer, what the Zen-visible API at
each layer looks like, which of Zen 2.x's 30 networking modules survive, and which of them were
never real to begin with.

### 1.1 The three facts that shape everything else

| Fact | Number | Why it decides the plan |
|---|---|---|
| The networking in Zen 2.x is **tiny** | 15 Rust files, 72 203 bytes, **4.6 %** of `src/*.rs` | Most of it is a name table. `src/socket.rs` is 23 lines and 1 155 bytes and contains *no* implementation — just 13 `Value::NativeFunction("socket_…")` strings. The old tree has no transport to port; it has a registry |
| The **WhatsApp** stack is enormous | 7 files, 307 687 bytes, **19.5 %** of `src/*.rs`, **48.8 %** of `src/*.rs` excluding `runtime.rs` | The single largest thing in the tree is a reverse-engineered client for a third-party protocol with no test coverage. §8.3 drops it, and the argument is 307 687 bytes of opportunity cost, not ideology |
| Two of the "existing" protocols **do not exist** | `mqtt`: 0 implementations. `grpc`/`tonic`: 0 implementations. `hydra`: documented in the help text at `src/runtime.rs:19255` and implemented **nowhere** | The brief asked for a plan that wraps "the existing gRPC, MQTT, SMTP, IMAP and FTP modules". SMTP, IMAP and FTP are real. gRPC and MQTT are not, and `hydra` is a documented ghost. §8.1 gives the receipts, and §7.7–§7.8 *add* the two missing layers instead of pretending to port them |

### 1.2 What is explicitly not in this document

Raw packet capture depth, kernel offload, multicast, IPsec, a proxy-protocol client, an HTTP/3
interop matrix, and TLS client-certificate authentication. Each has a one-line reason in §12.4, and
each is a `zen` issue rather than a gap in the language.

---

## 2. The seven layers

### 2.1 The decomposition, and what each layer is *for* in Zen

The OSI model is a poor API design tool and a good *dependency* tool: it tells you that a bug in
your URL parser cannot possibly be a bug in your `sendto`, because the layers above cannot see
the layers below. The table below is the dependency order, the crate that owns each layer, and the
Zen-visible surface.

| Layer | What it does | Owning crate | Zen-visible surface | Shipped in |
|---|---|---|---|---|
| **L7** Application | HTTP/1.1, HTTP/2, WebSocket, gRPC, MQTT, SMTP, IMAP, POP3, FTP, SSH, Telnet, HTTP | `hyper` 1, `tonic` 0.14, `rumqttc-next` 0.34, `lettre` 0.11, `imap-next` 0.3, `suppaftp` 12, `russh` 0.63, hand-rolled textproto | `http`, `ws`, `grpc`, `mqtt`, `smtp`, `imap`, `pop3`, `ftp`, `ssh`, `telnet` | M10.1–M10.12 / M10.13–M10.14 |
| **L6** Presentation | TLS 1.3, `bytes` ⇄ typed codecs, `Content-Encoding` | `rustls` 0.23, `flate2` 1, `base64` 0.22 | (invisible — `bytes` is already a language type) | M10 |
| **L5** Session | connection lifecycle, keep-alive, the *protocol state machine* | the L7 crate | `conn.close()`, `conn.is_open()` | M10 |
| **L4** Transport | TCP, UDP, QUIC; flow control; `poll`/`select` semantics | `tokio` 1 (`TcpStream`, `UdpSocket`) | `tcp`, `udp`, `quic` | M10 |
| **L3** Network | IP addressing, `getsockname`/`getpeername`, packet headers | `socket2` 0.6, `pnet` 0.35 (raw only) | `ip` — **flag-gated** | M10 |
| **L2** Data link | Ethernet, ARP, interface enumeration, sniffing | `pnet_datalink` 0.35 | `net.link` — **flag-gated** | M10 |
| **L1** Physical | the NIC | the OS | — | never |

**The rule this table encodes: a Zen module may only import from its own layer and below.** `http`
imports L6 and L5. `net.link` may not import `http`. Enforced where? Not by the compiler — the
layer boundary is a *code review* rule in `crates/zen-std-net/src/lib.rs`, which declares one Rust
module per layer and whose `mod` graph makes an upward import impossible within the crate. That is
a real enforcement mechanism, not a convention: Rust module privacy makes `l7` unable to name
`l1::raw` if `l1` is a private child of the crate root and the L7 modules only import through a
`pub(crate) trait Transport` defined in L4. §3.2 gives the trait, and it is the load-bearing
interface of the whole plan.

### 2.2 Crates, versions, and the exact feature sets

Versions are the current stable line at the time of writing and **must be re-resolved at M10
kickoff** — a plan that pins a version it cannot build is a plan that gets skipped. The feature
lists are the part that is not negotiable, and each one is there because of a specific
misfeature of the default set.

```toml
# crates/zen-std-net/Cargo.toml — the whole network stack, one crate, four layers.

[features]
default = ["tls", "http", "textproto"]
tls        = ["dep:tokio-rustls", "dep:rustls", "dep:webpki-roots", "dep:rustls-pki-types"]
http       = ["dep:hyper", "dep:hyper-util", "dep:http-body-util", "dep:bytes", "dep:http"]
websocket  = ["dep:tokio-tungstenite", "http"]
raw        = ["dep:pnet", "dep:pnet_datalink", "dep:pnet_packet"]   # §4.4
quic       = ["dep:quinn", "tls"]
grpc       = ["dep:tonic", "dep:prost", "dep:prost-types", "tls"]  # §7.7
mqtt       = ["dep:rumqttc-next", "tls"]                            # §7.8
mail       = ["dep:lettre", "dep:imap-next", "dep:suppaftp", "dep:russh"]
textproto  = ["dep:httparse", "dep:russh"]                          # pop3 + telnet, §7.9
default-tls = ["rustls/webpki-roots"]                               # NOT native-tls

[dependencies]
# ── L1–L4 ──────────────────────────────────────────────────────────────────
tokio        = { workspace = true }              # 1.x; features are already
                                                # right in the workspace
socket2      = { version = "0.6", features = ["all"] }
rustls       = { workspace = true }              # 0.23
tokio-rustls = { version = "0.26", default-features = false,
                 features = ["ring", "tls12", "logging"] }
rustls-pki-types = { version = "1", features = ["std"] }
webpki-roots = { version = "1" }                 # Mozilla's root set, compiled in
pnet         = { version = "0.35", optional = true }
pnet_datalink= { version = "0.35", optional = true }
pnet_packet  = { version = "0.35", optional = true }
quinn        = { version = "0.11", optional = true }
quinn-proto  = { version = "0.11", optional = true }

# ── L6–L7 ──────────────────────────────────────────────────────────────────
hyper        = { version = "1", features = ["http1", "http2", "client"] }
hyper-util   = { version = "0.1", features = ["client", "client-legacy", "tokio"] }
http         = { version = "1" }
http-body    = { version = "1" }
http-body-util = { version = "0.1" }
bytes        = { version = "1" }
tokio-tungstenite = { version = "0.28", optional = true, default-features = false,
                      features = ["connect", "handshake"] }
tonic        = { version = "0.14", optional = true, default-features = false,
                 features = ["transport", "codegen", "prost"] }
prost        = { version = "0.14", optional = true }
prost-types  = { version = "0.14", optional = true }
rumqttc-next = { version = "0.34", optional = true, default-features = false,
                 features = ["use-rustls-ring"] }
lettre       = { version = "0.11", optional = true, default-features = false,
                 features = ["tokio1-rustls", "ring", "webpki-roots", "smtp-transport"] }
imap-next    = { version = "0.3",  optional = true }   # §7.6: 0.3.x is current; there is no 0.9
suppaftp     = { version = "12",   optional = true, features = ["tokio-rustls-ring"] }
russh        = { version = "0.63", optional = true, default-features = false,
                 features = ["ring"] }   # §7.5: `ring`, not `aws-lc-rs`; see §5.4
httparse     = { version = "1", optional = true }
flate2       = { workspace = true }
```

| Crate | Feature choice that is not the default, and why |
|---|---|
| `tokio-rustls` | **`default-features = false` + explicit `ring`.** The default pulls `aws-lc-rs`, which needs a C compiler and a NASM on Windows. `zen`'s MSRV is 1.95 and its build must work on a machine with nothing but a Rust toolchain; `ring` is the boring choice that builds everywhere. `native-tls` is **banned outright** (see §5.4) |
| `tokio-tungstenite` | `default-features = false` — the defaults include `native-tls` and `handshake` (the client); we want the server-side path too and we want *our* TLS stack, not a second one |
| `rumqttc-next` | `use-rustls-ring`, not the default `use-native-tls`, for the same reason as everything else. See §7.8's ruling on the crate's name |
| `lettre` | `tokio1-rustls-tls` rather than `tokio1-native-tls`; and **not** `builder`, which is a sync API and would drag in a second executor |
| `tonic` | `codegen` + `prost`, but **not** `tls` — TLS is a `tokio-rustls` concern at L6 and letting `tonic` install its own is how a project ends up with two root stores and no idea which one is live |
| `hyper` | `client` + `http1` + `http2`, and **`server` is deliberately off** in the default build. An embedded HTTP server is a real feature and it is not in M10; §12.4 says so rather than shipping a half-built one behind a flag that nobody tests |
| `rustls` | No `aws-lc-rs` provider, no `dangerous_configuration`. §5.4's ruling is that there is no escape hatch; a test asserts the symbol is not linked |
| `webpki-roots` | Not `rustls-native-certs`, because the *system* store is the single most common way a Rust TLS client ships a revocation-unaware, platform-inconsistent trust decision. §5.4 argues it in full; it is the ruling most likely to be reversed by a user request for corporate-proxy support |

### 2.3 The one interface everything above L4 is written against

```rust
// crates/zen-std-net/src/l4/mod.rs

/// The only thing an L5–L7 module is allowed to ask of the network.
///
/// Everything above this trait is protocol logic and is unit-testable with
/// a mock; everything below it is real I/O and is not. The trait exists so
/// that §10's test plan can put a `MockTransport` under `http`, `mqtt` and
/// `smtp` and assert wire behaviour without a socket.
pub trait Transport: Send + Sync {
    type Conn: AsyncRead + AsyncWrite + Unpin + Send + 'static;

    /// Resolve and connect. `host` may be a name, a literal address, or a
    /// `HostPort` — L4 does the resolution, L3 hands back the addresses,
    /// and §9.2's SSRF check runs *between* them.
    fn connect(&self, host: &str, port: u16) -> BoxFuture<'static, Result<Self::Conn, NetError>>;

    /// One datagram. Unspecified on stream transports, and the method
    /// does not exist on `TcpTransport` — `UdpTransport` is a separate
    /// type for the same reason `File` and `Socket` are separate
    /// `ResKind`s.
    fn local_addr(&self) -> Result<SocketAddr, NetError>;
}

/// The object a Zen `handle` of `ResKind::Socket` points at. Not the
/// `tokio` type, so the handle table does not have to name `tokio` and a
/// `ResKind::Socket` can hold a `UdpSocket` or a `quinn::Connection`
/// without a second variant.
#[async_trait]
pub trait Stream: Send + Sync {
    fn read<'a>(&'a self, buf: &'a mut BytesMut) -> BoxFuture<'a, Result<usize, NetError>>;
    fn write_all(&self, buf: &[u8]) -> BoxFuture<'_, Result<(), NetError>>;
    fn shutdown(&self) -> BoxFuture<'_, Result<(), NetError>>;
    fn local_addr(&self) -> Result<SocketAddr, NetError>;
    fn peer_addr(&self) -> Result<SocketAddr, NetError>;
}
```

**Why `Stream` is a trait and not an enum.** Three protocols in this document need a
`stream` that is not a TCP socket: `quic` gives a `quinn::Connection` with a
`send_datagram`/`recv_datagram` pair and *no* ordered byte stream, WebSocket is a byte stream with
a frame layer, and `ssh` is several interleaved channels over one socket. An enum with four
variants means every call site matches on it, and a fifth protocol means editing every call site.
A trait object in a `Box<dyn Stream>` inside a `ResKind::Socket` slot costs one pointer chase
per call, which is nothing next to a syscall, and it is the difference between adding a protocol in
one file and adding it in eleven.

---

## 3. The runtime, the handle table, and the three ways a socket is owned

### 3.1 Every socket is a handle, and that is not a detail

Per [`11-ffi-handles.md`](11-ffi-handles.md) §4, a socket a Zen script holds is a
`ResKind::Socket` handle: a `Value` pointing at a 24-byte `HandleObj` whose token indexes a slot
holding `Box<dyn Any + Send>`. The consequences for this document are concrete and all of them are
good news:

| Consequence | Detail |
|---|---|
| **No socket is a Zen integer** | `socket.open` returns a `handle`. The old `src/socket.rs` returns an `int` fd, and `20_sockets.z` compares fd numbers — that test is ported, not kept (§10.4) |
| **Closing is explicit and once** | `close(h)` frees the slot; a second `close` is `NativeError: … is already closed`. `shutdown` is separate, because a half-closed socket is a protocol state and silently doing it would change observable behaviour ([`11-ffi-handles.md`](11-ffi-handles.md) §4.3) |
| **`bool(h)` is liveness** | A closed socket handle makes `bool(h)` false, so a script can poll its own state without a syscall |
| **The `Drop` is right** | `Box<dyn Any>` holds a `socket2::Socket` or a `tokio::net::TcpStream`; dropping the `Vm` closes every socket deterministically, which is what makes the test suite able to bind the same loopback port 400 times without `TIME_WAIT` exhaustion |
| **Two handles, one socket** | `dup` gives two slots over one open file description, so `h1 == h2` is false and they share a file position — exactly as [`11-ffi-handles.md`](11-ffi-handles.md) §8.2's `dup` row now says |

### 3.2 The blocking rule, restated for network natives

[`12-concurrency.md`](12-concurrency.md) §7.3 gives every process exactly one tokio runtime, on a
dedicated thread, and every blocking `spawn`ed task gets its own runtime. Network natives declare
a `BlockingClass`:

| Native | `BlockingClass` | Why |
|---|---|---|
| `socket.open` / `connect` / `accept` | `MayBlock` | A connect to an unroutable address takes the kernel's full SYN retry budget — up to 130 s on Linux. §10's test 9 is exactly this case |
| `socket.send` / `recv` | `MayBlock` | `recv` on a stream with no data blocks until the peer speaks or the timeout fires |
| `socket.listen` / `bind` | `NeverBlocks` | Both are local syscalls that cannot wait on a peer |
| `socket.getsockname` / `getpeername` / `getsockopt` | `NeverBlocks` | Pure kernel reads |
| `http.get` and every L7 native | `MayBlock` | Every one of them is, transitively |
| `dns.resolve` (system resolver) | `MayBlock` | `getaddrinfo` blocks. The `hickory` resolver does not, but the system resolver is the compatibility path |
| `http.get` with `.timeout(0)` on a cached connection | `NeverBlocks` | The only L7 native that can claim it, and only on a keep-alive hit |

**The consequence that will bite someone:** a `MayBlock` native cannot appear in a `pmap` closure
([`12-concurrency.md`](12-concurrency.md) §7.6 ruling 9), and *every* network call is `MayBlock`.
So `[urls].pmap(fn(u) { http.get(u) })` is a **compile error**, and the correct spelling is

```zen
// The idiomatic parallel fetch, and it is in the docs because the error
// message alone is not enough.
let bodies = urls.pmap_spawned(fn(u) { http.get(u).text })
// or, better, because HTTP/1.1 is the case where a real client pool wins:
let bodies = http.parallel(urls, {max: 8}) { u in http.get(u).text }
```

`http.parallel` is a single native that owns a connection pool and a `Semaphore`, because that is
the thing a user actually wants and it is not expressible as a `pmap` of blocking calls. It is the
only concurrency helper in this document and §7.1 says why it is allowed to exist when
`http.get` composes.

### 3.3 The resolver is a `ResKind`, not a global

`hickory-resolver` 0.25 keeps caches, a connection pool and a `LruCache` per resolver instance. A
process-global resolver is the obvious implementation and it is wrong for two reasons: a Zen
script can run on any thread, and a shared mutable resolver needs a lock that a per-thread resolver
does not — and §9.1's SSRF check needs to be *per script*, not per process.

**Ruling: the resolver is a handle, `net.resolver()`, backed by `ResKind::DnsResolver` (already in
[`11-ffi-handles.md`](11-ffi-handles.md) §4.3's table).** It is created on first use per `Vm`,
cached in `Vm::resolver`, and dropped with the `Vm`, which is what drops the UDP connection to the
nameserver. A script that wants a specific nameserver builds its own:

```zen
let r = net.resolver({nameservers: ["1.1.1.1:53"], cache_size: 256})
let addrs = r.resolve("example.com", {kind: "A"})
let v6    = r.resolve("example.com", {kind: "AAAA"})     // §4.3, and the flag
```

Two things this gets for free: `net.resolver()` handles `resolve`, `lookup` and `query` — the three
names `src/dns.rs` registers, two of which are the *same* native (`lookup` is an alias for
`dns_resolve`, a fact the old code hides) — and the resolver's own 5-second timeout is a
per-resolver setting rather than a process-wide `/etc/resolv.conf` accident.

---
## 4. Layer 1–2: sockets and packet I/O

### 4.1 What the old tree actually has, and what it is worth

`src/socket.rs` is 23 lines: a `Value::Dict` of 13 `NativeFunction` name strings. The
implementations live in `src/runtime.rs` and are not in this document's scope, but three facts about
them are, because they are the reason the L1 surface is redesigned rather than ported:

1. **The fd is a Zen `int`.** `20_sockets.z:31` does `sys.send(pair[0], …)` on the result of
   `sys.socketpair`, and `20_sockets.z:76` compares `sys.wexitstatus(w[1])` against a raw status
   word. This is the K7 violation that [`11-ffi-handles.md`](11-ffi-handles.md) §1 forbids, and it
   is the single largest compatibility break in this document.
2. **`socket.scan` exists** — `src/socket.rs` registers it. A port scanner is a credential-attack
   primitive with one function name between a user and an exploit, and it is a *default* member of
   the language's namespace today.
3. **The sockaddr is hand-packed in Zen.** `20_sockets.z:8` builds a `sockaddr_in` as a hex string
   (`"0200" + P2(port) + "7f000001" + …`). It works, it is a beautiful demonstration of why `bytes`
   is a language type, and it is exactly the kind of thing a typed `Addr` removes.

### 4.2 The L1/L2 surface

```zen
// ── Addressing ───────────────────────────────────────────────────────────
let a4 = ip.parse("127.0.0.1")            // → ip4, raises AddressError
let a6 = ip.parse("::1")                  // → ip6
let any = ip.v4_any(); ip.v6_any()
ip.is_loopback(a4)                        // true
ip.to_string(a4)                          // "127.0.0.1"  — RFC 5952 form for v6
let [host, port] = ip.split(a4)           // ("127.0.0.1", 80)

// ── Sockets ───────────────────────────────────────────────────────────────
let s = tcp.listen(ip.parse("127.0.0.1:0"), {backlog: 128})
s.port()                                  // the ephemeral port, resolved after bind
let c = tcp.connect(ip.parse("example.com:443"), {timeout_ms: 5000})
c.local_addr(); c.peer_addr()             // → (ip, port)
c.settimeout({read_ms: 30000, write_ms: 30000})
c.set_nodelay(true)                       // TCP_NODELAY, on by default
c.set_keepalive({idle_s: 60, interval_s: 10, count: 3})
c.shutdown(write)                         // → half-close. `write` is an enum, not an int
c.close()

let u = udp.bind("0.0.0.0:0")
u.sendto(bytes, to: ip.parse("1.2.3.4:53"))
let [n, from, data] = u.recvfrom(2048)    // → (len, addr, bytes)
u.set_broadcast(false)                    // §9.4: off unless asked for

// ── Waiting ───────────────────────────────────────────────────────────────
let ready = poll([{stream: c, events: read}, {stream: s, events: accept}], 5000)
//  → [(index, revents)] — an `events` value, not a bitmask int. §4.3.
```

**`events` is a two-variant value, not an integer.** The old API is
`sys.POLLIN` — a magic constant compared against a returned bitmask
(`20_sockets.z:24`). A bitmask is exactly the kind of thing that produces `check(pr[0].revents ==
sys.POLLIN, "poll on socketpair")` where the real bug is that the caller assumed a single event
could not co-occur with another. `events` is a heap object with `read`, `write`, `read_closed`,
`error` and `hup` flags as `ObjHeader`-style booleans, `events.has(read)`, and `poll` returns a
`list` of `{stream, events}` pairs. The port of `20_sockets.z` loses two lines and gains a
`has()` call, and the class of bug the old API invited is gone.

**`shutdown` takes a symbolic argument.** `sys.SHUT_WR` becomes `write` (or `read`, or `both`).
A raw `int` here is how you get `shutdown(fd, 2)` on a library that means `SHUT_RD` by 2, and the
whole point of a symbolic enum is that the mistake is unrepresentable.

### 4.3 The `ResKind` rows this document adds

[`11-ffi-handles.md`](11-ffi-handles.md) §4.3 already declares `Socket` and `TlsSocket`. Two rows
are needed and they are added to that table:

| `ResKind` | Stored as | `Drop` does | `Send` | `Sync` |
|---|---|---|---|---|
| `UdpSocket` | `tokio::net::UdpSocket` | `close(2)` | yes | yes |
| `TlsStream` | `tokio_rustls::client::TlsStream<TcpStream>` | TLS `close_notify`, then the socket | yes | **no** — a TLS session is a state machine and two concurrent writes corrupt the record layer |

`UdpSocket` is a *new* `ResKind` rather than a mode flag on `Socket`, because a connected UDP
socket (`connect`ed to a peer, so `recv` works and ICMP errors are delivered) and an unconnected one
have different error semantics, and a script should not be able to confuse them. It also means
`UdpSocket` is `Sync` (two threads may each `sendto`, and the kernel serialises) while `TlsStream`
is not, which is a distinction a single `Socket` variant with a mode field could not express.

### 4.4 Raw sockets: a flag, a feature, and a design argument

`scapy` registers 25 natives, 10 of which are packet construction and 15 of which are transmission
or capture — `scapy.sendp`, `scapy.sniff`, `scapy.syn_scan`, `scapy.arp_scan`. That is a packet
*forge* and a *scanner* in the default namespace. `socket.scan` is a port scanner. This document
does not ship them enabled.

**Ruling, three parts:**

1. **The crates are behind the `raw` Cargo feature**, off in every default build. A build that does
   not link `libpcap` or require `CAP_NET_RAW` is a build that works everywhere.
2. **The natives are behind `--allow-raw-sockets` at run time**, the flag
   [`11-ffi-handles.md`](11-ffi-handles.md) §7.3 already reserves. The error is specific:

   ```
   FeatureDisabled: net.raw needs CAP_NET_RAW and --allow-raw-sockets
     at probe.z:3
   ```
3. **Two natives are not ported at all, regardless of flags.** `scapy.syn_scan` and
   `scapy.arp_scan` are port scanners with a convenience API; `net.pcap` keeps the primitives
   (`build`, `parse`, `send`, `sniff`) and a scanner is 40 lines of Zen over them. The argument:
   a language that ships a one-call SYN scanner ships it to every script that can reach the
   module, and the people who want one already have `nmap` — which is in `std/nmap.z` today and
   stays (§8.5).

**What *is* ported, and why it is worth the flag.** The stated use case for this language is packet
analysis ([`00-blueprint-review.md`](00-blueprint-review.md) §3 cites it as the reason `bytes` is
first-class), and that use case needs three things: build a packet, send it, and read what comes
back. `net.raw` provides them with `pnet_packet` 0.35 for construction and parsing and
`pnet_datalink` 0.35 for capture. A user with `CAP_NET_RAW` — which on Linux is `sudo`, and on
macOS is `/dev/bpf*` ownership — gets a real tool. A user without it gets a clear error at the
first call, not a mysterious `EPERM` from `sendto`.

| Old native | New | Change |
|---|---|---|
| `scapy.ip/tcp/udp/icmp/raw/ether/arp` | `net.raw.ip/tcp/udp/icmp/ether/arp` | `pnet_packet` structs instead of hand-rolled builders. Shapes are pinned by tests, not by hope |
| `scapy.build` / `scapy.parse` | `net.raw.build` / `net.raw.parse` | Returns `bytes` both ways. The old versions returned hex `string`s |
| `scapy.checksum` | `net.raw.checksum` | Same function, on `bytes` |
| `scapy.ip_to_int` / `int_to_ip` | `ip.parse` / `ip.to_string` | The old pair is a lossless integer encoding, which is the old handle-ruling violation in a different costume |
| `scapy.cidr_expand` / `subnet_hosts` | `net.raw.cidr_hosts` | **Changed on purpose:** a `/8` returns 16 777 216 hosts, i.e. an OOM, which [`00-blueprint-review.md`](00-blueprint-review.md) §5 makes the one fatal error. A prefix wider than `/20` requires an explicit `{limit: n}`, default 65 536 |
| `scapy.send` / `sendp` | `net.raw.send` / `sendp` | `--allow-raw-sockets` |
| `scapy.sniff` | `net.raw.sniff` | `pnet_datalink`; returns a `handle` of a new `ResKind::Capture` that yields packets from a background task, not a blocking call |
| `scapy.syn_scan` / `arp_scan` | — | **DROPPED.** §4.4.3 |
| `scapy.srp1` / `handshake` / `src_mac` / `src_ip` / `hostname` | folded into `net.raw` | Utility natives with no independent meaning |
| `socket.scan` | — | **DROPPED.** A port scanner is 20 lines over `net.raw`/`tcp` and does not belong in the language's default namespace |

---

## 5. Layer 3–4: transport and TLS

### 5.1 TCP and UDP: `tokio`, and what "the socket module" now means

The `tokio` feature set in `zen/Cargo.toml` is already exactly right — `rt`, `rt-multi-thread`,
`net`, `time`, `io-util`, `sync`, `macros` — and this document adds no features. `net::TcpStream`,
`net::TcpListener`, `net::UdpSocket` and `net::lookup_host` are L4, `tokio::io::{AsyncRead,
AsyncWrite, AsyncBufRead}` are the L4↔L5 boundary, and `tokio::sync::Semaphore` is the pool in
§7.1. That is the entire L4 implementation: **zero new code beyond the natives**, because the crate
is the implementation.

**Two behaviours that need a ruling because `tokio` differs from `std`:**

| Behaviour | `std` | `tokio` | Ruling |
|---|---|---|---|
| `read` on a closed peer | `Ok(0)` | `Ok(0)` | Same. `Ok(0)` is EOF and is *not* an error; the natives return an empty `bytes` |
| DNS resolution | `ToSocketAddrs` uses `getaddrinfo`, blocks | `net::lookup_host` uses `spawn_blocking` + `getaddrinfo` by default | Use `hickory` (§3.3) rather than `tokio`'s, for the cache and because it does not consume a blocking-pool thread for a DNS timeout |
| `SO_REUSEADDR` | set by hand | set by hand | **`tcp.listen` sets it by default.** A test suite that binds `:0` 400 times does not need it, but a user who binds a fixed port after a crash does, and the cost of the wrong default is a confusing `EADDRINUSE` |

### 5.2 QUIC: shipped, off by default, and it is not a priority

`quinn` 0.11 is behind the `quic` feature and is a real implementation: `udp.bind`, then
`quic.connect`/`quic.listen`/`quic.accept` over that socket, with `0-RTT` and HTTP/3 available
through the same `Transport` trait. It is **off by default** for a reason that is about trust, not
effort: an off-by-default dependency is one that nobody runs the tests against, and a networking
stack whose QUIC path is untested is worse than one without QUIC.

The plan is therefore: implement it, gate it, and put exactly one test — a 1-RTT handshake and a
1000-byte echo over loopback — behind the feature. When the crate reaches 1.0 and two independent
projects are using it in anger, the feature becomes default and the test becomes unconditional. A
reviewer should read §12's ruling 9 as "this is a bet on a pre-1.0 crate, and here is the exit".

### 5.3 DNS: what `dns.resolve` may not do

`src/dns.rs` registers 3 names for 2 functions. `dns.query` is the interesting one: on the old tree
it can only mean "send a hand-built DNS packet and parse the reply", because `getaddrinfo` cannot
express "give me the MX records". `hickory-resolver` 0.25 can, so:

| Old | New | Notes |
|---|---|---|
| `dns.resolve(host)` | `net.resolver().resolve(host, {kind: "A"})` | Returns `(list of ip, ttl)`. TTL is new and is the point: §7.1's connection pool needs it |
| `dns.lookup(host)` | alias, kept | Two names for one function is what the old tree did; the alias keeps `06_modules_core.z` working |
| `dns.query(host, type)` | `net.resolver().query(host, type)` | Real MX/TXT/NS/CAA/SOA/SRV queries. A capability the old tree advertised and did not have |

**AAAA is opt-in per resolve, not per process.** `resolve(host, {kind: "AAAA"})` asks for v6 and
`net.connect` honours a Happy Eyeballs-style interleaved attempt, because a dual-stack
connect that tries v6 first and waits the full SYN timeout is a 2-second penalty on every
IPv4-only network. The implementation is: resolve both, race the connects with a 250 ms skew
(`tokio::time::sleep` between attempts), keep the winner, close the loser. 40 lines, and it is the
single most user-visible improvement in this document on a corporate network.

### 5.4 TLS: 1.3, rustls, compiled-in roots, and no escape hatch

**Ruling: `rustls` 0.23 with the `ring` provider, `webpki-roots` 1 compiled in, TLS 1.3 preferred
and TLS 1.2 permitted, and no way for a script to disable certificate verification.**

| Question | Ruling | The alternative and why it loses |
|---|---|---|
| Provider | `ring` | `aws-lc-rs` is faster and is the Rustls default. It needs a C compiler and NASM. `zen`'s `cargo build` must work on a bare toolchain |
| Root store | `webpki-roots`, compiled in | `rustls-native-certs` reads the OS store, which differs per platform, is unreadable in a scratch container, and is how revocation-unaware trust decisions get shipped. **This is the ruling most likely to be reversed**, and §12.4 says what reversing it costs: a `--use-system-roots` flag, a test that runs only where a store exists, and a documented "your platform decides" caveat |
| Versions | 1.3 preferred, 1.2 permitted, **1.0/1.1 and 3.0–3.3 rejected** | 1.2 is 8 years old and still ~30 % of the web. 3.0–3.3 are not in `webpki-roots` and will not be |
| Verification | mandatory, and there is **no** `danger_accept_invalid_certs` | This is a scripting language that will be pointed at a URL a user typed. A script that can turn verification off turns it off *globally*, including for the next script in the same process, because the setting lives on a shared client |
| Client certs | not in M10 | §12.4. It needs `rustls` client-cert verifier plumbing and a credential store, and nothing in the old tree implies it |
| SNI / ALPN | automatic | `tonic` and `hickory` both need ALPN, so it is not optional plumbing — it is load-bearing for §7.7 and §5.3 |
| Session resumption | on, with a 256-entry in-process cache | Free with `rustls`; saves a full handshake on the second request to a host |

**Why "no escape hatch" is the important row.** `reqwest` ships
`.danger_accept_invalid_certs(true)` and every language that wraps TLS ships the same escape
hatch, always for the same reason: someone debugging a self-signed dev server. The cost is that the
escape hatch is a *process-global* flag on a *shared* client, so the debugging session's
`accept_any = true` silently persists for every subsequent request in that process, and the failure
mode when it is wrong is not an error — it is a successful connection to whatever answered. The
mitigation that keeps the ergonomics without the persistence: a **per-connection** option,
`tcp.connect(addr, {tls: {verify: "system-ca-bundle", ca_pem: bytes}})`, so a dev server is reached
by naming *its* CA in *this* call, and nothing global changes. That is one extra `Vec<u8>` on a
config struct and it removes the entire class of bug. A test asserts that no global in
`zen-std-net` can set `ServerCertVerifier` to a permissive implementation — a grep, in CI,
alongside §9.5's.

---

## 6. Layer 5–6: session and presentation

### 6.1 Session state is a protocol's problem, not the socket's

There is no "session module". A session — an HTTP keep-alive pool, an FTP control connection plus
its data connection, an IMAP `SELECT`-ed mailbox, an SSH channel — belongs to the L7 protocol that
defined it, and it is a `ResKind` because it is a resource with a lifetime:

| Protocol | Session object | `ResKind` | Lifetime |
|---|---|---|---|
| HTTP/1.1, HTTP/2 | `http.Client` | `HttpClient` (new) | Explicit `close()`, or `Vm` drop. A client pools per-host connections internally |
| WebSocket | `ws.Conn` | `WsConn` (new) | `close()` sends a close frame; peer EOF is an error value, not EOF |
| gRPC | `grpc.Channel` | `GrpcChannel` (new) | `Vm` drop |
| MQTT | `mqtt.Client` | `MqttClient` (new) | `disconnect()` sends DISCONNECT |
| SMTP | `smtp.Session` | `SmtpSession` (new) | `quit()` |
| IMAP | `imap.Session` | `ImapSession` (new) | `logout()` |
| POP3 | `pop3.Session` | `Pop3Session` (new) | `quit()` |
| FTP | `ftp.Session` | `FtpSession` (new) | `quit()` |
| SSH | `ssh.Session` | `SshSession` (new) | `close()` |
| Telnet | `telnet.Session` | `TelnetSession` (new) | `close()` |

Ten new `ResKind` variants is a lot of enum, and the honest alternative — one `ResKind::Protocol`
with a `Box<dyn Any>` per protocol — loses the §4.4 `ResKind` check that gives
`socket.read: handle 41 is a file, not a socket` for free. The ruling is: **one variant per
protocol, because the whole point of `ResKind` is the error message**, and the cost is a `match`
arm in `arg_handle`'s caller. §10's test 2 asserts the message for a wrong-protocol handle, and that
test is the justification.

### 6.2 Presentation is `bytes` plus two codecs, and that is the whole layer

| Concern | Ruling | Crate |
|---|---|---|
| Byte ↔ typed values | Already handled: `bytes` is a language type ([`00-blueprint-review.md`](00-blueprint-review.md) §3), so a codec reads `bytes` and returns a `dict` of `Value`s | — |
| Text encoding | UTF-8 only, always. `string` is UTF-8; a protocol that says `latin-1` decodes to `bytes` and the script asks | `encoding_rs` is **not** a dependency; §12.4 |
| `Content-Encoding: gzip` | Automatic in `http.Client`, opt-out via `{decompress: false}` | `flate2` 1 |
| Chunked transfer | Handled by `hyper` | — |
| `base64` | `std/base64.z` already exists and is pure Zen; `base64` 0.22 is used for the *headers* only | — |

The UTF-8 ruling costs something real: `smtp` and `imap` both have 8-bit header fields, and
`imap.fetch` of a message with a Latin-1 body now returns `bytes` for that body. That is correct —
the old tree's `recv_text` would have replaced the invalid bytes with `U+FFFD` and lied about the
message — and it is the kind of change that goes in the migration notes rather than a bug report.

---

## 7. Layer 7: application protocols

### 7.1 HTTP: replacing 51 727 bytes of `reqwest` glue with hyper

`src/requests.rs` is 1 357 lines and 38 native registrations. It is *not* a hand-written HTTP
client: it is a hand-written **session, cookie-jar, redirect and retry layer on top of
`reqwest::blocking`** — `src/requests.rs:518` builds a `reqwest::blocking::Client`, `:542` and
`:570` implement redirect following, `:609` keys a cookie jar by host, `:702`/`:708` resolve
`Location` against a base URL. So the diagnosis is precise, and it matters for the plan:

| Layer | Old implementation | Verdict |
|---|---|---|
| HTTP/1.1 + HTTP/2 wire protocol | `reqwest` 0.13.4 (blocking, json, stream) | Replaced by `hyper` 1 — same job, async, and the only version with a 2026 EOL date |
| Redirects | Hand-rolled, `Policy::none` at `:519` then re-implemented at `:542`/`:570` | Replaced by hyper-util's `Redirect` middleware. The old code is 130 lines re-deriving a solved problem |
| Cookie jar | Hand-rolled, host-keyed, `:609` | Replaced by a 60-line `CookieJar` **in Zen**, exposed as `http.jar()` — see below |
| Retries | — | `RetryPolicy` with a jittered backoff, default off (§9.3) |
| Timeouts | `reqwest` builder | Explicit `{connect_ms, read_ms, total_ms}` with a **total** deadline, which `reqwest` has no single field for |
| Async | `reqwest::blocking` — a `std::thread` per in-flight request | Replaced by the shared runtime of [`12-concurrency.md`](12-concurrency.md) §7.3. One runtime, not one thread per request |

**The cookie jar is the one thing worth porting as Zen code.** Cookies are the part of a client that
users actually want to *read and edit* — a script that logs in, then adjusts a session cookie
before the next call, is a real pattern. So `http.jar` is a first-class object with a specified
policy (RFC 6265 `Max-Age` beats `Expires`, `Domain` matching, `Secure`, `HttpOnly` respected,
`SameSite` not sent because Zen has no browser), and the test suite asserts RFC 6265 §5 examples.

```zen
// ── The one-call form ─────────────────────────────────────────────────────
let r = http.get("https://example.com/api")
r.status                     // 200
r.ok                          // 200..=299
r.text                        // string, decompressed, UTF-8-validated
r.bytes                       // bytes
r.json()                      // a Zen value, via serde_json — a parse error is a catchable error
r.headers["content-type"]     // a case-insensitive lookup, not a dict
r.cookies.set("sid", "x", {path: "/", max_age: 3600})
r.cookies                    // the jar, mutable
http.get(url, {jar: r.cookies, timeout_ms: 5000, retries: 2})

// ── A session, which is a client ──────────────────────────────────────────
let http = http.client({jar: true, max_conns: 8, timeout_ms: 30000})
http.get(url).text
http.close()                  // closes the pooled sockets. §6.1's ResKind::HttpClient.

// ── Forms and JSON, because they are 90 % of real scripts ─────────────────
http.post(url, form: {a: "1", b: "2"})
http.post(url, json: {a: 1})
http.request("PATCH", url, body: bytes, headers: {"x-trace": id})

// ── Parallel, and the only reason to reach for it ─────────────────────────
let pages = http.parallel(urls, {max: 8}) { u in http.get(u).text }
```

`r.text` on a non-UTF-8 body is a catchable `DecodeError`, and `r.bytes` is the escape hatch. The
old `requests.resp_text` decoded with lossy replacement; the new one refuses, because a script
that hashes a download must not have its bytes silently changed.

**HTTP/2 and HTTP/3.** `hyper` 1 with `http2` enabled does h2 over TLS with ALPN, and h2c only
when explicitly requested. HTTP/3 is not wired to `quinn` in M10 — `hyper`'s HTTP/3 support is
still marked unstable and §12.4 records the decision not to bet on it. `http.get` therefore never
negotiates h3 and a script that needs it uses `quic` directly.

### 7.2 WebSocket: `tokio-tungstenite`, and what changes

| Old | New | Note |
|---|---|---|
| `websocket` 0.27 crate, used from `src/runtime.rs` | `tokio-tungstenite` 0.28 with **our** TLS | Two TLS stacks in one binary is how a "verified" connection ends up on the wrong one. §5.4's no-`native-tls` ruling covers this |
| `ws.connect(url)` | `ws.connect(url, {headers, timeout_ms})` | — |
| `ws.send(text)` | `ws.send(text)` / `ws.send_bytes(bytes)` | `string` and `bytes` are different sends, not a hex convention |
| `ws.recv()` → `string` | `ws.recv()` → `Value` (`string` or `bytes`) | The old one lost every binary frame |
| `ws.close()` | `ws.close({code: 1000, reason: "…"})` | Close codes are a wire protocol, so they get symbolic constants rather than integers |
| — | `ws.ping()` / `ws.pong()` | Absent from the old tree, which means a dead connection was only discoverable by a read timeout |

Ping/pong is worth its own line: without it a `ws.recv()` on a half-open TCP connection blocks
until the OS TCP timeout — 15 minutes on Linux with default `tcp_retries2`. A 30-second
application-level ping interval turns a 15-minute hang into a 30-second error, and it is 20 lines
in a `tokio::spawn`.

### 7.3 DNS, TLS, HTTP: the composition is a test, not a hope

The three layers meet in exactly one place that matters — `http.get("https://…")` — so that is
where the tests go, and §10.4 has six of them against a loopback TLS server with a generated CA.
They cover: a 200 with a body, a 301 chain of 3, a 302 with a relative `Location`, a 4xx with an
error body, chunked transfer, gzip, a `Set-Cookie` round-trip, and a certificate signed by a CA
that is *not* in the jar (which must fail). That last one is the test that proves §5.4's ruling is
implemented rather than documented.

### 7.4 SMTP, IMAP, POP3, FTP: four crates, one shape

| Module | Crate | The shape | Notes on the old tree |
|---|---|---|---|
| `smtp` | `lettre` 0.11 | `let s = smtp.connect(host, {port: 587, starttls: true}); s.login(u, p); s.send(from, to, message); s.quit()` | 5 natives today (`connect`, `login`, `sendmail`, `quit`, `message`). `smtp.message` is **kept** — building a MIME message in pure Zen is the useful part and `lettre` does it worse |
| `imap` | `imap-next` 0.9 | `let s = imap.connect(host, {tls: true}); s.login(u, p); s.select("INBOX"); s.search("UNSEEN")`; `fetch` returns `bytes` per message | 6 natives today. The old `imap_fetch` returns a `string`; the new one returns `bytes` and `fetch_text` is the lossy version that says so |
| `pop3` | **hand-rolled on L4, `httparse` for nothing** — POP3 is 5 commands and `+OK`/`-ERR` line parsing | `let s = pop3.connect(host); s.login(u, p); s.stat(); s.retr(1)` | 6 natives today. There is no maintained async POP3 crate; the honest answer is ~350 lines of textproto, and §7.9 shares the helper with Telnet |
| `ftp` | `suppaftp` 0.8 | `let s = ftp.connect(host); s.login(u, p); s.list("/")` | 13 natives today — the most of any protocol module, and the most mechanical. PASV/EPSV, RETR, STOR, and the 150/226 dance are all `suppaftp`'s problem now |

**`tests/mock_ftp.py` is the compatibility target and it stays.** The 43-line test
`09_socket_hydra.z` depends on it (127.0.0.1:2121, `admin`/`s3cret99`, expecting a `220` banner and
answering `USER`/`PASS`/`PWD`/`SYST`/`TYPE`/`CWD`/`PASV`/`LIST`/`NLST`/`QUIT`). `suppaftp` will
issue commands that file does not implement and it will answer `502 Not implemented`, which
`suppaftp` handles. The port keeps the same server, the same credentials, and the same
`nc -z 127.0.0.1 2121` readiness probe in `tests/run_tests.sh:20` — so §10's FTP test is the old
FTP test with a new client.

### 7.5 SSH: `russh`, and the module it replaces is a shell-out

`src/ssh.rs` registers 4 natives — `ssh_run`, `ssh_upload`, `ssh_download`, `ssh_available` — and
the "available" one is a strong hint: the old implementation shells out to the `ssh` binary and
reports whether it was found. A language module that requires another program is a module with a
dependency the user cannot audit and a protocol it does not actually implement.

**Ruling: `russh` 0.5, a real SSH client.** `ssh.connect(host, {user, key: pem, password})` returns
a session handle; `exec(cmd)` returns a `(exit_status, stdout, stderr)` triple; `upload`/`download`
use SFTP. The cost is honest: `russh` is pre-1.0, it is a much larger dependency than the four
shelled-out lines it replaces, and it pulls `russh-keys`/`russh-cryptovec`. §12's ruling 8 is the
one to argue about, and it offers a fallback: keep a `net.ssh_shell` native that shells out,
flag-gated, for the user who wants the system `ssh` and its `~/.ssh/config`. Both are honest; only
one can be the default.

### 7.6 Browser automation: out of the language, available as a crate

`src/browser.rs` registers 25 natives (18 unique) and `src/cdp.rs` is 210 lines of Chrome
DevTools Protocol client that **spawns a browser with `std::process::Command`** (`src/cdp.rs:3`).
Ruling: `crates/zen-cdp`, an *optional crate outside the workspace's default members*, with a plain
Rust API. The reasoning is short and does not involve taste: a language that can launch Chrome
gives every script the ability to launch Chrome, the CDP surface changes every Chrome release, and
none of it is a networking layer. A user who wants it adds a dependency to their project, which is
where an unstable third-party protocol belongs.

### 7.7 gRPC: not in the old tree, and here is why that is a good thing

The brief said Zen 2.x has gRPC via tonic. It does not: `grep -c '"grpc\|"tonic' src/*.rs` finds
nothing, and `tonic` is not in `Cargo.toml`. So this is greenfield, and greenfield is where the
cost is lowest and the temptation to over-build is highest.

**Ruling: `tonic` 0.14 + `prost` 0.14, behind the `grpc` feature, and Zen does not have a
`.proto` compiler.**

```zen
// A gRPC call, with the service path and method name as strings and the
// body as bytes. No generated code, no build step, no protoc.
let ch = grpc.connect("https://api.example.com", {tls: true})
let res = ch.unary("/pkg.Service/Method",
                   bytes,                 // a prost-encoded message
                   {deadline_ms: 5000, metadata: {"authorization": "Bearer …"}})
res.status                     // 0 == OK, the canonical gRPC code
res.message                    // bytes
res.metadata["x-request-id"]

// Streaming is a channel, which makes it a first-class Zen citizen rather
// than a future.
let st = ch.server_stream("/pkg.Service/List", bytes)
for msg in st { process(msg) }
```

A `.proto` compiler is the thing to be tempted by and it is the wrong call: it needs `protoc` at
build time, it generates code into a repository that is mostly a language implementation, and it
makes a gRPC dependency a *build* dependency. Strings and `bytes` cost one `format!` at the call
site and keep `protoc` out of the toolchain. A user who wants generated stubs generates them in
*their* project and calls `ch.unary` with the result.

**Deadline and metadata are the two features that justify the dependency.** `deadline_ms` maps to
gRPC's propagation and is the difference between a client that fails fast and one that hangs; and
gRPC metadata is how auth and tracing headers travel, so `metadata` is a first-class parameter
rather than an escape hatch.

### 7.8 MQTT: not in the old tree either

Also absent. `rumqttc` 0.24 behind the `mqtt` feature:

```zen
let c = mqtt.connect("mqtts://broker.example.com:8883", {
    client_id: "zen-" + uuid4(), username: u, password: p,
    clean_session: true, keepalive_s: 60,
})
c.subscribe("sensors/+/temp", {qos: 1})
c.publish("sensors/a/temp", payload: bytes, {qos: 1, retain: false})
c.on_message(fn(t, p) { print(t, p) })     // a callback, dispatched on the runtime
c.unsubscribe("sensors/+/temp")
c.disconnect({reason: "bye"})              // sends DISCONNECT; a Drop does not
```

Three things make this more than a wrapper. **`on_message` is a callback on the shared runtime**,
so a message handler runs a Zen function on a runtime thread — which means the handler must not
block, and §3.2's `BlockingClass` rule is the enforcement: a `MayBlock` native in a message
handler is a compile error, exactly as in a `pmap` closure. **`qos` is an enum, not an int** (0, 1,
2), for the same reason `shutdown` is. And **a QoS-1 publish must be acknowledged before
`publish` returns**, or the API lies about delivery; `rumqttc` exposes the ack, and the native
blocks on it with the caller's deadline, which is why `publish` is `MayBlock` and not
`NeverBlocks`.

### 7.9 POP3 and Telnet: the honest hand-rolled case

Two protocols have no maintained async crate worth depending on, and both are line protocols over
L4. Rather than write two parsers, this document writes one:

```rust
// crates/zen-std-net/src/l5/textproto.rs

/// The shared implementation behind `pop3` and `telnet`.
///
/// Both are RFC 821-family line protocols: a line of text in, a line of
/// text out, with a small state machine and a `+OK`/`-ERR`-style reply
/// code. ~350 lines each in the old tree, ~350 lines total here, and the
/// part that is easy to get wrong — reading a line **without** overrunning
/// the caller's buffer, and never treating a partial read as a complete
/// line — is written once and tested once.
pub struct TextProto<S> {
    inner: S,
    buf: BytesMut,          // retained across calls: this is the perf decision
    max_line: usize,        // 512 for POP3 (RFC 1939), 1024 for Telnet (RFC 854)
}
```

**`max_line` is the security-relevant field.** A line protocol that will read a "line" until
`\n` with no limit is a memory-exhaustion bug reachable by a hostile server — a 4 GiB line with no
newline. Both RFCs specify the limit (POP3: 512 octets including CRLF; Telnet: the IAC buffer rule),
and both defaults here are the RFC value. Exceeding it is a catchable `LineTooLong`, not an
allocation. §10's test 12 is exactly this: a mock server that sends 1 MiB without a newline, and
the assertion is that the session raises within 1 KiB of receiving it.

**Telnet's IAC negotiation is the other half.** Telnet has an in-band option-negotiation state
machine (IAC DO/WILL/WONT/DO, ~20 option codes) that a naive client gets wrong by echoing the
negotiation bytes back as data. The old `telnet.rs` has 5 natives and no state machine, so its
`telnet_read` returns negotiation bytes to the script. The new one implements the state machine,
negotiates SGA and ECHO off, and exposes the *negotiated* option set as `s.options` so a script can
see what the server agreed to. This is a genuine feature, and it is also the reason `telnet` is
M10.7 and not M10.13: it is the smallest protocol in this document that needs a real state machine.

---
## 8. Module inventory and dispositions

### 8.1 The premise, corrected with receipts

The brief for this document said: *"Zen 2.x has working networking modules across all seven OSI
layers: gRPC (tonic), mqtt, smtp, imap, ftp …"* The first half of that is not true and the plan is
better for saying so.

| Claim | Verdict | Evidence |
|---|---|---|
| SMTP, IMAP, FTP, POP3, Telnet, SSH, DNS, HTTP exist | **true** | 5, 6, 13, 6, 5, 4, 2, 6 unique natives respectively; all present in `src/runtime.rs` |
| `requests` exists | **true** | `src/requests.rs`, 51 727 bytes, 34 unique natives over `reqwest::blocking` |
| gRPC via `tonic` exists | **FALSE** | No `tonic` in `Cargo.toml`; no `grpc` module in `src/`; no `grpc_*` native in `src/runtime.rs`. §7.7 is greenfield |
| `mqtt` exists | **FALSE** | No `mqtt` module, no `mqtt_*` native, no `rumqttc` dependency. §7.8 is greenfield |
| All seven OSI layers are covered | **false** | L1/L2 exist only as `sys.*` raw syscalls; L3 has no module; L6 exists only inside `requests`' TLS config; L7 has 8 protocols, not "all" |
| A `hydra` brute-forcer exists | **FALSE, and worse** | It is documented in the help text at `src/runtime.rs:19255` with a full API listing — `hydra.ssh`, `hydra.run`, `hydra.run_file` — and `grep -c hydra_ src/runtime.rs` returns **0**. `tests/cases/09_socket_hydra.z` imports it and calls it five times. That test cannot pass, and has never passed |

**The hydra finding is the one that matters for the plan.** A module that is documented with a
complete API and has no implementation is worse than an undocumented gap: the test suite references
it, so a newcomer debugging a red test finds a module that does not exist, and the help text tells
them it does. The disposition is **DROP**, and §10.4 ports the *other* half of
`09_socket_hydra.z` (the `socket` round-trip against `mock_ftp.py`) while dropping the hydra half
with a note in the migration guide, because `nmap`'s scriptable auth mode and a 20-line Zen loop
cover it and shipping a credential-attack primitive in the default namespace does not.

### 8.2 The module-level table — 25 modules, every one with a disposition

| # | Old module | Bytes | Natives | Disposition | New home | One-line reason |
|---|---|---|---|---|---|---|
| 1 | `socket` | 1 155 | 13 | **REPLACE** | `tcp`, `udp`, `poll` | 13 name strings and no code. A handle-based `tcp`/`udp` is a different shape, and `socket.scan` is dropped (§4.4) |
| 2 | `http` | 601 | 6 | **REPLACE** | `http` | 6 name strings. The verbs survive; the implementation moves to hyper, and `http` gains request/response objects |
| 3 | `requests` | 51 727 | 34 | **REPLACE** | `http` | Hand-rolled session/cookie/redirect layer over `reqwest::blocking` ([§7.1](#71-http-replacing-51-727-bytes-of-reqwest-glue-with-hyper)). Largest file in the network stack; smallest after the rewrite |
| 4 | `urllib` | 661 | 6 | **SPLIT** | `url` + keep in `urllib` | `quote`/`unquote`/`urlencode` are pure string functions and belong in `std/url.z`. `urlopen` is an HTTP client and becomes `http.get` |
| 5 | `dns` | 418 | 2 | **REPLACE** | `net` | 3 names, 2 functions. Becomes `net.resolver()` plus real `query` ([§5.3](#53-dns-what-dnsresolve-may-not-do)) |
| 6 | `telnet` | 583 | 5 | **REPLACE** | `telnet` | Needs the IAC state machine the old one lacks ([§7.9](#79-pop3-and-telnet-the-honest-hand-rolled-case)) |
| 7 | `ftp` | 1 049 | 13 | **REPLACE** | `ftp` | Most mechanical module in the tree; `suppaftp` does PASV/EPSV/RETR/STOR and the 150/226 dance |
| 8 | `imap` | 625 | 6 | **REPLACE** | `imap` | `imap-next`. `fetch` returns `bytes`, not `string` |
| 9 | `pop3` | 611 | 6 | **REPLACE** | `pop3` | Hand-rolled on the shared `TextProto` ([§7.9](#79-pop3-and-telnet-the-honest-hand-rolled-case)) |
| 10 | `smtp` | 563 | 5 | **REPLACE** | `smtp` | `lettre`. `smtp.message` is **kept verbatim** — pure-Zen MIME building is the useful half |
| 11 | `ssh` | 488 | 4 | **REPLACE** | `ssh` | Shells out to the `ssh` binary today; `russh` is a real client ([§7.5](#75-ssh-russh-and-the-module-it-replaces-is-a-shell-out)) |
| 12 | `wifi` | 707 | 7 | **DROP** | — | Wraps `nmcli`. A module that shells out to a platform tool adds a dependency the language cannot audit |
| 13 | `bluetooth` | 974 | 11 | **DROP** | — | Wraps `bluetoothctl`, same argument. 11 natives for a subprocess is a bad trade |
| 14 | `browser` | 2 060 | 17 | **EXTRACT** | `crates/zen-cdp` (optional) | Launches Chrome. Not a networking layer ([§7.6](#76-browser-automation-out-of-the-language-available-as-a-crate)) |
| 15 | `scapy` | 1 949 | 25 | **REPLACE, reduced** | `net.raw` | `pnet`. Two scanners dropped, `cidr_hosts` bounded ([§4.4](#44-raw-sockets-a-flag-a-feature-and-a-design-argument)) |
| 16 | `cdp` | 8 032 | 0 | **EXTRACT** | `crates/zen-cdp` | 210 lines of DevTools Protocol over `std::process::Command` |
| 17 | `hydra` | 0 | 0 | **DROP** | — | Documented at `src/runtime.rs:19255`, implemented nowhere ([§8.1](#81-the-premise-corrected-with-receipts)) |
| 18 | `pm` | 28 106 | 0 | **KEEP, extend** | `pm` | The module fetcher. Its `sha256` manifest and `zen.json` are how a network stack is *distributed*; §9.6 adds signature verification to it |
| 19 | `wa` | 63 378 | 11 | **DROP** | — | WhatsApp. §8.3 |
| 20 | `wa2` | 78 616 | 0 | **DROP** | — | WhatsApp |
| 21 | `wa_bot` | 61 173 | 0 | **DROP** | — | WhatsApp |
| 22 | `wa_signal` | 20 853 | 0 | **DROP** | — | WhatsApp |
| 23 | `wa_signal_session` | 53 002 | 0 | **DROP** | — | WhatsApp |
| 24 | `wa_media` | 11 615 | 0 | **DROP** | — | WhatsApp |
| 25 | `wa2_tokens` | 19 050 | 0 | **DROP** | — | WhatsApp |

**Totals: 15 networking modules, 182 keys, 160 unique natives, 72 203 bytes** — 4.6 % of
`src/*.rs`. After the rewrite: 11 modules in the language, 1 optional crate, 4 dropped, and the
`wa` block is 307 687 bytes of the plan's freed attention.

### 8.3 The `wa*` decision, in the terms a reviewer will use

| Question | Answer |
|---|---|
| Size | 7 files, 307 687 bytes, **19.5 %** of `src/*.rs`, **48.8 %** of `src/*.rs` excluding `runtime.rs` |
| Tests | `tests/cases/14_wa_native.z` is 43 lines and tests 11 native *names* — it cannot test a protocol |
| Protocol | An undocumented, reverse-engineered, third-party protocol with no compatibility guarantee and no version negotiation |
| Stated purpose | Chat with WhatsApp |
| Dependence on the rest of the tree | `wa` natives in `src/runtime.rs`; `wa_bridge.mjs` needs a Node subprocess; `wa_signal` is a second, parallel implementation of the same protocol |
| **Disposition** | **DROP from `zen`.** Keep the repository history. Do not delete the branch until M10 lands |

**The argument, stated once and without rhetoric.** 307 687 bytes is more than the entire
networking stack this document has to write, four times over, and it buys a protocol that can
change without notice. A user who needs WhatsApp integration can depend on a `zen-wa` crate the way
they would depend on any package; a user who needs it *in the language* will say so on an issue and
the answer can be yes then, on evidence, rather than yes now, on enthusiasm. The counter-argument a
reviewer should put to this document is: *the effort is already spent, and porting 4 of 7 files is
cheaper than the migration for 11 users of `wa.send_text`.* The answer to that is §8.3.1.

### 8.3.1 What "drop" costs a user, concretely

| User code | Migration |
|---|---|
| `wa.connect()` | No replacement. `import wa` raises `ModuleNotFound: wa was removed in 3.0; see docs/13-networking.md §8.3` — a **named** error, not "no such module" |
| `wa.send_text(phone, text)` | A `zen-wa` crate on the package index, or 200 lines of Zen over `http` and `crypto` if WhatsApp's Cloud API is what they should have been using |
| `wa.qr()` | Nothing. QR rendering is in `src/qr.rs` and `tests/cases/16_qr.z`, and both stay |
| The 11 native names | Stubs that raise the named error, for one minor release, then removed. A hard removal in a major version is correct, but a stub that *says what happened* costs 11 lines and saves every confused user an hour |

### 8.4 Native-level dispositions, so nothing is unaccounted for

Module-level rows hide the interesting cases. This table is the 160 natives, grouped.

| Old native | Count | Disposition | Note |
|---|---|---|---|
| `socket_open`, `socket_open_udp`, `socket_listen`, `socket_accept`, `socket_close`, `socket_set_timeout` | 6 | **REPLACE** | `tcp.listen`, `tcp.connect`, `udp.bind`, `tcp.accept`, `close`, `settimeout`. Same operations, handle-typed |
| `socket_send`, `socket_send_to`, `socket_recv`, `socket_recv_from`, `socket_recv_text`, `socket_recv_all` | 6 | **REPLACE** | `send`/`sendto` take `bytes` or `string` (both are legal, no hex convention). `recv_text` becomes `recv` + `str()` and is **dropped** as a native: a text mode that is not UTF-8 is a lie, and §6.2's ruling is that it returns `bytes` |
| `socket_scan` | 1 | **DROP** | §4.4.3 |
| `http_get/post/put/del/head/patch` | 6 | **REPLACE** | `http.get` etc. plus `http.request` for anything else. `del` becomes `http.delete` — `del` is a keyword |
| `__requests_sess_*` (11), `__requests_resp_*` (7) | 18 | **REPLACE** | `http.client()`, `r.status`, `r.headers`, `r.cookies` — the *shape* survives, the implementation does not. `iter_content`/`iter_lines` become `r.body_chunks` (a channel, §10's blocking rule) rather than a blocking iterator |
| `requests_get/post/put/delete/patch/head/options/request` (8) | 8 | **REPLACE** | `http.get` etc. and `http.request` for the rest. `del` → `delete` because `del` is a keyword |
| `requests_session`, `requests_codes`, `requests_utils_quote/unquote/encode`, `requests_utils_ua`, `requests_exc`, `requests_exc_http` | 8 | **SPLIT / DROP** | `quote`/`unquote` → `std/url.z`. `encode` → `bytes`-aware, and it is the one utility that improves. `codes` → a `http` constant table. `ua` → **dropped**: it was one hardcoded string pretending to be a user agent. `requests_exc*` → the `error` value type, so 2 natives become zero |
| `urllib_urlopen` | 1 | **REPLACE** | `http.get` |
| `urllib_quote/unquote/urlencode/parse/parse_qs` | 5 | **KEEP** | Move to `std/url.z` as pure Zen. These are correct today |
| `dns_resolve` (2 names) | 1 | **REPLACE** | `net.resolver().resolve`. One native, two old names — the only place in the tree where two keys already point at one function |
| `dns_query` | 1 | **EXTEND** | `net.resolver().query` gains real MX/TXT/NS/CAA/SRV, which the old one could not do |
| `telnet_*` (5) | 5 | **REPLACE** | Same names, session-typed, with the IAC state machine |
| `ftp_*` (13) | 13 | **REPLACE** | Same 13 names. `suppaftp`. A rare case where 1:1 is correct, which is why the module is boring |
| `imap_*` (6) | 6 | **REPLACE** | Same 6 names; `fetch` returns `bytes` |
| `pop3_*` (6) | 6 | **REPLACE** | Same 6 names, hand-rolled |
| `smtp_*` (5) | 5 | **REPLACE** | 4 become `lettre`; **`smtp_message` is kept byte-for-byte** |
| `ssh_*` (4) | 4 | **REPLACE** | `russh`; `ssh_available` is **dropped** — with a real client there is nothing to be unavailable |
| `wifi_*` (7) | 7 | **DROP** | §8.2.12 |
| `bt_*` (11) | 11 | **DROP** | §8.2.13 |
| `browser_*` (17) | 17 | **EXTRACT** | `crates/zen-cdp` |
| `scapy_*` (25) | 25 | **REPLACE (18) / DROP (7)** | §4.4. The 7 dropped: `syn_scan`, `arp_scan`, `srp1`, `handshake`, `src_mac`, `src_ip`, `hostname` |

### 8.5 The `std/*.z` modules, all 23, one row each

The earlier draft of this section grouped the twenty-two non-network modules into one row and
described `nmap.z` as "a pure-Zen wrapper that does not import the network stack". **That was
wrong**, and it was wrong in the way that matters: `nmap.z` is the single largest consumer of the
raw-socket layer in the entire tree, and M10 deletes that layer. Line counts are from the current
tree (`wc -l std/*.z`, 5 215 total).

| # | Module | Lines | Imports the network stack? | Disposition |
|---|---|---|---|---|
| 1 | `nmap.z` | 648 | **yes** — `import socket` (line 36), `import scapy` (line 35) | **REWRITE.** See below. This is the only `std` file M10 touches, and it is a 648-line consumer of eight `socket.*` natives and six `scapy.*` natives |
| 2 | `argparser.z` | 904 | no | Unchanged |
| 3 | `pathlib.z` | 500 | no | Unchanged |
| 4 | `math.z` | 388 | no | Unchanged |
| 5 | `statistics.z` | 363 | no | Unchanged |
| 6 | `json.z` | 295 | no | Unchanged. `serde_json` is already a workspace dependency; this stays pure Zen and M10 does not touch it |
| 7 | `string.z` | 223 | no | Unchanged |
| 8 | `glob.z` | 212 | no | Unchanged |
| 9 | `uuid.z` | 186 | no | Unchanged |
| 10 | `itertools.z` | 171 | no | Unchanged |
| 11 | `ffmpeg.z` | 169 | no | Unchanged. A subprocess wrapper, and it stays one |
| 12 | `logging.z` | 166 | no | Unchanged |
| 13 | `random.z` | 148 | no | Unchanged |
| 14 | `hashlib.z` | 147 | no | Unchanged |
| 15 | `operator.z` | 123 | no | Unchanged |
| 16 | `functools.z` | 102 | no | Unchanged |
| 17 | `base64.z` | 96 | no | Unchanged |
| 18 | `sys.z` | 84 | no | Unchanged. Distinct from the native `sys` module of `11-ffi-handles.md` §8 — this is a Zen-level facade, and the name collision is a §12 ruling |
| 19 | `shutil.z` | 73 | no | Unchanged |
| 20 | `bytes.z` | 63 | no | Unchanged |
| 21 | `tempfile.z` | 54 | no | Unchanged |
| 22 | `binascii.z` | 54 | no | Unchanged |
| 23 | `collections.z` | 46 | no | Unchanged |

#### 8.5.1 `nmap.z`, the only `std` file in M10's blast radius

`nmap.z` calls fourteen natives across two modules:

| Native | Call sites | What M10 offers instead |
|---|---|---|
| `socket.recv` | 3 | `sock.recv(n)` on a `tcp` handle |
| `socket.close` | 3 | handle finalization; the explicit call becomes unnecessary |
| `socket.set_timeout` | 2 | `sock.set_timeout(2500)` |
| `socket.send` | 2 | `sock.send(bytes)` |
| `socket.open` | 1 | `tcp.connect(host, port)` |
| `socket.scan` | 1 | **`net.raw.tcp_syn_scan`**, and it now needs `--allow-raw-sockets` |
| `socket.recv_from` | 1 | `udp.recvfrom()` |
| `socket.open_udp` | 1 | `udp.bind()` / `udp.connect()` |
| `scapy.arp_scan` | 1 | **`net.raw.arp_scan`**, `--allow-raw-sockets` |
| `scapy.syn_scan` | 1 | **`net.raw.tcp_syn_scan`**, `--allow-raw-sockets` |
| `scapy.handshake` | 2 | **`net.raw.tcp_handshake`**, `--allow-raw-sockets` |
| `scapy.sr` | 1 | **`net.raw.sr`**, `--allow-raw-sockets` |
| `scapy.src_ip` | 2 | `net.raw.src_ip(addr, ifaddr)` |
| `scapy.cidr_expand` | 1 | **`net.raw.cidr_hosts`, with a mandatory `limit`** — §4.4's row 5 |

Five of the fourteen need `--allow-raw-sockets`, which means **`nmap.z` stops working by default at
M10** even after the mechanical rename. That is the correct outcome (§4.4.3) and it is a
user-visible break in a file that predates the flag, so it goes in the changelog, not in a
footnote.

**`nmap.z` has no test.** A grep of `tests/` for `nmap` returns one hit,
`tests/cases/19_sys_ffi.z:84`, and that is the substring in `sys.munmap` — a false positive. So the
rewrite is unguarded by CI, and the first thing that will catch a mistake in it is a user with
root. §10.2 adds `net_nmap_stdlib_smoke`, which parses `std/nmap.z` and resolves every name it
imports without opening a socket.

The mechanical rename is about 40 call sites and is a one-afternoon change. It does not get its own
milestone; it is a row in M10's exit criteria in §11.

## 9. Security posture

### 9.1 SSRF is a design decision, not a checklist

A scripting language with a network stack is a server-side request forgery engine, and the
mitigations that exist in web frameworks do not transfer to a general-purpose language. There is no
trust boundary to enforce against: a script that can `http.get` can be told to `http.get` by
whatever data it is processing.

**Ruling: no global SSRF filter. Instead, three opt-in mechanisms, and one hard default.**

| Mechanism | Default | What it does |
|---|---|---|
| **`net.guard`** | **on** | A per-`Vm` policy object. `net.guard.deny_private = true` rejects any connect to a loopback, link-local, RFC 1918, CGNAT, or unique-local address — including a *hostname* that resolves to one, and including a redirect to one. This is the default because the failure it prevents (a script reading `169.254.169.254` and printing the response) has no legitimate use in the overwhelming majority of scripts |
| **`net.guard.audit`** | **on** | Every connect is appended to `net.guard.log`, a bounded ring buffer of `{time, host, addr, allowed, reason}`. 1024 entries, `net.guard.log()` returns a list. A user who turns `deny_private` off still has the record |
| **`{redirects: "none"}`** | `"safe"` | Redirect following stops when a redirect target is denied by the guard. hyper-util's `Redirect::custom` gets the policy closure, so the *middleware* enforces it rather than a wrapper around the client that a script can bypass by calling `http.client()` directly |

**Why "on by default" and not "off by default".** A library that ships an SSRF filter off is
shipping a vulnerability with a configuration knob. A library that ships it on breaks exactly one
kind of script — the one that talks to a service on `127.0.0.1` — and does so with an error that
names the flag. The opt-out is one line: `net.guard.allow_private = true`. §10's test 6 asserts
the default, and test 7 asserts the opt-out, because a security default with no test for its
escape hatch is a security default that gets turned off "temporarily" in a test and then in
production.

**DNS rebinding is the gap, and it is stated rather than claimed solved.** `net.guard` checks the
address it is about to connect to, which is correct for the connection being made. A DNS record
that returns `127.0.0.1` and is re-resolved to a public IP between the check and a later request
sneaks through on the *second* request, because there is no address to check. The mitigation that
works is per-connection address pinning: resolve once, connect to the resolved address, and set the
TLS `ServerName` to the original hostname — which is what `rustls` does anyway, and is why
`net.guard` refusing a private *hostname* matters less than refusing a private *address*. The
remaining exposure is a script that re-resolves on every request, and the honest statement is that
this document does not fully solve rebinding and does not claim to.

### 9.2 Timeouts are not optional, and there is no "wait forever"

| Layer | Default | Rationale |
|---|---|---|
| DNS | 5 s per nameserver, 2 attempts | A resolver that hangs is a script that hangs |
| TCP connect | 10 s | Linux SYN retries are ~127 s. This is a 12× improvement, not a preference |
| TCP connect (dual-stack) | 250 ms Happy Eyeballs skew, 10 s total | §5.3 |
| TLS handshake | 10 s | Inherits the connect deadline; never additive |
| HTTP total | 30 s | One deadline for the whole request, including the body |
| HTTP read idle | 30 s | Per-read, so a slow streaming response is not killed by the total |
| WebSocket idle | 300 s, with a 30 s ping | §7.2. A 30 s ping makes the idle timeout mean something |
| MQTT keepalive | 60 s | The protocol's own requirement; the client must ping at half |
| gRPC | 5 s default `deadline_ms` | gRPC deadlines are propagated, not local |
| `poll` | the caller's timeout, **required, no default** | There is **one** poll and it is spelled `poll(socks, timeout_ms)` (§4.3). Omitting the timeout is a `TypeError`, not an infinite wait, because an uninterruptible hang in a language whose only concurrency primitive is an OS thread (§12.2 of [`12-concurrency.md`](12-concurrency.md)) is a hang nobody can Ctrl-C. The table in §9.2 was the only place a second name, `poll_wait`, appeared; it is deleted, because a blocking form and a non-blocking form of the same call is one API with a bug in it |

**A deadline is one absolute `Instant`, not four nested ones.** A request with 4 × 30 s of nested
timeouts can live 120 s, which is the bug nested timeouts always have. The implementation takes an
`Instant` at the entry point and each layer checks it, and §10's test 8 asserts that a server which
accepts a connection and then sends nothing fails at ~10 s, not ~30 s and certainly not 120 s.

### 9.3 Redirects, methods and retries: the three amplification vectors

| Vector | Default | Why |
|---|---|---|
| Redirects | max 5, and the guard applies at every hop | An unbounded redirect chain is a request amplifier. 5 is hyper's default and it is right |
| Retries | **off** | A retry on a POST that timed out *after* the server processed it sends the request twice. A script that wants retries must opt in, and the opt-in only retries idempotent methods: `GET`, `HEAD`, `PUT`, `DELETE`, and `OPTIONS` |
| `Expect: 100-continue` | off | It is a DoS amplifier against a server and buys nothing on loopback |
| Chunked request bodies | allowed, capped at 64 MiB | Unbounded request bodies are the other half of the same problem |
| Response body cap | none by default, `max_bytes` available | A legitimate `http.get` of a 4 GiB file must work; an accidental one is the script's bug, and `r.bytes.len()` is right there |

Retries-off is the ruling most likely to annoy someone, and the reason it is not a default is
written above: the failure it prevents is a **double-charged credit card**, not a slow request. A
script that is GETting a JSON API and wants the retry writes `{retries: 2}` and gets exactly the
idempotent subset.

### 9.4 Socket-level defaults, and the two that are not "off"

| Setting | Default | Reason |
|---|---|---|
| `TCP_NODELAY` | **on** | A request/response protocol on loopback with Nagle adds 40 ms per small write. The old tree left it off |
| `SO_KEEPALIVE` | off | An idle keepalive is a battery cost. `set_keepalive` is one call |
| `SO_REUSEADDR` | **on for listeners** | §5.1. Binding a fixed port after a crash is a common script |
| `SO_BROADCAST` | **off** | A UDP socket that can broadcast is a UDP amplifier in a user's script. `u.set_broadcast(true)` is explicit |
| `IPV6_V6ONLY` | off (dual-stack where available) | A v6-only default means `tcp.listen(":0")` is unreachable over v4, which is a confusing bug. Off, and documented |
| `SO_RCVBUF`/`SO_SNDBUF` | OS default | A native that silently raises a buffer to make a benchmark fast is a native that makes production worse |
| `IP_TTL` / `IP_MTU_DISCOVER` | untouched | Path MTU discovery is the OS's job and `pnet` exposes it for the raw path only |

### 9.5 What is grepped in CI, because a posture nobody checks is a comment

| Check | Command | Gate |
|---|---|---|
| No permissive TLS verifier | `grep -rn "danger\|accept_invalid\|NoVerifier\|verify_server_cert.*false" crates/zen-std-net/` | Empty. The only hit allowed is the §5.4 test that asserts the symbol is *absent* |
| `native-tls` is gone | `grep -rn "native-tls\|native_tls" zen/Cargo.toml crates/` | Empty. §5.4 |
| No `unsafe` outside the allow-list | [`11-ffi-handles.md`](11-ffi-handles.md) §9.1's `build.rs` grep | Same gate, same count window |
| No unbounded line reads | `grep -rn "read_line\|read_until" crates/zen-std-net/src/l5/` | Every hit must be on a `BytesMut` with a `max_line` bound; reviewed, not automated |
| Every network native declares `BlockingClass` | the `NativeEntry` table grep from [`11-ffi-handles.md`](11-ffi-handles.md) §9.4 | Fails the build if a `net`/`http`/`tcp` native has `Unspecified` |
| No outbound connection in any test but loopback | `grep -rn "example.com\|https://\|8.8.8.8\|1.1.1.1" tests/` | Only inside `net.guard` tests and the mock-server fixtures, each marked |

### 9.6 The one supply-chain rule: `pm` verifies

`src/pm.rs` is the module fetcher: 741 lines, a `zen.json` manifest, `sha256` per module, and a
`ZEN_MODULES` directory override. It is not a networking module, but it is *the* code that puts
bytes fetched over HTTP into the interpreter, so it is in this document's scope by one sentence:
**the next module a `pm` install runs is a module the network chose.**

**Ruling: `pm install` requires a signature, and `--insecure` is not a flag.**
`zen release` signs a manifest with an ed25519 key; `pm install` verifies the manifest signature
against a keyring in `~/.zen/trusted-publishers.toml` before it fetches, and verifies each module's
`sha256` against the *signed* manifest rather than one the server can re-serve. A self-signed local
package is installed with `pm install --trust-local`, which writes the key to the trust file and
says so. The workspace already depends on `ed25519-dalek` 2, so the cost is a `verify` function and
a keyring file, and the alternative — a `sha256` that the server re-serves with the payload — is a
checksum, not a signature, and every one of the last twenty years of supply-chain incidents was a
checksum that was correct.

---

## 10. Test plan

### 10.1 The rules, which are stricter than the other documents' rules

| Rule | Detail |
|---|---|
| **Loopback only, and every test says so out loud** | Every test binds or dials `127.0.0.1` / `::1`. No test contacts a name server, a registry, or a real host. Because §9.1's `deny_private` is **on by default**, loopback is *also* denied by default, so every test file starts with the same two lines — `net.guard.allow_private = true` and a `loopback_only()` helper that asserts every address in `net.guard.log()` is loopback at teardown. Without that helper the suite is self-contradictory: 23 of 24 rows dial loopback and §9.1's default would fail all of them. The opt-out is explicit, in one place, in every file, and it is a *test* setting — nothing in `src/` may set it |
| **Test 6 is the only file without the opt-out** | `net_guard_denies_private.z` asserts the default *is* denial, so it must not disable it. If someone adds the opt-out line to that file, the test passes vacuously and the whole security default becomes untested. The suite has a lint for this: `net_guard_optout` may appear in exactly one helper, and `net_guard_denies_private.z` may not import it |
| **No fixed ports** | Every listener uses port `0` and reads the assigned port back, except the two FTP tests, which use 2121 because `tests/mock_ftp.py` hardcodes it and that file is a compatibility target |
| **A mock server, in-tree** | `tests/mock/` gets `http.rs`, `tls.rs`, `lineproto.rs` and keeps `mock_ftp.py`. A Rust mock for the TLS case, because generating a CA and a leaf cert is 30 lines of `rcgen` and 30 lines of Python and the Rust one is in the same build |
| **Watchdog on every blocking test** | 5 s default, 10× the protocol's own default timeout, so a regression fails rather than hangs. Same rule as [`12-concurrency.md`](12-concurrency.md) §10 |
| **No DNS in tests** | A test that needs a name uses `/etc/hosts`-free `net.resolver` with a stub, or connects to a literal address. `127.0.0.1.nip.io` is a DNS query and is banned |
| **24 entries, 3 of them CI gates** | Stated so nobody counts gates as coverage. Two rows assert an API *rejects* an argument (§9.2's timeout, §8.5.1's `cidr_hosts` limit); a test suite that only proves things work is a test suite that cannot catch a loosened bound |

### 10.2 The table

| # | Test | File | What it proves | Gate |
|---|---|---|---|---|
| 1 | `net_tcp_loopback_roundtrip` | `net_tcp.z` | `tcp.listen("127.0.0.1:0")` → `accept` in a `spawn`ed task → `connect` → send/recv both ways → `close`. Handles, not fds: asserts `typeof(c) == "handle"` and `bool(c)` | exit 0 |
| 2 | `net_handle_kind_mismatch` ★ | `net_tcp.z` | `tcp.send(u, bytes)` on a UDP handle raises `TypeError: tcp.send: handle 7#1 is a socket (udp), not a stream`. The §6.1 `ResKind` argument, paid off | exact message |
| 3 | `net_udp_roundtrip` | `net_udp.z` | `udp.bind("127.0.0.1:0")`, `sendto`, `recvfrom`, assert the returned `from` address is `127.0.0.1:<sender>`. Covers the §4.3 `UdpSocket` `ResKind` row | exact |
| 4 | `net_shutdown_half_close` | `net_tcp.z` | `c.shutdown(write)` → the peer's `recv` returns `bytes` of length 0 → the peer can still `send` → the first side's `recv` gets the data. **Half-close is a protocol state, not a close** ([`11-ffi-handles.md`](11-ffi-handles.md) §4.3), and this is its test | bidirectional |
| 5 | `net_poll_events` | `net_poll.z` | A socket that is readable and one that is not, in one `poll(socks, 500)`. A second test row in the same file asserts `poll(socks)` is a `TypeError` naming the missing `timeout_ms`. Asserts `has(read)` on one and absence on the other, and that the result is a `list` of pairs — not a bitmask | both branches |
| 6 | `net_guard_denies_private` ★ | `net_guard.z` | With no configuration, `http.get("http://127.0.0.1:<mock>/")` raises `NetBlocked: connect to 127.0.0.1 is denied by net.guard (deny_private); call net.guard.allow_private = true`. The §9.1 default, asserted | exact message |
| 7 | `net_guard_optout_and_log` | `net_guard.z` | `net.guard.allow_private = true` then the same call succeeds, and `net.guard.log()` contains the earlier **denied** entry. Both halves of §9.1: the escape hatch works *and* the denial is recorded | both |
| 8 | `net_connect_timeout_bounded` ★ | `net_time.z` | A mock server that accepts and then sends nothing. `tcp.connect` must raise at **10 s ± 500 ms**, not 127 s. Proves §9.2's deadline is one `Instant` and not four nested timeouts | 10 s ± 0.5 s |
| 9 | `net_dns_aaaa_dual_stack` | `net_dns.z` | A stub resolver returning one v6 and one v4 address, one of them unroutable. `connect` succeeds via the other within 2 s, proving the Happy Eyeballs skew of §5.3 and that a broken v6 is not a 10 s penalty | < 2 s |
| 10 | `http_get_over_tls` ★ | `http.z` | A mock TLS server with a CA generated by `rcgen` in the test. `http.get("https://127.0.0.1:<mock>/x")` → 200, body, `content-type`, and `r.ok`. The §5.4 happy path | exact |
| 11 | `http_rejects_untrusted_cert` ★★ | `http_tls.z` | The same server, but the client trusts a *different* CA. Must raise `TlsError: certificate signed by unknown authority`. **This is the test that proves §5.4 is implemented rather than documented**, and it fails on any build where verification can be disabled | must raise |
| 12 | `textproto_line_limit` ★ | `lineproto.z` | A mock server that sends 1 MiB with no newline. Both sessions raise `LineTooLong` after at most 1 KiB (§7.9's `max_line` is 512/1024), and the process RSS does not move by 1 MiB. A memory-exhaustion bug reachable from a hostile server | RSS delta < 64 KB |
| 13 | `http_redirect_chain_and_guard` | `http.z` | A mock server with `/a → /b → /c → 200` (3 hops) and `/priv → http://127.0.0.1:1/` (denied). Asserts 3 hops succeed and the denied hop raises, proving the guard is in the *middleware* and not a wrapper | both |
| 14 | `http_no_retry_on_post` | `http.z` | A mock server that counts requests and 500s the first. `http.post(url, json: …)` → **1** request. `{retries: 2}` on a GET → **3** requests. §9.3's idempotency rule, which is the whole reason retries are off by default | 1 and 3 |
| 15 | `ws_echo_and_close_code` | `ws.z` | A `tokio-tungstenite` echo server on loopback. `ws.connect`, `send` a `string`, `send_bytes` a `bytes`, assert both come back with the right type; `close({code: 1001})` and assert the peer's `recv` reports `going_away` | both types |
| 16 | `mqtt_loopback_publish_subscribe` | `mqtt.z` | `rumqttc`'s loopback broker: `subscribe("a/+/c", {qos: 1})`, `publish` QoS 1, assert the message arrives **and** the ack is observed before `publish` returns (§7.8). QoS is an enum: `qos: 3` raises at the call | message + ack |
| 17 | `grpc_unary_over_tls` | `grpc.z` | A `tonic` test server with a hand-built prost message. `ch.unary("/test.Svc/Echo", bytes)` → `status == 0` and the message round-trips. Greenfield (§7.7), so this is its only test | exact |
| 18 | `ftp_login_against_mock_ftp_py` ★ | `09_ftp.z` | **The ported `09_socket_hydra.z`.** The same `tests/mock_ftp.py`, the same `127.0.0.1:2121`, the same `admin`/`s3cret99`, the same `220` banner, plus `ftp.login`, `ftp.pwd`, `ftp.list`. The hydra half of the old file is dropped with a named reason (§8.1) | exit 0 |
| 19 | `net_raw_requires_flag` | `net_raw.z` | `net.raw.ip("1.1.1.1", "2.2.2.2")` without `--allow-raw-sockets` raises `FeatureDisabled: net.raw needs CAP_NET_RAW and --allow-raw-sockets`. **Skipped, not failed**, when the binary was built without the `raw` feature — and the skip is reported, never silent | raises or documented skip |
| 20 | `net_no_cipher_downgrade` | unit (`net/tls_tests.rs`) | Asserts the client config's min version is TLS 1.2, that `webpki-roots` is the verifier, and that **no `ServerCertVerifier` in the crate returns `Ok` unconditionally** — a trait-object sweep, not a grep. §5.4 and §9.5's first row, as code | unit |
| 21 | `net_ffi_fd_leak` | `net_fd.z` | A fresh `Vm` runs 2000 `tcp.connect`/`close` cycles against the mock, then asserts all three numbers: `sys.handle_count() == 0` (nothing leaked), `sys.identities_allocated() == 2001` (one listener plus 2000 clients), and RSS delta < 1 MB. The middle number is the tie to `11-ffi-handles.md` §4.5's accounting — it is an exact figure, not a bound, so the two documents cannot drift apart without failing here | three exact numbers |
| 22 | `net_test_offline_gate` | CI | `grep -rn "example\.com\|1\.1\.1\.1\|8\.8\.8\.8\|google\.com" tests/` returns only the marked `net.guard` fixtures. §9.5's last row. Fails the build | build gate |
| 23 | `net_no_native_tls_gate` | CI | `grep -rn "native-tls\|native_tls" zen/Cargo.toml crates/` is empty, and `cargo tree -i native-tls` errors. A transitive `native-tls` breaks TLS as surely as a direct one | build gate |
| 24 | `net_nmap_stdlib_smoke` ★ | `nmap_stdlib.z` | Parses `std/nmap.z` and resolves every name it imports — `socket`, `scapy` — against the M10 module table, then walks its body and asserts all fourteen call sites in §8.5.1 bind to a live native. **Opens no socket**, so it needs no root and no `NET_RAW`. The only coverage `nmap.z` has ever had; §8.5.1's finding is that the sole `std` consumer of the raw-socket layer had zero tests, so this row exists to keep the §8.5.1 rename honest | 14 bindings |

### 10.3 Why the CI gates are gates and not tests

Tests 22 and 23 assert properties of the *repository*, not of a running program, so they belong in
the same place as [`11-ffi-handles.md`](11-ffi-handles.md) §10's two gates: a `grep` that fails the
build. Test 23 goes further and runs `cargo tree -i native-tls`, which catches the case that
matters most — a transitive dependency reintroducing OpenSSL, which a `grep` of our own files would
miss and which would silently select a different TLS stack for the whole process.

### 10.4 The two old tests, and what happens to them

| Old test | Lines | Fate |
|---|---|---|
| `tests/cases/09_socket_hydra.z` | 43 | **Split.** The `socket` half becomes `09_ftp.z` (test 18 above). The `hydra` half is dropped: `hydra` has no implementation (`grep -c hydra_ src/runtime.rs` → 0), so that test has never passed and there is nothing to port. The drop is named in the migration guide and in the test's replacement comment |
| `tests/cases/20_sockets.z` | 94 | **Rewritten**, not ported. It is 94 lines of hex-packed `sockaddr_in` and raw `sys.*` fds, and both of those are ruled out: the sockaddr becomes a typed `ip` value and the fds become handles. The *assertions* are kept — socketpair echo, `poll`, `getsockopt`/`setsockopt` round-trip, `shutdown` → EOF, TCP loopback with a forked child, UDP `sendto`/`recvfrom` with the peer address checked — and become tests 1, 3, 4, 5. The `sys.fork` child becomes a `spawn`ed task ([`12-concurrency.md`](12-concurrency.md) §2.4), and `sys.poll`'s bitmask becomes the `events` value of §4.2 |
| `tests/cases/14_wa_native.z` | 43 | **Dropped** with `wa` (§8.3). It tests 11 native *names* and no protocol |
| `tests/cases/17_ffmpeg.z` | 50 | Unchanged. Not a network test |

### 10.5 Measurement, so "fast" is a number and not a hope

| Benchmark | What it measures | Target | Where |
|---|---|---|---|
| `bench_net_tcp_throughput` | 64 MiB over loopback, `sendfile`-less | ≥ 2 GB/s, ≥ 80 % of `nc` | §5.1 |
| `bench_net_tcp_latency` | 1-byte request/response round-trip, loopback | ≤ 25 µs p50 | The number `poll` exists for |
| `bench_http_small_get` | `GET /` against the mock TLS server, keep-alive | ≤ 400 µs p50, session resumption on | §5.4, §7.1 |
| `bench_http_cold` | New connection, full handshake, no reuse | ≤ 90 ms p50 | This is the number TLS 1.3 with 0-RTT or resumption moves |
| `bench_net_guard_overhead` | `http.get` with the guard on vs compiled out | ≤ 1 % | §9.1. A security default that costs 8 % gets turned off |
| `bench_net_handle_alloc` | `tcp.connect` + `close` | ≤ 400 ns | §4.1, and the §4.5 identity accounting |
| `bench_ffi_call` | The §7.4 dispatcher, for reference | No regression | [`11-ffi-handles.md`](11-ffi-handles.md) §10 |

**The gate is "no measurable regression" on `bench_core`**, the same gate as
[`00-blueprint-review.md`](00-blueprint-review.md) §4's aggregate, because a networking stack
should not make a script that does not use the network slower. The interesting measurement is
`bench_net_guard_overhead`: §9.1's guard is a `u32` compare and a lookup in a per-`Vm` policy
struct, and if that measures above 1 % the design is wrong, not the budget.

---

## 11. Milestone sequencing

### 11.1 M10, in the order the layers actually build

| Step | Days | Work | Gate |
|---|---|---|---|
| **M10.1** | 2 | `crates/zen-std-net` skeleton: the layer module graph, `Transport`/`Stream` traits, `MockTransport` | `cargo build` with `--no-default-features` succeeds (§2.2's feature matrix) |
| **M10.2** | 3 | L1–L4: `tcp`, `udp`, `poll`, `ip`, the three `ResKind` rows, `net.guard`'s address check | Tests 1–5, 8 |
| **M10.3** | 2 | L6: rustls, `webpki-roots`, the `tls` config, ALPN | Tests 10, 11, 20 |
| **M10.4** | 2 | L3: `net.resolver` on `hickory`, the dual-stack connect | Tests 6, 7, 9 |
| **M10.5** | 4 | L5–L7 HTTP: `http.client`, the response object, the Zen cookie jar, `http.parallel` | Tests 10, 13, 14, 21 |
| **M10.6** | 1 | `std/url.z` extraction of the 5 `urllib` pure functions | The old 5 tests port unchanged |
| **M10.7** | 2 | L5 `TextProto` + `pop3` + `telnet` | Test 12 |
| **M10.8** | 3 | `smtp`, `imap`, `ftp`, `ssh` | Test 18 |
| **M10.9** | 1 | `ws` | Test 15 |
| **M10.10** | 1 | `net.raw` (feature + flag), the 18 ported `scapy` natives | Tests 19, 24 |
| **M10.11** | 2 | Migration: the two old tests rewritten, the `dropped`-module stubs, the migration guide, **and the `std/nmap.z` rename** (§8.5.1's fourteen call sites) | §10.4, Test 24 |
| **M10.12** | 1 | CI gates 22, 23; `bench_net_*` recorded | §10.5 |
| **Total** | **24** | | |

### 11.2 M10.13 and M10.14: the two greenfield protocols

| Step | Days | Work | Gate |
|---|---|---|---|
| **M10.13** | 2 | `grpc` behind the feature: `connect`, `unary`, `server_stream`, metadata, deadlines | Test 17 |
| **M10.14** | 1 | `mqtt` behind the feature: `connect`, `subscribe`, `publish`, `on_message` | Test 16 |

**Why these are the last two steps of M10 and not the first two.** They are greenfield (§8.1),
they are the two least-used protocols in this document, and they are the two with the fewest users
in the old tree — because they do not exist in the old tree. They land after M10.1–M10.12, so if
M10 slips the slip is two optional modules rather than a half-finished transport.

**The naming, because the first draft of this section was ambiguous.** M10.13 and M10.14 are *steps
of M10*, not a separate milestone. The earlier draft also numbered the HTTP step M10.5, which made
"M10.5" mean two different things in one document — the HTTP session layer in §11.1 and the
greenfield pair in §11.2. M10.5 is HTTP; the greenfield pair is M10.13/M10.14. One number, one
thing. `quic` is in the same bucket for the
reasons in §5.2 and ships *after* both, if at all.

### 11.3 What is explicitly not in M10.1–M10.14

| Not in | Why |
|---|---|
| An HTTP **server** | `hyper`'s `server` feature is off by default (§2.2). It is a real feature — `http.serve` is 40 lines once hyper's server API is wrapped — and it is not a *networking* milestone item, it is a framework decision. M12 |
| HTTP/3 over `quinn` | `hyper`'s HTTP/3 support is unstable. §12.4 |
| IPv6-only mode | `IPV6_V6ONLY` is set to `false` and that is the only IPv6 control. A per-request family filter is a `net.guard` concern, not a socket one |
| Multicast, broadcast receivers, `SO_REUSEPORT` | Not in the old tree, not in any test, and each is a distinct design. §12.4 |
| TLS client certificates, CRLs, OCSP | §5.4. All three need a credential store and a policy language |
| An FTP **server**, an SMTP server, an MQTT broker | Clients only. This is a client library |
| Zero-copy `sendfile`/`splice` | `tokio`'s `sendfile` is Linux-only and the abstraction would be per-platform. A `sys.sendfile` escape hatch in M12 if the benchmark says so |
| `browser`, `cdp` | Extracted to an optional crate (§7.6) |
| `wifi`, `bluetooth`, `hydra`, `scapy` scanners, `socket.scan` | Dropped, with reasons in §8 |
| `wa*` | Dropped, with the argument in §8.3 |

---

## 12. Rulings a reviewer should double-check

### 12.1 The structural rulings

| # | Ruling | Where | The alternative view |
|---|---|---|---|
| 1 | **Wrap the layers; implement none of them** | §2.1, §2.2 | [`00-blueprint-review.md`](00-blueprint-review.md) §3 endorses wrapping. The exception is `TextProto` (§7.9), which is ~350 lines of hand-rolled parser because two protocols have no crate. A reviewer could argue for shelling out to `openssl s_client` instead; the answer is that it is a process per connection and cannot do §9.2's deadline |
| 2 | **The layer boundary is enforced by Rust module privacy, not by review** | §2.1 | The alternative is trusting a convention. The `Transport` trait is 20 lines and makes an upward import a compile error inside the crate |
| 3 | **`net.guard` denies private addresses *by default*** | §9.1 | The alternative is off-by-default, which is what every HTTP client in every language does. The argument is that a default-off security control is a vulnerability with a config knob, and that the one script this breaks is a dev-server script with a one-line opt-out and a named error. **This is the ruling most likely to be overturned by a single user request**, and §10's tests 6 and 7 are the two halves of the answer either way |
| 4 | **DNS rebinding is not fully solved, and is not claimed to be** | §9.1 | The alternative is a full pin-every-connection policy with per-host address caches. That is ~200 lines and it breaks virtual hosting, `localhost` round-robins, and any CDN that re-resolves. Stating the gap is better than a mitigation that is wrong 5 % of the time |
| 5 | **Retries are off by default and, when enabled, are idempotent-methods-only** | §9.3 | The alternative is retry everything, which double-charges cards. The cost is that a script hammering a flaky endpoint must opt in — and it says so in the error-free path, by requiring the flag |
| 6 | **One `ResKind` per protocol, so a wrong-protocol handle is a typed error** | §6.1 | The alternative is one `ResKind::Protocol`, which saves nine enum variants and loses `handle 41 is an imap session, not a socket`. Test 2 is the justification |

### 12.2 The dependency rulings

| # | Ruling | Where | The alternative view |
|---|---|---|---|
| 7 | **TLS 1.3 preferred, 1.2 permitted, no verification escape hatch, `webpki-roots` only** | §5.4 | Four separate decisions, each arguable. Verification is the one that will generate the most requests ("I need to test against a self-signed server"), and §5.4's answer is a *per-connection* `ca_pem` rather than a global — which keeps the ergonomics and removes the bug. `webpki-roots` vs the system store is the one that breaks corporate proxies, and the reversal cost is written down in the same section |
| 8 | **`russh` for SSH, replacing a 4-native shell-out** | §7.5 | `russh` is pre-1.0 and substantially larger than four shelled-out lines. The alternative — keep the shell-out as the default and offer `russh` behind a flag — is defensible, and §7.5 offers `net.ssh_shell` for exactly that reason. The argument for the swap is that `ssh_available` is a tell: a module whose first job is to check whether a binary exists is not implementing SSH |
| 9 | **`quinn` is shipped behind a flag with one test** | §5.2 | The alternative is not shipping it. An off-by-default dependency is untested, and an untested network path is worse than an absent one; but *not shipping it* means the decision is never revisited under pressure. One loopback echo test is the cheapest way to keep the question open |
| 10 | **`tonic`/`prost` for gRPC with strings and `bytes`, no `.proto` compiler** | §7.7 | The alternative is a build-time codegen step, which makes `protoc` a build dependency of a language that has no business requiring it. The cost is that a user writes the method name as a string, and a typo is a runtime error rather than a compile error |
| 11 | **`ring`, not `aws-lc-rs`; no `native-tls` anywhere** | §2.2, §5.4 | `aws-lc-rs` is faster. It needs a C toolchain and NASM on Windows. Test 23 asserts the *transitive* case with `cargo tree -i native-tls`, because a direct-dependency grep misses the one that matters |
| 12 | **POP3 and Telnet are hand-rolled on one shared helper** | §7.9 | The alternative is two more dependencies, or a blocking `std::net::TcpStream` per connection (which would put a `MayBlock` native on a *different* threading model from every other network native — a bug factory). ~350 lines with one `max_line` test is the cheaper end |

### 12.3 The scope rulings

| # | Ruling | Where | The alternative view |
|---|---|---|---|
| 13 | **The `wa*` stack is dropped: 307 687 bytes, 19.5 % of `src/*.rs`** | §8.3 | The strongest counter-argument is that the work is already done and 11 users have `wa.send_text`. §8.3.1 answers it with named-error stubs for one minor release and a package-index story for the rest. A reviewer who disagrees should say what a WhatsApp module is *for* in a systems language whose stated use case is packet analysis |
| 14 | **`socket.scan`, `scapy.syn_scan` and `scapy.arp_scan` are not ported, at any flag** | §4.4 | The alternative is to ship them behind `--allow-raw-sockets`. The argument is that a flag is a poor fit for a credential-attack primitive that a *network* flag implies nothing about, and that a scanner is 20 lines over primitives that *are* ported. `std/nmap.z` remains |
| 15 | **`hydra` is dropped, and the test that calls it is split rather than ported** | §8.1, §10.4 | The alternative is to implement it, which would make a documented ghost into a real credential-attack tool. The cost of the drop is that `09_socket_hydra.z`'s hydra half never worked, so nobody loses a passing test |
| 16 | **`wifi` and `bluetooth` are dropped rather than reimplemented** | §8.2 | The alternative is `dbus`/`btleplug` bindings, which is a second async runtime and a platform matrix. Both modules are `nmcli`/`bluetoothctl` wrappers; a user who wants them has the tools |
| 17 | **No HTTP server in M10** | §2.2, §11.3 | A server is a natural companion to a client and `hyper` makes it easy. It is also a framework decision — routing, middleware, graceful shutdown, keep-alive policy — and it is not a networking layer. `hyper`'s `server` feature is off so that "we do not have a server" is a compile error rather than a runtime surprise |

### 12.4 The three questions this document is answering badly

Stated plainly, because a plan that lists only its strong rulings is not being read carefully.

| Question | Why it is weak | What would fix it |
|---|---|---|
| **Corporate TLS interception** | `webpki-roots` means a MITM appliance's root is not trusted, so `http.get` to an internal service fails in exactly the environment where the language's users work. §5.4's answer is honest and probably insufficient | A `--use-system-roots` flag, a test that skips where no store exists, and a documented "your platform decides trust" caveat. §5.4 costs this out: a flag, a conditional test, and a support burden |
| **gRPC and MQTT have one test each** | They are greenfield, and one test is not coverage. A streaming `grpc` call that deadlocks under backpressure, or an MQTT reconnect storm, would ship | Six more tests, in M10.13's own week, once the greenfield implementation exists to write them against. Writing them now would be writing tests for code that does not exist |
| **`net.raw` is 18 natives and one flag test** | The `pnet` port is the least-reviewed code here because it needs privileges to test, and the tests that matter (a checksum against a known vector, a parse of a captured frame) do **not** need privileges and are therefore the ones most likely to be skipped | Three privilege-free tests — checksum vectors, a hand-built frame parsed back to its fields, a `cidr_hosts` bound assertion — in M10.10, plus a documented "run these with `sudo`" for the rest. §4.4's `cidr_hosts` limit deserves its own test in any case |

### 12.5 The accounting and naming rulings

| # | Ruling | Where | The alternative view |
|---|---|---|---|
| 18 | **§8 is 106 disposition rows over 55 individually named modules and all 160 natives; a literal 60-row *module* table is not available and is not manufactured** | §8 | The brief asked for a 60+ module disposition table. The tree has 25 network-side modules and 23 `std` modules — 48 in scope, not 60 — and padding a table to 60 rows with rows that say "unchanged" would make the table harder to audit, not easier, which is the opposite of what a disposition table is for. So §8 counts differently: 25 (§8.2) + 7 (`wa*`, §8.3) + 23 (`std`, §8.5) = **55 module rows, each naming a file that exists**, plus 21 native-group rows covering all 160 natives (§8.4) and 14 per-call-site rows for the one `std` file that M10 touches (§8.5.1) = 106 rows, none of them filler. A reviewer who wants 60 *module* rows should ask for 12 more modules to exist |
| 19 | **`std/sys.z` and the native `sys` module keep the same name and are disambiguated by resolution order, not by renaming either** | §8.5 row 18 | The alternative is renaming one of them. `std/sys.z` is a user-facing pure-Zen facade that predates the native module and has callers; the native `sys` is being specified in `11-ffi-handles.md` §8 and has no users yet, because the whole native surface is new. Renaming the *new* thing is cheaper and breaks nobody — but "sys" is the name the POSIX natives want, and a module called `posix` that wraps 94 syscalls is a worse name than one called `sys` that is shadowed by a facade. The rule is the one already in force for every other `std` module: an explicit `import` of `sys` resolves to the native module, `std.sys` is always addressable, and §12.6's third correction is the test that keeps this honest |

### 12.6 The corrections this document needed during review

A disposition table is only worth the review that produced it, so the errors found in the first
draft are recorded here rather than quietly fixed. Each was a factual error against the tree, not
a matter of taste.

| # | What the first draft said | What is true | How it was found |
|---|---|---|---|
| 1 | "`nmap.z` is a pure-Zen wrapper that does not import the network stack" | `std/nmap.z:35-36` is `import scapy` / `import socket`, and the file makes **14** native calls across both. It is the largest consumer of the raw-socket layer in the tree and it breaks at M10 | `grep -n "import" std/nmap.z` |
| 2 | "`std/nmap.z` is unchanged and already works" | It is unchanged *today* and has **no test at all**. The only `tests/` hit for "nmap" is the substring in `sys.munmap` at `19_sys_ffi.z:84` | `grep -rn nmap tests/` |
| 3 | "`std/sys.z` is one of the sixteen out-of-scope modules" | True, but it shares a name with the native `sys` module of `11-ffi-handles.md` §8, which makes the resolution order a ruling rather than an omission | Ruling 19 |
| 4 | The native disposition groups sum to 158 | 160. The `requests` group was short by three (`__requests_sess_*` is 11 names, not 10) and `dns_resolve` was counted as two uniques when it is one native behind two keys | Added the groups up |
| 5 | The plan has two poll calls, `poll` and `poll_wait` | There is one. `poll([…], 5000)` appears in §4.3's example and `poll_wait` appeared only in §9.2's table, so §9.2 invented a second name for a call that already had one | Grepping every `poll` call site |
| 6 | 23 tests, of which test 5 exercises `poll_wait` | 24. Test 24 is the `nmap.z` smoke test from correction 1, and test 5 gained the missing-timeout `TypeError` half | §8.5.1 |
| 7 | `net.guard` denies private addresses by default, and 23 tests dial loopback | Both are true, and together they meant the suite contradicted itself: 23 of 24 rows would have been denied by the default they also assert. Every test file now sets `net.guard.allow_private = true` in one shared helper, and that helper is banned from the one file that asserts the default | §10.1 |
| 8 | `00-blueprint-review.md` §5.6 and `12-concurrency.md` §9.4 | `00` has a §5 with no subsections, and §9.4 is in `11-ffi-handles.md` | Resolving every `§`-reference against the target's actual headings |
