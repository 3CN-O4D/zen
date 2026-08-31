# hashlib — Cryptographic hashing

The `hashlib` module provides a high-level interface for secure hashes and message digests. It is available globally as `hashlib`.

```zen
# 1. Quick SHA256
print(hashlib.sha256("data to hash"))

# 2. Using the creator for generic hashing
var h = hashlib.create("md5")
h.update("chunk 1")
h.update("chunk 2")
print(h.digest())
```

## Functions

| Function | Description |
|----------|-------------|
| `sha256(data)` | Quick SHA-256 digest of the data. |
| `md5(data)` | Quick MD5 digest. |
| `sha512(data)` / `sha1(data)` | Other common algorithms. |
| `create(algo)` | Creates a hash object for incremental updates. |
| `pbkdf2_hmac(...)` | Password-based key derivation. |
| `algorithms_available()` | Returns a list of supported hash algorithms. |

## The Hash Object
When you use `hashlib.create(name)`, it returns an object with these methods:
- `update(data)`: Append more data to the hash.
- `digest()`: Get the final hex-encoded digest string.

## Examples

### Hashing a file
```zen
var h = hashlib.create("sha256")
var content = fs.read("file.txt")
h.update(content)
print("File hash: ${h.digest()}")
```

## See Also
- [crypto](crypto.md) — Low-level cryptographic functions.
- [binascii](binascii.md) — For hex and base64 conversions.
