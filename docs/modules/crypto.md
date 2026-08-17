# Cryptography Module

Complete reference for hashing, HMAC, AES encryption, PBKDF2, and random byte generation in Zen.

## Hashing

### Available hash algorithms

| Function | Algorithm | Output Length (hex) |
|----------|-----------|-------------------|
| `crypto.md5(data)` | MD5 | 32 |
| `crypto.sha1(data)` | SHA-1 | 40 |
| `crypto.sha224(data)` | SHA-224 | 56 |
| `crypto.sha256(data)` | SHA-256 | 64 |
| `crypto.sha384(data)` | SHA-384 | 96 |
| `crypto.sha512(data)` | SHA-512 | 128 |
| `crypto.sha3_256(data)` | SHA3-256 | 64 |
| `crypto.sha3_512(data)` | SHA3-512 | 128 |
| `crypto.blake2b(data)` | BLAKE2b | 128 |
| `crypto.blake2s(data)` | BLAKE2s | 64 |

### Basic hashing

```
print crypto.md5("hello")
// 5d41402abc4b2a76b9719d911017c592

print crypto.sha256("hello")
// 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824

print crypto.sha512("hello")
// 9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca7...
```

### Hashing binary-like data

Hash functions accept strings. For binary data, convert first:

```
let data = "binary content"
print crypto.sha256(data)
```

### Comparing hashes

```
let stored_hash = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
let input_hash = crypto.sha256("hello")

if stored_hash == input_hash {
    print "Password correct!"
} else {
    print "Wrong password"
}
```

---

## HMAC (Hash-based Message Authentication)

HMAC combines a secret key with a hash function for authenticated hashing.

| Function | Algorithm |
|----------|-----------|
| `crypto.hmac_md5(key, data)` | HMAC-MD5 |
| `crypto.hmac_sha1(key, data)` | HMAC-SHA1 |
| `crypto.hmac_sha256(key, data)` | HMAC-SHA256 |

### Basic HMAC

```
let key = "my-secret-key"
let message = "Hello, World!"

print crypto.hmac_sha256(key, message)
// hex-encoded HMAC-SHA256
```

### API signature verification

```
let api_secret = os.env("API_SECRET")
let payload = '{"action": "transfer", "amount": 100}'

let expected_sig = crypto.hmac_sha256(api_secret, payload)

// Verify incoming request
let received_sig = request.headers["X-Signature"]
if received_sig == expected_sig {
    print "Signature valid"
    // process request
} else {
    print "Invalid signature"
    throw "Unauthorized"
}
```

### Comparing HMACs (timing-safe)

```
let key = os.env("WEBHOOK_SECRET")
let payload = request.body
let expected = crypto.hmac_sha256(key, payload)
let received = request.headers["X-Hub-Signature-256"]

// Use simple comparison (Zen doesn't have constant-time compare)
if expected == received {
    print "Webhook verified"
}
```

---

## PBKDF2 (Password-Based Key Derivation)

Derives a cryptographic key from a password.

### Basic usage

```
// Default: 100000 iterations, 32 bytes output
let key = crypto.pbkdf2("password", "salt")
print key    // hex-encoded key
```

### Custom iterations and length

```
// Custom iterations and output length
let key = crypto.pbkdf2("password", "salt", 10000, 16)
print key    // 32 hex characters (16 bytes)
```

### Password hashing pattern

```
function hash_password(password) {
    let salt = crypto.random_hex(16)
    let key = crypto.pbkdf2(password, salt, 100000, 32)
    return salt + ":" + key
}

function verify_password(password, stored) {
    let parts = stored.split(":")
    let salt = parts[0]
    let expected = parts[1]
    let key = crypto.pbkdf2(password, salt, 100000, 32)
    return key == expected
}

// Usage
let stored = hash_password("my-secret")
print verify_password("my-secret", stored)     // true
print verify_password("wrong-password", stored) // false
```

### Secure defaults

| Parameter | Recommended Minimum | Notes |
|-----------|-------------------|-------|
| Iterations | 100,000 | Higher = slower but more secure |
| Key length | 32 bytes | 256 bits for AES-256 |
| Salt | Random, 16+ bytes | Never reuse salts |

---

## Random Bytes

### Generate random bytes

```
let bytes = crypto.random_bytes(16)
print bytes    // 32 hex characters (16 bytes)
```

### Generate random hex string

```
let hex = crypto.random_hex(8)
print hex      // 16 hex characters (8 bytes)
```

### Use cases

```
// Session token
let token = crypto.random_hex(32)
print "Session token: {token}"

// API key
let api_key = "zen_" + crypto.random_hex(24)
print "API key: {api_key}"

// Salt for password hashing
let salt = crypto.random_hex(16)

// IV for AES encryption
let iv = crypto.random_bytes(16)
```

---

## AES Encryption

Symmetric encryption using AES-256-CBC.

### Encrypt

```
let key = crypto.random_bytes(32)    // 256-bit key
let data = "Sensitive data"

let encrypted = crypto.aes_encrypt(key, data)
print encrypted    // hex string (IV + ciphertext)
```

### Decrypt

```
let decrypted = crypto.aes_decrypt(key, encrypted)
print decrypted    // Sensitive data
```

### With explicit IV

```
let key = crypto.random_bytes(32)
let iv = crypto.random_bytes(16)

let encrypted = crypto.aes_encrypt(key, "Secret", iv)
let decrypted = crypto.aes_decrypt(key, encrypted, iv)
print decrypted    // Secret
```

### Complete encryption workflow

```
// Key generation (do this once, store securely)
let key = crypto.random_hex(32)

// Encrypt
let plaintext = "This is sensitive information"
let ciphertext = crypto.aes_encrypt(key, plaintext)
print "Encrypted: {ciphertext}"

// Save to file
fs.write("secret.enc", ciphertext)

// Load and decrypt
let loaded = fs.read("secret.enc")
let decrypted = crypto.aes_decrypt(key, loaded)
print "Decrypted: {decrypted}"

// Verify
print decrypted == plaintext    // true
```

### Encrypting JSON data

```
let key = crypto.random_hex(32)

let sensitive_data = {
    "credit_card": "4111-1111-1111-1111",
    "expiry": "12/25",
    "cvv": "123"
}

let json_str = json.encode(sensitive_data)
let encrypted = crypto.aes_encrypt(key, json_str)

// Store encrypted
fs.write("payment.enc", encrypted)

// Later: decrypt and parse
let loaded = fs.read("payment.enc")
let decrypted = crypto.aes_decrypt(key, loaded)
let data = json.parse(decrypted)
print data.credit_card    // 4111-1111-1111-1111
```

---

## Fernet Encryption (cryptography module)

Higher-level authenticated encryption using the Fernet standard.

### Generate a key

```
let key = cryptography.fernet.generate_key()
print key    // URL-safe base64-encoded key
```

### Encrypt

```
let key = cryptography.fernet.generate_key()
let token = cryptography.fernet.encrypt(key, "Secret message")
print token    // encrypted token
```

### Decrypt

```
let plaintext = cryptography.fernet.decrypt(key, token)
print plaintext    // Secret message
```

### Complete Fernet workflow

```
// Generate and store key
let key = cryptography.fernet.generate_key()
fs.write("fernet.key", key)

// Encrypt
let data = "Top secret information"
let token = cryptography.fernet.encrypt(key, data)
fs.write("encrypted.dat", token)

// Decrypt
let stored_key = fs.read("fernet.key")
let stored_token = fs.read("encrypted.dat")
let plaintext = cryptography.fernet.decrypt(stored_key, stored_token)
print plaintext    // Top secret information
```

### Fernet vs AES

| Feature | Fernet | AES |
|---------|--------|-----|
| Key management | Simple (single key) | Manual (key + IV) |
| Authentication | Built-in (HMAC) | None (need to add) |
| Timestamp | Built-in | None |
| Best for | Simple encryption | Custom protocols |

---

## Security Notes

### Never hardcode keys

```
// BAD
let key = "my-secret-key-1234567890123456"

// GOOD — generate randomly
let key = crypto.random_hex(32)

// BETTER — load from environment
let key = os.env("ENCRYPTION_KEY")
```

### Use strong algorithms

```
// GOOD — SHA-256, SHA-512, SHA3
let hash = crypto.sha256(data)

// ACCEPTABLE — SHA-1 for non-security purposes
let hash = crypto.sha1(data)

// AVOID — MD5 for security (collision-prone)
let hash = crypto.md5(data)
```

### Salts for password hashing

```
// BAD — no salt
let hash = crypto.sha256(password)

// GOOD — with random salt
let salt = crypto.random_hex(16)
let hash = crypto.pbkdf2(password, salt, 100000, 32)
```

### Key length

| Algorithm | Minimum Key Length |
|-----------|-------------------|
| AES | 32 bytes (256-bit) |
| HMAC-SHA256 | 32 bytes |
| PBKDF2 | 32 bytes output |
| Fernet | 32 bytes (generated for you) |

---

## Pro Tips

1. **Use SHA-256 as default.** It's fast, secure, and widely supported.
2. **Use PBKDF2 for passwords.** It's slow by design (brute-force resistant).
3. **Use Fernet for simple encryption.** It handles key management and authentication.
4. **Generate keys with `crypto.random_hex()`.** Never use weak or predictable keys.
5. **Store keys separately from encrypted data.** Use environment variables or a key vault.

---

## Common Mistakes

### Using MD5 for security

```
// BAD — MD5 is fast and has known collisions
let hash = crypto.md5(password)

// GOOD — use PBKDF2 for passwords
let salt = crypto.random_hex(16)
let hash = crypto.pbkdf2(password, salt, 100000, 32)
```

### Reusing IVs with AES

```
// BAD — same IV for same key
let encrypted1 = crypto.aes_encrypt(key, "data1")
let encrypted2 = crypto.aes_encrypt(key, "data2")

// GOOD — generate fresh IV each time
let iv1 = crypto.random_bytes(16)
let iv2 = crypto.random_bytes(16)
let encrypted1 = crypto.aes_encrypt(key, "data1", iv1)
let encrypted2 = crypto.aes_encrypt(key, "data2", iv2)
```

### Not handling decryption errors

```
// BAD — crashes on wrong key
let data = crypto.aes_decrypt(wrong_key, ciphertext)

// GOOD — handle errors
let data = try crypto.aes_decrypt(key, ciphertext) catch err {
    print "Decryption failed: " + err
    null
}
```

---

## See Also

- [Base64 Module](overview.md#base64) — Encoding for encrypted data
- [os Module](overview.md) — Environment variables for keys
- [json Module](json.md) — Encoding data before encryption
- [Module Overview](overview.md) — All available modules
