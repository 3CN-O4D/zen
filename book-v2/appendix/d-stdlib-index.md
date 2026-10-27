# Appendix D: Standard Library Index

Zen's standard library includes pure-Zen modules (`std/*.z`) and native (Rust) modules. This appendix indexes every module.

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
| `nmap` | `std/nmap.z` | Network scanner |
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

| Module | Description | Chapter |
|--------|-------------|---------|
| `fs` | File system operations | 13 |
| `os` | OS operations, env, process info | 21 |
| `time` | Time operations | 20 |
| `json` | JSON native tier | 19 |
| `socket` | TCP/UDP/raw sockets | 27 |
| `ffi` | Foreign function interface | 39-40 |
| `sys` | Low-level syscalls | 41 |
| `process` | Process spawn/control | 41 |
| `thread` | Threading | 35 |
| `channel` | Message-passing channels | 36 |
| `shared` | Atomic shared values | 37 |
| `net` | TCP/UDP helpers | 26 |
| `net.dns` | DNS resolution | 32 |
| `requests` | HTTP client | 29 |
| `http` | HTTP client | 28 |
| `http.server` | HTTP static server | 30 |
| `urllib` | URL utilities | 33 |
| `urllib.parse` | URL parsing | 33 |
| `urllib.request` | URL fetching | 33 |
| `html` | HTML parsing | - |
| `html.parser` | HTML parser | - |
| `lxml` | lxml compatibility | - |
| `lxml.etree` | XML element tree | - |
| `lxml.html` | HTML parsing | - |
| `raw` | Raw socket/packet crafting | 34 |
| `crypto` | Cryptography | 25 |
| `tls` | TLS with cert pinning | 31 |
| `sqlite` | SQLite database | - |
| `qr` | QR code generation | - |
| `wa` | WebAssembly | - |
| `ffmpeg` | FFmpeg | - |
| `hydra` | Hydra | - |

## D.3 Module tiers

Modules with both pure-Zen and native implementations expose the native tier under `<name>.native`:

```zen
import math
print(math.pi)           # pure tier
print(math.native.pi)    # native tier
```

Force tiers with:
- `ZEN_NATIVE_ONLY=math` — force native
- `ZEN_PURE = true` — force pure-Zen (set at top of `.z` module)

## D.4 Built-ins (always available)

`print`, `input`, `open`, `exit`, `len`, `str`, `int`, `float`, `bool`, `list`, `dict`, `set`, `tuple`, `range`, `enumerate`, `zip`, `sorted`, `reversed`, `any`, `all`, `sum`, `min`, `max`, `map`, `filter`, `typeof`, `id`, `callable`. See Appendix A.

## D.5 Error codes

`Z01xx`–Z05xx`. See Appendix B.
