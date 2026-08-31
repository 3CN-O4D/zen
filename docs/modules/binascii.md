# binascii — Binary/ASCII conversion

The `binascii` module provides functions for converting between binary data and various ASCII representations (hex, base64). It is available globally as `binascii`.

```zen
# 1. Hex conversion
var h = binascii.hexlify("abc")
print(h)  # 616263

var s = binascii.unhexlify("616263")
print(s)  # abc

# 2. Base64 (low-level)
print(binascii.b2a_base64("hello")) # aGVsbG8=
```

## Functions

| Function | Description |
|----------|-------------|
| `hexlify(s)` | Converts a string to its hex representation. |
| `unhexlify(h)` | Converts a hex string back to its original form. |
| `b2a_base64(s)` | "Binary to ASCII" — Base64 encodes a string. |
| `a2b_base64(b)` | "ASCII to Binary" — Decodes a Base64 string. |

## See Also
- [base64](base64.md) — High-level Base64 encoding.
- [base32](base32.md) — Base32 encoding.
- [crypto](crypto.md) — For random hex generation.
