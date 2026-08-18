# Cryptography Module (`cryptography`)

Fernet symmetric encryption.

```zen
let key = cryptography.fernet.generate_key()
let token = cryptography.fernet.encrypt(key, "secret message")
let msg = cryptography.fernet.decrypt(key, token)
```
