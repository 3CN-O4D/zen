# Cryptographic Modules (`crypto`, `cryptography`)

## `crypto` Module

The `crypto` module provides fundamental symmetric and hash functions for secure data processing, checksums, and encryption.

### Hash Functions (All return hex-encoded strings)

| Function | Input | Example |
|----------|-------|---------|
| `crypto.sha256(text)` | String | `crypto.sha256("hello")` → `"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"` |
| `crypto.sha1(text)` | String | `crypto.sha1("hello")` → `"aaf4c6f0d2e4b1651588c2c58f6e65d3"` |
| `crypto.md5(text)` | String | `crypto.md5("hello")` → `"5d41402abc4b2a76b9719d911017c592"` |
| `crypto.sha512(text)` | String | `crypto.sha512("hello")` → `"185f8db32..."` |
| `crypto.sha224(text)` | String | `crypto.sha224("hello")` → `"22f0e5b..."` |
| `crypto.sha384(text)` | String | `crypto.sha384("hello")` → `"..."` |
| `crypto.sha3_256(text)` | String | `crypto.sha3_256("hello")` → `"..."` |
| `crypto.sha3_512(text)` | String | `crypto.sha3_512("hello")` → `"..."` |
| `crypto.blake2b(text)` | String | `crypto.blake2b("hello")` → `"..."` |
| `crypto.blake2s(text)` | String | `crypto.blake2s("hello")` → `"..."` |

### HMAC Functions

| Function | Key | Message | Example |
|----------|-----|---------|---------|
| `crypto.hmac_sha256(key, message)` | String / bytes | String / bytes | `crypto.hmac_sha256("key", "msg")` → `"..."` |
| `crypto.hmac_sha1(key, message)` | String / bytes | String / bytes | |
| `crypto.hmac_md5(key, message)` | String / bytes | String / bytes | |

### Symmetric Encryption (AES-256-CBC with PKCS7 padding)

| Function | Parameters | Example |
|----------|------------|---------|
| `crypto.aes_encrypt(key, plaintext)` | 32-byte key, string plaintext | `crypto.aes_encrypt("mysecretkey1234567890123456789012", "Hello")` → base64 string |
| `crypto.aes_decrypt(key, ciphertext_b64)` | 32-byte key, base64 ciphertext | `crypto.aes_decrypt("mysecretkey1234567890123456789012", base64_cipher)` → plaintext |

### Random Utilities

| Function | Parameters | Example |
|----------|------------|---------|
| `crypto.random_bytes(n)` | Integer n | Generates n random bytes as hex string |
| `crypto.random_hex(n)` | Integer n | Generates a random hex string of n bytes |
| `crypto.pbkdf2(password, salt, iterations, keylen)` | Strings + ints | Derives a key suitable for AES |

### Examples
```zen
let hash = crypto.sha256("test data")
print hash

let mac = crypto.hmac_sha256("my-secret-key", "message to authenticate")
print mac

let encrypted = crypto.aes_encrypt("0123456789abcdef0123456789abcdef", "Secret message")
let decrypted = crypto.aes_decrypt("0123456789abcdef0123456789abcdef", encrypted)
print decrypted
```

---

## `cryptography` Module (Fernet Symmetric Encryption)

The `cryptography` module provides Fernet symmetric encryption, which uses AES-128-CBC with PKCS7 padding and HMAC for authentication, plus Base64 encoding. Fernet guarantees that messages are encrypted and can only be decrypted by someone possessing the key.

### Key Generation

| Function | Example |
|----------|---------|
| `cryptography.generate_key()` | Generates a fresh 32-byte key as a base64 string |

```zen
let key = cryptography.generate_key()
```

### Encryption

Encrypts data using the provided key, returning a Fernet token (base64 string).

```zen
function encrypt_data() {
    let key = cryptography.generate_key()
    let data = "This is a secret message"
    let token = cryptography.encrypt(key, data)
    print token  // Fernet base64 token
}
```

### Decryption

Decrypts a Fernet token back to the original plaintext.

```zen
function decrypt_data() {
    let key = cryptography.generate_key()
    let data = "This is a secret message"
    let token = cryptography.encrypt(key, data)
    let decrypted = cryptography.decrypt(key, token)
    print decrypted  // "This is a secret message"
}
```

### Security Notes
* The encryption uses AES-128-CBC with PKCS7 padding.
* HMAC-SHA256 is used for authentication.
* Each encrypted message includes its own unique IV (initialization vector), so the same plaintext encrypted twice produces different ciphertexts.
* Ensure the key is kept secret and rotated periodically.