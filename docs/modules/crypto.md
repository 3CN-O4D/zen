# Cryptography (crypto)

## Hashing

```
crypto.sha256("data")           // hex SHA-256
crypto.sha1("data")             // hex SHA-1
crypto.md5("data")              // hex MD5
crypto.sha512("data")           // hex SHA-512
crypto.sha224("data")
crypto.sha384("data")
crypto.sha3_256("data")
crypto.sha3_512("data")
crypto.blake2b("data")
crypto.blake2s("data")
```

## HMAC

```
crypto.hmac_sha256("key", "data")   // hex HMAC-SHA256
crypto.hmac_sha1("key", "data")
crypto.hmac_md5("key", "data")
```

## PBKDF2

```
crypto.pbkdf2("password", "salt")                   // 100000 iterations, 32 bytes
crypto.pbkdf2("password", "salt", 10000, 16)         // custom iterations & length
```

## Random

```
crypto.random_bytes(16)          // 16 random bytes as hex
crypto.random_hex(8)             // 8 random hex characters
```

## AES Encryption

```
crypto.aes_encrypt(key, data)           // AES-256-CBC encrypt, returns hex (iv + ct)
crypto.aes_encrypt(key, data, iv)       // with explicit IV
crypto.aes_decrypt(key, hex_token)      // AES-256-CBC decrypt
crypto.aes_decrypt(key, hex_token, iv)  // with explicit IV
```

## Fernet Encryption

```
cryptography.fernet.generate_key()                        // generate a random key
let key = cryptography.fernet.generate_key()
let token = cryptography.fernet.encrypt(key, "secret data")
cryptography.fernet.decrypt(key, token)                    // "secret data"
```
