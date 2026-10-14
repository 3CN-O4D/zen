# binascii — binary/ASCII conversions

Binary-to-text helpers in the spirit of Python's `binascii`.

```zen
import binascii

print binascii.hexlify("hi")  # 6869
```

## Functions

| Function | Description |
|----------|-------------|
| `hexlify(data)` | Bytes/string to lowercase hex. |
| `unhexlify(hex)` | Hex back to a string. |
| `b2a_base64(data)` | Bytes to Base64 ASCII. |
| `a2b_base64(data)` | Base64 ASCII back to bytes. |

## Example

```zen
import binascii

print binascii.hexlify("Zen")      # 5a656e
print binascii.unhexlify("5a656e") # Zen
```
