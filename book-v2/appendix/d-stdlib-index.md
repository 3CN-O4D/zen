# Appendix D: Standard Library Index

Zen's standard library includes pure-Zen modules (`std/*.z`) and native
(Rust) modules. This appendix indexes every module.

## D.1 Pure-Zen modules (std/*.z)

| Module | File | Description |
|--------|------|-------------|
| `argparser` | `std/argparser.z` | Command-line argument parsing |
| `base64` | `std/base64.z` | Base64 encode/decode |
| `binascii` | `std/binascii.z` | Hex/binary conversions |
| `bytes` | `std/bytes.z` | Byte buffer operations |
| `collections` | `std/collections.z` | deque, Counter, OrderedDict, namedtuple |
| `crunch` | `std/crunch.z` | Data crunching utilities |
| `crypto` | `std/crypto/` | Cryptography (see Chapter 25) |
| `ffmpeg` | `std/ffmpeg.z` | FFmpeg integration |
| `functools` | `std/functools.z` | Functional programming tools |
| `glob` | `std/glob.z` | File pattern matching |
| `hashlib` | `std/hashlib.z` | Cryptographic hashes (see Chapter 24) |
| `itertools` | `std/itertools.z` | Iterator utilities |
| `json` | `std/json.z` | JSON parse/serialize (see Chapter 19) |
| `logging` | `std/logging.z` | Logging framework |
| `math` | `std/math.z` | Mathematical functions (see Chapter 15) |
| `nmap` | `std/nmap.z` | Network scanner (see Chapter 44) |
| `operator` | `std/operator.z` | Operators as functions |
| `pathlib` | `std/pathlib.z` | Object-oriented paths (see Chapter 21) |
| `random` | `std/random.z` | Random numbers (see Chapter 16) |
| `shutil` | `std/shutil.z` | File operations |
| `statistics` | `std/statistics.z` | Statistical functions |
| `string` | `std/string.z` | String utilities (see Chapter 17) |
| `struct` | `std/struct.z` | Binary struct packing |
| `tempfile` | `std/tempfile.z` | Temporary files/dirs |
| `uuid` | `std/uuid.z` | UUID generation |

## D.2 Native modules (Rust)

| Module | Description |
|--------|-------------|
| `fs` | File system operations (Chapter 13) |
| `os` | OS operations, env, process info (Chapter 21) |
| `time` | Time operations (Chapter 20) |
| `json` | JSON native tier |
| `socket` | TCP/UDP/raw sockets (Chapter 34) |
| `ffi` | Foreign function interface (Chapters 39-40) |
| `channel` | Message-passing channels (Chapter 37) |
| `thread` | Threading (Chapter 37) |
| `shared` | Atomic shared values (Chapter 37) |
| `process` | Process spawn/control |
| `net` | TCP/UDP helpers |
| `net.dns` | DNS resolution (Chapter 32) |
| `requests` | HTTP client (Chapter 29) |
| `http` | HTTP client |
| `http.server` | HTTP static server (Chapter 30) |
| `urllib` | URL utilities |
| `urllib.parse` | URL parsing (Chapter 33) |
| `urllib.request` | URL fetching |
| `html` | HTML parsing |
| `html.parser` | HTML parser |
| `lxml` | lxml compatibility |
| `lxml.etree` | XML element tree |
| `lxml.html` | HTML parsing |
| `raw` | Raw socket/packet crafting (Chapter 34) |
| `crypto` | Cryptography (Chapter 25) |
| `tls` | TLS with cert pinning |
| `sqlite` | SQLite database |
| `qr` | QR code generation |
| `wa` | WebAssembly |
| `ffmpeg` | FFmpeg |
| `hydra` | Hydra |
| `sys` | Low-level syscalls (Chapter 41) |

## D.3 Module tiers

Modules with both pure-Zen and native implementations expose the native tier
under `<name>.native`:

```zen
import math
print(math.pi)           # pure tier
print(math.native.pi)    # native tier
```

Force tiers with:
- `ZEN_NATIVE_ONLY=math` — force native
- `ZEN_PURE = true` — force pure-Zen

## D.4 Built-ins (always available)

`print`, `input`, `open`, `exit`, `len`, `str`, `int`, `float`, `bool`,
`list`, `dict`, `set`, `tuple`, `range`, `enumerate`, `zip`, `sorted`,
`reversed`, `any`, `all`, `sum`, `min`, `max`, `map`, `filter`,
`typeof`, `id`, `callable`. See Appendix A.
