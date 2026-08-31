# base32 — Base32 encoding

Base32 encoding and decoding as defined in RFC 4648. This module is globally available as `base32`.

```zen
# Encoding a string
var encoded = base32.encode("hello")
print(encoded)  # NBSWY3DP

# Decoding back to a string
var decoded = base32.decode("NBSWY3DP")
print(decoded)  # hello
```

## Functions

| Function | Description |
|----------|-------------|
| `encode(text)` | Encodes a string into Base32. |
| `decode(text)` | Decodes a Base32 string back to its original form. |

## Examples

### Working with binary-like data
While Zen strings are UTF-8, `base32` can be used to represent any data in a human-readable format that is case-insensitive and safe for filesystems.

```zen
var secret = "top secret data"
var b32 = base32.encode(secret)
print("Safe for storage: ${b32}")
```

## See Also
- [base64](base64.md) — For Base64 encoding.
- [binascii](binascii.md) — For hex and other binary conversions.
