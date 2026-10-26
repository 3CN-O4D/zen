# Chapter 25: crypto Module — Complete Guide

The `crypto` module provides symmetric encryption, asymmetric keys, hashing,
HMAC, key derivation, JWT, OTP, and X.509. This chapter covers the complete
crypto API.

> WARNING:️ Warning: Cryptography is easy to get wrong. Use well-established
> algorithms and modes. Never roll your own crypto for production security.

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

### With associated data (AAD)

```zen
import crypto

let key = crypto.generate_key("aes-256-gcm")
let aad = "header-data"

let ct = crypto.encrypt_aes_gcm(key, "secret", aad)
let pt = crypto.decrypt_aes_gcm(key, ct, aad)
print(pt)  # secret
```

If you tamper with the ciphertext or AAD, decryption raises an error.

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

## 25.5 Hashing and HMAC

```zen
import crypto

print(crypto.hash("sha256", "hello"))
print(crypto.hmac("sha256", "key", "message"))
```

Hash algorithms: `md5`, `sha1`, `sha224`, `sha256`, `sha384`, `sha512`,
`sha3_256`, `sha3_512`, `blake2b`, `blake2s`.

HMAC algorithms: `sha256`, `sha512`.

## 25.6 Key derivation (PBKDF2)

```zen
import crypto

let key = crypto.pbkdf2("password", "salt", 100000, 32)
print(key)  # derived key (hex)
```

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

## 25.8 OTP (TOTP/HOTP)

```zen
import crypto

let totp = crypto.otp_totp("secret")
print(totp)  # 6-digit code

let valid = crypto.otp_verify("secret", "123456")
print(valid)  # true/false
```

## 25.9 X.509 certificates

```zen
import crypto

let cert = crypto.x509_parse(cert_pem)
print(cert.subject)
print(cert.issuer)
print(cert.not_after)
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

## 25.11 Algorithm policy (FIPS-ish mode)

```zen
import crypto

crypto.policy("strict")   # block md5, sha1, weak ciphers
crypto.policy("default")  # allow all
```

Or set `ZEN_FIPS=1` in the environment.

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
