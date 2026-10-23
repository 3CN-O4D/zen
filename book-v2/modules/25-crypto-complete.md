# Chapter 25: crypto Module - Complete Guide

The `crypto` module provides comprehensive cryptographic functionality including hashing, encryption, decryption, digital signatures, JWT, certificates, and more.

## Importing the crypto Module

```zen
import crypto
```

The crypto module is organized into submodules:
- `crypto.hash` - Hashing functions
- `crypto.hmac` - HMAC
- `crypto.symmetric` - Symmetric encryption (AES, etc.)
- `crypto.asymmetric` - Asymmetric encryption (RSA, etc.)
- `crypto.kdf` - Key derivation functions
- `crypto.rand` - Cryptographic random
- `crypto.jwt` - JSON Web Tokens
- `crypto.x509` - X.509 certificates
- `crypto.encoding` - Encoding utilities
- `crypto.otp` - One-time passwords

## Cryptographic Hashing

### Hash Functions

```zen
import crypto.hash

# SHA-256
let h = crypto.hash.sha256("Hello World")
print(h.hexdigest())

# SHA-512
let h2 = crypto.hash.sha512("Hello World")
print(h2.hexdigest())

# SHA-1 (less secure, avoid for new applications)
let h3 = crypto.hash.sha1("Hello World")
print(h3.hexdigest())

# MD5 (not cryptographically secure)
let h4 = crypto.hash.md5("Hello World")
print(h4.hexdigest())

# Blake2b
let h5 = crypto.hash.blake2b("Hello World")
print(h5.hexdigest())
```

### Hashing Files

```zen
import crypto.hash

func hash_file(filename, algorithm) {
    let content = fs.read(filename)
    if algorithm == "sha256" {
        return crypto.hash.sha256(content).hexdigest()
    }
    return null
}

print(hash_file("important.txt", "sha256"))
```

## HMAC (Hash-based Message Authentication Code)

```zen
import crypto.hmac

let key = "secret-key"
let message = "Hello World"

let mac = crypto.hmac.new(key, message, "sha256")
print(mac.hexdigest())
```

### Verifying HMAC

```zen
func verify_hmac(message, key, signature) {
    let mac = crypto.hmac.new(key, message, "sha256")
    return mac.hexdigest() == signature
}
```

## Cryptographic Random

```zen
import crypto.rand

# Random bytes
let bytes = crypto.rand.bytes(32)
print(bytes)

# Random integer
let num = crypto.rand.int(1, 100)
print(num)

# Random string
let token = crypto.rand.token(32)
print(token)  # Hex string
```

## Symmetric Encryption (AES)

Symmetric encryption uses the same key for encryption and decryption.

### AES-GCM Encryption

```zen
import crypto.symmetric
import crypto.rand

# Generate key (32 bytes for AES-256)
let key = crypto.rand.bytes(32)
let plaintext = "Secret message"

# Encrypt
let encrypted = crypto.symmetric.aes_gcm_encrypt(key, plaintext)
print("Nonce:", encrypted.nonce)
print("Ciphertext:", encrypted.ciphertext)
print("Tag:", encrypted.tag)

# Decrypt
let decrypted = crypto.symmetric.aes_gcm_decrypt(key, encrypted.nonce, 
    encrypted.ciphertext, encrypted.tag)
print("Decrypted:", decrypted)
```

### AES-CBC Encryption

```zen
import crypto.symmetric
import crypto.rand

let key = crypto.rand.bytes(32)
let iv = crypto.rand.bytes(16)
let plaintext = "Another secret message"

let ciphertext = crypto.symmetric.aes_cbc_encrypt(key, iv, plaintext)
let decrypted = crypto.symmetric.aes_cbc_decrypt(key, iv, ciphertext)
print(decrypted)
```

## Key Derivation Functions (KDF)

Used to derive encryption keys from passwords.

### PBKDF2

```zen
import crypto.kdf
import crypto.rand

let password = "my_password"
let salt = crypto.rand.bytes(16)

# Derive key
let key = crypto.kdf.pbkdf2(password, salt, iterations=100000, length=32)
print(key)
```

### Argon2 (if supported)

```zen
import crypto.kdf

let password = "my_password"
let salt = crypto.rand.bytes(16)
let key = crypto.kdf.argon2(password, salt, length=32)
print(key)
```

## Asymmetric Encryption (RSA)

Asymmetric uses public key for encryption, private key for decryption.

### Generating RSA Key Pair

```zen
import crypto.asymmetric

let keypair = crypto.asymmetric.rsa_generate(2048)
print("Public key generated")
print("Private key generated")
```

### RSA Encryption/Decryption

```zen
import crypto.asymmetric

let keypair = crypto.asymmetric.rsa_generate(2048)
let plaintext = "Secret message"

# Encrypt with public key
let ciphertext = crypto.asymmetric.rsa_encrypt(keypair.public_key, plaintext)

# Decrypt with private key
let decrypted = crypto.asymmetric.rsa_decrypt(keypair.private_key, ciphertext)
print(decrypted)
```

### RSA Signatures

```zen
import crypto.asymmetric
import crypto.hash

let keypair = crypto.asymmetric.rsa_generate(2048)
let message = "Important message"

# Sign
let signature = crypto.asymmetric.rsa_sign(keypair.private_key, message, "sha256")

# Verify
let valid = crypto.asymmetric.rsa_verify(keypair.public_key, message, signature, "sha256")
print("Valid:", valid)
```

## JSON Web Tokens (JWT)

JWTs are used for authentication and information exchange.

### Creating a JWT

```zen
import crypto.jwt

let secret = "my-secret-key"
let payload = {
    "user_id": 123,
    "username": "alice",
    "exp": time.time() + 3600  # Expires in 1 hour
}

let token = crypto.jwt.encode(payload, secret, algorithm="HS256")
print(token)
```

### Verifying and Decoding a JWT

```zen
import crypto.jwt

let token = "eyJhbGc..."
let secret = "my-secret-key"

try {
    let payload = crypto.jwt.decode(token, secret, algorithm="HS256")
    print("User ID:", payload.user_id)
    print("Valid token")
} catch (e) {
    print("Invalid token:", e)
}
```

### JWT with RS256 (Asymmetric)

```zen
import crypto.jwt
import crypto.asymmetric

let keypair = crypto.asymmetric.rsa_generate(2048)
let payload = {"user_id": 123}

let token = crypto.jwt.encode(payload, keypair.private_key, algorithm="RS256")
let decoded = crypto.jwt.decode(token, keypair.public_key, algorithm="RS256")
```

## X.509 Certificates

Working with SSL/TLS certificates.

### Loading a Certificate

```zen
import crypto.x509

let cert_pem = fs.read("certificate.crt")
let cert = crypto.x509.load_cert(cert_pem)

print("Subject:", cert.subject)
print("Issuer:", cert.issuer)
print("Valid from:", cert.not_before)
print("Valid until:", cert.not_after)
```

### Verifying Certificate

```zen
import crypto.x509

let cert = crypto.x509.load_cert(fs.read("cert.crt"))
let ca_cert = crypto.x509.load_cert(fs.read("ca.crt"))

let valid = crypto.x509.verify(cert, ca_cert)
print("Certificate valid:", valid)
```

### Getting Certificate Information

```zen
let cert = crypto.x509.load_cert(pem)
print(cert.serial_number)
print(cert.signature_algorithm)
print(cert.public_key)
```

## Base64 Encoding/Decoding

```zen
import crypto.encoding

let data = "Hello World"
let encoded = crypto.encoding.base64_encode(data)
print(encoded)  # SGVsbG8gV29ybGQ=

let decoded = crypto.encoding.base64_decode(encoded)
print(decoded)  # Hello World

# URL-safe base64
let url_encoded = crypto.encoding.base64url_encode(data)
let url_decoded = crypto.encoding.base64url_decode(url_encoded)
```

## One-Time Passwords (OTP)

### TOTP (Time-based OTP)

```zen
import crypto.otp

let secret = crypto.otp.generate_secret()
print("Secret:", secret)

# Generate TOTP code
let code = crypto.otp.totp(secret)
print("Current code:", code)

# Verify TOTP code
let valid = crypto.otp.verify_totp(secret, "123456")
print("Valid:", valid)
```

### HOTP (HCounter-based OTP)

```zen
import crypto.otp

let secret = crypto.otp.generate_secret()
let counter = 0

let code = crypto.otp.hotp(secret, counter)
print(code)
counter = counter + 1
```

## Practical Examples

### Password Hashing

```zen
import crypto.kdf
import crypto.rand

func hash_password(password) {
    let salt = crypto.rand.bytes(16)
    let hash = crypto.kdf.pbkdf2(password, salt, iterations=100000, length=32)
    return {
        "hash": hash,
        "salt": salt
    }
}

func verify_password(password, stored) {
    let hash = crypto.kdf.pbkdf2(password, stored.salt, iterations=100000, length=32)
    return hash == stored.hash
}

let stored = hash_password("mysecretpassword")
print(verify_password("mysecretpassword", stored))  # true
print(verify_password("wrongpassword", stored))     # false
```

### Secure File Encryption

```zen
import crypto.symmetric
import crypto.rand
import crypto.kdf

func encrypt_file(password, input_file, output_file) {
    let salt = crypto.rand.bytes(16)
    let key = crypto.kdf.pbkdf2(password, salt, iterations=100000, length=32)
    let plaintext = fs.read(input_file)
    let encrypted = crypto.symmetric.aes_gcm_encrypt(key, plaintext)
    
    # Save salt + nonce + ciphertext + tag
    let data = salt + encrypted.nonce + encrypted.ciphertext + encrypted.tag
    fs.write_bytes(output_file, data)
}

func decrypt_file(password, input_file, output_file) {
    let data = fs.read_bytes(input_file)
    let salt = data[0:16]
    let nonce = data[16:28]
    let tag = data[-16:]
    let ciphertext = data[28:-16]
    
    let key = crypto.kdf.pbkdf2(password, salt, iterations=100000, length=32)
    let plaintext = crypto.symmetric.aes_gcm_decrypt(key, nonce, ciphertext, tag)
    fs.write(output_file, plaintext)
}
```

### API Authentication with JWT

```zen
import crypto.jwt
import http
import json

let JWT_SECRET = "super-secret-key-change-in-production"

func auth_handler(req) {
    if req.path == "/login" && req.method == "POST" {
        let body = json.parse(req.body)
        # Validate credentials (simplified)
        if body.username == "admin" && body.password == "pass" {
            let token = crypto.jwt.encode({
                "user": body.username,
                "exp": time.time() + 3600
            }, JWT_SECRET)
            return {"status": 200, "body": json.stringify({"token": token})}
        }
        return {"status": 401, "body": json.stringify({"error": "Invalid credentials"})}
    }
    
    # Protected route
    if req.path == "/protected" {
        let auth = req.headers.get("Authorization", "")
        if auth.starts_with("Bearer ") {
            let token = auth.split(" ")[1]
            try {
                let payload = crypto.jwt.decode(token, JWT_SECRET)
                return {"status": 200, "body": json.stringify({"message": "Hello " + payload.user})}
            } catch {
                return {"status": 401, "body": json.stringify({"error": "Invalid token"})}
            }
        }
        return {"status": 401, "body": json.stringify({"error": "No token"})}
    }
    
    return {"status": 404, "body": "Not found"}
}

http.serve("0.0.0.0", 8080, auth_handler)
```

## Security Best Practices

1. **Use strong algorithms** - SHA-256+, AES-256, RSA-2048+
2. **Never use MD5/SHA-1** for security
3. **Use cryptographically secure random** - Always use `crypto.rand`
4. **Derive keys from passwords** - Use PBKDF2/Argon2, not raw hashes
5. **Use authenticated encryption** - AES-GCM provides encryption + authentication
6. **Store secrets securely** - Never hardcode in production
7. **Use HTTPS** - Always encrypt in transit
8. **Verify signatures** - Always verify JWTs and signatures
9. **Keep libraries updated** - Use latest secure versions
10. **Salt passwords** - Never hash passwords without salt

## Chapter Summary

- Crypto module provides comprehensive cryptographic tools
- Use hash functions for integrity, HMAC for authentication
- Use symmetric encryption (AES-GCM) for data encryption
- Use asymmetric (RSA) for key exchange and signatures
- Use KDFs (PBKDF2) for password-based keys
- JWT for token-based authentication
- Always follow cryptographic best practices
- Use secure random for keys and nonces

**Next:** Part 5 continues with HTTPS & SSL Certificates
