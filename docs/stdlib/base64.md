# base64 — Base64 encoding

Base64 codecs in the spirit of Python's `base64`.

```zen
import base64

print base64.encode("hi")  # aGk=
```

## Functions

| Function | Description |
|----------|-------------|
| `encode(data)` | Standard Base64 (accepts a string or bytes). |
| `decode(data)` | Decodes standard Base64 back to a string. |
| `url_encode(data)` | URL-safe alphabet (`-` and `_`). |
| `url_decode(data)` | Decodes URL-safe Base64. |

## Example

```zen
import base64

let enc = base64.encode("hello")
print enc                  # aGVsbG8=
print base64.decode(enc)   # hello
```
