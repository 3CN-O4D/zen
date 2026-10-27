# Chapter 59: Cookbook — Crypto & Security

Complete recipes for hashing, encryption, signing, JWTs, passwords, and TOTP.

## 59.1 Recipe: SHA-256 hash of a string

```zen
import hashlib
print(hashlib.sha256("hello"))   # 2cf24dba... (hex digest)
```

Expected output:
```
2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
```

## 59.2 Recipe: SHA-256 of a file

```zen
import hashlib
func file_hash(path) {
    return hashlib.sha256(read_all(path))
}
print(file_hash("document.pdf"))
```

## 59.3 Recipe: Verify a file against an expected hash

```zen
func verify_file(path, expected) {
    let actual = file_hash(path)
    if actual == expected { print("MATCH") }
    else { print("MISMATCH"); return false }
    return true
}
verify_file("document.pdf", "2cf24dba...")
```

## 59.4 Recipe: AES-GCM encrypt/decrypt

```zen
import crypto
let key = crypto.generate_key("aes-256-gcm")
let plaintext = "secret message"
let ct = crypto.encrypt_aes_gcm(key, plaintext)
let pt = crypto.decrypt_aes_gcm(key, ct)
print(pt)   # secret message
```

Expected output:
```
secret message
```

## 59.5 Recipe: Ed25519 sign and verify

```zen
import crypto
let pair = crypto.generate_ed25519()
let msg = "hello world"
let sig = crypto.ed25519_sign(pair.private_key, msg)
print(crypto.ed25519_verify(pair.public_key, msg, sig))   # true
print(crypto.ed25519_verify(pair.public_key, "tampered", sig))   # false
```

Expected output:
```
true
false
```

## 59.6 Recipe: RSA encrypt with public key, decrypt with private key

```zen
import crypto
let pair = crypto.generate_rsa(2048)
let ct = crypto.rsa_encrypt(pair.public_key, "secret")
let pt = crypto.rsa_decrypt(pair.private_key, ct)
print(pt)   # secret
```

## 59.7 Recipe: X25519 shared secret (both sides match)

```zen
import crypto
let a = crypto.generate_x25519()
let b = crypto.generate_x25519()
let sa = crypto.x25519_shared_secret(a.private_key, b.public_key)
let sb = crypto.x25519_shared_secret(b.private_key, a.public_key)
print(sa == sb)   # true
```

Expected output:
```
true
```

## 59.8 Recipe: Sign and verify a JWT (HS256)

```zen
import crypto
let token = crypto.sign(#{sub: "alice", role: "admin"}, "my-secret")
let payload = crypto.verify(token, "my-secret", ["HS256"])
print(payload["sub"])   # alice
# tampering fails:
# crypto.verify(token, "wrong-key", ["HS256"])  # throws
```

Expected output:
```
alice
```

## 59.9 Recipe: Hash a password with Argon2id and verify

```zen
import crypto
let hash = crypto.argon2id_hash("hunter2")
print(crypto.verify_argon2id("hunter2", hash))   # true
print(crypto.verify_argon2id("wrong", hash))     # false
```

Expected output:
```
true
false
```

## 59.10 Recipe: PBKDF2 key derivation

```zen
import crypto
let key = crypto.pbkdf2("password", "salt", 100000, 32)
print(key)   # derived key string
```

## 59.11 Recipe: Generate a TOTP token and verify

```zen
import crypto
let token = crypto.totp("JBSWY3DPEHPK3PXP")
print(token)   # 6-digit code
print(crypto.totp_verify("JBSWY3DPEHPK3PXP", token))   # true (within window)
```

Expected output:
```
742193
true
```

## 59.12 Recipe: Generate a secure random token

```zen
import crypto
print(crypto.secure_token())   # e.g. aB3xK9mP2vR5wL8n
```

Never use `random` for tokens; always use `crypto.secure_token`.

## 59.13 Recipe: HMAC-SHA256 of a message

```zen
import crypto
let mac = crypto.sha256("my-key", "the message")
print(mac)
```

## 59.14 Recipe: Parse an X.509 certificate (PEM)

```zen
import crypto
let cert = crypto.parse(pem_string)
# inspect cert fields (subject, issuer, validity) per build
```

## 59.15 Security checklist

1. Use SHA-256+ (never MD5/SHA-1 for security).
2. Use Argon2id or PBKDF2 for passwords (with salt).
3. Use AES-GCM or ChaCha20-Poly1305 for encryption.
4. Verify JWT signatures before trusting payloads.
5. Use `crypto.secure_token`, not `random`, for tokens/session IDs.
6. Never log secrets (keys, passwords, tokens).
7. Rotate keys regularly; never hardcode secrets.
8. Use constant-time comparison when checking secrets.
