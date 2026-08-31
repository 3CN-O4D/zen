# base64 — Base64 encoding

Base64 encode/decode is available globally via the `base64` module (a dict, no
import needed).

```zen
var encoded = base64.encode("hello")
print(encoded)                 # aGVsbG8=

var decoded = base64.decode("aGVsbG8=")
print(decoded)                 # hello
```

## Functions

| Function | Description |
|----------|-------------|
| `encode(text)` | Base64-encode a string (standard alphabet with padding) |
| `decode(text)` | Base64-decode back to a string |

`encode` works on any string (including UTF-8); `decode` expects valid base64
and returns the decoded bytes as a string.

For binary-safe variants and URL-safe characters, also see the `binascii`
module:

```zen
print(binascii.a2b_base64("aGVsbG8="))   # bytes
print(binascii.b2a_base64("hello"))      # aGVsbG8=
```

## See also
- [binascii module docs](binascii.md)
- [base32](base32.md)