# cryptography — Fernet encryption

The `cryptography` module provides symmetric encryption via the Fernet specification. This ensures that a message encrypted cannot be read or modified without the key.

This module is available globally as `cryptography`.

```zen
# 1. Generate a key
var key = cryptography.fernet.generate_key()
print("Key: ${key}")

# 2. Encrypt a message
var message = "Sensitive information"
var token = cryptography.fernet.encrypt(key, message)
print("Token: ${token}")

# 3. Decrypt the message
var decrypted = cryptography.fernet.decrypt(key, token)
print("Decrypted: ${decrypted}")  # Sensitive information
```

## The `fernet` sub-module

All encryption functionality lives under `cryptography.fernet`.

| Function | Description |
|----------|-------------|
| `generate_key()` | Generates a new random Fernet key (base64 string). |
| `encrypt(key, data)` | Encrypts the data string using the provided key. Returns a token string. |
| `decrypt(key, token)` | Decrypts the token string using the key. Returns the original message string. |

## Security Note
Fernet uses AES-128 in CBC mode with HMAC-SHA256 for authentication. It provides **authenticated encryption**, meaning the token is tamper-proof. If the token is modified or the wrong key is used, `decrypt` will throw an error.

```zen
try {
    cryptography.fernet.decrypt(key, "invalid token")
} catch as e {
    print("Decryption failed: ${e}")
}
```

## See Also
- [crypto](crypto.md) — For hashing (SHA256, MD5).
- [hashlib](hashlib.md) — Advanced hashing functions.
