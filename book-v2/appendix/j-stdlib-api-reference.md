# Appendix J: Standard Library — Complete API Reference

Exhaustive per-module reference: every function, its signature, purpose, and a
worked example. Signatures are written in Zen-like notation (`func name(param:
Type) -> ReturnType`) where the build supports type annotations; otherwise the
parameter list is shown plainly.

## J.1 math — mathematical functions and constants

Constants: `pi`, `e`, `tau`, `inf`, `nan`.

```
abs(x: Number) -> Number
```
Return the absolute value of `x`.
```zen
import math
print(math.abs(-7))   # 7
```

```
sqrt(x: Float) -> Float
```
Square root. Raises a domain error for negative `x`.
```zen
print(math.sqrt(25))   # 5.0
print(math.sqrt(2))    # 1.41421356...
```

```
pow(a: Number, b: Number) -> Number
```
`a` raised to the power `b`.
```zen
print(math.pow(2, 10))   # 1024.0
```

```
exp(x: Float) -> Float
```
e raised to `x`.
```zen
print(math.exp(1))   # 2.718281828...
```

```
log(x: Float, base: Float = null) -> Float
```
Natural log by default; `log(x, base)` for an arbitrary base.
```zen
print(math.log(math.e))     # 1.0
print(math.log(100, 10))    # 2.0
```

```
log2(x: Float) -> Float
log10(x: Float) -> Float
```
Base-2 and base-10 logarithms.
```zen
print(math.log2(8))    # 3.0
print(math.log10(100)) # 2.0
```

```
floor(x: Float) -> Float
ceil(x: Float) -> Float
trunc(x: Float) -> Float
round(x: Float) -> Float
```
Round toward −∞, +∞, toward zero, and to nearest, respectively.
```zen
print(math.floor(3.7))   # 3.0
print(math.ceil(3.2))    # 4.0
print(math.trunc(-3.7))  # -3.0
print(math.round(3.5))   # 4.0
```

```
sin(x) / cos(x) / tan(x)
asin(x) / acos(x) / atan(x) / atan2(y, x)
```
Trigonometric functions (radians).
```zen
print(math.sin(0))          # 0.0
print(math.cos(0))          # 1.0
print(math.atan2(1, 1))     # 0.785398... (pi/4)
```

```
degrees(x) / radians(x)
```
Convert between radians and degrees.
```zen
print(math.degrees(math.pi))   # 180.0
print(math.radians(180))       # 3.14159...
```

```
gcd(a: Int, b: Int) -> Int
lcm(a: Int, b: Int) -> Int
```
Greatest common divisor / least common multiple.
```zen
print(math.gcd(48, 18))   # 6
print(math.lcm(4, 6))     # 12
```

```
factorial(n: Int) -> Int
comb(n: Int, k: Int) -> Int
perm(n: Int, k: Int = null) -> Int
```
Factorial, combinations ("n choose k"), permutations.
```zen
print(math.factorial(5))   # 120
print(math.comb(5, 2))     # 10
print(math.perm(5, 2))     # 20
```

```
remainder(x, y) / copysign(x, y)
min(a, b = null) / max(a, b = null)
fsum(a, b = null) / prod(a, b = null) / hypot(a, b = null)
```
Remainder, sign copy, min/max, precise sum, product, hypotenuse.
```zen
print(math.min(3, 1))      # 1
print(math.max(3, 1))      # 3
print(math.hypot(3, 4))    # 5.0
print(math.prod([2, 3, 4]))# 24
```

```
isnan(x) / isfinite(x) / isinf(x)
modf(x) / frexp(x) / ldexp(x, exponent)
```
NaN/finite/infinite tests; fraction/integer split; mantissa/exponent; multiply
by power of two.
```zen
print(math.isnan(float("nan")))   # true
print(math.isinf(float("inf")))   # true
print(math.modf(3.5))             # (0.5, 3.0)
```

## J.2 random — pseudorandom numbers (NOT for secrets)

```
seed(n: Int)
```
Seed the generator for reproducible sequences.
```zen
import random
random.seed(42)
print(random.random())   # deterministic given the seed
```

```
random() -> Float      # [0, 1)
randint(a: Int, b: Int) -> Int   # [a, b] inclusive
randrange(start, stop=None, step=1) -> Int
uniform(a: Float, b: Float) -> Float
hex(n: Int) -> String
```
```zen
print(random.randint(1, 6))     # die roll 1-6
print(random.uniform(1.0, 2.0)) # random float
print(random.hex(16))           # 16 random hex chars
```

```
choice(seq) -> element
choices(seq, k: Int = 1) -> list
sample(seq, k: Int) -> list
shuffle(seq)   # in place
```
```zen
print(random.choice(["a","b","c"]))
let deck = list(range(1, 53))
random.shuffle(deck)
print(random.sample(deck, 5))   # 5 unique cards
```

⚠️ Use `crypto` module functions (`secure_token`, `bytes`, `uuid4`) for
passwords, tokens, and keys — `random` is not cryptographically secure.

## J.3 string — string utilities

Constants: `digits`, `hexdigits`, `octdigits`, `ascii_letters`,
`ascii_lowercase`, `ascii_uppercase`, `punctuation`, `whitespace`, `printable`.

```
upper(s) / lower(s) / title(s) / capitalize(s) / swapcase(s)
strip(s) / lstrip(s) / rstrip(s)
split(s, sep=None) / splitlines(s)
join(sep, parts)
replace(s, old, new)
count(s, sub)
find(s, sub) / rfind(s, sub)      # index or -1
startswith(s, prefix) / endswith(s, suffix)
contains(s, sub)
ljust(s, width) / rjust(s, width) / center(s, width) / zfill(s, width)
isdigit(s) / isalpha(s) / isalnum(s) / isspace(s) / islower(s) / isupper(s)
```
```zen
import string
print(string.upper("zen"))          # ZEN
print(string.join("-", ["2024","01","15"]))  # 2024-01-15
print(string.isdigit("42"))         # true
print(string.digits)                # 0123456789
```

## J.4 collections — Counter, chain, flatten

```
Counter(xs)   # count frequencies -> dict-like
chain(*iters) # concatenate iterables
flatten(xs)   # flatten one level
```
```zen
import collections
print(collections.Counter(["a","b","a","c"]))  # a:2, b:1, c:1
for x in collections.chain([1,2], [3,4]) { print(x) }   # 1 2 3 4
print(collections.flatten([[1,2],[3],[4,5]]))   # [1,2,3,4,5]
```

## J.5 json — JSON parse / serialize

```
parse(s: String) -> Value
stringify(x) / encode(x) -> String
pretty(x) -> String
load(path: String) -> Value
save(path: String, x)
```
```zen
import json
let d = #{name: "Ada", age: 36}
let s = json.stringify(d)
print(s)                    # {"name":"Ada","age":36}
let back = json.parse(s)
print(back["name"])         # Ada
```

JSON mapping: object↔dict, array↔list, string↔string, number↔int/float,
boolean↔bool, null↔nil.

## J.6 time / datetime

```
time() -> Float              # epoch seconds
sleep(ms: Int)
now() -> DateTime
today() -> Date
strftime(fmt: String, t) -> String
strptime(s: String, fmt: String) -> DateTime
timestamp(dt) -> Float
fromtimestamp(ts: Float) -> DateTime
timedelta(...) -> Duration
```
```zen
import time
import datetime
print(time.time())                      # epoch seconds
time.sleep(500)                         # 0.5 s
let now = datetime.now()
print(datetime.strftime("%Y-%m-%d %H:%M:%S", now))
let parsed = datetime.strptime("2024-01-15", "%Y-%m-%d")
```

## J.7 pathlib / os — file system

pathlib:
```
Path(s) -> Path
p / q                     # join
p.exists() / p.is_file() / p.is_dir()
p.name / p.stem / p.suffix / p.parent
p.resolve() -> absolute Path
p.mkdir()
p.glob(pattern) -> list of Path
```
os:
```
getcwd() -> String
chdir(path)
listdir(path) -> list
makedirs(path)
remove(path) / rmdir(path)
rename(src, dst)
stat(path) -> file info
environ   # dict-like environment variables
```
```zen
import pathlib
import os
let p = pathlib.Path("data") / "config.txt"
print(p.exists(), p.name, p.suffix)
print(os.getcwd())
for f in pathlib.Path(".").glob("*.z") { print(f.name) }
```

## J.8 re — regular expressions

```
match(pattern, s) -> Match|nil     # anchor at start
search(pattern, s) -> Match|nil    # search anywhere
findall(pattern, s) -> list
sub(pattern, repl, s) -> String
split(pattern, s) -> list
compile(pattern) -> Pattern
```
```zen
import re
print(re.findall(r"\d+", "order 123, item 456"))   # ["123","456"]
print(re.sub(r"\s+", " ", "a   b    c"))           # "a b c"
```
Patterns: `\d` `\D` `\w` `\W` `\s` `\S` `.` `^` `$` `*` `+` `?` `{n,m}` `(...)`
`|` `\b`.

## J.9 base64 / binascii — encoding

base64: `b64encode`, `b64decode`, `url_encode`, `url_decode`.
binascii: `hexlify`, `unhexlify`.

```zen
import base64
import binascii
let enc = base64.b64encode("hello")   # aGVsbG8=
print(base64.b64decode(enc))          # hello
let hx = binascii.hexlify(bytes([72,101,108,108,111]))  # 48656c6c6f
print(binascii.unhexlify(hx))
```

## J.10 hashlib — cryptographic hashes

```
md5 / sha1 / sha224 / sha256 / sha384 / sha512
sha3_256 / sha3_512
blake2b / blake2s
pbkdf2_hmac(password, salt, iterations) -> String
```
Each hash function: `hashlib.<name>(data: String) -> String` (hex digest).
```zen
import hashlib
print(hashlib.sha256("hello"))   # 2cf24dba...
print(hashlib.md5("hello"))      # 5d41402abc...
```
⚠️ MD5 and SHA-1 are broken for security; use SHA-256/SHA-512/BLAKE2 for
integrity, and Argon2id/PBKDF2 for passwords.

## J.11 crypto — full cryptography

Hashing: `sha256/384/512`, `sha3_256/512`, `blake3`, `md5`, `sha1`.
HMAC: `sha256(key, data)`, `sha512(key, data)`.

Symmetric:
- `generate_key(alg="aes-256-gcm") -> String`
- `encrypt(alg, key, plaintext, aad="") -> String`
- `decrypt(alg, key, ciphertext, aad="") -> String`
- `encrypt_aes_gcm(key, plaintext, aad="") -> String`
- `decrypt_aes_gcm(key, ciphertext, aad="") -> String`
- `encrypt_chacha20_poly1305(key, plaintext, aad="") -> String`
- `decrypt_chacha20_poly1305(key, ciphertext, aad="") -> String`

Asymmetric:
- `generate_rsa(bits=2048) -> KeyPair`
- `rsa_encrypt(public_key, plaintext) -> String` / `rsa_decrypt(...)`
- `rsa_sign(private_key, message) -> String` / `rsa_verify(...) -> Bool`
- `generate_ed25519() -> KeyPair`
- `ed25519_sign(private_key, message) -> String` / `ed25519_verify(...) -> Bool`
- `generate_x25519() -> KeyPair`
- `x25519_shared_secret(private_key, public_key) -> String`

Encoding:
- `b64encode/decode`, `b64urlencode/decode`, `hex_encode/decode`

JWT:
- `sign(payload: Dict, key: String, algorithm="HS256") -> String`
- `verify(token, key, algorithms=["HS256"]) -> Dict`
- `decode(token) -> Dict`

KDF / passwords:
- `pbkdf2(password, salt, iterations=100000, length=32) -> String`
- `argon2id(password, salt="", memory=65536, iterations=3, parallelism=1, length=32) -> String`
- `argon2id_hash(password) -> String`
- `verify_argon2id(password, hash) -> Bool`
- `hkdf(ikm, salt="", info="", length=32) -> String`

OTP:
- `totp(secret, time_step=30, digits=6) -> String`
- `totp_verify(secret, token, window=1, time_step=30, digits=6) -> Bool`
- `hotp(secret, counter, digits=6) -> String`
- `hotp_verify(secret, token, counter, window=1, digits=6) -> Bool`
- `generate_secret() -> String`
- `bytes(n: Int) -> String` / `hex(n: Int) -> String` / `base64(n: Int) -> String`
- `uuid4() -> String` / `secure_token() -> String` / `int(min, max) -> Int`

X.509:
- `parse(pem: String) -> Certificate`

```zen
import crypto
let key = crypto.generate_key("aes-256-gcm")
let ct = crypto.encrypt_aes_gcm(key, "secret")
let pt = crypto.decrypt_aes_gcm(key, ct)
print(pt)   # secret

let pair = crypto.generate_ed25519()
let sig = crypto.ed25519_sign(pair.private_key, "msg")
print(crypto.ed25519_verify(pair.public_key, "msg", sig))   # true

let token = crypto.sign(#{sub: "alice"}, "my-secret")
print(crypto.verify(token, "my-secret")["sub"])   # alice

let ph = crypto.argon2id_hash("hunter2")
print(crypto.verify_argon2id("hunter2", ph))   # true
print(crypto.totp("JBSWY3DPEHPK3PXP"))         # 6-digit token
```

## J.12 socket — low-level networking

```
socket(family, type, proto=0) -> Socket
connect(address) / bind(address) / listen(backlog) / accept() -> (conn, addr)
send(data) / recv(size) -> data
sendto(data, address) / recvfrom(size) -> (data, addr)
close()
settimeout(seconds) / setblocking(bool)
setsockopt(level, option, value)
```
Constants: `AF_INET`, `AF_INET6`, `AF_UNIX`, `AF_PACKET`; `SOCK_STREAM`,
`SOCK_DGRAM`, `SOCK_RAW`; `IPPROTO_TCP`, `IPPROTO_UDP`, `IPPROTO_ICMP`,
`IPPROTO_IP`; options `SO_REUSEADDR`, `SO_KEEPALIVE`, `TCP_NODELAY`,
`SO_RCVBUF`, `SO_SNDBUF`, `IP_TTL`, `IP_HDRINCL`.

```zen
import socket
let s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.connect(("example.com", 80))
s.send(b"GET / HTTP/1.0\r\nHost: example.com\r\n\r\n")
print(s.recv(4096))
s.close()
```

## J.13 requests — high-level HTTP client

```
get(url, *, timeout, headers, json, params, verify, stream, allow_redirects) -> Response
post / put / patch / delete / head / options (same signature)
Session()   # connection reuse, default headers
```
Response: `status_code`, `reason`, `text`, `content` (bytes), `json()`, `headers`,
`cookies`, `url`, `elapsed`, `raise_for_status()`.

```zen
import requests
let r = requests.get("https://api.example.com/data", timeout=5000)
print(r.status_code)
print(r.json())
let s = requests.Session()
s.headers.update({"User-Agent": "ZenBot/1.0"})
```

## J.14 dns — domain name resolution

```
resolve(host, type="A") -> list of String   # A, AAAA, CNAME, MX, TXT, NS, PTR
reverse(ip) -> list of String
```
```zen
import dns
print(dns.resolve("example.com", "A"))   # ["93.184.216.34", ...]
print(dns.resolve("example.com", "MX"))
```

## J.15 url — URL parsing and manipulation

```
parse(url) -> #{scheme, host, port, path, query, fragment, ...}
build(scheme, host, path, query, ...) -> String
join(base, relative) -> String
quote(s) / unquote(s)            # percent-encode/decode
quote_plus(s) / unquote_plus(s)
parse_qs(query) -> dict of lists
```
```zen
import url
let u = url.parse("https://example.com:8080/path?q=1#sec")
print(u.scheme, u.host, u.port, u.path, u.query, u.fragment)
print(url.quote("hello world"))   # hello%20world
```

## J.16 threading / channels / mutexes — concurrency

threading:
```
Thread(fn) -> Thread
t.start() / t.join() / t.daemon = bool
```
channels:
```
Channel(buffered=false, capacity=0) -> Channel
ch.send(value) / ch.recv() -> value / ch.close()
```
mutexes:
```
Mutex() -> Mutex
m.lock() / m.unlock()
```

```zen
import threading, channels, mutexes
let ch = channels.Channel()
let mu = mutexes.Mutex()
let t = threading.Thread(fn() { ch.send(42); ch.close() })
t.start()
print(ch.recv())   # 42
t.join()
```

## J.17 ffi — Foreign Function Interface (C)

```
load_library(path) -> LibraryHandle
get_symbol(handle, name) -> Callable
call(func, args) -> Value
close(handle)
```
Types: Pointer, Handle. See Chapter 40 for usage and safety rules.

## J.18 sys — system information

```
argv -> list of String       # command-line args
platform -> String           # "linux", "darwin", "windows"
version -> String
exit(code: Int)
environ -> dict-like         # environment variables
path -> list                 # module search path
modules -> dict              # loaded modules
```
```zen
import sys
print(sys.argv)
print(sys.platform)
print(sys.environ["PATH"] ?? "")
```

## J.19 pymport — Python interoperability

```
pymport.import(module_name) -> Module
```
Import Python modules (stdlib, venv, project-local `.py`), marshal values, and
chain Python tracebacks under Zen frames (Z0508).

```zen
import pymport
let m = pymport.import("math")
print(m.sqrt(16))   # 4.0
```

## J.20 logging — logging facility

Levels: `DEBUG`, `INFO`, `WARNING`, `ERROR`, `CRITICAL`, `NOTSET`.

## J.21 functools — higher-order helpers

```
reduce(func, seq, init) / partial(func, arg) / wraps(func)
lru_cache(func) / cmp_to_key(func) / total_ordering(cls)
```

## J.22 itertools — iteration tools

```
enumerate(seq) / zip(*seqs) / chain(*seqs)
repeat(value) / product(*seqs) / permutations(seq, r)
combinations(seq, r) / accumulate(seq) / take(n, seq) / drop(n, seq) / range(...)
```

## J.23 statistics — statistical functions

```
mean(seq) / fmean(seq) / geometric_mean(seq) / harmonic_mean(seq)
median(seq) / median_low(seq) / median_high(seq) / median_grouped(seq)
mode(seq) / multimode(seq)
pvariance(seq) / variance(seq) / pstdev(seq) / stdev(seq)
quantiles(seq) / linear_regression(x, y)
```

## J.24 operator — functions for operators

```
add / sub / mul / truediv / floordiv / mod / divmod / pow
neg / pos / abs / inv / invert
lshift / rshift / and_ / or_ / xor
truth / not_ / is_ / is_not
eq / ne / lt / le / gt / ge
contains / countOf / indexOf / length_hint
getitem / setitem / getslice / concat / repeat
itemgetter(name) / attrgetter(name) / methodcaller(name)
```

## J.25 struct / tempfile / uuid / glob / shutil / crunch / nmap / ffmpeg

```
struct.calcsize(fmt) -> Int        # binary struct size
tempfile.dir / tempfile.mkdtemp() / tempfile.mkstemp()
uuid.UUID / uuid4() / uuid3() / uuid5() / uuid1() / uuid1_fields()
uuid.NAMESPACE_DNS / NAMESPACE_URL / NAMESPACE_OID / NAMESPACE_X500
glob.glob(pattern) / glob.iglob(pattern) / glob.escape(s)
shutil.copy / copy2 / copyfile / copytree / move / rmtree / which
crunch.generate(...)               # random data generation
nmap.*                             # network scanning helpers
ffmpeg.*                           # media processing helpers
```

## J.26 Module → chapter quick map

| Chapter | Module |
|---------|--------|
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
| 27 | socket |
| 28 | http |
| 29 | requests |
| 30 | http.server |
| 32 | dns |
| 33 | url |
| 34 | raw sockets |
| 35 | threading |
| 36 | channels |
| 37 | mutexes |
| 39 | ffi |
| 41 | sys |
| 42 | pymport |

## J.27 Exercises

1. For each of math/random/string/json/hashlib/crypto, name its three most-used functions.
2. Write a one-liner summarizing what socket, requests, dns, and url each do.
3. Which module would you use for: parsing JSON? hashing a file? making an HTTP GET? resolving a DNS name? generating a secure token?
4. Name two modules you must NOT use for cryptographic secrets — and which to use instead.
5. Build your own cheat sheet: module → most-used 3 functions → one example each.
