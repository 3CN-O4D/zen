# Chapter 25: crypto Module — Complete Guide

The `crypto` module provides symmetric encryption, asymmetric keys, hashing, HMAC, key derivation, JWT, OTP, and X.509. This chapter covers the complete crypto API.

> ⚠️ Warning: Cryptography is easy to get wrong. Use well-established algorithms and modes. Never roll your own crypto for production security.

## 25.1 Symmetric encryption

```zen
import crypto

# Generate a key
let key = crypto.generate_key("aes-256-gcm")

# Encrypt
let ciphertext = crypto.encrypt_aes_gcm(key, "secret message")

# Decrypt
let plaintext = crypto.decrypt_aes_gcm(key, ciphertext)
print(plaintext)  # secret message
```

### Symmetric functions

| Function | Description |
|----------|-------------|
| `generate_key(alg?)` | Generate a key ("aes-256-gcm" or "chacha20-poly1305") |
| `encrypt(alg, key, plaintext, aad?)` | Encrypt with given algorithm |
| `decrypt(alg, key, ciphertext, aad?)` | Decrypt with given algorithm |
| `encrypt_aes_gcm(key, plaintext, aad?)` | AES-256-GCM encrypt |
| `decrypt_aes_gcm(key, ciphertext, aad?)` | AES-256-GCM decrypt |
| `encrypt_chacha20_poly1305(key, plaintext, aad?)` | ChaCha20-Poly1305 encrypt |
| `decrypt_chacha20_poly1305(key, ciphertext, aad?)` | ChaCha20-Poly1305 decrypt |

```zen
import crypto

let key = crypto.generate_key("aes-256-gcm")
let ct = crypto.encrypt_aes_gcm(key, "hello world")
let pt = crypto.decrypt_aes_gcm(key, ct)
print(pt)  # hello world

let key2 = crypto.generate_key("chacha20-poly1305")
let ct2 = crypto.encrypt_chacha20_poly1305(key2, "secret")
let pt2 = crypto.decrypt_chacha20_poly1305(key2, ct2)
print(pt2)  # secret
```

### With associated data (AAD)

AAD is authenticated but not encrypted. If you tamper with the ciphertext or AAD, decryption raises an error:

```zen
import crypto

let key = crypto.generate_key("aes-256-gcm")
let aad = "header-data"

let ct = crypto.encrypt_aes_gcm(key, "secret", aad)
let pt = crypto.decrypt_aes_gcm(key, ct, aad)
print(pt)  # secret

# Tampered AAD fails
try {
    crypto.decrypt_aes_gcm(key, ct, "tampered-aad")
} catch as e {
    print(e.msg)  # authentication failed
}
```

### Generic encrypt/decrypt

```zen
import crypto

let key = crypto.generate_key("aes-256-gcm")

let ct = crypto.encrypt("aes-256-gcm", key, "plaintext")
let pt = crypto.decrypt("aes-256-gcm", key, ct)
print(pt)  # plaintext
```

## 25.2 Asymmetric encryption (RSA)

```zen
import crypto

let keys = crypto.generate_rsa(2048)
let public_key = keys.public_key
let private_key = keys.private_key

let ciphertext = crypto.rsa_encrypt(public_key, "secret")
let plaintext = crypto.rsa_decrypt(private_key, ciphertext)
print(plaintext)  # secret
```

### Asymmetric functions

| Function | Description |
|----------|-------------|
| `generate_rsa(bits?)` | Generate RSA keypair (default 2048) |
| `rsa_encrypt(public_key, plaintext)` | RSA encrypt |
| `rsa_decrypt(private_key, ciphertext)` | RSA decrypt |
| `rsa_sign(private_key, message)` | RSA sign |
| `rsa_verify(public_key, message, sig)` | RSA verify → bool |

```zen
import crypto

let keys = crypto.generate_rsa(2048)

print(keys.public_key)   # PEM public key
print(keys.private_key)  # PEM private key

let ct = crypto.rsa_encrypt(keys.public_key, "top secret")
let pt = crypto.rsa_decrypt(keys.private_key, ct)
print(pt)  # top secret
```

RSA encryption has size limits (key size / padding overhead). For large data, use hybrid encryption: encrypt data with a random symmetric key, encrypt the key with RSA.

## 25.3 Digital signatures

### RSA sign/verify

```zen
import crypto

let keys = crypto.generate_rsa(2048)
let message = "hello world"

let sig = crypto.rsa_sign(keys.private_key, message)
let valid = crypto.rsa_verify(keys.public_key, message, sig)
print(valid)  # true

# Tampered message fails
let invalid = crypto.rsa_verify(keys.public_key, "tampered", sig)
print(invalid)  # false
```

### Ed25519 sign/verify

```zen
import crypto

let keys = crypto.generate_ed25519()
let message = "hello world"

let sig = crypto.ed25519_sign(keys.private_key, message)
let valid = crypto.ed25519_verify(keys.public_key, message, sig)
print(valid)  # true

# Tampered fails
let invalid = crypto.ed25519_verify(keys.public_key, "tampered", sig)
print(invalid)  # false
```

### Signature functions

| Function | Description |
|----------|-------------|
| `rsa_sign(private_key, message)` | RSA sign → signature |
| `rsa_verify(public_key, message, sig)` | RSA verify → bool |
| `ed25519_sign(private_key, message)` | Ed25519 sign → signature |
| `ed25519_verify(public_key, message, sig)` | Ed25519 verify → bool |

```zen
import crypto

let keys = crypto.generate_ed25519()
let msg = "important document"

let sig = crypto.ed25519_sign(keys.private_key, msg)

# Anyone with the public key can verify
let ok1 = crypto.ed25519_verify(keys.public_key, msg, sig)
print(ok1)  # true

let ok2 = crypto.ed25519_verify(keys.public_key, "tampered", sig)
print(ok2)  # false
```

## 25.4 Key agreement (X25519)

```zen
import crypto

let alice = crypto.generate_x25519()
let bob = crypto.generate_x25519()

let alice_secret = crypto.x25519_shared_secret(alice.private_key, bob.public_key)
let bob_secret = crypto.x25519_shared_secret(bob.private_key, alice.public_key)

print(alice_secret == bob_secret)  # true (shared secret matches)
```

### X25519 functions

| Function | Description |
|----------|-------------|
| `generate_x25519()` | Generate X25519 keypair |
| `x25519_shared_secret(private_key, public_key)` | Compute shared secret |

```zen
import crypto

# Two parties establish a shared secret without transmitting it
let alice = crypto.generate_x25519()
let bob = crypto.generate_x25519()

// Alice computes: her private key + Bob's public key
let alice_shared = crypto.x25519_shared_secret(alice.private_key, bob.public_key)
// Bob computes: his private key + Alice's public key
let bob_shared = crypto.x25519_shared_secret(bob.private_key, alice.public_key)

print(alice_shared == bob_shared)  # true
```

## 25.5 Hashing and HMAC

```zen
import crypto

print(crypto.hash("sha256", "hello"))
print(crypto.hmac("sha256", "key", "message"))
```

Hash algorithms: `md5`, `sha1`, `sha224`, `sha256`, `sha384`, `sha512`, `sha3_256`, `sha3_512`, `blake2b`, `blake2s`.

HMAC algorithms: `sha256`, `sha512`.

```zen
import crypto

print(crypto.hash("sha256", "hello"))     # 2cf24dba...
print(crypto.hash("sha512", "hello"))     # longer hex
print(crypto.hash("blake2b", "hello"))    # BLAKE2b
print(crypto.hash("md5", "hello"))        # MD5 (broken, don't use for security)

let mac = crypto.hmac("sha256", "secret-key", "message")
print(mac)  # HMAC-SHA256 hex

let mac2 = crypto.hmac("sha512", "secret-key", "message")
print(mac2)  # HMAC-SHA512 hex
```

HMAC for message authentication:
```zen
import crypto

let key = "shared-secret"
let message = "authenticate this"

let mac = crypto.hmac("sha256", key, message)

# Receiver with same key verifies
let expected = crypto.hmac("sha256", key, message)
print(mac == expected)  # true

let wrong = crypto.hmac("sha256", "wrong-key", message)
print(mac == wrong)  # false
```

## 25.6 Key derivation (PBKDF2)

```zen
import crypto

let key = crypto.pbkdf2("password", "salt", 100000, 32)
print(key)  # derived key (hex)
```

### PBKDF2

```zen
import crypto

# Derive a key from a password
let derived = crypto.pbkdf2("my-password", "random-salt", 100000, 32)
print(derived)  # 64 hex chars (32 bytes)

# More iterations = slower but more secure
let slow = crypto.pbkdf2("password", "salt", 1000000, 32)
```

Use PBKDF2 for password hashing (though Argon2/scrypt are preferred in modern systems). Always use a unique random salt per password.

## 25.7 JWT (JSON Web Tokens)

```zen
import crypto

let payload = {"sub": "123", "name": "Zen"}
let secret = "my-secret"

let token = crypto.jwt_encode(payload, secret)
print(token)  # eyJ...

let decoded = crypto.jwt_decode(token, secret)
print(decoded["sub"])  # 123
```

### JWT functions

| Function | Description |
|----------|-------------|
| `jwt_encode(payload, secret)` | Create JWT |
| `jwt_decode(token, secret)` | Verify and decode JWT |

```zen
import crypto

let payload = {"sub": "user-42", "role": "admin", "exp": 9999999999}
let secret = "super-secret"

let token = crypto.jwt_encode(payload, secret)
print(token)  # eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...

let decoded = crypto.jwt_decode(token, secret)
print(decoded["sub"])    # user-42
print(decoded["role"])   # admin

# Tampered token fails
try {
    crypto.jwt_decode(token, "wrong-secret")
} catch as e {
    print(e.msg)  # signature verification failed
}
```

## 25.8 OTP (TOTP/HOTP)

```zen
import crypto

let totp = crypto.otp_totp("secret")
print(totp)  # 6-digit code

let valid = crypto.otp_verify("secret", "123456")
print(valid)  # true/false
```

### OTP functions

| Function | Description |
|----------|-------------|
| `otp_totp(secret)` | Generate TOTP code |
| `otp_verify(secret, code)` | Verify TOTP code → bool |

```zen
import crypto

let secret = "JBSWY3DPEHPK3PXP"

let code = crypto.otp_totp(secret)
print(code)  # 6-digit time-based code

let valid = crypto.otp_verify(secret, code)
print(valid)  # true (if within time window)

let invalid = crypto.otp_verify(secret, "000000")
print(invalid)  # false
```

TOTP codes change every 30 seconds. `otp_verify` accepts codes within a small time window (±1 step) to account for clock skew.

## 25.9 X.509 certificates

```zen
import crypto

let cert = crypto.x509_parse(cert_pem)
print(cert.subject)
print(cert.issuer)
print(cert.not_after)
```

### X.509 functions

| Function | Description |
|----------|-------------|
| `x509_parse(pem)` | Parse PEM certificate |

```zen
import crypto

let pem = "-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----"

let cert = crypto.x509_parse(pem)
print(cert.subject)    # subject DN
print(cert.issuer)     # issuer DN
print(cert.not_after)  # expiry
print(cert.not_before) # validity start
```

## 25.10 Constant-time compare and secure zero

```zen
import crypto

# Timing-safe comparison (for HMACs, tokens, passwords)
let ok = crypto.constant_time_eq(expected, received)
print(ok)  # true/false

# Zeroize a secret after use
let pass = read_secret()
# ... use pass ...
crypto.secure_zero(pass)
pass = nil
```

```zen
import crypto

// Constant-time comparison prevents timing attacks
let expected = compute_hmac(key, message)
let received = hmac_from_request()

if crypto.constant_time_eq(expected, received) {
    print("authenticated")
} else {
    print("rejected")
}

// Secure zero - overwrite secret in memory
let password = read_user_password()
authenticate(password)
crypto.secure_zero(password)
password = nil
```

## 25.11 Algorithm policy (FIPS-ish mode)

```zen
import crypto

crypto.policy("strict")   # block md5, sha1, weak ciphers
crypto.policy("default")  # allow all
```

Or set `ZEN_FIPS=1` in the environment.

```zen
import crypto

// Check current policy behavior
try {
    crypto.hash("md5", "data")  // raises if strict
} catch as e {
    print(e.msg)  // blocked by crypto policy
}

crypto.policy("strict")
try {
    crypto.hash("md5", "data")  // still blocked
} catch as e {
    print("blocked as expected")
}

crypto.policy("default")
print(crypto.hash("md5", "data"))  # works again
```

### Blocked algorithms in strict mode

| Algorithm | Blocked in strict? |
|-----------|-------------------|
| md5 | yes |
| sha1 | yes |
| sha224 | yes |
| sha256 | no |
| sha384 | no |
| sha512 | no |
| sha3_256 | no |
| sha3_512 | no |
| blake2b | no |
| blake2s | no |
| des | yes |
| rc4 | yes |

## 25.12 Quick reference

```zen
import crypto

# Symmetric
let key = crypto.generate_key("aes-256-gcm")
let ct = crypto.encrypt_aes_gcm(key, "secret")
let pt = crypto.decrypt_aes_gcm(key, ct)

# Asymmetric
let keys = crypto.generate_rsa(2048)
let ct = crypto.rsa_encrypt(keys.public_key, "secret")
let pt = crypto.rsa_decrypt(keys.private_key, ct)

# Signatures
let sig = crypto.ed25519_sign(keys.private_key, "msg")
let ok = crypto.ed25519_verify(keys.public_key, "msg", sig)

# Key agreement
let shared = crypto.x25519_shared_secret(a.private_key, b.public_key)

# Hashing
print(crypto.hash("sha256", "hello"))
print(crypto.hmac("sha256", "key", "msg"))

# KDF
let derived = crypto.pbkdf2("password", "salt", 100000, 32)

# JWT
let token = crypto.jwt_encode({"sub": "1"}, "secret")
let decoded = crypto.jwt_decode(token, "secret")

# Security utils
crypto.constant_time_eq(a, b)
crypto.secure_zero(secret)
crypto.policy("strict")
```

## 25.13 Exercises

1. Encrypt and decrypt a message with AES-256-GCM.
2. Generate an Ed25519 keypair, sign a message, and verify it.
3. Perform X25519 key agreement between two parties and confirm shared secrets match.
4. Create a JWT, decode it, and verify tampering fails.
5. Generate a TOTP code and verify it.
6. Write `hash_password(password, salt)` using PBKDF2.
7. Write `verify_password(password, salt, expected_hash)` using PBKDF2.
8. Demonstrate `crypto.constant_time_eq` vs regular `==` for timing safety.


## 25.12 crypto module — advanced reference

### Hashing

| Function | Purpose |
|----------|---------|
| `sha256/384/512`, `sha3_256/512`, `blake3`, `md5`, `sha1` | cryptographic hashes |

### HMAC

| Function | Purpose |
|----------|---------|
| `sha256(key, data)` / `sha512(key, data)` | keyed hash |

### Symmetric encryption

| Function | Purpose |
|----------|---------|
| `generate_key(alg)` | generate a key |
| `encrypt(alg, key, plaintext, aad="")` | encrypt |
| `decrypt(alg, key, ciphertext, aad="")` | decrypt |
| `encrypt_aes_gcm(key, plaintext, aad="")` | AES-GCM |
| `decrypt_aes_gcm(key, ciphertext, aad="")` | AES-GCM |
| `encrypt_chacha20_poly1305(...)` | ChaCha20-Poly1305 |
| `decrypt_chacha20_poly1305(...)` | ChaCha20-Poly1305 |

### Asymmetric (public key)

| Function | Purpose |
|----------|---------|
| `generate_rsa(bits=2048)` | RSA keypair |
| `rsa_encrypt(pub, pt)` / `rsa_decrypt(priv, ct)` | RSA encrypt/decrypt |
| `rsa_sign(priv, msg)` / `rsa_verify(pub, msg, sig)` | RSA sign/verify |
| `generate_ed25519()` | Ed25519 keypair |
| `ed25519_sign(priv, msg)` / `ed25519_verify(pub, msg, sig)` | Ed25519 |
| `generate_x25519()` | X25519 keypair |
| `x25519_shared_secret(priv, pub)` | Diffie-Hellman shared secret |

### Encoding

| Function | Purpose |
|----------|---------|
| `b64encode/decode`, `b64urlencode/decode`, `hex_encode/decode` | encodings |

### JWT

| Function | Purpose |
|----------|---------|
| `sign(payload, key, alg="HS256")` | sign JWT |
| `verify(token, key, algorithms=["HS256"])` | verify JWT |
| `decode(token)` | decode JWT (no verify) |

### KDF / passwords

| Function | Purpose |
|----------|---------|
| `pbkdf2(password, salt, iterations, length)` | PBKDF2 |
| `argon2id(password, salt, memory, iterations, parallelism, length)` | Argon2id |
| `argon2id_hash(password)` | hash password |
| `verify_argon2id(password, hash)` | verify password hash |

### OTP

| Function | Purpose |
|----------|---------|
| `totp(secret, time_step, digits)` | TOTP token |
| `totp_verify(secret, token, window, time_step, digits)` | verify TOTP |
| `hotp(secret, counter, digits)` | HOTP token |
| `hotp_verify(secret, token, counter, window, digits)` | verify HOTP |
| `generate_secret()` / `bytes(n)` / `hex(n)` / `base64(n)` / `uuid4()` / `secure_token()` | randomness |

### X.509

| Function | Purpose |
|----------|---------|
| `parse(pem)` | parse certificate |

### Usage

```zen
import crypto
let key = crypto.generate_key("aes-256-gcm")
let ct = crypto.encrypt_aes_gcm(key, "secret message")
let pt = crypto.decrypt_aes_gcm(key, ct)
print(pt)   # secret message

let pair = crypto.generate_ed25519()
let sig = crypto.ed25519_sign(pair.private_key, "message")
print(crypto.ed25519_verify(pair.public_key, "message", sig))  # true

let token = crypto.sign(#{sub: "alice"}, "my-secret")
let payload = crypto.verify(token, "my-secret")
print(payload["sub"])   # alice

let pw_hash = crypto.argon2id_hash("hunter2")
print(crypto.verify_argon2id("hunter2", pw_hash))   # true
print(crypto.verify_argon2id("wrong", pw_hash))     # false

print(crypto.totp("JBSWY3DPEHPK3PXP"))   # 6-digit token
```

### Security best practices

1. Never use MD5/SHA-1 for security; use SHA-256 or better.
2. Use Argon2id or PBKDF2 for password hashing (with salt).
3. Use AES-GCM or ChaCha20-Poly1305 for encryption (authenticated).
4. Verify JWT signatures before trusting payloads.
5. Use `crypto.secure_token`/`secrets`, not `random`, for tokens.
6. Rotate keys regularly; never hardcode secrets.
7. Constant-time comparison for secrets.

### Exercises

1. Encrypt/decrypt a string with AES-GCM; verify round-trip.
2. Sign/verify a message with Ed25519.
3. Generate an RSA keypair; encrypt with public, decrypt with private.
4. Compute `x25519_shared_secret` for two keypairs; both sides match.
5. Sign a JWT and verify it; tamper with payload and confirm verification fails.
6. Hash a password with Argon2id and verify correct/wrong passwords.
7. Generate a TOTP token and verify it within the time window.
8. Compute HMAC-SHA256 of a message with a key.
9. Compare timing of constant-time vs naive comparison (conceptually).
10. Build a small "vault" CLI: encrypt a file, decrypt with passphrase.

