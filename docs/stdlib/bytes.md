# bytes — byte-level helpers

Byte helpers backing the binary modules (`base64`, `binascii`).

```zen
import bytes

print bytes.utf8_bytes("A")  # [65]
```

## Functions

| Function | Description |
|----------|-------------|
| `utf8_bytes(s)` | String to its UTF-8 byte list. |
| `str_from_bytes(bl)` | Byte list back to a string. |

`is_cont(b)` (continuation-byte check) is internal.
