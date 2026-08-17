# Crypto Module (`crypto`)

Cryptographic hashing and encryption.

```zen
crypto.sha256("hello")
crypto.sha1("hello")
crypto.md5("hello")
crypto.sha512("hello")
crypto.sha224("hello")
crypto.sha384("hello")
crypto.sha3_256("hello")
crypto.sha3_512("hello")
crypto.blake2b("hello")
crypto.blake2s("hello")
crypto.hmac_sha256("key", "msg")
crypto.random_bytes(16)
crypto.random_hex(8)
crypto.pbkdf2("password", "salt", 100000, 32)
crypto.aes_encrypt("key", "data")
crypto.aes_decrypt("key", "encrypted")
```
