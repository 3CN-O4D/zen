# Appendix D: Standard Library Index (exhaustive)

A complete index of Zen 3.0 standard library modules and their principal
members. "→" indicates the category of each member.

## D.1 Module list

| Module | Purpose |
|--------|---------|
| `math` | Mathematical functions and constants |
| `random` | Pseudorandom numbers (not for secrets) |
| `string` | String utilities and constants |
| `collections` | `Counter`, `chain`, `flatten` |
| `json` | JSON parse/serialize |
| `time` / `datetime` | Time and date operations |
| `pathlib` / `os` | File system operations |
| `re` | Regular expressions |
| `base64` / `binascii` | Base64 and hex encoding |
| `hashlib` | Cryptographic hashes (MD5, SHA, BLAKE2) |
| `crypto` | Full cryptography (hash, HMAC, symmetric, asymmetric, JWT, KDF, OTP, X.509) |
| `socket` | Low-level socket programming |
| `http` | HTTP protocol helpers |
| `requests` | High-level HTTP client |
| `http.server` | Building HTTP servers |
| `dns` | Domain name resolution |
| `url` | URL parsing and manipulation |
| `raw sockets` | Packet-level networking (AF_PACKET) |
| `threading` | Threads and synchronization |
| `channels` | Inter-thread communication |
| `mutexes` / locks | Synchronization primitives |
| `ffi` | Foreign Function Interface (C) |
| `sys` | System information and operations |
| `pymport` | Python interoperability |
| `logging` | Logging facility |
| `functools` | `reduce`, `partial`, `lru_cache`, `cmp_to_key` |
| `itertools` | `enumerate`, `zip`, `chain`, `product`, `permutations`, `combinations` |
| `statistics` | `mean`, `median`, `mode`, `variance`, `stdev`, `linear_regression` |
| `operator` | `add`, `sub`, `mul`, `itemgetter`, `attrgetter`, `methodcaller` |
| `struct` | `calcsize` (binary pack/unpack helpers) |
| `tempfile` | `mkdtemp`, `mkstemp`, `dir` |
| `uuid` | UUID generation (`uuid4`, `uuid1`, `uuid3`, `uuid5`) |
| `glob` | pathname pattern expansion |
| `shutil` | `copy`, `copy2`, `copytree`, `move`, `rmtree`, `which` |
| `crunch` | random data generation |
| `nmap` | network scanning helpers |
| `ffmpeg` | media processing helpers |

## D.2 math module

Constants: `pi`, `e`, `tau`, `inf`, `nan`.

Functions: `abs`, `sqrt`, `pow`, `exp`, `log`, `log2`, `log10`, `floor`, `ceil`,
`trunc`, `round`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`,
`degrees`, `radians`, `gcd`, `lcm`, `factorial`, `comb`, `perm`, `remainder`,
`copysign`, `min`, `max`, `fsum`, `prod`, `hypot`, `isnan`, `isfinite`, `isinf`,
`modf`, `frexp`, `ldexp`.

## D.3 random module

Functions: `seed`, `random`, `randint`, `randrange`, `choice`, `choices`,
`sample`, `shuffle`, `uniform`, `hex`.

⚠️ NOT cryptographically secure. Use `crypto` module for secrets/tokens.

## D.4 string module

Constants: `digits`, `hexdigits`, `octdigits`, `ascii_letters`,
`ascii_lowercase`, `ascii_uppercase`, `punctuation`, `whitespace`, `printable`.

Functions: `upper`, `lower`, `title`, `capitalize`, `swapcase`, `strip`,
`lstrip`, `rstrip`, `split`, `join`, `replace`, `count`, `find`, `startswith`,
`endswith`, `isdigit`, `isalpha`, `isalnum`, `isspace`, `islower`, `isupper`.

## D.5 collections module

Functions: `Counter`, `chain`, `flatten`.

## D.6 json module

Functions: `parse`, `stringify`/`encode`, `pretty`, `load`, `save`.

JSON ↔ Zen mapping: object↔dict, array↔list, string↔string, number↔int/float,
boolean↔bool, null↔nil.

## D.7 time / datetime module

Functions: `time`, `sleep`, `now`, `today`, `strftime`, `strptime`,
`timestamp`, `fromtimestamp`, `timedelta`.

## D.8 pathlib / os module

pathlib: `Path`, `/` join, `exists`, `is_file`, `is_dir`, `name`, `stem`,
`suffix`, `parent`, `resolve`, `mkdir`, `glob`.

os: `getcwd`, `chdir`, `listdir`, `makedirs`, `remove`, `rmdir`, `rename`,
`stat`, `environ`.

## D.9 re module

Functions: `match`, `search`, `findall`, `sub`, `split`, `compile`.

Patterns: `\d` `\D` `\w` `\W` `\s` `\S` `.` `^` `$` `*` `+` `?` `{n,m}` `(...)`
`|` `\b`.

## D.10 base64 / binascii module

base64: `b64encode`, `b64decode`, `url_encode`, `url_decode`.
binascii: `hexlify`, `unhexlify`.

## D.11 hashlib module

Hashes: `md5`, `sha1`, `sha224`, `sha256`, `sha384`, `sha512`, `sha3_256`,
`sha3_512`, `blake2b`, `blake2s`, `pbkdf2_hmac`.

## D.12 crypto module

Hashing: `sha256/384/512`, `sha3_256/512`, `blake3`, `md5`, `sha1`.
HMAC: `sha256(key,data)`, `sha512(key,data)`.
Symmetric: `generate_key`, `encrypt`, `decrypt`, `encrypt_aes_gcm`,
`decrypt_aes_gcm`, `encrypt_chacha20_poly1305`, `decrypt_chacha20_poly1305`.
Asymmetric: `generate_rsa`, `rsa_encrypt/decrypt`, `rsa_sign/verify`,
`generate_ed25519`, `ed25519_sign/verify`, `generate_x25519`,
`x25519_shared_secret`.
Encoding: `b64encode/decode`, `b64urlencode/decode`, `hex_encode/decode`.
JWT: `sign`, `verify`, `decode`.
KDF/passwords: `pbkdf2`, `argon2id`, `argon2id_hash`, `verify_argon2id`, `hkdf`.
OTP: `totp`, `totp_verify`, `hotp`, `hotp_verify`, `generate_secret`.
Random: `bytes(n)`, `hex(n)`, `base64(n)`, `uuid4()`, `secure_token()`,
`int(min, max)`.
X.509: `parse(pem)`.

## D.13 networking modules

socket: `socket`, `connect`, `bind`, `listen`, `accept`, `send`, `recv`,
`sendto`, `recvfrom`, `close`, `setsockopt`, `settimeout`, `setblocking`.
http: request/response parsing, status codes, headers.
requests: `get`, `post`, `put`, `patch`, `delete`, `head`, `options`, `Session`.
http.server: request handler, routing, static files, middleware.
dns: `resolve`, `reverse`, record types A/AAAA/CNAME/MX/TXT/NS/PTR.
url: `parse`, `build`, `join`, `quote`, `unquote`, `parse_qs`.
raw sockets: `AF_PACKET`, `SOCK_RAW`, packet crafting.

## D.14 concurrency modules

threading: `Thread`, `spawn`, `join`, `start`.
channels: `Channel`, `send`, `recv`, `close`.
mutexes: `Mutex`, `Lock`, `acquire`, `release`.
patterns: thread-safe caches, producer-consumer, thread pools.

## D.15 FFI / system / interop modules

ffi: `load_library`, `get_symbol`, `call`, `Pointer`, `Handle`.
sys: `argv`, `platform`, `version`, `exit`, `environ`, `getsizeof` (if present).
pymport: `import` Python modules, marsalling values, chained tracebacks.

## D.16 Module → chapter mapping

| Chapter | Module(s) |
|---------|-----------|
| 15 | math |
| 16 | random |
| 17 | string |
| 18 | collections |
| 19 | json |
| 20 | time / datetime |
| 21 | pathlib / os |
| 22 | re |
| 23 | base64 / binascii |
| 24 | hashlib |
| 25 | crypto (complete) |
| 26 | networking fundamentals |
| 27 | socket |
| 28 | http |
| 29 | requests |
| 30 | http.server |
| 31 | HTTPS / SSL / TLS |
| 32 | dns |
| 33 | url |
| 34 | raw sockets |
| 35 | threading |
| 36 | channels |
| 37 | mutexes / locks |
| 38 | thread safety patterns |
| 39 | ffi fundamentals |
| 40 | ffi in practice |
| 41 | sys |
| 42 | pymport |

## D.17 Exercises

1. List the hash functions available in `hashlib` and their digest sizes.
2. Name three modules you must NOT use for cryptographic secrets (random, math,
   etc.) and say which to use instead (`crypto`).
3. For the URL `https://a.b:8080/p?q=1#f`, name each component.
4. List the DNS record types and what each resolves.
5. Name the exit codes used by the `zen` CLI.
6. For `crypto`, name two authenticated-encryption algorithms supported.
7. List the concurrency primitives and their purpose.
8. For `pymport`, name two error codes in the Z05xx range.
9. Write a one-line summary of what each of modules math/random/string does.
10. Build your own "cheat sheet" mapping module → most-used 3 functions.

'''
print('appendix d done')