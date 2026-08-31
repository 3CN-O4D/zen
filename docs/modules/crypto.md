# crypto — Cryptographic hashes and utilities

The `crypto` module provides a suite of low-level cryptographic functions, including hashing, HMAC, and symmetric encryption. It is available globally as `crypto`.

```zen
# 1. Simple SHA256 hash
print(crypto.sha256("hello"))

# 2. HMAC with SHA256
var key = "secret-key"
print(crypto.hmac_sha256(key, "data"))

# 3. Random bytes
var iv = crypto.random_bytes(16)
```

## Hashing Functions

All hashing functions return the digest as a hex-encoded string.

| Function | Description |
|----------|-------------|
| `sha256(data)` | SHA-256 hash. |
| `sha512(data)` | SHA-512 hash. |
| `sha1(data)` | SHA-1 hash. |
| `md5(data)` | MD5 hash. |
| `sha224`, `sha384` | Variants of SHA-2. |
| `sha3_256`, `sha3_512` | SHA-3 variants. |
| `blake2b`, `blake2s` | BLAKE2 variants. |

## HMAC Functions

HMAC functions require a key and a message string.

- `crypto.hmac_sha256(key, message)`
- `crypto.hmac_sha1(key, message)`
- `crypto.hmac_md5(key, message)`

## Encryption & Utilities

| Function | Description |
|----------|-------------|
| `random_bytes(n)` | Returns `n` random bytes as a string. |
| `random_hex(n)` | Returns a random hex string of length `n`. |
| `pbkdf2(pass, salt, iter, len)` | Password-based key derivation. |
| `aes_encrypt(key, iv, data)` | Raw AES-256-CBC encryption. |
| `aes_decrypt(key, iv, token)` | Raw AES-256-CBC decryption. |

## See Also
- [hashlib](hashlib.md) — Higher-level hashing API.
- [cryptography](cryptography.md) — Simple Fernet symmetric encryption.
