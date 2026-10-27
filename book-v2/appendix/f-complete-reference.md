# Appendix F: Zen 3.0 Complete Language Reference

An exhaustive, compact reference of every builtin, keyword, operator, flag,
environment variable, error code, and standard library member. Use this
appendix when you need to look something up quickly.

## F.1 Keywords and reserved words

```
let const var global shared
func def function fn lambda
if elif else while for in break continue return
try catch except finally throw
import from as class new extends inherit super
pub match when
true false nil null
is typeof and or not is not
```

## F.2 Literal forms

| Type | Form | Example |
|------|------|---------|
| nil | `nil` / `null` | `let x = nil` |
| bool | `true` / `false` | `let ok = true` |
| int decimal | digits, `_` allowed | `1_000_000` |
| int hex | `0x` / `0X` | `0xFF` |
| int binary | `0b` / `0B` | `0b1010` |
| int octal | `0o` / `0O` | `0o755` |
| float | digits `.` digits, exponent | `3.14`, `1.5e-10` |
| string double | `"...${expr}..."` (interpolated) | `"hi ${name}"` |
| string single | `'...'` (no interpolation) | `'raw'` |
| string triple | `"""..."""` / `'''...'''` | multiline |
| string backtick | `` `...` `` (raw, no interpolation) | `` `C:\path` `` |
| bytes | `bytes([1,2,0xFF])` | `bytes([0x41])` |
| list | `[a, b, c]` | `[1,2,3]` |
| dict | `#{k: v}` | `#{a:1}` |
| set | `%{a, b}` | `%{1,2}` |
| tuple | `#(a, b)` | `#(1,"x")` |

## F.3 Operator precedence (high → low)

| Level | Operators | Associativity |
|-------|-----------|---------------|
| 15 | `()` `[]` `.` `?.` | left |
| 14 | `**` | right |
| 13 | unary `-` `!` `not` `~` `typeof` | right |
| 12 | `*` `/` `%` | left |
| 11 | `+` `-` | left |
| 10 | `<<` `>>` | left |
| 9 | `&` | left |
| 8 | `^` | left |
| 7 | `|` | left |
| 6 | `is` `is not` `===` `!==` `==` `!=` `<` `>` `<=` `>=` `in` `not in` | left |
| 5 | `and` `&&` | left |
| 4 | `or` `||` | left |
| 3 | `??` | left |
| 2 | `?:` / `if expr else expr` | right |
| 1 | `=` `+=` `-=` ... | right |

## F.4 Truthiness

Falsy: `false`, `nil`, `null`.
Truthy: everything else, including `0`, `0.0`, `""`, `[]`, `#{}`, `%{}`, `#()`.

## F.5 Type coercion matrix

| From | int | float | string | bool |
|------|-----|-------|--------|------|
| int | identity | widen | decimal digits | 0→false, else true |
| float | truncate | identity | repr | 0.0→false, else true |
| string | parse (or error) | parse (or error) | identity | empty→false? (Zen: true) |
| bool | true→1, false→0 | true→1.0, false→0.0 | "true"/"false" | identity |
| nil | error | error | "nil" | false |
| list/dict/set/tuple | error | error | repr | empty→true in Zen |

## F.6 Built-in functions (complete)

| Function | Purpose |
|----------|---------|
| `print` | print to stdout |
| `str` | to string |
| `int` | to int |
| `float` | to float |
| `bool` | to bool |
| `list` | to list |
| `tuple` | to tuple |
| `set` | to set |
| `dict` | to dict |
| `len` | length |
| `range` | range |
| `enumerate` | index+value |
| `zip` | aggregate |
| `sorted` | sorted list |
| `reversed` | reversed |
| `any` | any true |
| `all` | all true |
| `sum` | sum |
| `min` / `max` | min/max |
| `abs` | absolute value |
| `typeof` / `type` | type name |
| `chr` | int → char |
| `ord` | char → int |
| `open` | open file |
| `exit` | exit process |
| `assert` | assert condition |
| `repr` | developer representation |

## F.7 Statement forms

| Form | Meaning |
|------|---------|
| `let x = e` | immutable binding |
| `var x = e` | mutable binding |
| `const x = e` | constant |
| `shared x = e` | thread-shared mutable |
| `x = e` | assignment |
| `x += e` etc | compound assignment |
| `x++` / `x--` | increment/decrement |
| `if c { }` | conditional |
| `while c { }` | loop |
| `for x in i { }` | iterate |
| `break` / `continue` | loop control |
| `return [e]` | return from function |
| `throw e` | raise error |
| `try { } catch [T] as e { } finally { }` | error handling |
| `match v { p => e, ... }` | pattern match |
| `when { c => e, ... }` | cond chain |
| `import m` / `from m import n` | imports |

## F.8 Zen CLI commands

| Command | Purpose |
|---------|---------|
| `zen` / `zen repl` | REPL |
| `zen run <file>` | execute script |
| `zen -m <module>` | run module as main |
| `zen -e <code>` | inline code |
| `zen check <file>` | type-check only |
| `zen lint <file>` | static analysis |
| `zen explain <code>` | explain error code |
| `zen build <file>` | bundle to executable |
| `zen test` | run tests |
| `zen fmt` | format sources |
| `zen help` | help |
| `zen --version` | version |

Exit codes: 0 success, 1 general, 2 usage, 127 not found, 130 signal.

## F.9 Environment variables

| Variable | Purpose |
|----------|---------|
| `ZEN_PATH` | module search paths (colon-separated) |
| `ZEN_DEBUG` | enable debug logging |
| `PYTHONPATH` | Python module resolution for pymport |
| `VIRTUAL_ENV` | Python venv for pymport |

## F.10 Error codes (exhaustive Z01xx–Z05xx)

### Z01xx Lexer
- Z0101 InvalidCharacter
- Z0102 UnterminatedString
- Z0103 InvalidNumber
- Z0104 InvalidEscape
- Z0105 InvalidIdentifier
- Z0106 NumericLiteralOverflow
- Z0107 MultiDotRejected
- Z0108 BadComment

### Z02xx Parser
- Z0201 UnexpectedToken
- Z0202 MissingSemicolon
- Z0203 UnbalancedBraces
- Z0204 UnexpectedEOF
- Z0205 InvalidStatement
- Z0206 DuplicateParameter
- Z0207 InvalidDefault
- Z0208 InvalidMatchArm
- Z0209 InvalidImport
- Z0210 Redefinition

### Z03xx Runtime
- Z0301 TypeError
- Z0302 IndexError
- Z0303 KeyError
- Z0304 DivideByZero
- Z0305 ValueError
- Z0306 AttributeError
- Z0307 NotCallable
- Z0308 ArgumentCount
- Z0309 UnboundVariable
- Z0310 MaxRecursionDepth
- Z0311 OverflowError
- Z0312 DomainError
- Z0313 RangeError
- Z0314 EncodingError
- Z0315 ImmutableError
- Z0316 ConcurrentModification
- Z0317 StackOverflow

### Z04xx I/O / system
- Z0401 FileNotFound
- Z0402 PermissionDenied
- Z0403 FileExists
- Z0404 IsADirectory
- Z0405 NotADirectory
- Z0406 DirectoryNotEmpty
- Z0407 BrokenPipe
- Z0408 ConnectionRefused
- Z0409 ConnectionReset
- Z0410 Timeout
- Z0411 NetworkUnreachable
- Z0412 HostUnreachable
- Z0413 AddressInUse
- Z0414 DiskFull
- Z0415 TooManyOpenFiles
- Z0416 InvalidArgument
- Z0417 SignalInterrupted

### Z05xx FFI / interop
- Z0501 SymbolNotFound
- Z0502 HandleInvalid
- Z0503 CallingConvention
- Z0504 TypeMismatch
- Z0505 MemoryAccess
- Z0506 PymportNotFound
- Z0507 PymportMarshal
- Z0508 PythonError
- Z0509 GILContention
- Z0510 ForeignTypeUnsupported

Run `zen explain Z####` for details on any code.

## F.11 Standard library — module → principal members

math: `pi e tau inf nan abs sqrt pow exp log log2 log10 floor ceil trunc round
sin cos tan asin acos atan atan2 degrees radians gcd lcm factorial comb perm
remainder copysign min max fsum prod hypot isnan isfinite isinf modf frexp ldexp`

random: `seed random randint randrange choice choices sample shuffle uniform hex`

string: `upper lower title capitalize swapcase strip lstrip rstrip split
splitlines join replace count find rfind startswith endswith contains ljust
rjust center zfill isdigit isalpha isalnum isspace islower isupper`
constants: `digits hexdigits octdigits ascii_letters ascii_lowercase
ascii_uppercase punctuation whitespace printable`

collections: `Counter chain flatten`

json: `parse stringify encode pretty load save`

time/datetime: `time sleep now today strftime strptime timestamp fromtimestamp timedelta`

pathlib: `Path / exists is_file is_dir name stem suffix parent resolve mkdir glob`
os: `getcwd chdir listdir makedirs remove rmdir rename stat environ`

re: `match search findall sub split compile`

base64: `b64encode b64decode url_encode url_decode`
binascii: `hexlify unhexlify`

hashlib: `md5 sha1 sha224 sha256 sha384 sha512 sha3_256 sha3_512 blake2b blake2s pbkdf2_hmac`

crypto: `sha256/384/512 sha3_256/512 blake3 md5 sha1 sha256(key,data) sha512(key,data)
generate_key encrypt decrypt encrypt_aes_gcm decrypt_aes_gcm
encrypt_chacha20_poly1305 decrypt_chacha20_poly1305 generate_rsa rsa_encrypt
rsa_decrypt rsa_sign rsa_verify generate_ed25519 ed25519_sign ed25519_verify
generate_x25519 x25519_shared_secret b64encode b64decode b64urlencode b64urldecode
hex_encode hex_decode sign verify decode pbkdf2 argon2id argon2id_hash
verify_argon2id hkdf totp totp_verify hotp hotp_verify generate_secret bytes
hex base64 uuid4 secure_token int parse`

socket: `socket connect bind listen accept send recv sendto recvfrom close
setsockopt settimeout setblocking SO_REUSEADDR SO_KEEPALIVE TCP_NODELAY
SO_RCVBUF SO_SNDBUF IP_TTL IP_HDRINCL AF_INET AF_INET6 AF_UNIX AF_PACKET
SOCK_STREAM SOCK_DGRAM SOCK_RAW IPPROTO_TCP IPPROTO_UDP IPPROTO_ICMP IPPROTO_IP`

http/requests: `get post put patch delete head options Session raise_for_status`

http.server: request handler routing static files middleware

dns: `resolve reverse` records A AAAA CNAME MX TXT NS PTR

url: `parse build join quote unquote parse_qs`

threading: `Thread start join daemon`
channels: `Channel send recv close buffered capacity`
mutexes: `Mutex lock unlock`

ffi: `load_library get_symbol call close Pointer Handle`
sys: `argv platform version exit environ path modules`
pymport: `import` Python modules with marshalling and chained tracebacks

logging: `DEBUG INFO WARNING ERROR CRITICAL NOTSET`
functools: `reduce partial wraps lru_cache cmp_to_key`
itertools: `enumerate zip chain repeat product permutations combinations accumulate take drop range`
statistics: `mean median mode multimode variance stdev pvariance pstdev quantiles linear_regression`
operator: `add sub mul truediv floordiv mod divmod pow neg pos abs inv invert
lshift rshift and_ or_ xor truth is_ is_not eq ne lt le gt ge contains
itemgetter attrgetter methodcaller`
struct: `calcsize`
tempfile: `dir mkdtemp mkstemp`
uuid: `NAMESPACE_DNS NAMESPACE_URL NAMESPACE_OID NAMESPACE_X500 UUID uuid4 uuid3
uuid5 uuid1 uuid1_fields`
glob: `glob iglob escape`
shutil: `copy copy2 copyfile copytree move rmtree which`
crunch: `generate`
nmap: scanning helpers
ffmpeg: media helpers

## F.12 Socket option reference

| Option | Level | Purpose |
|--------|-------|---------|
| SO_REUSEADDR | SOL_SOCKET | reuse local address |
| SO_REUSEPORT | SOL_SOCKET | multiple binds to same port |
| SO_KEEPALIVE | SOL_SOCKET | TCP keepalive probes |
| SO_RCVBUF | SOL_SOCKET | receive buffer size |
| SO_SNDBUF | SOL_SOCKET | send buffer size |
| SO_TIMEOUT | SOL_SOCKET | operation timeout |
| TCP_NODELAY | IPPROTO_TCP | disable Nagle's algorithm |
| IP_TTL | IPPROTO_IP | time-to-live |
| IP_HDRINCL | IPPROTO_IP | include IP header (raw) |

## F.13 Common ports

20/21 FTP, 22 SSH, 23 Telnet, 25 SMTP, 53 DNS, 67/68 DHCP, 80 HTTP,
110 POP3, 143 IMAP, 443 HTTPS, 993 IMAPS, 995 POP3S.

## F.14 HTTP status codes

1xx Informational | 2xx Success (200 201 202 204 206) |
3xx Redirection (301 302 303 304 307 308) |
4xx Client Error (400 401 403 404 405 406 409 410 413 414 415 422 429) |
5xx Server Error (500 501 502 503 504 505)

## F.15 HTTP methods

| Method | Safe | Idempotent | Body |
|--------|------|------------|------|
| GET | yes | yes | no |
| HEAD | yes | yes | no |
| OPTIONS | yes | yes | optional |
| POST | no | no | yes |
| PUT | no | yes | yes |
| PATCH | no | no | yes |
| DELETE | no | yes | optional |
| TRACE | yes | yes | no |
| CONNECT | no | no | no |

## F.16 DNS record types

A (IPv4), AAAA (IPv6), CNAME (alias), MX (mail), TXT (text), NS (nameserver),
PTR (reverse IP→name).

## F.17 URL components

scheme :// [user:pass@] host [:port] /path [?query] [#fragment]

## F.18 IPv4 header (20 bytes min)

ver/IHL DSCP/ECN | total length | identification | flags/frag | TTL | proto | checksum | src IP (4) | dst IP (4)

## F.19 TCP connection state machine

CLOSED → SYN_SENT → SYN_RECEIVED → ESTABLISHED → FIN_WAIT_1 → FIN_WAIT_2 →
TIME_WAIT → CLOSED (normal close); also RST transitions at any error.

## F.20 Coercion / truthiness quick examples

```zen
print(1 == 1.0)     # true  (numeric promotion)
print(1 === 1.0)    # false (strict: different types)
print(bool(0))      # false  (0 is falsy in Zen)
print(bool(""))     # false  (empty string falsy)
print(bool(nil))    # false
print(nil ?? "d")   # "d"
print(0 ?? "d")     # 0     (0 is not nil, so ?? keeps it)
```

## F.21 Exercises (reference drill)

1. Look up `zen explain Z0304` and summarize the fix.
2. Name ten falsy values in Zen (e.g. nil, false, 0, "", []...).
3. List the HTTP methods that are idempotent.
4. Name the socket option to disable Nagle's algorithm.
5. List the DNS record types.
6. Name the exit code for "command not found" (127).
7. Name the environment variable for module search paths.
8. Name a value that is truthy in Zen but falsy in some other languages (or vice versa).
9. Name the function to parse JSON: `json.parse`.
10. Name the crypto function for time-based one-time passwords: `crypto.totp`.

'''
print('appendix f done')