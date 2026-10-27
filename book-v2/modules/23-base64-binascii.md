# Chapter 23: base64 & binascii

The `base64` and `binascii` modules provide binary-to-text encoding: Base64, hex, and other binary representations.

## 23.1 base64

```zen
import base64

# Encode
let encoded = base64.b64encode("hello")
print(encoded)  # aGVsbG8=

# Decode
let decoded = base64.b64decode("aGVsbG8=")
print(decoded)  # hello

# URL-safe variant
let encoded_url = base64.b64encode_url("hello?x=1&y=2")
print(encoded_url)
let decoded_url = base64.b64decode_url(encoded_url)
print(decoded_url)
```

### Functions

| Function | Description |
|----------|-------------|
| `b64encode(data)` | Base64 encode |
| `b64decode(data)` | Base64 decode |
| `b64encode_url(data)` | URL-safe Base64 encode |
| `b64decode_url(data)` | URL-safe Base64 decode |

```zen
import base64

print(base64.b64encode("hello"))     # aGVsbG8=
print(base64.b64decode("aGVsbG8="))  # hello
print(base64.b64encode(""))          # ""
print(base64.b64decode(""))          # ""
```

Base64 encoding of binary data:
```zen
import base64
import fs

let data = fs.read_binary("image.png")
let encoded = base64.b64encode(data)
print(encoded)  # long base64 string

let decoded = base64.b64decode(encoded)
print(len(decoded) == len(data))  # true
```

## 23.2 binascii

```zen
import binascii

# Hex encode / decode
let hexed = binascii.hexlify("hello")
print(hexed)  # 68656c6c6f

let unhexed = binascii.unhexlify("68656c6c6f")
print(unhexed)  # hello
```

### Functions

| Function | Description |
|----------|-------------|
| `hexlify(data)` | Hex encode |
| `unhexlify(data)` | Hex decode |

```zen
import binascii

print(binascii.hexlify("hello"))      # 68656c6c6f
print(binascii.hexlify(""))           # ""
print(binascii.unhexlify("68656c6c6f"))  # hello
print(binascii.unhexlify(""))         # ""
```

Hex encoding produces lowercase hex digits:
```zen
print(binascii.hexlify("AB"))         # 4142
print(binascii.hexlify("ab"))         # 6162
```

`unhexlify` requires an even number of hex digits:
```zen
try {
    binascii.unhexlify("abc")  # odd length
} catch as e {
    print(e.msg)  # odd-length string
}
```

## 23.3 Common patterns

### Encode binary for safe transport

```zen
import base64
import fs

let data = fs.read_binary("image.png")
let encoded = base64.b64encode(data)
# Send `encoded` over text-based protocols (email, JSON, URLs)
```

### Decode received base64

```zen
import base64
import fs

let encoded = "iVBORw0KGgoAAAANSUhEUg..."  # received from network
let data = base64.b64decode(encoded)
fs.write_binary("out.png", data)
```

### Hex dump

```zen
import binascii

let data = fs.read_binary("file.bin")
let hexed = binascii.hexlify(data)
# Print in 16-byte lines
for i in 0..len(hexed) step 32 {
    print(hexed[i:i+32])
}
```

### Round-trip verification

```zen
import base64

let original = "binary data \x00\x01\x02"
let encoded = base64.b64encode(original)
let decoded = base64.b64decode(encoded)
print(decoded == original)  # true
```

```zen
import binascii

let original = "binary data"
let hexed = binascii.hexlify(original)
let decoded = binascii.unhexlify(hexed)
print(decoded == original)  # true
```

## 23.4 Quick reference

```zen
import base64
import binascii

base64.b64encode("hello")     # aGVsbG8=
base64.b64decode("aGVsbG8=")  # hello
base64.b64encode_url("a/b")   # use - and _ instead of + and /

binascii.hexlify("hello")     # 68656c6c6f
binascii.unhexlify("68656c6c6f")  # hello
```

## 23.5 Exercises

1. Write a function `encode_file(path)` that base64-encodes a file's contents.
2. Write `decode_file(encoded, path)` that decodes base64 and writes to a file.
3. Write `hex_dump(data)` that prints a hex dump with ASCII side column.
4. Write `safe_transfer(data)` that base64-encodes for URL-safe transport.
5. Write `round_trip(data)` that base64-encodes then decodes and verifies.
6. Write `is_valid_hex(s)` that checks if a string is valid hexadecimal.
7. Write `hex_to_bytes(s)` that converts hex to a list of byte values.
